//! DTOs for temporal search tools (E2 V_freshness, E3 V_periodicity).
//!
//! Per Constitution v6.5 and ARCH-25:
//! - E2 (V_freshness) finds recency patterns
//! - E3 (V_periodicity) finds time-of-day and day-of-week patterns
//! - Temporal boost is POST-RETRIEVAL only, NOT in similarity fusion

use serde::{Deserialize, Serialize};

use context_graph_core::traits::DecayFunction;

/// Temporal scale for decay calculation.
///
/// Controls the time horizon over which decay is computed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemporalScale {
    /// Micro scale: 1 hour horizon
    /// Use for: very recent activity, fast-moving contexts
    Micro,

    /// Meso scale: 1 day horizon (default)
    /// Use for: typical recency queries
    #[default]
    Meso,

    /// Macro scale: 1 week horizon
    /// Use for: broader time range queries
    Macro,

    /// Long scale: 1 month horizon
    /// Use for: extended time range queries
    Long,
}

impl TemporalScale {
    /// Get the time horizon in seconds for this scale.
    pub fn horizon_seconds(&self) -> i64 {
        match self {
            TemporalScale::Micro => 3600,       // 1 hour
            TemporalScale::Meso => 86400,       // 1 day
            TemporalScale::Macro => 604800,     // 1 week
            TemporalScale::Long => 2592000,     // 30 days
        }
    }
}

/// Parameters for search_recent tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecentParams {
    /// The search query text.
    pub query: String,

    /// Maximum number of results (default: 10).
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Temporal boost weight [0.1, 1.0] (default: 0.3).
    #[serde(default = "default_temporal_weight")]
    pub temporal_weight: f32,

    /// Decay function (default: exponential).
    #[serde(default)]
    pub decay_function: DecayFunctionParam,

    /// Temporal scale (default: meso).
    #[serde(default)]
    pub temporal_scale: TemporalScale,

    /// Include content in results (default: true).
    #[serde(default = "default_true")]
    pub include_content: bool,

    /// Minimum semantic similarity threshold (default: 0.1).
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
}

/// Decay function parameter with string parsing.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecayFunctionParam {
    Linear,
    #[default]
    Exponential,
    Step,
}

impl From<DecayFunctionParam> for DecayFunction {
    fn from(param: DecayFunctionParam) -> Self {
        match param {
            DecayFunctionParam::Linear => DecayFunction::Linear,
            DecayFunctionParam::Exponential => DecayFunction::Exponential,
            DecayFunctionParam::Step => DecayFunction::Step,
        }
    }
}

/// Result entry for search_recent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalSearchResultEntry {
    /// Memory ID.
    pub id: String,

    /// Original semantic similarity score.
    pub semantic_score: f32,

    /// Recency score [0.0, 1.0].
    pub recency_score: f32,

    /// Final boosted score.
    pub final_score: f32,

    /// Age in a human-readable format.
    pub age_description: String,

    /// Content text (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Created at timestamp (ISO 8601).
    pub created_at: String,
}

/// Response for search_recent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecentResponse {
    /// Query that was executed.
    pub query: String,

    /// Results sorted by final score.
    pub results: Vec<TemporalSearchResultEntry>,

    /// Number of results.
    pub count: usize,

    /// Temporal boost configuration used.
    pub temporal_config: TemporalConfigSummary,
}

/// Summary of temporal configuration used.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalConfigSummary {
    /// Temporal weight used.
    pub temporal_weight: f32,

    /// Decay function used.
    pub decay_function: String,

    /// Temporal scale used.
    pub temporal_scale: String,

    /// The time horizon in seconds for the selected temporal scale.
    pub horizon_seconds: i64,

    /// Half-life in seconds (for exponential decay: horizon / 4).
    pub half_life_seconds: f64,

    /// The boost formula applied: "clamp(1 + w*(recency - 0.5), 0.8, 1.2)".
    pub boost_formula: String,
}

// Default value functions
fn default_top_k() -> usize {
    10
}

fn default_temporal_weight() -> f32 {
    0.3
}

fn default_true() -> bool {
    true
}

fn default_min_similarity() -> f32 {
    0.1
}

