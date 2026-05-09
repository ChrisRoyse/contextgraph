//! Typed-edge training-data tool handlers (Phase 4 — F1/F2/F4).
//!
//! Four MCP handlers backing the typed-edges training-data factory:
//! - [`Handlers::call_export_typed_edges_corpus`] — walk every row in
//!   `CF_TYPED_EDGES`, join optional content / source_metadata /
//!   mechanism_type / LLM-validation payloads, and persist one
//!   `TypedEdgeTrainingRecord` per edge into `CF_TYPED_EDGE_RECORDS`.
//! - [`Handlers::call_derive_anomalies_from_edges`] — classify typed edges
//!   against the five expressible `AnomalyKind` patterns and persist matching
//!   pairs into `CF_CONTRASTIVE_PAIRS`.
//! - [`Handlers::call_list_typed_edge_records`] — paginated read view of
//!   `CF_TYPED_EDGE_RECORDS`.
//!
//! All handlers downcast `Handlers::teleological_store` to
//! [`RocksDbTeleologicalStore`]; other backends return a tool error.

use std::time::Instant;

use chrono::Utc;
use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_core::typed_edge_export::{LLMValidationSummary, TypedEdgeTrainingRecord};
use context_graph_storage::teleological::rocksdb_store::AnomalyDerivationConfig;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::{json, Value as JsonValue};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

// ---------------------------------------------------------------------------
// Hard caps (mirror the schema's maxima to keep dispatch bounded).
// ---------------------------------------------------------------------------

const DEFAULT_EXPORT_MAX_EDGES: usize = 10_000;
const HARD_MAX_EXPORT_EDGES: usize = 1_000_000;
const HARD_MAX_DERIVE_PAIRS: usize = 1_000_000;
const DEFAULT_LIST_LIMIT: usize = 100;
const HARD_MAX_LIST_LIMIT: usize = 1_000;

const EXPORTER_VERSION: &str = "typed_edge_export_v1";

impl Handlers {
    // =======================================================================
    // F1 — export_typed_edges_corpus
    // =======================================================================

