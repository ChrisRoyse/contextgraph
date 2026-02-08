//! Search options for teleological memory queries.
//!
//! # Search Strategies
//!
//! Three search strategies are available:
//!
//! - **E1Only** (default): Uses only E1 Semantic HNSW index. Fastest, backward compatible.
//! - **MultiSpace**: Weighted fusion of semantic embedders (E1, E5, E7, E8, E10, E11).
//!   Temporal embedders (E2-E4) are excluded from scoring per research findings.
//! - **Pipeline**: Full 3-stage retrieval: Recall → Score → Re-rank.
//!
//! # Fusion Strategies (ARCH-18)
//!
//! When using MultiSpace or Pipeline strategies, score fusion can use:
//!
//! - **WeightedSum** (legacy): Simple weighted sum of similarity scores
//! - **WeightedRRF** (default per ARCH-18): Weighted Reciprocal Rank Fusion
//!
//! RRF formula: `RRF_score(d) = Sum(weight_i / (rank_i + k))`
//!
//! RRF is more robust to score distribution differences between embedders.
//!
//! # Temporal Search Options
//!
//! Temporal embedders (E2-E4) are applied POST-retrieval per ARCH-14:
//!
//! - **E2 (V_freshness)**: Recency decay with configurable decay functions
//! - **E3 (V_periodicity)**: Periodic pattern matching (hour-of-day, day-of-week)
//! - **E4 (V_ordering)**: Sequence understanding (before/after relationships)
//!
//! Use `TemporalSearchOptions` to configure temporal boosts:
//! - Decay functions: linear, exponential, step, none
//! - Time windows: filter by absolute time ranges
//! - Session filtering: filter by session ID
//! - Periodic matching: boost memories from similar time patterns
//! - Sequence anchoring: find memories before/after a reference memory
//!
//! # Research References
//!
//! - [Cascading Retrieval](https://www.pinecone.io/blog/cascading-retrieval/) - 48% improvement
//! - [Fusion Analysis](https://dl.acm.org/doi/10.1145/3596512) - Convex combination beats RRF
//! - [Elastic Weighted RRF](https://www.elastic.co/blog/weighted-reciprocal-rank-fusion-rrf)
//! - [ColBERT Late Interaction](https://weaviate.io/blog/late-interaction-overview)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::causal::asymmetric::CausalDirection;
use crate::code::CodeQueryType;
use crate::fusion::FusionStrategy;
use crate::types::fingerprint::SemanticFingerprint;

/// Search strategy for semantic queries.
///
/// Controls how the 13-embedder multi-space index is used for ranking.
///
/// # Key Insight
///
/// Temporal embedders (E2-E4) measure TIME proximity, not TOPIC similarity.
/// They are excluded from similarity scoring and applied as post-retrieval boosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchStrategy {
    /// E1 HNSW only. Backward compatible, fastest.
    #[default]
    E1Only,

    /// Weighted fusion of semantic embedders (E1, E5, E7, E8, E10, E11).
    /// Temporal embedders (E2-E4) have weight 0.0 per AP-71.
    MultiSpace,

    /// Full 3-stage pipeline: Recall → Score → Re-rank.
    /// Stage 1: E13 SPLADE + E1 for broad recall.
    /// Stage 2: Multi-space scoring with semantic embedders.
    /// Stage 3: Optional E12 ColBERT re-ranking.
    Pipeline,
}

/// Intent direction for E10 asymmetric intent gate.
///
/// Controls how E10 similarity is computed during the intent gate stage.
/// Per the plan Phase 4: E10 intent gate between E1 scoring and E12 reranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntentDirection {
    /// Query is a goal/purpose seeking context.
    /// Uses query.e10_as_intent vs doc.e10_as_context with 1.2x boost.
    SeekingIntent,

    /// Query is a situation seeking intent/purpose.
    /// Uses query.e10_as_context vs doc.e10_as_intent with 0.8x dampening.
    SeekingContext,

    /// Auto-detect from query text.
    /// Falls back to symmetric similarity if detection uncertain.
    #[default]
    Auto,
}

impl IntentDirection {
    /// Get the direction modifier for E10 asymmetric similarity.
    ///
    /// - SeekingIntent: 1.2x (intent→context boost)
    /// - SeekingContext: 0.8x (context→intent dampening)
    /// - Auto: 1.0x (no modifier, use symmetric)
    pub fn modifier(&self) -> f32 {
        match self {
            IntentDirection::SeekingIntent => 1.2,
            IntentDirection::SeekingContext => 0.8,
            IntentDirection::Auto => 1.0,
        }
    }

    /// Check if this is an explicit direction (not Auto).
    pub fn is_explicit(&self) -> bool {
        !matches!(self, IntentDirection::Auto)
    }
}

/// Score normalization strategy for multi-space fusion.
///
/// Applied before combining scores from multiple embedders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationStrategyOption {
    /// No normalization - use raw similarity scores.
    None,

    /// Min-max normalization to [0, 1] range.
    #[default]
    MinMax,

    /// Z-score normalization (mean=0, std=1), scaled to [0, 1].
    ZScore,

    /// Convex combination (research-backed best practice).
    /// See: https://dl.acm.org/doi/10.1145/3596512
    Convex,
}

// =============================================================================
// TEMPORAL SEARCH OPTIONS (ARCH-14 Compliant)
// =============================================================================

/// Decay function for E2 recency scoring.
///
/// Controls how the recency score decreases with age.
/// Per ARCH-14: Temporal is POST-retrieval only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DecayFunction {
    /// Linear decay: score = 1.0 - (age / max_age)
    /// Simple and predictable, good for short time horizons.
    #[default]
    Linear,

    /// Exponential decay: score = exp(-age * 0.693 / half_life)
    /// Natural forgetting curve, good for long time horizons.
    Exponential,

    /// Step function decay with predefined time buckets.
    /// - < 5 min: 1.0 (Fresh)
    /// - < 1 hour: 0.8 (Recent)
    /// - < 1 day: 0.5 (Today)
    /// - >= 1 day: 0.1 (Older)
    Step,

    /// No decay - all memories have equal recency score.
    /// Use when recency is not a factor.
    NoDecay,
}

impl DecayFunction {
    /// Check if this decay function is active (not NoDecay).
    #[inline]
    pub fn is_active(&self) -> bool {
        !matches!(self, DecayFunction::NoDecay)
    }
}

/// Time window for filtering memories by timestamp.
///
/// Both bounds are optional - use None for open-ended ranges.
/// Timestamps are in milliseconds since Unix epoch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeWindow {
    /// Start of time window (inclusive), None for no lower bound.
    pub start_ms: Option<i64>,

    /// End of time window (exclusive), None for no upper bound.
    pub end_ms: Option<i64>,
}

impl TimeWindow {
    /// Create a time window from the last N hours.
    pub fn last_hours(hours: u64) -> Self {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = now_ms - (hours as i64 * 3600 * 1000);
        Self {
            start_ms: Some(start_ms),
            end_ms: None,
        }
    }

    /// Create a time window from the last N days.
    pub fn last_days(days: u64) -> Self {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = now_ms - (days as i64 * 24 * 3600 * 1000);
        Self {
            start_ms: Some(start_ms),
            end_ms: None,
        }
    }

    /// Create a time window for a specific day.
    pub fn for_date(year: i32, month: u32, day: u32) -> Self {
        use chrono::{TimeZone, Utc};
        if let Some(date) = Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).single() {
            let start_ms = date.timestamp_millis();
            let end_ms = start_ms + 24 * 3600 * 1000;
            Self {
                start_ms: Some(start_ms),
                end_ms: Some(end_ms),
            }
        } else {
            Self::default()
        }
    }

    /// Check if a timestamp falls within this window.
    pub fn contains(&self, timestamp_ms: i64) -> bool {
        if let Some(start) = self.start_ms {
            if timestamp_ms < start {
                return false;
            }
        }
        if let Some(end) = self.end_ms {
            if timestamp_ms >= end {
                return false;
            }
        }
        true
    }

    /// Check if this window is defined (has at least one bound).
    pub fn is_defined(&self) -> bool {
        self.start_ms.is_some() || self.end_ms.is_some()
    }
}

/// Options for E3 periodic pattern matching.
///
/// Boosts memories that match the target time pattern (hour-of-day, day-of-week).
/// Useful for finding memories from similar work patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicOptions {
    /// Target hour of day (0-23) for matching.
    /// None means no hour matching.
    pub target_hour: Option<u8>,

    /// Target day of week (0=Sunday, 6=Saturday) for matching.
    /// None means no day matching.
    pub target_day_of_week: Option<u8>,

    /// Auto-detect target from current time.
    /// When true, target_hour and target_day_of_week are computed from now.
    #[serde(default)]
    pub auto_detect: bool,

    /// Weight for periodic boost [0.0, 1.0].
    /// Higher values give more weight to periodic matches.
    #[serde(default = "PeriodicOptions::default_weight")]
    pub weight: f32,
}

impl PeriodicOptions {
    fn default_weight() -> f32 {
        0.3
    }

