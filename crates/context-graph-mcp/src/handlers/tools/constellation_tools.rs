//! Constellation tool handlers (Phase 2).
//!
//! Handlers for the five constellation MCP tools:
//! - `call_compile_constellation` — resolve members from the selector,
//!   stream them into a [`ConstellationAccumulator`], persist.
//! - `call_list_constellations` — paginated UUID listing + optional full
//!   record shape via `multi_get_constellations`.
//! - `call_get_constellation` — point lookup with optional centroid arrays.
//! - `call_score_against_constellation` — run
//!   [`score_memory_against_constellation`] over a fetched fingerprint.
//! - `call_delete_constellation` — remove primary + secondary index.
//!
//! All five downcast `Handlers::teleological_store` to
//! [`RocksDbTeleologicalStore`]; other backends return an MCP tool error.
//! This mirrors the training-tools pattern (Phase 1).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use context_graph_core::constellation::{
    compile_constellation, score_memory_against_constellation, Constellation,
    ConstellationAccumulator, ConstellationError, ConstellationScoringResult,
    ConstellationSelector, EmbedderStats, VectorKind, DEFAULT_MAX_MEMBERS,
    MIN_CONSTELLATION_MEMBERS,
};
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::{json, Value as JsonValue};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Hard cap on memories scanned during selector resolution. Larger than the
/// accumulator's max_members so the caller can pre-filter server-side.
const SCAN_CAP: usize = 200_000;

impl Handlers {
    /// Handle `compile_constellation`.
    pub(crate) async fn call_compile_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling compile_constellation: {:?}", args);

