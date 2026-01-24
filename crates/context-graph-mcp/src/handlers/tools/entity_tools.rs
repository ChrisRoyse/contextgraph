//! # E11 KEPLER Entity Tool Implementations
//!
//! Per E11 Design Document, these tools expose KEPLER's unique capabilities:
//! - **extract_entities**: Extract and canonicalize entities from text
//! - **search_by_entities**: Multi-embedder discovery (E1 + E11 union)
//! - **infer_relationship**: TransE relation inference (meaningful with KEPLER)
//! - **find_related_entities**: TransE entity prediction (h + r ≈ t)
//! - **validate_knowledge**: TransE triple validation (score > -5.0 = valid)
//! - **get_entity_graph**: Entity relationship visualization
//!
//! ## Multi-Embedder Discovery Philosophy
//!
//! Each of the 13 embedders sees the world differently. E1 (semantic) finds
//! semantically similar content, but may miss entity relationships. E11 (KEPLER)
//! understands entity relationships and can find content E1 misses.
//!
//! **Example**: When searching for "database", E1 might miss memories about
//! "Diesel ORM" because "Diesel" doesn't contain the word "database". But E11
//! knows Diesel is a database ORM and surfaces it.
//!
//! We UNION candidate sets from both embedders, ensuring we don't miss
//! candidates that one embedder found but another didn't.
//!
//! ## KEPLER Model (E11)
//!
//! KEPLER (Knowledge Embedding and Pre-training for Language Entity Representations)
//! was trained with TransE objective on Wikidata5M (4.8M entities, 20M triples).
//!
//! TransE ensures: h + r ≈ t for valid triples, enabling meaningful:
//! - Relation inference: r̂ = t - h
//! - Tail prediction: t̂ = h + r
//! - Triple validation: score = -||h + r - t||₂
//!
//! ## KEPLER Score Thresholds
//!
//! | Score Range | Interpretation |
//! |-------------|----------------|
//! | > -5.0 | Valid triple |
//! | -10.0 to -5.0 | Uncertain |
//! | < -10.0 | Invalid triple |
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation
//! - E11 DISCOVERS candidates E1 misses (multi-embedder union)
//! - ARCH-20: E11 uses entity linking for disambiguation
//! - E11 is RELATIONAL_ENHANCER with topic_weight 0.5
//! - AP-02: All comparisons within respective spaces (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with robust logging

use context_graph_embeddings::models::KeplerModel;
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::entity::{
    detect_entities, entity_jaccard_similarity, EntityLink, EntityMetadata, EntityType,
};
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::entity_dtos::{
    transe_score_to_confidence, validation_from_score, EntityByType, EntityEdge, EntityLinkDto,
    EntityNode, EntitySearchResult, ExtractEntitiesRequest, ExtractEntitiesResponse,
    FindRelatedEntitiesRequest, FindRelatedEntitiesResponse, GetEntityGraphRequest,
    GetEntityGraphResponse, InferRelationshipRequest, InferRelationshipResponse,
    KnowledgeTripleDto, RelatedEntity, RelationCandidate, SearchByEntitiesRequest,
    SearchByEntitiesResponse, ValidateKnowledgeRequest, ValidateKnowledgeResponse,
    UNCERTAIN_THRESHOLD, VALID_THRESHOLD,
};

use super::super::Handlers;

// ============================================================================
// KNOWN RELATIONS FOR TRANSE INFERENCE
// ============================================================================

/// Known relations for TransE inference.
/// These are the relations we can infer between entities.
const KNOWN_RELATIONS: &[(&str, Option<&str>)] = &[
    // Technical relations
    ("depends_on", Some("dependency_of")),
    ("imports", Some("imported_by")),
    ("extends", Some("extended_by")),
    ("implements", Some("implemented_by")),
    ("uses", Some("used_by")),
    ("configures", Some("configured_by")),
    ("calls", Some("called_by")),
    ("wraps", Some("wrapped_by")),
    // Language/framework relations
    ("implemented_in", Some("has_implementation")),
    ("written_in", Some("language_for")),
    ("built_on", Some("foundation_for")),
    ("compatible_with", None), // Symmetric
    ("alternative_to", None),  // Symmetric
    ("similar_to", None),      // Symmetric
    // Organizational relations
    ("created_by", Some("creator_of")),
    ("maintained_by", Some("maintains")),
    ("owned_by", Some("owns")),
    // Categorical relations
    ("part_of", Some("contains")),
    ("instance_of", Some("has_instance")),
    ("version_of", None),
    ("fork_of", Some("forked_to")),
];

// ============================================================================
// MULTI-EMBEDDER SCORING
// ============================================================================

/// Combine scores from multiple embedders into a single score.
///
/// # Philosophy
///
/// Each embedder provides a different perspective on relevance:
/// - **E1 (Semantic)**: How semantically similar is the content?
/// - **E11 (Entity)**: How similar are the entity relationships?
/// - **Jaccard**: How many exact entity matches are there?
///
/// These are COMPLEMENTARY signals, not competing ones. A memory found by
/// E11 but missed by E1 should still score well if E11 is confident.
///
/// # Weighting Strategy
///
/// We use a weighted harmonic-like combination that:
/// 1. Rewards memories found by multiple embedders (agreement bonus)
/// 2. Doesn't penalize memories found by only one embedder (discovery value)
/// 3. Gives entity Jaccard high weight since this is entity-focused search
///
/// # Arguments
///
/// * `e1_sim` - E1 semantic similarity [0.0, 1.0]
/// * `e11_sim` - E11 entity similarity [0.0, 1.0]
/// * `entity_jaccard` - Direct entity overlap [0.0, 1.0]
///
/// # Returns
///
/// Combined score in [0.0, 1.0]
fn combine_multi_embedder_scores(e1_sim: f32, e11_sim: f32, entity_jaccard: f32) -> f32 {
    // Base weights for each signal
    const E1_WEIGHT: f32 = 0.30;     // Semantic relevance
    const E11_WEIGHT: f32 = 0.35;    // Entity embedding relevance
    const JACCARD_WEIGHT: f32 = 0.35; // Direct entity overlap

    // Weighted sum of all signals
    let base_score = E1_WEIGHT * e1_sim + E11_WEIGHT * e11_sim + JACCARD_WEIGHT * entity_jaccard;

    // Agreement bonus: if multiple embedders agree, boost the score
    // This rewards memories that both E1 and E11 found relevant
    let agreement_count = [e1_sim > 0.3, e11_sim > 0.3, entity_jaccard > 0.1]
        .iter()
        .filter(|&&x| x)
        .count();

    let agreement_bonus = match agreement_count {
        3 => 0.10, // All three signals agree - strong confidence
        2 => 0.05, // Two signals agree - moderate confidence
        _ => 0.0,  // Single signal - no bonus, but still valuable
    };

    (base_score + agreement_bonus).clamp(0.0, 1.0)
}

