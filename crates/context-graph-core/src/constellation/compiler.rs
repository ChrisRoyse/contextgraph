//! Constellation compiler: turn a stream of member fingerprints into a
//! finalized [`Constellation`] record.
//!
//! # Strategy
//!
//! A constellation requires both centroid stats (mean vector, sparse term
//! means, pooled token centroid, topic / group / cross-correlation
//! centroids) **and** per-member cosine-to-centroid spread statistics. Cosine
//! stats require the centroid, so this is intrinsically two-pass work:
//!
//! 1. **Pass 1** (`observe`): feed fingerprints into [`WelfordVector`],
//!    reservoir samples of L2 norms, sparse term sums, and pooled token
//!    centroids. Also buffer the members so pass 2 can revisit them.
//! 2. **Pass 2** (`finalize`): with centroids in hand, compute cosine of each
//!    member vector against the embedder centroid and feed into dedicated
//!    cosine reservoirs.
//!
//! We bound the in-memory buffer at `max_members` (default **50 000**).
//! Exceeding the cap returns a `ConstellationError::TooManyMembers`; the
//! caller is expected to pre-filter to a manageable size.
//!
//! Because we buffer the full member set, a constellation of N memories
//! uses O(N × dense_sizes) bytes transiently during compilation. For 50 000
//! members × 14 embedder slots × 1024 f32 worst-case that's roughly 2.7 GB.
//! Reduce `max_members` for low-RAM environments.

use std::collections::HashMap;

use thiserror::Error;
use uuid::Uuid;

use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

use super::reservoir::ReservoirSample;
use super::types::{
    Constellation, ConstellationScoringResult, ConstellationSelector, EmbedderStats, VectorKind,
    CROSS_CORRELATION_CENTROID_DIM, GROUP_ALIGNMENT_CENTROID_DIM, NUM_CONSTELLATION_EMBEDDERS,
    TOPIC_PROFILE_CENTROID_DIM,
};
use super::welford::{WelfordStats, WelfordVector};
use super::{MIN_CONSTELLATION_MEMBERS, RESERVOIR_SAMPLE_SIZE};

/// Compiler default cap on in-memory members per constellation.
pub const DEFAULT_MAX_MEMBERS: usize = 50_000;

/// Max terms kept in `EmbedderStats::sparse_top_terms`.
const SPARSE_TOP_K: usize = 50;

/// E12 ColBERT pooled dimension (128 per token).
const E12_TOKEN_DIM: usize = 128;

/// Errors returned by the constellation compiler.
#[derive(Debug, Error)]
pub enum ConstellationError {
    #[error("constellation requires at least {min} members (got {count})")]
    TooFewMembers { count: usize, min: usize },
    #[error("constellation exceeded max_members={max}; got {count}")]
    TooManyMembers { count: usize, max: usize },
    #[error("embedder {embedder_index} dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        embedder_index: u8,
        expected: usize,
        actual: usize,
    },
    #[error("embedder {embedder_index} centroid is all-zero; constellation is degenerate")]
    DegenerateCentroid { embedder_index: u8 },
}

/// Per-embedder raw observation. Kept in memory between pass 1 and pass 2 so
/// the cosine-against-centroid computation in `finalize` can revisit each
/// member's vector without a second round of downstream fetches.
#[derive(Debug, Clone)]
enum ObservedVector {
    /// Dense observation (E1 / E2 / E3 / E4 / E5 cause / E7 / E8 source /
    /// E9 / E10 paraphrase / E11).
    Dense(Vec<f32>),
    /// Sparse observation (E6 / E13).
    Sparse { indices: Vec<u16>, values: Vec<f32> },
    /// Token-level observation (E12). Each inner `Vec<f32>` is one token
    /// (expected length 128).
    Tokens(Vec<Vec<f32>>),
    /// Placeholder when the member had no vector for this embedder.
    Missing,
}

