//! DTOs for topic-related MCP tools.
//!
//! Per PRD v6 Section 10.2, these DTOs support:
//! - get_topic_portfolio: Retrieve all discovered topics with profiles
//! - get_topic_stability: Get portfolio-level stability metrics
//! - detect_topics: Force topic detection recalculation
//! - get_divergence_alerts: Check for divergence from recent activity
//!
//! Constitution References:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) NEVER count toward topic detection
//! - AP-62: Divergence alerts use SEMANTIC embedders only (E1, E5, E6, E7, E10, E12, E13)
//! - AP-70: Dream triggers when entropy > 0.7 AND churn > 0.5

// Allow unused items - constants and methods are defined for constitution compliance
// and will be used when full clustering integration is complete
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default output format for topic portfolio.
pub const DEFAULT_FORMAT: &str = "standard";

/// Default lookback hours for stability metrics (6 hours per constitution).
pub const DEFAULT_STABILITY_HOURS: u32 = 6;

/// Maximum lookback hours for stability metrics (1 week).
pub const MAX_STABILITY_HOURS: u32 = 168;

/// Default lookback hours for divergence alerts.
pub const DEFAULT_DIVERGENCE_LOOKBACK: u32 = 2;

/// Maximum lookback hours for divergence alerts.
pub const MAX_DIVERGENCE_LOOKBACK: u32 = 48;

/// Maximum weighted agreement per constitution (7 semantic + 2*0.5 relational + 1*0.5 structural).
pub const MAX_WEIGHTED_AGREEMENT: f32 = 8.5;

/// Topic threshold per ARCH-09.
pub const TOPIC_THRESHOLD: f32 = 2.5;

/// Churn threshold for dream trigger per AP-70.
pub const CHURN_THRESHOLD: f32 = 0.5;

/// Entropy threshold for dream trigger per AP-70.
pub const ENTROPY_THRESHOLD: f32 = 0.7;

/// Severity thresholds for divergence alerts.
/// High severity when avg delta exceeds this value.
const SEVERITY_HIGH_DELTA_THRESHOLD: f32 = 0.3;
/// Medium severity when avg delta exceeds this value.
const SEVERITY_MEDIUM_DELTA_THRESHOLD: f32 = 0.15;
/// High severity when alert count reaches this value.
const SEVERITY_HIGH_ALERT_COUNT: usize = 3;
/// Medium severity when alert count reaches this value.
const SEVERITY_MEDIUM_ALERT_COUNT: usize = 2;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for get_topic_portfolio tool.
///
/// # Example JSON
/// ```json
/// {"format": "standard"}
/// ```
///
/// # Defaults
/// - `format`: "standard"
#[derive(Debug, Clone, Deserialize)]
pub struct GetTopicPortfolioRequest {
    /// Output format: "brief", "standard", or "verbose"
    /// - brief: Topic names and confidence only
    /// - standard: Includes contributing spaces and member counts
    /// - verbose: Full topic profiles with all 13 strengths
    #[serde(default = "default_format")]
    pub format: String,
}

impl Default for GetTopicPortfolioRequest {
    fn default() -> Self {
        Self {
            format: DEFAULT_FORMAT.to_string(),
        }
    }
}

fn default_format() -> String {
    DEFAULT_FORMAT.to_string()
}

impl GetTopicPortfolioRequest {
    /// Valid format values for topic portfolio requests.
    pub const VALID_FORMATS: [&'static str; 3] = ["brief", "standard", "verbose"];

    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if format is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if !Self::VALID_FORMATS.contains(&self.format.as_str()) {
            return Err(format!(
                "Invalid format '{}'. Valid formats: {}",
                self.format,
                Self::VALID_FORMATS.join(", ")
            ));
        }
        Ok(())
    }
}

