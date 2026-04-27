//! Contrastive pair miner tool handlers (Phase 3).
//!
//! Four MCP handlers backing the Phase 3 contrastive pair miner:
//! - `call_mine_contrastive_pairs` — scan anchors, compute full 13-slot
//!   similarity profiles against a bounded candidate pool, classify into
//!   one of six `AnomalyKind`s, and persist each surviving pair atomically
//!   to the three contrastive CFs.
//! - `call_list_contrastive_pairs` — paginated `(anchorId, negativeId)`
//!   listing with optional kind / anchor filters, full-record hydration on
//!   demand.
//! - `call_get_contrastive_pair` — composite-key point lookup.
//! - `call_count_contrastive_pairs` — total or per-kind row count.
//!
//! ## Design note — "reuse search_cross_embedder_anomalies logic"
//!
//! The plan (`docs/TRAINING_DATA_EXPORT_PLAN.md` §5.5) calls for reusing the
//! cross-embedder anomaly search primitive. Rather than re-entering the
//! search pipeline and paying its full retrieval cost per anchor (and
//! coupling the miner to the search API), we reproduce the same *algorithmic*
//! step in-process:
//!
//! 1. Sample a bounded candidate pool (default `candidatePoolSize = 500`)
//!    from `list_fingerprints_unbiased` once per run.
//! 2. For each anchor, compute the full 13-slot similarity profile against
//!    every pool member via [`similarity_profile`].
//! 3. Keep the `topKCandidatesPerAnchor` candidates with the largest
//!    `max(high_sim) - min(low_sim)` (same scoring as
//!    `search_cross_embedder_anomalies`).
//! 4. Classify each survivor with [`classify_anomaly`] and persist.
//!
//! This keeps the miner deterministic over the candidate pool, avoids an
//! O(anchors × search_pipeline) cost, and produces real pairs on the
//! synthetic FSV data. The MCP tool's `candidatePoolSize` knob documents the
//! cost/quality tradeoff.
//!
//! All four handlers downcast `Handlers::teleological_store` to
//! `RocksDbTeleologicalStore`; other backends return a tool error.

use std::collections::HashMap;
use std::time::Instant;

use context_graph_core::contrastive::{
    mine_pair_from_candidate, AnomalyKind, ContrastivePair, MiningConfig, MiningSummary,
};
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde_json::{json, Value as JsonValue};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

/// Default candidate pool size per anchor (peers considered before top-K
/// filtering). Larger = more anomaly hits, smaller = faster.
const DEFAULT_CANDIDATE_POOL_SIZE: usize = 500;

/// Hard cap on the candidate pool.
const HARD_MAX_CANDIDATE_POOL: usize = 50_000;

/// Hard cap on `maxPairs` per run (mirrors the MCP schema).
const HARD_MAX_PAIRS: usize = 100_000;

/// Hard cap on `topKCandidatesPerAnchor`.
const HARD_MAX_TOP_K: usize = 500;