    /// Handle `export_typed_edges_corpus`. Walks every typed edge, assembles
    /// a `TypedEdgeTrainingRecord` with optional joins, and persists it.
    pub(crate) async fn call_export_typed_edges_corpus(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling export_typed_edges_corpus: {:?}", args);
        let started = Instant::now();

        // ---- Parse args ----
        let max_edges = parse_usize(
            &args,
            "maxEdges",
            DEFAULT_EXPORT_MAX_EDGES,
            HARD_MAX_EXPORT_EDGES,
        );
        let include_content = parse_bool(&args, "includeContent", true);
        let include_source_metadata = parse_bool(&args, "includeSourceMetadata", true);
        let include_mechanism_type = parse_bool(&args, "includeMechanismType", true);
        let join_llm_validation = parse_bool(&args, "joinLLMValidation", true);
        let clear_existing = parse_bool(&args, "clearExisting", false);

        // ---- Require RocksDB backend ----
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "export_typed_edges_corpus requires RocksDbTeleologicalStore. \
                 Current backend does not expose CF_TYPED_EDGE_RECORDS.",
            );
        };

        // ---- Require an EdgeRepository ----
        let Some(edge_repo) = self.edge_repository.as_ref() else {
            return self.tool_error(
                id,
                "export_typed_edges_corpus requires an EdgeRepository. \
                 Graph linking is not enabled on this handler.",
            );
        };

        // ---- Optionally clear CF_TYPED_EDGE_RECORDS ----
        let cleared = if clear_existing {
            match rocksdb_store.clear_all_typed_edge_records().await {
                Ok(n) => n,
                Err(e) => {
                    error!(error = %e, "clear_all_typed_edge_records failed");
                    return self
                        .tool_error(id, &format!("Failed to clear CF_TYPED_EDGE_RECORDS: {}", e));
                }
            }
        } else {
            0
        };

        // ---- Walk every typed edge ----
        let edges = match edge_repo.iter_all_typed_edges() {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "iter_all_typed_edges failed");
                return self.tool_error(id, &format!("Failed to iterate typed edges: {}", e));
            }
        };

        let total_edges = edges.len();
        let mut exported = 0usize;
        let mut errors = 0usize;
        let mut skipped_after_max = 0usize;

        let export_now = Utc::now();

        for edge in edges.iter() {
            if exported >= max_edges {
                skipped_after_max = total_edges.saturating_sub(exported);
                break;
            }

            // --- Content join ---
            let (source_content, target_content) = if include_content {
                let src = match self.teleological_store.get_content(edge.source()).await {
                    Ok(c) => c.unwrap_or_default(),
                    Err(e) => {
                        error!(src = %edge.source(), error = %e, "get_content(source) failed");
                        errors += 1;
                        continue;
                    }
                };
                let tgt = match self.teleological_store.get_content(edge.target()).await {
                    Ok(c) => c.unwrap_or_default(),
                    Err(e) => {
                        error!(tgt = %edge.target(), error = %e, "get_content(target) failed");
                        errors += 1;
                        continue;
                    }
                };
                (src, tgt)
            } else {
                (String::new(), String::new())
            };

            // --- Source metadata join ---
            let (source_session_id, source_type, target_session_id, target_type) =
                if include_source_metadata {
                    let src_meta = match self
                        .teleological_store
                        .get_source_metadata(edge.source())
                        .await
                    {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(src = %edge.source(), error = %e, "get_source_metadata(source) failed");
                            None
                        }
                    };
                    let tgt_meta = match self
                        .teleological_store
                        .get_source_metadata(edge.target())
                        .await
                    {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(tgt = %edge.target(), error = %e, "get_source_metadata(target) failed");
                            None
                        }
                    };
                    (
                        src_meta.as_ref().and_then(|m| m.session_id.clone()),
                        src_meta.as_ref().map(|m| format!("{:?}", m.source_type)),
                        tgt_meta.as_ref().and_then(|m| m.session_id.clone()),
                        tgt_meta.as_ref().map(|m| format!("{:?}", m.source_type)),
                    )
                } else {
                    (None, None, None, None)
                };

            // --- Mechanism-type join (CausalChain only) ---
            let mechanism_type = if include_mechanism_type
                && edge.edge_type() == GraphLinkEdgeType::CausalChain
            {
                match rocksdb_store
                    .get_causal_relationships_by_source(edge.source())
                    .await
                {
                    Ok(rels) => {
                        // Take the first non-empty mechanism_type from the
                        // source's causal relationships. CausalRelationship
                        // does not carry a separate target fingerprint id, so
                        // we cannot disambiguate by target — the first
                        // resolvable mechanism_type is the best we can do.
                        rels.into_iter().find_map(|r| {
                            if r.mechanism_type.trim().is_empty() {
                                None
                            } else {
                                Some(r.mechanism_type)
                            }
                        })
                    }
                    Err(e) => {
                        warn!(src = %edge.source(), error = %e, "get_causal_relationships_by_source failed");
                        None
                    }
                }
            } else {
                None
            };

            // --- LLM validation join ---
            let et_u8 = edge.edge_type().as_u8();
            let llm_validation = if join_llm_validation {
                match rocksdb_store
                    .get_llm_edge_validation(edge.source(), edge.target(), et_u8)
                    .await
                {
                    Ok(Some(v)) => Some(LLMValidationSummary {
                        validated_at: v.validated_at,
                        verdict: v.verdict,
                        confidence: v.confidence,
                        rationale: v.rationale,
                        validator_version: v.validator_version,
                    }),
                    Ok(None) => None,
                    Err(e) => {
                        warn!(
                            src = %edge.source(),
                            tgt = %edge.target(),
                            et = et_u8,
                            error = %e,
                            "get_llm_edge_validation failed"
                        );
                        None
                    }
                }
            } else {
                None
            };

            let record = TypedEdgeTrainingRecord {
                source_memory_id: edge.source(),
                target_memory_id: edge.target(),
                edge_type: et_u8,
                edge_type_name: edge.edge_type().to_string(),
                weight: edge.weight(),
                direction: edge.direction().as_u8(),
                embedder_scores: *edge.embedder_scores(),
                agreement_count: edge.agreement_count(),
                agreeing_embedders: edge.agreeing_embedders(),
                source_content,
                target_content,
                source_session_id,
                target_session_id,
                source_type,
                target_type,
                mechanism_type,
                llm_validation,
                exported_at: export_now,
                exporter_version: EXPORTER_VERSION.to_string(),
            };

            if let Err(e) = rocksdb_store.store_typed_edge_record(&record).await {
                error!(
                    src = %record.source_memory_id,
                    tgt = %record.target_memory_id,
                    et = record.edge_type,
                    error = %e,
                    "store_typed_edge_record failed"
                );
                errors += 1;
                continue;
            }
            exported += 1;
        }

        let duration_ms = started.elapsed().as_millis() as u64;
        info!(
            exported,
            errors,
            cleared,
            total_edges,
            skipped_after_max,
            duration_ms,
            "export_typed_edges_corpus complete"
        );

        self.tool_result(
            id,
            json!({
                "status": "exported",
                "exported": exported,
                "errors": errors,
                "cleared_before_export": cleared,
                "edges_scanned": total_edges,
                "skipped_after_max": skipped_after_max,
                "duration_ms": duration_ms,
                "storage": {
                    "backend": "rocksdb",
                    "column_family": "typed_edge_records",
                    "format": "version_byte + bincode",
                },
                "record_shape": {
                    "content_included": include_content,
                    "source_metadata_included": include_source_metadata,
                    "mechanism_type_included": include_mechanism_type,
                    "llm_validation_included": join_llm_validation,
                },
            }),
        )
    }

    // =======================================================================
    // F2 — derive_anomalies_from_edges
    // =======================================================================

    /// Handle `derive_anomalies_from_edges`. Classifies typed edges against
    /// the five expressible anomaly patterns and persists matching pairs.
    pub(crate) async fn call_derive_anomalies_from_edges(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling derive_anomalies_from_edges: {:?}", args);

        // ---- Parse args ----
        let default_cfg = AnomalyDerivationConfig::default();
        let high_threshold = parse_f32(&args, "highThreshold", default_cfg.high_threshold);
        let low_threshold = parse_f32(&args, "lowThreshold", default_cfg.low_threshold);
        let min_disagreement = parse_f32(&args, "minDisagreement", default_cfg.min_disagreement);
        let max_pairs = parse_usize(
            &args,
            "maxPairs",
            default_cfg.max_pairs,
            HARD_MAX_DERIVE_PAIRS,
        );

        // ---- Reject `kinds` filter (blocker: not supported by storage layer) ----
        if args.get("kinds").is_some() {
            return JsonRpcResponse::error(
                id,
                crate::protocol::error_codes::INVALID_PARAMS,
                "kinds filter not yet supported in storage layer",
            );
        }

        // ---- Require RocksDB backend ----
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "derive_anomalies_from_edges requires RocksDbTeleologicalStore. \
                 Current backend does not expose CF_CONTRASTIVE_PAIRS.",
            );
        };

        let Some(edge_repo) = self.edge_repository.as_ref() else {
            return self.tool_error(
                id,
                "derive_anomalies_from_edges requires an EdgeRepository. \
                 Graph linking is not enabled on this handler.",
            );
        };

        let cfg = AnomalyDerivationConfig {
            high_threshold,
            low_threshold,
            max_pairs,
            min_disagreement,
        };

        let summary = match rocksdb_store
            .derive_anomalies_from_edges(edge_repo, &cfg)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "derive_anomalies_from_edges failed");
                return self.tool_error(id, &format!("derive_anomalies_from_edges failed: {}", e));
            }
        };

        let per_kind_counts: serde_json::Map<String, JsonValue> = summary
            .per_kind_counts
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), json!(v)))
            .collect();

        info!(
            scanned = summary.edges_scanned,
            written = summary.pairs_written,
            skipped_below_threshold = summary.skipped_below_threshold,
            skipped_missing_content = summary.skipped_missing_content,
            duration_ms = summary.duration_ms,
            "derive_anomalies_from_edges complete"
        );

        self.tool_result(
            id,
            json!({
                "status": "derived",
                "summary": {
                    "edges_scanned": summary.edges_scanned,
                    "pairs_written": summary.pairs_written,
                    "skipped_below_threshold": summary.skipped_below_threshold,
                    "skipped_missing_content": summary.skipped_missing_content,
                    "per_kind_counts": per_kind_counts,
                    "duration_ms": summary.duration_ms,
                },
                "config": {
                    "high_threshold": cfg.high_threshold,
                    "low_threshold": cfg.low_threshold,
                    "min_disagreement": cfg.min_disagreement,
                    "max_pairs": cfg.max_pairs,
                },
            }),
        )
    }

    // =======================================================================
    // list_typed_edge_records
    // =======================================================================

    /// Handle `list_typed_edge_records`. Paginated read view; set
    /// `includeFull=true` to hydrate the full record per row.
    pub(crate) async fn call_list_typed_edge_records(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling list_typed_edge_records: {:?}", args);

        let limit = parse_usize(&args, "limit", DEFAULT_LIST_LIMIT, HARD_MAX_LIST_LIMIT);
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let include_full = parse_bool(&args, "includeFull", false);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "list_typed_edge_records requires RocksDbTeleologicalStore. \
                 Current backend does not expose CF_TYPED_EDGE_RECORDS.",
            );
        };

        let keys = match rocksdb_store.list_typed_edge_record_keys().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("list_typed_edge_record_keys failed: {}", e));
            }
        };

        let total = keys.len();
        let slice: Vec<(Uuid, Uuid, u8)> = keys.into_iter().skip(offset).take(limit).collect();
        let returned = slice.len();

        let mut rows = Vec::with_capacity(returned);
        if include_full {
            for (src, tgt, et) in slice {
                match rocksdb_store.get_typed_edge_record(src, tgt, et).await {
                    Ok(Some(rec)) => rows.push(record_to_json(&rec)),
                    Ok(None) => rows.push(json!({
                        "source_memory_id": src.to_string(),
                        "target_memory_id": tgt.to_string(),
                        "edge_type": et,
                        "edge_type_name": edge_type_name(et),
                        "absent": true,
                    })),
                    Err(e) => {
                        return self.tool_error(
                            id,
                            &format!(
                                "get_typed_edge_record({}, {}, {}) failed: {}",
                                src, tgt, et, e
                            ),
                        );
                    }
                }
            }
        } else {
            for (src, tgt, et) in slice {
                rows.push(json!({
                    "source_memory_id": src.to_string(),
                    "target_memory_id": tgt.to_string(),
                    "edge_type": et,
                    "edge_type_name": edge_type_name(et),
                }));
            }
        }

        self.tool_result(
            id,
            json!({
                "total": total,
                "offset": offset,
                "limit": limit,
                "returned": returned,
                "include_full": include_full,
                "records": rows,
            }),
        )
    }
}

