//! # Embedder-First Search Tool Implementations
//!
//! Per Constitution v6.3, these tools enable AI agents to search using any of the
//! 14 embedders as the primary perspective.
//!
//! ## Philosophy
//!
//! Each active embedder sees the knowledge graph from a unique perspective:
//! - E1 (semantic): Dense semantic similarity - foundation
//! - E7 (code): Code patterns - finds `tokio::spawn` when searching for "async runtime"
//! - E5 (causal): retired and disabled
//! - E11 (entity): disabled until a verified code-symbol entity embedder exists
//!
//! Sometimes the best perspective isn't E1. These tools let agents choose.
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation (default), but other embedders can be primary
//! - ARCH-02: All comparisons within same embedder space (no cross-embedder comparison)
//! - Each embedder has its own FAISS/HNSW index on GPU

use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::CausalDirection;
use context_graph_core::memory::ast::{EntityType as AstEntityType, Language as AstLanguage};
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::weights::{active_embedder_count, disabled_embedder_names};

/// Maximum number of custom weight profiles to prevent unbounded HashMap growth.
const MAX_CUSTOM_PROFILES: usize = 1_000;
const E11_DISABLED_TOOL_MESSAGE: &str = "E11 entity embedder is disabled because the available Kepler assets are incomplete/incompatible with the runtime and no verified code-symbol entity replacement has been installed; choose an active embedder";

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use context_graph_core::teleological::Embedder;
use context_graph_mejepa_embedders::{
    query_vram_budget as mejepa_query_vram_budget,
    route_for_entity_type as mejepa_route_for_entity_type, EmbedderId as MejepaEmbedderId,
    Ensemble as MejepaEnsemble, VramBudget as MejepaVramBudget,
};

use super::embedder_dtos::{
    AllEmbedderScores, AsymmetricVariant, CompareEmbedderViewsRequest,
    CompareEmbedderViewsResponse, CreateWeightProfileRequest, CreateWeightProfileResponse,
    EmbedderAnomaly, EmbedderCluster, EmbedderId, EmbedderIndexInfo, EmbedderRanking,
    EmbedderSearchResult, EmbedderVectorInfo, GetEmbedderClustersRequest,
    GetEmbedderClustersResponse, GetMemoryFingerprintRequest, GetMemoryFingerprintResponse,
    ListEmbedderIndexesRequest, ListEmbedderIndexesResponse, RankedMemory, SearchByEmbedderRequest,
    SearchByEmbedderResponse, SearchCrossEmbedderAnomaliesRequest,
    SearchCrossEmbedderAnomaliesResponse, UniqueFind,
};

use super::super::Handlers;
use super::helpers::ToolErrorKind;
use crate::telemetry::gpu_wsl::{default_query as gpu_wsl_default_query, nvidia_smi_query};