/// Request parameters for get_topic_stability tool.
///
/// # Example JSON
/// ```json
/// {"hours": 6}
/// ```
///
/// # Defaults
/// - `hours`: 6 (per constitution)
#[derive(Debug, Clone, Deserialize)]
pub struct GetTopicStabilityRequest {
    /// Lookback period in hours for computing averages (default 6, max 168)
    #[serde(default = "default_hours")]
    pub hours: u32,
}

impl Default for GetTopicStabilityRequest {
    fn default() -> Self {
        Self {
            hours: DEFAULT_STABILITY_HOURS,
        }
    }
}

fn default_hours() -> u32 {
    DEFAULT_STABILITY_HOURS
}

impl GetTopicStabilityRequest {
    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if hours is out of range [1, 168].
    pub fn validate(&self) -> Result<(), String> {
        if self.hours == 0 {
            return Err("hours must be at least 1".to_string());
        }
        if self.hours > MAX_STABILITY_HOURS {
            return Err(format!(
                "hours must be at most {}, got {}",
                MAX_STABILITY_HOURS, self.hours
            ));
        }
        Ok(())
    }
}

/// Request parameters for detect_topics tool.
///
/// # Example JSON
/// ```json
/// {"force": false}
/// ```
///
/// # Defaults
/// - `force`: false
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DetectTopicsRequest {
    /// Force detection even if recently computed
    #[serde(default)]
    pub force: bool,
}

/// Request parameters for get_divergence_alerts tool.
///
/// # Example JSON
/// ```json
/// {"lookback_hours": 2}
/// ```
///
/// # Defaults
/// - `lookback_hours`: 2
#[derive(Debug, Clone, Deserialize)]
pub struct GetDivergenceAlertsRequest {
    /// Hours to look back for recent activity comparison (default 2, max 48)
    #[serde(default = "default_lookback")]
    pub lookback_hours: u32,
}

impl Default for GetDivergenceAlertsRequest {
    fn default() -> Self {
        Self {
            lookback_hours: DEFAULT_DIVERGENCE_LOOKBACK,
        }
    }
}

fn default_lookback() -> u32 {
    DEFAULT_DIVERGENCE_LOOKBACK
}