        let label = match args.get("label").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return self.tool_error(id, "Missing or empty 'label' parameter"),
        };
        let selector = match parse_selector(&args) {
            Ok(s) => s,
            Err(e) => return self.tool_error(id, &e),
        };
        let max_members = args
            .get("maxMembers")
            .and_then(|v| v.as_u64())
            .map(|v| v.clamp(3, 100_000) as usize)
            .unwrap_or(DEFAULT_MAX_MEMBERS);
        let rebuild_if_exists = args
            .get("rebuildIfExists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Downcast to RocksDB store (CF_CONSTELLATIONS lives there).
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "compile_constellation requires RocksDbTeleologicalStore",
            );
        };

        // Existence check.
        if !rebuild_if_exists {
            match rocksdb_store
                .find_constellation_by_selector(&selector)
                .await
            {
                Ok(Some(existing_id)) => {
                    return self.tool_result(
                        id,
                        json!({
                            "constellation_id": existing_id.to_string(),
                            "already_exists": true,
                            "message": "Constellation already exists for this selector; pass rebuildIfExists=true to recompile."
                        }),
                    );
                }
                Ok(None) => {}
                Err(e) => {
                    return self
                        .tool_error(id, &format!("find_constellation_by_selector failed: {}", e));
                }
            }
        }

        // Resolve members. All paths scan the fingerprint set once (post-
        // filter per selector) to keep selector logic in a single place.
        let members = match self.resolve_members(&selector, max_members).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        if members.is_empty() {
            return self.tool_error(id, "Selector resolved zero members — nothing to compile.");
        }

        // Compile.
        let synergy = SynergyMatrix::with_base_synergies();
        let mut acc = ConstellationAccumulator::with_max_members(
            selector.clone(),
            label.clone(),
            max_members,
        );
        let topic_match_target = match &selector {
            ConstellationSelector::Topic { topic_id } => Some(topic_id.clone()),
            _ => None,
        };

        for fp in &members {
            // Fail fast on storage errors instead of silently using the fallback
            // profile. The earlier `.unwrap_or_default()` swallowed RocksDB read
            // failures (code-simplifier H-2); a failing point-get here indicates
            // something is wrong with the DB and we want the caller to see it.
            let stored_profile = match rocksdb_store.get_topic_profile(fp.id).await {
                Ok(v) => v,
                Err(e) => {
                    error!(id = %fp.id, error = %e, "get_topic_profile failed");
                    return self.tool_error(
                        id,
                        &format!("Failed to read CF_TOPIC_PROFILES for {}: {}", fp.id, e),
                    );
                }
            };
            if let Err(e) = acc.observe(fp.id, &fp.semantic, stored_profile, &synergy) {
                warn!(id = %fp.id, error = %e, "observe failed");
                return self.tool_error(
                    id,
                    &format!("Constellation accumulator rejected a member: {}", e),
                );
            }
            if let Some(t) = &topic_match_target {
                let matches = self.topic_membership_matches(fp.id, t).await;
                acc.observe_topic_match(matches);
            }
        }

        let constellation = match acc.finalize() {
            Ok(c) => c,
            Err(ConstellationError::TooFewMembers { count, min }) => {
                return self.tool_error(
                    id,
                    &format!(
                        "Too few members for a constellation (got {}, min {})",
                        count, min
                    ),
                );
            }
            Err(e) => {
                error!(error = %e, "finalize failed");
                return self.tool_error(id, &format!("finalize failed: {}", e));
            }
        };

        // Persist.
        if let Err(e) = rocksdb_store.store_constellation(&constellation).await {
            error!(id = %constellation.id, error = %e, "store_constellation failed");
            return self.tool_error(id, &format!("store_constellation failed: {}", e));
        }

        info!(
            id = %constellation.id,
            members = constellation.member_count,
            coherence = constellation.coherence,
            "compile_constellation complete"
        );

        self.tool_result(
            id,
            json!({
                "status": "compiled",
                "constellation_id": constellation.id.to_string(),
                "label": constellation.label,
                "selector_kind": selector_kind_str(&selector),
                "member_count": constellation.member_count,
                "coherence": constellation.coherence,
                "purity": constellation.purity,
                "per_embedder_coverage": constellation
                    .per_embedder
                    .iter()
                    .map(|s| s.coverage)
                    .collect::<Vec<_>>(),
            }),
        )
    }

    /// Handle `list_constellations`.
    pub(crate) async fn call_list_constellations(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .clamp(1, 1000) as usize;
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let include_centroids = args
            .get("includeCentroids")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "list_constellations requires RocksDbTeleologicalStore");
        };

        let ids = match rocksdb_store.list_constellation_ids().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("list_constellation_ids failed: {}", e));
            }
        };
        let total = ids.len();
        let page: Vec<Uuid> = ids.into_iter().skip(offset).take(limit).collect();

        let records = match rocksdb_store.multi_get_constellations(&page).await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("multi_get_constellations failed: {}", e));
            }
        };

        let items: Vec<JsonValue> = records
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                let uid = page[i];
                match r {
                    Some(c) => render_constellation(&c, include_centroids),
                    None => json!({
                        "constellation_id": uid.to_string(),
                        "missing": true
                    }),
                }
            })
            .collect();

        self.tool_result(
            id,
            json!({
                "total": total,
                "offset": offset,
                "limit": limit,
                "returned": items.len(),
                "constellations": items,
            }),
        )
    }

    /// Handle `get_constellation`.
    pub(crate) async fn call_get_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let raw = match args.get("constellationId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'constellationId'"),
        };
        let cid = match Uuid::parse_str(raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "constellationId must be a valid UUID"),
        };
        let include_centroids = args
            .get("includeCentroids")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "get_constellation requires RocksDbTeleologicalStore");
        };

        match rocksdb_store.get_constellation(cid).await {
            Ok(Some(c)) => self.tool_result(id, render_constellation(&c, include_centroids)),
            Ok(None) => self.tool_result(id, json!({"constellation_id": raw, "found": false})),
            Err(e) => self.tool_error(id, &format!("get_constellation failed: {}", e)),
        }
    }

    /// Handle `score_against_constellation`.
    pub(crate) async fn call_score_against_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let cid_raw = match args.get("constellationId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'constellationId'"),
        };
        let cid = match Uuid::parse_str(cid_raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "constellationId must be a valid UUID"),
        };
        let mid_raw = match args.get("memoryId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'memoryId'"),
        };
        let mid = match Uuid::parse_str(mid_raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "memoryId must be a valid UUID"),
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "score_against_constellation requires RocksDbTeleologicalStore",
            );
        };

        let constellation = match rocksdb_store.get_constellation(cid).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                return self.tool_error(id, &format!("Constellation {} not found", cid_raw));
            }
            Err(e) => {
                return self.tool_error(id, &format!("get_constellation failed: {}", e));
            }
        };

        // Fetch the memory's fingerprint via the trait (works on any backend).
        let fp = match self.teleological_store.retrieve(mid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                return self.tool_error(
                    id,
                    &format!("Memory {} not found in fingerprint store", mid_raw),
                );
            }
            Err(e) => {
                return self.tool_error(id, &format!("retrieve fingerprint failed: {}", e));
            }
        };

        let result = score_memory_against_constellation(&constellation, mid, &fp.semantic);
        self.tool_result(id, render_scoring(&result))
    }

    /// Handle `delete_constellation`.
    pub(crate) async fn call_delete_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let raw = match args.get("constellationId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'constellationId'"),
        };
        let cid = match Uuid::parse_str(raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "constellationId must be a valid UUID"),
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "delete_constellation requires RocksDbTeleologicalStore");
        };

        match rocksdb_store.delete_constellation(cid).await {
            Ok(deleted) => {
                self.tool_result(id, json!({"constellation_id": raw, "deleted": deleted}))
            }
            Err(e) => self.tool_error(id, &format!("delete_constellation failed: {}", e)),
        }
    }

    /// Handle `derive_constellation`.
    ///
    /// This condenses the conceptual constellation arithmetic surface into
    /// one persisted operation:
    /// - interpolate/add/difference operate on stored constellation centroids.
    /// - anti_pole mines the lowest-scoring real memories from a caller-selected
    ///   candidate pool, recompiles them, and persists the resulting opposite
    ///   anchor.
    pub(crate) async fn call_derive_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling derive_constellation: {:?}", args);

        let operation = match args.get("operation").and_then(|v| v.as_str()) {
            Some(op @ ("interpolate" | "add" | "difference" | "anti_pole")) => op,
            Some(other) => {
                return self.tool_error(
                    id,
                    &format!(
                        "operation must be interpolate, add, difference, or anti_pole; got {other}"
                    ),
                )
            }
            None => return self.tool_error(id, "Missing required 'operation' parameter"),
        };
        let label = match args.get("label").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return self.tool_error(id, "Missing or empty 'label' parameter"),
        };
        let source_id = match parse_uuid_arg(&args, "sourceConstellationId") {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "derive_constellation requires RocksDbTeleologicalStore");
        };

        let before_count = match rocksdb_store.count_constellations().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("count_constellations failed: {e}")),
        };
        let source = match rocksdb_store.get_constellation(source_id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                return self.tool_error(id, &format!("source constellation {source_id} not found"))
            }
            Err(e) => return self.tool_error(id, &format!("get source constellation failed: {e}")),
        };

        let (derived, derivation) = match operation {
            "interpolate" | "add" | "difference" => {
                let other_id = match parse_uuid_arg(&args, "otherConstellationId") {
                    Ok(v) => v,
                    Err(e) => return self.tool_error(id, &e),
                };
                let other = match rocksdb_store.get_constellation(other_id).await {
                    Ok(Some(v)) => v,
                    Ok(None) => {
                        return self
                            .tool_error(id, &format!("other constellation {other_id} not found"))
                    }
                    Err(e) => {
                        return self.tool_error(id, &format!("get other constellation failed: {e}"))
                    }
                };
                let alpha = args.get("alpha").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
                let derived =
                    match derive_linear_constellation(&source, &other, operation, alpha, label) {
                        Ok(v) => v,
                        Err(e) => return self.tool_error(id, &e),
                    };
                (
                    derived,
                    json!({
                        "operation": operation,
                        "source_constellation_id": source_id.to_string(),
                        "other_constellation_id": other_id.to_string(),
                        "alpha": alpha,
                        "method": "linear centroid arithmetic over persisted CF_CONSTELLATIONS rows"
                    }),
                )
            }
            "anti_pole" => {
                let selector = match parse_selector(&args) {
                    Ok(v) => v,
                    Err(e) => return self.tool_error(id, &e),
                };
                let max_candidates = args
                    .get("maxCandidates")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.clamp(MIN_CONSTELLATION_MEMBERS as u64, 200_000) as usize)
                    .unwrap_or(10_000);
                let selected_members = args
                    .get("selectedMembers")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.clamp(MIN_CONSTELLATION_MEMBERS as u64, 100_000) as usize)
                    .unwrap_or(50);

                let candidates = match self.resolve_members(&selector, max_candidates).await {
                    Ok(v) => v,
                    Err(e) => return self.tool_error(id, &e),
                };
                let source_members: HashSet<Uuid> = source.member_ids.iter().copied().collect();
                let mut scored = Vec::new();
                for fp in candidates {
                    if source_members.contains(&fp.id) {
                        continue;
                    }
                    let score = score_memory_against_constellation(&source, fp.id, &fp.semantic)
                        .combined_score;
                    if !score.is_finite() {
                        return self.tool_error(
                            id,
                            &format!("anti_pole candidate {} produced non-finite score", fp.id),
                        );
                    }
                    scored.push((score, fp));
                }
                scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let selected = scored
                    .into_iter()
                    .take(selected_members)
                    .collect::<Vec<_>>();
                if selected.len() < MIN_CONSTELLATION_MEMBERS {
                    return self.tool_error(
                        id,
                        &format!(
                            "anti_pole requires at least {MIN_CONSTELLATION_MEMBERS} non-source candidates; got {}",
                            selected.len()
                        ),
                    );
                }
                let mut member_rows = Vec::with_capacity(selected.len());
                let mut selected_scores = Vec::with_capacity(selected.len());
                for (score, fp) in selected {
                    let stored_profile = match rocksdb_store.get_topic_profile(fp.id).await {
                        Ok(v) => v,
                        Err(e) => {
                            error!(id = %fp.id, error = %e, "get_topic_profile failed during anti_pole");
                            return self.tool_error(
                                id,
                                &format!("Failed to read CF_TOPIC_PROFILES for {}: {}", fp.id, e),
                            );
                        }
                    };
                    selected_scores.push(json!({
                        "memory_id": fp.id.to_string(),
                        "source_combined_score": score
                    }));
                    member_rows.push((fp.id, fp.semantic, stored_profile));
                }
                let selector_tag = derived_selector_tag("anti_pole", source_id, None, None);
                let derived = match compile_constellation(
                    ConstellationSelector::Tag { tag: selector_tag },
                    label,
                    member_rows,
                    &SynergyMatrix::with_base_synergies(),
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("anti_pole compile failed: {e}"))
                    }
                };
                (
                    derived,
                    json!({
                        "operation": "anti_pole",
                        "source_constellation_id": source_id.to_string(),
                        "candidate_selector_kind": selector_kind_str(&selector),
                        "candidate_selector": selector_json(&selector),
                        "max_candidates": max_candidates,
                        "selected_members": selected_scores.len(),
                        "selected_lowest_scores": selected_scores,
                        "method": "lowest combined_score real memories from persisted fingerprint store, recompiled into CF_CONSTELLATIONS"
                    }),
                )
            }
            _ => unreachable!(),
        };

        if let Err(e) = rocksdb_store.store_constellation(&derived).await {
            error!(id = %derived.id, error = %e, "store derived constellation failed");
            return self.tool_error(id, &format!("store_constellation failed: {e}"));
        }
        let readback = match rocksdb_store.get_constellation(derived.id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                return self.tool_error(
                    id,
                    "derive_constellation store reported success but readback was missing",
                )
            }
            Err(e) => return self.tool_error(id, &format!("derived readback failed: {e}")),
        };
        let after_count = match rocksdb_store.count_constellations().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("post-store count failed: {e}")),
        };

        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_families": ["constellations", "constellation_by_selector"],
                    "format": "version_byte + bincode"
                },
                "before_count": before_count,
                "after_count": after_count,
                "derived_constellation_id": readback.id.to_string(),
                "label": readback.label,
                "member_count": readback.member_count,
                "coherence": readback.coherence,
                "selector": selector_json(&readback.selector),
                "per_embedder_coverage": readback
                    .per_embedder
                    .iter()
                    .map(|s| s.coverage)
                    .collect::<Vec<_>>(),
                "readback_verified": true,
                "derivation": derivation,
            }),
        )
    }

    /// Resolve the member set for a selector by scanning up to `SCAN_CAP`
    /// fingerprints and filtering per selector kind. Expensive selectors
    /// (Tag, Session, TimeRange) do per-memory `get_source_metadata` calls —
    /// avoid them on cold databases.
    async fn resolve_members(
        &self,
        selector: &ConstellationSelector,
        max_members: usize,
    ) -> Result<Vec<TeleologicalFingerprint>, String> {
        // ExplicitIds path: fetch each id directly; no scan.
        if let ConstellationSelector::ExplicitIds { ids, .. } = selector {
            let mut out = Vec::with_capacity(ids.len().min(max_members));
            for id in ids.iter().take(max_members) {
                match self.teleological_store.retrieve(*id).await {
                    Ok(Some(fp)) => out.push(fp),
                    Ok(None) => {
                        debug!(id = %id, "explicit_ids: fingerprint not found; skipping");
                    }
                    Err(e) => return Err(format!("retrieve({}) failed: {}", id, e)),
                }
            }
            return Ok(out);
        }

        // All other paths: unbiased scan, then filter.
        let scan = self
            .teleological_store
            .list_fingerprints_unbiased(SCAN_CAP)
            .await
            .map_err(|e| format!("list_fingerprints_unbiased failed: {}", e))?;

        let mut out = Vec::new();
        for fp in scan {
            if out.len() >= max_members {
                break;
            }
            let include = match selector {
                ConstellationSelector::Session { session_id } => {
                    match self.teleological_store.get_source_metadata(fp.id).await {
                        Ok(Some(m)) => m.session_id.as_deref() == Some(session_id.as_str()),
                        _ => false,
                    }
                }
                ConstellationSelector::Tag { tag } => {
                    // Tag matches SourceMetadata.tool_name — the closest thing
                    // to a user-controllable tag today. Documented in the
                    // tool's description.
                    match self.teleological_store.get_source_metadata(fp.id).await {
                        Ok(Some(m)) => m.tool_name.as_deref() == Some(tag.as_str()),
                        _ => false,
                    }
                }
                ConstellationSelector::TimeRange { start, end } => {
                    in_range(fp.created_at, *start, *end)
                }
                ConstellationSelector::Topic { topic_id: _ } => {
                    // Topic resolution: require that the memory has an entry
                    // in the loaded topic portfolio. We look this up via the
                    // store's portfolio persistence layer.
                    //
                    // For Phase 2 we delegate to the general unbiased scan and
                    // mark matches through observe_topic_match; any memory in
                    // the scan is *candidate* for a topic-based constellation.
                    // The purity field then measures how many were actually
                    // in the topic.
                    true
                }
                ConstellationSelector::ExplicitIds { .. } => unreachable!(),
            };
            if include {
                out.push(fp);
            }
        }
        Ok(out)
    }
}

