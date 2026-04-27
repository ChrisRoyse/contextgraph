//! Divergence alert types for topic drift detection.
//!
//! This module provides types for representing divergence alerts when current
//! activity shows LOW similarity to recent work in SEMANTIC embedding spaces.
//!
//! # Architecture Rules
//!
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders
//! - AP-63: NEVER trigger divergence from temporal proximity differences

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::Embedder;

/// Semantic embedding spaces used for divergence detection.
///
/// Per ARCH-10, only SEMANTIC category embedders are checked for divergence.
/// Temporal (E2-E4), Relational (E8, E11), and Structural (E9) are excluded.
///
/// NOTE: E5 (Causal) is semantic but EXCLUDED because AP-77 requires
/// CausalDirection for meaningful scores. Without direction, E5 returns 0.0
/// from compute_embedder_scores_sync(), causing false-positive alerts.
pub const DIVERGENCE_SPACES: [Embedder; 7] = [
    Embedder::Semantic,        // E1
    Embedder::Sparse,          // E6
    Embedder::Code,            // E7
    Embedder::Contextual,      // E10
    Embedder::LateInteraction, // E12
    Embedder::KeywordSplade,   // E13
    Embedder::BgeM3Dense,      // E14
];

/// Maximum length for memory summary in DivergenceAlert.
pub const MAX_SUMMARY_LEN: usize = 100;

/// Severity level of a divergence alert based on similarity score.
///
/// Lower similarity = higher severity (more divergent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DivergenceSeverity {
    /// Score 0.20..0.30 - Minor topic drift
    Low,
    /// Score 0.10..0.20 - Significant topic change
    Medium,
    /// Score 0.00..0.10 - Major topic divergence
    High,
}

impl DivergenceSeverity {
    /// Determine severity from similarity score.
    ///
    /// Lower score = higher severity.
    pub fn from_score(score: f32) -> Self {
        if score < 0.10 {
            Self::High
        } else if score < 0.20 {
            Self::Medium
        } else {
            Self::Low
        }
    }

    /// Get human-readable severity string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

impl std::fmt::Display for DivergenceSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Alert indicating divergence from recent work in a semantic embedding space.
///
/// Created when similarity score falls BELOW the low threshold for a semantic space.
/// Only SEMANTIC embedders (E1, E6, E7, E10, E12, E13) can generate alerts.
/// E5 excluded per AP-77 (requires CausalDirection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceAlert {
    /// ID of the recent memory showing divergence.
    pub memory_id: Uuid,
    /// Semantic embedding space where divergence was detected.
    pub space: Embedder,
    /// Similarity score that triggered the alert (below low threshold).
    pub similarity_score: f32,
    /// Truncated summary of the memory content (max 100 chars).
    pub memory_summary: String,
    /// Timestamp when divergence was detected.
    pub detected_at: DateTime<Utc>,
}

impl DivergenceAlert {
    /// Create a new DivergenceAlert.
    ///
    /// # Arguments
    /// * `memory_id` - UUID of the divergent memory
    /// * `space` - Semantic embedder where divergence was detected (MUST be in DIVERGENCE_SPACES)
    /// * `similarity_score` - The low similarity score that triggered this alert
    /// * `memory_content` - Full content to truncate for summary
    ///
    /// # Panics
    /// Panics if `space` is not in DIVERGENCE_SPACES (semantic embedders only).
    pub fn new(
        memory_id: Uuid,
        space: Embedder,
        similarity_score: f32,
        memory_content: &str,
    ) -> Self {
        // CORE-M3 FIX: Real assertion in both debug AND release builds.
        // debug_assert! was compiled out in release, silently accepting invalid spaces.
        assert!(
            DIVERGENCE_SPACES.contains(&space),
            "E_INVALID_DIVERGENCE_SPACE: DivergenceAlert can only be created for semantic spaces \
             (E1, E6, E7, E10, E12, E13). Got: {:?}. This is a programmer error.",
            space
        );

        Self {
            memory_id,
            space,
            similarity_score: similarity_score.clamp(0.0, 1.0),
            memory_summary: truncate_summary(memory_content, MAX_SUMMARY_LEN),
            detected_at: Utc::now(),
        }
    }

    /// Get the severity level of this alert.
    pub fn severity(&self) -> DivergenceSeverity {
        DivergenceSeverity::from_score(self.similarity_score)
    }

    /// Format alert for display/injection.
    ///
    /// Returns format: `⚠️ DIVERGENCE in {space}: "{summary}" (similarity: {score:.2})`
    pub fn format_alert(&self) -> String {
        format!(
            "⚠️ DIVERGENCE in {}: \"{}\" (similarity: {:.2})",
            self.space.short_name(),
            self.memory_summary,
            self.similarity_score
        )
    }

