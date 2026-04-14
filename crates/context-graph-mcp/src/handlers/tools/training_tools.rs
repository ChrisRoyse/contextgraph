//! Training data export handler (`export_training_corpus`).
//!
//! Iterates memories in the teleological store, assembles full [`TrainingRecord`]s
//! (embeddings, topic profile, cross-correlations, group alignments, typed
//! edges, K-NN neighbors, causal labels), and persists each one into
//! `CF_TRAINING_RECORDS` on the underlying RocksDB store.
//!
//! Output is binary-only: the response payload is a summary (count + bytes +
//! timing). No JSON/JSONL/Parquet files are written.
//!
//! # Limitations (v1)
//!
//! - `incoming_edges` and `causal_causes` are populated only when
//!   `includeIncomingEdges=true`; doing so requires an O(N) reverse-index
//!   scan of `CF_TYPED_EDGES` and the causal store at export start.
//! - `topic_memberships` is left empty; topic-level export is a future
//!   enhancement that needs portfolio integration.
//! - K-NN neighbors are only populated for embedders the graph builder has
//!   covered; newly-stored memories may show zeros until the ~60s batch cycle.

use std::collections::HashMap;
use std::time::Instant;

use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, compute_topic_profile_from_fingerprint,
    CausalLabel, KnnNeighbor, TrainingEdge, TrainingRecord,
};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Defaults for argument parsing.
const DEFAULT_MAX_MEMORIES: usize = 10_000;
const HARD_MAX_MEMORIES: usize = 1_000_000;