// ==========================================================================
// Argument parsing + rendering helpers
// ==========================================================================

fn parse_selector(args: &JsonValue) -> Result<ConstellationSelector, String> {
    let kind = args
        .get("selector")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required 'selector' parameter".to_string())?;
    match kind {
        "topic" => {
            let topic_id = args
                .get("topicId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "selector='topic' requires 'topicId'".to_string())?;
            Ok(ConstellationSelector::Topic {
                topic_id: topic_id.to_string(),
            })
        }
        "session" => {
            let session_id = args
                .get("sessionId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "selector='session' requires 'sessionId'".to_string())?;
            Ok(ConstellationSelector::Session {
                session_id: session_id.to_string(),
            })
        }
        "tag" => {
            let tag = args
                .get("tag")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "selector='tag' requires 'tag'".to_string())?;
            Ok(ConstellationSelector::Tag {
                tag: tag.to_string(),
            })
        }
        "time_range" => {
            let start = args
                .get("startIso")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "selector='time_range' requires 'startIso'".to_string())?;
            let end = args
                .get("endIso")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "selector='time_range' requires 'endIso'".to_string())?;
            let start = DateTime::parse_from_rfc3339(start)
                .map_err(|e| format!("startIso parse failed: {}", e))?
                .with_timezone(&Utc);
            let end = DateTime::parse_from_rfc3339(end)
                .map_err(|e| format!("endIso parse failed: {}", e))?
                .with_timezone(&Utc);
            if end < start {
                return Err("selector='time_range' requires end >= start".into());
            }
            Ok(ConstellationSelector::TimeRange { start, end })
        }
        "explicit_ids" => {
            let ids_val = args
                .get("memoryIds")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "selector='explicit_ids' requires 'memoryIds' array".to_string())?;
            let mut ids = Vec::with_capacity(ids_val.len());
            for entry in ids_val {
                let s = entry
                    .as_str()
                    .ok_or_else(|| "memoryIds must contain UUID strings".to_string())?;
                let u =
                    Uuid::parse_str(s).map_err(|e| format!("invalid memoryId '{}': {}", s, e))?;
                ids.push(u);
            }
            if ids.is_empty() {
                return Err("selector='explicit_ids' requires at least one memoryId".into());
            }
            let rationale = args
                .get("rationale")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ConstellationSelector::ExplicitIds { rationale, ids })
        }
        other => Err(format!("Unknown selector '{}'", other)),
    }
}