    /// Format alert with severity prefix.
    ///
    /// Returns format: `[{severity}] ⚠️ DIVERGENCE in {space}: ...`
    pub fn format_with_severity(&self) -> String {
        format!("[{}] {}", self.severity(), self.format_alert())
    }
}

/// Collection of divergence alerts with helper methods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DivergenceReport {
    /// All divergence alerts detected.
    pub alerts: Vec<DivergenceAlert>,
}

impl DivergenceReport {
    /// Create an empty report.
    pub fn new() -> Self {
        Self { alerts: Vec::new() }
    }

    /// Add an alert to the report.
    pub fn add(&mut self, alert: DivergenceAlert) {
        self.alerts.push(alert);
    }

    /// Check if report has no alerts.
    pub fn is_empty(&self) -> bool {
        self.alerts.is_empty()
    }

    /// Get number of alerts.
    pub fn len(&self) -> usize {
        self.alerts.len()
    }

    /// Get the most severe alert (lowest similarity score).
    pub fn most_severe(&self) -> Option<&DivergenceAlert> {
        self.alerts
            .iter()
            .min_by(|a, b| compare_scores(a.similarity_score, b.similarity_score))
    }

    /// Sort alerts by severity (lowest score first = most severe first).
    pub fn sort_by_severity(&mut self) {
        self.alerts
            .sort_by(|a, b| compare_scores(a.similarity_score, b.similarity_score));
    }

    /// Format all alerts for injection, one per line.
    pub fn format_all(&self) -> String {
        if self.alerts.is_empty() {
            return String::new();
        }
        self.alerts
            .iter()
            .map(|a| a.format_alert())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get count of alerts by severity level.
    pub fn count_by_severity(&self) -> (usize, usize, usize) {
        let mut high = 0;
        let mut medium = 0;
        let mut low = 0;
        for alert in &self.alerts {
            match alert.severity() {
                DivergenceSeverity::High => high += 1,
                DivergenceSeverity::Medium => medium += 1,
                DivergenceSeverity::Low => low += 1,
            }
        }
        (high, medium, low)
    }
}

/// Compare two similarity scores for ordering.
///
/// Handles NaN by treating it as greater than all other values,
/// ensuring NaN values sort to the end (least severe).
fn compare_scores(a: f32, b: f32) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or_else(|| {
        // Handle NaN: NaN values sort to the end
        match (a.is_nan(), b.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => unreachable!("partial_cmp only fails for NaN"),
        }
    })
}

