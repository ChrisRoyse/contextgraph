//! Value types for the contrastive pair miner (Phase 3).
//!
//! A [`ContrastivePair`] bundles an anchor + negative pair along with the full
//! per-embedder similarity profile that produced the pair. The shape is
//! deliberately rigid (fixed 13-slot similarity profile) so bincode round-trips
//! without ambiguity.
//!
//! All structs are `#[derive(Serialize, Deserialize)]` with **no**
//! `skip_serializing_if` — bincode is positional and silently corrupts on
//! optional fields (see PRD v6 §2.2 and `tasks/lessons.md`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::teleological::types::NUM_EMBEDDERS;

/// Number of [`AnomalyKind`] variants. Kept in sync with
/// `AnomalyKind::all()` so `from_u8` + serialization round-trip fully.
pub const NUM_ANOMALY_KINDS: u8 = 6;

/// Default cap on pairs mined per run.
pub const DEFAULT_MAX_PAIRS: usize = 10_000;

/// Default minimum `high_sim - low_sim` required to keep a pair.
pub const DEFAULT_MIN_DISAGREEMENT: f32 = 0.3;

/// Default similarity threshold for classifying an embedder as "high" on a pair.
pub const DEFAULT_HIGH_THRESHOLD: f32 = 0.6;

/// Default similarity threshold for classifying an embedder as "low" on a pair.
pub const DEFAULT_LOW_THRESHOLD: f32 = 0.3;

/// Default number of top candidates scored per anchor during a mining run.
pub const DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR: usize = 10;

/// Which kind of cross-embedder disagreement a pair represents.
///
/// The six kinds are exhaustive partitioning of the canonical anomaly axes in
/// PRD §5.3. `Other` catches any high/low combination that does not match a
/// named axis.
///
/// On-disk encoding: serialized as a single `u8` via [`AnomalyKind::as_u8`] /
/// [`AnomalyKind::from_u8`]. The secondary index
/// `CF_CONTRASTIVE_BY_KIND` uses that same byte as the first key prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyKind {
    /// High E1 (semantic), low E5 (causal): memories are semantically similar
    /// but structurally unrelated on the causal axis.
    SemanticButNotCausal,
    /// High E6 *or* E13 (sparse lexical), low E10 (multimodal paraphrase):
    /// memories share keywords but carry different contextual meaning.
    KeywordButNotParaphrase,
    /// High E7 (code), low E1 (semantic): similar code shape but different
    /// intent.
    CodeShapeButDifferentIntent,
    /// High E11 (entity), low E8 (graph): memories mention the same entity but
    /// sit in very different relationship structures.
    EntitySharedButDifferentStructure,
    /// High E9 (HDC / typo-robust), low E1 (semantic): the hypervector space
    /// ties the pair together while semantic embedding splits it.
    HdcRobustButSemanticDifferent,
    /// Any other high/low combination that doesn't match a named axis.
    Other,
}

impl AnomalyKind {
    /// Compact discriminant byte, used for on-disk secondary index keys and
    /// (indirectly) for bincode layout.
    ///
    /// Ordering is part of the on-disk contract — reordering requires a
    /// `CONTRASTIVE_PAIR_VERSION` bump.
    pub fn as_u8(&self) -> u8 {
        match self {
            AnomalyKind::SemanticButNotCausal => 0,
            AnomalyKind::KeywordButNotParaphrase => 1,
            AnomalyKind::CodeShapeButDifferentIntent => 2,
            AnomalyKind::EntitySharedButDifferentStructure => 3,
            AnomalyKind::HdcRobustButSemanticDifferent => 4,
            AnomalyKind::Other => 5,
        }
    }

    /// Inverse of [`AnomalyKind::as_u8`]. Returns `None` on any other byte.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(AnomalyKind::SemanticButNotCausal),
            1 => Some(AnomalyKind::KeywordButNotParaphrase),
            2 => Some(AnomalyKind::CodeShapeButDifferentIntent),
            3 => Some(AnomalyKind::EntitySharedButDifferentStructure),
            4 => Some(AnomalyKind::HdcRobustButSemanticDifferent),
            5 => Some(AnomalyKind::Other),
            _ => None,
        }
    }

    /// All variants in `as_u8` order.
    pub fn all() -> [AnomalyKind; 6] {
        [
            AnomalyKind::SemanticButNotCausal,
            AnomalyKind::KeywordButNotParaphrase,
            AnomalyKind::CodeShapeButDifferentIntent,
            AnomalyKind::EntitySharedButDifferentStructure,
            AnomalyKind::HdcRobustButSemanticDifferent,
            AnomalyKind::Other,
        ]
    }

    /// Snake-case string form, matching the MCP tool's `kinds` parameter.
    pub fn as_str(&self) -> &'static str {
        match self {
            AnomalyKind::SemanticButNotCausal => "semantic_but_not_causal",
            AnomalyKind::KeywordButNotParaphrase => "keyword_but_not_paraphrase",
            AnomalyKind::CodeShapeButDifferentIntent => "code_shape_but_different_intent",
            AnomalyKind::EntitySharedButDifferentStructure => {
                "entity_shared_but_different_structure"
            }
            AnomalyKind::HdcRobustButSemanticDifferent => "hdc_robust_but_semantic_different",
            AnomalyKind::Other => "other",
        }
    }

    /// Parse the snake-case form. Returns `None` on unknown inputs.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "semantic_but_not_causal" => Some(AnomalyKind::SemanticButNotCausal),
            "keyword_but_not_paraphrase" => Some(AnomalyKind::KeywordButNotParaphrase),
            "code_shape_but_different_intent" => Some(AnomalyKind::CodeShapeButDifferentIntent),
            "entity_shared_but_different_structure" => {
                Some(AnomalyKind::EntitySharedButDifferentStructure)
            }
            "hdc_robust_but_semantic_different" => Some(AnomalyKind::HdcRobustButSemanticDifferent),
            "other" => Some(AnomalyKind::Other),
            _ => None,
        }
    }
}