/// Accumulator for a single embedder's observations. Dense / sparse / token
/// paths are all multiplexed here so the compiler can iterate uniformly.
struct EmbedderAccumulator {
    kind: VectorKind,
    /// Declared dimension (0 for sparse / token-level).
    dimension: usize,
    /// Pass-1 centroid accumulator for dense / asymmetric embedders.
    welford: Option<WelfordVector>,
    /// Sum of sparse values keyed by term id. Divided by member count at
    /// finalize time to get mean weight.
    sparse_term_sums: Option<HashMap<u16, f32>>,
    /// Pooled token centroid for E12 (each member contributes one
    /// mean-pooled 128D vector).
    token_pooled: Option<WelfordVector>,
    /// Per-member token count for E12.
    token_count_stats: Option<WelfordStats>,
    /// Reservoir of L2 norms (dense / asymmetric / pooled-token) or sqrt(sumsq)
    /// (sparse) values.
    l2_samples: ReservoirSample,
    /// Reservoir of cosine-to-centroid values (populated in pass 2).
    cosine_samples: ReservoirSample,
    /// Number of members that contributed a non-zero vector for this embedder.
    non_empty_count: u64,
    /// Per-member observations, kept so pass 2 can walk them.
    observations: Vec<ObservedVector>,
}

impl EmbedderAccumulator {
    fn new(kind: VectorKind, dimension: usize) -> Self {
        let welford = match kind {
            VectorKind::Dense | VectorKind::Asymmetric => Some(WelfordVector::new(dimension)),
            _ => None,
        };
        let sparse_term_sums = match kind {
            VectorKind::Sparse => Some(HashMap::new()),
            _ => None,
        };
        let token_pooled = match kind {
            VectorKind::TokenLevel => Some(WelfordVector::new(E12_TOKEN_DIM)),
            _ => None,
        };
        let token_count_stats = match kind {
            VectorKind::TokenLevel => Some(WelfordStats::new()),
            _ => None,
        };
        Self {
            kind,
            dimension,
            welford,
            sparse_term_sums,
            token_pooled,
            token_count_stats,
            l2_samples: ReservoirSample::new(RESERVOIR_SAMPLE_SIZE),
            cosine_samples: ReservoirSample::new(RESERVOIR_SAMPLE_SIZE),
            non_empty_count: 0,
            observations: Vec::new(),
        }
    }
}

/// Streaming constellation accumulator. Owns all in-memory observation data
/// until `finalize` is called.
pub struct ConstellationAccumulator {
    selector: ConstellationSelector,
    label: String,
    max_members: usize,
    member_ids: Vec<Uuid>,
    e_stats: Vec<EmbedderAccumulator>,
    topic_profile: WelfordVector,
    group_alignments: WelfordVector,
    cross_correlations: WelfordVector,
    /// Running count of topic-match observations fed via `observe_topic_match`.
    /// Populated only for `Topic` selectors. Numerator of `purity`.
    topic_match_count: u64,
}

impl ConstellationAccumulator {
    /// Construct with default `max_members = 50_000`.
    pub fn new(selector: ConstellationSelector, label: String) -> Self {
        Self::with_max_members(selector, label, DEFAULT_MAX_MEMBERS)
    }

    /// Construct with an explicit cap on in-memory members.
    pub fn with_max_members(
        selector: ConstellationSelector,
        label: String,
        max_members: usize,
    ) -> Self {
        let e_stats: Vec<EmbedderAccumulator> = (0..NUM_CONSTELLATION_EMBEDDERS)
            .map(|i| {
                let (kind, dim) = default_kind_and_dim(i as u8);
                EmbedderAccumulator::new(kind, dim)
            })
            .collect();
        Self {
            selector,
            label,
            max_members,
            member_ids: Vec::new(),
            e_stats,
            topic_profile: WelfordVector::new(TOPIC_PROFILE_CENTROID_DIM),
            group_alignments: WelfordVector::new(GROUP_ALIGNMENT_CENTROID_DIM),
            cross_correlations: WelfordVector::new(CROSS_CORRELATION_CENTROID_DIM),
            topic_match_count: 0,
        }
    }