fn selector_kind_str(s: &ConstellationSelector) -> &'static str {
    match s {
        ConstellationSelector::Topic { .. } => "topic",
        ConstellationSelector::Session { .. } => "session",
        ConstellationSelector::Tag { .. } => "tag",
        ConstellationSelector::TimeRange { .. } => "time_range",
        ConstellationSelector::ExplicitIds { .. } => "explicit_ids",
    }
}

fn in_range(ts: DateTime<Utc>, start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
    ts >= start && ts <= end
}

fn render_constellation(c: &Constellation, include_centroids: bool) -> JsonValue {
    let mut per = Vec::with_capacity(c.per_embedder.len());
    for s in &c.per_embedder {
        let mut obj = json!({
            "embedder_index": s.embedder_index,
            "dimension": s.dimension,
            "vector_kind": format!("{:?}", s.vector_kind),
            "mean_l2": s.mean_l2,
            "stddev_l2": s.stddev_l2,
            "cosine_spread_p50": s.cosine_spread_p50,
            "cosine_spread_p95": s.cosine_spread_p95,
            "min_cosine": s.min_cosine,
            "max_cosine": s.max_cosine,
            "coverage": s.coverage,
            "mean_token_count": s.mean_token_count,
        });
        if include_centroids {
            obj["centroid"] = json!(s.centroid);
            obj["sparse_top_terms"] = json!(s.sparse_top_terms);
            obj["pooled_token_centroid"] = json!(s.pooled_token_centroid);
        } else {
            obj["centroid_len"] = json!(s.centroid.len());
            obj["sparse_top_terms_count"] = json!(s.sparse_top_terms.len());
            obj["pooled_token_centroid_len"] = json!(s.pooled_token_centroid.len());
        }
        per.push(obj);
    }

    let mut out = json!({
        "found": true,
        "constellation_id": c.id.to_string(),
        "label": c.label,
        "created_at": c.created_at,
        "selector_kind": selector_kind_str(&c.selector),
        "selector": selector_json(&c.selector),
        "member_count": c.member_count,
        "coherence": c.coherence,
        "purity": c.purity,
        "per_embedder": per,
    });
    if include_centroids {
        out["topic_profile_centroid"] = json!(c.topic_profile_centroid.to_vec());
        out["group_alignment_centroid"] = json!(c.group_alignment_centroid.to_vec());
        out["cross_correlation_centroid"] = json!(c.cross_correlation_centroid);
        out["member_ids"] = json!(c.member_ids.iter().map(Uuid::to_string).collect::<Vec<_>>());
    } else {
        out["topic_profile_centroid_len"] = json!(c.topic_profile_centroid.len());
        out["group_alignment_centroid_len"] = json!(c.group_alignment_centroid.len());
        out["cross_correlation_centroid_len"] = json!(c.cross_correlation_centroid.len());
        out["member_ids_count"] = json!(c.member_ids.len());
    }
    out
}

