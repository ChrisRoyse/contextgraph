//! Training data export handlers (`export_training_corpus`,
//! `list_training_records`, `get_training_record`, `count_training_records`).
//!
//! Iterates memories in the teleological store, assembles full
//! [`TrainingRecord`]s (embeddings, topic profile, cross-correlations, group
//! alignments, typed edges, K-NN neighbors, causal labels, temporal labels),
//! and persists each one into `CF_TRAINING_RECORDS` on the underlying RocksDB
//! store.
//!
//! Output is binary-only: the response payload is a summary (count + bytes +
//! timing). No JSON/JSONL/Parquet files are written.
//!
//! # Code-simplifier fixes applied (Phase 1)
//!
//! 1. `CausalLabel` carries `rel_id` + optional `mechanism_type`; the peer
//!    end stays `Uuid::nil()` until per-relationship peer resolution is wired
//!    up.
//! 2. Topic profile is read from `CF_TOPIC_PROFILES` per memory via
//!    `get_topic_profile`; the fallback derivation runs only when the row is
//!    missing.
//! 3. Session filter fetches `source_metadata` **once** per memory and reuses
//!    the value for record population.
//! 4. `includeIncomingEdges=true` builds a single reverse-index HashMap over
//!    `CF_TYPED_EDGES` at export start — O(N) once, not O(N²).
//! 5. Phase-5 temporal labels are populated when `includeTemporalLabels` is
//!    true (default); the export uses a single `Utc::now()` snapshot for the
//!    whole run.
//! 6. `list_training_records(includeShape=true)` batches RocksDB reads via
//!    `multi_get_training_records`.

use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use context_graph_core::graph_linking::TypedEdge;
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::tucker::{
    CpuTuckerCompressor, TuckerCompressor, DEFAULT_RANKS,
};
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, extract_temporal_labels,
    topic_profile_or_fallback, CausalLabel, KnnNeighbor, TrainingEdge, TrainingRecord,
    NUM_CROSS_CORRELATIONS,
};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_core::types::SourceMetadata;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::EdgeRepository;
use serde_json::json;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Structured errors surfaced by training-corpus assembly helpers.
///
/// Fixes Sherlock findings F-011 and F-012: previously `fetch_knn_per_embedder`
/// and `fetch_outgoing_edges` silently returned empty `Vec`s on
/// `EdgeRepository` errors, leaving callers unable to distinguish "no
/// neighbors / no outgoing edges" from "the K-NN / typed-edge column family is
/// broken." Training records were silently truncated.
///
/// Variants are mapped 1:1 into the MCP JSON-RPC response so consumers see
/// the structured failure (`code` + `source_id` + `embedder_idx` + `cause`).
#[derive(Debug, Error)]
pub enum TrainingToolError {
    /// `EdgeRepository::get_typed_edges_from` or `get_embedder_edges` failed.
    /// `embedder_idx` is `None` for typed-edge sweeps and `Some(idx)` for K-NN
    /// per-embedder fetches.
    #[error(
        "TRAINING_TOOL_EDGE_REPOSITORY_FETCH_FAILED: source={source_id} \
         embedder_idx={embedder_idx:?} cause={cause}"
    )]
    EdgeRepositoryFetchFailed {
        source_id: Uuid,
        embedder_idx: Option<u8>,
        cause: String,
    },
}

