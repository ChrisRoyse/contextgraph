//! # Embedder-First Search Tool Implementations
//!
//! Per Constitution v6.3, these tools enable AI agents to search using any of the
//! 13 embedders as the primary perspective.
//!
//! ## Philosophy
//!
//! Each embedder sees the knowledge graph from a unique perspective:
//! - E1 (semantic): Dense semantic similarity - foundation
//! - E11 (entity): Entity knowledge via KEPLER - finds "Diesel" when searching "database"
//! - E7 (code): Code patterns - finds `tokio::spawn` when searching for "async runtime"
//! - E5 (causal): Cause-effect chains - finds "migration broke production"
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
use std::time::Instant;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::embedder_dtos::{
    CompareEmbedderViewsRequest, CompareEmbedderViewsResponse, EmbedderCluster,
    EmbedderIndexInfo, EmbedderRanking, EmbedderSearchResult, GetEmbedderClustersRequest,
    GetEmbedderClustersResponse, ListEmbedderIndexesRequest, ListEmbedderIndexesResponse,
    RankedMemory, SearchByEmbedderRequest, SearchByEmbedderResponse, UniqueFind,
};

use super::super::Handlers;

impl Handlers {
    /// search_by_embedder tool implementation.
    ///
    /// Search using any embedder (E1-E13) as the primary perspective.
    ///
    /// # Algorithm
    ///
    /// 1. Parse and validate embedder selection
    /// 2. Create query embedding for all 13 spaces
    /// 3. Search in the selected embedder's space
    /// 4. Optionally compute scores from all 13 embedders
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
        let request: SearchByEmbedderRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_by_embedder: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_by_embedder: Validation failed");
            return self.tool_error(id, &e);
        }

        let embedder_id = match request.embedder_id() {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.embedder, "search_by_embedder: Invalid embedder");
                return self.tool_error(
                    id,
                    &format!("Invalid embedder: {}", request.embedder),
                );
            }
        };

        let embedder_index = embedder_id.to_index();

        info!(
            embedder = %request.embedder,
            embedder_name = %embedder_id.name(),
            query = %request.query,
            top_k = request.top_k,
            "search_by_embedder: Starting embedder-first search"
        );

        // Step 1: Create query embedding (all 13 embedders)
        let query_fingerprint = match self.multi_array_provider.embed_all(&request.query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_by_embedder: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search in the selected embedder's space
        let options = TeleologicalSearchOptions::quick(request.top_k)
            .with_strategy(SearchStrategy::E1Only) // Strategy doesn't matter with explicit embedders
            .with_embedders(vec![embedder_index])
            .with_min_similarity(request.min_similarity);

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
                    &format!(
                        "Search in {} space failed: {}",
                        embedder_id.name(),
                        e
                    ),
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

        // Step 3: Get content if requested
        let candidate_ids: Vec<Uuid> = candidates.iter().map(|c| c.fingerprint.id).collect();
        let contents = if request.include_content {
            match self.teleological_store.get_content_batch(&candidate_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "search_by_embedder: Content retrieval failed, continuing without content");
                    vec![None; candidate_ids.len()]
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

                // Optionally compute all scores if requested
                // Note: For now, we only return the primary embedder's score
                // Full all_scores would require accessing all 13 embedder similarities
                // which is expensive. This is a TODO for full implementation.
                let all_scores = if request.include_all_scores {
                    // Placeholder: would need to search all 13 spaces for this memory
                    // For now, just return None - full implementation would be expensive
                    None
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

        self.tool_result(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    /// get_embedder_clusters tool implementation.
    ///
    /// Explore clusters of memories in a specific embedder's space.
    ///
    /// # Algorithm
    ///
    /// 1. Get all memories from the system
    /// 2. Run HDBSCAN clustering in the selected embedder's space
    /// 3. Return top clusters with optional samples
    ///
    /// # Note
    ///
    /// This is a simplified implementation. Full clustering would use
    /// cuML HDBSCAN on GPU. This version uses a placeholder that
    /// groups by similarity threshold.
    pub(crate) async fn call_get_embedder_clusters(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse and validate request
        let request: GetEmbedderClustersRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_embedder_clusters: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "get_embedder_clusters: Validation failed");
            return self.tool_error(id, &e);
        }

        let embedder_id = match request.embedder_id() {
            Some(eid) => eid,
            None => {
                error!(embedder = %request.embedder, "get_embedder_clusters: Invalid embedder");
                return self.tool_error(
                    id,
                    &format!("Invalid embedder: {}", request.embedder),
                );
            }
        };

        info!(
            embedder = %request.embedder,
            embedder_name = %embedder_id.name(),
            min_cluster_size = request.min_cluster_size,
            "get_embedder_clusters: Starting cluster exploration"
        );

        // For now, return a placeholder response.
        // Full implementation would:
        // 1. Get all fingerprints from the store
        // 2. Extract the selected embedder's vectors
        // 3. Run cuML HDBSCAN clustering
        // 4. Return clusters with samples

        // Check if we have any memories
        let sample_query = "technology software programming";
        let sample_fingerprint = match self.multi_array_provider.embed_all(sample_query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "get_embedder_clusters: Sample embedding FAILED");
                return self.tool_error(id, &format!("Embedding failed: {}", e));
            }
        };

        let options = TeleologicalSearchOptions::quick(100)
            .with_strategy(SearchStrategy::E1Only)
            .with_min_similarity(0.0);

        let all_memories = match self
            .teleological_store
            .search_semantic(&sample_fingerprint, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "get_embedder_clusters: Memory scan FAILED");
                return self.tool_error(id, &format!("Memory scan failed: {}", e));
            }
        };

        let total_memories = all_memories.len();

        // Simple clustering: group by similarity buckets
        // This is a placeholder - real implementation would use HDBSCAN
        let mut clusters: Vec<EmbedderCluster> = Vec::new();

        if total_memories >= request.min_cluster_size {
            // Create a single "main cluster" with samples
            let sample_ids: Vec<Uuid> = all_memories
                .iter()
                .take(request.samples_per_cluster)
                .map(|m| m.fingerprint.id)
                .collect();

            // Get content for samples
            let sample_snippets = if request.include_samples {
                match self.teleological_store.get_content_batch(&sample_ids).await {
                    Ok(contents) => Some(
                        contents
                            .iter()
                            .map(|c| {
                                c.as_ref()
                                    .map(|s| {
                                        if s.len() > 100 {
                                            format!("{}...", &s[..100])
                                        } else {
                                            s.clone()
                                        }
                                    })
                                    .unwrap_or_else(|| "[no content]".to_string())
                            })
                            .collect(),
                    ),
                    Err(_) => None,
                }
            } else {
                None
            };

            clusters.push(EmbedderCluster {
                cluster_id: 0,
                size: total_memories,
                sample_ids: if request.include_samples {
                    Some(sample_ids)
                } else {
                    None
                },
                sample_snippets,
                description: Some(format!(
                    "Main cluster in {} space",
                    embedder_id.name()
                )),
            });
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = GetEmbedderClustersResponse {
            embedder: request.embedder.clone(),
            embedder_perspective: embedder_id.finds().to_string(),
            clusters: clusters.clone(),
            total_memories,
            total_clusters: clusters.len(),
            time_ms: elapsed_ms,
        };

        info!(
            embedder = %request.embedder,
            clusters = clusters.len(),
            total_memories = total_memories,
            elapsed_ms = elapsed_ms,
            "get_embedder_clusters: Completed cluster exploration"
        );

        self.tool_result(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
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
        let request: CompareEmbedderViewsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "compare_embedder_views: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "compare_embedder_views: Validation failed");
            return self.tool_error(id, &e);
        }

        let embedder_ids = request.embedder_ids();

        info!(
            embedders = ?request.embedders,
            query = %request.query,
            top_k = request.top_k,
            "compare_embedder_views: Starting embedder comparison"
        );

        // Step 1: Create query embedding (all 13 embedders)
        let query_fingerprint = match self.multi_array_provider.embed_all(&request.query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "compare_embedder_views: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search each embedder
        let mut rankings: Vec<EmbedderRanking> = Vec::new();
        let mut all_memory_sets: Vec<HashSet<Uuid>> = Vec::new();
        let mut embedder_results: HashMap<String, Vec<(Uuid, f32)>> = HashMap::new();

        for embedder_id in &embedder_ids {
            let embedder_index = embedder_id.to_index();

            let options = TeleologicalSearchOptions::quick(request.top_k)
                .with_strategy(SearchStrategy::E1Only)
                .with_embedders(vec![embedder_index])
                .with_min_similarity(0.0);

            let candidates = match self
                .teleological_store
                .search_semantic(&query_fingerprint, options)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    warn!(
                        error = %e,
                        embedder = ?embedder_id,
                        "compare_embedder_views: Search in {} failed, skipping",
                        embedder_id.name()
                    );
                    continue;
                }
            };

            let mut memory_set: HashSet<Uuid> = HashSet::new();
            let mut results_list: Vec<(Uuid, f32)> = Vec::new();
            let mut ranked_memories: Vec<RankedMemory> = Vec::new();

            // Get the embedder index for extracting the correct score
            let embedder_index = embedder_id.to_index();

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
                    content: None, // Content would be fetched if include_content=true
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

        let response = CompareEmbedderViewsResponse {
            query: request.query.clone(),
            rankings,
            agreement: agreement.clone(),
            unique_finds: unique_finds.clone(),
            agreement_score,
            time_ms: elapsed_ms,
        };

        info!(
            embedders = request.embedders.len(),
            agreement_count = agreement.len(),
            unique_finds = unique_finds.len(),
            agreement_score = agreement_score,
            elapsed_ms = elapsed_ms,
            "compare_embedder_views: Completed embedder comparison"
        );

        self.tool_result(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }

    /// list_embedder_indexes tool implementation.
    ///
    /// List all 13 embedder indexes with their statistics.
    pub(crate) async fn call_list_embedder_indexes(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let start = Instant::now();

        // Parse request (optional params)
        let request: ListEmbedderIndexesRequest =
            serde_json::from_value(args.clone()).unwrap_or(ListEmbedderIndexesRequest {
                include_details: true,
            });

        if let Err(e) = request.validate() {
            error!(error = %e, "list_embedder_indexes: Validation failed");
            return self.tool_error(id, &e);
        }

        info!("list_embedder_indexes: Listing all 13 embedder indexes");

        // Get actual memory count from store
        let total_memories = match self.teleological_store.count().await {
            Ok(count) => count,
            Err(e) => {
                warn!(error = %e, "list_embedder_indexes: Failed to get memory count");
                0
            }
        };

        // Build index info for all 13 embedders
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

                EmbedderIndexInfo {
                    embedder: format!("{:?}", e),
                    name: e.name().to_string(),
                    finds: e.finds().to_string(),
                    dimension: e.dimension().to_string(),
                    index_type: index_type.to_string(),
                    vector_count: total_memories,
                    size_mb: if request.include_details {
                        Some(0.0) // Would be calculated from actual index
                    } else {
                        None
                    },
                    gpu_resident: true, // Per Constitution, all embedders are GPU-resident
                    topic_weight,
                    category: category.to_string(),
                }
            })
            .collect();

        let elapsed_ms = start.elapsed().as_millis() as u64;

        let response = ListEmbedderIndexesResponse {
            indexes,
            total_memories,
            total_vram_mb: if request.include_details {
                Some(10.0) // Approximate VRAM for all embedders
            } else {
                None
            },
        };

        info!(
            embedder_count = 13,
            elapsed_ms = elapsed_ms,
            "list_embedder_indexes: Listed all embedder indexes"
        );

        self.tool_result(
            id,
            serde_json::to_value(response).unwrap_or_else(|_| json!({})),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_id_properties() {
        use super::super::embedder_dtos::EmbedderId;

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