    /// Create periodic options that match the current time pattern.
    pub fn current_time() -> Self {
        Self {
            target_hour: None,
            target_day_of_week: None,
            auto_detect: true,
            weight: Self::default_weight(),
        }
    }

    /// Create periodic options for a specific hour.
    pub fn for_hour(hour: u8) -> Self {
        Self {
            target_hour: Some(hour.min(23)),
            target_day_of_week: None,
            auto_detect: false,
            weight: Self::default_weight(),
        }
    }

    /// Create periodic options for a specific day of week.
    pub fn for_day(day_of_week: u8) -> Self {
        Self {
            target_hour: None,
            target_day_of_week: Some(day_of_week.min(6)),
            auto_detect: false,
            weight: Self::default_weight(),
        }
    }

    /// Create periodic options for a specific hour with validation.
    ///
    /// Returns an error if hour is not in range 0-23.
    ///
    /// # Arguments
    ///
    /// * `hour` - Hour of day (0-23)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let opts = PeriodicOptions::try_for_hour(14)?; // 2 PM
    /// assert!(PeriodicOptions::try_for_hour(24).is_err());
    /// ```
    pub fn try_for_hour(hour: u8) -> Result<Self, &'static str> {
        if hour > 23 {
            return Err("Hour must be 0-23");
        }
        Ok(Self::for_hour(hour))
    }

    /// Create periodic options for a specific day of week with validation.
    ///
    /// Returns an error if day is not in range 0-6 (Sun-Sat).
    ///
    /// # Arguments
    ///
    /// * `day` - Day of week (0=Sunday, 6=Saturday)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let opts = PeriodicOptions::try_for_day(3)?; // Wednesday
    /// assert!(PeriodicOptions::try_for_day(7).is_err());
    /// ```
    pub fn try_for_day(day: u8) -> Result<Self, &'static str> {
        if day > 6 {
            return Err("Day must be 0-6 (Sun-Sat)");
        }
        Ok(Self::for_day(day))
    }

    /// Alias for try_for_day for clarity.
    #[inline]
    pub fn try_for_day_of_week(day_of_week: u8) -> Result<Self, &'static str> {
        Self::try_for_day(day_of_week)
    }

    /// Set the periodic boost weight.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Get effective target hour (auto-detect if needed).
    pub fn effective_hour(&self) -> Option<u8> {
        if self.auto_detect {
            Some(chrono::Utc::now().format("%H").to_string().parse().unwrap_or(12))
        } else {
            self.target_hour
        }
    }

    /// Get effective target day of week (auto-detect if needed).
    pub fn effective_day_of_week(&self) -> Option<u8> {
        if self.auto_detect {
            // chrono weekday: Mon=0, Sun=6, but we use Sun=0, Sat=6
            let weekday = chrono::Utc::now().format("%w").to_string().parse().unwrap_or(0);
            Some(weekday)
        } else {
            self.target_day_of_week
        }
    }
}

impl Default for PeriodicOptions {
    fn default() -> Self {
        Self {
            target_hour: None,
            target_day_of_week: None,
            auto_detect: false,
            weight: Self::default_weight(),
        }
    }
}

/// Direction for sequence-based retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SequenceDirection {
    /// Find memories that occurred BEFORE the anchor.
    Before,

    /// Find memories that occurred AFTER the anchor.
    After,

    /// Find memories both before and after the anchor.
    #[default]
    Both,
}

/// Mode for combining scores from multiple anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MultiAnchorMode {
    /// Use minimum score across all anchors (most restrictive).
    Min,

    /// Use maximum score across all anchors (most permissive).
    Max,

    /// Use average score across all anchors (balanced).
    #[default]
    Average,
}

/// Options for E4 sequence-based retrieval.
///
/// Finds memories that are temporally related to an anchor memory.
/// Useful for reconstructing conversation flow or finding context.
///
/// Supports multi-anchor queries for finding memories "between" two points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceOptions {
    /// UUID of the anchor memory to find before/after.
    /// Required for sequence-based retrieval.
    pub anchor_id: Uuid,

    /// Additional anchors for "between" queries.
    ///
    /// When set, finds memories that are positioned between the primary anchor
    /// and these additional anchors. Useful for finding "what happened between
    /// event A and event B".
    #[serde(default)]
    pub additional_anchors: Vec<Uuid>,

    /// How to combine multi-anchor scores.
    ///
    /// - `Min`: Must be close to ALL anchors (most restrictive)
    /// - `Max`: Must be close to ANY anchor (most permissive)
    /// - `Average`: Balanced combination (default)
    #[serde(default)]
    pub multi_anchor_mode: MultiAnchorMode,

    /// Direction to search (before, after, or both).
    #[serde(default)]
    pub direction: SequenceDirection,

    /// Maximum distance in sequence positions.
    /// Default: 10 (find up to 10 memories before/after).
    #[serde(default = "SequenceOptions::default_max_distance")]
    pub max_distance: usize,

    /// Weight for sequence boost [0.0, 1.0].
    /// Higher values give more weight to sequence proximity.
    #[serde(default = "SequenceOptions::default_weight")]
    pub weight: f32,

    /// Use exponential decay for fallback (when E4 embeddings unavailable).
    ///
    /// When true, uses exponential decay that better matches E4's learned
    /// distance semantics. When false, uses linear decay.
    ///
    /// Default: true (exponential is recommended)
    #[serde(default = "SequenceOptions::default_use_exponential")]
    pub use_exponential_fallback: bool,

    /// Anchor's session_sequence (populated during search setup).
    ///
    /// When set, enables sequence-based direction filtering using session
    /// sequence numbers instead of timestamps. This provides more accurate
    /// "before/after" queries within a session.
    #[serde(skip)]
    pub anchor_sequence: Option<u64>,

    /// Anchor's session_id (for same-session validation).
    ///
    /// When set, can be used to ensure sequence comparisons only happen
    /// between memories from the same session.
    #[serde(skip)]
    pub anchor_session_id: Option<String>,
}

impl SequenceOptions {
    fn default_max_distance() -> usize {
        10
    }

    fn default_weight() -> f32 {
        0.3
    }

    fn default_use_exponential() -> bool {
        true // Exponential decay better matches E4's learned semantics
    }

    /// Create sequence options to find memories before an anchor.
    pub fn before(anchor_id: Uuid) -> Self {
        Self {
            anchor_id,
            additional_anchors: Vec::new(),
            multi_anchor_mode: MultiAnchorMode::default(),
            direction: SequenceDirection::Before,
            max_distance: Self::default_max_distance(),
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: None,
            anchor_session_id: None,
        }
    }

    /// Create sequence options to find memories after an anchor.
    pub fn after(anchor_id: Uuid) -> Self {
        Self {
            anchor_id,
            additional_anchors: Vec::new(),
            multi_anchor_mode: MultiAnchorMode::default(),
            direction: SequenceDirection::After,
            max_distance: Self::default_max_distance(),
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: None,
            anchor_session_id: None,
        }
    }

    /// Create sequence options to find memories around an anchor.
    pub fn around(anchor_id: Uuid) -> Self {
        Self {
            anchor_id,
            additional_anchors: Vec::new(),
            multi_anchor_mode: MultiAnchorMode::default(),
            direction: SequenceDirection::Both,
            max_distance: Self::default_max_distance(),
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: None,
            anchor_session_id: None,
        }
    }

    /// Create sequence options to anchor by sequence number directly.
    ///
    /// This is useful for anchoring to the current conversation turn without
    /// having a specific memory UUID. Uses Uuid::nil() as a sentinel.
    ///
    /// # Arguments
    /// * `sequence` - The session sequence number to anchor to
    /// * `direction` - Direction to search (before, after, both)
    /// * `max_dist` - Maximum distance in sequence positions
    pub fn from_sequence(sequence: u64, direction: SequenceDirection, max_dist: u32) -> Self {
        Self {
            anchor_id: Uuid::nil(), // Sentinel value - sequence takes priority
            additional_anchors: Vec::new(),
            multi_anchor_mode: MultiAnchorMode::default(),
            direction,
            max_distance: max_dist as usize,
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: Some(sequence),
            anchor_session_id: None,
        }
    }

    /// Check if this sequence options is anchored by sequence number.
    ///
    /// Returns true if anchor_sequence is set (regardless of anchor_id).
    pub fn is_sequence_anchored(&self) -> bool {
        self.anchor_sequence.is_some()
    }

    /// Create sequence options to find memories between two anchors.
    ///
    /// This finds memories that are temporally positioned between the two
    /// anchor points, useful for "what happened between A and B" queries.
    pub fn between(anchor1: Uuid, anchor2: Uuid) -> Self {
        Self {
            anchor_id: anchor1,
            additional_anchors: vec![anchor2],
            multi_anchor_mode: MultiAnchorMode::Average,
            direction: SequenceDirection::Both,
            max_distance: Self::default_max_distance(),
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: None,
            anchor_session_id: None,
        }
    }

    /// Set the maximum distance in sequence positions.
    pub fn with_max_distance(mut self, max_distance: usize) -> Self {
        self.max_distance = max_distance;
        self
    }