/// A mined contrastive pair, persisted to `CF_CONTRASTIVE_PAIRS`.
///
/// Encoded on-disk as `[CONTRASTIVE_PAIR_VERSION: u8][bincode]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContrastivePair {
    pub anchor_id: Uuid,
    pub negative_id: Uuid,
    pub anchor_text: String,
    pub negative_text: String,

    /// Per-embedder similarity between anchor and negative. Length is always
    /// [`NUM_EMBEDDERS`] (13). Entries live in `[0, 1]`.
    pub similarity_profile: [f32; NUM_EMBEDDERS],

    /// Indices into `similarity_profile` that cleared the high threshold at
    /// mining time.
    pub high_embedders: Vec<u8>,
    /// Indices into `similarity_profile` that fell below the low threshold at
    /// mining time.
    pub low_embedders: Vec<u8>,

    /// `max(similarity_profile[h] for h in high) - min(similarity_profile[l]
    /// for l in low)`. Always `>= 0.0` for a pair that survived mining.
    pub disagreement_magnitude: f32,

    /// Which canonical axis this pair best fits.
    pub anomaly_kind: AnomalyKind,

    pub mined_at: DateTime<Utc>,
    /// Free-form tag identifying the generator; today `cross_embedder_anomaly_v1`.
    pub generator: String,
}

/// Caller-tunable miner configuration.
///
/// Defaults match the MCP tool's defaults — see PRD §5.5.
#[derive(Debug, Clone)]
pub struct MiningConfig {
    /// Cap on total pairs mined in this run. Once reached, the miner returns.
    pub max_pairs: usize,
    /// Optional filter on which anomaly kinds to emit. `None` accepts all.
    pub kinds: Option<Vec<AnomalyKind>>,
    /// Minimum `max(high_sim) - min(low_sim)` required to keep a pair.
    pub min_disagreement: f32,
    /// Similarity threshold above which an embedder is tagged as "high" on a pair.
    pub high_threshold: f32,
    /// Similarity threshold below which an embedder is tagged as "low" on a pair.
    pub low_threshold: f32,
    /// Candidate pool size per anchor (top-K by primary similarity); larger =
    /// more anomaly hits, smaller = faster.
    pub top_k_candidates_per_anchor: usize,
    /// When `Some`, only anchors whose `SourceMetadata.session_id` matches are
    /// scanned. `None` scans every memory.
    pub session_filter: Option<String>,
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            max_pairs: DEFAULT_MAX_PAIRS,
            kinds: None,
            min_disagreement: DEFAULT_MIN_DISAGREEMENT,
            high_threshold: DEFAULT_HIGH_THRESHOLD,
            low_threshold: DEFAULT_LOW_THRESHOLD,
            top_k_candidates_per_anchor: DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR,
            session_filter: None,
        }
    }
}

/// Summary returned by the miner after a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MiningSummary {
    /// Number of pairs successfully written to `CF_CONTRASTIVE_PAIRS`.
    pub pairs_stored: usize,
    /// Candidate pairs dropped because `disagreement_magnitude <
    /// min_disagreement`.
    pub pairs_skipped_below_threshold: usize,
    /// Candidate pairs dropped because their classified kind was not in the
    /// `kinds` filter.
    pub pairs_skipped_kind_filter: usize,
    /// Number of distinct anchors the miner visited.
    pub anchors_scanned: usize,
    /// Anchors that produced zero candidates (no peers, or every peer skipped).
    pub anchors_no_candidate: usize,
    /// Wall-clock duration of the mining run in milliseconds.
    pub duration_ms: u64,
}

/// Errors raised by the miner's configuration / pure-functional layer.
///
/// Storage-layer errors live in `context_graph_storage` and surface through
/// the MCP handler.
#[derive(Error, Debug)]
pub enum ContrastiveError {
    /// `min_disagreement` was outside `[0, 1]`.
    #[error("min_disagreement must be in [0, 1], got {0}")]
    InvalidDisagreement(f32),

    /// `low_threshold >= high_threshold`.
    #[error("thresholds must satisfy low < high, got low={0}, high={1}")]
    InvalidThresholds(f32, f32),

    /// Storage backend failed during a mining run. Wraps the backend's error
    /// as a formatted string so the public type has no backend coupling.
    #[error("storage error: {0}")]
    Storage(String),
}

impl MiningConfig {
    /// Validate thresholds and disagreement bounds. Returns `Err` on any
    /// out-of-range or inconsistent value.
    pub fn validate(&self) -> Result<(), ContrastiveError> {
        if !self.min_disagreement.is_finite()
            || self.min_disagreement < 0.0
            || self.min_disagreement > 1.0
        {
            return Err(ContrastiveError::InvalidDisagreement(self.min_disagreement));
        }
        if !self.low_threshold.is_finite()
            || !self.high_threshold.is_finite()
            || self.low_threshold >= self.high_threshold
        {
            return Err(ContrastiveError::InvalidThresholds(
                self.low_threshold,
                self.high_threshold,
            ));
        }
        Ok(())
    }
}