fn selector_json(s: &ConstellationSelector) -> JsonValue {
    match s {
        ConstellationSelector::Topic { topic_id } => json!({"topicId": topic_id}),
        ConstellationSelector::Session { session_id } => json!({"sessionId": session_id}),
        ConstellationSelector::Tag { tag } => json!({"tag": tag}),
        ConstellationSelector::TimeRange { start, end } => {
            json!({"startIso": start, "endIso": end})
        }
        ConstellationSelector::ExplicitIds { rationale, ids } => json!({
            "rationale": rationale,
            "memoryIds": ids.iter().map(Uuid::to_string).collect::<Vec<_>>(),
        }),
    }
}

fn render_scoring(r: &ConstellationScoringResult) -> JsonValue {
    json!({
        "constellation_id": r.constellation_id.to_string(),
        "memory_id": r.memory_id.to_string(),
        "per_embedder_cosine": r.per_embedder_cosine.to_vec(),
        "combined_score": r.combined_score,
        "in_spread_p95": r.in_spread_p95,
    })
}

fn parse_uuid_arg(args: &JsonValue, field: &str) -> Result<Uuid, String> {
    let raw = args
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing required '{field}' parameter"))?;
    Uuid::parse_str(raw).map_err(|_| format!("{field} must be a valid UUID"))
}