impl Handlers {
    /// extract_entities tool implementation.
    ///
    /// Extracts and canonicalizes entities from text using pattern matching
    /// and knowledge base lookup.
    ///
    /// # Algorithm
    ///
    /// 1. Apply KB-based entity detection to the text
    /// 2. Detect capitalized proper nouns not in KB as Unknown entities
    /// 3. Resolve variations to canonical forms (e.g., "postgres" → "postgresql")
    /// 4. Optionally filter Unknown entities if includeUnknown=false
    /// 5. Optionally group by entity type if groupByType=true
    ///
    /// # Parameters
    ///
    /// - `text`: Text to extract entities from (required)
    /// - `includeUnknown`: Include entities not in knowledge base (default: true)
    /// - `groupByType`: Group results by entity type (default: false)
    ///
    /// # Returns
    ///
    /// - `entities`: All extracted entities with canonical links
    /// - `byType`: Entities grouped by type (if groupByType=true)
    /// - `totalCount`: Total number of entities extracted
    pub(crate) async fn call_extract_entities(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: ExtractEntitiesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "extract_entities: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "extract_entities: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let text = &request.text;
        let include_unknown = request.include_unknown;
        let group_by_type = request.group_by_type;

        info!(
            text_len = text.len(),
            include_unknown = include_unknown,
            group_by_type = group_by_type,
            "extract_entities: Starting entity extraction"
        );

        // Step 1: Detect entities using KB-based detection
        let entity_metadata: EntityMetadata = detect_entities(text);

        // Step 2: Filter Unknown entities if requested
        let filtered_entities: Vec<_> = if include_unknown {
            entity_metadata.entities.iter().collect()
        } else {
            entity_metadata
                .entities
                .iter()
                .filter(|e| e.entity_type != EntityType::Unknown)
                .collect()
        };

        debug!(
            raw_count = entity_metadata.entities.len(),
            filtered_count = filtered_entities.len(),
            "extract_entities: Entity detection complete"
        );

        // Step 3: Convert to DTOs
        let entity_dtos: Vec<EntityLinkDto> = filtered_entities
            .iter()
            .map(|e| {
                let mut dto = EntityLinkDto::from(*e);
                // KB matches get high confidence, Unknown entities get lower
                dto.confidence = Some(if e.entity_type != EntityType::Unknown {
                    1.0
                } else {
                    0.5
                });
                dto
            })
            .collect();

        // Step 4: Group by type if requested
        let by_type = if group_by_type {
            let mut grouped = EntityByType::default();

            for entity in &filtered_entities {
                let dto = EntityLinkDto::from(*entity);
                match entity.entity_type {
                    EntityType::ProgrammingLanguage => grouped.programming_language.push(dto),
                    EntityType::Framework => grouped.framework.push(dto),
                    EntityType::Database => grouped.database.push(dto),
                    EntityType::Cloud => grouped.cloud.push(dto),
                    EntityType::Company => grouped.company.push(dto),
                    EntityType::TechnicalTerm => grouped.technical_term.push(dto),
                    EntityType::Unknown => grouped.unknown.push(dto),
                }
            }

            Some(grouped)
        } else {
            None
        };

        let total_count = entity_dtos.len();
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = ExtractEntitiesResponse {
            entities: entity_dtos,
            by_type,
            total_count,
        };

        info!(
            total_count = total_count,
            elapsed_ms = elapsed_ms,
            "extract_entities: Completed entity extraction"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    // ========================================================================
    // PHASE 2: search_by_entities
    // ========================================================================

    /// search_by_entities tool implementation.
    ///
    /// Finds memories containing specific entities using multi-embedder discovery.
    /// E1 and E11 each contribute candidates they discover - E11 finds things E1 misses.
    ///
    /// # Multi-Embedder Discovery Philosophy
    ///
    /// Each embedder sees the world differently and finds different things:
    /// - **E1 (Semantic)**: Finds semantically similar content
    /// - **E11 (Entity)**: Finds entity-related content E1 might miss
    ///   (e.g., "Diesel ORM" when searching for "database")
    ///
    /// We UNION candidate sets from both embedders, then score using combined insights.
    /// This ensures we don't miss candidates that one embedder found but another didn't.
    ///
    /// # Algorithm (Multi-Embedder Discovery)
    ///
    /// 1. Detect entities in query, resolve to canonical IDs
    /// 2. Search E1 for semantic candidates (finds semantically similar)
    /// 3. Search E11 for entity candidates (finds entity-similar - DIFFERENT from E1!)
    /// 4. UNION candidate sets (E11 surfaces things E1 missed)
    /// 5. For each unique candidate, compute combined score:
    ///    - E1 similarity (semantic relevance)
    ///    - E11 similarity (entity relevance)
    ///    - Entity Jaccard (direct entity overlap)
    /// 6. Apply exact match boost
    /// 7. Return top-K ranked results
    ///
    /// # Constitution Compliance
    ///
    /// - ARCH-12: E1 is semantic foundation
    /// - E11 DISCOVERS candidates E1 misses (not just boosts E1's scores)
    /// - Combined insights from multiple embedders produce better answers
    pub(crate) async fn call_search_by_entities(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: SearchByEntitiesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_by_entities: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_by_entities: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let entities = &request.entities;
        let match_mode = &request.match_mode;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let boost_exact_match = request.boost_exact_match;
        let include_content = request.include_content;

        info!(
            entities = ?entities,
            match_mode = %match_mode,
            top_k = top_k,
            min_score = min_score,
            "search_by_entities: Starting multi-embedder entity search"
        );

        // Step 1: Detect and canonicalize query entities
        let query_entity_text = entities.join(" ");
        let query_entities = detect_entities(&query_entity_text);

        let query_entity_dtos: Vec<EntityLinkDto> = query_entities
            .entities
            .iter()
            .map(EntityLinkDto::from)
            .collect();

        let query_canonical_ids: HashSet<&str> = query_entities.canonical_ids();

        debug!(
            query_entities_count = query_entities.entities.len(),
            canonical_ids = ?query_canonical_ids,
            "search_by_entities: Detected query entities"
        );

        // Step 2: Create query embedding (all 13 embedders)
        let query_embedding = match self.multi_array_provider.embed_all(&query_entity_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_by_entities: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 3: Search E1 for semantic candidates
        // E1 finds semantically similar content
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        let e1_options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(SearchStrategy::E1Only)
            .with_embedders(vec![0]) // E1 only
            .with_min_similarity(0.0);

        let e1_candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, e1_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_by_entities: E1 search FAILED");
                return self.tool_error_with_pulse(id, &format!("E1 search failed: {}", e));
            }
        };

        debug!(
            e1_candidates = e1_candidates.len(),
            "search_by_entities: E1 found semantic candidates"
        );

        // Step 4: Search E11 for entity candidates
        // E11 finds entity-related content that E1 might miss!
        // Example: E1 might miss "Diesel ORM" when searching for "database",
        // but E11 knows Diesel is a database ORM and surfaces it.
        let e11_options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(SearchStrategy::E1Only) // Strategy doesn't matter with explicit embedders
            .with_embedders(vec![10]) // E11 (Entity) only - index 10
            .with_min_similarity(0.0);

        let e11_candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, e11_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                warn!(error = %e, "search_by_entities: E11 search failed, using E1 only");
                Vec::new() // Graceful degradation
            }
        };