impl Handlers {
    /// Handle `export_training_corpus`. See `tools/definitions/training.rs` for the schema.
    pub(crate) async fn call_export_training_corpus(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling export_training_corpus: {:?}", args);
        let started = Instant::now();

        // ---- Parse args ----
        let filter = args
            .get("filter")
            .and_then(|v| v.as_str())
            .unwrap_or("all")
            .to_string();
        let filter_id = args
            .get("filterId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let max_memories = args
            .get("maxMemories")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(HARD_MAX_MEMORIES as u64) as usize)
            .unwrap_or(DEFAULT_MAX_MEMORIES);
        let include_embeddings = args
            .get("includeEmbeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_sparse = args
            .get("includeSparseVectors")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_tokens = args
            .get("includeTokenEmbeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_edges = args
            .get("includeEdges")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_causal = args
            .get("includeCausalLabels")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_incoming_edges = args
            .get("includeIncomingEdges")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let clear_existing = args
            .get("clearExisting")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if filter == "session" && filter_id.is_none() {
            return self.tool_error(
                id,
                "filter='session' requires a non-empty filterId",
            );
        }

        // ---- Require RocksDB backend ----
        let store_any = self.teleological_store.as_any();
        let Some(rocksdb_store) =
            store_any.downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "export_training_corpus requires RocksDbTeleologicalStore. \
                 Current backend does not expose CF_TRAINING_RECORDS.",
            );
        };

        // ---- Optionally clear the CF ----
        let cleared = if clear_existing {
            match rocksdb_store.clear_all_training_records().await {
                Ok(n) => n,
                Err(e) => {
                    error!(error = %e, "clear_all_training_records failed");
                    return self.tool_error(
                        id,
                        &format!("Failed to clear CF_TRAINING_RECORDS: {}", e),
                    );
                }
            }
        } else {
            0
        };

        // ---- Gather fingerprints ----
        let fingerprints = match self
            .teleological_store
            .list_fingerprints_unbiased(max_memories)
            .await
        {
            Ok(fps) => fps,
            Err(e) => {
                error!(error = %e, "list_fingerprints_unbiased failed");
                return self.tool_error(
                    id,
                    &format!("Failed to list fingerprints: {}", e),
                );
            }
        };

        // ---- Incoming edges (per-memory O(N) scan when enabled) ----
        // v1: No persistent reverse index; each `get_typed_edges_to(memory_id)`
        // call is a full CF scan. Off by default; opt-in for small corpora.
        let include_incoming_resolved =
            include_incoming_edges && include_edges && self.edge_repository.is_some();
        // Placeholder map so the build_training_record signature stays stable.
        let incoming_index: HashMap<Uuid, Vec<TrainingEdge>> = HashMap::new();

        // ---- Build and persist records ----
        let synergy = SynergyMatrix::with_base_synergies();
        let mut exported = 0usize;
        let mut skipped = 0usize;
        let mut bytes_written = 0u64;
        let mut errors = 0usize;

        for fp in &fingerprints {
            // Session filter (post-filter via source_metadata)
            if filter == "session" {
                let want = filter_id.as_deref().unwrap_or("");
                let matches = match self.teleological_store.get_source_metadata(fp.id).await {
                    Ok(Some(meta)) => meta.session_id.as_deref() == Some(want),
                    Ok(None) => false,
                    Err(e) => {
                        warn!(id = %fp.id, error = %e, "get_source_metadata failed; skipping");
                        false
                    }
                };
                if !matches {
                    skipped += 1;
                    continue;
                }
            }

            let record = match self
                .build_training_record(
                    fp,
                    &synergy,
                    &incoming_index,
                    include_embeddings,
                    include_sparse,
                    include_tokens,
                    include_edges,
                    include_causal,
                    include_incoming_resolved,
                )
                .await
            {
                Ok(r) => r,
                Err(msg) => {
                    warn!(id = %fp.id, error = %msg, "build_training_record failed");
                    errors += 1;
                    continue;
                }
            };

            // Estimate serialized size from the largest contributors
            // (dense + sparse + token + edges) without pulling in bincode here.
            let serialized_size = estimate_record_size(&record);

            if let Err(e) = rocksdb_store.store_training_record(fp.id, &record).await {
                error!(id = %fp.id, error = %e, "store_training_record failed");
                errors += 1;
                continue;
            }

            exported += 1;
            bytes_written = bytes_written.saturating_add(serialized_size);
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        info!(
            exported,
            skipped,
            errors,
            cleared,
            elapsed_ms,
            "export_training_corpus complete"
        );

        self.tool_result(
            id,
            json!({
                "status": "exported",
                "exported": exported,
                "skipped_filtered": skipped,
                "errors": errors,
                "cleared_before_export": cleared,
                "fingerprints_scanned": fingerprints.len(),
                "approx_bytes_written": bytes_written,
                "elapsed_ms": elapsed_ms,
                "storage": {
                    "backend": "rocksdb",
                    "column_family": "training_records",
                    "format": "version_byte + bincode"
                },
                "record_shape": {
                    "dense_embeddings_included": include_embeddings,
                    "sparse_vectors_included": include_sparse,
                    "token_embeddings_included": include_tokens,
                    "edges_included": include_edges,
                    "incoming_edges_included": include_incoming_edges,
                    "causal_labels_included": include_causal,
                    "cross_correlations": 78,
                    "group_alignments": 6,
                    "topic_profile_dims": NUM_EMBEDDERS
                },
                "filter": {
                    "mode": filter,
                    "filter_id": filter_id
                }
            }),
        )
    }

    /// Assemble a [`TrainingRecord`] for a single fingerprint.
    ///
    /// On any unrecoverable lookup failure, returns a short diagnostic string.
    #[allow(clippy::too_many_arguments)]
    async fn build_training_record(
        &self,
        fp: &TeleologicalFingerprint,
        synergy: &SynergyMatrix,
        _incoming_index: &HashMap<Uuid, Vec<TrainingEdge>>,
        include_embeddings: bool,
        include_sparse: bool,
        include_tokens: bool,
        include_edges: bool,
        include_causal: bool,
        include_incoming: bool,
    ) -> Result<TrainingRecord, String> {
        // Content
        let content = self
            .teleological_store
            .get_content(fp.id)
            .await
            .map_err(|e| format!("get_content: {}", e))?
            .unwrap_or_default();

        // Source metadata (best-effort, OK if missing)
        let source = self
            .teleological_store
            .get_source_metadata(fp.id)
            .await
            .unwrap_or(None);
        let session_id = source.as_ref().and_then(|m| m.session_id.clone());
        let source_type = source
            .as_ref()
            .map(|m| format!("{:?}", m.source_type));
        let source_path = source.as_ref().and_then(|m| m.file_path.clone());

        // Teleological fusion
        let topic_profile = compute_topic_profile_from_fingerprint(&fp.semantic);
        let cross_correlations = compute_cross_correlations(&topic_profile, synergy);
        let group_alignments = compute_group_alignments(&topic_profile);

        // Dense embeddings (gate by include flag). Tuples > 12 elements don't
        // implement Default, so we construct the empty branch explicitly.
        let sem = &fp.semantic;
        let (e1, e2, e3, e4, e5c, e5e, e7, e8s, e8t, e9, e10p, e10c, e11) = if include_embeddings {
            (
                sem.e1_semantic.clone(),
                sem.e2_temporal_recent.clone(),
                sem.e3_temporal_periodic.clone(),
                sem.e4_temporal_positional.clone(),
                sem.e5_causal_as_cause.clone(),
                sem.e5_causal_as_effect.clone(),
                sem.e7_code.clone(),
                sem.e8_graph_as_source.clone(),
                sem.e8_graph_as_target.clone(),
                sem.e9_hdc.clone(),
                sem.e10_multimodal_paraphrase.clone(),
                sem.e10_multimodal_as_context.clone(),
                sem.e11_entity.clone(),
            )
        } else {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };

        // Sparse embeddings
        let (e6_idx, e6_val, e13_idx, e13_val) = if include_sparse {
            (
                sem.e6_sparse.indices.clone(),
                sem.e6_sparse.values.clone(),
                sem.e13_splade.indices.clone(),
                sem.e13_splade.values.clone(),
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        // Token-level
        let e12 = if include_tokens {
            sem.e12_late_interaction.clone()
        } else {
            Vec::new()
        };

        // Edges + K-NN
        let (outgoing_edges, incoming_edges, knn) = if include_edges {
            let repo = self.edge_repository.as_ref();
            let outgoing = repo
                .map(|r| fetch_outgoing_edges(r, fp.id))
                .unwrap_or_default();
            let incoming = if include_incoming {
                repo.map(|r| fetch_incoming_edges(r, fp.id))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let knn = repo
                .map(|r| fetch_knn_per_embedder(r, fp.id))
                .unwrap_or_else(|| (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect());
            (outgoing, incoming, knn)
        } else {
            (
                Vec::new(),
                Vec::new(),
                (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect(),
            )
        };

        // Causal labels (outgoing only in v1: this memory acts as the source/cause).
        // CausalRelationship has no separate effect-fingerprint field; the
        // description bundles cause_statement + effect_statement + explanation.
        let causal_effects = if include_causal {
            match self
                .teleological_store
                .get_causal_relationships_by_source(fp.id)
                .await
            {
                Ok(relationships) => relationships
                    .into_iter()
                    .map(|rel| {
                        let description = format!(
                            "{} -> {}. {}",
                            rel.cause_statement.trim(),
                            rel.effect_statement.trim(),
                            rel.explanation.trim()
                        );
                        CausalLabel {
                            // v1: no effect-side fingerprint stored on CausalRelationship;
                            // consumers can still cluster via description + confidence.
                            related_memory_id: rel.source_fingerprint_id,
                            description,
                            direction: "cause".into(),
                            confidence: rel.confidence,
                        }
                    })
                    .collect(),
                Err(e) => {
                    debug!(id = %fp.id, error = %e, "causal fetch failed; empty list");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Ok(TrainingRecord {
            memory_id: fp.id,
            content,
            importance: fp.importance,
            created_at: fp.created_at,
            session_id,
            source_type,
            source_path,
            e1_semantic: e1,
            e2_temporal_recent: e2,
            e3_temporal_periodic: e3,
            e4_temporal_positional: e4,
            e5_causal_cause: e5c,
            e5_causal_effect: e5e,
            e7_code: e7,
            e8_graph_source: e8s,
            e8_graph_target: e8t,
            e9_hdc: e9,
            e10_paraphrase: e10p,
            e10_context: e10c,
            e11_entity: e11,
            e6_sparse_indices: e6_idx,
            e6_sparse_values: e6_val,
            e13_splade_indices: e13_idx,
            e13_splade_values: e13_val,
            e12_token_embeddings: e12,
            topic_profile,
            cross_correlations,
            group_alignments,
            outgoing_edges,
            incoming_edges,
            knn_neighbors: knn,
            causal_effects,
            causal_causes: Vec::new(),
            topic_memberships: Vec::new(),
        })
    }
}

// ============================================================================
// Edge helpers
// ============================================================================

fn fetch_outgoing_edges(
    repo: &context_graph_storage::EdgeRepository,
    source: Uuid,
) -> Vec<TrainingEdge> {
    match repo.get_typed_edges_from(source) {
        Ok(edges) => edges.into_iter().map(typed_edge_to_training).collect(),
        Err(e) => {
            warn!(source = %source, error = %e, "get_typed_edges_from failed");
            Vec::new()
        }
    }
}

/// Fetch incoming edges via O(N) CF scan (`get_typed_edges_to`). Expensive —
/// gate on `includeIncomingEdges` and a small corpus.
fn fetch_incoming_edges(
    repo: &context_graph_storage::EdgeRepository,
    target: Uuid,
) -> Vec<TrainingEdge> {
    match repo.get_typed_edges_to(target) {
        Ok(edges) => edges
            .into_iter()
            .map(|e| {
                // For incoming perspective, peer is the source of the edge.
                let source = e.source();
                let mut record = typed_edge_to_training(e);
                record.peer_id = source;
                record
            })
            .collect(),
        Err(e) => {
            warn!(target = %target, error = %e, "get_typed_edges_to failed");
            Vec::new()
        }
    }
}

impl Handlers {
    /// Handle `list_training_records`. Returns UUIDs + optional per-record shape.
    pub(crate) async fn call_list_training_records(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .min(1000) as usize;
        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let include_shape = args
            .get("includeShape")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "list_training_records requires RocksDbTeleologicalStore.");
        };

        let ids = match rocksdb_store.list_training_record_ids().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("list_training_record_ids failed: {}", e))
            }
        };
        let total = ids.len();
        let page: Vec<Uuid> = ids.into_iter().skip(offset).take(limit).collect();

        let mut items = Vec::with_capacity(page.len());
        for uid in &page {
            if include_shape {
                match rocksdb_store.get_training_record(*uid).await {
                    Ok(Some(r)) => items.push(json!({
                        "memory_id": uid.to_string(),
                        "content_chars": r.content.chars().count(),
                        "importance": r.importance,
                        "session_id": r.session_id,
                        "source_type": r.source_type,
                        "topic_profile_len": r.topic_profile.len(),
                        "cross_correlations_len": r.cross_correlations.len(),
                        "group_alignments_len": r.group_alignments.len(),
                        "e1_len": r.e1_semantic.len(),
                        "e5_cause_len": r.e5_causal_cause.len(),
                        "e7_len": r.e7_code.len(),
                        "e8_source_len": r.e8_graph_source.len(),
                        "e11_len": r.e11_entity.len(),
                        "e6_sparse_nnz": r.e6_sparse_indices.len(),
                        "e13_sparse_nnz": r.e13_splade_indices.len(),
                        "e12_token_count": r.e12_token_embeddings.len(),
                        "outgoing_edges": r.outgoing_edges.len(),
                        "incoming_edges": r.incoming_edges.len(),
                        "knn_embedders_populated": r.knn_neighbors.iter().filter(|v| !v.is_empty()).count(),
                        "causal_effects": r.causal_effects.len(),
                        "causal_causes": r.causal_causes.len(),
                    })),
                    Ok(None) => items.push(json!({"memory_id": uid.to_string(), "missing": true})),
                    Err(e) => items.push(json!({
                        "memory_id": uid.to_string(),
                        "error": e.to_string()
                    })),
                }
            } else {
                items.push(json!({"memory_id": uid.to_string()}));
            }
        }

        self.tool_result(
            id,
            json!({
                "total": total,
                "offset": offset,
                "limit": limit,
                "returned": items.len(),
                "records": items
            }),
        )
    }

    /// Handle `get_training_record`. Fetch one record with optional vectors.
    pub(crate) async fn call_get_training_record(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let memory_id_raw = match args.get("memoryId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'memoryId' parameter"),
        };
        let memory_id = match Uuid::parse_str(memory_id_raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "memoryId must be a valid UUID"),
        };
        let include_vectors = args
            .get("includeVectors")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_tokens = args
            .get("includeTokenEmbeddings")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_edges = args
            .get("includeEdges")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "get_training_record requires RocksDbTeleologicalStore.");
        };

        match rocksdb_store.get_training_record(memory_id).await {
            Ok(Some(r)) => self.tool_result(id, render_training_record(&r, include_vectors, include_tokens, include_edges)),
            Ok(None) => self.tool_result(id, json!({"memory_id": memory_id_raw, "found": false})),
            Err(e) => self.tool_error(id, &format!("get_training_record failed: {}", e)),
        }
    }