    /// Set the sequence boost weight.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Use exponential decay for fallback (when E4 embeddings unavailable).
    pub fn with_exponential_fallback(mut self, use_exponential: bool) -> Self {
        self.use_exponential_fallback = use_exponential;
        self
    }

    /// Add additional anchor(s) for multi-anchor queries.
    ///
    /// Use this for "between" queries that find memories positioned
    /// relative to multiple anchors.
    pub fn with_additional_anchors(mut self, anchors: Vec<Uuid>) -> Self {
        self.additional_anchors = anchors;
        self
    }

    /// Add a single additional anchor.
    pub fn with_additional_anchor(mut self, anchor: Uuid) -> Self {
        self.additional_anchors.push(anchor);
        self
    }

    /// Set the mode for combining multi-anchor scores.
    pub fn with_multi_anchor_mode(mut self, mode: MultiAnchorMode) -> Self {
        self.multi_anchor_mode = mode;
        self
    }

    /// Set the anchor's session sequence number.
    ///
    /// When set, enables sequence-based direction filtering using session
    /// sequence numbers instead of timestamps.
    pub fn with_anchor_sequence(mut self, sequence: u64) -> Self {
        self.anchor_sequence = Some(sequence);
        self
    }

    /// Set the anchor's session ID.
    ///
    /// When set, can be used to ensure sequence comparisons only happen
    /// between memories from the same session.
    pub fn with_anchor_session_id(mut self, session_id: String) -> Self {
        self.anchor_session_id = Some(session_id);
        self
    }

    /// Set the sequence direction (before, after, both).
    pub fn with_direction(mut self, direction: SequenceDirection) -> Self {
        self.direction = direction;
        self
    }
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            anchor_id: Uuid::nil(),
            additional_anchors: Vec::new(),
            multi_anchor_mode: MultiAnchorMode::default(),
            direction: SequenceDirection::Both,
            max_distance: Self::default_max_distance(),
            weight: Self::default_weight(),
            use_exponential_fallback: Self::default_use_exponential(),
            anchor_sequence: None,
            anchor_session_id: None,
        }
    }
}

/// Options for chain-aware retrieval mode.
///
/// Enables retrieval of all memories belonging to a specific conversation/task chain,
/// with optional expansion to related chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainRetrievalOptions {
    /// Chain ID to retrieve all members of.
    pub chain_id: String,

    /// Include memories from related chains (determined by shared topics or sequence).
    #[serde(default)]
    pub include_related: bool,

    /// Maximum depth when traversing related chains.
    /// Only used when include_related=true.
    #[serde(default = "ChainRetrievalOptions::default_max_depth")]
    pub max_related_depth: usize,
}

impl ChainRetrievalOptions {
    fn default_max_depth() -> usize {
        1 // Only immediate neighbors by default
    }

    /// Create options for a specific chain.
    pub fn for_chain(chain_id: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
            include_related: false,
            max_related_depth: Self::default_max_depth(),
        }
    }

    /// Include related chains in retrieval.
    pub fn with_related(mut self) -> Self {
        self.include_related = true;
        self
    }

    /// Set the maximum depth for related chain traversal.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_related_depth = depth;
        self
    }
}

impl Default for ChainRetrievalOptions {
    fn default() -> Self {
        Self {
            chain_id: String::new(),
            include_related: false,
            max_related_depth: Self::default_max_depth(),
        }
    }
}

/// Temporal scale for multi-scale temporal reasoning.
///
/// Allows queries to target specific temporal granularities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TemporalScale {
    /// Micro scale: seconds to minutes (recent conversation turns).
    Micro,

    /// Meso scale: hours to a day (current session).
    #[default]
    Meso,

    /// Macro scale: days to a week (recent work).
    Macro,

    /// Long scale: weeks to months (project history).
    Long,

    /// Archival scale: months to years (long-term knowledge).
    Archival,
}

impl TemporalScale {
    /// Get the typical time horizon in seconds for this scale.
    pub fn horizon_seconds(&self) -> u64 {
        match self {
            TemporalScale::Micro => 300,          // 5 minutes
            TemporalScale::Meso => 3600,          // 1 hour
            TemporalScale::Macro => 86400 * 7,    // 1 week
            TemporalScale::Long => 86400 * 30,    // 1 month
            TemporalScale::Archival => 86400 * 365, // 1 year
        }
    }

    /// Get the decay half-life in seconds appropriate for this scale.
    pub fn decay_half_life(&self) -> u64 {
        match self {
            TemporalScale::Micro => 60,           // 1 minute
            TemporalScale::Meso => 1800,          // 30 minutes
            TemporalScale::Macro => 43200,        // 12 hours
            TemporalScale::Long => 86400 * 3,     // 3 days
            TemporalScale::Archival => 86400 * 30, // 30 days
        }
    }
}

/// Default step buckets for the Step decay function.
///
/// Format: Vec<(max_age_secs, score)> where items with age <= max_age_secs get the score.
/// Default buckets:
/// - < 5 min: 1.0 (Fresh)
/// - < 1 hour: 0.8 (Recent)
/// - < 1 day: 0.5 (Today)
/// - < 1 week: 0.3 (This week)
/// - Older: 0.1
pub fn default_step_buckets() -> Vec<(u64, f32)> {
    vec![
        (300, 1.0),      // < 5 min: Fresh
        (3600, 0.8),     // < 1 hour: Recent
        (86400, 0.5),    // < 1 day: Today
        (604800, 0.3),   // < 1 week: This week
    ]
}

/// Default component weights (E2 recency, E3 periodic, E4 sequence).
///
/// Optimized from benchmark analysis:
/// - E2 recency: 50% (strongest signal)
/// - E3 periodic: 15% (weakest signal)
/// - E4 sequence: 35% (moderate signal)
///
/// Previous weights (40/30/30) caused negative interference.
pub fn default_component_weights() -> (f32, f32, f32) {
    (0.50, 0.15, 0.35)
}

/// Temporal search options for time-aware retrieval.
///
/// Controls how temporal embedders (E2-E4) are applied POST-retrieval
/// per ARCH-14: "Recency boost is applied POST-retrieval".
///
/// # Example
///
/// ```ignore
/// use context_graph_core::traits::{TemporalSearchOptions, DecayFunction, TimeWindow};
///
/// // Search with exponential decay and session filtering
/// let temporal = TemporalSearchOptions::default()
///     .with_decay_function(DecayFunction::Exponential)
///     .with_session_id("session-123")
///     .with_time_window(TimeWindow::last_hours(24));
///
/// let options = TeleologicalSearchOptions::quick(10)
///     .with_temporal_options(temporal);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSearchOptions {
    // =========================================================================
    // E2 Recency Options
    // =========================================================================

    /// Decay function for recency scoring.
    /// Default: Linear (simple and predictable).
    #[serde(default)]
    pub decay_function: DecayFunction,

    /// Half-life in seconds for exponential decay.
    /// Default: 86400 (1 day).
    /// Only used when decay_function is Exponential.
    #[serde(default = "TemporalSearchOptions::default_decay_half_life")]
    pub decay_half_life_secs: u64,

    /// Custom step function buckets: Vec<(max_age_secs, score)>.
    /// Only used when decay_function is Step.
    /// Default: [(300, 1.0), (3600, 0.8), (86400, 0.5), (604800, 0.3)]
    #[serde(default = "default_step_buckets")]
    pub step_buckets: Vec<(u64, f32)>,

    /// Component weights for combining E2/E3/E4 scores.
    /// Format: (e2_recency, e3_periodic, e4_sequence).
    /// Default: (0.50, 0.15, 0.35) based on benchmark optimization.
    #[serde(default = "default_component_weights")]
    pub component_weights: (f32, f32, f32),

    /// Time window for filtering memories.
    /// Memories outside this window are excluded from results.
    #[serde(default)]
    pub time_window: Option<TimeWindow>,

    /// Session ID for filtering memories to a specific session.
    /// Memories with different session IDs are excluded.
    #[serde(default)]
    pub session_id: Option<String>,

    // =========================================================================
    // E3 Periodic Options
    // =========================================================================

    /// Periodic pattern matching options.
    /// When set, boosts memories from similar time patterns.
    #[serde(default)]
    pub periodic_options: Option<PeriodicOptions>,

    // =========================================================================
    // E4 Sequence Options
    // =========================================================================

    /// Sequence-based retrieval options.
    /// When set, finds memories temporally related to an anchor.
    #[serde(default)]
    pub sequence_options: Option<SequenceOptions>,

    // =========================================================================
    // Chain-Aware Retrieval Options
    // =========================================================================

    /// Chain retrieval options.
    /// When set, retrieves all memories in a specific conversation/task chain.
    #[serde(default)]
    pub chain_options: Option<ChainRetrievalOptions>,

    // =========================================================================
    // Multi-Scale Options
    // =========================================================================

    /// Temporal scale for multi-scale reasoning.
    /// Affects decay half-life and time horizon automatically.
    #[serde(default)]
    pub temporal_scale: TemporalScale,

    // =========================================================================
    // Master Boost Weight
    // =========================================================================

    /// Master weight for all temporal boosts [0.0, 1.0].
    /// Applied as: final = semantic * (1.0 - weight) + temporal_combined * weight.
    /// Default: 0.0 (no temporal boost, pure semantic).
    #[serde(default)]
    pub temporal_weight: f32,
}