    /// Observe one member fingerprint.
    ///
    /// `stored_topic_profile` is the 14D topic profile read from
    /// `CF_TOPIC_PROFILES` at the caller's discretion; a `None` here simply
    /// skips the topic-profile contribution for this member (it is *not*
    /// zero-imputed, since zero-imputing would bias the centroid toward
    /// origin).
    ///
    /// Returns `Err(TooManyMembers)` when the configured `max_members` cap is
    /// exceeded.
    pub fn observe(
        &mut self,
        id: Uuid,
        fp: &SemanticFingerprint,
        stored_topic_profile: Option<[f32; NUM_EMBEDDERS]>,
        synergy: &SynergyMatrix,
    ) -> Result<(), ConstellationError> {
        if self.member_ids.len() >= self.max_members {
            return Err(ConstellationError::TooManyMembers {
                count: self.member_ids.len() + 1,
                max: self.max_members,
            });
        }
        self.member_ids.push(id);

        // ---- Dense embedders ----
        observe_dense(&mut self.e_stats[0], &fp.e1_semantic);
        observe_dense(&mut self.e_stats[1], &fp.e2_temporal_recent);
        observe_dense(&mut self.e_stats[2], &fp.e3_temporal_periodic);
        observe_dense(&mut self.e_stats[3], &fp.e4_temporal_positional);
        observe_dense(&mut self.e_stats[4], &fp.e5_causal_as_cause);
        observe_sparse(&mut self.e_stats[5], &fp.e6_sparse);
        observe_dense(&mut self.e_stats[6], &fp.e7_code);
        observe_dense(&mut self.e_stats[7], &fp.e8_graph_as_source);
        observe_dense(&mut self.e_stats[8], &fp.e9_hdc);
        observe_dense(&mut self.e_stats[9], &fp.e10_multimodal_paraphrase);
        observe_dense(&mut self.e_stats[10], &fp.e11_entity);
        observe_tokens(&mut self.e_stats[11], &fp.e12_late_interaction);
        observe_sparse(&mut self.e_stats[12], &fp.e13_splade);
        observe_dense(&mut self.e_stats[13], &fp.e14_bge_m3_dense);

        // ---- Topic profile / groups / cross-correlations ----
        if let Some(prof) = stored_topic_profile {
            self.topic_profile.observe(&prof);
            let groups = crate::training::compute_group_alignments(&prof);
            self.group_alignments.observe(&groups);
            let cc = crate::training::compute_cross_correlations(&prof, synergy);
            self.cross_correlations.observe(&cc);
        }
        Ok(())
    }

    /// Record an additional topic-match observation for purity tracking.
    ///
    /// The caller is responsible for deciding whether the just-observed
    /// member matches the constellation's topic label. For a
    /// `ConstellationSelector::Topic { topic_id }` selector, the caller
    /// normally calls this once per member with `matches = (topic_id ∈
    /// fp.topic_memberships)`.
    pub fn observe_topic_match(&mut self, matches: bool) {
        if matches {
            self.topic_match_count += 1;
        }
    }

    /// Run pass 2 over the buffered observations to produce the final
    /// constellation record.
    ///
    /// Fails with `TooFewMembers` when fewer than
    /// [`MIN_CONSTELLATION_MEMBERS`] members were observed.
    pub fn finalize(self) -> Result<Constellation, ConstellationError> {
        let count = self.member_ids.len();
        if count < MIN_CONSTELLATION_MEMBERS {
            return Err(ConstellationError::TooFewMembers {
                count,
                min: MIN_CONSTELLATION_MEMBERS,
            });
        }

        // Build per-embedder stats (drive pass 2 cosine computation).
        let mut per_embedder: Vec<EmbedderStats> = Vec::with_capacity(NUM_CONSTELLATION_EMBEDDERS);
        let ConstellationAccumulator {
            selector,
            label,
            max_members: _,
            member_ids,
            e_stats,
            topic_profile,
            group_alignments,
            cross_correlations,
            topic_match_count,
        } = self;
        for (idx, mut acc) in e_stats.into_iter().enumerate() {
            let emb_idx = idx as u8;
            finalize_cosine_pass(&mut acc, emb_idx)?;
            per_embedder.push(build_stats(emb_idx, acc, count));
        }

        // Topic / group / cross-correlation centroids.
        let mut topic_centroid = [0.0f32; TOPIC_PROFILE_CENTROID_DIM];
        for (i, x) in topic_profile.mean().iter().enumerate() {
            topic_centroid[i] = *x;
        }
        let mut group_centroid = [0.0f32; GROUP_ALIGNMENT_CENTROID_DIM];
        for (i, x) in group_alignments.mean().iter().enumerate() {
            group_centroid[i] = *x;
        }
        let cc_centroid = cross_correlations.mean().to_vec();

        // Coherence = mean of E1 cosines.
        let coherence = mean_cosine(&per_embedder[0]);

        // Purity is only reported for topic selectors.
        let purity = match &selector {
            ConstellationSelector::Topic { .. } => {
                Some((topic_match_count as f32) / (count as f32))
            }
            _ => None,
        };

        Ok(Constellation {
            id: Uuid::new_v4(),
            label,
            created_at: chrono::Utc::now(),
            selector,
            member_count: count,
            member_ids,
            per_embedder,
            topic_profile_centroid: topic_centroid,
            group_alignment_centroid: group_centroid,
            cross_correlation_centroid: cc_centroid,
            coherence,
            purity,
        })
    }
}