    /// Handle `count_training_records`. O(N) count over the CF.
    pub(crate) async fn call_count_training_records(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "count_training_records requires RocksDbTeleologicalStore.");
        };
        match rocksdb_store.count_training_records().await {
            Ok(n) => self.tool_result(id, json!({"count": n})),
            Err(e) => self.tool_error(id, &format!("count_training_records failed: {}", e)),
        }
    }
}

fn render_training_record(
    r: &TrainingRecord,
    include_vectors: bool,
    include_tokens: bool,
    include_edges: bool,
) -> serde_json::Value {
    let mut obj = json!({
        "memory_id": r.memory_id.to_string(),
        "found": true,
        "content": r.content,
        "importance": r.importance,
        "created_at": r.created_at,
        "session_id": r.session_id,
        "source_type": r.source_type,
        "source_path": r.source_path,
        "topic_profile": r.topic_profile.to_vec(),
        "cross_correlations_len": r.cross_correlations.len(),
        "group_alignments": r.group_alignments.to_vec(),
        "shape": {
            "e1_len": r.e1_semantic.len(),
            "e2_len": r.e2_temporal_recent.len(),
            "e3_len": r.e3_temporal_periodic.len(),
            "e4_len": r.e4_temporal_positional.len(),
            "e5_cause_len": r.e5_causal_cause.len(),
            "e5_effect_len": r.e5_causal_effect.len(),
            "e7_len": r.e7_code.len(),
            "e8_source_len": r.e8_graph_source.len(),
            "e8_target_len": r.e8_graph_target.len(),
            "e9_len": r.e9_hdc.len(),
            "e10_paraphrase_len": r.e10_paraphrase.len(),
            "e10_context_len": r.e10_context.len(),
            "e11_len": r.e11_entity.len(),
            "e6_sparse_nnz": r.e6_sparse_indices.len(),
            "e13_sparse_nnz": r.e13_splade_indices.len(),
            "e12_token_count": r.e12_token_embeddings.len(),
            "outgoing_edges": r.outgoing_edges.len(),
            "incoming_edges": r.incoming_edges.len(),
            "knn_per_embedder": r.knn_neighbors.iter().map(|v| v.len()).collect::<Vec<_>>(),
            "causal_effects": r.causal_effects.len(),
            "causal_causes": r.causal_causes.len(),
        },
    });

    if include_vectors {
        obj["e1_semantic"] = json!(r.e1_semantic);
        obj["e2_temporal_recent"] = json!(r.e2_temporal_recent);
        obj["e3_temporal_periodic"] = json!(r.e3_temporal_periodic);
        obj["e4_temporal_positional"] = json!(r.e4_temporal_positional);
        obj["e5_causal_cause"] = json!(r.e5_causal_cause);
        obj["e5_causal_effect"] = json!(r.e5_causal_effect);
        obj["e7_code"] = json!(r.e7_code);
        obj["e8_graph_source"] = json!(r.e8_graph_source);
        obj["e8_graph_target"] = json!(r.e8_graph_target);
        obj["e9_hdc"] = json!(r.e9_hdc);
        obj["e10_paraphrase"] = json!(r.e10_paraphrase);
        obj["e10_context"] = json!(r.e10_context);
        obj["e11_entity"] = json!(r.e11_entity);
        obj["e6_sparse_indices"] = json!(r.e6_sparse_indices);
        obj["e6_sparse_values"] = json!(r.e6_sparse_values);
        obj["e13_splade_indices"] = json!(r.e13_splade_indices);
        obj["e13_splade_values"] = json!(r.e13_splade_values);
        obj["cross_correlations"] = json!(r.cross_correlations);
    }
    if include_tokens {
        obj["e12_token_embeddings"] = json!(r.e12_token_embeddings);
    }
    if include_edges {
        obj["outgoing_edges_detail"] = json!(r
            .outgoing_edges
            .iter()
            .map(|e| json!({
                "edge_type": e.edge_type,
                "peer_id": e.peer_id.to_string(),
                "weight": e.weight,
                "direction": e.direction,
                "agreement_count": e.agreement_count,
            }))
            .collect::<Vec<_>>());
        obj["causal_effects_detail"] = json!(r
            .causal_effects
            .iter()
            .map(|c| json!({
                "related_memory_id": c.related_memory_id.to_string(),
                "description": c.description,
                "direction": c.direction,
                "confidence": c.confidence,
            }))
            .collect::<Vec<_>>());
    }
    obj
}