// ---------------------------------------------------------------------------
// Argument parsing helpers (MCP schema-level bounds are a guideline; server
// still enforces hard maxima here).
// ---------------------------------------------------------------------------

fn parse_usize(args: &JsonValue, key: &str, default: usize, hard_max: usize) -> usize {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|v| (v.min(hard_max as u64)) as usize)
        .unwrap_or(default)
}

fn parse_bool(args: &JsonValue, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn parse_f32(args: &JsonValue, key: &str, default: f32) -> f32 {
    args.get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// JSON rendering
// ---------------------------------------------------------------------------

fn edge_type_name(et: u8) -> String {
    GraphLinkEdgeType::from_u8(et)
        .map(|e| e.to_string())
        .unwrap_or_else(|| format!("unknown_edge_type_{}", et))
}

fn record_to_json(rec: &TypedEdgeTrainingRecord) -> JsonValue {
    json!({
        "source_memory_id": rec.source_memory_id.to_string(),
        "target_memory_id": rec.target_memory_id.to_string(),
        "edge_type": rec.edge_type,
        "edge_type_name": rec.edge_type_name,
        "weight": rec.weight,
        "direction": rec.direction,
        "embedder_scores": rec.embedder_scores.to_vec(),
        "agreement_count": rec.agreement_count,
        "agreeing_embedders": rec.agreeing_embedders,
        "source_content": rec.source_content,
        "target_content": rec.target_content,
        "source_session_id": rec.source_session_id,
        "target_session_id": rec.target_session_id,
        "source_type": rec.source_type,
        "target_type": rec.target_type,
        "mechanism_type": rec.mechanism_type,
        "llm_validation": rec.llm_validation.as_ref().map(|v| json!({
            "validated_at": v.validated_at.to_rfc3339(),
            "verdict": v.verdict.as_str(),
            "confidence": v.confidence,
            "rationale": v.rationale,
            "validator_version": v.validator_version,
        })),
        "exported_at": rec.exported_at.to_rfc3339(),
        "exporter_version": rec.exporter_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_type_name_covers_known_and_unknown() {
        assert_eq!(edge_type_name(3), "causal_chain");
        assert!(edge_type_name(255).starts_with("unknown_edge_type_"));
    }

    #[test]
    fn parse_helpers_respect_defaults_and_caps() {
        let v = json!({});
        assert_eq!(parse_usize(&v, "x", 10, 100), 10);
        assert!(!parse_bool(&v, "x", false));
        assert!((parse_f32(&v, "x", 0.5) - 0.5).abs() < 1e-6);

        let v = json!({ "n": 1_000_000_000u64 });
        assert_eq!(parse_usize(&v, "n", 10, 100), 100);
    }
}
