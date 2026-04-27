//! Pure-functional contrastive pair mining primitives.
//!
//! This module contains no I/O. Everything operates on
//! [`SemanticFingerprint`] references plus a [`MiningConfig`]; the surrounding
//! MCP handler is responsible for enumerating anchors/candidates and
//! persisting results.
//!
//! ## Similarity contract
//!
//! All entries of the 13-slot similarity profile live in `[0, 1]` using the
//! codebase's SRC-3 convention. Dense / asymmetric embedders map raw cosine
//! `c ∈ [-1, 1]` via `(c + 1) / 2`. Sparse embedders (E6, E13) use sparse
//! Jaccard, which is already in `[0, 1]`. Token-level (E12) mean-pools both
//! sides' token vectors and runs the SRC-3 mapping; when either side has no
//! tokens or the pooled vector is zero-norm the entry is `0.0`.
//!
//! For asymmetric embedders the cause / source / paraphrase side is used
//! (matches the constellation compiler in `crate::constellation`).

use chrono::Utc;
use uuid::Uuid;

use crate::contrastive::types::{
    AnomalyKind, ContrastivePair, MiningConfig, DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD,
};
use crate::similarity::jaccard_similarity;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::SemanticFingerprint;

/// Generator tag written to [`ContrastivePair::generator`].
pub const GENERATOR_TAG: &str = "cross_embedder_anomaly_v1";

// ---------------------------------------------------------------------------
// Similarity profile
// ---------------------------------------------------------------------------

/// Compute the full 13-slot per-embedder similarity profile between two
/// fingerprints.
///
/// See module docs for the per-embedder contract. Missing / zero-norm vectors
/// produce `0.0`; they do **not** panic.
pub fn similarity_profile(
    anchor: &SemanticFingerprint,
    negative: &SemanticFingerprint,
) -> [f32; NUM_EMBEDDERS] {
    let mut out = [0.0f32; NUM_EMBEDDERS];

    // E1 (dense, semantic) — symmetric.
    out[0] = src3_cosine(&anchor.e1_semantic, &negative.e1_semantic);
    // E2 / E3 / E4 (dense, temporal) — symmetric.
    out[1] = src3_cosine(&anchor.e2_temporal_recent, &negative.e2_temporal_recent);
    out[2] = src3_cosine(&anchor.e3_temporal_periodic, &negative.e3_temporal_periodic);
    out[3] = src3_cosine(
        &anchor.e4_temporal_positional,
        &negative.e4_temporal_positional,
    );
    // E5 (asymmetric) — use cause side on both ends to match constellation / topic profile.
    out[4] = src3_cosine(&anchor.e5_causal_as_cause, &negative.e5_causal_as_cause);
    // E6 (sparse) — Jaccard already in [0, 1].
    out[5] = jaccard_similarity(&anchor.e6_sparse, &negative.e6_sparse);
    // E7 (dense, code).
    out[6] = src3_cosine(&anchor.e7_code, &negative.e7_code);
    // E8 (asymmetric) — source side.
    out[7] = src3_cosine(&anchor.e8_graph_as_source, &negative.e8_graph_as_source);
    // E9 (dense, HDC).
    out[8] = src3_cosine(&anchor.e9_hdc, &negative.e9_hdc);
    // E10 (asymmetric) — paraphrase side.
    out[9] = src3_cosine(
        &anchor.e10_multimodal_paraphrase,
        &negative.e10_multimodal_paraphrase,
    );
    // E11 (dense, entity).
    out[10] = src3_cosine(&anchor.e11_entity, &negative.e11_entity);
    // E12 (token-level) — mean-pool both sides, then SRC-3 cosine.
    out[11] = token_level_src3(&anchor.e12_late_interaction, &negative.e12_late_interaction);
    // E13 (sparse) — Jaccard.
    out[12] = jaccard_similarity(&anchor.e13_splade, &negative.e13_splade);
    // E14 (dense, BGE-M3) — symmetric SRC-3 cosine. When either side is empty
    // (legacy fingerprint or provider not wired), return 0.0 which the
    // downstream classifier treats as "inactive" rather than a real signal.
    out[13] = if anchor.e14_bge_m3_dense.is_empty() || negative.e14_bge_m3_dense.is_empty() {
        0.0
    } else {
        src3_cosine(&anchor.e14_bge_m3_dense, &negative.e14_bge_m3_dense)
    };

    // Clamp once at the end to recover from tiny FP drift.
    for x in out.iter_mut() {
        if !x.is_finite() {
            *x = 0.0;
        } else {
            *x = x.clamp(0.0, 1.0);
        }
    }
    out
}

/// Classify a similarity profile into one of the six canonical anomaly kinds.
///
/// Priority ordering is stable: the first rule whose antecedent holds wins.
/// The named rules are:
/// 1. High `E7` + low `E1` → `CodeShapeButDifferentIntent`
///    (checked first to avoid accidental `SemanticButNotCausal` for code).
/// 2. High `E11` + low `E8` → `EntitySharedButDifferentStructure`.
/// 3. High `E9` + low `E1` → `HdcRobustButSemanticDifferent`.
/// 4. High `E6` OR `E13` + low `E10` → `KeywordButNotParaphrase`.
/// 5. High `E1` + low `E5` → `SemanticButNotCausal`.
/// 6. Otherwise → `Other`.
pub fn classify_anomaly(
    profile: &[f32; NUM_EMBEDDERS],
    high_threshold: f32,
    low_threshold: f32,
) -> AnomalyKind {
    let high = |i: usize| profile[i] > high_threshold;
    let low = |i: usize| profile[i] < low_threshold;

    // 1. Code shape wins over plain semantic-similar-but-not-causal because
    //    code often carries a non-trivial E1 signal too.
    if high(6) && low(0) {
        return AnomalyKind::CodeShapeButDifferentIntent;
    }
    // 2. Entity shared but graph-context differs.
    if high(10) && low(7) {
        return AnomalyKind::EntitySharedButDifferentStructure;
    }
    // 3. HDC robust (typo-tolerant) tie, but E1 semantic disagrees.
    if high(8) && low(0) {
        return AnomalyKind::HdcRobustButSemanticDifferent;
    }
    // 4. Lexical overlap but paraphrase diverges.
    if (high(5) || high(12)) && low(9) {
        return AnomalyKind::KeywordButNotParaphrase;
    }
    // 5. Semantic-but-not-causal is last because E1 matches are common.
    if high(0) && low(4) {
        return AnomalyKind::SemanticButNotCausal;
    }
    AnomalyKind::Other
}