impl Handlers {
    /// search_by_embedder tool implementation.
    ///
    /// Search using any embedder (E1-E14) as the primary perspective.
    ///
    /// # Algorithm
    ///
    /// 1. Parse and validate embedder selection
    /// 2. Create query embedding for all 14 spaces
    /// 3. Search in the selected embedder's space
    /// 4. Optionally compute scores from all 14 embedders
    /// 5. Return results with selected embedder's perspective
    ///
    /// # Constitution Compliance
    ///
    /// - ARCH-12: Default is E1, but any embedder can be primary
    /// - ARCH-02: Search is within selected embedder's space only
    pub(crate) async fn call_search_by_embedder(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: SearchByEmbedderRequest =
            match self.parse_request(id.clone(), args, "search_by_embedder") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let embedder_id = match request.embedder_id() {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.embedder, "search_by_embedder: Invalid embedder");
                return self.tool_error(id, &format!("Invalid embedder: {}", request.embedder));
            }
        };

        let embedder_index = embedder_id.to_index();

        if matches!(embedder_id, EmbedderId::E11)
            && !context_graph_core::weights::E11_ENTITY_ENABLED
        {
            error!(
                embedder = %request.embedder,
                "search_by_embedder: rejected disabled E11 request"
            );
            return self.tool_error_typed(id, ToolErrorKind::Validation, E11_DISABLED_TOOL_MESSAGE);
        }
        if matches!(embedder_id, EmbedderId::E5) && !context_graph_core::weights::E5_CAUSAL_ENABLED
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E5 causal embedder is retired and disabled; choose an active embedder",
            );
        }

        info!(
            embedder = %request.embedder,
            embedder_name = %embedder_id.name(),
            query = %request.query,
            top_k = request.top_k,
            "search_by_embedder: Starting embedder-first search"
        );

        // Step 1: Create query embedding (all 14 embedders)
        let query_fingerprint = match self
            .embed_query(id.clone(), &request.query, "search_by_embedder")
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search in the selected embedder's space
        // AP-77: E5 requires explicit causal direction for scoring (not symmetric).
        // Default to Cause (cause→effect) when user explicitly selects E5.
        let mut options = TeleologicalSearchOptions::quick(request.top_k)
            .with_strategy(SearchStrategy::E1Only) // Strategy doesn't matter with explicit embedders
            .with_embedders(vec![embedder_index])
            .with_min_similarity(request.min_similarity);
        if matches!(embedder_id, EmbedderId::E5) {
            options = options.with_causal_direction(CausalDirection::Cause);
        }

        let candidates = match self
            .teleological_store
            .search_semantic(&query_fingerprint, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(
                    error = %e,
                    embedder = %request.embedder,
                    "search_by_embedder: Search in {} space FAILED",
                    embedder_id.name()
                );
                return self.tool_error(
                    id,
                    &format!("Search in {} space failed: {}", embedder_id.name(), e),
                );
            }
        };

        let total_searched = candidates.len();

        debug!(
            candidates = total_searched,
            embedder = %request.embedder,
            "search_by_embedder: Found candidates in {} space",
            embedder_id.name()
        );

        // Step 3: Get content if requested - FAIL FAST on error
        let candidate_ids: Vec<Uuid> = candidates.iter().map(|c| c.fingerprint.id).collect();
        let contents = if request.include_content {
            match self
                .teleological_store
                .get_content_batch(&candidate_ids)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        result_count = candidate_ids.len(),
                        "search_by_embedder: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} results: {}",
                            candidate_ids.len(),
                            e
                        ),
                    );
                }
            }
        } else {
            vec![None; candidate_ids.len()]
        };

        // Step 4: Build results
        let results: Vec<EmbedderSearchResult> = candidates
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                let content = contents.get(i).and_then(|c| c.clone());

                // Optionally include all 14 embedder scores.
                // TeleologicalSearchResult already computes all 14 scores during search.
                let all_scores = if request.include_all_scores {
                    // Convert [f32; 14] array to AllEmbedderScores struct.
                    let s = &cand.embedder_scores;
                    Some(AllEmbedderScores {
                        e1: s[0],
                        e2: s[1],
                        e3: s[2],
                        e4: s[3],
                        // E5 sentinel (-1.0) = no causal direction detected.
                        e5: if s[4] < 0.0 { None } else { Some(s[4]) },
                        e6: s[5],
                        e7: s[6],
                        e8: s[7],
                        e9: s[8],
                        e10: s[9],
                        e11: s[10],
                        e12: s[11],
                        e13: s[12],
                        e14: s[13],
                    })
                } else {
                    None
                };

                // Use the selected embedder's score from embedder_scores array
                // instead of cand.similarity (which is always E1's score)
                let selected_score = cand.embedder_scores[embedder_index];

                EmbedderSearchResult {
                    memory_id: cand.fingerprint.id,
                    similarity: selected_score,
                    content,
                    all_scores,
                }
            })
            .collect();

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = SearchByEmbedderResponse {
            embedder: request.embedder.clone(),
            embedder_finds: embedder_id.finds().to_string(),
            results,
            total_searched,
            search_time_ms: elapsed_ms,
        };

        info!(
            embedder = %request.embedder,
            results = response.results.len(),
            elapsed_ms = elapsed_ms,
            "search_by_embedder: Completed embedder-first search"
        );

        // Emit SearchPerformed audit (non-fatal)
        {
            let audit_record = AuditRecord::new(
                AuditOperation::SearchPerformed {
                    tool_name: "search_by_embedder".to_string(),
                    results_returned: response.results.len(),
                    weight_profile: None,
                    strategy: Some(format!("embedder_first:{}", request.embedder)),
                },
                candidate_ids.first().copied().unwrap_or(Uuid::nil()),
            )
            .with_operator("search_by_embedder")
            .with_parameters(json!({
                "query_preview": request.query.chars().take(100).collect::<String>(),
                "top_k": request.top_k,
                "embedder": request.embedder,
            }));

            if let Err(e) = self
                .teleological_store
                .append_audit_record(&audit_record)
                .await
            {
                error!(error = %e, "search_by_embedder: Failed to write audit record (non-fatal)");
            }
        }

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_by_embedder: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// get_embedder_clusters tool implementation.
    ///
    /// Explore clusters of memories in a specific embedder's space.
    ///
    /// Reads pre-computed clusters from the MultiSpaceClusterManager (HDBSCAN + BIRCH).
    ///
    /// # Algorithm
    ///
    /// 1. Read clusters for the selected embedder from cluster_manager
    /// 2. Filter by min_cluster_size, sort by size descending
    /// 3. Optionally include sample memory IDs and content snippets
    pub(crate) async fn call_get_embedder_clusters(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        let request: GetEmbedderClustersRequest =
            match self.parse_request(id.clone(), args, "get_embedder_clusters") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let embedder_id = match request.embedder_id() {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.embedder, "get_embedder_clusters: Invalid embedder");
                return self.tool_error(id, &format!("Invalid embedder: {}", request.embedder));
            }
        };

        let embedder = match Embedder::from_index(embedder_id.to_index()) {
            Some(e) => e,
            None => {
                return self.tool_error(
                    id,
                    &format!("Invalid embedder index: {}", embedder_id.to_index()),
                );
            }
        };
        if matches!(embedder_id, EmbedderId::E5) && !context_graph_core::weights::E5_CAUSAL_ENABLED
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E5 causal embedder is retired and disabled; choose an active embedder",
            );
        }
        if matches!(embedder_id, EmbedderId::E11)
            && !context_graph_core::weights::E11_ENTITY_ENABLED
        {
            error!(
                embedder = %request.embedder,
                "get_embedder_clusters: rejected disabled E11 request"
            );
            return self.tool_error_typed(id, ToolErrorKind::Validation, E11_DISABLED_TOOL_MESSAGE);
        }

        // Auto-populate cluster manager from storage if empty (e.g. after MCP restart).
        // parking_lot::RwLockReadGuard is !Send — must drop before .await
        let needs_clustering = {
            let cm = self.cluster_manager.read();
            cm.total_memories() == 0
        };
        if needs_clustering {
            let fingerprints = match self
                .teleological_store
                .scan_fingerprints_for_clustering(None)
                .await
            {
                Ok(fp) => fp,
                Err(e) => {
                    error!(error = %e, "get_embedder_clusters: Failed to scan fingerprints");
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Storage,
                        &format!("Failed to scan fingerprints for clustering: {}", e),
                    );
                }
            };
            if !fingerprints.is_empty() {
                info!(
                    count = fingerprints.len(),
                    "get_embedder_clusters: Auto-loading fingerprints and clustering"
                );
                let mut cm = self.cluster_manager.write();
                cm.clear_all_spaces();
                let mut insert_failures = 0usize;
                for (fp_id, cluster_array) in &fingerprints {
                    if let Err(e) = cm.insert(*fp_id, cluster_array) {
                        insert_failures += 1;
                        warn!(
                            error = %e,
                            fingerprint_id = %fp_id,
                            "get_embedder_clusters: Failed to insert fingerprint into cluster manager"
                        );
                    }
                }
                if insert_failures > 0 {
                    warn!(
                        failed = insert_failures,
                        total = fingerprints.len(),
                        "get_embedder_clusters: {} of {} fingerprint inserts failed — clustering may be incomplete",
                        insert_failures, fingerprints.len()
                    );
                }
                if let Err(e) = cm.recluster() {
                    error!(error = %e, "get_embedder_clusters: Auto-recluster failed");
                    return self.tool_error_typed(
                        id,
                        ToolErrorKind::Execution,
                        &format!("Auto-recluster failed: {}", e),
                    );
                }
            }
        }

        // Read clusters from MultiSpaceClusterManager
        let (cluster_info, total_clusters, total_memories) = {
            let cluster_manager = self.cluster_manager.read();
            let clusters = cluster_manager.get_clusters(embedder);

            let mut cluster_vec: Vec<_> = clusters
                .values()
                .filter(|c| c.member_count as usize >= request.min_cluster_size)
                .collect();
            cluster_vec.sort_by_key(|cluster| std::cmp::Reverse(cluster.member_count));
            cluster_vec.truncate(request.top_clusters);

            let total_clusters = clusters.len();
            let total_mem = cluster_manager.total_memories();

            // Collect cluster info and member IDs while lock is held
            let data: Vec<(i32, usize, Vec<Uuid>)> = cluster_vec
                .iter()
                .map(|c| {
                    let members: Vec<Uuid> = if request.include_samples {
                        cluster_manager
                            .get_cluster_members(embedder, c.id)
                            .into_iter()
                            .take(request.samples_per_cluster)
                            .collect()
                    } else {
                        Vec::new()
                    };
                    (c.id, c.member_count as usize, members)
                })
                .collect();

            (data, total_clusters, total_mem)
        };
        // Lock dropped here

        // Build response clusters with optional content samples
        let mut response_clusters = Vec::new();
        for (cluster_id, size, members) in &cluster_info {
            let mut sample_ids = None;
            let mut sample_snippets = None;

            if request.include_samples && !members.is_empty() {
                let contents = match self.teleological_store.get_content_batch(members).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "get_embedder_clusters: Failed to retrieve cluster sample content");
                        return self.tool_error_typed(
                            id,
                            ToolErrorKind::Storage,
                            &format!("Failed to retrieve cluster sample content: {}", e),
                        );
                    }
                };
                let snippets: Vec<String> = contents
                    .iter()
                    .filter_map(|opt| opt.as_ref())
                    .map(|s| {
                        if s.len() > 100 {
                            format!("{}...", &s[..s.floor_char_boundary(100)])
                        } else {
                            s.clone()
                        }
                    })
                    .collect();
                if !snippets.is_empty() {
                    sample_snippets = Some(snippets);
                }
                sample_ids = Some(members.clone());
            }

            response_clusters.push(EmbedderCluster {
                cluster_id: *cluster_id as usize,
                size: *size,
                sample_ids,
                sample_snippets,
                description: None,
            });
        }

        // Provide a hint when clusters are empty so users know what to do
        let hint = if total_clusters == 0 && total_memories == 0 {
            Some("No clusters available. Call detect_topics first to trigger clustering, or store more memories.".to_string())
        } else if total_clusters == 0 {
            Some("No clusters found for this embedder. Try calling detect_topics to refresh clustering.".to_string())
        } else {
            None
        };

        let response = GetEmbedderClustersResponse {
            embedder: request.embedder.clone(),
            embedder_perspective: embedder_id.name().to_string(),
            clusters: response_clusters,
            total_memories,
            total_clusters,
            time_ms: start.elapsed().as_millis() as u64,
            hint,
        };

        info!(
            embedder = %request.embedder,
            total_clusters = total_clusters,
            returned_clusters = cluster_info.len(),
            total_memories = total_memories,
            elapsed_ms = response.time_ms,
            "get_embedder_clusters: Completed"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "get_embedder_clusters: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// compare_embedder_views tool implementation.
    ///
    /// Compare how different embedders rank the same query.
    ///
    /// # Algorithm
    ///
    /// 1. Search each selected embedder for top-K results
    /// 2. Find agreement (memories in all results)
    /// 3. Find unique finds (memories in only one embedder's results)
    /// 4. Compute agreement score
    pub(crate) async fn call_compare_embedder_views(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: CompareEmbedderViewsRequest =
            match self.parse_request(id.clone(), args, "compare_embedder_views") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let embedder_ids = request.embedder_ids();
        if !context_graph_core::weights::E5_CAUSAL_ENABLED
            && embedder_ids.iter().any(|eid| matches!(eid, EmbedderId::E5))
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E5 causal embedder is retired and disabled; compare only active embedders",
            );
        }
        if !context_graph_core::weights::E11_ENTITY_ENABLED
            && embedder_ids
                .iter()
                .any(|eid| matches!(eid, EmbedderId::E11))
        {
            error!(
                embedders = ?request.embedders,
                "compare_embedder_views: rejected disabled E11 request"
            );
            return self.tool_error_typed(id, ToolErrorKind::Validation, E11_DISABLED_TOOL_MESSAGE);
        }

        info!(
            embedders = ?request.embedders,
            query = %request.query,
            top_k = request.top_k,
            "compare_embedder_views: Starting embedder comparison"
        );

        // Step 1: Create query embedding (all 14 embedders)
        let query_fingerprint = match self
            .embed_query(id.clone(), &request.query, "compare_embedder_views")
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search each embedder
        let mut rankings: Vec<EmbedderRanking> = Vec::new();
        let mut all_memory_sets: Vec<HashSet<Uuid>> = Vec::new();
        let mut embedder_results: HashMap<String, Vec<(Uuid, f32)>> = HashMap::new();

        for embedder_id in &embedder_ids {
            let embedder_index = embedder_id.to_index();

            let mut options = TeleologicalSearchOptions::quick(request.top_k)
                .with_strategy(SearchStrategy::E1Only)
                .with_embedders(vec![embedder_index])
                .with_min_similarity(0.0);
            // AP-77: E5 requires causal direction for scoring
            if matches!(embedder_id, EmbedderId::E5) {
                options = options.with_causal_direction(CausalDirection::Cause);
            }

            // FAIL FAST: If search fails for any embedder, the entire comparison fails
            let candidates = match self
                .teleological_store
                .search_semantic(&query_fingerprint, options)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(
                        error = %e,
                        embedder = ?embedder_id,
                        "compare_embedder_views: Search in {} FAILED",
                        embedder_id.name()
                    );
                    return self.tool_error(
                        id,
                        &format!("Search failed for embedder {}: {}", embedder_id.name(), e),
                    );
                }
            };

            let mut memory_set: HashSet<Uuid> = HashSet::new();
            let mut results_list: Vec<(Uuid, f32)> = Vec::new();
            let mut ranked_memories: Vec<RankedMemory> = Vec::new();

            // HIGH-10 FIX: Batch fetch content if include_content=true
            let memory_ids: Vec<Uuid> = candidates.iter().map(|c| c.fingerprint.id).collect();
            let content_map: HashMap<Uuid, String> = if request.include_content
                && !memory_ids.is_empty()
            {
                match self.teleological_store.get_content_batch(&memory_ids).await {
                    Ok(contents) => memory_ids
                        .iter()
                        .zip(contents)
                        .filter_map(|(id, content)| content.map(|c| (*id, c)))
                        .collect(),
                    Err(e) => {
                        error!(error = %e, "compare_embedder_views: Content retrieval FAILED");
                        return self.tool_error(id, &format!("Content retrieval failed: {}", e));
                    }
                }
            } else {
                HashMap::new()
            };

            for (rank, cand) in candidates.iter().enumerate() {
                let memory_id = cand.fingerprint.id;
                // Use the selected embedder's score from embedder_scores array
                // instead of cand.similarity (which is always E1's score)
                let selected_score = cand.embedder_scores[embedder_index];
                memory_set.insert(memory_id);
                results_list.push((memory_id, selected_score));

                ranked_memories.push(RankedMemory {
                    memory_id,
                    rank: rank + 1,
                    similarity: selected_score,
                    content: content_map.get(&memory_id).cloned(),
                });
            }

            let embedder_str = format!("{:?}", embedder_id);
            embedder_results.insert(embedder_str.clone(), results_list);
            all_memory_sets.push(memory_set);

            rankings.push(EmbedderRanking {
                embedder: embedder_str,
                finds: embedder_id.finds().to_string(),
                results: ranked_memories,
            });
        }

        // Step 3: Find agreement (intersection of all sets)
        let agreement: Vec<Uuid> = if all_memory_sets.is_empty() {
            Vec::new()
        } else {
            let mut intersection = all_memory_sets[0].clone();
            for set in all_memory_sets.iter().skip(1) {
                intersection = intersection.intersection(set).cloned().collect();
            }
            intersection.into_iter().collect()
        };

        // Step 4: Find unique finds (in exactly one set)
        let mut all_memories: HashSet<Uuid> = HashSet::new();
        for set in &all_memory_sets {
            all_memories.extend(set);
        }

        let mut unique_finds: Vec<UniqueFind> = Vec::new();
        for memory_id in &all_memories {
            let mut found_in: Vec<&String> = Vec::new();
            for (embedder_str, results) in &embedder_results {
                if results.iter().any(|(id, _)| id == memory_id) {
                    found_in.push(embedder_str);
                }
            }

            if found_in.len() == 1 {
                let found_by = found_in[0].clone();
                // Find the embedder ID to get its "finds" description
                let why_unique = embedder_ids
                    .iter()
                    .find(|e| format!("{:?}", e) == found_by)
                    .map(|e| format!("{} found this because it sees: {}", found_by, e.finds()))
                    .unwrap_or_else(|| format!("{} found this uniquely", found_by));

                unique_finds.push(UniqueFind {
                    memory_id: *memory_id,
                    found_by,
                    why_unique,
                });
            }
        }

        // Step 5: Compute agreement score
        // Score = intersection size / total unique memories found
        let agreement_score = if all_memories.is_empty() {
            0.0
        } else {
            agreement.len() as f32 / all_memories.len() as f32
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Capture counts and audit ID before moving vectors into response
        let agreement_count = agreement.len();
        let unique_finds_count = unique_finds.len();
        let audit_target_id = agreement.first().copied().unwrap_or(Uuid::nil());

        let response = CompareEmbedderViewsResponse {
            query: request.query.clone(),
            rankings,
            agreement,
            unique_finds,
            agreement_score,
            time_ms: elapsed_ms,
        };

        info!(
            embedders = request.embedders.len(),
            agreement_count = agreement_count,
            unique_finds = unique_finds_count,
            agreement_score = agreement_score,
            elapsed_ms = elapsed_ms,
            "compare_embedder_views: Completed embedder comparison"
        );

        // Emit SearchPerformed audit (non-fatal)
        {
            let total_results: usize = response.rankings.iter().map(|r| r.results.len()).sum();
            let audit_record = AuditRecord::new(
                AuditOperation::SearchPerformed {
                    tool_name: "compare_embedder_views".to_string(),
                    results_returned: total_results,
                    weight_profile: None,
                    strategy: Some(format!("compare:{}", request.embedders.join(","))),
                },
                audit_target_id,
            )
            .with_operator("compare_embedder_views")
            .with_parameters(json!({
                "query_preview": request.query.chars().take(100).collect::<String>(),
                "top_k": request.top_k,
                "embedders": request.embedders,
                "agreement_score": agreement_score,
            }));

            if let Err(e) = self
                .teleological_store
                .append_audit_record(&audit_record)
                .await
            {
                error!(error = %e, "compare_embedder_views: Failed to write audit record (non-fatal)");
            }
        }

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "compare_embedder_views: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// list_embedder_indexes tool implementation.
    ///
    /// List all 14 embedder indexes with their statistics.
    pub(crate) async fn call_list_embedder_indexes(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse request (optional params)
        let request: ListEmbedderIndexesRequest =
            serde_json::from_value(args).unwrap_or(ListEmbedderIndexesRequest {});

        if let Err(e) = request.validate() {
            error!(error = %e, "list_embedder_indexes: Validation failed");
            return self.tool_error(id, &e);
        }

        info!("list_embedder_indexes: Listing all 14 embedder indexes");

        // Get actual memory count from store - FAIL FAST on error
        let total_memories = match self.teleological_store.count().await {
            Ok(count) => count,
            Err(e) => {
                error!(error = %e, "list_embedder_indexes: Failed to get memory count");
                return self
                    .tool_error(id, &format!("Failed to get memory count from store: {}", e));
            }
        };

        // Build index info for all 14 embedders
        use super::embedder_dtos::EmbedderId;

        let all_embedders = [
            EmbedderId::E1,
            EmbedderId::E2,
            EmbedderId::E3,
            EmbedderId::E4,
            EmbedderId::E5,
            EmbedderId::E6,
            EmbedderId::E7,
            EmbedderId::E8,
            EmbedderId::E9,
            EmbedderId::E10,
            EmbedderId::E11,
            EmbedderId::E12,
            EmbedderId::E13,
            EmbedderId::E14,
        ];

        let indexes: Vec<EmbedderIndexInfo> = all_embedders
            .iter()
            .map(|e| {
                let index_type = if e.is_sparse() {
                    "Inverted"
                } else if matches!(e, EmbedderId::E12) {
                    "MaxSim"
                } else {
                    "HNSW"
                };

                let topic_weight = if e.is_temporal() {
                    0.0
                } else if e.is_semantic() {
                    1.0
                } else {
                    0.5
                };

                let category = if e.is_temporal() {
                    "Temporal"
                } else if e.is_semantic() {
                    "Semantic"
                } else if matches!(e, EmbedderId::E8 | EmbedderId::E11) {
                    "Relational"
                } else {
                    "Structural"
                };

                // MED-5 FIX: Differentiate vector_count by embedder type.
                // - Dense HNSW embedders (E1-E5, E7-E11, E14): 1 vector per memory -> total_memories
                // - Sparse inverted (E6, E13): term postings, not vectors → None
                // - ColBERT MaxSim (E12): multiple token vectors per memory → None
                let vector_count = if e.is_sparse() || matches!(e, EmbedderId::E12) {
                    None
                } else {
                    Some(total_memories)
                };

                EmbedderIndexInfo {
                    embedder: format!("{:?}", e),
                    name: e.name().to_string(),
                    finds: e.finds().to_string(),
                    dimension: e.dimension().to_string(),
                    index_type: index_type.to_string(),
                    vector_count,
                    // Size not available: would require querying actual HNSW/FAISS index stats
                    size_mb: None,
                    // GPU residency cannot be verified at runtime without querying CUDA allocator
                    // Per Constitution ARCH-GPU-04, GPU is expected but we can't confirm
                    gpu_resident: None,
                    topic_weight,
                    category: category.to_string(),
                }
            })
            .collect();

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = ListEmbedderIndexesResponse {
            indexes,
            total_memories,
            // VRAM not available: would require querying CUDA allocator for actual usage
            total_vram_mb: None,
        };

        info!(
            embedder_count = NUM_EMBEDDERS,
            elapsed_ms = elapsed_ms,
            "list_embedder_indexes: Listed all embedder indexes"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "list_embedder_indexes: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// get_memory_fingerprint tool implementation.
    ///
    /// Retrieve per-embedder fingerprint vectors for a specific memory.
    /// Shows dimension, L2 norm, presence, and asymmetric variants for each embedder.
    pub(crate) async fn call_get_memory_fingerprint(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        let request: GetMemoryFingerprintRequest =
            match self.parse_request(id.clone(), args, "get_memory_fingerprint") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let memory_uuid = match Uuid::parse_str(&request.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!(error = %e, memory_id = %request.memory_id, "get_memory_fingerprint: Invalid UUID");
                return self.tool_error(id, &format!("Invalid UUID: {}", e));
            }
        };

        info!(memory_id = %memory_uuid, "get_memory_fingerprint: Retrieving fingerprint");

        // Retrieve the fingerprint from the store — FAIL FAST
        let fingerprint = match self.teleological_store.retrieve(memory_uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                warn!(memory_id = %memory_uuid, "get_memory_fingerprint: Memory not found");
                return self.tool_error(id, &format!("Memory {} not found", memory_uuid));
            }
            Err(e) => {
                error!(error = %e, memory_id = %memory_uuid, "get_memory_fingerprint: Retrieve FAILED");
                return self.tool_error(
                    id,
                    &format!("Failed to retrieve memory {}: {}", memory_uuid, e),
                );
            }
        };

        let sem = &fingerprint.semantic;
        let filter = request.embedder_filter();
        let show_all = filter.is_empty();

        // Helper: compute L2 norm of a dense vector
        fn l2_norm(v: &[f32]) -> f32 {
            v.iter().map(|x| x * x).sum::<f32>().sqrt()
        }

        use super::embedder_dtos::EmbedderId;

        let all_embedders = [
            EmbedderId::E1,
            EmbedderId::E2,
            EmbedderId::E3,
            EmbedderId::E4,
            EmbedderId::E5,
            EmbedderId::E6,
            EmbedderId::E7,
            EmbedderId::E8,
            EmbedderId::E9,
            EmbedderId::E10,
            EmbedderId::E11,
            EmbedderId::E12,
            EmbedderId::E13,
            EmbedderId::E14,
        ];

        let mut embedder_infos: Vec<EmbedderVectorInfo> = Vec::new();
        let mut present_count = 0usize;

        for eid in &all_embedders {
            if !show_all && !filter.contains(eid) {
                continue;
            }

            let (present, actual_dim, norm_val, variants) = match eid {
                EmbedderId::E1 => {
                    let v = &sem.e1_semantic;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E2 => {
                    let v = &sem.e2_temporal_recent;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E3 => {
                    let v = &sem.e3_temporal_periodic;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E4 => {
                    let v = &sem.e4_temporal_positional;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E5 => {
                    // Asymmetric: cause + effect variants
                    let cause = &sem.e5_causal_as_cause;
                    let effect = &sem.e5_causal_as_effect;
                    let active = sem.e5_active_vector();
                    let p = !active.is_empty();
                    let variants = Some(vec![
                        AsymmetricVariant {
                            variant: "cause".to_string(),
                            present: !cause.is_empty(),
                            dimension: cause.len(),
                            l2_norm: if request.include_vector_norms && !cause.is_empty() {
                                Some(l2_norm(cause))
                            } else {
                                None
                            },
                        },
                        AsymmetricVariant {
                            variant: "effect".to_string(),
                            present: !effect.is_empty(),
                            dimension: effect.len(),
                            l2_norm: if request.include_vector_norms && !effect.is_empty() {
                                Some(l2_norm(effect))
                            } else {
                                None
                            },
                        },
                    ]);
                    (p, active.len(), l2_norm(active), variants)
                }
                EmbedderId::E6 => {
                    // Sparse
                    let nnz = sem.e6_sparse.nnz();
                    (nnz > 0, nnz, 0.0, None)
                }
                EmbedderId::E7 => {
                    let v = &sem.e7_code;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E8 => {
                    // Asymmetric: source + target variants
                    let source = &sem.e8_graph_as_source;
                    let target = &sem.e8_graph_as_target;
                    let active = sem.e8_active_vector();
                    let p = !active.is_empty();
                    let variants = Some(vec![
                        AsymmetricVariant {
                            variant: "source".to_string(),
                            present: !source.is_empty(),
                            dimension: source.len(),
                            l2_norm: if request.include_vector_norms && !source.is_empty() {
                                Some(l2_norm(source))
                            } else {
                                None
                            },
                        },
                        AsymmetricVariant {
                            variant: "target".to_string(),
                            present: !target.is_empty(),
                            dimension: target.len(),
                            l2_norm: if request.include_vector_norms && !target.is_empty() {
                                Some(l2_norm(target))
                            } else {
                                None
                            },
                        },
                    ]);
                    (p, active.len(), l2_norm(active), variants)
                }
                EmbedderId::E9 => {
                    let v = &sem.e9_hdc;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E10 => {
                    // Asymmetric: paraphrase + context variants
                    let paraphrase_vec = &sem.e10_multimodal_paraphrase;
                    let context = &sem.e10_multimodal_as_context;
                    let active = sem.e10_active_vector();
                    let p = !active.is_empty();
                    let variants = Some(vec![
                        AsymmetricVariant {
                            variant: "paraphrase".to_string(),
                            present: !paraphrase_vec.is_empty(),
                            dimension: paraphrase_vec.len(),
                            l2_norm: if request.include_vector_norms && !paraphrase_vec.is_empty() {
                                Some(l2_norm(paraphrase_vec))
                            } else {
                                None
                            },
                        },
                        AsymmetricVariant {
                            variant: "context".to_string(),
                            present: !context.is_empty(),
                            dimension: context.len(),
                            l2_norm: if request.include_vector_norms && !context.is_empty() {
                                Some(l2_norm(context))
                            } else {
                                None
                            },
                        },
                    ]);
                    (p, active.len(), l2_norm(active), variants)
                }
                EmbedderId::E11 => {
                    let v = &sem.e11_entity;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
                EmbedderId::E12 => {
                    // Late interaction: Vec<Vec<f32>> - each token has a 128D vector
                    let p = !sem.e12_late_interaction.is_empty();
                    let tokens = sem.e12_late_interaction.len();
                    // L2 norm = norm of flattened matrix
                    let norm = if p {
                        sem.e12_late_interaction
                            .iter()
                            .flat_map(|v| v.iter())
                            .map(|x| x * x)
                            .sum::<f32>()
                            .sqrt()
                    } else {
                        0.0
                    };
                    (p, tokens, norm, None)
                }
                EmbedderId::E13 => {
                    // Sparse
                    let nnz = sem.e13_splade.nnz();
                    (nnz > 0, nnz, 0.0, None)
                }
                EmbedderId::E14 => {
                    // BGE-M3 Dense — first-class 1024D field on SemanticFingerprint.
                    // Legacy records deserialize with an empty Vec via serde(default).
                    let v = &sem.e14_bge_m3_dense;
                    (!v.is_empty(), v.len(), l2_norm(v), None)
                }
            };

            if present {
                present_count += 1;
            }

            let norm_option = if request.include_vector_norms && present && !eid.is_sparse() {
                Some(norm_val)
            } else {
                None
            };

            embedder_infos.push(EmbedderVectorInfo {
                embedder: format!("{:?}", eid),
                name: eid.name().to_string(),
                dimension: eid.dimension().to_string(),
                present,
                actual_dimension: actual_dim,
                l2_norm: norm_option,
                variants,
            });
        }

        // Optionally get content
        let content = if request.include_content {
            match self
                .teleological_store
                .get_content_batch(&[memory_uuid])
                .await
            {
                Ok(contents) => contents.into_iter().next().flatten(),
                Err(e) => {
                    error!(error = %e, "get_memory_fingerprint: Content retrieval FAILED");
                    return self.tool_error(id, &format!("Failed to retrieve content: {}", e));
                }
            }
        } else {
            None
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = GetMemoryFingerprintResponse {
            memory_id: memory_uuid,
            embedders: embedder_infos,
            embedders_present: present_count,
            active_embedder_count: active_embedder_count(),
            storage_slot_count: NUM_EMBEDDERS,
            disabled_embedders: disabled_embedder_names()
                .into_iter()
                .map(str::to_string)
                .collect(),
            content,
            created_at: fingerprint.created_at.to_rfc3339(),
        };

        info!(
            memory_id = %memory_uuid,
            embedders_present = present_count,
            active_embedder_count = active_embedder_count(),
            storage_slot_count = NUM_EMBEDDERS,
            disabled_embedders = ?disabled_embedder_names(),
            elapsed_ms = elapsed_ms,
            "get_memory_fingerprint: Completed fingerprint introspection"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "get_memory_fingerprint: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// create_weight_profile tool implementation.
    ///
    /// Creates a session-scoped custom weight profile that can be referenced
    /// by name in search_graph and get_unified_neighbors.
    pub(crate) async fn call_create_weight_profile(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: CreateWeightProfileRequest =
            match self.parse_request(id.clone(), args, "create_weight_profile") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        // Reject names that conflict with built-in profiles
        let built_in = context_graph_core::weights::get_profile_names();
        if built_in.contains(&request.name.as_str()) {
            error!(name = %request.name, "create_weight_profile: Name conflicts with built-in profile");
            return self.tool_error(
                id,
                &format!(
                    "Profile name '{}' conflicts with a built-in profile. Built-in profiles: {}",
                    request.name,
                    built_in.join(", ")
                ),
            );
        }

        // Convert to weight array and validate
        let weights = request.to_weight_array();
        if !context_graph_core::weights::E5_CAUSAL_ENABLED && weights[4] > 0.0 {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E5 causal embedder is retired and disabled; custom E5 weight must be 0.0",
            );
        }
        if !context_graph_core::weights::E11_ENTITY_ENABLED && weights[10] > 0.0 {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E11 entity embedder is disabled; custom E11 weight must be 0.0",
            );
        }
        if let Err(e) = context_graph_core::weights::validate_weights(&weights) {
            error!(error = %e, "create_weight_profile: Weight validation failed");
            return self.tool_error(id, &format!("Invalid weights: {}", e));
        }

        // M7 FIX: Enforce maximum custom weight profiles to prevent unbounded HashMap growth.
        // Check count BEFORE persisting to avoid orphaned RocksDB entries on rejection.
        {
            let profiles = self.custom_profiles.read();
            // Allow overwrite of existing profile (not counted as new)
            if profiles.len() >= MAX_CUSTOM_PROFILES && !profiles.contains_key(&request.name) {
                error!(
                    current_count = profiles.len(),
                    max = MAX_CUSTOM_PROFILES,
                    "create_weight_profile: Maximum profile count reached"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Maximum number of custom weight profiles ({}) reached. Delete unused profiles first.",
                        MAX_CUSTOM_PROFILES
                    ),
                );
            }
        }

        // Persist to RocksDB (source of truth) AND update in-memory cache
        if let Err(e) = self
            .teleological_store
            .store_custom_weight_profile(&request.name, &weights)
            .await
        {
            error!(error = %e, name = %request.name, "create_weight_profile: Failed to persist to RocksDB");
            return self.tool_error(id, &format!("Failed to persist weight profile: {}", e));
        }

        let total = {
            let mut profiles = self.custom_profiles.write();
            profiles.insert(request.name.clone(), weights);
            profiles.len()
        };

        // Build response weight map
        let weight_map: HashMap<String, f32> = [
            ("E1", weights[0]),
            ("E2", weights[1]),
            ("E3", weights[2]),
            ("E4", weights[3]),
            ("E5", weights[4]),
            ("E6", weights[5]),
            ("E7", weights[6]),
            ("E8", weights[7]),
            ("E9", weights[8]),
            ("E10", weights[9]),
            ("E11", weights[10]),
            ("E12", weights[11]),
            ("E13", weights[12]),
        ]
        .into_iter()
        .filter(|(_, v)| *v > 0.0)
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let response = CreateWeightProfileResponse {
            name: request.name.clone(),
            weights: weight_map,
            description: request.description,
            total_custom_profiles: total,
        };

        info!(
            name = %request.name,
            total_profiles = total,
            "create_weight_profile: Profile created"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "create_weight_profile: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// search_cross_embedder_anomalies tool implementation.
    ///
    /// Finds memories that score high in one embedder but low in another,
    /// revealing blind spots and perspective disagreements.
    pub(crate) async fn call_search_cross_embedder_anomalies(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        let request: SearchCrossEmbedderAnomaliesRequest =
            match self.parse_request(id.clone(), args, "search_cross_embedder_anomalies") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        // MED-22 FIX: Use proper error handling instead of .expect() - defense in depth
        let high_eid = match EmbedderId::parse(&request.high_embedder) {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.high_embedder, "search_cross_embedder_anomalies: Invalid high_embedder");
                return self.tool_error(
                    id,
                    &format!("Invalid high_embedder '{}'", request.high_embedder),
                );
            }
        };
        let low_eid = match EmbedderId::parse(&request.low_embedder) {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.low_embedder, "search_cross_embedder_anomalies: Invalid low_embedder");
                return self.tool_error(
                    id,
                    &format!("Invalid low_embedder '{}'", request.low_embedder),
                );
            }
        };
        let high_idx = high_eid.to_index();
        let low_idx = low_eid.to_index();
        if !context_graph_core::weights::E5_CAUSAL_ENABLED
            && (matches!(high_eid, EmbedderId::E5) || matches!(low_eid, EmbedderId::E5))
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "E5 causal embedder is retired and disabled; select active embedders for anomaly search",
            );
        }
        if !context_graph_core::weights::E11_ENTITY_ENABLED
            && (matches!(high_eid, EmbedderId::E11) || matches!(low_eid, EmbedderId::E11))
        {
            error!(
                high = %request.high_embedder,
                low = %request.low_embedder,
                "search_cross_embedder_anomalies: rejected disabled E11 request"
            );
            return self.tool_error_typed(id, ToolErrorKind::Validation, E11_DISABLED_TOOL_MESSAGE);
        }

        info!(
            query = %request.query,
            high = %request.high_embedder,
            low = %request.low_embedder,
            "search_cross_embedder_anomalies: Starting anomaly search"
        );

        // Step 1: Embed the query
        let query_fingerprint = match self
            .embed_query(
                id.clone(),
                &request.query,
                "search_cross_embedder_anomalies",
            )
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search in the HIGH embedder's space (get more candidates than topK for filtering)
        //
        // EB-L1: E1Only strategy intentionally populates only E1 scores in results.
        // Cross-embedder anomaly detection compares HIGH vs LOW embedder scores, which
        // are computed post-retrieval below (not from the search strategy's score map).
        // Full anomaly detection across all 14 embedders requires MultiSpace strategy.
        let search_k = (request.top_k * 5).min(100);
        let mut options = TeleologicalSearchOptions::quick(search_k)
            .with_strategy(SearchStrategy::E1Only)
            .with_embedders(vec![high_idx])
            .with_min_similarity(0.0); // no threshold - we filter ourselves
                                       // AP-77: E5 requires causal direction for scoring
        if matches!(high_eid, EmbedderId::E5) || matches!(low_eid, EmbedderId::E5) {
            options = options.with_causal_direction(CausalDirection::Cause);
        }

        let candidates = match self
            .teleological_store
            .search_semantic(&query_fingerprint, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_cross_embedder_anomalies: Search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let total_searched = candidates.len();

        // Step 3: Filter for anomalies (high in one, low in other)
        let mut anomalies: Vec<EmbedderAnomaly> = candidates
            .iter()
            .filter_map(|cand| {
                let high_score = cand.embedder_scores[high_idx];
                let low_score = cand.embedder_scores[low_idx];

                if high_score >= request.high_threshold && low_score <= request.low_threshold {
                    Some(EmbedderAnomaly {
                        memory_id: cand.fingerprint.id,
                        high_score,
                        low_score,
                        anomaly_score: high_score - low_score,
                        content: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by anomaly score descending
        anomalies.sort_by(|a, b| b.anomaly_score.total_cmp(&a.anomaly_score));
        anomalies.truncate(request.top_k);

        // Step 4: Fetch content if requested
        if request.include_content && !anomalies.is_empty() {
            let ids: Vec<Uuid> = anomalies.iter().map(|a| a.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "search_cross_embedder_anomalies: Content retrieval FAILED");
                    return self.tool_error(id, &format!("Content retrieval failed: {}", e));
                }
            };
            for (anomaly, content) in anomalies.iter_mut().zip(contents) {
                anomaly.content = content;
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let blind_spot_rate = if total_searched > 0 {
            anomalies.len() as f32 / total_searched as f32
        } else {
            0.0
        };

        let response = SearchCrossEmbedderAnomaliesResponse {
            query: request.query.clone(),
            high_embedder: request.high_embedder.clone(),
            high_embedder_finds: high_eid.finds().to_string(),
            low_embedder: request.low_embedder.clone(),
            low_embedder_finds: low_eid.finds().to_string(),
            anomalies,
            total_searched,
            search_time_ms: elapsed_ms,
            scoring_method: "difference".to_string(),
            scoring_formula: "anomaly_score = high_score - low_score".to_string(),
            high_threshold: request.high_threshold,
            low_threshold: request.low_threshold,
            search_multiplier: 5,
            primary_embedder: request.primary_embedder.clone(),
            contrast_embedder: request.contrast_embedder.clone(),
            blind_spot_rate,
        };

        info!(
            anomalies = response.anomalies.len(),
            total_searched = total_searched,
            elapsed_ms = elapsed_ms,
            "search_cross_embedder_anomalies: Completed"
        );

        // Emit SearchPerformed audit (non-fatal)
        {
            let anomaly_ids: Vec<Uuid> = response.anomalies.iter().map(|a| a.memory_id).collect();
            let audit_record = AuditRecord::new(
                AuditOperation::SearchPerformed {
                    tool_name: "search_cross_embedder_anomalies".to_string(),
                    results_returned: response.anomalies.len(),
                    weight_profile: None,
                    strategy: Some(format!(
                        "anomaly:{}>{}",
                        request.high_embedder, request.low_embedder
                    )),
                },
                anomaly_ids.first().copied().unwrap_or(Uuid::nil()),
            )
            .with_operator("search_cross_embedder_anomalies")
            .with_parameters(json!({
                "query_preview": request.query.chars().take(100).collect::<String>(),
                "top_k": request.top_k,
                "high_embedder": request.high_embedder,
                "low_embedder": request.low_embedder,
                "high_threshold": request.high_threshold,
                "low_threshold": request.low_threshold,
            }));

            if let Err(e) = self
                .teleological_store
                .append_audit_record(&audit_record)
                .await
            {
                error!(error = %e, "search_cross_embedder_anomalies: Failed to write audit record (non-fatal)");
            }
        }

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_cross_embedder_anomalies: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// search_by_tokens tool (E12 ColBERT MaxSim).
    ///
    /// Uses pipeline strategy (E13 recall → E1 dense → multi-space scoring) which
    /// is the only search path that incorporates E12 token-level matching.
    /// E12 uses MaxSim scoring (not HNSW) so it cannot be queried as a standalone
    /// embedder index — it participates through the pipeline's reranking stages.
    pub(crate) async fn call_search_by_tokens(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        use super::embedder_dtos::{
            SearchByTokensRequest, SearchByTokensResponse, TokenSearchResult,
        };

        let req: SearchByTokensRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => return self.tool_error(id, &format!("Invalid arguments: {}", e)),
        };

        let top_k = req.top_k.min(100);
        let start = Instant::now();

        // Embed query to get all 14 embeddings (including E12 token vectors)
        let query_fp = match self
            .embed_query(id.clone(), &req.query, "search_by_tokens")
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Use pipeline strategy — E13 SPLADE recall + E1 dense + multi-space scoring.
        // E12 MaxSim participates through the pipeline's late-interaction reranking.
        let options = TeleologicalSearchOptions::quick(top_k)
            .with_strategy(SearchStrategy::Pipeline)
            .with_min_similarity(req.min_similarity);

        match self
            .teleological_store
            .search_semantic(&query_fp, options)
            .await
        {
            Ok(results) => {
                let token_results: Vec<TokenSearchResult> = results
                    .iter()
                    .take(top_k)
                    .map(|r| TokenSearchResult {
                        memory_id: r.fingerprint.id.to_string(),
                        score: r.similarity,
                    })
                    .collect();
                let count = token_results.len();
                let resp = SearchByTokensResponse {
                    results: token_results,
                    count,
                    latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                };
                match serde_json::to_value(&resp) {
                    Ok(v) => self.tool_result(id, v),
                    Err(e) => self.tool_error(id, &format!("Serialization failed: {}", e)),
                }
            }
            Err(e) => self.tool_error(id, &format!("E12 token search failed: {}", e)),
        }
    }

    /// search_by_expansion tool (E13 SPLADE).
    ///
    /// Uses pipeline strategy which starts with E13 SPLADE sparse recall
    /// (inverted index, not HNSW) for enhanced keyword expansion matching,
    /// followed by multi-space scoring for precision.
    pub(crate) async fn call_search_by_expansion(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        use super::embedder_dtos::{
            ExpansionSearchResult, SearchByExpansionRequest, SearchByExpansionResponse,
        };

        let req: SearchByExpansionRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => return self.tool_error(id, &format!("Invalid arguments: {}", e)),
        };

        let top_k = req.top_k.min(100);
        let start = Instant::now();

        // Embed query to get all 14 embeddings (including E13 SPLADE sparse terms)
        let query_fp = match self
            .embed_query(id.clone(), &req.query, "search_by_expansion")
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Use pipeline strategy — Stage 1 uses E13 SPLADE inverted index for recall,
        // followed by E1 dense scoring and multi-space fusion.
        let options = TeleologicalSearchOptions::quick(top_k)
            .with_strategy(SearchStrategy::Pipeline)
            .with_min_similarity(req.min_score);

        match self
            .teleological_store
            .search_semantic(&query_fp, options)
            .await
        {
            Ok(results) => {
                let expansion_results: Vec<ExpansionSearchResult> = results
                    .iter()
                    .take(top_k)
                    .map(|r| ExpansionSearchResult {
                        memory_id: r.fingerprint.id.to_string(),
                        score: r.similarity,
                    })
                    .collect();
                let count = expansion_results.len();
                let resp = SearchByExpansionResponse {
                    results: expansion_results,
                    count,
                    latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                };
                match serde_json::to_value(&resp) {
                    Ok(v) => self.tool_result(id, v),
                    Err(e) => self.tool_error(id, &format!("Serialization failed: {}", e)),
                }
            }
            Err(e) => self.tool_error(id, &format!("E13 expansion search failed: {}", e)),
        }
    }

    pub(crate) async fn call_mejepa_load_learner_state(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        let config_path = match required_str(&arguments, "modelsConfigPath") {
            Ok(value) => PathBuf::from(value),
            Err(message) => return self.tool_error(id, &message),
        };
        let embedder = match required_str(&arguments, "embedder").and_then(|value| {
            value
                .parse::<MejepaEmbedderId>()
                .map_err(|err| err.to_string())
        }) {
            Ok(value) => value,
            Err(message) => return self.tool_error(id, &message),
        };
        let cert_path = arguments
            .get("calibrationCertPath")
            .and_then(|value| value.as_str())
            .map(PathBuf::from);
        match MejepaEnsemble::load_learner_state(&config_path, embedder, cert_path.as_deref()) {
            Ok(loaded) => self.tool_result(
                id,
                json!({
                    "sourceOfTruth": config_path.display().to_string(),
                    "loaded": loaded,
                    "passes": true
                }),
            ),
            Err(err) => self.tool_error(
                id,
                &format!(
                    "{}: {}. sourceOfTruth={}",
                    err.code(),
                    err,
                    config_path.display()
                ),
            ),
        }
    }

    pub(crate) async fn call_mejepa_routing_lookup(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        let language = match required_str(&arguments, "language").and_then(|value| {
            AstLanguage::from_slug(value).map_err(|err| format!("{}: {}", err.code(), err))
        }) {
            Ok(value) => value,
            Err(message) => return self.tool_error(id, &message),
        };
        let entity_type =
            match required_str(&arguments, "entityType").and_then(parse_ast_entity_type) {
                Ok(value) => value,
                Err(message) => return self.tool_error(id, &message),
            };
        match mejepa_route_for_entity_type(entity_type, language) {
            Ok(route) => self.tool_result(id, json!({"route": route, "passes": true})),
            Err(err) => self.tool_error(id, &format!("{}: {}", err.code(), err)),
        }
    }

    pub(crate) async fn call_mejepa_vram_budget_report(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        let budget = match arguments
            .get("budget")
            .and_then(|value| value.as_str())
            .unwrap_or("content_set")
        {
            "content_set" => MejepaVramBudget::content_set_rtx5090(),
            "full_phase1" => MejepaVramBudget::full_phase1_rtx5090(),
            other => {
                return self.tool_error(
                    id,
                    &format!(
                        "MEJEPA_EMBED_INVALID_INPUT: unsupported budget {:?}; use content_set or full_phase1",
                        other
                    ),
                )
            }
        };
        let nvidia_smi_diagnostic = match nvidia_smi_query(gpu_wsl_default_query()).await {
            Ok(value) => json!({
                "status": "available",
                "unavailable": !value.unavailable_fields.is_empty(),
                "telemetry": value
            }),
            Err(err) => json!({
                "status": "unavailable",
                "unavailable": true,
                "fault": err
            }),
        };
        match mejepa_query_vram_budget(budget) {
            Ok(report) => self.tool_result(
                id,
                json!({
                    "report": report,
                    "passes": true,
                    "sourceOfTruth": "CUDA driver cuMemGetInfo_v2",
                    "nvidiaSmiDiagnostic": nvidia_smi_diagnostic
                }),
            ),
            Err(err) => self.tool_error(id, &format!("{}: {}", err.code(), err)),
        }
    }
}

fn required_str<'a>(arguments: &'a serde_json::Value, field: &str) -> Result<&'a str, String> {
    arguments
        .get(field)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "MEJEPA_EMBED_INVALID_INPUT: missing required non-empty string field {}",
                field
            )
        })
}

fn parse_ast_entity_type(value: &str) -> Result<AstEntityType, String> {
    match value {
        "function" => Ok(AstEntityType::Function),
        "method" => Ok(AstEntityType::Method),
        "class" => Ok(AstEntityType::Class),
        "struct" => Ok(AstEntityType::Struct),
        "enum" => Ok(AstEntityType::Enum),
        "trait_or_interface" => Ok(AstEntityType::TraitOrInterface),
        "impl" => Ok(AstEntityType::Impl),
        "module" => Ok(AstEntityType::Module),
        "namespace" => Ok(AstEntityType::Namespace),
        "test_function" => Ok(AstEntityType::TestFunction),
        "import" => Ok(AstEntityType::Import),
        "comment_block" => Ok(AstEntityType::CommentBlock),
        "docstring" => Ok(AstEntityType::Docstring),
        other => Err(format!(
            "MEJEPA_EMBED_INVALID_INPUT: unsupported entityType {:?}",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::embedder_dtos::EmbedderId;

    #[test]
    fn test_embedder_id_properties() {
        // Test E1 (foundation)
        assert_eq!(EmbedderId::E1.to_index(), 0);
        assert!(EmbedderId::E1.is_semantic());
        assert!(!EmbedderId::E1.is_temporal());
        assert!(!EmbedderId::E1.is_sparse());

        // Test E11 (entity)
        assert_eq!(EmbedderId::E11.to_index(), 10);
        assert!(!EmbedderId::E11.is_semantic());
        assert!(!EmbedderId::E11.is_temporal());

        // Test E6 (sparse)
        assert!(EmbedderId::E6.is_sparse());
        assert!(EmbedderId::E6.is_semantic());

        // Test E2 (temporal)
        assert!(EmbedderId::E2.is_temporal());
        assert!(!EmbedderId::E2.is_semantic());
    }
}