impl TemporalSearchOptions {
    fn default_decay_half_life() -> u64 {
        86400 // 1 day
    }

    /// Check if any temporal boost is active.
    ///
    /// Returns true if:
    /// - temporal_weight > 0, AND
    /// - At least one temporal feature is enabled (decay, periodic, sequence, or time window)
    pub fn has_any_boost(&self) -> bool {
        if self.temporal_weight <= 0.0 {
            return false;
        }

        self.decay_function.is_active()
            || self.periodic_options.is_some()
            || self.sequence_options.is_some()
            || self.time_window.as_ref().map_or(false, |w| w.is_defined())
            || self.session_id.is_some()
    }

    /// Check if time window filtering is active.
    pub fn has_time_filter(&self) -> bool {
        self.time_window.as_ref().map_or(false, |w| w.is_defined())
            || self.session_id.is_some()
    }

    /// Get effective decay half-life based on temporal scale.
    pub fn effective_half_life(&self) -> u64 {
        if self.decay_half_life_secs != Self::default_decay_half_life() {
            // User specified explicit half-life
            self.decay_half_life_secs
        } else {
            // Use scale-appropriate half-life
            self.temporal_scale.decay_half_life()
        }
    }

    /// Get adaptive decay half-life based on corpus size.
    ///
    /// E2 decay accuracy degrades at larger corpus sizes with fixed half-life.
    /// This method scales the half-life with corpus size to maintain accuracy.
    ///
    /// Uses log-based formula for gentler adjustment than sqrt:
    /// - At 100 memories: multiplier ≈ 0.9 (-10%)
    /// - At 1,000 memories: multiplier = 1.0 (no change, calibration point)
    /// - At 10,000 memories: multiplier ≈ 1.1 (+10%)
    /// - At 100,000 memories: multiplier ≈ 1.2 (+20%)
    ///
    /// Formula: base_half_life * (1 + 0.1 * log10(corpus_size / 1000))
    ///
    /// # Arguments
    ///
    /// * `corpus_size` - Number of memories in the corpus
    ///
    /// # Returns
    ///
    /// Adaptive half-life in seconds (minimum 60s)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let options = TemporalSearchOptions::default();
    /// // At 100 memories: ~0.9x base half-life
    /// let hl_100 = options.adaptive_half_life(100);
    /// // At 1K memories: 1.0x base half-life (calibration point)
    /// let hl_1k = options.adaptive_half_life(1000);
    /// assert!((hl_1k as f64 - options.effective_half_life() as f64).abs() < 1.0);
    /// // At 10K memories: ~1.1x base half-life
    /// let hl_10k = options.adaptive_half_life(10000);
    /// assert!(hl_10k > hl_1k);
    /// ```
    pub fn adaptive_half_life(&self, corpus_size: usize) -> u64 {
        let base = self.effective_half_life() as f64;

        // Log-based formula: base * (1 + 0.1 * log10(corpus_size / 1000))
        // Calibrated at 1K memories (multiplier = 1.0)
        let ratio = (corpus_size as f64 / 1000.0).max(0.1);
        let multiplier = (1.0 + 0.1 * ratio.log10()).clamp(0.8, 2.0);

        (base * multiplier).max(60.0) as u64
    }

    // =========================================================================
    // Builder Methods
    // =========================================================================

    /// Set the decay function for E2 recency scoring.
    pub fn with_decay_function(mut self, decay: DecayFunction) -> Self {
        self.decay_function = decay;
        self
    }

    /// Set the decay half-life in seconds.
    pub fn with_decay_half_life(mut self, secs: u64) -> Self {
        self.decay_half_life_secs = secs;
        self
    }

    /// Set the time window for filtering.
    pub fn with_time_window(mut self, window: TimeWindow) -> Self {
        self.time_window = Some(window);
        self
    }

    /// Filter to the last N hours.
    pub fn with_last_hours(mut self, hours: u64) -> Self {
        self.time_window = Some(TimeWindow::last_hours(hours));
        self
    }

    /// Filter to the last N days.
    pub fn with_last_days(mut self, days: u64) -> Self {
        self.time_window = Some(TimeWindow::last_days(days));
        self
    }

    /// Filter to a specific session.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set periodic pattern matching options.
    pub fn with_periodic_options(mut self, options: PeriodicOptions) -> Self {
        self.periodic_options = Some(options);
        self
    }

    /// Enable periodic matching for current time.
    pub fn with_current_time_periodic(mut self) -> Self {
        self.periodic_options = Some(PeriodicOptions::current_time());
        self
    }

    /// Set sequence-based retrieval options.
    pub fn with_sequence_options(mut self, options: SequenceOptions) -> Self {
        self.sequence_options = Some(options);
        self
    }

    /// Find memories before a specific anchor.
    pub fn with_sequence_before(mut self, anchor_id: Uuid) -> Self {
        self.sequence_options = Some(SequenceOptions::before(anchor_id));
        self
    }

    /// Find memories after a specific anchor.
    pub fn with_sequence_after(mut self, anchor_id: Uuid) -> Self {
        self.sequence_options = Some(SequenceOptions::after(anchor_id));
        self
    }

    /// Find memories around a specific anchor.
    pub fn with_sequence_around(mut self, anchor_id: Uuid) -> Self {
        self.sequence_options = Some(SequenceOptions::around(anchor_id));
        self
    }

    /// Retrieve all memories in a specific chain.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID to retrieve
    ///
    /// # Example
    ///
    /// ```ignore
    /// let options = TemporalSearchOptions::default()
    ///     .with_chain("session-123")
    ///     .with_temporal_weight(0.5);
    /// ```
    pub fn with_chain(mut self, chain_id: impl Into<String>) -> Self {
        self.chain_options = Some(ChainRetrievalOptions::for_chain(chain_id));
        self
    }

    /// Retrieve all memories in a chain and related chains.
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID to retrieve
    pub fn with_chain_and_related(mut self, chain_id: impl Into<String>) -> Self {
        self.chain_options = Some(ChainRetrievalOptions::for_chain(chain_id).with_related());
        self
    }

    /// Set the temporal scale for multi-scale reasoning.
    pub fn with_temporal_scale(mut self, scale: TemporalScale) -> Self {
        self.temporal_scale = scale;
        self
    }

    /// Set the master temporal weight.
    pub fn with_temporal_weight(mut self, weight: f32) -> Self {
        self.temporal_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Set custom step function buckets for the Step decay function.
    ///
    /// # Arguments
    ///
    /// * `buckets` - Vec of (max_age_secs, score) pairs.
    ///   Items with age <= max_age_secs get the corresponding score.
    ///   Items older than all buckets get 0.1.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let options = TemporalSearchOptions::default()
    ///     .with_decay_function(DecayFunction::Step)
    ///     .with_step_buckets(vec![
    ///         (60, 1.0),    // < 1 min: highest
    ///         (300, 0.8),   // < 5 min: high
    ///         (3600, 0.5),  // < 1 hour: medium
    ///     ]);
    /// ```
    pub fn with_step_buckets(mut self, buckets: Vec<(u64, f32)>) -> Self {
        self.step_buckets = buckets;
        self
    }

    /// Set component weights for combining E2/E3/E4 scores.
    ///
    /// # Arguments
    ///
    /// * `e2_recency` - Weight for E2 recency score [0.0, 1.0]
    /// * `e3_periodic` - Weight for E3 periodic score [0.0, 1.0]
    /// * `e4_sequence` - Weight for E4 sequence score [0.0, 1.0]
    ///
    /// Note: Weights are normalized internally, so they don't need to sum to 1.0.
    pub fn with_component_weights(mut self, e2_recency: f32, e3_periodic: f32, e4_sequence: f32) -> Self {
        self.component_weights = (
            e2_recency.max(0.0),
            e3_periodic.max(0.0),
            e4_sequence.max(0.0),
        );
        self
    }
}

impl Default for TemporalSearchOptions {
    fn default() -> Self {
        Self {
            decay_function: DecayFunction::default(),
            decay_half_life_secs: Self::default_decay_half_life(),
            step_buckets: default_step_buckets(),
            component_weights: default_component_weights(),
            time_window: None,
            session_id: None,
            periodic_options: None,
            sequence_options: None,
            chain_options: None,
            temporal_scale: TemporalScale::default(),
            temporal_weight: 0.0, // No temporal boost by default
        }
    }
}

/// Search options for teleological memory queries.
///
/// Controls filtering, pagination, and result formatting for
/// semantic and purpose-based searches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalSearchOptions {
    /// Maximum number of results to return.
    /// Default: 10, Max: 1000
    pub top_k: usize,

    /// Minimum similarity threshold [0.0, 1.0].
    /// Results below this threshold are filtered out.
    /// Default: 0.0 (no filtering)
    pub min_similarity: f32,