/// Top-level convenience: build a constellation from an iterator of member
/// tuples `(id, fingerprint, stored_topic_profile)`.
///
/// Intended for callers that already have the full member set in hand; for
/// streaming use cases, drive `ConstellationAccumulator` manually.
pub fn compile_constellation<I>(
    selector: ConstellationSelector,
    label: String,
    members: I,
    synergy: &SynergyMatrix,
) -> Result<Constellation, ConstellationError>
where
    I: IntoIterator<Item = (Uuid, SemanticFingerprint, Option<[f32; NUM_EMBEDDERS]>)>,
{
    let mut acc = ConstellationAccumulator::new(selector, label);
    for (id, fp, profile) in members {
        acc.observe(id, &fp, profile, synergy)?;
    }
    acc.finalize()
}

/// Score a candidate memory against an already-compiled constellation.
///
/// For each embedder, computes the cosine of the candidate's active vector
/// against the constellation's centroid. Returns a summary including the
/// combined (unweighted) mean over embedders with `coverage > 0` and a flag
/// marking whether the candidate is inside the E1 95th-percentile spread.
pub fn score_memory_against_constellation(
    constellation: &Constellation,
    memory_id: Uuid,
    fp: &SemanticFingerprint,
) -> ConstellationScoringResult {
    let mut per = [0.0f32; NUM_CONSTELLATION_EMBEDDERS];
    let mut active_count = 0u32;
    let mut summed = 0.0f32;

    for (i, stats) in constellation.per_embedder.iter().enumerate() {
        if stats.coverage <= 0.0 {
            continue;
        }
        let c = score_one(stats, fp);
        per[i] = c;
        summed += c;
        active_count += 1;
    }

    let combined = if active_count == 0 {
        0.0
    } else {
        summed / active_count as f32
    };

    // E1 gate: candidate is inside spread when E1 cosine ≥ p95 threshold.
    let in_spread_p95 = {
        let e1 = &constellation.per_embedder[0];
        if e1.coverage > 0.0 && e1.cosine_spread_p95.is_finite() {
            per[0] >= e1.cosine_spread_p95 - 1e-6
        } else {
            false
        }
    };

    ConstellationScoringResult {
        constellation_id: constellation.id,
        memory_id,
        per_embedder_cosine: per,
        combined_score: combined,
        in_spread_p95,
    }
}

// ==========================================================================
// Internals
// ==========================================================================