/// Compute recency score using the specified decay function and horizon.
///
/// Returns [0.0, 1.0] where 1.0 is most recent.
///
/// # Arguments
/// * `memory_ts_ms` - Memory timestamp in milliseconds since epoch
/// * `now_ms` - Current timestamp in milliseconds since epoch
/// * `decay` - Decay function to use
/// * `horizon_secs` - Time horizon in seconds for decay calculation
pub fn compute_recency_score(
    memory_ts_ms: i64,
    now_ms: i64,
    decay: DecayFunction,
    horizon_secs: i64,
) -> f32 {
    let age_secs = ((now_ms - memory_ts_ms).max(0) / 1000) as f64;
    let horizon = horizon_secs as f64;

    match decay {
        DecayFunction::Linear => {
            let normalized = age_secs / horizon;
            (1.0 - normalized.min(1.0)) as f32
        }
        DecayFunction::Exponential => {
            // Half-life = horizon / 4 (gives reasonable decay curve)
            let half_life = horizon / 4.0;
            let lambda = 0.693 / half_life; // ln(2) / half_life
            (-lambda * age_secs).exp() as f32
        }
        DecayFunction::Step => {
            let fraction = age_secs / horizon;
            if fraction < 0.05 {
                1.0 // Very fresh
            } else if fraction < 0.25 {
                0.8 // Recent
            } else if fraction < 0.75 {
                0.5 // Middle
            } else {
                0.1 // Older
            }
        }
        DecayFunction::NoDecay => 0.5, // Neutral
    }
}

/// Format age as human-readable string.
pub fn format_age(memory_ts_ms: i64, now_ms: i64) -> String {
    let age_secs = (now_ms - memory_ts_ms).max(0) / 1000;
    let age_mins = age_secs / 60;
    let age_hours = age_mins / 60;
    let age_days = age_hours / 24;

    if age_secs < 60 {
        format!("{} seconds ago", age_secs)
    } else if age_mins < 60 {
        format!("{} minutes ago", age_mins)
    } else if age_hours < 24 {
        format!("{} hours ago", age_hours)
    } else if age_days < 7 {
        format!("{} days ago", age_days)
    } else {
        format!("{} weeks ago", age_days / 7)
    }
}

// =============================================================================
// E3 V_PERIODICITY - search_periodic DTOs
// =============================================================================

/// Parameters for search_periodic tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPeriodicParams {
    /// The search query text.
    pub query: String,

    /// Maximum number of results (default: 10).
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Target hour of day (0-23).
    /// If omitted and auto_detect=true, uses current hour.
    pub target_hour: Option<u8>,

    /// Target day of week (0=Sunday, 6=Saturday).
    /// If omitted and auto_detect=true, uses current day.
    pub target_day_of_week: Option<u8>,

    /// Auto-detect target from current time.
    #[serde(default)]
    pub auto_detect: bool,

    /// Periodic boost weight [0.1, 1.0] (default: 0.3).
    #[serde(default = "default_periodic_weight")]
    pub periodic_weight: f32,

    /// Include content in results (default: true).
    #[serde(default = "default_true")]
    pub include_content: bool,

    /// Minimum semantic similarity threshold (default: 0.1).
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
}

fn default_periodic_weight() -> f32 {
    0.3
}

/// Result entry for search_periodic.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodicSearchResultEntry {
    /// Memory ID.
    pub id: String,

    /// Original semantic similarity score.
    pub semantic_score: f32,

    /// Periodic pattern match score [0.0, 1.0].
    pub periodic_score: f32,

    /// Final boosted score.
    pub final_score: f32,

    /// Memory's hour of creation (0-23).
    pub memory_hour: u8,

    /// Memory's day of week (0=Sunday, 6=Saturday).
    pub memory_day_of_week: u8,

    /// Day name for readability.
    pub day_name: String,

    /// Content text (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Created at timestamp (ISO 8601).
    pub created_at: String,
}

/// Response for search_periodic.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPeriodicResponse {
    /// Query that was executed.
    pub query: String,

    /// Results sorted by final score.
    pub results: Vec<PeriodicSearchResultEntry>,

    /// Number of results.
    pub count: usize,

    /// Periodic configuration used.
    pub periodic_config: PeriodicConfigSummary,
}

/// Summary of periodic configuration used.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodicConfigSummary {
    /// Target hour used (if any).
    pub target_hour: Option<u8>,

    /// Target day of week used (if any).
    pub target_day_of_week: Option<u8>,

    /// Periodic boost weight used.
    pub periodic_weight: f32,

    /// Whether auto-detect was used.
    pub auto_detected: bool,
}

/// Convert day-of-week number to name.
pub fn day_name(dow: u8) -> &'static str {
    match dow {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => "Unknown",
    }
}