impl Handlers {
    /// Handle `mine_contrastive_pairs`.
    pub(crate) async fn call_mine_contrastive_pairs(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        debug!("Handling mine_contrastive_pairs: {:?}", args);
        let started = Instant::now();

        // ---- Parse args ----
        let max_pairs = args
            .get("maxPairs")
            .and_then(|v| v.as_u64())
            .map(|v| (v.min(HARD_MAX_PAIRS as u64)) as usize)
            .unwrap_or(1_000);
        let min_disagreement = args
            .get("minDisagreement")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(context_graph_core::contrastive::DEFAULT_MIN_DISAGREEMENT);
        let high_threshold = args
            .get("highThreshold")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(context_graph_core::contrastive::DEFAULT_HIGH_THRESHOLD);
        let low_threshold = args
            .get("lowThreshold")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(context_graph_core::contrastive::DEFAULT_LOW_THRESHOLD);
        let top_k_candidates_per_anchor = args
            .get("topKCandidatesPerAnchor")
            .and_then(|v| v.as_u64())
            .map(|v| (v.min(HARD_MAX_TOP_K as u64)) as usize)
            .unwrap_or(context_graph_core::contrastive::DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR);
        let candidate_pool_size = args
            .get("candidatePoolSize")
            .and_then(|v| v.as_u64())
            .map(|v| (v.min(HARD_MAX_CANDIDATE_POOL as u64)) as usize)
            .unwrap_or(DEFAULT_CANDIDATE_POOL_SIZE);
        let session_filter = args
            .get("sessionFilter")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let kinds_opt: Option<Vec<AnomalyKind>> = match args.get("kinds") {
            Some(JsonValue::Array(arr)) => {
                let mut out = Vec::with_capacity(arr.len());
                for entry in arr {
                    let s = match entry.as_str() {
                        Some(s) => s,
                        None => {
                            return self.tool_error(id, "kinds entries must be strings");
                        }
                    };
                    match AnomalyKind::parse(s) {
                        Some(k) => out.push(k),
                        None => {
                            return self.tool_error(id, &format!("Unknown kind '{}'", s));
                        }
                    }
                }
                if out.is_empty() {
                    None
                } else {
                    Some(out)
                }
            }
            Some(_) => {
                return self.tool_error(id, "kinds must be an array of strings");
            }
            None => None,
        };

        let cfg = MiningConfig {
            max_pairs,
            kinds: kinds_opt,
            min_disagreement,
            high_threshold,
            low_threshold,
            top_k_candidates_per_anchor,
            session_filter: session_filter.clone(),
        };
        if let Err(e) = cfg.validate() {
            return self.tool_error(id, &format!("Invalid mining config: {}", e));
        }

        // ---- Require RocksDB backend ----
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "mine_contrastive_pairs requires RocksDbTeleologicalStore. \
                 Current backend does not expose CF_CONTRASTIVE_PAIRS.",
            );
        };

        // ---- Load candidate pool (one unbiased scan shared across anchors) ----
        let pool: Vec<TeleologicalFingerprint> = match self
            .teleological_store
            .list_fingerprints_unbiased(candidate_pool_size)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "list_fingerprints_unbiased failed during mine");
                return self.tool_error(id, &format!("Failed to list fingerprints: {}", e));
            }
        };

        // Build an index of candidate UUIDs → position in pool for O(1) lookups.
        let pool_len = pool.len();
        if pool_len < 2 {
            let summary = MiningSummary {
                duration_ms: started.elapsed().as_millis() as u64,
                ..Default::default()
            };
            return self.tool_result(
                id,
                json!({
                    "status": "no-op",
                    "reason": "candidate pool has fewer than 2 fingerprints",
                    "summary": summary_to_json(&summary),
                }),
            );
        }

        // ---- Optionally load per-anchor session filter table ----
        // We fetch source metadata lazily: anchors may be session-filtered, and
        // every candidate scored against an anchor is independent of session.
        let mut anchors_scanned: usize = 0;
        let mut anchors_no_candidate: usize = 0;
        let mut pairs_stored: usize = 0;
        let mut pairs_skipped_below_threshold: usize = 0;
        let mut pairs_skipped_kind_filter: usize = 0;

        // Cache anchor texts so we don't re-read the same row as both anchor
        // and negative. O(pool_len) up-front read.
        let pool_ids: Vec<Uuid> = pool.iter().map(|fp| fp.id).collect();
        let contents = match self.teleological_store.get_content_batch(&pool_ids).await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "get_content_batch failed; falling back to empty strings");
                vec![None; pool_ids.len()]
            }
        };
        let content_by_id: HashMap<Uuid, String> = pool_ids
            .iter()
            .zip(contents.iter())
            .filter_map(|(id, c)| c.as_ref().map(|s| (*id, s.clone())))
            .collect();

        // Pre-fetch session metadata only when we're filtering anchors.
        let session_filter_set: Option<Vec<bool>> = if let Some(sid) = &session_filter {
            let metas = match self
                .teleological_store
                .get_source_metadata_batch(&pool_ids)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    warn!(error = %e, "get_source_metadata_batch failed; treating all as mismatch");
                    vec![None; pool_ids.len()]
                }
            };
            Some(
                metas
                    .iter()
                    .map(|m| {
                        m.as_ref()
                            .and_then(|mm| mm.session_id.as_deref())
                            .map(|s| s == sid.as_str())
                            .unwrap_or(false)
                    })
                    .collect(),
            )
        } else {
            None
        };

        // ---- Main mining loop ----
        'anchor_loop: for (anchor_idx, anchor_fp) in pool.iter().enumerate() {
            if pairs_stored >= cfg.max_pairs {
                break;
            }
            if let Some(mask) = &session_filter_set {
                if !mask[anchor_idx] {
                    continue;
                }
            }
            anchors_scanned += 1;

            // Score every other candidate, keep top-K by disagreement magnitude.
            let mut scored: Vec<(usize, f32, [f32; NUM_EMBEDDERS])> =
                Vec::with_capacity(pool_len.saturating_sub(1));
            for (neg_idx, neg_fp) in pool.iter().enumerate() {
                if neg_idx == anchor_idx {
                    continue;
                }
                let profile = context_graph_core::contrastive::similarity_profile(
                    &anchor_fp.semantic,
                    &neg_fp.semantic,
                );
                // Cheap pre-filter using the raw profile; we recompute thresholds
                // properly inside mine_pair_from_candidate.
                let (max_high, min_low) =
                    high_low_bounds(&profile, cfg.high_threshold, cfg.low_threshold);
                let disagreement = max_high - min_low;
                if !disagreement.is_finite() || disagreement <= 0.0 {
                    continue;
                }
                scored.push((neg_idx, disagreement, profile));
            }

            if scored.is_empty() {
                anchors_no_candidate += 1;
                continue;
            }
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            scored.truncate(cfg.top_k_candidates_per_anchor);

            for (neg_idx, _disagreement, _profile) in scored {
                if pairs_stored >= cfg.max_pairs {
                    break 'anchor_loop;
                }
                let neg_fp = &pool[neg_idx];
                let anchor_text = content_by_id
                    .get(&anchor_fp.id)
                    .cloned()
                    .unwrap_or_default();
                let negative_text = content_by_id.get(&neg_fp.id).cloned().unwrap_or_default();

                let Some(pair) = mine_pair_from_candidate(
                    anchor_fp.id,
                    &anchor_text,
                    &anchor_fp.semantic,
                    neg_fp.id,
                    &negative_text,
                    &neg_fp.semantic,
                    &cfg,
                ) else {
                    // Figure out *why* we skipped: disagreement vs kind filter.
                    let profile = context_graph_core::contrastive::similarity_profile(
                        &anchor_fp.semantic,
                        &neg_fp.semantic,
                    );
                    let (max_high, min_low) =
                        high_low_bounds(&profile, cfg.high_threshold, cfg.low_threshold);
                    if !(max_high - min_low).is_finite()
                        || (max_high - min_low) < cfg.min_disagreement
                    {
                        pairs_skipped_below_threshold += 1;
                    } else {
                        pairs_skipped_kind_filter += 1;
                    }
                    continue;
                };

                if let Err(e) = rocksdb_store.store_contrastive_pair(&pair).await {
                    error!(
                        anchor = %pair.anchor_id,
                        negative = %pair.negative_id,
                        error = %e,
                        "store_contrastive_pair failed; continuing"
                    );
                    continue;
                }
                pairs_stored += 1;
            }
        }

        let summary = MiningSummary {
            pairs_stored,
            pairs_skipped_below_threshold,
            pairs_skipped_kind_filter,
            anchors_scanned,
            anchors_no_candidate,
            duration_ms: started.elapsed().as_millis() as u64,
        };

        info!(
            pairs_stored = summary.pairs_stored,
            anchors_scanned = summary.anchors_scanned,
            duration_ms = summary.duration_ms,
            "mine_contrastive_pairs complete"
        );

        self.tool_result(
            id,
            json!({
                "status": "mined",
                "summary": summary_to_json(&summary),
                "config": {
                    "max_pairs": cfg.max_pairs,
                    "min_disagreement": cfg.min_disagreement,
                    "high_threshold": cfg.high_threshold,
                    "low_threshold": cfg.low_threshold,
                    "top_k_candidates_per_anchor": cfg.top_k_candidates_per_anchor,
                    "candidate_pool_size": candidate_pool_size,
                    "session_filter": session_filter,
                    "kinds_filter": cfg.kinds.as_ref().map(|ks| {
                        ks.iter().map(|k| k.as_str()).collect::<Vec<_>>()
                    }),
                },
            }),
        )
    }

    /// Handle `list_contrastive_pairs`.
    pub(crate) async fn call_list_contrastive_pairs(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100)
            .clamp(1, 10_000) as usize;
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let include_full = args
            .get("includeFull")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let kind_filter: Option<AnomalyKind> = match args.get("kind").and_then(|v| v.as_str()) {
            Some(s) => match AnomalyKind::parse(s) {
                Some(k) => Some(k),
                None => return self.tool_error(id, &format!("Unknown kind '{}'", s)),
            },
            None => None,
        };
        let anchor_filter: Option<Uuid> = match args.get("anchorId").and_then(|v| v.as_str()) {
            Some(s) => match Uuid::parse_str(s) {
                Ok(u) => Some(u),
                Err(_) => return self.tool_error(id, "anchorId must be a valid UUID"),
            },
            None => None,
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "list_contrastive_pairs requires RocksDbTeleologicalStore",
            );
        };

        // Resolve the key list based on the most specific filter available.
        let keys: Vec<(Uuid, Uuid)> = match (kind_filter, anchor_filter) {
            (Some(kind), Some(anchor)) => {
                // Need pairs by anchor AND kind — walk the anchor's negative
                // list, then filter by fetching each primary record's kind.
                let negatives = match rocksdb_store.pairs_for_anchor(anchor).await {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("pairs_for_anchor failed: {}", e));
                    }
                };
                let mut out = Vec::new();
                for neg in negatives {
                    match rocksdb_store.get_contrastive_pair(anchor, neg).await {
                        Ok(Some(p)) if p.anomaly_kind == kind => out.push((anchor, neg)),
                        Ok(_) => {}
                        Err(e) => {
                            warn!(anchor = %anchor, negative = %neg, error = %e, "get_contrastive_pair failed during list");
                        }
                    }
                }
                out
            }
            (Some(kind), None) => {
                // Prefix scan by kind. We over-fetch because list_pairs_by_kind
                // does not support offset — we apply offset/limit post-hoc.
                let over = offset.saturating_add(limit);
                match rocksdb_store.list_pairs_by_kind(kind, over).await {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("list_pairs_by_kind failed: {}", e));
                    }
                }
            }
            (None, Some(anchor)) => {
                let negatives = match rocksdb_store.pairs_for_anchor(anchor).await {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("pairs_for_anchor failed: {}", e));
                    }
                };
                negatives.into_iter().map(|n| (anchor, n)).collect()
            }
            (None, None) => match rocksdb_store.list_contrastive_pair_keys().await {
                Ok(v) => v,
                Err(e) => {
                    return self
                        .tool_error(id, &format!("list_contrastive_pair_keys failed: {}", e));
                }
            },
        };

        let total = keys.len();
        let slice: Vec<(Uuid, Uuid)> = keys.into_iter().skip(offset).take(limit).collect();
        let returned = slice.len();

        let mut out = Vec::with_capacity(returned);
        if include_full {
            for (anchor, negative) in slice {
                match rocksdb_store.get_contrastive_pair(anchor, negative).await {
                    Ok(Some(p)) => out.push(pair_to_json(&p)),
                    Ok(None) => out.push(json!({
                        "anchor_id": anchor.to_string(),
                        "negative_id": negative.to_string(),
                        "found": false,
                    })),
                    Err(e) => {
                        error!(anchor = %anchor, negative = %negative, error = %e, "get_contrastive_pair failed during includeFull");
                        out.push(json!({
                            "anchor_id": anchor.to_string(),
                            "negative_id": negative.to_string(),
                            "error": format!("{}", e),
                        }));
                    }
                }
            }
        } else {
            for (anchor, negative) in slice {
                out.push(json!({
                    "anchor_id": anchor.to_string(),
                    "negative_id": negative.to_string(),
                }));
            }
        }

        self.tool_result(
            id,
            json!({
                "total_matching": total,
                "returned": returned,
                "offset": offset,
                "limit": limit,
                "include_full": include_full,
                "kind_filter": kind_filter.map(|k| k.as_str()),
                "anchor_filter": anchor_filter.map(|u| u.to_string()),
                "pairs": out,
            }),
        )
    }

    /// Handle `get_contrastive_pair`.
    pub(crate) async fn call_get_contrastive_pair(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let anchor_raw = match args.get("anchorId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'anchorId'"),
        };
        let negative_raw = match args.get("negativeId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'negativeId'"),
        };
        let anchor = match Uuid::parse_str(anchor_raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "anchorId must be a valid UUID"),
        };
        let negative = match Uuid::parse_str(negative_raw) {
            Ok(u) => u,
            Err(_) => return self.tool_error(id, "negativeId must be a valid UUID"),
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "get_contrastive_pair requires RocksDbTeleologicalStore");
        };

        match rocksdb_store.get_contrastive_pair(anchor, negative).await {
            Ok(Some(p)) => self.tool_result(id, pair_to_json(&p)),
            Ok(None) => self.tool_result(
                id,
                json!({
                    "found": false,
                    "anchor_id": anchor_raw,
                    "negative_id": negative_raw,
                }),
            ),
            Err(e) => self.tool_error(id, &format!("get_contrastive_pair failed: {}", e)),
        }
    }

    /// Handle `count_contrastive_pairs`.
    pub(crate) async fn call_count_contrastive_pairs(
        &self,
        id: Option<JsonRpcId>,
        args: JsonValue,
    ) -> JsonRpcResponse {
        let kind_filter: Option<AnomalyKind> = match args.get("kind").and_then(|v| v.as_str()) {
            Some(s) => match AnomalyKind::parse(s) {
                Some(k) => Some(k),
                None => return self.tool_error(id, &format!("Unknown kind '{}'", s)),
            },
            None => None,
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "count_contrastive_pairs requires RocksDbTeleologicalStore",
            );
        };

        match kind_filter {
            Some(kind) => match rocksdb_store.count_contrastive_pairs_by_kind(kind).await {
                Ok(n) => self.tool_result(
                    id,
                    json!({
                        "count": n,
                        "kind": kind.as_str(),
                    }),
                ),
                Err(e) => self.tool_error(
                    id,
                    &format!("count_contrastive_pairs_by_kind failed: {}", e),
                ),
            },
            None => match rocksdb_store.count_contrastive_pairs().await {
                Ok(n) => self.tool_result(id, json!({"count": n})),
                Err(e) => self.tool_error(id, &format!("count_contrastive_pairs failed: {}", e)),
            },
        }
    }
}