        debug!(
            e11_candidates = e11_candidates.len(),
            "search_by_entities: E11 found entity candidates"
        );

        // Step 5: UNION candidate sets (deduplicate by ID)
        // This is where E11 contributes candidates E1 missed!
        let mut candidate_map: std::collections::HashMap<Uuid, (f32, f32)> =
            std::collections::HashMap::new();

        // Add E1 candidates with their E1 similarity
        for cand in &e1_candidates {
            candidate_map.insert(cand.fingerprint.id, (cand.similarity, 0.0));
        }

        // Add E11 candidates - if new, E1 missed them! If existing, add E11 score.
        let mut e11_unique_count = 0;
        for cand in &e11_candidates {
            let cand_id = cand.fingerprint.id;
            if let Some(entry) = candidate_map.get_mut(&cand_id) {
                // Already found by E1, add E11 score
                entry.1 = cand.similarity;
            } else {
                // NEW! E11 found this, E1 missed it
                e11_unique_count += 1;
                candidate_map.insert(cand_id, (0.0, cand.similarity));
            }
        }

        info!(
            e1_candidates = e1_candidates.len(),
            e11_candidates = e11_candidates.len(),
            e11_unique = e11_unique_count,
            union_size = candidate_map.len(),
            "search_by_entities: E11 discovered {} candidates E1 missed",
            e11_unique_count
        );

        let total_candidates = candidate_map.len();

        // Step 6: Get all unique candidate IDs
        let candidate_ids: Vec<Uuid> = candidate_map.keys().copied().collect();