/// Return `(VectorKind, dimension)` for a given embedder index. Dimensions
/// follow the ModelId constants in `teleological/indexes.rs` (E1=1024,
/// E2/3/4=512, E5=768, E7=1536, E8=1024, E9=1024, E10=768, E11=768). Sparse
/// and token-level kinds have `dimension = 0`.
fn default_kind_and_dim(idx: u8) -> (VectorKind, usize) {
    match idx {
        0 => (VectorKind::Dense, 1024),      // E1
        1 => (VectorKind::Dense, 512),       // E2
        2 => (VectorKind::Dense, 512),       // E3
        3 => (VectorKind::Dense, 512),       // E4
        4 => (VectorKind::Asymmetric, 768),  // E5 cause
        5 => (VectorKind::Sparse, 0),        // E6
        6 => (VectorKind::Dense, 1536),      // E7
        7 => (VectorKind::Asymmetric, 1024), // E8 source
        8 => (VectorKind::Dense, 1024),      // E9
        9 => (VectorKind::Asymmetric, 768),  // E10 paraphrase
        10 => (VectorKind::Dense, 768),      // E11
        11 => (VectorKind::TokenLevel, 0),   // E12
        12 => (VectorKind::Sparse, 0),       // E13
        13 => (VectorKind::Dense, 1024),     // E14 BGE-M3 Dense
        _ => (VectorKind::Dense, 0),
    }
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

fn sparse_sumsq(s: &SparseVector) -> f32 {
    s.values.iter().map(|x| x * x).sum()
}

fn sparse_dot(av: &[(u16, f32)], bv: &[(u16, f32)]) -> f32 {
    // Both sides are independent (a, v) lists; build a small map for the
    // smaller side and probe.
    let (small, big) = if av.len() <= bv.len() {
        (av, bv)
    } else {
        (bv, av)
    };
    let map: HashMap<u16, f32> = small.iter().copied().collect();
    let mut dot = 0.0f32;
    for (idx, val) in big {
        if let Some(m) = map.get(idx) {
            dot += val * m;
        }
    }
    dot
}

fn observe_dense(acc: &mut EmbedderAccumulator, v: &[f32]) {
    if v.is_empty() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    if v.len() != acc.dimension {
        // Dimension mismatch — treat as missing and continue. We log nothing
        // here because this function is on a hot path; finalize will surface
        // the coverage drop via EmbedderStats.coverage.
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    // Drop vectors that are all-zero or contain NaN/Inf; they would poison
    // the centroid and the cosine reservoir.
    if !v.iter().all(|x| x.is_finite()) {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    let n = l2_norm(v);
    if n <= 0.0 || !n.is_finite() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }

    if let Some(w) = acc.welford.as_mut() {
        w.observe(v);
    }
    acc.l2_samples.observe(n);
    acc.non_empty_count += 1;
    acc.observations.push(ObservedVector::Dense(v.to_vec()));
}

fn observe_sparse(acc: &mut EmbedderAccumulator, s: &SparseVector) {
    if s.indices.is_empty() || s.indices.len() != s.values.len() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    if !s.values.iter().all(|x| x.is_finite()) {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    let sumsq = sparse_sumsq(s);
    if sumsq <= 0.0 || !sumsq.is_finite() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    if let Some(map) = acc.sparse_term_sums.as_mut() {
        for (idx, val) in s.indices.iter().zip(s.values.iter()) {
            *map.entry(*idx).or_insert(0.0) += *val;
        }
    }
    acc.l2_samples.observe(sumsq.sqrt());
    acc.non_empty_count += 1;
    acc.observations.push(ObservedVector::Sparse {
        indices: s.indices.clone(),
        values: s.values.clone(),
    });
}

fn observe_tokens(acc: &mut EmbedderAccumulator, tokens: &[Vec<f32>]) {
    if tokens.is_empty() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    // Mean-pool each member's tokens into one E12_TOKEN_DIM vector.
    let mut pooled = vec![0.0f32; E12_TOKEN_DIM];
    let mut valid = 0usize;
    for tok in tokens {
        if tok.len() != E12_TOKEN_DIM {
            continue;
        }
        if !tok.iter().all(|x| x.is_finite()) {
            continue;
        }
        for i in 0..E12_TOKEN_DIM {
            pooled[i] += tok[i];
        }
        valid += 1;
    }
    if valid == 0 {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    let inv = 1.0 / valid as f32;
    for x in &mut pooled {
        *x *= inv;
    }
    let n = l2_norm(&pooled);
    if n <= 0.0 || !n.is_finite() {
        acc.observations.push(ObservedVector::Missing);
        return;
    }
    if let Some(w) = acc.token_pooled.as_mut() {
        w.observe(&pooled);
    }
    if let Some(stats) = acc.token_count_stats.as_mut() {
        stats.observe(tokens.len() as f32);
    }
    acc.l2_samples.observe(n);
    acc.non_empty_count += 1;
    acc.observations
        .push(ObservedVector::Tokens(tokens.to_vec()));
}

/// Pass 2: walk buffered observations and fill the cosine-to-centroid
/// reservoir.
fn finalize_cosine_pass(
    acc: &mut EmbedderAccumulator,
    _embedder_index: u8,
) -> Result<(), ConstellationError> {
    match acc.kind {
        VectorKind::Dense | VectorKind::Asymmetric => {
            let Some(welford) = acc.welford.as_ref() else {
                return Ok(());
            };
            if welford.count() == 0 || welford.mean_l2() == 0.0 {
                return Ok(());
            }
            let centroid = welford.mean().to_vec();
            // Consume observations; we no longer need them after cosines
            // are sampled.
            let obs = std::mem::take(&mut acc.observations);
            for o in &obs {
                if let ObservedVector::Dense(v) = o {
                    let c = cosine(v, &centroid);
                    acc.cosine_samples.observe(c);
                }
            }
        }
        VectorKind::Sparse => {
            let Some(sums) = acc.sparse_term_sums.as_ref() else {
                return Ok(());
            };
            if acc.non_empty_count == 0 {
                return Ok(());
            }
            let inv = 1.0 / acc.non_empty_count as f32;
            let mean_sparse: Vec<(u16, f32)> = sums.iter().map(|(k, v)| (*k, *v * inv)).collect();
            let mean_norm = sparse_l2(&mean_sparse);
            let obs = std::mem::take(&mut acc.observations);
            if mean_norm <= 0.0 {
                return Ok(());
            }
            for o in &obs {
                if let ObservedVector::Sparse { indices, values } = o {
                    let member: Vec<(u16, f32)> = indices
                        .iter()
                        .zip(values.iter())
                        .map(|(i, v)| (*i, *v))
                        .collect();
                    let dot = sparse_dot(&member, &mean_sparse);
                    let m_norm = (values.iter().map(|v| v * v).sum::<f32>()).sqrt();
                    if m_norm <= 0.0 {
                        continue;
                    }
                    let c = (dot / (m_norm * mean_norm)).clamp(-1.0, 1.0);
                    acc.cosine_samples.observe(c);
                }
            }
        }
        VectorKind::TokenLevel => {
            let Some(w) = acc.token_pooled.as_ref() else {
                return Ok(());
            };
            if w.count() == 0 || w.mean_l2() == 0.0 {
                return Ok(());
            }
            let centroid = w.mean().to_vec();
            let obs = std::mem::take(&mut acc.observations);
            for o in &obs {
                if let ObservedVector::Tokens(toks) = o {
                    let pooled = pool_tokens_for_cosine(toks);
                    let c = cosine(&pooled, &centroid);
                    acc.cosine_samples.observe(c);
                }
            }
        }
    }
    Ok(())
}

fn sparse_l2(v: &[(u16, f32)]) -> f32 {
    v.iter().map(|(_, x)| x * x).sum::<f32>().sqrt()
}

fn pool_tokens_for_cosine(tokens: &[Vec<f32>]) -> Vec<f32> {
    let mut pooled = vec![0.0f32; E12_TOKEN_DIM];
    let mut valid = 0usize;
    for tok in tokens {
        if tok.len() != E12_TOKEN_DIM {
            continue;
        }
        for i in 0..E12_TOKEN_DIM {
            pooled[i] += tok[i];
        }
        valid += 1;
    }
    if valid > 0 {
        let inv = 1.0 / valid as f32;
        for x in &mut pooled {
            *x *= inv;
        }
    }
    pooled
}

fn build_stats(emb_idx: u8, acc: EmbedderAccumulator, total_members: usize) -> EmbedderStats {
    let centroid = match &acc.welford {
        Some(w) if w.count() > 0 => w.mean().to_vec(),
        _ => Vec::new(),
    };
    let mean_l2 = match &acc.welford {
        Some(w) if w.count() > 0 => w.mean_l2(),
        _ => 0.0,
    };
    // Population stddev of L2 norms across members (not the WelfordVector's
    // per-coordinate dispersion). Use the reservoir sample as a bounded-size
    // proxy — for N ≤ capacity the stats are exact; above capacity they are
    // unbiased estimates.
    let stddev_l2 = reservoir_stddev(&acc.l2_samples);
    let (sparse_top_terms, centroid_sparse_mean_l2) = match (&acc.sparse_term_sums, acc.kind) {
        (Some(map), VectorKind::Sparse) if acc.non_empty_count > 0 => {
            let inv = 1.0 / acc.non_empty_count as f32;
            let mut v: Vec<(u16, f32)> = map.iter().map(|(k, val)| (*k, *val * inv)).collect();
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let mean_norm = sparse_l2(&v);
            v.truncate(SPARSE_TOP_K);
            (v, mean_norm)
        }
        _ => (Vec::new(), 0.0),
    };
    let (pooled_token_centroid, token_mean_l2) = match (&acc.token_pooled, acc.kind) {
        (Some(w), VectorKind::TokenLevel) if w.count() > 0 => (w.mean().to_vec(), w.mean_l2()),
        _ => (Vec::new(), 0.0),
    };
    // Final mean_l2 for sparse/token-level uses their own centroid norms.
    let final_mean_l2 = match acc.kind {
        VectorKind::Dense | VectorKind::Asymmetric => mean_l2,
        VectorKind::Sparse => centroid_sparse_mean_l2,
        VectorKind::TokenLevel => token_mean_l2,
    };
    let mean_token_count = acc
        .token_count_stats
        .as_ref()
        .filter(|s| s.count() > 0)
        .map(|s| s.mean());

    let p50 = acc.cosine_samples.percentile(0.5);
    let p95 = acc.cosine_samples.percentile(0.95);
    let min_c = if acc.cosine_samples.is_empty() {
        0.0
    } else {
        acc.cosine_samples.min()
    };
    let max_c = if acc.cosine_samples.is_empty() {
        0.0
    } else {
        acc.cosine_samples.max()
    };
    let coverage = if total_members == 0 {
        0.0
    } else {
        acc.non_empty_count as f32 / total_members as f32
    };

    EmbedderStats {
        embedder_index: emb_idx,
        dimension: acc.dimension as u16,
        vector_kind: acc.kind,
        centroid,
        sparse_top_terms,
        mean_token_count,
        pooled_token_centroid,
        mean_l2: final_mean_l2,
        stddev_l2,
        cosine_spread_p50: p50,
        cosine_spread_p95: p95,
        min_cosine: min_c,
        max_cosine: max_c,
        coverage,
    }
}

fn reservoir_stddev(r: &ReservoirSample) -> f32 {
    let samples = r.sorted();
    if samples.len() < 2 {
        return 0.0;
    }
    let n = samples.len() as f32;
    let mean = samples.iter().sum::<f32>() / n;
    let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    var.sqrt()
}

fn mean_cosine(stats: &EmbedderStats) -> f32 {
    // Use the median as a robust proxy for the cosine distribution's center.
    // (Mean would require storing the raw average, which the reservoir does
    // not reconstruct faithfully at scale.) At small N the reservoir is exact
    // so median ≈ mean for near-constant streams.
    stats.cosine_spread_p50
}

fn score_one(stats: &EmbedderStats, fp: &SemanticFingerprint) -> f32 {
    match stats.vector_kind {
        VectorKind::Dense | VectorKind::Asymmetric => {
            if stats.centroid.is_empty() {
                return 0.0;
            }
            let v = pick_dense(stats.embedder_index, fp);
            cosine(v, &stats.centroid)
        }
        VectorKind::Sparse => {
            if stats.sparse_top_terms.is_empty() {
                return 0.0;
            }
            let member = pick_sparse(stats.embedder_index, fp);
            let Some((idx, val)) = member else { return 0.0 };
            let member_vec: Vec<(u16, f32)> =
                idx.iter().zip(val.iter()).map(|(i, v)| (*i, *v)).collect();
            let mean_vec: Vec<(u16, f32)> = stats.sparse_top_terms.clone();
            let dot = sparse_dot(&member_vec, &mean_vec);
            let m_norm = (val.iter().map(|x| x * x).sum::<f32>()).sqrt();
            let c_norm = sparse_l2(&mean_vec);
            if m_norm <= 0.0 || c_norm <= 0.0 {
                return 0.0;
            }
            (dot / (m_norm * c_norm)).clamp(-1.0, 1.0)
        }
        VectorKind::TokenLevel => {
            if stats.pooled_token_centroid.is_empty() {
                return 0.0;
            }
            let pooled = pool_tokens_for_cosine(&fp.e12_late_interaction);
            cosine(&pooled, &stats.pooled_token_centroid)
        }
    }
}

fn pick_dense(emb_idx: u8, fp: &SemanticFingerprint) -> &[f32] {
    match emb_idx {
        0 => &fp.e1_semantic,
        1 => &fp.e2_temporal_recent,
        2 => &fp.e3_temporal_periodic,
        3 => &fp.e4_temporal_positional,
        4 => &fp.e5_causal_as_cause,
        6 => &fp.e7_code,
        7 => &fp.e8_graph_as_source,
        8 => &fp.e9_hdc,
        9 => &fp.e10_multimodal_paraphrase,
        10 => &fp.e11_entity,
        13 => &fp.e14_bge_m3_dense,
        _ => &[],
    }
}

fn pick_sparse(emb_idx: u8, fp: &SemanticFingerprint) -> Option<(&[u16], &[f32])> {
    match emb_idx {
        5 => Some((&fp.e6_sparse.indices, &fp.e6_sparse.values)),
        12 => Some((&fp.e13_splade.indices, &fp.e13_splade.values)),
        _ => None,
    }
}