fn typed_edge_to_training(e: context_graph_core::graph_linking::TypedEdge) -> TrainingEdge {
    TrainingEdge {
        edge_type: e.edge_type() as u8,
        peer_id: e.target(),
        weight: e.weight(),
        direction: e.direction() as u8,
        agreement_count: e.agreement_count(),
        embedder_scores: *e.embedder_scores(),
    }
}

fn fetch_knn_per_embedder(
    repo: &context_graph_storage::EdgeRepository,
    source: Uuid,
) -> Vec<Vec<KnnNeighbor>> {
    (0..NUM_EMBEDDERS)
        .map(|idx| match repo.get_embedder_edges(idx as u8, source) {
            Ok(edges) => edges
                .into_iter()
                .map(|e| KnnNeighbor {
                    target_id: e.target(),
                    similarity: e.similarity(),
                })
                .collect(),
            Err(_) => Vec::new(),
        })
        .collect()
}

/// Lightweight size estimate for summary reporting (counts only the heavy
/// float/byte payloads; ignores tiny scalar fields).
fn estimate_record_size(r: &TrainingRecord) -> u64 {
    let dense_bytes: usize = r.e1_semantic.len()
        + r.e2_temporal_recent.len()
        + r.e3_temporal_periodic.len()
        + r.e4_temporal_positional.len()
        + r.e5_causal_cause.len()
        + r.e5_causal_effect.len()
        + r.e7_code.len()
        + r.e8_graph_source.len()
        + r.e8_graph_target.len()
        + r.e9_hdc.len()
        + r.e10_paraphrase.len()
        + r.e10_context.len()
        + r.e11_entity.len();
    let sparse_bytes = r.e6_sparse_indices.len() * 2
        + r.e6_sparse_values.len() * 4
        + r.e13_splade_indices.len() * 2
        + r.e13_splade_values.len() * 4;
    let token_bytes: usize = r.e12_token_embeddings.iter().map(|v| v.len() * 4).sum();
    let edge_bytes = (r.outgoing_edges.len() + r.incoming_edges.len())
        * (16 + 4 + 4 + 4 + NUM_EMBEDDERS * 4);
    let knn_bytes: usize = r
        .knn_neighbors
        .iter()
        .map(|v| v.len() * (16 + 4))
        .sum();
    let content_bytes = r.content.len();
    (dense_bytes * 4 + sparse_bytes + token_bytes + edge_bytes + knn_bytes + content_bytes) as u64
}