impl TrainingToolError {
    /// Stable machine-readable error code for MCP response surfaces.
    pub fn code(&self) -> &'static str {
        match self {
            Self::EdgeRepositoryFetchFailed { .. } => "TRAINING_TOOL_EDGE_REPOSITORY_FETCH_FAILED",
        }
    }
}

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
        let include_temporal_labels = args
            .get("includeTemporalLabels")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_tucker_core = args
            .get("includeTuckerCore")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let clear_existing = args
            .get("clearExisting")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // ---- Validate session filter ----
        if filter == "session" && filter_id.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
            return self.tool_error(id, "filter='session' requires a non-empty filterId");
        }

        // ---- Require RocksDB backend ----
        let store_any = self.teleological_store.as_any();
        let Some(rocksdb_store) = store_any.downcast_ref::<RocksDbTeleologicalStore>() else {
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
                    return self
                        .tool_error(id, &format!("Failed to clear CF_TRAINING_RECORDS: {}", e));
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
                return self.tool_error(id, &format!("Failed to list fingerprints: {}", e));
            }
        };

        // ---- Reverse index for incoming edges (fix #4: build once) ----
        let include_incoming_resolved =
            include_incoming_edges && include_edges && self.edge_repository.is_some();
        let incoming_index: HashMap<Uuid, Vec<TrainingEdge>> = if include_incoming_resolved {
            match self
                .edge_repository
                .as_ref()
                .map(EdgeRepository::iter_all_typed_edges)
                .transpose()
            {
                Ok(Some(all_edges)) => build_incoming_index(&all_edges),
                Ok(None) => HashMap::new(),
                Err(e) => {
                    warn!(error = %e, "iter_all_typed_edges failed; incoming edges will be empty");
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        // ---- Build and persist records ----
        let synergy = SynergyMatrix::with_base_synergies();
        let export_now = Utc::now();
        let mut exported = 0usize;
        let mut skipped = 0usize;
        let mut bytes_written = 0u64;
        let mut errors = 0usize;
        // F-011 / F-012: surface per-embedder K-NN and typed-edge fetch failures
        // separately from generic build/store errors so MCP consumers can tell
        // "edge CF is broken" from "topic profile fetch returned empty."
        // Capped at HARD_MAX_MEMORIES so the JSON response stays bounded.
        let mut fetch_failures: Vec<serde_json::Value> = Vec::new();
        const FETCH_FAILURES_CAP: usize = 1024;

        for fp in &fingerprints {
            // Fetch source metadata ONCE per memory (fix #3). Used for both
            // the session filter and the record's session_id/source_path.
            let source = self
                .teleological_store
                .get_source_metadata(fp.id)
                .await
                .unwrap_or(None);

            // Session filter: reuse the already-fetched source.
            if filter == "session" {
                let want = filter_id.as_deref().unwrap_or("");
                let matches = source
                    .as_ref()
                    .and_then(|m| m.session_id.as_deref())
                    .map(|s| s == want)
                    .unwrap_or(false);
                if !matches {
                    skipped += 1;
                    continue;
                }
            }

            // Topic profile: prefer CF_TOPIC_PROFILES; fall back to
            // fingerprint-magnitude derivation only when the row is missing (fix #2).
            let stored_profile = match rocksdb_store.get_topic_profile(fp.id).await {
                Ok(p) => p,
                Err(e) => {
                    warn!(id = %fp.id, error = %e, "get_topic_profile failed; using fallback");
                    None
                }
            };
            let topic_profile = topic_profile_or_fallback(stored_profile, &fp.semantic);

            let record = match self
                .build_training_record(
                    fp,
                    source,
                    topic_profile,
                    &synergy,
                    &incoming_index,
                    export_now,
                    include_embeddings,
                    include_sparse,
                    include_tokens,
                    include_edges,
                    include_causal,
                    include_incoming_resolved,
                    include_temporal_labels,
                    include_tucker_core,
                )
                .await
            {
                Ok(r) => r,
                Err(msg) => {
                    warn!(id = %fp.id, error = %msg, "build_training_record failed");
                    errors += 1;
                    // F-011 / F-012: surface structured edge-repository failures
                    // in the MCP response. The error code prefix is the stable
                    // identifier; we also stash the raw message for diagnosis.
                    if msg.starts_with("TRAINING_TOOL_EDGE_REPOSITORY_FETCH_FAILED")
                        && fetch_failures.len() < FETCH_FAILURES_CAP
                    {
                        fetch_failures.push(json!({
                            "code": "TRAINING_TOOL_EDGE_REPOSITORY_FETCH_FAILED",
                            "memory_id": fp.id.to_string(),
                            "message": msg,
                        }));
                    }
                    continue;
                }
            };

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
            skipped, errors, cleared, elapsed_ms, "export_training_corpus complete"
        );

        let fetch_failure_count = fetch_failures.len();
        self.tool_result(
            id,
            json!({
                "status": "exported",
                "exported": exported,
                "skipped_filtered": skipped,
                "errors": errors,
                "edge_repository_fetch_failure_count": fetch_failure_count,
                "edge_repository_fetch_failures": fetch_failures,
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
                    "incoming_edges_included": include_incoming_resolved,
                    "causal_labels_included": include_causal,
                    "temporal_labels_included": include_temporal_labels,
                    "tucker_core_included": include_tucker_core,
                    "cross_correlations": NUM_CROSS_CORRELATIONS,
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
    /// `source` and `topic_profile` are already resolved by the caller (fix
    /// #2/#3). `incoming_index` is the caller-built reverse index (fix #4).
    /// `export_now` is shared across the whole run.
    #[allow(clippy::too_many_arguments)]
    async fn build_training_record(
        &self,
        fp: &TeleologicalFingerprint,
        source: Option<SourceMetadata>,
        topic_profile: [f32; NUM_EMBEDDERS],
        synergy: &SynergyMatrix,
        incoming_index: &HashMap<Uuid, Vec<TrainingEdge>>,
        export_now: chrono::DateTime<Utc>,
        include_embeddings: bool,
        include_sparse: bool,
        include_tokens: bool,
        include_edges: bool,
        include_causal: bool,
        include_incoming: bool,
        include_temporal_labels: bool,
        include_tucker_core: bool,
    ) -> Result<TrainingRecord, String> {
        // Content
        let content = self
            .teleological_store
            .get_content(fp.id)
            .await
            .map_err(|e| format!("get_content: {}", e))?
            .unwrap_or_default();

        let session_id = source.as_ref().and_then(|m| m.session_id.clone());
        let source_type = source.as_ref().map(|m| format!("{:?}", m.source_type));
        let source_path = source.as_ref().and_then(|m| m.file_path.clone());

        // Teleological fusion
        let cross_correlations = compute_cross_correlations(&topic_profile, synergy);
        let group_alignments = compute_group_alignments(&topic_profile);

        // Dense embeddings (gate by include flag).
        let sem = &fp.semantic;
        let (e1, e2, e3, e4, e5c, e5e, e7, e8s, e8t, e9, e10p, e10c, e11, e14) =
            if include_embeddings {
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
                    sem.e14_bge_m3_dense.clone(),
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

        // Edges + K-NN.
        //
        // F-011 / F-012 fix: `fetch_outgoing_edges` and `fetch_knn_per_embedder`
        // now return `Result<_, TrainingToolError>` rather than silently
        // collapsing repo errors to empty `Vec`s. Propagate via `String` into
        // the caller's `Result<TrainingRecord, String>`. The MCP-tool entry
        // point counts these failures into `fetch_failures` so the JSON-RPC
        // response surfaces the structured error to consumers.
        let (outgoing_edges, incoming_edges, knn) =
            if include_edges {
                let repo = self.edge_repository.as_ref();
                let outgoing = match repo {
                    Some(r) => fetch_outgoing_edges(r, fp.id)
                        .map_err(|e| format!("{}: {}", e.code(), e))?,
                    None => Vec::new(),
                };
                let incoming = if include_incoming {
                    incoming_index.get(&fp.id).cloned().unwrap_or_default()
                } else {
                    Vec::new()
                };
                let knn = match repo {
                    Some(r) => fetch_knn_per_embedder(r, fp.id)
                        .map_err(|e| format!("{}: {}", e.code(), e))?,
                    None => (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect(),
                };
                (outgoing, incoming, knn)
            } else {
                (
                    Vec::new(),
                    Vec::new(),
                    (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect(),
                )
            };

        // Causal labels (fix #1: rel_id + mechanism_type, peer stays Uuid::nil()
        // because CausalRelationship has no separate effect-fingerprint field).
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
                            related_memory_id: Uuid::nil(),
                            rel_id: rel.id,
                            description,
                            direction: "cause".into(),
                            confidence: rel.confidence,
                            mechanism_type: Some(rel.mechanism_type.clone()),
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

        // Phase-5 temporal labels. Session sequence is stored on SourceMetadata
        // (when present); session_total is not tracked for stored memories in
        // this codepath, so it's passed as None. relative_position stays None
        // until session bookkeeping is extended.
        let temporal_labels = if include_temporal_labels {
            let session_sequence = source
                .as_ref()
                .and_then(|m| m.session_sequence)
                .and_then(|seq| u32::try_from(seq).ok());
            Some(extract_temporal_labels(
                fp,
                source.as_ref(),
                session_sequence,
                None,
                export_now,
            ))
        } else {
            None
        };

        // Phase-4 Tucker-core decomposition (CPU, streaming HOSVD).
        // Runs against the live SemanticFingerprint, not the gated record fields
        // — so the decomposition quality is independent of includeEmbeddings.
        // Failures are logged and leave tucker_core=None so the rest of the
        // record still persists.
        let tucker_core = if include_tucker_core {
            match CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS) {
                Ok(core) => Some(core),
                Err(e) => {
                    warn!(id = %fp.id, error = %e, "tucker compress failed; tucker_core=None");
                    None
                }
            }
        } else {
            None
        };

        // F3: derive the 8-dim relational signature directly from outgoing
        // TrainingEdges. Matches the storage-layer exporter's formula so the
        // two codepaths produce identical records for the same fingerprint.
        let mut edge_type_distribution =
            [0u32; context_graph_core::training::NUM_EDGE_TYPE_DISTRIBUTION];
        for e in outgoing_edges.iter() {
            let idx = e.edge_type as usize;
            if idx < edge_type_distribution.len() {
                edge_type_distribution[idx] = edge_type_distribution[idx].saturating_add(1);
            }
        }

        Ok(TrainingRecord {
            memory_id: fp.id,
            content,
            importance: fp.importance,
            created_at: fp.created_at,
            session_id,
            source_type,
            source_path,
            content_hash: Some(fp.content_hash),
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
            e14_bge_m3_dense: e14,
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
            temporal_labels,
            tucker_core,
            edge_type_distribution,
        })
    }

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
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let include_shape = args
            .get("includeShape")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "list_training_records requires RocksDbTeleologicalStore.",
            );
        };

        let ids = match rocksdb_store.list_training_record_ids().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("list_training_record_ids failed: {}", e));
            }
        };
        let total = ids.len();
        let page: Vec<Uuid> = ids.into_iter().skip(offset).take(limit).collect();

        let items = if include_shape {
            // Fix #6: single batched RocksDB read.
            match rocksdb_store.multi_get_training_records(&page).await {
                Ok(records) => records
                    .into_iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let uid = page[i];
                        match r {
                            Some(rec) => json!({
                                "memory_id": uid.to_string(),
                                "content_chars": rec.content.chars().count(),
                                "importance": rec.importance,
                                "session_id": rec.session_id,
                                "source_type": rec.source_type,
                                "topic_profile_len": rec.topic_profile.len(),
                                "cross_correlations_len": rec.cross_correlations.len(),
                                "group_alignments_len": rec.group_alignments.len(),
                                "e1_len": rec.e1_semantic.len(),
                                "e5_cause_len": rec.e5_causal_cause.len(),
                                "e7_len": rec.e7_code.len(),
                                "e8_source_len": rec.e8_graph_source.len(),
                                "e11_len": rec.e11_entity.len(),
                                "e14_len": rec.e14_bge_m3_dense.len(),
                                "e6_sparse_nnz": rec.e6_sparse_indices.len(),
                                "e13_sparse_nnz": rec.e13_splade_indices.len(),
                                "e12_token_count": rec.e12_token_embeddings.len(),
                                "outgoing_edges": rec.outgoing_edges.len(),
                                "incoming_edges": rec.incoming_edges.len(),
                                "knn_embedders_populated": rec.knn_neighbors.iter().filter(|v| !v.is_empty()).count(),
                                "causal_effects": rec.causal_effects.len(),
                                "causal_causes": rec.causal_causes.len(),
                                "temporal_labels_present": rec.temporal_labels.is_some(),
                            }),
                            None => json!({
                                "memory_id": uid.to_string(),
                                "missing": true
                            }),
                        }
                    })
                    .collect::<Vec<_>>(),
                Err(e) => {
                    return self
                        .tool_error(id, &format!("multi_get_training_records failed: {}", e));
                }
            }
        } else {
            page.iter()
                .map(|uid| json!({"memory_id": uid.to_string()}))
                .collect()
        };

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
            Ok(Some(r)) => self.tool_result(
                id,
                render_training_record(&r, include_vectors, include_tokens, include_edges),
            ),
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
            return self.tool_error(
                id,
                "count_training_records requires RocksDbTeleologicalStore.",
            );
        };
        match rocksdb_store.count_training_records().await {
            Ok(n) => self.tool_result(id, json!({"count": n})),
            Err(e) => self.tool_error(id, &format!("count_training_records failed: {}", e)),
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Build the reverse index `target_id -> Vec<TrainingEdge>` from a full sweep
/// of `CF_TYPED_EDGES`. O(N) over all typed edges, and O(1) lookup per memory.
fn build_incoming_index(all_edges: &[TypedEdge]) -> HashMap<Uuid, Vec<TrainingEdge>> {
    let mut out: HashMap<Uuid, Vec<TrainingEdge>> = HashMap::new();
    for edge in all_edges {
        let target = edge.target();
        let mut training = typed_edge_to_training_ref(edge);
        // From the incoming perspective, the peer is the edge's source.
        training.peer_id = edge.source();
        out.entry(target).or_default().push(training);
    }
    out
}

/// Fetch outgoing typed edges for `source` and convert to training rows.
///
/// Fail-closed: any `EdgeRepository::get_typed_edges_from` error surfaces as
/// `TrainingToolError::EdgeRepositoryFetchFailed { embedder_idx: None }`. The
/// `warn!` is preserved for diagnostic continuity (the underlying RocksDB
/// error chain is otherwise lost once we stringify the cause in the variant
/// payload), but the caller MUST also see the error in the MCP response.
fn fetch_outgoing_edges(
    repo: &EdgeRepository,
    source: Uuid,
) -> Result<Vec<TrainingEdge>, TrainingToolError> {
    match repo.get_typed_edges_from(source) {
        Ok(edges) => Ok(edges.iter().map(typed_edge_to_training_ref).collect()),
        Err(e) => {
            warn!(source = %source, error = %e, "get_typed_edges_from failed");
            Err(TrainingToolError::EdgeRepositoryFetchFailed {
                source_id: source,
                embedder_idx: None,
                cause: e.to_string(),
            })
        }
    }
}

fn typed_edge_to_training_ref(e: &TypedEdge) -> TrainingEdge {
    TrainingEdge {
        edge_type: e.edge_type() as u8,
        peer_id: e.target(),
        weight: e.weight(),
        direction: e.direction() as u8,
        agreement_count: e.agreement_count(),
        embedder_scores: *e.embedder_scores(),
    }
}

/// Fetch K-NN neighbors per embedder for `source`.
///
/// Fail-closed: the first failing embedder's error is returned as
/// `TrainingToolError::EdgeRepositoryFetchFailed { embedder_idx: Some(idx) }`.
/// Previously `Err(_) => Vec::new()` silently truncated training data; consumers
/// could not distinguish "no neighbors" from "K-NN CF broken."
fn fetch_knn_per_embedder(
    repo: &EdgeRepository,
    source: Uuid,
) -> Result<Vec<Vec<KnnNeighbor>>, TrainingToolError> {
    let mut out = Vec::with_capacity(NUM_EMBEDDERS);
    for idx in 0..NUM_EMBEDDERS {
        let embedder_idx = idx as u8;
        match repo.get_embedder_edges(embedder_idx, source) {
            Ok(edges) => out.push(
                edges
                    .into_iter()
                    .map(|e| KnnNeighbor {
                        target_id: e.target(),
                        similarity: e.similarity(),
                    })
                    .collect(),
            ),
            Err(e) => {
                warn!(
                    source = %source,
                    embedder_idx,
                    error = %e,
                    "get_embedder_edges failed"
                );
                return Err(TrainingToolError::EdgeRepositoryFetchFailed {
                    source_id: source,
                    embedder_idx: Some(embedder_idx),
                    cause: e.to_string(),
                });
            }
        }
    }
    Ok(out)
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
        "content_hash": r.content_hash.as_ref().map(hex::encode),
        "topic_profile": r.topic_profile.to_vec(),
        "cross_correlations_len": r.cross_correlations.len(),
        "group_alignments": r.group_alignments.to_vec(),
        "temporal_labels_present": r.temporal_labels.is_some(),
        "tucker_core_present": r.tucker_core.is_some(),
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
            "e14_len": r.e14_bge_m3_dense.len(),
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

    if let Some(t) = r.temporal_labels.as_ref() {
        obj["temporal_labels"] = json!({
            "stored_at": t.stored_at,
            "stored_hour_utc": t.stored_hour_utc,
            "stored_day_of_week": t.stored_day_of_week,
            "stored_month": t.stored_month,
            "age_seconds_at_export": t.age_seconds_at_export,
            "session_sequence": t.session_sequence,
            "session_total": t.session_total,
            "relative_position": t.relative_position,
            "periodic_bucket": format!("{:?}", t.periodic_bucket),
            "e2_recency_norm": t.e2_recency_norm,
            "e3_periodic_norm": t.e3_periodic_norm,
            "e4_positional_norm": t.e4_positional_norm,
        });
    }

    if let Some(tc) = r.tucker_core.as_ref() {
        obj["tucker_core"] = json!({
            "ranks": [tc.ranks.0, tc.ranks.1, tc.ranks.2],
            "data_len": tc.data.len(),
            "u1_len": tc.u1.len(),
            "u2_len": tc.u2.len(),
            "u3_len": tc.u3.len(),
            "compression_ratio": tc.compression_ratio(),
        });
    }

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
        obj["e14_bge_m3_dense"] = json!(r.e14_bge_m3_dense);
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
        obj["incoming_edges_detail"] = json!(r
            .incoming_edges
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
                "rel_id": c.rel_id.to_string(),
                "description": c.description,
                "direction": c.direction,
                "confidence": c.confidence,
                "mechanism_type": c.mechanism_type,
            }))
            .collect::<Vec<_>>());
    }
    obj
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
        + r.e11_entity.len()
        + r.e14_bge_m3_dense.len();
    let sparse_bytes = r.e6_sparse_indices.len() * 2
        + r.e6_sparse_values.len() * 4
        + r.e13_splade_indices.len() * 2
        + r.e13_splade_values.len() * 4;
    let token_bytes: usize = r.e12_token_embeddings.iter().map(|v| v.len() * 4).sum();
    let edge_bytes =
        (r.outgoing_edges.len() + r.incoming_edges.len()) * (16 + 4 + 4 + 4 + NUM_EMBEDDERS * 4);
    let knn_bytes: usize = r.knn_neighbors.iter().map(|v| v.len() * (16 + 4)).sum();
    let content_bytes = r.content.len();
    (dense_bytes * 4 + sparse_bytes + token_bytes + edge_bytes + knn_bytes + content_bytes) as u64
}