fn derive_linear_constellation(
    source: &Constellation,
    other: &Constellation,
    operation: &str,
    alpha: f32,
    label: String,
) -> Result<Constellation, String> {
    if source.per_embedder.len() != other.per_embedder.len() {
        return Err(format!(
            "constellation embedder count mismatch: source={}, other={}",
            source.per_embedder.len(),
            other.per_embedder.len()
        ));
    }
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err("alpha must be finite and in [0, 1]".into());
    }

    let (wa, wb) = match operation {
        "interpolate" => (1.0 - alpha, alpha),
        "add" => (1.0, 1.0),
        "difference" => (1.0, -1.0),
        _ => {
            return Err(format!(
                "unsupported linear derivation operation: {operation}"
            ))
        }
    };

    let mut per_embedder = Vec::with_capacity(source.per_embedder.len());
    for (a, b) in source.per_embedder.iter().zip(other.per_embedder.iter()) {
        per_embedder.push(combine_embedder_stats(a, b, wa, wb)?);
    }

    let topic_profile_centroid = combine_array(
        source.topic_profile_centroid,
        other.topic_profile_centroid,
        wa,
        wb,
    )?;
    let group_alignment_centroid = combine_array(
        source.group_alignment_centroid,
        other.group_alignment_centroid,
        wa,
        wb,
    )?;
    let cross_correlation_centroid = combine_vec(
        &source.cross_correlation_centroid,
        &other.cross_correlation_centroid,
        wa,
        wb,
        "cross_correlation_centroid",
    )?;

    let mut member_ids = source.member_ids.clone();
    member_ids.extend(other.member_ids.iter().copied());
    member_ids.sort();
    member_ids.dedup();

    let selector_tag = derived_selector_tag(operation, source.id, Some(other.id), Some(alpha));
    Ok(Constellation {
        id: Uuid::new_v4(),
        label,
        created_at: Utc::now(),
        selector: ConstellationSelector::Tag { tag: selector_tag },
        member_count: member_ids.len(),
        member_ids,
        per_embedder,
        topic_profile_centroid,
        group_alignment_centroid,
        cross_correlation_centroid,
        coherence: combine_scalar(source.coherence, other.coherence, wa, wb).clamp(-1.0, 1.0),
        purity: match (source.purity, other.purity) {
            (Some(a), Some(b)) => Some(combine_scalar(a, b, wa, wb).clamp(0.0, 1.0)),
            _ => None,
        },
    })
}

