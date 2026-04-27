//! Value types for the constellation compiler (Phase 2).
//!
//! A [`Constellation`] bundles per-embedder centroids and spread statistics for
//! a set of memories selected by topic, session, tag, time range, or explicit
//! id list. The shape is deliberately rigid (fixed 14 embedders, fixed 14D
//! topic profile, fixed 6D group alignments, fixed 91D cross-correlation
//! centroid) so bincode round-trips without ambiguity.
//!
//! All structs are `#[derive(Serialize, Deserialize)]` with **no**
//! `skip_serializing_if` — bincode is positional and silently corrupts on
//! optional fields (see PRD v6 §2.2 and `tasks/lessons.md`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::types::NUM_EMBEDDERS;

/// Expected length of the per-embedder stats array — one entry per embedder.
pub const NUM_CONSTELLATION_EMBEDDERS: usize = NUM_EMBEDDERS;

/// Expected length of `topic_profile_centroid`: 14D.
pub const TOPIC_PROFILE_CENTROID_DIM: usize = NUM_EMBEDDERS;

/// Expected length of `group_alignment_centroid`: 6D (Factual, Temporal,
/// Causal, Relational, Qualitative, Implementation).
pub const GROUP_ALIGNMENT_CENTROID_DIM: usize = 6;

/// Expected length of `cross_correlation_centroid`: C(14, 2) = 91.
pub const CROSS_CORRELATION_CENTROID_DIM: usize = NUM_EMBEDDERS * (NUM_EMBEDDERS - 1) / 2;

/// A compiled constellation record, persisted to `CF_CONSTELLATIONS`.
///
/// Encoded on-disk as `[CONSTELLATION_VERSION: u8][bincode]`. Bump the version
/// byte whenever the layout changes in a non-backwards-compatible way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constellation {
    /// Fresh UUID per compilation. Distinct from any member memory id.
    pub id: Uuid,
    /// Human-readable tag (e.g. "PRD §7 — Case management"). Caller-supplied.
    pub label: String,
    pub created_at: DateTime<Utc>,

    /// How the member set was resolved. Hashed into the secondary index.
    pub selector: ConstellationSelector,
    pub member_ids: Vec<Uuid>,
    pub member_count: usize,

    /// Per-embedder centroid + spread stats, one entry per embedder index.
    /// Length is always `NUM_CONSTELLATION_EMBEDDERS` (14 after E14 BGE-M3 Dense).
    pub per_embedder: Vec<EmbedderStats>,

    /// Mean 14D topic profile across all members. **SILENT-ZERO WARNING**:
    /// compiler only observes this when `stored_topic_profile = Some(_)`
    /// (read from `CF_TOPIC_PROFILES`). On a fresh DB before the clustering
    /// pipeline has populated that CF, every member yields `None` and the
    /// Welford accumulator stays at `count = 0`, so this field serializes as
    /// all-zeros. An all-zero `topic_profile_centroid` is therefore
    /// indistinguishable from "no observations" — callers that need
    /// high-confidence topic centroids should check that at least one
    /// non-zero slot exists before trusting this field, or use
    /// [`Self::has_topic_profile_signal`].
    pub topic_profile_centroid: [f32; TOPIC_PROFILE_CENTROID_DIM],
    /// Mean 6D group alignment across all members. **SILENT-ZERO WARNING**:
    /// derived from `topic_profile_centroid`; identical caveat applies.
    pub group_alignment_centroid: [f32; GROUP_ALIGNMENT_CENTROID_DIM],
    /// Mean 91-entry cross-correlation vector across all members. Always
    /// length `CROSS_CORRELATION_CENTROID_DIM` (91). **SILENT-ZERO WARNING**:
    /// derived from `topic_profile_centroid`; identical caveat applies.
    pub cross_correlation_centroid: Vec<f32>,

    /// Median (p50) cosine of members against the E1 centroid, sampled via a
    /// bounded reservoir. 1.0 = perfect agreement, 0.0 = orthogonal on
    /// average, negative = anti-correlated. Using the median rather than the
    /// mean keeps coherence robust to outliers when a constellation contains
    /// one or two distant members.
    pub coherence: f32,
    /// Topic purity, in `[0.0, 1.0]`. Populated only when the selector is
    /// `Topic { .. }`; `None` for all other selectors.
    pub purity: Option<f32>,
}

impl Constellation {
    /// Returns `true` when at least one slot of `topic_profile_centroid` is
    /// non-zero. Because topic profiles are SRC-3-normalized alignments in
    /// `[0, 1]` and a real member essentially always has a non-empty vector
    /// for some embedder, an all-zero centroid reliably indicates that
    /// `stored_topic_profile` was `None` for every member (e.g., fresh DB,
    /// clustering pipeline has not yet populated `CF_TOPIC_PROFILES`) —
    /// **not** that the members' topic signal actually averaged to zero.
    ///
    /// Callers that want to condition behavior on topic-centroid
    /// trustworthiness should prefer this helper over raw `.iter().any(...)`
    /// so the intent is obvious at the call site.
    pub fn has_topic_profile_signal(&self) -> bool {
        self.topic_profile_centroid.iter().any(|&x| x != 0.0)
    }
}