/// Build a [`ContrastivePair`] from a candidate `(anchor, negative)` pair.
///
/// Returns `None` when:
/// - The pair is below `cfg.min_disagreement`, **or**
/// - The pair has no "high" embedder above `cfg.high_threshold` **and no**
///   "low" embedder below `cfg.low_threshold` (there is no anomaly signal to
///   train on), **or**
/// - `cfg.kinds` is `Some(list)` and the classified kind is not in `list`.
///
/// The caller owns deduplication (composite `(anchor, negative)` key).
pub fn mine_pair_from_candidate(
    anchor_id: Uuid,
    anchor_text: &str,
    anchor_fp: &SemanticFingerprint,
    negative_id: Uuid,
    negative_text: &str,
    negative_fp: &SemanticFingerprint,
    cfg: &MiningConfig,
) -> Option<ContrastivePair> {
    if anchor_id == negative_id {
        return None;
    }
    let profile = similarity_profile(anchor_fp, negative_fp);

    let mut high_embedders: Vec<u8> = Vec::new();
    let mut low_embedders: Vec<u8> = Vec::new();
    for (i, v) in profile.iter().enumerate() {
        if *v > cfg.high_threshold {
            high_embedders.push(i as u8);
        }
        if *v < cfg.low_threshold {
            low_embedders.push(i as u8);
        }
    }

    // No disagreement signal → skip entirely.
    if high_embedders.is_empty() || low_embedders.is_empty() {
        return None;
    }

    let max_high = high_embedders
        .iter()
        .map(|&i| profile[i as usize])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_low = low_embedders
        .iter()
        .map(|&i| profile[i as usize])
        .fold(f32::INFINITY, f32::min);
    let disagreement = max_high - min_low;

    if !disagreement.is_finite() || disagreement < cfg.min_disagreement {
        return None;
    }

    let kind = classify_anomaly(&profile, cfg.high_threshold, cfg.low_threshold);
    if let Some(kinds) = &cfg.kinds {
        if !kinds.contains(&kind) {
            return None;
        }
    }

    Some(ContrastivePair {
        anchor_id,
        negative_id,
        anchor_text: anchor_text.to_string(),
        negative_text: negative_text.to_string(),
        similarity_profile: profile,
        high_embedders,
        low_embedders,
        disagreement_magnitude: disagreement,
        anomaly_kind: kind,
        mined_at: Utc::now(),
        generator: GENERATOR_TAG.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers (SRC-3 cosine + token-level pooling)
// ---------------------------------------------------------------------------

/// Compute SRC-3 normalized cosine similarity, mapping raw `[-1, 1]` → `[0, 1]`.
///
/// Returns `0.0` on empty / dimension-mismatched / zero-norm inputs (caller
/// should treat these as "no signal"). Never panics.
fn src3_cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        let ai = a[i];
        let bi = b[i];
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    let raw = dot / (na.sqrt() * nb.sqrt());
    let raw = raw.clamp(-1.0, 1.0);
    ((raw + 1.0) * 0.5).clamp(0.0, 1.0)
}

/// Mean-pool a token-level embedding into a single vector, skipping empty /
/// mismatched token rows.
fn mean_pool_tokens(tokens: &[Vec<f32>]) -> Vec<f32> {
    if tokens.is_empty() {
        return Vec::new();
    }
    // Find the canonical token dim from the first non-empty row.
    let dim = match tokens.iter().find(|t| !t.is_empty()).map(|t| t.len()) {
        Some(d) => d,
        None => return Vec::new(),
    };
    let mut sum = vec![0.0f32; dim];
    let mut count = 0usize;
    for tok in tokens {
        if tok.len() != dim {
            continue;
        }
        for (i, v) in tok.iter().enumerate() {
            sum[i] += *v;
        }
        count += 1;
    }
    if count == 0 {
        return Vec::new();
    }
    let inv = 1.0 / count as f32;
    for v in sum.iter_mut() {
        *v *= inv;
    }
    sum
}

/// SRC-3 cosine between mean-pooled E12 tokens. Zero when either side is empty.
fn token_level_src3(a: &[Vec<f32>], b: &[Vec<f32>]) -> f32 {
    let a_pool = mean_pool_tokens(a);
    let b_pool = mean_pool_tokens(b);
    if a_pool.is_empty() || b_pool.is_empty() {
        return 0.0;
    }
    src3_cosine(&a_pool, &b_pool)
}

// ---------------------------------------------------------------------------
// Constants re-exported for documentation convenience.
// ---------------------------------------------------------------------------

/// Re-export: default high threshold (see [`DEFAULT_HIGH_THRESHOLD`]).
pub const HIGH_THRESHOLD_DEFAULT: f32 = DEFAULT_HIGH_THRESHOLD;
/// Re-export: default low threshold (see [`DEFAULT_LOW_THRESHOLD`]).
pub const LOW_THRESHOLD_DEFAULT: f32 = DEFAULT_LOW_THRESHOLD;