impl GetDivergenceAlertsRequest {
    /// Validate the request parameters.
    ///
    /// # Errors
    /// Returns an error message if lookback_hours is out of range [1, 48].
    pub fn validate(&self) -> Result<(), String> {
        if self.lookback_hours == 0 {
            return Err("lookback_hours must be at least 1".to_string());
        }
        if self.lookback_hours > MAX_DIVERGENCE_LOOKBACK {
            return Err(format!(
                "lookback_hours must be at most {}, got {}",
                MAX_DIVERGENCE_LOOKBACK, self.lookback_hours
            ));
        }
        Ok(())
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// Response for get_topic_portfolio tool.
#[derive(Debug, Clone, Serialize)]
pub struct TopicPortfolioResponse {
    /// List of discovered topics
    pub topics: Vec<TopicSummary>,

    /// Portfolio-level stability metrics
    pub stability: StabilityMetricsSummary,

    /// Total number of topics
    pub total_topics: usize,

    /// Current progressive tier (0-6) based on memory count
    /// - Tier 0: 0 memories
    /// - Tier 1: 1-2 memories
    /// - Tier 2: 3-9 memories (basic clustering)
    /// - Tier 3: 10-29 memories (divergence detection)
    /// - Tier 4: 30-99 memories (reliable statistics)
    /// - Tier 5: 100-499 memories (sub-clustering)
    /// - Tier 6: 500+ memories (full personalization)
    pub tier: u8,
}

impl TopicPortfolioResponse {
    /// Create an empty response for tier 0 (no memories).
    pub fn empty_tier_0() -> Self {
        Self {
            topics: Vec::new(),
            stability: StabilityMetricsSummary::default(),
            total_topics: 0,
            tier: 0,
        }
    }

    /// Calculate the tier based on memory count.
    pub fn tier_for_memory_count(count: usize) -> u8 {
        match count {
            0 => 0,
            1..=2 => 1,
            3..=9 => 2,
            10..=29 => 3,
            30..=99 => 4,
            100..=499 => 5,
            _ => 6,
        }
    }
}

/// Summary of a single topic for API response.
///
/// This is a serializable view of the core Topic type,
/// suitable for JSON transmission.
#[derive(Debug, Clone, Serialize)]
pub struct TopicSummary {
    /// Topic UUID
    pub id: Uuid,

    /// Optional human-readable name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Confidence score = weighted_agreement / 8.5 (range 0.0-1.0)
    pub confidence: f32,

    /// Weighted agreement score per ARCH-09
    /// - Threshold for topic: >= 2.5
    /// - Max possible: 8.5 (7 semantic + 2 relational*0.5 + 1 structural*0.5)
    pub weighted_agreement: f32,

    /// Number of memories belonging to this topic
    pub member_count: usize,

    /// Contributing embedding spaces (those with strength > 0.5)
    /// Example: ["Semantic", "Causal", "Code"]
    pub contributing_spaces: Vec<String>,

    /// Current lifecycle phase
    pub phase: String,
}

impl TopicSummary {
    /// Check if this topic meets the validity threshold.
    ///
    /// Per ARCH-09: weighted_agreement >= 2.5
    #[inline]
    pub fn is_valid_topic(&self) -> bool {
        self.weighted_agreement >= TOPIC_THRESHOLD
    }

    /// Compute confidence from weighted agreement.
    ///
    /// confidence = weighted_agreement / 8.5
    #[inline]
    pub fn compute_confidence(weighted_agreement: f32) -> f32 {
        (weighted_agreement / MAX_WEIGHTED_AGREEMENT).clamp(0.0, 1.0)
    }
}

/// Portfolio-level stability metrics summary.
#[derive(Debug, Clone, Serialize, Default)]
pub struct StabilityMetricsSummary {
    /// Churn rate [0.0-1.0] where 0.0=stable, 1.0=complete turnover
    /// Computed as |symmetric_difference| / |union| of topic IDs over time
    pub churn_rate: f32,

    /// Topic distribution entropy [0.0-1.0]
    pub entropy: f32,

    /// Whether portfolio is stable (churn < 0.3 per constitution)
    pub is_stable: bool,
}

impl StabilityMetricsSummary {
    /// Create stability metrics with the given values.
    pub fn new(churn_rate: f32, entropy: f32) -> Self {
        Self {
            churn_rate,
            entropy,
            is_stable: churn_rate < 0.3,
        }
    }
}

/// Response for get_topic_stability tool.
#[derive(Debug, Clone, Serialize)]
pub struct TopicStabilityResponse {
    /// Current churn rate
    pub churn_rate: f32,

    /// Current entropy
    pub entropy: f32,

    /// Breakdown by lifecycle phase
    pub phases: PhaseBreakdown,

    /// Whether dream consolidation is recommended
    /// Per AP-70: entropy > 0.7 AND churn > 0.5
    pub dream_recommended: bool,

    /// Warning flag for high churn (churn >= 0.5)
    pub high_churn_warning: bool,

    /// Average churn over the requested lookback period
    pub average_churn: f32,
}

impl TopicStabilityResponse {
    /// Check if dream consolidation is recommended.
    ///
    /// Per AP-70: entropy > 0.7 AND churn > 0.5
    #[inline]
    pub fn should_recommend_dream(entropy: f32, churn: f32) -> bool {
        entropy > ENTROPY_THRESHOLD && churn > CHURN_THRESHOLD
    }

    /// Check if high churn warning should be shown.
    #[inline]
    pub fn is_high_churn(churn: f32) -> bool {
        churn >= CHURN_THRESHOLD
    }
}

/// Count of topics in each lifecycle phase.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PhaseBreakdown {
    /// Topics < 1hr old with high churn
    pub emerging: u32,

    /// Topics >= 24hr old with churn < 0.1
    pub stable: u32,

    /// Topics with churn >= 0.5
    pub declining: u32,

    /// Topics being absorbed into others
    pub merging: u32,
}

impl PhaseBreakdown {
    /// Total count of all topics.
    pub fn total(&self) -> u32 {
        self.emerging + self.stable + self.declining + self.merging
    }
}

/// Response for detect_topics tool.
#[derive(Debug, Clone, Serialize)]
pub struct DetectTopicsResponse {
    /// Newly discovered topics from this detection run
    pub new_topics: Vec<TopicSummary>,