fn combine_embedder_stats(
    a: &EmbedderStats,
    b: &EmbedderStats,
    wa: f32,
    wb: f32,
) -> Result<EmbedderStats, String> {
    if a.embedder_index != b.embedder_index {
        return Err(format!(
            "embedder index mismatch: {} vs {}",
            a.embedder_index, b.embedder_index
        ));
    }
    if a.vector_kind != b.vector_kind {
        return Err(format!(
            "embedder {} vector kind mismatch: {:?} vs {:?}",
            a.embedder_index, a.vector_kind, b.vector_kind
        ));
    }
    if a.dimension != b.dimension {
        return Err(format!(
            "embedder {} dimension mismatch: {} vs {}",
            a.embedder_index, a.dimension, b.dimension
        ));
    }

    let centroid = combine_vec(
        &a.centroid,
        &b.centroid,
        wa,
        wb,
        &format!("embedder {} centroid", a.embedder_index),
    )?;
    let pooled_token_centroid = combine_vec(
        &a.pooled_token_centroid,
        &b.pooled_token_centroid,
        wa,
        wb,
        &format!("embedder {} pooled_token_centroid", a.embedder_index),
    )?;
    let sparse_top_terms = combine_sparse_terms(&a.sparse_top_terms, &b.sparse_top_terms, wa, wb);

    if matches!(a.vector_kind, VectorKind::Dense | VectorKind::Asymmetric)
        && a.coverage > 0.0
        && b.coverage > 0.0
        && l2_norm(&centroid) <= 1e-12
    {
        return Err(format!(
            "embedder {} derived centroid is degenerate all-zero",
            a.embedder_index
        ));
    }
    if a.vector_kind == VectorKind::TokenLevel
        && a.coverage > 0.0
        && b.coverage > 0.0
        && l2_norm(&pooled_token_centroid) <= 1e-12
    {
        return Err(format!(
            "embedder {} derived pooled token centroid is degenerate all-zero",
            a.embedder_index
        ));
    }

    Ok(EmbedderStats {
        embedder_index: a.embedder_index,
        dimension: a.dimension,
        vector_kind: a.vector_kind,
        centroid,
        sparse_top_terms,
        mean_token_count: match (a.mean_token_count, b.mean_token_count) {
            (Some(x), Some(y)) => Some(combine_scalar(x, y, wa, wb).max(0.0)),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        },
        pooled_token_centroid,
        mean_l2: combine_scalar(a.mean_l2, b.mean_l2, wa.abs(), wb.abs()).max(0.0),
        stddev_l2: combine_scalar(a.stddev_l2, b.stddev_l2, wa.abs(), wb.abs()).max(0.0),
        // Derived spread cannot be recomputed without raw members. Use the
        // stricter inherited spread so membership tests fail closed.
        cosine_spread_p50: a.cosine_spread_p50.min(b.cosine_spread_p50),
        cosine_spread_p95: a.cosine_spread_p95.min(b.cosine_spread_p95),
        min_cosine: a.min_cosine.min(b.min_cosine),
        max_cosine: a.max_cosine.max(b.max_cosine),
        coverage: a.coverage.min(b.coverage),
    })
}