/// Truncate content to max_len, preferring word boundaries.
///
/// If content is longer than max_len, truncates and adds "..." suffix.
/// Tries to break at word boundary if possible.
#[allow(clippy::incompatible_msrv)]
pub fn truncate_summary(content: &str, max_len: usize) -> String {
    let trimmed = content.trim();

    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }

    // Reserve 3 chars for "..."
    let target_len = max_len.saturating_sub(3);
    // Snap to nearest valid UTF-8 char boundary to avoid panic on multi-byte chars
    let safe_target = trimmed.floor_char_boundary(target_len);

    // Try to find a word boundary
    if let Some(boundary) = trimmed[..safe_target].rfind(char::is_whitespace) {
        if boundary > safe_target / 2 {
            // Only use word boundary if it's not too short
            return format!("{}...", trimmed[..boundary].trim_end());
        }
    }

    // Fall back to hard truncation at char boundary
    format!("{}...", &trimmed[..safe_target])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::category::category_for;

    #[test]
    fn test_severity_from_score_combined() {
        // High: score < 0.10
        assert_eq!(
            DivergenceSeverity::from_score(0.05),
            DivergenceSeverity::High
        );
        assert_eq!(
            DivergenceSeverity::from_score(0.00),
            DivergenceSeverity::High
        );
        assert_eq!(
            DivergenceSeverity::from_score(0.09),
            DivergenceSeverity::High
        );
        // Medium: 0.10..0.20
        assert_eq!(
            DivergenceSeverity::from_score(0.10),
            DivergenceSeverity::Medium
        );
        assert_eq!(
            DivergenceSeverity::from_score(0.15),
            DivergenceSeverity::Medium
        );
        // Low: >= 0.20
        assert_eq!(
            DivergenceSeverity::from_score(0.20),
            DivergenceSeverity::Low
        );
        assert_eq!(
            DivergenceSeverity::from_score(0.25),
            DivergenceSeverity::Low
        );
        // Display
        assert_eq!(DivergenceSeverity::High.as_str(), "High");
        assert_eq!(format!("{}", DivergenceSeverity::High), "High");
    }

    #[test]
    fn test_divergence_spaces_are_semantic() {
        assert_eq!(DIVERGENCE_SPACES.len(), 7);
        for space in &DIVERGENCE_SPACES {
            let cat = category_for(*space);
            assert!(
                cat.is_semantic(),
                "DIVERGENCE_SPACES contains non-semantic embedder: {:?}",
                space
            );
        }
        // E5 must NOT be included (AP-77)
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Causal));
        // Temporal excluded
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalRecent));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalPeriodic));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalPositional));
        // Relational/structural excluded
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Graph));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Entity));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Hdc));
    }

    #[test]
    fn test_alert_creation() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let alert = DivergenceAlert::new(
            id,
            Embedder::Semantic,
            0.15,
            "Working on memory consolidation algorithm",
        );

        assert_eq!(alert.memory_id, id);
        assert_eq!(alert.space, Embedder::Semantic);
        assert_eq!(alert.similarity_score, 0.15);
        assert_eq!(
            alert.memory_summary,
            "Working on memory consolidation algorithm"
        );

        // Score clamping
        let alert_high = DivergenceAlert::new(id, Embedder::Code, 1.5, "test");
        assert_eq!(alert_high.similarity_score, 1.0);
        let alert_low = DivergenceAlert::new(id, Embedder::Code, -0.5, "test");
        assert_eq!(alert_low.similarity_score, 0.0);
    }

    #[test]
    fn test_alert_severity() {
        let id = Uuid::new_v4();
        let high = DivergenceAlert::new(id, Embedder::Semantic, 0.05, "test");
        assert_eq!(high.severity(), DivergenceSeverity::High);
        let medium = DivergenceAlert::new(id, Embedder::Semantic, 0.15, "test");
        assert_eq!(medium.severity(), DivergenceSeverity::Medium);
        let low = DivergenceAlert::new(id, Embedder::Semantic, 0.25, "test");
        assert_eq!(low.severity(), DivergenceSeverity::Low);

        // Format
        let formatted = high.format_with_severity();
        assert!(formatted.starts_with("[High]"));
    }

    #[test]
    fn test_report_most_severe() {
        let mut report = DivergenceReport::new();
        assert!(report.is_empty());
        assert!(report.most_severe().is_none());

        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.25,
            "low",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Code,
            0.05,
            "high",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Sparse,
            0.15,
            "medium",
        ));

        let most_severe = report.most_severe().unwrap();
        assert_eq!(most_severe.similarity_score, 0.05);
        assert_eq!(most_severe.severity(), DivergenceSeverity::High);
    }

    #[test]
    fn test_report_sort_by_severity() {
        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.25,
            "a",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Code,
            0.05,
            "b",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Sparse,
            0.15,
            "c",
        ));

        report.sort_by_severity();
        assert_eq!(report.alerts[0].similarity_score, 0.05);
        assert_eq!(report.alerts[1].similarity_score, 0.15);
        assert_eq!(report.alerts[2].similarity_score, 0.25);

        let (high, medium, low) = report.count_by_severity();
        assert_eq!(high, 1);
        assert_eq!(medium, 1);
        assert_eq!(low, 1);
    }

    #[test]
    fn test_arch10_divergence_semantic_only() {
        for embedder in Embedder::all() {
            let cat = category_for(embedder);
            if embedder == Embedder::Causal {
                assert!(!DIVERGENCE_SPACES.contains(&embedder));
                continue;
            }
            if cat.used_for_divergence_detection() {
                assert!(
                    DIVERGENCE_SPACES.contains(&embedder),
                    "{:?} should be in DIVERGENCE_SPACES",
                    embedder
                );
            } else {
                assert!(
                    !DIVERGENCE_SPACES.contains(&embedder),
                    "{:?} should NOT be in DIVERGENCE_SPACES",
                    embedder
                );
            }
        }
    }

    #[test]
    fn test_truncate_summary_cases() {
        // Short content unchanged
        assert_eq!(truncate_summary("Hello world", 100), "Hello world");
        // Empty/whitespace
        assert_eq!(truncate_summary("", 100), "");
        assert_eq!(truncate_summary("     ", 100), "");
        // Exact length
        let exact = "a".repeat(100);
        assert_eq!(truncate_summary(&exact, 100).len(), 100);
        // Long content truncated
        let long =
            "This is a long sentence that needs to be truncated at word boundary for readability";
        let result = truncate_summary(long, 50);
        assert!(result.len() <= 50);
        assert!(result.ends_with("..."));
        // No spaces
        let no_spaces = "a".repeat(200);
        let result = truncate_summary(&no_spaces, 50);
        assert_eq!(result.len(), 50);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_alert_serialization_roundtrip() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let alert = DivergenceAlert::new(id, Embedder::Semantic, 0.15, "test content");
        let json = serde_json::to_string(&alert).expect("serialize");
        let recovered: DivergenceAlert = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(recovered.memory_id, id);
        assert_eq!(recovered.space, Embedder::Semantic);
        assert_eq!(recovered.similarity_score, 0.15);

        // Report roundtrip
        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Code,
            0.12,
            "test",
        ));
        let json = serde_json::to_string(&report).expect("serialize");
        let recovered: DivergenceReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(recovered.len(), 1);
    }
}