    /// Topics that were merged during detection
    pub merged_topics: Vec<MergedTopicInfo>,

    /// Total topic count after detection
    pub total_after: usize,

    /// Human-readable message about what happened
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DetectTopicsResponse {
    /// Create an empty response when detection found nothing.
    pub fn empty() -> Self {
        Self {
            new_topics: Vec::new(),
            merged_topics: Vec::new(),
            total_after: 0,
            message: None,
        }
    }

    /// Create a response when insufficient memories exist.
    pub fn insufficient_memories(current_count: usize) -> Self {
        Self {
            new_topics: Vec::new(),
            merged_topics: Vec::new(),
            total_after: 0,
            message: Some(format!(
                "Need at least 3 memories for topic detection, currently have {}",
                current_count
            )),
        }
    }
}

/// Information about a topic merge operation.
#[derive(Debug, Clone, Serialize)]
pub struct MergedTopicInfo {
    /// ID of the topic that was absorbed
    pub absorbed_id: Uuid,

    /// ID of the topic it was merged into
    pub into_id: Uuid,
}

/// Response for get_divergence_alerts tool.
///
/// CRITICAL: Per AP-62, ONLY SEMANTIC embedders trigger alerts:
/// E1 (Semantic), E5 (Causal), E6 (Sparse), E7 (Code),
/// E10 (Multimodal), E12 (LateInteraction), E13 (SPLADE)
///
/// Temporal embedders (E2-E4) NEVER trigger divergence per AP-63.
#[derive(Debug, Clone, Serialize)]
pub struct DivergenceAlertsResponse {
    /// List of divergence alerts from SEMANTIC spaces only
    pub alerts: Vec<DivergenceAlert>,

    /// Overall severity: "none", "low", "medium", "high"
    pub severity: String,
}

impl DivergenceAlertsResponse {
    /// Create an empty response with no alerts.
    pub fn no_alerts() -> Self {
        Self {
            alerts: Vec::new(),
            severity: "none".to_string(),
        }
    }

    /// Compute severity based on alert count and average delta.
    ///
    /// Severity levels:
    /// - "none": No alerts
    /// - "low": Single alert with small delta
    /// - "medium": 2+ alerts or moderate delta (> 0.15)
    /// - "high": 3+ alerts or large delta (> 0.3)
    pub fn compute_severity(alerts: &[DivergenceAlert]) -> String {
        if alerts.is_empty() {
            return "none".to_string();
        }

        let avg_delta: f32 = alerts
            .iter()
            .map(|a| (a.threshold - a.similarity_score).max(0.0))
            .sum::<f32>()
            / alerts.len() as f32;

        // High severity: large delta or many alerts
        if avg_delta > SEVERITY_HIGH_DELTA_THRESHOLD || alerts.len() >= SEVERITY_HIGH_ALERT_COUNT {
            return "high".to_string();
        }

        // Medium severity: moderate delta or multiple alerts
        if avg_delta > SEVERITY_MEDIUM_DELTA_THRESHOLD
            || alerts.len() >= SEVERITY_MEDIUM_ALERT_COUNT
        {
            return "medium".to_string();
        }

        "low".to_string()
    }
}

/// A single divergence alert from a semantic embedding space.
#[derive(Debug, Clone, Serialize)]
pub struct DivergenceAlert {
    /// The semantic space that detected divergence
    /// One of: "E1_Semantic", "E5_Causal", "E6_Sparse", "E7_Code",
    /// "E10_Multimodal", "E12_LateInteraction", "E13_SPLADE"
    pub semantic_space: String,