    /// Include soft-deleted items in results.
    /// Default: false
    pub include_deleted: bool,

    /// Embedder indices to use for search (0-12).
    /// Empty = use all embedders.
    pub embedder_indices: Vec<usize>,

    /// Optional semantic fingerprint for computing per-embedder scores.
    /// When provided, enables computation of actual cosine similarity scores
    /// for each embedder instead of returning zeros.
    #[serde(skip)]
    pub semantic_query: Option<SemanticFingerprint>,

    /// Whether to include original content text in search results.
    ///
    /// When `true`, the `content` field of `TeleologicalSearchResult` will be
    /// populated with the original text (if available). When `false` (default),
    /// the `content` field will be `None` for better performance.
    ///
    /// Default: `false` (opt-in for performance reasons)
    ///
    /// TASK-CONTENT-005: Added for content hydration in search results.
    #[serde(default)]
    pub include_content: bool,

    // =========================================================================
    // Multi-Space Search Options (TASK-MULTISPACE)
    // =========================================================================

    /// Search strategy: E1Only (default), MultiSpace, or Pipeline.
    ///
    /// - `E1Only`: Backward compatible, uses only E1 Semantic HNSW.
    /// - `MultiSpace`: Weighted fusion of semantic embedders.
    /// - `Pipeline`: Full 3-stage retrieval with optional re-ranking.
    ///
    /// Default: `E1Only` for backward compatibility.
    #[serde(default)]
    pub strategy: SearchStrategy,

    /// Weight profile name for multi-space scoring.
    ///
    /// Available profiles:
    /// - `"semantic_search"`: General queries (E1: 35%, E7: 20%, E5/E10: 15%)
    /// - `"code_search"`: Programming queries (E7: 40%, E1: 20%)
    /// - `"causal_reasoning"`: "Why" questions (E5: 45%, E1: 20%)
    /// - `"fact_checking"`: Entity/fact queries (E11: 40%, E6: 15%)
    /// - `"category_weighted"`: Constitution-compliant category weights
    ///
    /// All profiles have E2-E4 (temporal) = 0.0 per research findings.
    /// Default: `"semantic_search"`.
    #[serde(default)]
    pub weight_profile: Option<String>,

    /// Custom per-embedder weights `[f32; 13]` for multi-space search.
    ///
    /// When provided, overrides `weight_profile`. Must pass `validate_weights()`:
    /// - Each weight in `[0.0, 1.0]`
    /// - Sum must be ~1.0 (tolerance 0.01)
    ///
    /// Index mapping: `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]`
    #[serde(default)]
    pub custom_weights: Option<[f32; 13]>,

    /// Embedder indices to exclude from multi-space fusion.
    ///
    /// When non-empty, the specified embedder weights are zeroed out and
    /// the remaining weights are renormalized to sum to 1.0.
    /// Index mapping: 0=E1, 1=E2, ..., 12=E13
    #[serde(default)]
    pub exclude_embedders: Vec<usize>,

    /// **DEPRECATED**: Use `temporal_options.temporal_weight` instead.
    ///
    /// Legacy recency boost factor [0.0, 1.0].
    ///
    /// Applied POST-retrieval as: `final = semantic * (1.0 - boost) + temporal * boost`.
    /// Uses E2 temporal embedding similarity for recency scoring.
    ///
    /// - `0.0`: No recency boost (default)
    /// - `0.5`: Balance semantic and recency
    /// - `1.0`: Strong recency preference
    ///
    /// Per ARCH-14: Temporal is a POST-retrieval boost, not similarity.
    ///
    /// # Migration
    ///
    /// ```ignore
    /// // Old:
    /// let options = TeleologicalSearchOptions::quick(10).with_recency_boost(0.3);
    ///
    /// // New:
    /// let options = TeleologicalSearchOptions::quick(10)
    ///     .with_temporal_options(TemporalSearchOptions::default()
    ///         .with_temporal_weight(0.3)
    ///         .with_decay_function(DecayFunction::Exponential));
    /// ```
    #[deprecated(since = "6.1.0", note = "Use temporal_options.temporal_weight instead")]
    #[serde(default)]
    pub recency_boost: f32,

    /// Enable E12 ColBERT re-ranking (Stage 3 in Pipeline strategy).
    ///
    /// More accurate but slower. Per AP-73: ColBERT is for re-ranking only.
    /// Default: `false`.
    #[serde(default)]
    pub enable_rerank: bool,

    /// E12 rerank weight for blending with fusion score.
    ///
    /// Controls how much weight E12 MaxSim scores have in the final ranking.
    /// Formula: `final_score = fusion_score * (1 - weight) + maxsim_score * weight`
    ///
    /// - 0.0: Pure fusion score (E12 has no effect)
    /// - 0.4: Default - 60% fusion + 40% E12 MaxSim
    /// - 1.0: Pure E12 MaxSim score (ignores fusion)
    ///
    /// Only used when `enable_rerank = true`.
    /// Default: `0.4` (40% E12 weight, per Phase 3 E12/E13 integration).
    #[serde(default = "TeleologicalSearchOptions::default_rerank_weight")]
    pub rerank_weight: f32,

    /// Normalization strategy for score fusion.
    ///
    /// Applied before combining scores from multiple embedders.
    /// Default: `MinMax`.
    #[serde(default)]
    pub normalization: NormalizationStrategyOption,

    /// Fusion strategy for combining multi-embedder results (ARCH-18).
    ///
    /// - `WeightedSum`: Legacy weighted sum of similarity scores
    /// - `WeightedRRF`: Weighted Reciprocal Rank Fusion (default per ARCH-18)
    ///
    /// RRF formula: `RRF_score(d) = Sum(weight_i / (rank_i + k))`
    ///
    /// RRF is recommended because it:
    /// - Preserves individual embedder rankings
    /// - Is robust to score distribution differences between embedders
    /// - Works well with varying numbers of results per embedder
    ///
    /// Default: `WeightedRRF` per ARCH-18.
    #[serde(default)]
    pub fusion_strategy: FusionStrategy,

    // =========================================================================
    // Code Query Type Detection (ARCH-16)
    // =========================================================================

    /// Original query text for code query type detection.
    ///
    /// When provided, enables E7 Code embedder similarity adjustment
    /// based on detected query type (Code2Code, Text2Code, NonCode).
    ///
    /// Per ARCH-16: E7 Code MUST detect query type and use appropriate
    /// similarity computation.
    #[serde(default)]
    pub query_text: Option<String>,

    /// Pre-computed code query type.
    ///
    /// If `None` and `query_text` is provided, the type will be
    /// auto-detected. If explicitly set, skips auto-detection.
    ///
    /// - `Code2Code`: Query is actual code syntax (e.g., "fn process<T>")
    /// - `Text2Code`: Query is natural language about code (e.g., "batch function")
    /// - `NonCode`: Query is not code-related
    #[serde(default)]
    pub code_query_type: Option<CodeQueryType>,

    // =========================================================================
    // Temporal Search Options (ARCH-14)
    // =========================================================================

    /// Temporal search options for time-aware retrieval.
    ///
    /// Controls how temporal embedders (E2-E4) are applied POST-retrieval.
    /// Per ARCH-14: Temporal is a POST-retrieval boost, not similarity.
    ///
    /// Features:
    /// - E2 Recency: Decay functions, time windows, session filtering
    /// - E3 Periodic: Hour-of-day, day-of-week pattern matching
    /// - E4 Sequence: Before/after anchor memory retrieval
    ///
    /// When `temporal_options.temporal_weight > 0`, these boosts are applied
    /// after semantic retrieval to reorder results by temporal relevance.
    ///
    /// Default: No temporal boost (pure semantic search).
    #[serde(default)]
    pub temporal_options: TemporalSearchOptions,

    // =========================================================================
    // Causal Search Options (ARCH-15)
    // =========================================================================

    /// Causal direction for asymmetric E5 retrieval.
    ///
    /// When set to `Cause` or `Effect`, enables direction-aware E5 similarity
    /// computation during multi-space retrieval (not just post-retrieval reranking).
    ///
    /// Per ARCH-15 and AP-77:
    /// - `Cause`: Query seeks causes (use query.e5_as_cause vs doc.e5_as_effect)
    /// - `Effect`: Query seeks effects (use query.e5_as_effect vs doc.e5_as_cause)
    /// - `Unknown`: Use symmetric E5 similarity (default, backward compatible)
    ///
    /// Direction modifiers are applied:
    /// - cause→effect: 1.2x boost
    /// - effect→cause: 0.8x dampening
    /// - same direction: 1.0x (no change)
    ///
    /// Default: `Unknown` (symmetric similarity for backward compatibility)
    #[serde(default)]
    pub causal_direction: CausalDirection,

    // =========================================================================
    // E10 Intent Gate Options (Phase 4)
    // =========================================================================

