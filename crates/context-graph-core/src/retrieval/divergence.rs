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
pub const DIVERGENCE_SPACES: [Embedder; 7] = [
    Embedder::Semantic,        // E1
    Embedder::Causal,          // E5
    Embedder::Sparse,          // E6
    Embedder::Code,            // E7
    Embedder::Multimodal,      // E10
    Embedder::LateInteraction, // E12
    Embedder::KeywordSplade,   // E13
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
/// Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) can generate alerts.
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
    /// Panics in debug mode if `space` is not a semantic embedder.
    pub fn new(
        memory_id: Uuid,
        space: Embedder,
        similarity_score: f32,
        memory_content: &str,
    ) -> Self {
        // Verify space is semantic (debug-only check for performance)
        debug_assert!(
            DIVERGENCE_SPACES.contains(&space),
            "DivergenceAlert can only be created for semantic spaces. Got: {:?}",
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
pub fn truncate_summary(content: &str, max_len: usize) -> String {
    let trimmed = content.trim();

    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }

    // Reserve 3 chars for "..."
    let target_len = max_len.saturating_sub(3);

    // Try to find a word boundary
    if let Some(boundary) = trimmed[..target_len].rfind(char::is_whitespace) {
        if boundary > target_len / 2 {
            // Only use word boundary if it's not too short
            return format!("{}...", trimmed[..boundary].trim_end());
        }
    }

    // Fall back to hard truncation
    format!("{}...", &trimmed[..target_len])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::category::category_for;

    // =========================================================================
    // DivergenceSeverity Tests
    // =========================================================================

    #[test]
    fn test_severity_from_score_high() {
        assert_eq!(DivergenceSeverity::from_score(0.05), DivergenceSeverity::High);
        assert_eq!(DivergenceSeverity::from_score(0.00), DivergenceSeverity::High);
        assert_eq!(DivergenceSeverity::from_score(0.09), DivergenceSeverity::High);
        println!("[PASS] Score < 0.10 -> High severity");
    }

    #[test]
    fn test_severity_from_score_medium() {
        assert_eq!(DivergenceSeverity::from_score(0.10), DivergenceSeverity::Medium);
        assert_eq!(DivergenceSeverity::from_score(0.15), DivergenceSeverity::Medium);
        assert_eq!(DivergenceSeverity::from_score(0.19), DivergenceSeverity::Medium);
        println!("[PASS] Score 0.10..0.20 -> Medium severity");
    }

    #[test]
    fn test_severity_from_score_low() {
        assert_eq!(DivergenceSeverity::from_score(0.20), DivergenceSeverity::Low);
        assert_eq!(DivergenceSeverity::from_score(0.25), DivergenceSeverity::Low);
        assert_eq!(DivergenceSeverity::from_score(0.29), DivergenceSeverity::Low);
        println!("[PASS] Score >= 0.20 -> Low severity");
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(DivergenceSeverity::High.as_str(), "High");
        assert_eq!(DivergenceSeverity::Medium.as_str(), "Medium");
        assert_eq!(DivergenceSeverity::Low.as_str(), "Low");
        assert_eq!(format!("{}", DivergenceSeverity::High), "High");
        println!("[PASS] DivergenceSeverity display works");
    }

    // =========================================================================
    // DIVERGENCE_SPACES Constant Tests
    // =========================================================================

    #[test]
    fn test_divergence_spaces_count() {
        assert_eq!(DIVERGENCE_SPACES.len(), 7);
        println!("[PASS] DIVERGENCE_SPACES has exactly 7 semantic embedders");
    }

    #[test]
    fn test_divergence_spaces_are_semantic() {
        for space in &DIVERGENCE_SPACES {
            let cat = category_for(*space);
            assert!(
                cat.is_semantic(),
                "DIVERGENCE_SPACES contains non-semantic embedder: {:?}",
                space
            );
        }
        println!("[PASS] All DIVERGENCE_SPACES are semantic category");
    }

    #[test]
    fn test_divergence_spaces_excludes_temporal() {
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalRecent));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalPeriodic));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::TemporalPositional));
        println!("[PASS] DIVERGENCE_SPACES excludes all temporal embedders");
    }

    #[test]
    fn test_divergence_spaces_excludes_relational() {
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Emotional));
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Entity));
        println!("[PASS] DIVERGENCE_SPACES excludes relational embedders");
    }

    #[test]
    fn test_divergence_spaces_excludes_structural() {
        assert!(!DIVERGENCE_SPACES.contains(&Embedder::Hdc));
        println!("[PASS] DIVERGENCE_SPACES excludes structural embedder");
    }

    #[test]
    fn test_divergence_spaces_contains_all_semantic() {
        // Verify all 7 semantic embedders are present
        let expected = [
            Embedder::Semantic,
            Embedder::Causal,
            Embedder::Sparse,
            Embedder::Code,
            Embedder::Multimodal,
            Embedder::LateInteraction,
            Embedder::KeywordSplade,
        ];
        for e in &expected {
            assert!(
                DIVERGENCE_SPACES.contains(e),
                "{:?} not in DIVERGENCE_SPACES",
                e
            );
        }
        println!("[PASS] DIVERGENCE_SPACES contains all 7 semantic embedders");
    }

    // =========================================================================
    // DivergenceAlert Tests
    // =========================================================================

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
        assert_eq!(alert.memory_summary, "Working on memory consolidation algorithm");
        println!("[PASS] DivergenceAlert creation works");
    }

    #[test]
    fn test_alert_score_clamping() {
        let id = Uuid::new_v4();

        let alert_high = DivergenceAlert::new(id, Embedder::Code, 1.5, "test");
        assert_eq!(alert_high.similarity_score, 1.0);

        let alert_low = DivergenceAlert::new(id, Embedder::Code, -0.5, "test");
        assert_eq!(alert_low.similarity_score, 0.0);

        println!("[PASS] DivergenceAlert clamps score to [0.0, 1.0]");
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

        println!("[PASS] DivergenceAlert.severity() computes correctly");
    }

    #[test]
    fn test_alert_format() {
        let id = Uuid::new_v4();
        let alert = DivergenceAlert::new(
            id,
            Embedder::Code,
            0.15,
            "Implementing test suite",
        );

        let formatted = alert.format_alert();
        assert!(formatted.contains("DIVERGENCE"));
        assert!(formatted.contains("E7"));
        assert!(formatted.contains("Implementing test suite"));
        assert!(formatted.contains("0.15"));
        println!("[PASS] format_alert() produces expected output: {}", formatted);
    }

    #[test]
    fn test_alert_format_with_severity() {
        let id = Uuid::new_v4();
        let alert = DivergenceAlert::new(id, Embedder::Semantic, 0.05, "test");

        let formatted = alert.format_with_severity();
        assert!(formatted.starts_with("[High]"));
        println!("[PASS] format_with_severity() includes severity prefix");
    }

    // =========================================================================
    // truncate_summary Tests
    // =========================================================================

    #[test]
    fn test_truncate_short_content() {
        let short = "Hello world";
        let result = truncate_summary(short, 100);
        assert_eq!(result, "Hello world");
        println!("[PASS] Short content not truncated");
    }

    #[test]
    fn test_truncate_exact_length() {
        let exact = "a".repeat(100);
        let result = truncate_summary(&exact, 100);
        assert_eq!(result.len(), 100);
        assert!(!result.contains("..."));
        println!("[PASS] Exact length content not truncated");
    }

    #[test]
    fn test_truncate_long_content_word_boundary() {
        let long = "This is a long sentence that needs to be truncated at word boundary for readability";
        let result = truncate_summary(long, 50);
        assert!(result.len() <= 50);
        assert!(result.ends_with("..."));
        assert!(!result.ends_with(" ..."));  // No trailing space
        println!("[PASS] Long content truncated at word boundary: {}", result);
    }

    #[test]
    fn test_truncate_no_spaces() {
        let no_spaces = "a".repeat(200);
        let result = truncate_summary(&no_spaces, 50);
        assert_eq!(result.len(), 50);
        assert!(result.ends_with("..."));
        println!("[PASS] Content without spaces truncated to exact length");
    }

    #[test]
    fn test_truncate_trims_whitespace() {
        let padded = "   content with spaces   ";
        let result = truncate_summary(padded, 100);
        assert_eq!(result, "content with spaces");
        println!("[PASS] Whitespace is trimmed");
    }

    #[test]
    fn test_truncate_empty_content() {
        let empty = "";
        let result = truncate_summary(empty, 100);
        assert_eq!(result, "");
        println!("[PASS] Empty content returns empty string");
    }

    #[test]
    fn test_truncate_whitespace_only() {
        let whitespace = "     ";
        let result = truncate_summary(whitespace, 100);
        assert_eq!(result, "");
        println!("[PASS] Whitespace-only content returns empty string");
    }

    // =========================================================================
    // DivergenceReport Tests
    // =========================================================================

    #[test]
    fn test_report_empty() {
        let report = DivergenceReport::new();
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(report.most_severe().is_none());
        assert_eq!(report.format_all(), "");
        println!("[PASS] Empty DivergenceReport works");
    }

    #[test]
    fn test_report_add_alert() {
        let mut report = DivergenceReport::new();
        let id = Uuid::new_v4();

        report.add(DivergenceAlert::new(id, Embedder::Semantic, 0.15, "test"));

        assert!(!report.is_empty());
        assert_eq!(report.len(), 1);
        println!("[PASS] DivergenceReport.add() works");
    }

    #[test]
    fn test_report_most_severe() {
        let mut report = DivergenceReport::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        report.add(DivergenceAlert::new(id1, Embedder::Semantic, 0.25, "low"));
        report.add(DivergenceAlert::new(id2, Embedder::Code, 0.05, "high"));  // Most severe
        report.add(DivergenceAlert::new(id3, Embedder::Causal, 0.15, "medium"));

        let most_severe = report.most_severe().unwrap();
        assert_eq!(most_severe.similarity_score, 0.05);
        assert_eq!(most_severe.severity(), DivergenceSeverity::High);
        println!("[PASS] most_severe() returns lowest score alert");
    }

    #[test]
    fn test_report_sort_by_severity() {
        let mut report = DivergenceReport::new();

        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Semantic, 0.25, "a"));
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.05, "b"));
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Causal, 0.15, "c"));

        report.sort_by_severity();

        assert_eq!(report.alerts[0].similarity_score, 0.05);  // Most severe first
        assert_eq!(report.alerts[1].similarity_score, 0.15);
        assert_eq!(report.alerts[2].similarity_score, 0.25);  // Least severe last
        println!("[PASS] sort_by_severity() orders by ascending score");
    }

    #[test]
    fn test_report_format_all() {
        let mut report = DivergenceReport::new();

        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Semantic, 0.15, "first"));
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.20, "second"));

        let formatted = report.format_all();
        let lines: Vec<&str> = formatted.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("E1"));
        assert!(lines[1].contains("E7"));
        println!("[PASS] format_all() produces one line per alert:\n{}", formatted);
    }

    #[test]
    fn test_report_count_by_severity() {
        let mut report = DivergenceReport::new();

        // 2 high (score < 0.10)
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Semantic, 0.05, "h1"));
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.08, "h2"));

        // 1 medium (0.10 <= score < 0.20)
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Causal, 0.15, "m1"));

        // 2 low (score >= 0.20)
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Sparse, 0.22, "l1"));
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Multimodal, 0.28, "l2"));

        let (high, medium, low) = report.count_by_severity();
        assert_eq!(high, 2);
        assert_eq!(medium, 1);
        assert_eq!(low, 2);
        println!("[PASS] count_by_severity() returns (2, 1, 2)");
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_alert_serialization_roundtrip() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let alert = DivergenceAlert::new(id, Embedder::Semantic, 0.15, "test content");

        let json = serde_json::to_string(&alert).expect("serialize");
        let recovered: DivergenceAlert = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(recovered.memory_id, id);
        assert_eq!(recovered.space, Embedder::Semantic);
        assert_eq!(recovered.similarity_score, 0.15);
        println!("[PASS] DivergenceAlert JSON roundtrip works");
    }

    #[test]
    fn test_report_serialization_roundtrip() {
        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(Uuid::new_v4(), Embedder::Code, 0.12, "test"));

        let json = serde_json::to_string(&report).expect("serialize");
        let recovered: DivergenceReport = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(recovered.len(), 1);
        println!("[PASS] DivergenceReport JSON roundtrip works");
    }

    #[test]
    fn test_severity_serialization() {
        let severity = DivergenceSeverity::High;
        let json = serde_json::to_string(&severity).expect("serialize");
        let recovered: DivergenceSeverity = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(recovered, DivergenceSeverity::High);
        println!("[PASS] DivergenceSeverity JSON roundtrip works");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_alert_boundary_scores() {
        let id = Uuid::new_v4();

        // Test exact boundary values
        let at_zero = DivergenceAlert::new(id, Embedder::Semantic, 0.0, "test");
        assert_eq!(at_zero.severity(), DivergenceSeverity::High);
        assert_eq!(at_zero.similarity_score, 0.0);

        let at_one = DivergenceAlert::new(id, Embedder::Semantic, 1.0, "test");
        assert_eq!(at_one.severity(), DivergenceSeverity::Low);
        assert_eq!(at_one.similarity_score, 1.0);

        // Test exact threshold boundaries
        let at_0_10 = DivergenceAlert::new(id, Embedder::Semantic, 0.10, "test");
        assert_eq!(at_0_10.severity(), DivergenceSeverity::Medium);

        let at_0_20 = DivergenceAlert::new(id, Embedder::Semantic, 0.20, "test");
        assert_eq!(at_0_20.severity(), DivergenceSeverity::Low);

        println!("[PASS] Boundary scores handled correctly");
    }

    #[test]
    fn test_truncate_at_exact_boundary() {
        // Content that is exactly 103 chars should truncate to 100
        let content = "a".repeat(103);
        let result = truncate_summary(&content, 100);
        assert_eq!(result.len(), 100);
        assert!(result.ends_with("..."));
        assert_eq!(&result[..97], &"a".repeat(97));
        println!("[PASS] Exact boundary truncation works");
    }

    #[test]
    fn test_report_single_alert() {
        let mut report = DivergenceReport::new();
        let id = Uuid::new_v4();
        report.add(DivergenceAlert::new(id, Embedder::Semantic, 0.12, "single"));

        let most_severe = report.most_severe().unwrap();
        assert_eq!(most_severe.memory_id, id);

        report.sort_by_severity();
        assert_eq!(report.alerts.len(), 1);
        assert_eq!(report.alerts[0].memory_id, id);

        println!("[PASS] Single alert report works correctly");
    }

    // =========================================================================
    // Constitution Compliance Tests
    // =========================================================================

    #[test]
    fn test_arch_10_divergence_semantic_only() {
        // ARCH-10: Divergence detection uses SEMANTIC embedders only
        for embedder in Embedder::all() {
            let cat = category_for(embedder);
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
        println!("[PASS] ARCH-10 compliance verified: DIVERGENCE_SPACES matches category.used_for_divergence_detection()");
    }

    #[test]
    fn test_ap62_divergence_alerts_semantic_only() {
        // AP-62: Divergence alerts MUST only use SEMANTIC embedders
        // All embedders in DIVERGENCE_SPACES must be semantic
        for space in &DIVERGENCE_SPACES {
            let cat = category_for(*space);
            assert!(
                cat.is_semantic(),
                "AP-62 violation: {:?} in DIVERGENCE_SPACES is not semantic",
                space
            );
        }
        println!("[PASS] AP-62 verified: All DIVERGENCE_SPACES are semantic");
    }

    #[test]
    fn test_ap63_no_temporal_divergence() {
        // AP-63: NEVER trigger divergence from temporal proximity differences
        for embedder in [Embedder::TemporalRecent, Embedder::TemporalPeriodic, Embedder::TemporalPositional] {
            assert!(
                !DIVERGENCE_SPACES.contains(&embedder),
                "AP-63 violation: {:?} should not be in DIVERGENCE_SPACES",
                embedder
            );
        }
        println!("[PASS] AP-63 verified: No temporal embedders in DIVERGENCE_SPACES");
    }
}