    /// Similarity score between current activity and recent memory
    pub similarity_score: f32,

    /// Brief summary of the recent memory for context
    pub recent_memory_summary: String,

    /// The threshold that was crossed to trigger this alert
    /// Per constitution divergence_detection thresholds
    pub threshold: f32,
}

impl DivergenceAlert {
    /// Valid semantic spaces that can trigger divergence alerts per AP-62.
    pub const VALID_SEMANTIC_SPACES: [&'static str; 7] = [
        "E1_Semantic",
        "E5_Causal",
        "E6_Sparse",
        "E7_Code",
        "E10_Multimodal",
        "E12_LateInteraction",
        "E13_SPLADE",
    ];

    /// Check if the semantic space is valid for divergence alerts.
    pub fn is_valid_semantic_space(space: &str) -> bool {
        Self::VALID_SEMANTIC_SPACES.contains(&space)
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Request DTO Tests =====

    #[test]
    fn test_get_topic_portfolio_request_defaults() {
        let json = "{}";
        let req: GetTopicPortfolioRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "standard");
        println!("[PASS] GetTopicPortfolioRequest defaults to 'standard' format");
    }

    #[test]
    fn test_get_topic_portfolio_request_custom_format() {
        let json = r#"{"format": "verbose"}"#;
        let req: GetTopicPortfolioRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "verbose");
        println!("[PASS] GetTopicPortfolioRequest accepts custom format");
    }

    #[test]
    fn test_get_topic_portfolio_request_validation() {
        let req = GetTopicPortfolioRequest {
            format: "standard".to_string(),
        };
        assert!(req.validate().is_ok());

        let invalid = GetTopicPortfolioRequest {
            format: "invalid".to_string(),
        };
        assert!(invalid.validate().is_err());
        println!("[PASS] GetTopicPortfolioRequest validation works");
    }

    #[test]
    fn test_get_topic_stability_request_defaults() {
        let json = "{}";
        let req: GetTopicStabilityRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.hours, 6);
        println!("[PASS] GetTopicStabilityRequest defaults to 6 hours");
    }

    #[test]
    fn test_get_topic_stability_request_validation() {
        let req = GetTopicStabilityRequest { hours: 12 };
        assert!(req.validate().is_ok());

        let zero = GetTopicStabilityRequest { hours: 0 };
        assert!(zero.validate().is_err());

        let too_large = GetTopicStabilityRequest { hours: 200 };
        assert!(too_large.validate().is_err());
        println!("[PASS] GetTopicStabilityRequest validation works");
    }

    #[test]
    fn test_detect_topics_request_defaults() {
        let json = "{}";
        let req: DetectTopicsRequest = serde_json::from_str(json).unwrap();
        assert!(!req.force);
        println!("[PASS] DetectTopicsRequest defaults to force=false");
    }

    #[test]
    fn test_detect_topics_request_force_true() {
        let json = r#"{"force": true}"#;
        let req: DetectTopicsRequest = serde_json::from_str(json).unwrap();
        assert!(req.force);
        println!("[PASS] DetectTopicsRequest accepts force=true");
    }