    /// Enable E10 intent gate in pipeline strategy.
    ///
    /// When enabled with `SearchStrategy::Pipeline`, adds an E10 intent filtering
    /// stage between E1 dense scoring and E12 ColBERT reranking:
    ///
    /// Pipeline stages: E13 SPLADE (recall) → E1 Dense (score) → **E10 Intent Gate** → E12 Rerank
    ///
    /// The intent gate filters candidates based on E10 asymmetric similarity,
    /// keeping only those that match the query's intent direction.
    ///
    /// Per the plan: "Only compute E10 similarity for top-100 from E1 stage"
    ///
    /// Default: `false` (disabled, backward compatible)
    #[serde(default)]
    pub enable_intent_gate: bool,

    /// Minimum E10 intent similarity threshold for the intent gate [0.0, 1.0].
    ///
    /// Candidates below this threshold are filtered out during the intent gate stage.
    /// Higher values = more restrictive filtering.
    ///
    /// Only used when `enable_intent_gate = true` and `strategy = Pipeline`.
    ///
    /// Default: `0.3` (moderate filtering)
    #[serde(default = "TeleologicalSearchOptions::default_intent_gate_threshold")]
    pub intent_gate_threshold: f32,

    /// Intent direction for E10 asymmetric intent gate.
    ///
    /// Controls how E10 similarity is computed:
    /// - `SeekingIntent`: Query is a goal, seeking relevant context (1.2x boost)
    /// - `SeekingContext`: Query is a situation, seeking relevant intent (0.8x dampening)
    /// - `Auto`: Auto-detect from query text
    ///
    /// Default: `Auto`
    #[serde(default)]
    pub intent_direction: IntentDirection,

    /// E10 intent blend weight for combining E1 and E10 scores [0.0, 1.0].
    ///
    /// When `enable_intent_gate = true`, this controls how E10 scores are blended
    /// with E1 scores: `final = e1_score * (1.0 - blend) + e10_score * blend`
    ///
    /// - `0.0`: Pure E1 scoring (E10 only used for filtering)
    /// - `0.3`: Balanced blend (default)
    /// - `1.0`: Pure E10 scoring (not recommended)
    ///
    /// Default: `0.3`
    #[serde(default = "TeleologicalSearchOptions::default_intent_blend")]
    pub intent_blend: f32,
}

impl TeleologicalSearchOptions {
    fn default_intent_gate_threshold() -> f32 {
        0.3
    }

    fn default_intent_blend() -> f32 {
        0.3
    }

    fn default_rerank_weight() -> f32 {
        0.4 // 40% E12 MaxSim, 60% fusion score
    }
}

impl Default for TeleologicalSearchOptions {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            top_k: 10,
            min_similarity: 0.0,
            include_deleted: false,
            embedder_indices: Vec::new(),
            semantic_query: None,
            include_content: false, // TASK-CONTENT-005: Opt-in for performance
            // Multi-space options (TASK-MULTISPACE)
            strategy: SearchStrategy::default(),
            weight_profile: None,
            custom_weights: None,
            exclude_embedders: Vec::new(),
            recency_boost: 0.0, // Deprecated: use temporal_options.temporal_weight
            enable_rerank: false,
            rerank_weight: Self::default_rerank_weight(),
            normalization: NormalizationStrategyOption::default(),
            // Fusion strategy (ARCH-18) - WeightedRRF by default
            fusion_strategy: FusionStrategy::default(),
            // Code query type detection (ARCH-16)
            query_text: None,
            code_query_type: None,
            // Temporal search options (ARCH-14) - No boost by default
            temporal_options: TemporalSearchOptions::default(),
            // Causal search options (ARCH-15) - Unknown (symmetric) by default
            causal_direction: CausalDirection::Unknown,
            // E10 Intent Gate options (Phase 4) - Disabled by default
            enable_intent_gate: false,
            intent_gate_threshold: Self::default_intent_gate_threshold(),
            intent_direction: IntentDirection::default(),
            intent_blend: Self::default_intent_blend(),
        }
    }
}

impl TeleologicalSearchOptions {
    /// Create options for a quick top-k search.
    #[inline]
    pub fn quick(top_k: usize) -> Self {
        Self {
            top_k,
            ..Default::default()
        }
    }