/// How a constellation's member set was chosen.
///
/// Each variant serializes to a canonical string form that is SHA-256-hashed
/// into the `CF_CONSTELLATION_BY_SELECTOR` secondary index key. See
/// `canonical_selector_string` in the storage layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstellationSelector {
    /// All members of a given topic cluster.
    Topic { topic_id: String },
    /// All memories stored in a given session.
    Session { session_id: String },
    /// All memories carrying a given tag.
    Tag { tag: String },
    /// All memories in `[start, end]` (inclusive).
    TimeRange {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    /// A caller-specified id list. `rationale` is free-form metadata (not part
    /// of the hash) and `ids` is what determines the cluster identity.
    ExplicitIds { rationale: String, ids: Vec<Uuid> },
}

impl ConstellationSelector {
    /// Canonical byte form for hashing into the secondary index.
    ///
    /// The output is **stable** across runs so the same selector always maps
    /// to the same 16-byte hash prefix in `CF_CONSTELLATION_BY_SELECTOR`.
    ///
    /// For `ExplicitIds` the `rationale` is intentionally **not** hashed; only
    /// the (sorted, deduplicated) id set determines identity. For `TimeRange`
    /// we emit RFC-3339 timestamps with nanosecond precision.
    pub fn canonical_form(&self) -> String {
        match self {
            ConstellationSelector::Topic { topic_id } => format!("topic:{}", topic_id),
            ConstellationSelector::Session { session_id } => format!("session:{}", session_id),
            ConstellationSelector::Tag { tag } => format!("tag:{}", tag),
            ConstellationSelector::TimeRange { start, end } => format!(
                "timerange:{}-{}",
                start.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                end.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            ),
            ConstellationSelector::ExplicitIds { ids, .. } => {
                let mut sorted: Vec<Uuid> = ids.clone();
                sorted.sort();
                sorted.dedup();
                let joined = sorted
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("explicit:{}", joined)
            }
        }
    }

    /// Discriminant byte used as the first byte of the secondary-index key.
    pub fn kind_byte(&self) -> u8 {
        match self {
            ConstellationSelector::Topic { .. } => 0,
            ConstellationSelector::Session { .. } => 1,
            ConstellationSelector::Tag { .. } => 2,
            ConstellationSelector::TimeRange { .. } => 3,
            ConstellationSelector::ExplicitIds { .. } => 4,
        }
    }
}

/// How a given embedder stores its vector — drives which centroid / stats
/// fields get populated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorKind {
    /// Symmetric dense vector (E1, E2, E3, E4, E7, E9, E11).
    Dense,
    /// Sparse vector (E6, E13) — `centroid` stays empty; `sparse_top_terms`
    /// holds the ranked mean-weight terms instead.
    Sparse,
    /// Token-level embedding (E12) — `centroid` stays empty;
    /// `pooled_token_centroid` + `mean_token_count` carry the stats.
    TokenLevel,
    /// Asymmetric dense vector where only one side participates (E5 cause,
    /// E8 source, E10 paraphrase). The centroid is computed on that side.
    Asymmetric,
}

/// Per-embedder centroid + spread stats. One entry exists for every embedder
/// index `0..NUM_CONSTELLATION_EMBEDDERS` regardless of whether that embedder
/// was populated — check `coverage` to find out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderStats {
    /// Embedder index in `0..NUM_CONSTELLATION_EMBEDDERS`.
    pub embedder_index: u8,
    /// Dense vector dimension when `vector_kind == Dense | Asymmetric`.
    /// `0` for sparse or token-level embedders.
    pub dimension: u16,
    /// Classification of this embedder's storage/representation.
    pub vector_kind: VectorKind,

    /// Mean dense vector. Non-empty only for `Dense` / `Asymmetric` embedders.
    pub centroid: Vec<f32>,

    /// Top terms by mean weight, for sparse embedders only. Up to 50 entries,
    /// sorted by weight descending. Empty for non-sparse embedders.
    pub sparse_top_terms: Vec<(u16, f32)>,

    /// Mean number of tokens per member, populated only for token-level
    /// embedders (E12). `None` otherwise.
    pub mean_token_count: Option<f32>,

    /// Pooled 128D token centroid for E12: per member, tokens are
    /// mean-pooled; the resulting 128D vectors are averaged across members.
    /// Empty for non-token embedders.
    pub pooled_token_centroid: Vec<f32>,

    /// Mean L2 norm of member vectors (before cosine normalization).
    pub mean_l2: f32,
    /// Stddev of L2 norms across members.
    pub stddev_l2: f32,
    /// Median cosine similarity of member vectors to the centroid.
    pub cosine_spread_p50: f32,
    /// 95th percentile cosine similarity of member vectors to the centroid.
    pub cosine_spread_p95: f32,
    /// Minimum observed cosine-to-centroid similarity.
    pub min_cosine: f32,
    /// Maximum observed cosine-to-centroid similarity.
    pub max_cosine: f32,
    /// Fraction of members in `[0.0, 1.0]` that had a non-zero vector for
    /// this embedder.
    pub coverage: f32,
}

/// Result of scoring a candidate memory against a compiled constellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationScoringResult {
    pub constellation_id: Uuid,
    pub memory_id: Uuid,
    /// Per-embedder cosine of the candidate's vector against the embedder
    /// centroid. `0.0` when the candidate lacks that embedder's vector or the
    /// centroid is absent.
    pub per_embedder_cosine: [f32; NUM_CONSTELLATION_EMBEDDERS],
    /// Unweighted mean of `per_embedder_cosine` over entries where
    /// `coverage > 0`.
    pub combined_score: f32,
    /// True when `combined_score >= cosine_spread_p95` on E1 — i.e. the
    /// candidate is at least as central as the 95th-percentile member.
    pub in_spread_p95: bool,
}