    #[test]
    fn test_get_divergence_alerts_request_defaults() {
        let json = "{}";
        let req: GetDivergenceAlertsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.lookback_hours, 2);
        println!("[PASS] GetDivergenceAlertsRequest defaults to 2 hours");
    }

    #[test]
    fn test_get_divergence_alerts_request_validation() {
        let req = GetDivergenceAlertsRequest { lookback_hours: 24 };
        assert!(req.validate().is_ok());

        let zero = GetDivergenceAlertsRequest { lookback_hours: 0 };
        assert!(zero.validate().is_err());

        let too_large = GetDivergenceAlertsRequest {
            lookback_hours: 100,
        };
        assert!(too_large.validate().is_err());
        println!("[PASS] GetDivergenceAlertsRequest validation works");
    }

    // ===== Response DTO Tests =====

    #[test]
    fn test_topic_summary_serialization() {
        let summary = TopicSummary {
            id: Uuid::nil(),
            name: Some("Test Topic".to_string()),
            confidence: 0.35,
            weighted_agreement: 3.0,
            member_count: 15,
            contributing_spaces: vec!["Semantic".to_string(), "Causal".to_string()],
            phase: "Stable".to_string(),
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"confidence\":0.35"));
        assert!(json.contains("\"weighted_agreement\":3"));
        assert!(json.contains("\"phase\":\"Stable\""));
        assert!(json.contains("\"name\":\"Test Topic\""));
        println!("[PASS] TopicSummary serializes correctly");
    }

    #[test]
    fn test_topic_summary_no_name_skipped() {
        let summary = TopicSummary {
            id: Uuid::nil(),
            name: None,
            confidence: 0.35,
            weighted_agreement: 3.0,
            member_count: 15,
            contributing_spaces: vec![],
            phase: "Emerging".to_string(),
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("\"name\""));
        println!("[PASS] TopicSummary skips None name");
    }

    #[test]
    fn test_topic_summary_validity_check() {
        let valid = TopicSummary {
            id: Uuid::nil(),
            name: None,
            confidence: 0.35,
            weighted_agreement: 3.0,
            member_count: 10,
            contributing_spaces: vec![],
            phase: "Stable".to_string(),
        };
        assert!(valid.is_valid_topic());

        let invalid = TopicSummary {
            id: Uuid::nil(),
            name: None,
            confidence: 0.2,
            weighted_agreement: 2.0,
            member_count: 5,
            contributing_spaces: vec![],
            phase: "Emerging".to_string(),
        };
        assert!(!invalid.is_valid_topic());
        println!("[PASS] TopicSummary validity check works (threshold 2.5)");
    }

    #[test]
    fn test_topic_summary_compute_confidence() {
        assert!((TopicSummary::compute_confidence(3.0) - 0.3529).abs() < 0.01);
        assert!((TopicSummary::compute_confidence(8.5) - 1.0).abs() < 0.001);
        assert!((TopicSummary::compute_confidence(0.0) - 0.0).abs() < 0.001);
        println!("[PASS] TopicSummary::compute_confidence works");
    }

    #[test]
    fn test_stability_metrics_summary() {
        let stable = StabilityMetricsSummary::new(0.2, 0.5);
        assert!(stable.is_stable);

        let unstable = StabilityMetricsSummary::new(0.4, 0.7);
        assert!(!unstable.is_stable);
        println!("[PASS] StabilityMetricsSummary is_stable based on churn < 0.3");
    }

    #[test]
    fn test_topic_stability_response_dream_recommendation() {
        // Both high entropy AND high churn needed per AP-70
        assert!(TopicStabilityResponse::should_recommend_dream(0.8, 0.6));
        assert!(!TopicStabilityResponse::should_recommend_dream(0.8, 0.4));
        assert!(!TopicStabilityResponse::should_recommend_dream(0.6, 0.6));
        assert!(!TopicStabilityResponse::should_recommend_dream(0.6, 0.4));
        println!("[PASS] Dream recommendation requires entropy > 0.7 AND churn > 0.5 (AP-70)");
    }

    #[test]
    fn test_phase_breakdown_serialization() {
        let phases = PhaseBreakdown {
            emerging: 2,
            stable: 8,
            declining: 1,
            merging: 0,
        };

        let json = serde_json::to_string(&phases).unwrap();
        assert!(json.contains("\"emerging\":2"));
        assert!(json.contains("\"stable\":8"));
        assert_eq!(phases.total(), 11);
        println!("[PASS] PhaseBreakdown serializes correctly");
    }

    #[test]
    fn test_detect_topics_response_message_skipped_when_none() {
        let response = DetectTopicsResponse {
            new_topics: vec![],
            merged_topics: vec![],
            total_after: 5,
            message: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("\"message\""));
        println!("[PASS] DetectTopicsResponse skips None message");
    }

    #[test]
    fn test_detect_topics_response_insufficient_memories() {
        let response = DetectTopicsResponse::insufficient_memories(2);
        assert!(response.message.is_some());
        assert!(response.message.unwrap().contains("Need at least 3"));
        println!("[PASS] DetectTopicsResponse::insufficient_memories works");
    }

    #[test]
    fn test_divergence_alert_serialization() {
        let alert = DivergenceAlert {
            semantic_space: "E1_Semantic".to_string(),
            similarity_score: 0.22,
            recent_memory_summary: "Working on auth...".to_string(),
            threshold: 0.30,
        };

        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("\"semantic_space\":\"E1_Semantic\""));
        assert!(json.contains("\"threshold\":0.3"));
        println!("[PASS] DivergenceAlert serializes correctly");
    }

    #[test]
    fn test_divergence_alert_valid_semantic_spaces() {
        // Valid semantic spaces per AP-62
        assert!(DivergenceAlert::is_valid_semantic_space("E1_Semantic"));
        assert!(DivergenceAlert::is_valid_semantic_space("E5_Causal"));
        assert!(DivergenceAlert::is_valid_semantic_space("E7_Code"));

        // Invalid - temporal spaces per AP-63
        assert!(!DivergenceAlert::is_valid_semantic_space(
            "E2_TemporalRecent"
        ));
        assert!(!DivergenceAlert::is_valid_semantic_space(
            "E3_TemporalPeriodic"
        ));
        assert!(!DivergenceAlert::is_valid_semantic_space(
            "E4_TemporalPositional"
        ));

        // Invalid - relational/structural (not for divergence)
        assert!(!DivergenceAlert::is_valid_semantic_space("E8_Graph"));
        assert!(!DivergenceAlert::is_valid_semantic_space("E11_Entity"));
        println!("[PASS] DivergenceAlert semantic space validation per AP-62/AP-63");
    }

    #[test]
    fn test_divergence_alerts_response_severity() {
        // No alerts = none
        assert_eq!(DivergenceAlertsResponse::compute_severity(&[]), "none");

        // Single alert with small delta = low
        let low_alerts = vec![DivergenceAlert {
            semantic_space: "E1_Semantic".to_string(),
            similarity_score: 0.25,
            recent_memory_summary: "test".to_string(),
            threshold: 0.30,
        }];
        assert_eq!(
            DivergenceAlertsResponse::compute_severity(&low_alerts),
            "low"
        );

        // Multiple alerts = medium or high
        let multi_alerts = vec![
            DivergenceAlert {
                semantic_space: "E1_Semantic".to_string(),
                similarity_score: 0.2,
                recent_memory_summary: "test".to_string(),
                threshold: 0.30,
            },
            DivergenceAlert {
                semantic_space: "E5_Causal".to_string(),
                similarity_score: 0.15,
                recent_memory_summary: "test".to_string(),
                threshold: 0.25,
            },
        ];
        let severity = DivergenceAlertsResponse::compute_severity(&multi_alerts);
        assert!(severity == "medium" || severity == "high");
        println!("[PASS] DivergenceAlertsResponse severity computation works");
    }

    #[test]
    fn test_topic_portfolio_response_tier_calculation() {
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(0), 0);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(1), 1);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(2), 1);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(3), 2);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(9), 2);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(10), 3);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(29), 3);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(30), 4);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(99), 4);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(100), 5);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(499), 5);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(500), 6);
        assert_eq!(TopicPortfolioResponse::tier_for_memory_count(1000), 6);
        println!("[PASS] TopicPortfolioResponse tier calculation per progressive_tiers");
    }

    #[test]
    fn test_topic_portfolio_response_empty_tier_0() {
        let response = TopicPortfolioResponse::empty_tier_0();
        assert!(response.topics.is_empty());
        assert_eq!(response.total_topics, 0);
        assert_eq!(response.tier, 0);
        println!("[PASS] TopicPortfolioResponse::empty_tier_0 works");
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_empty_format_string_accepted() {
        let json = r#"{"format": ""}"#;
        let req: GetTopicPortfolioRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.format, "");
        // Handler should reject this via validate()
        assert!(req.validate().is_err());
        println!("[PASS] Empty format string is accepted but fails validation");
    }

    #[test]
    fn test_zero_hours_boundary() {
        let json = r#"{"hours": 0}"#;
        let req: GetTopicStabilityRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.hours, 0);
        assert!(req.validate().is_err());
        println!("[PASS] Zero hours is accepted but fails validation");
    }

    #[test]
    fn test_max_hours_boundary() {
        // At boundary
        let req = GetTopicStabilityRequest { hours: 168 };
        assert!(req.validate().is_ok());

        // Over boundary
        let req = GetTopicStabilityRequest { hours: 169 };
        assert!(req.validate().is_err());
        println!("[PASS] Hours boundary validation at 168");
    }

    #[test]
    fn test_merged_topic_info_serialization() {
        let info = MergedTopicInfo {
            absorbed_id: Uuid::nil(),
            into_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"absorbed_id\""));
        assert!(json.contains("\"into_id\""));
        println!("[PASS] MergedTopicInfo serializes correctly");
    }

    // ===== Constitution Compliance Tests =====

    #[test]
    fn test_constants_match_constitution() {
        // ARCH-09: Topic threshold is weighted_agreement >= 2.5
        assert!((TOPIC_THRESHOLD - 2.5).abs() < f32::EPSILON);

        // Max weighted agreement = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
        assert!((MAX_WEIGHTED_AGREEMENT - 8.5).abs() < f32::EPSILON);

        // AP-70: Dream triggers when entropy > 0.7 AND churn > 0.5
        assert!((ENTROPY_THRESHOLD - 0.7).abs() < f32::EPSILON);
        assert!((CHURN_THRESHOLD - 0.5).abs() < f32::EPSILON);

        println!("[PASS] Constants match constitution requirements");
    }

    // ===== Default Impl Tests =====

    #[test]
    fn test_get_topic_portfolio_request_default_impl() {
        let req = GetTopicPortfolioRequest::default();
        assert_eq!(req.format, DEFAULT_FORMAT);
        assert!(req.validate().is_ok());
        println!("[PASS] GetTopicPortfolioRequest::default() produces valid request");
    }

    #[test]
    fn test_get_topic_stability_request_default_impl() {
        let req = GetTopicStabilityRequest::default();
        assert_eq!(req.hours, DEFAULT_STABILITY_HOURS);
        assert!(req.validate().is_ok());
        println!("[PASS] GetTopicStabilityRequest::default() produces valid request");
    }

    #[test]
    fn test_detect_topics_request_default_impl() {
        let req = DetectTopicsRequest::default();
        assert!(!req.force);
        println!("[PASS] DetectTopicsRequest::default() produces valid request");
    }

    #[test]
    fn test_get_divergence_alerts_request_default_impl() {
        let req = GetDivergenceAlertsRequest::default();
        assert_eq!(req.lookback_hours, DEFAULT_DIVERGENCE_LOOKBACK);
        assert!(req.validate().is_ok());
        println!("[PASS] GetDivergenceAlertsRequest::default() produces valid request");
    }

    #[test]
    fn test_valid_formats_constant() {
        for format in GetTopicPortfolioRequest::VALID_FORMATS {
            let req = GetTopicPortfolioRequest {
                format: format.to_string(),
            };
            assert!(
                req.validate().is_ok(),
                "Format '{}' should be valid",
                format
            );
        }
        println!("[PASS] All VALID_FORMATS pass validation");
    }
}