    /// Create options with minimum similarity threshold.
    #[inline]
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = threshold;
        self
    }

    /// Create options filtering by specific embedders.
    #[inline]
    pub fn with_embedders(mut self, indices: Vec<usize>) -> Self {
        self.embedder_indices = indices;
        self
    }

    /// Attach semantic fingerprint for computing per-embedder similarity scores.
    /// When provided, computes actual cosine similarities between query and
    /// stored semantic fingerprints instead of returning zeros.
    #[inline]
    pub fn with_semantic_query(mut self, semantic: SemanticFingerprint) -> Self {
        self.semantic_query = Some(semantic);
        self
    }

    /// Set whether to include original content text in search results.
    ///
    /// When `true`, content will be fetched and included in results.
    /// Default is `false` for better performance.
    ///
    /// TASK-CONTENT-005: Builder method for content inclusion.
    #[inline]
    pub fn with_include_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    // =========================================================================
    // Multi-Space Search Builder Methods (TASK-MULTISPACE)
    // =========================================================================

    /// Set the search strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy` - One of `E1Only`, `MultiSpace`, or `Pipeline`
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::{TeleologicalSearchOptions, SearchStrategy};
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_strategy(SearchStrategy::MultiSpace);
    /// ```
    #[inline]
    pub fn with_strategy(mut self, strategy: SearchStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the weight profile for multi-space scoring.
    ///
    /// # Arguments
    ///
    /// * `profile` - Profile name (e.g., "semantic_search", "code_search")
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_weight_profile("code_search");
    /// ```
    #[inline]
    pub fn with_weight_profile(mut self, profile: &str) -> Self {
        self.weight_profile = Some(profile.to_string());
        self
    }

    /// Set custom per-embedder weights. Overrides `weight_profile` when set.
    ///
    /// The weights array maps `[E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13]`.
    /// Must pass `validate_weights()`: each in [0.0, 1.0] and sum ~1.0.
    #[inline]
    pub fn with_custom_weights(mut self, weights: [f32; 13]) -> Self {
        self.custom_weights = Some(weights);
        self
    }

    /// Exclude specific embedders from multi-space fusion.
    ///
    /// Excluded embedder weights are zeroed and remaining weights renormalized.
    #[inline]
    pub fn with_exclude_embedders(mut self, indices: Vec<usize>) -> Self {
        self.exclude_embedders = indices;
        self
    }

    /// Set the recency boost factor.
    ///
    /// Applied POST-retrieval as: `final = semantic * (1.0 - boost) + temporal * boost`.
    ///
    /// # Arguments
    ///
    /// * `factor` - Boost factor [0.0, 1.0]. Clamped to valid range.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    ///
    /// // DEPRECATED: Use with_temporal_options instead
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_recency_boost(0.3);
    /// ```
    #[deprecated(since = "6.1.0", note = "Use with_temporal_options instead")]
    #[inline]
    #[allow(deprecated)]
    pub fn with_recency_boost(mut self, factor: f32) -> Self {
        self.recency_boost = factor.clamp(0.0, 1.0);
        self
    }

    /// Migrate legacy recency_boost to temporal_options.
    ///
    /// Call this on options to ensure backward compatibility.
    /// If recency_boost is set (>0) but temporal_options.temporal_weight is not,
    /// this will migrate the value.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut opts = legacy_options.migrate_legacy();
    /// // Now uses temporal_options.temporal_weight instead of recency_boost
    /// ```
    #[allow(deprecated)]
    pub fn migrate_legacy(mut self) -> Self {
        if self.recency_boost > 0.0 && self.temporal_options.temporal_weight == 0.0 {
            self.temporal_options.temporal_weight = self.recency_boost;
            self.temporal_options.decay_function = DecayFunction::Exponential;
        }
        self
    }

    /// Enable or disable E12 ColBERT re-ranking.
    ///
    /// Only effective with `SearchStrategy::Pipeline`.
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable re-ranking
    #[inline]
    pub fn with_rerank(mut self, enable: bool) -> Self {
        self.enable_rerank = enable;
        self
    }

    /// Set the E12 rerank weight for blending with fusion score.
    ///
    /// Controls how much weight E12 MaxSim scores have in final ranking:
    /// - 0.0: Pure fusion score (E12 has no effect)
    /// - 0.4: Default - 60% fusion + 40% E12 MaxSim
    /// - 1.0: Pure E12 MaxSim score (ignores fusion)
    ///
    /// # Panics
    ///
    /// Panics if weight is not in range [0.0, 1.0].
    #[inline]
    pub fn with_rerank_weight(mut self, weight: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&weight),
            "rerank_weight must be between 0.0 and 1.0, got {}",
            weight
        );
        self.rerank_weight = weight;
        self
    }

    /// Set the normalization strategy for score fusion.
    ///
    /// # Arguments
    ///
    /// * `strategy` - Normalization strategy
    #[inline]
    pub fn with_normalization(mut self, strategy: NormalizationStrategyOption) -> Self {
        self.normalization = strategy;
        self
    }

    /// Set the fusion strategy for combining multi-embedder results (ARCH-18).
    ///
    /// # Arguments
    ///
    /// * `strategy` - Fusion strategy (`WeightedSum` or `WeightedRRF`)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    /// use context_graph_core::fusion::FusionStrategy;
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_fusion_strategy(FusionStrategy::WeightedRRF);
    /// ```
    #[inline]
    pub fn with_fusion_strategy(mut self, strategy: FusionStrategy) -> Self {
        self.fusion_strategy = strategy;
        self
    }

    // =========================================================================
    // Code Query Type Builder Methods (ARCH-16)
    // =========================================================================

    /// Set the query text for E7 Code query type detection.
    ///
    /// When provided, enables automatic detection of whether the query is:
    /// - Code2Code: Actual code syntax (e.g., "fn process<T>()")
    /// - Text2Code: Natural language about code (e.g., "batch processing function")
    /// - NonCode: Not code-related
    ///
    /// E7 similarity computation is adjusted based on detected type.
    ///
    /// # Arguments
    ///
    /// * `query` - The original query text
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_query_text("impl Iterator for Counter");
    /// ```
    #[inline]
    pub fn with_query_text(mut self, query: &str) -> Self {
        self.query_text = Some(query.to_string());
        self
    }

    /// Explicitly set the code query type.
    ///
    /// Use this to override auto-detection when you know the query type.
    ///
    /// # Arguments
    ///
    /// * `query_type` - The code query type
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    /// use context_graph_core::code::CodeQueryType;
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_code_query_type(CodeQueryType::Code2Code);
    /// ```
    #[inline]
    pub fn with_code_query_type(mut self, query_type: CodeQueryType) -> Self {
        self.code_query_type = Some(query_type);
        self
    }

    /// Get the effective code query type, detecting if necessary.
    ///
    /// Returns:
    /// - The explicitly set `code_query_type` if present
    /// - Auto-detected type from `query_text` if present
    /// - `None` if neither is available
    pub fn effective_code_query_type(&self) -> Option<CodeQueryType> {
        if let Some(explicit) = self.code_query_type {
            return Some(explicit);
        }
        if let Some(ref text) = self.query_text {
            return Some(crate::code::detect_code_query_type(text));
        }
        None
    }

    // =========================================================================
    // Temporal Search Builder Methods (ARCH-14)
    // =========================================================================

    /// Set temporal search options.
    ///
    /// Per ARCH-14: Temporal boosts are applied POST-retrieval.
    ///
    /// # Arguments
    ///
    /// * `options` - Temporal search options
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::traits::{TeleologicalSearchOptions, TemporalSearchOptions, DecayFunction};
    ///
    /// let temporal = TemporalSearchOptions::default()
    ///     .with_decay_function(DecayFunction::Exponential)
    ///     .with_temporal_weight(0.3);
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_temporal_options(temporal);
    /// ```
    #[inline]
    pub fn with_temporal_options(mut self, options: TemporalSearchOptions) -> Self {
        self.temporal_options = options;
        self
    }

    /// Set the temporal weight for POST-retrieval temporal boosting.
    ///
    /// Applied as: `final = semantic * (1.0 - weight) + temporal * weight`.
    ///
    /// Note: This sets `temporal_options.temporal_weight`. If you need the legacy
    /// `recency_boost` behavior, use `with_recency_boost()` instead.
    ///
    /// # Arguments
    ///
    /// * `weight` - Weight for temporal boost [0.0, 1.0]
    #[inline]
    pub fn with_temporal_weight(mut self, weight: f32) -> Self {
        self.temporal_options.temporal_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Set the decay function for E2 recency scoring.
    ///
    /// # Arguments
    ///
    /// * `decay` - Decay function (Linear, Exponential, Step, NoDecay)
    #[inline]
    pub fn with_decay_function(mut self, decay: DecayFunction) -> Self {
        self.temporal_options.decay_function = decay;
        self
    }

    /// Filter results to the last N hours.
    ///
    /// Memories outside this time window will be excluded.
    ///
    /// # Arguments
    ///
    /// * `hours` - Number of hours to include
    #[inline]
    pub fn with_last_hours(mut self, hours: u64) -> Self {
        self.temporal_options.time_window = Some(TimeWindow::last_hours(hours));
        self
    }

    /// Filter results to the last N days.
    ///
    /// Memories outside this time window will be excluded.
    ///
    /// # Arguments
    ///
    /// * `days` - Number of days to include
    #[inline]
    pub fn with_last_days(mut self, days: u64) -> Self {
        self.temporal_options.time_window = Some(TimeWindow::last_days(days));
        self
    }

    /// Filter results to a specific session.
    ///
    /// Memories from other sessions will be excluded.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Session ID to filter to
    #[inline]
    pub fn with_session_filter(mut self, session_id: impl Into<String>) -> Self {
        self.temporal_options.session_id = Some(session_id.into());
        self
    }

    /// Enable E3 periodic pattern matching for current time.
    ///
    /// Boosts memories created at similar times (same hour/day of week).
    ///
    /// # Arguments
    ///
    /// * `weight` - Weight for periodic boost [0.0, 1.0]
    #[inline]
    pub fn with_periodic_boost(mut self, weight: f32) -> Self {
        let mut periodic = PeriodicOptions::current_time();
        periodic.weight = weight.clamp(0.0, 1.0);
        self.temporal_options.periodic_options = Some(periodic);
        self
    }

    /// Find memories before a specific anchor memory using E4.
    ///
    /// Uses E4 positional embeddings to find memories that occurred before
    /// the anchor in temporal sequence.
    ///
    /// # Arguments
    ///
    /// * `anchor_id` - UUID of the anchor memory
    #[inline]
    pub fn with_sequence_before(mut self, anchor_id: Uuid) -> Self {
        self.temporal_options.sequence_options = Some(SequenceOptions::before(anchor_id));
        self
    }

    /// Find memories after a specific anchor memory using E4.
    ///
    /// Uses E4 positional embeddings to find memories that occurred after
    /// the anchor in temporal sequence.
    ///
    /// # Arguments
    ///
    /// * `anchor_id` - UUID of the anchor memory
    #[inline]
    pub fn with_sequence_after(mut self, anchor_id: Uuid) -> Self {
        self.temporal_options.sequence_options = Some(SequenceOptions::after(anchor_id));
        self
    }

    /// Find memories around a specific anchor memory using E4.
    ///
    /// Uses E4 positional embeddings to find memories that occurred
    /// both before and after the anchor in temporal sequence.
    ///
    /// # Arguments
    ///
    /// * `anchor_id` - UUID of the anchor memory
    #[inline]
    pub fn with_sequence_around(mut self, anchor_id: Uuid) -> Self {
        self.temporal_options.sequence_options = Some(SequenceOptions::around(anchor_id));
        self
    }

    /// Set the temporal scale for multi-scale reasoning.
    ///
    /// Affects decay half-life and time horizon automatically.
    ///
    /// # Arguments
    ///
    /// * `scale` - Temporal scale (Micro, Meso, Macro, Long, Archival)
    #[inline]
    pub fn with_temporal_scale(mut self, scale: TemporalScale) -> Self {
        self.temporal_options.temporal_scale = scale;
        self
    }

    /// Check if any temporal boost is active.
    ///
    /// Returns true if temporal_options has any active boost configured.
    #[inline]
    pub fn has_temporal_boost(&self) -> bool {
        self.temporal_options.has_any_boost()
    }

    // =========================================================================
    // Causal Search Builder Methods (ARCH-15)
    // =========================================================================

    /// Set the causal direction for asymmetric E5 retrieval.
    ///
    /// Per ARCH-15 and AP-77:
    /// - `Cause`: Query seeks causes (e.g., "why did X fail")
    /// - `Effect`: Query seeks effects (e.g., "what happens when X")
    /// - `Unknown`: Symmetric similarity (default)
    ///
    /// When set, E5 similarity computation uses asymmetric vectors:
    /// - Cause queries: query.e5_as_cause vs doc.e5_as_effect
    /// - Effect queries: query.e5_as_effect vs doc.e5_as_cause
    ///
    /// Direction modifiers are applied:
    /// - cause→effect: 1.2x boost
    /// - effect→cause: 0.8x dampening
    ///
    /// # Arguments
    ///
    /// * `direction` - The causal direction of the query
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::TeleologicalSearchOptions;
    /// use context_graph_core::causal::asymmetric::CausalDirection;
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_causal_direction(CausalDirection::Cause);
    /// ```
    #[inline]
    pub fn with_causal_direction(mut self, direction: CausalDirection) -> Self {
        self.causal_direction = direction;
        self
    }

    /// Check if asymmetric E5 causal retrieval is active.
    ///
    /// Returns true if causal_direction is `Cause` or `Effect` (not `Unknown`).
    #[inline]
    pub fn has_causal_direction(&self) -> bool {
        !matches!(self.causal_direction, CausalDirection::Unknown)
    }

    // =========================================================================
    // E10 Intent Gate Builder Methods (Phase 4)
    // =========================================================================

    /// Enable or disable E10 intent gate in pipeline strategy.
    ///
    /// When enabled, adds an E10 intent filtering stage between E1 scoring
    /// and E12 reranking. Only effective with `SearchStrategy::Pipeline`.
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable the intent gate
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::{TeleologicalSearchOptions, SearchStrategy};
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_strategy(SearchStrategy::Pipeline)
    ///     .with_intent_gate(true)
    ///     .with_intent_gate_threshold(0.3);
    /// ```
    #[inline]
    pub fn with_intent_gate(mut self, enable: bool) -> Self {
        self.enable_intent_gate = enable;
        self
    }

    /// Set the E10 intent gate threshold.
    ///
    /// Candidates below this threshold are filtered out during the intent gate.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum E10 similarity [0.0, 1.0]
    #[inline]
    pub fn with_intent_gate_threshold(mut self, threshold: f32) -> Self {
        self.intent_gate_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set the E10 intent direction for asymmetric similarity.
    ///
    /// # Arguments
    ///
    /// * `direction` - Intent direction (SeekingIntent, SeekingContext, Auto)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::traits::{TeleologicalSearchOptions, IntentDirection};
    ///
    /// let opts = TeleologicalSearchOptions::quick(10)
    ///     .with_intent_gate(true)
    ///     .with_intent_direction(IntentDirection::SeekingIntent);
    /// ```
    #[inline]
    pub fn with_intent_direction(mut self, direction: IntentDirection) -> Self {
        self.intent_direction = direction;
        self
    }

    /// Set the E10 intent blend weight.
    ///
    /// Controls how E10 scores are blended with E1 scores when the intent gate
    /// is enabled: `final = e1_score * (1.0 - blend) + e10_score * blend`
    ///
    /// # Arguments
    ///
    /// * `blend` - Blend weight [0.0, 1.0]
    #[inline]
    pub fn with_intent_blend(mut self, blend: f32) -> Self {
        self.intent_blend = blend.clamp(0.0, 1.0);
        self
    }

    /// Check if E10 intent gate is active.
    ///
    /// Returns true if `enable_intent_gate = true` and `strategy = Pipeline`.
    #[inline]
    pub fn has_intent_gate(&self) -> bool {
        self.enable_intent_gate && matches!(self.strategy, SearchStrategy::Pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_options_default() {
        let opts = TeleologicalSearchOptions::default();
        assert_eq!(opts.top_k, 10);
        assert_eq!(opts.min_similarity, 0.0);
        assert!(!opts.include_deleted);
        assert!(opts.embedder_indices.is_empty());
        // ARCH-18: Default fusion strategy should be WeightedRRF
        assert_eq!(opts.fusion_strategy, crate::fusion::FusionStrategy::WeightedRRF);
    }

    #[test]
    fn test_search_options_quick() {
        let opts = TeleologicalSearchOptions::quick(50);
        assert_eq!(opts.top_k, 50);
    }

    #[test]
    fn test_search_options_builder() {
        let opts = TeleologicalSearchOptions::quick(20)
            .with_min_similarity(0.5)
            .with_embedders(vec![0, 1, 2]);

        assert_eq!(opts.top_k, 20);
        assert_eq!(opts.min_similarity, 0.5);
        assert_eq!(opts.embedder_indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_search_options_fusion_strategy() {
        // Test default is WeightedRRF per ARCH-18
        let opts = TeleologicalSearchOptions::default();
        assert_eq!(opts.fusion_strategy, crate::fusion::FusionStrategy::WeightedRRF);

        // Test builder method
        let opts = TeleologicalSearchOptions::quick(10)
            .with_fusion_strategy(crate::fusion::FusionStrategy::WeightedSum);
        assert_eq!(opts.fusion_strategy, crate::fusion::FusionStrategy::WeightedSum);

        let opts = TeleologicalSearchOptions::quick(10)
            .with_fusion_strategy(crate::fusion::FusionStrategy::WeightedRRF);
        assert_eq!(opts.fusion_strategy, crate::fusion::FusionStrategy::WeightedRRF);
    }

    // =========================================================================
    // Temporal Options Tests (ARCH-14)
    // =========================================================================

    #[test]
    fn test_decay_function_is_active() {
        assert!(DecayFunction::Linear.is_active());
        assert!(DecayFunction::Exponential.is_active());
        assert!(DecayFunction::Step.is_active());
        assert!(!DecayFunction::NoDecay.is_active());
    }

    #[test]
    fn test_time_window_contains() {
        let window = TimeWindow {
            start_ms: Some(1000),
            end_ms: Some(2000),
        };
        assert!(!window.contains(999));
        assert!(window.contains(1000));
        assert!(window.contains(1500));
        assert!(!window.contains(2000));
        assert!(!window.contains(2001));

        // Open-ended windows
        let open_start = TimeWindow {
            start_ms: None,
            end_ms: Some(2000),
        };
        assert!(open_start.contains(0));
        assert!(open_start.contains(1000));
        assert!(!open_start.contains(2000));

        let open_end = TimeWindow {
            start_ms: Some(1000),
            end_ms: None,
        };
        assert!(!open_end.contains(999));
        assert!(open_end.contains(1000));
        assert!(open_end.contains(10000000));
    }

    #[test]
    fn test_time_window_is_defined() {
        assert!(!TimeWindow::default().is_defined());
        assert!(TimeWindow { start_ms: Some(0), end_ms: None }.is_defined());
        assert!(TimeWindow { start_ms: None, end_ms: Some(100) }.is_defined());
        assert!(TimeWindow { start_ms: Some(0), end_ms: Some(100) }.is_defined());
    }

    #[test]
    fn test_temporal_search_options_default() {
        let opts = TemporalSearchOptions::default();
        assert_eq!(opts.decay_function, DecayFunction::Linear);
        assert_eq!(opts.decay_half_life_secs, 86400);
        assert!(opts.time_window.is_none());
        assert!(opts.session_id.is_none());
        assert!(opts.periodic_options.is_none());
        assert!(opts.sequence_options.is_none());
        assert_eq!(opts.temporal_scale, TemporalScale::Meso);
        assert_eq!(opts.temporal_weight, 0.0);
    }

    #[test]
    fn test_temporal_search_options_has_any_boost() {
        // Default has no boost (temporal_weight = 0)
        let opts = TemporalSearchOptions::default();
        assert!(!opts.has_any_boost());

        // With weight but no features - still no boost (needs active features)
        let opts = TemporalSearchOptions::default()
            .with_temporal_weight(0.5)
            .with_decay_function(DecayFunction::NoDecay);
        assert!(!opts.has_any_boost());

        // With weight and decay - has boost
        let opts = TemporalSearchOptions::default()
            .with_temporal_weight(0.5)
            .with_decay_function(DecayFunction::Linear);
        assert!(opts.has_any_boost());

        // With weight and time window - has boost
        let opts = TemporalSearchOptions::default()
            .with_temporal_weight(0.5)
            .with_decay_function(DecayFunction::NoDecay)
            .with_last_hours(24);
        assert!(opts.has_any_boost());
    }

    #[test]
    fn test_temporal_scale_values() {
        assert_eq!(TemporalScale::Micro.horizon_seconds(), 300);
        assert_eq!(TemporalScale::Meso.horizon_seconds(), 3600);
        assert_eq!(TemporalScale::Macro.horizon_seconds(), 86400 * 7);

        assert_eq!(TemporalScale::Micro.decay_half_life(), 60);
        assert_eq!(TemporalScale::Meso.decay_half_life(), 1800);
    }

    #[test]
    fn test_search_options_temporal_builder() {
        let anchor_id = Uuid::new_v4();

        let opts = TeleologicalSearchOptions::quick(10)
            .with_temporal_weight(0.3)
            .with_decay_function(DecayFunction::Exponential)
            .with_last_hours(24)
            .with_sequence_before(anchor_id);

        assert_eq!(opts.temporal_options.temporal_weight, 0.3);
        assert_eq!(opts.temporal_options.decay_function, DecayFunction::Exponential);
        assert!(opts.temporal_options.time_window.is_some());
        assert!(opts.temporal_options.sequence_options.is_some());
        assert_eq!(
            opts.temporal_options.sequence_options.as_ref().unwrap().direction,
            SequenceDirection::Before
        );
        assert!(opts.has_temporal_boost());
    }

    #[test]
    fn test_periodic_options_effective_values() {
        // Explicit values
        let opts = PeriodicOptions::for_hour(14);
        assert_eq!(opts.effective_hour(), Some(14));

        let opts = PeriodicOptions::for_day(3);
        assert_eq!(opts.effective_day_of_week(), Some(3));

        // Auto-detect should return Some value (current time)
        let opts = PeriodicOptions::current_time();
        assert!(opts.effective_hour().is_some());
        assert!(opts.effective_day_of_week().is_some());
    }

    #[test]
    fn test_sequence_options_directions() {
        let id = Uuid::new_v4();

        let before = SequenceOptions::before(id);
        assert_eq!(before.direction, SequenceDirection::Before);
        assert_eq!(before.anchor_id, id);

        let after = SequenceOptions::after(id);
        assert_eq!(after.direction, SequenceDirection::After);

        let around = SequenceOptions::around(id);
        assert_eq!(around.direction, SequenceDirection::Both);
    }
}