fn combine_array<const N: usize>(
    a: [f32; N],
    b: [f32; N],
    wa: f32,
    wb: f32,
) -> Result<[f32; N], String> {
    let mut out = [0.0f32; N];
    for i in 0..N {
        let value = combine_scalar(a[i], b[i], wa, wb);
        if !value.is_finite() {
            return Err(format!("derived array value at index {i} is non-finite"));
        }
        out[i] = value;
    }
    Ok(out)
}

fn combine_vec(a: &[f32], b: &[f32], wa: f32, wb: f32, field: &str) -> Result<Vec<f32>, String> {
    if a.is_empty() && b.is_empty() {
        return Ok(Vec::new());
    }
    if a.len() != b.len() {
        return Err(format!(
            "{field} dimension mismatch: source={}, other={}",
            a.len(),
            b.len()
        ));
    }
    let mut out = Vec::with_capacity(a.len());
    for (idx, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        let value = combine_scalar(*x, *y, wa, wb);
        if !value.is_finite() {
            return Err(format!("{field}[{idx}] derived value is non-finite"));
        }
        out.push(value);
    }
    Ok(out)
}

fn combine_sparse_terms(a: &[(u16, f32)], b: &[(u16, f32)], wa: f32, wb: f32) -> Vec<(u16, f32)> {
    let mut merged: HashMap<u16, f32> = HashMap::new();
    for (idx, value) in a {
        *merged.entry(*idx).or_insert(0.0) += wa * *value;
    }
    for (idx, value) in b {
        *merged.entry(*idx).or_insert(0.0) += wb * *value;
    }
    let mut out: Vec<(u16, f32)> = merged
        .into_iter()
        .filter(|(_, value)| value.is_finite() && value.abs() > 1e-9)
        .collect();
    out.sort_by(|a, b| {
        b.1.abs()
            .partial_cmp(&a.1.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(50);
    out
}

fn combine_scalar(a: f32, b: f32, wa: f32, wb: f32) -> f32 {
    wa * a + wb * b
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn derived_selector_tag(
    operation: &str,
    source_id: Uuid,
    other_id: Option<Uuid>,
    alpha: Option<f32>,
) -> String {
    match (other_id, alpha) {
        (Some(other), Some(alpha)) => format!(
            "derived_constellation:{operation}:{source_id}:{other}:{:.6}",
            alpha
        ),
        (Some(other), None) => format!("derived_constellation:{operation}:{source_id}:{other}"),
        (None, _) => format!("derived_constellation:{operation}:{source_id}"),
    }
}

impl Handlers {
    /// Best-effort lookup: is `memory_id` a member of `topic_id` per the
    /// latest persisted topic portfolio? Returns `false` when the portfolio
    /// is missing or the read fails — purity is a soft metric, never a hard
    /// error.
    ///
    /// `topic_id` is matched against `Topic.id.to_string()` (Uuid) or
    /// `Topic.name` (if the caller passed a human-readable label).
    async fn topic_membership_matches(&self, memory_id: Uuid, topic_id: &str) -> bool {
        match self.teleological_store.load_latest_topic_portfolio().await {
            Ok(Some(portfolio)) => portfolio
                .topics
                .iter()
                .find(|t| {
                    t.id.to_string() == topic_id
                        || t.name.as_deref().map(|n| n == topic_id).unwrap_or(false)
                })
                .map(|t| t.member_memories.contains(&memory_id))
                .unwrap_or(false),
            _ => false,
        }
    }
}