// ==========================================================================
// Response rendering
// ==========================================================================

fn pair_to_json(p: &ContrastivePair) -> JsonValue {
    json!({
        "found": true,
        "anchor_id": p.anchor_id.to_string(),
        "negative_id": p.negative_id.to_string(),
        "anchor_text": p.anchor_text,
        "negative_text": p.negative_text,
        "similarity_profile": p.similarity_profile.to_vec(),
        "high_embedders": p.high_embedders,
        "low_embedders": p.low_embedders,
        "disagreement_magnitude": p.disagreement_magnitude,
        "anomaly_kind": p.anomaly_kind.as_str(),
        "anomaly_kind_byte": p.anomaly_kind.as_u8(),
        "mined_at": p.mined_at,
        "generator": p.generator,
    })
}

fn summary_to_json(s: &MiningSummary) -> JsonValue {
    json!({
        "pairs_stored": s.pairs_stored,
        "pairs_skipped_below_threshold": s.pairs_skipped_below_threshold,
        "pairs_skipped_kind_filter": s.pairs_skipped_kind_filter,
        "anchors_scanned": s.anchors_scanned,
        "anchors_no_candidate": s.anchors_no_candidate,
        "duration_ms": s.duration_ms,
    })
}

/// Cheap helper: compute `max(profile[i] for i in high)` and `min(profile[i]
/// for i in low)` given thresholds. Used to sort candidates before the full
/// classification step.
fn high_low_bounds(profile: &[f32; NUM_EMBEDDERS], high: f32, low: f32) -> (f32, f32) {
    let mut max_high = f32::NEG_INFINITY;
    let mut min_low = f32::INFINITY;
    for &v in profile.iter() {
        if v > high && v > max_high {
            max_high = v;
        }
        if v < low && v < min_low {
            min_low = v;
        }
    }
    (max_high, min_low)
}