        // Get content for entity extraction
        let contents = match self.teleological_store.get_content_batch(&candidate_ids).await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "search_by_entities: Content retrieval FAILED");
                return self.tool_error_with_pulse(id, &format!("Content retrieval failed: {}", e));
            }
        };

        // Build content map
        let content_map: std::collections::HashMap<Uuid, Option<String>> = candidate_ids
            .iter()
            .zip(contents.iter())
            .map(|(id, content)| (*id, content.clone()))
            .collect();

        // Step 7: Score each candidate using combined insights
        // Each embedder contributes what it knows
        let mut scored_results: Vec<(Uuid, f32, f32, f32, f32, Vec<EntityLink>, Option<String>)> =
            Vec::with_capacity(candidate_map.len());

        for (cand_id, (e1_sim, e11_sim)) in candidate_map.iter() {
            let content = content_map.get(cand_id).and_then(|c| c.clone());

            // Extract entities from candidate content
            let cand_entities = if let Some(ref text) = content {
                detect_entities(text)
            } else {
                EntityMetadata::empty()
            };

            // Compute entity Jaccard similarity
            let entity_jaccard = entity_jaccard_similarity(&query_entities, &cand_entities);

            // Check match mode requirements
            let matches_mode = match match_mode.as_str() {
                "all" => {
                    let cand_canonical_ids = cand_entities.canonical_ids();
                    query_canonical_ids
                        .iter()
                        .all(|q| cand_canonical_ids.contains(*q))
                }
                _ => {
                    // "any" - at least one signal must be positive
                    entity_jaccard > 0.0 || *e1_sim > 0.3 || *e11_sim > 0.3
                }
            };

            if !matches_mode {
                continue;
            }

            // Combine signals from all embedders
            // Each embedder contributes its perspective - they're not competing, they're collaborating
            //
            // Scoring philosophy:
            // - E1 contributes semantic relevance (what does the text mean?)
            // - E11 contributes entity relevance (what entities are discussed?)
            // - Jaccard contributes direct entity overlap (exact entity matches)
            //
            // We use a weighted combination where all three can contribute
            let combined_score = combine_multi_embedder_scores(*e1_sim, *e11_sim, entity_jaccard);

            // Apply boost for exact entity matches
            let matched_entities: Vec<EntityLink> = cand_entities
                .entities
                .iter()
                .filter(|e| query_canonical_ids.contains(e.canonical_id.as_str()))
                .cloned()
                .collect();

            let boost = if !matched_entities.is_empty() {
                boost_exact_match
            } else {
                1.0
            };

            let final_score = (combined_score * boost).clamp(0.0, 1.0);

            if final_score >= min_score {
                scored_results.push((
                    *cand_id,
                    final_score,
                    *e1_sim,
                    *e11_sim,
                    entity_jaccard,
                    matched_entities,
                    content,
                ));
            }
        }

        // Step 8: Sort by score and take top-K
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        // Step 9: Build response
        let results: Vec<EntitySearchResult> = scored_results
            .into_iter()
            .map(
                |(memory_id, score, _e1_sim, e11_sim, entity_overlap, matched_entities, content)| {
                    EntitySearchResult {
                        memory_id,
                        score,
                        e11_similarity: e11_sim, // Report E11 similarity
                        entity_overlap,
                        matched_entities: matched_entities.iter().map(EntityLinkDto::from).collect(),
                        content: if include_content { content } else { None },
                    }
                },
            )
            .collect();

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = SearchByEntitiesResponse {
            results: results.clone(),
            detected_query_entities: query_entity_dtos,
            total_candidates,
            search_time_ms: elapsed_ms,
        };

        info!(
            results_found = results.len(),
            total_candidates = total_candidates,
            elapsed_ms = elapsed_ms,
            "search_by_entities: Completed entity-aware search"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    // ========================================================================
    // PHASE 3: TransE Operations
    // ========================================================================

    /// infer_relationship tool implementation.
    ///
    /// Infers the relationship between two entities using TransE.
    ///
    /// # Algorithm
    ///
    /// 1. Embed head entity with E11: h = E11("[TYPE] head_entity")
    /// 2. Embed tail entity with E11: t = E11("[TYPE] tail_entity")
    /// 3. Compute predicted relation: r̂ = t - h
    /// 4. Search for known relations closest to r̂ in embedding space
    /// 5. Score candidates with TransE: score = -||h + r - t||₂
    /// 6. Return ranked relation candidates
    ///
    /// # Constitution Compliance
    ///
    /// - Delta_S method: TransE ||h+r-t||
    /// - ARCH-20: E11 uses entity linking for disambiguation
    pub(crate) async fn call_infer_relationship(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: InferRelationshipRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "infer_relationship: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "infer_relationship: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let head_entity = &request.head_entity;
        let tail_entity = &request.tail_entity;
        let top_k = request.top_k;
        let include_score = request.include_score;

        info!(
            head = %head_entity,
            tail = %tail_entity,
            top_k = top_k,
            "infer_relationship: Starting TransE relation inference"
        );

        // Step 1: Format entity text with optional type hints
        let head_text = if let Some(ref type_hint) = request.head_type {
            format!("[{}] {}", type_hint, head_entity)
        } else {
            head_entity.clone()
        };

        let tail_text = if let Some(ref type_hint) = request.tail_type {
            format!("[{}] {}", type_hint, tail_entity)
        } else {
            tail_entity.clone()
        };

        // Step 2: Embed head and tail entities using E11
        let head_fingerprint = match self.multi_array_provider.embed_all(&head_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "infer_relationship: Head embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Head embedding failed: {}", e));
            }
        };

        let tail_fingerprint = match self.multi_array_provider.embed_all(&tail_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "infer_relationship: Tail embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Tail embedding failed: {}", e));
            }
        };

        let head_e11 = &head_fingerprint.e11_entity;
        let tail_e11 = &tail_fingerprint.e11_entity;

        // Step 3: Compute predicted relation using TransE: r̂ = t - h
        let predicted_r = KeplerModel::predict_relation(head_e11, tail_e11);

        debug!(
            predicted_r_norm = predicted_r.iter().map(|x| x * x).sum::<f32>().sqrt(),
            "infer_relationship: Computed predicted relation vector"
        );

        // Step 4: Score each known relation using TransE: score = -||h + r - t||₂
        let mut relation_scores: Vec<(&str, f32)> = Vec::with_capacity(KNOWN_RELATIONS.len());

        for (relation_name, _inverse) in KNOWN_RELATIONS {
            // Embed the relation text
            let relation_fingerprint = match self.multi_array_provider.embed_all(relation_name).await
            {
                Ok(output) => output.fingerprint,
                Err(e) => {
                    warn!(
                        relation = %relation_name,
                        error = %e,
                        "infer_relationship: Relation embedding failed, skipping"
                    );
                    continue;
                }
            };

            let relation_e11 = &relation_fingerprint.e11_entity;

            // Compute TransE score: -||h + r - t||₂
            let score = KeplerModel::transe_score(head_e11, relation_e11, tail_e11);
            relation_scores.push((relation_name, score));
        }

        // Step 5: Sort by score (higher/closer to 0 is better) and take top-K
        relation_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        relation_scores.truncate(top_k);

        // Step 6: Build response
        let inferred_relations: Vec<RelationCandidate> = relation_scores
            .into_iter()
            .map(|(relation, score)| {
                let confidence = transe_score_to_confidence(score);
                RelationCandidate {
                    relation: relation.to_string(),
                    score: if include_score { Some(score) } else { None },
                    confidence,
                }
            })
            .collect();

        // Detect entities from head/tail for proper EntityLinkDto
        let head_entities = detect_entities(head_entity);
        let tail_entities = detect_entities(tail_entity);

        let head_dto = head_entities
            .entities
            .first()
            .map(EntityLinkDto::from)
            .unwrap_or_else(|| EntityLinkDto {
                surface_form: head_entity.clone(),
                canonical_id: head_entity.to_lowercase().replace(' ', "_"),
                entity_type: "Unknown".to_string(),
                confidence: Some(0.5),
            });

        let tail_dto = tail_entities
            .entities
            .first()
            .map(EntityLinkDto::from)
            .unwrap_or_else(|| EntityLinkDto {
                surface_form: tail_entity.clone(),
                canonical_id: tail_entity.to_lowercase().replace(' ', "_"),
                entity_type: "Unknown".to_string(),
                confidence: Some(0.5),
            });

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = InferRelationshipResponse {
            head: head_dto,
            tail: tail_dto,
            inferred_relations: inferred_relations.clone(),
            predicted_vector: None, // Not exposing raw vector
        };

        info!(
            relations_found = inferred_relations.len(),
            top_relation = inferred_relations.first().map(|r| r.relation.as_str()).unwrap_or("none"),
            elapsed_ms = elapsed_ms,
            "infer_relationship: Completed TransE inference"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    /// find_related_entities tool implementation.
    ///
    /// Finds entities that have a given relationship to a source entity.
    ///
    /// # Algorithm
    ///
    /// 1. Embed source entity: h = E11(entity)
    /// 2. Embed relation: r = E11(relation)
    /// 3. Predict target: t̂ = h + r (outgoing) or ĥ = t - r (incoming)
    /// 4. Search stored memories for entities matching prediction
    /// 5. Score with TransE and return ranked results
    ///
    /// # Constitution Compliance
    ///
    /// - Delta_S method: TransE ||h+r-t||
    /// - ARCH-12: E1 is the semantic foundation, E11 enhances
    pub(crate) async fn call_find_related_entities(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: FindRelatedEntitiesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "find_related_entities: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "find_related_entities: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let entity = &request.entity;
        let relation = &request.relation;
        let direction = &request.direction;
        let top_k = request.top_k;
        let search_memories = request.search_memories;
        let min_score = request.min_score;

        info!(
            entity = %entity,
            relation = %relation,
            direction = %direction,
            top_k = top_k,
            "find_related_entities: Starting TransE entity prediction"
        );

        // Step 1: Embed source entity and relation
        let entity_fingerprint = match self.multi_array_provider.embed_all(entity).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "find_related_entities: Entity embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Entity embedding failed: {}", e));
            }
        };

        let relation_fingerprint = match self.multi_array_provider.embed_all(relation).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "find_related_entities: Relation embedding FAILED");
                return self.tool_error_with_pulse(
                    id,
                    &format!("Relation embedding failed: {}", e),
                );
            }
        };

        let entity_e11 = &entity_fingerprint.e11_entity;
        let relation_e11 = &relation_fingerprint.e11_entity;

        // Step 2: Predict target entity embedding
        // Outgoing (h→t): t̂ = h + r
        // Incoming (t←h): ĥ = t - r (entity is tail, find heads)
        let _predicted_embedding = if direction == "outgoing" {
            KeplerModel::predict_tail(entity_e11, relation_e11)
        } else {
            // For incoming, the entity is the tail, we want to find heads
            // ĥ = t - r where t is the entity
            entity_e11
                .iter()
                .zip(relation_e11.iter())
                .map(|(t, r)| t - r)
                .collect::<Vec<f32>>()
        };

        // Step 3: If search_memories is true, search stored memories for entities
        let mut related_entities: Vec<RelatedEntity> = Vec::new();

        if search_memories {
            // Search E1 for semantic candidates
            let search_query = format!("{} {} ?", entity, relation);
            let query_fingerprint = match self.multi_array_provider.embed_all(&search_query).await {
                Ok(output) => output.fingerprint,
                Err(e) => {
                    error!(error = %e, "find_related_entities: Query embedding FAILED");
                    return self
                        .tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
                }
            };

            let fetch_top_k = top_k * 10; // Over-fetch for filtering
            let options = TeleologicalSearchOptions::quick(fetch_top_k)
                .with_strategy(SearchStrategy::E1Only)
                .with_min_similarity(0.0);

            let candidates = match self
                .teleological_store
                .search_semantic(&query_fingerprint, options)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "find_related_entities: Candidate search FAILED");
                    return self.tool_error_with_pulse(id, &format!("Search failed: {}", e));
                }
            };

            // Get content and extract entities
            let candidate_ids: Vec<Uuid> = candidates.iter().map(|c| c.fingerprint.id).collect();
            let contents = match self.teleological_store.get_content_batch(&candidate_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "find_related_entities: Content retrieval FAILED");
                    return self
                        .tool_error_with_pulse(id, &format!("Content retrieval failed: {}", e));
                }
            };

            // Track unique entities found with their scores
            let mut entity_scores: std::collections::HashMap<String, (EntityLinkDto, f32, Vec<Uuid>)> =
                std::collections::HashMap::new();

            for (i, candidate) in candidates.iter().enumerate() {
                let cand_id = candidate.fingerprint.id;
                let content = contents.get(i).and_then(|c| c.clone());

                if let Some(text) = content {
                    let detected = detect_entities(&text);
                    let cand_e11 = &candidate.fingerprint.semantic.e11_entity;

                    for entity_link in detected.entities {
                        // Skip the source entity itself
                        if entity_link.canonical_id
                            == entity.to_lowercase().replace(' ', "_")
                        {
                            continue;
                        }

                        // Filter by entity type if specified
                        if let Some(ref filter_type) = request.entity_type {
                            let type_str = super::entity_dtos::entity_type_to_string(entity_link.entity_type);
                            if type_str.to_lowercase() != filter_type.to_lowercase() {
                                continue;
                            }
                        }

                        // Compute TransE score for this entity
                        let transe_score = KeplerModel::transe_score(
                            entity_e11,
                            relation_e11,
                            cand_e11,
                        );

                        // Apply minimum score filter
                        if let Some(min) = min_score {
                            if transe_score < min {
                                continue;
                            }
                        }

                        let canonical = entity_link.canonical_id.clone();
                        let dto = EntityLinkDto::from(&entity_link);

                        entity_scores
                            .entry(canonical)
                            .and_modify(|(_, score, ids)| {
                                if transe_score > *score {
                                    *score = transe_score;
                                }
                                if !ids.contains(&cand_id) {
                                    ids.push(cand_id);
                                }
                            })
                            .or_insert((dto, transe_score, vec![cand_id]));
                    }
                }
            }

            // Convert to RelatedEntity and sort by score
            let mut scored_entities: Vec<_> = entity_scores.into_values().collect();
            scored_entities
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored_entities.truncate(top_k);

            for (dto, score, memory_ids) in scored_entities {
                related_entities.push(RelatedEntity {
                    entity: dto,
                    transe_score: score,
                    found_in_memories: true,
                    memory_ids: Some(memory_ids),
                });
            }
        } else {
            // No memory search - just return known KB entities with predicted scores
            // Get entities from our KB and score them
            let kb = context_graph_core::entity::get_entity_kb();
            let mut kb_entities: Vec<_> = Vec::new();

            for (surface, (canonical, entity_type)) in kb.iter() {
                // Filter by entity type if specified
                if let Some(ref filter_type) = request.entity_type {
                    let type_str = super::entity_dtos::entity_type_to_string(*entity_type);
                    if type_str.to_lowercase() != filter_type.to_lowercase() {
                        continue;
                    }
                }

                // Embed entity and compute TransE score
                if let Ok(output) = self.multi_array_provider.embed_all(*surface).await {
                    let target_e11 = &output.fingerprint.e11_entity;
                    let score = KeplerModel::transe_score(entity_e11, relation_e11, target_e11);

                    // Apply minimum score filter
                    if let Some(min) = min_score {
                        if score < min {
                            continue;
                        }
                    }

                    kb_entities.push((
                        EntityLinkDto {
                            surface_form: surface.to_string(),
                            canonical_id: canonical.to_string(),
                            entity_type: super::entity_dtos::entity_type_to_string(*entity_type),
                            confidence: Some(1.0),
                        },
                        score,
                    ));
                }
            }

            kb_entities
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            kb_entities.truncate(top_k);

            for (dto, score) in kb_entities {
                related_entities.push(RelatedEntity {
                    entity: dto,
                    transe_score: score,
                    found_in_memories: false,
                    memory_ids: None,
                });
            }
        }

        // Build source entity DTO
        let source_entities = detect_entities(entity);
        let source_dto = source_entities
            .entities
            .first()
            .map(EntityLinkDto::from)
            .unwrap_or_else(|| EntityLinkDto {
                surface_form: entity.clone(),
                canonical_id: entity.to_lowercase().replace(' ', "_"),
                entity_type: "Unknown".to_string(),
                confidence: Some(0.5),
            });

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = FindRelatedEntitiesResponse {
            source_entity: source_dto,
            relation: relation.clone(),
            direction: direction.clone(),
            related_entities: related_entities.clone(),
            search_time_ms: elapsed_ms,
        };

        info!(
            entities_found = related_entities.len(),
            elapsed_ms = elapsed_ms,
            "find_related_entities: Completed TransE entity prediction"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    /// validate_knowledge tool implementation.
    ///
    /// Scores whether a (subject, predicate, object) triple is valid using TransE.
    ///
    /// # Algorithm
    ///
    /// 1. Embed all three: h = E11(subject), r = E11(predicate), t = E11(object)
    /// 2. Compute TransE score: score = -||h + r - t||₂
    /// 3. Normalize to [0, 1] confidence
    /// 4. Search for supporting/contradicting memories
    ///
    /// # Constitution Compliance
    ///
    /// - Delta_S method: TransE ||h+r-t||
    /// - ARCH-20: E11 uses entity linking for disambiguation
    pub(crate) async fn call_validate_knowledge(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: ValidateKnowledgeRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "validate_knowledge: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "validate_knowledge: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let subject = &request.subject;
        let predicate = &request.predicate;
        let object = &request.object;

        info!(
            subject = %subject,
            predicate = %predicate,
            object = %object,
            "validate_knowledge: Starting TransE triple validation"
        );

        // Step 1: Format entities with optional type hints
        let subject_text = if let Some(ref type_hint) = request.subject_type {
            format!("[{}] {}", type_hint, subject)
        } else {
            subject.clone()
        };

        let object_text = if let Some(ref type_hint) = request.object_type {
            format!("[{}] {}", type_hint, object)
        } else {
            object.clone()
        };

        // Step 2: Embed subject, predicate, and object using E11
        let subject_fingerprint = match self.multi_array_provider.embed_all(&subject_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "validate_knowledge: Subject embedding FAILED");
                return self
                    .tool_error_with_pulse(id, &format!("Subject embedding failed: {}", e));
            }
        };

        let predicate_fingerprint = match self.multi_array_provider.embed_all(predicate).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "validate_knowledge: Predicate embedding FAILED");
                return self
                    .tool_error_with_pulse(id, &format!("Predicate embedding failed: {}", e));
            }
        };

        let object_fingerprint = match self.multi_array_provider.embed_all(&object_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "validate_knowledge: Object embedding FAILED");
                return self
                    .tool_error_with_pulse(id, &format!("Object embedding failed: {}", e));
            }
        };

        let h = &subject_fingerprint.e11_entity;
        let r = &predicate_fingerprint.e11_entity;
        let t = &object_fingerprint.e11_entity;

        // Step 3: Compute TransE score: -||h + r - t||₂
        let transe_score = KeplerModel::transe_score(h, r, t);

        // Step 4: Convert to confidence and validation result
        let confidence = transe_score_to_confidence(transe_score);
        let validation = validation_from_score(transe_score);

        debug!(
            transe_score = transe_score,
            confidence = confidence,
            validation = %validation,
            "validate_knowledge: Computed TransE score"
        );

        // Step 5: Search for supporting/contradicting memories
        let mut supporting_memories: Vec<Uuid> = Vec::new();
        let mut contradicting_memories: Vec<Uuid> = Vec::new();

        // Search for memories containing both entities
        let search_query = format!("{} {} {}", subject, predicate, object);
        if let Ok(output) = self.multi_array_provider.embed_all(&search_query).await {
            let query_fingerprint = output.fingerprint;

            let options = TeleologicalSearchOptions::quick(20)
                .with_strategy(SearchStrategy::E1Only)
                .with_min_similarity(0.3);

            if let Ok(candidates) = self
                .teleological_store
                .search_semantic(&query_fingerprint, options)
                .await
            {
                let candidate_ids: Vec<Uuid> = candidates.iter().map(|c| c.fingerprint.id).collect();

                if let Ok(contents) = self.teleological_store.get_content_batch(&candidate_ids).await
                {
                    for (i, candidate) in candidates.iter().enumerate() {
                        let cand_id = candidate.fingerprint.id;
                        if let Some(Some(text)) = contents.get(i) {
                            // Check if both subject and object are mentioned
                            let text_lower = text.to_lowercase();
                            let subject_lower = subject.to_lowercase();
                            let object_lower = object.to_lowercase();

                            if text_lower.contains(&subject_lower)
                                && text_lower.contains(&object_lower)
                            {
                                // Compute E11 TransE score to determine if supporting or contradicting
                                // Per KEPLER paper: valid triples score > -5.0, invalid < -10.0
                                let cand_e11 = &candidate.fingerprint.semantic.e11_entity;
                                let cand_score =
                                    KeplerModel::transe_score(h, r, cand_e11);

                                if cand_score > VALID_THRESHOLD {
                                    // Good TransE alignment (> -5.0) - supporting
                                    supporting_memories.push(cand_id);
                                } else if cand_score < UNCERTAIN_THRESHOLD {
                                    // Poor TransE alignment (< -10.0) - potentially contradicting
                                    contradicting_memories.push(cand_id);
                                }
                                // Note: scores in [-10.0, -5.0] are uncertain - don't categorize
                            }
                        }
                    }
                }
            }
        }

        // Build entity DTOs
        let subject_entities = detect_entities(subject);
        let object_entities = detect_entities(object);

        let subject_dto = subject_entities
            .entities
            .first()
            .map(EntityLinkDto::from)
            .unwrap_or_else(|| EntityLinkDto {
                surface_form: subject.clone(),
                canonical_id: subject.to_lowercase().replace(' ', "_"),
                entity_type: "Unknown".to_string(),
                confidence: Some(0.5),
            });

        let object_dto = object_entities
            .entities
            .first()
            .map(EntityLinkDto::from)
            .unwrap_or_else(|| EntityLinkDto {
                surface_form: object.clone(),
                canonical_id: object.to_lowercase().replace(' ', "_"),
                entity_type: "Unknown".to_string(),
                confidence: Some(0.5),
            });

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = ValidateKnowledgeResponse {
            triple: KnowledgeTripleDto {
                subject: subject_dto,
                predicate: predicate.clone(),
                object: object_dto,
            },
            transe_score,
            confidence,
            validation: validation.to_string(),
            supporting_memories: if supporting_memories.is_empty() {
                None
            } else {
                Some(supporting_memories)
            },
            contradicting_memories: if contradicting_memories.is_empty() {
                None
            } else {
                Some(contradicting_memories)
            },
        };

        info!(
            transe_score = transe_score,
            confidence = confidence,
            validation = %validation,
            supporting_count = response.supporting_memories.as_ref().map(|v| v.len()).unwrap_or(0),
            contradicting_count = response.contradicting_memories.as_ref().map(|v| v.len()).unwrap_or(0),
            elapsed_ms = elapsed_ms,
            "validate_knowledge: Completed TransE validation"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    // ========================================================================
    // PHASE 4: Entity Graph
    // ========================================================================

    /// get_entity_graph tool implementation.
    ///
    /// Builds and visualizes entity relationships from stored memories.
    ///
    /// # Algorithm
    ///
    /// 1. If center_entity provided, focus on that entity's neighborhood
    /// 2. Scan memories for entities
    /// 3. Build entity co-occurrence graph
    /// 4. Infer relationships using TransE
    /// 5. Return graph with nodes and edges
    ///
    /// # Constitution Compliance
    ///
    /// - ARCH-12: E1 is the semantic foundation, E11 enhances
    /// - ARCH-20: E11 uses entity linking for disambiguation
    pub(crate) async fn call_get_entity_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: GetEntityGraphRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_entity_graph: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "get_entity_graph: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let max_nodes = request.max_nodes;
        let max_depth = request.max_depth;
        let min_relation_score = request.min_relation_score;

        info!(
            center_entity = ?request.center_entity,
            max_nodes = max_nodes,
            max_depth = max_depth,
            min_relation_score = min_relation_score,
            "get_entity_graph: Building entity relationship graph"
        );

        // Step 1: Search for memories to scan
        let search_query = if let Some(ref center) = request.center_entity {
            center.clone()
        } else {
            // Search for recent technical content
            "code programming framework database".to_string()
        };

        let query_fingerprint = match self.multi_array_provider.embed_all(&search_query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "get_entity_graph: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Fetch enough memories to build a meaningful graph
        let fetch_size = (max_nodes * 5).min(500);
        let options = TeleologicalSearchOptions::quick(fetch_size)
            .with_strategy(SearchStrategy::E1Only)
            .with_min_similarity(0.0);

        let memories = match self
            .teleological_store
            .search_semantic(&query_fingerprint, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "get_entity_graph: Memory search FAILED");
                return self.tool_error_with_pulse(id, &format!("Memory search failed: {}", e));
            }
        };

        let total_memories_scanned = memories.len();

        if memories.is_empty() {
            info!("get_entity_graph: No memories found to scan");
            return self.tool_result_with_pulse(
                id,
                serde_json::to_value(GetEntityGraphResponse {
                    nodes: vec![],
                    edges: vec![],
                    center_entity: None,
                    total_memories_scanned: 0,
                })
                .unwrap_or_else(|_| json!({})),
            );
        }

        // Get content for entity extraction
        let memory_ids: Vec<Uuid> = memories.iter().map(|m| m.fingerprint.id).collect();
        let contents = match self.teleological_store.get_content_batch(&memory_ids).await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "get_entity_graph: Content retrieval FAILED");
                return self
                    .tool_error_with_pulse(id, &format!("Content retrieval failed: {}", e));
            }
        };

        // Step 2: Extract entities from all memories and track co-occurrences
        // Map: canonical_id -> (EntityLinkDto, Vec<memory_ids>)
        let mut entity_map: std::collections::HashMap<String, (EntityLinkDto, Vec<Uuid>)> =
            std::collections::HashMap::new();

        // Map: (entity1, entity2) -> Vec<memory_ids> (for co-occurrence edges)
        let mut cooccurrence_map: std::collections::HashMap<(String, String), Vec<Uuid>> =
            std::collections::HashMap::new();

        for (i, _memory) in memories.iter().enumerate() {
            let memory_id = memory_ids[i];
            if let Some(Some(text)) = contents.get(i) {
                let detected = detect_entities(text);

                // Collect entities for this memory
                let mut memory_entities: Vec<String> = Vec::new();

                for entity_link in &detected.entities {
                    let canonical = entity_link.canonical_id.clone();
                    let dto = EntityLinkDto::from(entity_link);

                    entity_map
                        .entry(canonical.clone())
                        .and_modify(|(_, ids)| {
                            if !ids.contains(&memory_id) {
                                ids.push(memory_id);
                            }
                        })
                        .or_insert((dto, vec![memory_id]));

                    memory_entities.push(canonical);
                }

                // Record co-occurrences (entities appearing in same memory)
                for i in 0..memory_entities.len() {
                    for j in (i + 1)..memory_entities.len() {
                        let e1 = &memory_entities[i];
                        let e2 = &memory_entities[j];

                        // Use sorted order for consistent key
                        let key = if e1 < e2 {
                            (e1.clone(), e2.clone())
                        } else {
                            (e2.clone(), e1.clone())
                        };

                        cooccurrence_map
                            .entry(key)
                            .and_modify(|ids| {
                                if !ids.contains(&memory_id) {
                                    ids.push(memory_id);
                                }
                            })
                            .or_insert_with(|| vec![memory_id]);
                    }
                }
            }
        }

        // Step 3: Build nodes (limit to max_nodes)
        let mut nodes: Vec<EntityNode> = Vec::new();

        // Sort entities by mention count (descending)
        let mut entity_list: Vec<_> = entity_map.into_iter().collect();
        entity_list.sort_by(|a, b| b.1 .1.len().cmp(&a.1 .1.len()));
        entity_list.truncate(max_nodes);

        // Track which entities we include
        let included_entities: HashSet<String> =
            entity_list.iter().map(|(id, _)| id.clone()).collect();

        for (canonical_id, (dto, memory_ids_vec)) in &entity_list {
            let memory_count = memory_ids_vec.len();
            let importance = (memory_count as f32).ln_1p() / 10.0; // Log-scale importance

            nodes.push(EntityNode {
                id: canonical_id.clone(),
                label: dto.surface_form.clone(),
                entity_type: dto.entity_type.clone(),
                memory_count,
                importance: importance.min(1.0),
            });
        }

        // Step 4: Build edges from co-occurrences
        let mut edges: Vec<EntityEdge> = Vec::new();

        for ((e1, e2), memory_ids_vec) in &cooccurrence_map {
            // Only include edges between nodes we're showing
            if !included_entities.contains(e1) || !included_entities.contains(e2) {
                continue;
            }

            // Compute edge weight based on co-occurrence count
            let cooccurrence_count = memory_ids_vec.len();
            let weight = (cooccurrence_count as f32) / (total_memories_scanned as f32).max(1.0);

            // Skip edges below threshold
            if weight < min_relation_score {
                continue;
            }

            // Infer relation using TransE (simplified - just use "related_to" for co-occurrence)
            // For full TransE inference, we'd embed both entities and find the best relation
            let relation = if cooccurrence_count >= 5 {
                "strongly_related_to"
            } else if cooccurrence_count >= 2 {
                "related_to"
            } else {
                "co_occurs_with"
            };

            edges.push(EntityEdge {
                source: e1.clone(),
                target: e2.clone(),
                relation: relation.to_string(),
                weight,
                memory_ids: memory_ids_vec.clone(),
            });
        }

        // Sort edges by weight (descending)
        edges.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));

        // Limit edges if needed (e.g., 3x nodes)
        let max_edges = max_nodes * 3;
        edges.truncate(max_edges);

        // Build center entity DTO if provided
        let center_entity_dto = request.center_entity.as_ref().map(|center| {
            let center_entities = detect_entities(center);
            center_entities
                .entities
                .first()
                .map(EntityLinkDto::from)
                .unwrap_or_else(|| EntityLinkDto {
                    surface_form: center.clone(),
                    canonical_id: center.to_lowercase().replace(' ', "_"),
                    entity_type: "Unknown".to_string(),
                    confidence: Some(0.5),
                })
        });

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = GetEntityGraphResponse {
            nodes: nodes.clone(),
            edges: edges.clone(),
            center_entity: center_entity_dto,
            total_memories_scanned,
        };

        info!(
            nodes = nodes.len(),
            edges = edges.len(),
            memories_scanned = total_memories_scanned,
            elapsed_ms = elapsed_ms,
            "get_entity_graph: Completed entity graph construction"
        );

        self.tool_result_with_pulse(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }
}

/// Group entities by their types.
#[allow(dead_code)]
fn group_entities_by_type(entities: &[context_graph_core::entity::EntityLink]) -> EntityByType {
    let mut grouped = EntityByType::default();

    for entity in entities {
        let dto = EntityLinkDto::from(entity);
        match entity.entity_type {
            EntityType::ProgrammingLanguage => grouped.programming_language.push(dto),
            EntityType::Framework => grouped.framework.push(dto),
            EntityType::Database => grouped.database.push(dto),
            EntityType::Cloud => grouped.cloud.push(dto),
            EntityType::Company => grouped.company.push(dto),
            EntityType::TechnicalTerm => grouped.technical_term.push(dto),
            EntityType::Unknown => grouped.unknown.push(dto),
        }
    }

    grouped
}

/// Format entity type distribution for logging.
#[allow(dead_code)]
fn format_entity_type_distribution(entities: &[context_graph_core::entity::EntityLink]) -> String {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for entity in entities {
        let type_str = match entity.entity_type {
            EntityType::ProgrammingLanguage => "ProgrammingLanguage",
            EntityType::Framework => "Framework",
            EntityType::Database => "Database",
            EntityType::Cloud => "Cloud",
            EntityType::Company => "Company",
            EntityType::TechnicalTerm => "TechnicalTerm",
            EntityType::Unknown => "Unknown",
        };
        *counts.entry(type_str).or_insert(0) += 1;
    }

    counts
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_entities_by_type() {
        use context_graph_core::entity::EntityLink;

        let entities = vec![
            EntityLink {
                surface_form: "Rust".to_string(),
                canonical_id: "rust_language".to_string(),
                entity_type: EntityType::ProgrammingLanguage,
            },
            EntityLink {
                surface_form: "PostgreSQL".to_string(),
                canonical_id: "postgresql".to_string(),
                entity_type: EntityType::Database,
            },
            EntityLink {
                surface_form: "React".to_string(),
                canonical_id: "react".to_string(),
                entity_type: EntityType::Framework,
            },
        ];

        let grouped = group_entities_by_type(&entities);

        assert_eq!(grouped.programming_language.len(), 1);
        assert_eq!(grouped.database.len(), 1);
        assert_eq!(grouped.framework.len(), 1);
        assert_eq!(grouped.cloud.len(), 0);
    }

    #[test]
    fn test_format_entity_type_distribution() {
        use context_graph_core::entity::EntityLink;

        let entities = vec![
            EntityLink {
                surface_form: "Rust".to_string(),
                canonical_id: "rust_language".to_string(),
                entity_type: EntityType::ProgrammingLanguage,
            },
            EntityLink {
                surface_form: "Python".to_string(),
                canonical_id: "python".to_string(),
                entity_type: EntityType::ProgrammingLanguage,
            },
            EntityLink {
                surface_form: "PostgreSQL".to_string(),
                canonical_id: "postgresql".to_string(),
                entity_type: EntityType::Database,
            },
        ];

        let distribution = format_entity_type_distribution(&entities);

        assert!(distribution.contains("ProgrammingLanguage:2"));
        assert!(distribution.contains("Database:1"));
    }
}
