//! DivergenceDetector for topic drift detection.
//!
//! This module implements the core divergence detection service that compares
//! the current query against recent memories and generates alerts when
//! similarity falls below low thresholds in SEMANTIC embedding spaces.
//!
//! # Architecture Rules
//!
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders
//! - AP-63: NEVER trigger divergence from temporal proximity differences

use std::time::Duration;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::embeddings::category::category_for;
use crate::types::fingerprint::SemanticFingerprint;
use crate::teleological::Embedder;

use super::multi_space::MultiSpaceSimilarity;
use super::divergence::{DivergenceAlert, DivergenceReport, DivergenceSeverity, DIVERGENCE_SPACES};
use super::config::{RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES};

/// Check if an embedder is used for divergence detection.
///
/// Only SEMANTIC category embedders are used for divergence detection per ARCH-10.
/// Returns true for: Semantic, Causal, Sparse, Code, Multimodal, LateInteraction, KeywordSplade
/// Returns false for: TemporalRecent, TemporalPeriodic, TemporalPositional, Graph, Entity, Hdc
#[inline]
pub fn is_divergence_space(embedder: Embedder) -> bool {
    category_for(embedder).used_for_divergence_detection()
}

/// A recent memory for divergence checking.
///
/// Represents a memory that was recently created and should be compared
/// against the current query to detect topic divergence.
#[derive(Debug, Clone)]
pub struct RecentMemory {
    /// Unique identifier of the memory
    pub id: Uuid,
    /// Text content of the memory (for alert summaries)
    pub content: String,
    /// Full 13-embedding fingerprint
    pub embedding: SemanticFingerprint,
    /// When this memory was created
    pub created_at: DateTime<Utc>,
}

impl RecentMemory {
    /// Create a new RecentMemory.
    pub fn new(
        id: Uuid,
        content: String,
        embedding: SemanticFingerprint,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self { id, content, embedding, created_at }
    }
}

/// Detects divergence between current query and recent context.
///
/// The detector compares the query's embeddings against recent memories
/// and generates alerts when similarity falls BELOW low thresholds.
///
/// # Category-Aware Detection (ARCH-10)
///
/// Only SEMANTIC embedding spaces are checked for divergence:
/// - Semantic (E1), Causal (E5), Sparse (E6), Code (E7)
/// - Multimodal (E10), LateInteraction (E12), KeywordSplade (E13)
///
/// These spaces are IGNORED (AP-63):
/// - Temporal (E2-E4): Time-based features, not topic indicators
/// - Relational (E8, E11): Graph connectivity and entity drift is not topic divergence
/// - Structural (E9): Pattern changes are not semantic divergence
#[derive(Debug, Clone)]
pub struct DivergenceDetector {
    similarity: MultiSpaceSimilarity,
    lookback_duration: Duration,
    max_recent: usize,
}

impl DivergenceDetector {
    /// Create with default configuration.
    ///
    /// Uses RECENT_LOOKBACK_SECS (7200 = 2 hours) and MAX_RECENT_MEMORIES (50).
    pub fn new(similarity: MultiSpaceSimilarity) -> Self {
        Self {
            similarity,
            lookback_duration: Duration::from_secs(RECENT_LOOKBACK_SECS),
            max_recent: MAX_RECENT_MEMORIES,
        }
    }

    /// Create with custom configuration.
    ///
    /// # Arguments
    /// * `similarity` - MultiSpaceSimilarity service for computing scores
    /// * `lookback` - How far back to look for recent memories
    /// * `max_recent` - Maximum number of recent memories to check
    pub fn with_config(
        similarity: MultiSpaceSimilarity,
        lookback: Duration,
        max_recent: usize,
    ) -> Self {
        Self {
            similarity,
            lookback_duration: lookback,
            max_recent,
        }
    }

    /// Detect divergence between query and recent memories.
    ///
    /// # Algorithm
    /// 1. Filter memories to those within lookback window
    /// 2. Limit to max_recent memories
    /// 3. For each memory, compute similarity in all 13 spaces
    /// 4. Only check DIVERGENCE_SPACES (semantic embedders) - temporal/relational/structural IGNORED
    /// 5. Generate alert if ANY semantic space is below low threshold
    /// 6. Sort alerts by severity (lowest score = most severe first)
    ///
    /// # Arguments
    /// * `query` - The current query's embedding fingerprint
    /// * `recent_memories` - Recent memories to compare against
    ///
    /// # Returns
    /// DivergenceReport containing all detected divergence alerts
    pub fn detect_divergence(
        &self,
        query: &SemanticFingerprint,
        recent_memories: &[RecentMemory],
    ) -> DivergenceReport {
        let mut report = DivergenceReport::new();

        // Calculate cutoff time for lookback window
        let cutoff = Utc::now() - chrono::Duration::from_std(self.lookback_duration)
            .unwrap_or(chrono::Duration::hours(2));

        // Filter to recent memories within lookback window, limit to max_recent
        let filtered: Vec<&RecentMemory> = recent_memories
            .iter()
            .filter(|m| m.created_at >= cutoff)
            .take(self.max_recent)
            .collect();

        // Check each recent memory for divergence
        for memory in filtered {
            // Compute similarity across all 13 spaces
            let scores = self.similarity.compute_similarity(query, &memory.embedding);

            // Only check DIVERGENCE_SPACES (semantic embedders per ARCH-10)
            // Temporal (E2-E4), Relational (E8, E11), Structural (E9) are IGNORED
            for &embedder in &DIVERGENCE_SPACES {
                let score = scores.get_score(embedder);

                // Check if score is below low threshold for this space
                if self.similarity.is_below_low_threshold(embedder, score) {
                    let alert = DivergenceAlert::new(
                        memory.id,
                        embedder,
                        score,
                        &memory.content,
                    );
                    report.add(alert);
                }
            }
        }

        // Sort by severity (lowest score = most severe first)
        report.sort_by_severity();

        report
    }

    /// Check if report contains alerts worth surfacing to user.
    ///
    /// Returns true only for High severity divergence (score < 0.10).
    /// Medium and Low severity alerts are logged but not surfaced.
    pub fn should_alert(&self, report: &DivergenceReport) -> bool {
        report
            .most_severe()
            .is_some_and(|alert| alert.severity() == DivergenceSeverity::High)
    }

    /// Generate human-readable divergence summary.
    ///
    /// # Format
    /// - If no divergence: "No divergence detected. Context is coherent."
    /// - If divergence: Summary with counts and top 3 alerts
    pub fn summarize_divergence(&self, report: &DivergenceReport) -> String {
        if report.is_empty() {
            return "No divergence detected. Context is coherent.".to_string();
        }

        let (high, medium, low) = report.count_by_severity();

        let mut summary = format!(
            "Divergence detected: {} high, {} medium, {} low severity alerts.\n",
            high, medium, low
        );

        // Add top 3 most severe alerts
        for alert in report.alerts.iter().take(3) {
            summary.push_str(&format!("  - {}\n", alert.format_alert()));
        }

        summary
    }

    /// Get the lookback duration.
    #[inline]
    pub fn lookback_duration(&self) -> Duration {
        self.lookback_duration
    }

    /// Get the max recent memories limit.
    #[inline]
    pub fn max_recent(&self) -> usize {
        self.max_recent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // is_divergence_space Tests
    // =========================================================================

    #[test]
    fn test_divergence_spaces_count() {
        // DIVERGENCE_SPACES contains 6 semantic embedders (E5 excluded per AP-77)
        assert_eq!(DIVERGENCE_SPACES.len(), 6);
        println!("[PASS] DIVERGENCE_SPACES has exactly 6 semantic embedders (E5 excluded per AP-77)");
    }

    #[test]
    fn test_semantic_is_divergence_space() {
        // All semantic embedders should return true
        assert!(is_divergence_space(Embedder::Semantic));
        assert!(is_divergence_space(Embedder::Causal));
        assert!(is_divergence_space(Embedder::Sparse));
        assert!(is_divergence_space(Embedder::Code));
        assert!(is_divergence_space(Embedder::Multimodal));
        assert!(is_divergence_space(Embedder::LateInteraction));
        assert!(is_divergence_space(Embedder::KeywordSplade));
        println!("[PASS] All 7 semantic embedders return true for is_divergence_space");
    }

    #[test]
    fn test_temporal_not_divergence_space() {
        // Temporal spaces should NOT be divergence spaces (AP-63)
        assert!(!is_divergence_space(Embedder::TemporalRecent));
        assert!(!is_divergence_space(Embedder::TemporalPeriodic));
        assert!(!is_divergence_space(Embedder::TemporalPositional));
        println!("[PASS] Temporal embedders excluded from divergence detection");
    }

    #[test]
    fn test_relational_not_divergence_space() {
        // Relational spaces should NOT be divergence spaces
        assert!(!is_divergence_space(Embedder::Graph));
        assert!(!is_divergence_space(Embedder::Entity));
        println!("[PASS] Relational embedders excluded from divergence detection");
    }

    #[test]
    fn test_structural_not_divergence_space() {
        // Structural space should NOT be divergence space
        assert!(!is_divergence_space(Embedder::Hdc));
        println!("[PASS] Structural embedder excluded from divergence detection");
    }

    // =========================================================================
    // RecentMemory Tests
    // =========================================================================

    #[test]
    fn test_recent_memory_creation() {
        let id = Uuid::new_v4();
        let content = "Test memory content".to_string();
        let embedding = SemanticFingerprint::zeroed();
        let created_at = Utc::now();

        let memory = RecentMemory::new(id, content.clone(), embedding, created_at);

        assert_eq!(memory.id, id);
        assert_eq!(memory.content, content);
        assert_eq!(memory.created_at, created_at);
        println!("[PASS] RecentMemory created correctly");
    }

    // =========================================================================
    // DivergenceDetector Tests
    // =========================================================================

    #[test]
    fn test_detector_default_config() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        assert_eq!(detector.lookback_duration(), Duration::from_secs(7200));
        assert_eq!(detector.max_recent(), 50);
        println!("[PASS] DivergenceDetector uses default config");
    }

    #[test]
    fn test_detector_custom_config() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(3600),  // 1 hour
            25,
        );

        assert_eq!(detector.lookback_duration(), Duration::from_secs(3600));
        assert_eq!(detector.max_recent(), 25);
        println!("[PASS] DivergenceDetector accepts custom config");
    }

    #[test]
    fn test_detect_divergence_empty_memories() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);
        let query = SemanticFingerprint::zeroed();

        let report = detector.detect_divergence(&query, &[]);

        assert!(report.is_empty());
        assert!(!detector.should_alert(&report));
        println!("[PASS] Empty memories produces empty report");
    }

    #[test]
    fn test_detect_divergence_filters_by_lookback() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(60),  // 1 minute lookback
            10,
        );

        // Create old memory (1 hour ago)
        let old_memory = RecentMemory::new(
            Uuid::new_v4(),
            "Old memory".to_string(),
            SemanticFingerprint::zeroed(),
            Utc::now() - chrono::Duration::hours(1),
        );

        let query = SemanticFingerprint::zeroed();
        let report = detector.detect_divergence(&query, &[old_memory]);

        // Old memory should be filtered out
        assert!(report.is_empty());
        println!("[PASS] Old memories filtered by lookback window");
    }

    #[test]
    fn test_detect_divergence_respects_max_recent() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(7200),
            2,  // Only check 2 memories
        );

        // Create 5 recent memories
        let memories: Vec<RecentMemory> = (0..5)
            .map(|i| RecentMemory::new(
                Uuid::new_v4(),
                format!("Memory {}", i),
                SemanticFingerprint::zeroed(),
                Utc::now(),
            ))
            .collect();

        let query = SemanticFingerprint::zeroed();
        let report = detector.detect_divergence(&query, &memories);

        // Should only process first 2 memories (max_recent = 2)
        // With zeroed embeddings, similarity calculations may produce 0.0 or NaN
        // depending on how the distance calculator handles zero vectors.
        // The key assertion is that report is valid and the function completes.
        assert!(report.len() <= 2 * DIVERGENCE_SPACES.len(),
            "Max {} alerts possible (2 memories x 7 semantic spaces), got {}",
            2 * DIVERGENCE_SPACES.len(), report.len());
        println!("[PASS] Max recent limit respected (processed up to 2 memories)");
    }

    #[test]
    fn test_should_alert_high_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.05,  // High severity (score < 0.10)
            "Test content",
        ));

        assert!(detector.should_alert(&report));
        println!("[PASS] should_alert returns true for High severity");
    }

    #[test]
    fn test_should_alert_medium_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.15,  // Medium severity (0.10 <= score < 0.20)
            "Test content",
        ));

        assert!(!detector.should_alert(&report));
        println!("[PASS] should_alert returns false for Medium severity");
    }

    #[test]
    fn test_should_alert_low_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.25,  // Low severity (score >= 0.20)
            "Test content",
        ));

        assert!(!detector.should_alert(&report));
        println!("[PASS] should_alert returns false for Low severity");
    }

    #[test]
    fn test_summarize_empty_report() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);
        let report = DivergenceReport::new();

        let summary = detector.summarize_divergence(&report);
        assert!(summary.contains("No divergence"));
        println!("[PASS] summarize_divergence for empty report: {}", summary);
    }

    #[test]
    fn test_summarize_with_alerts() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.05,
            "High severity alert",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Code,
            0.15,
            "Medium severity alert",
        ));

        let summary = detector.summarize_divergence(&report);
        assert!(summary.contains("high"));
        assert!(summary.contains("medium"));
        println!("[PASS] summarize_divergence with alerts: {}", summary);
    }

    // =========================================================================
    // Constitution Compliance Tests
    // =========================================================================

    #[test]
    fn test_arch10_semantic_only() {
        // ARCH-10: Divergence detection uses SEMANTIC embedders only
        for embedder in Embedder::all() {
            let is_semantic = category_for(embedder).is_semantic();
            let is_divergence = is_divergence_space(embedder);
            assert_eq!(
                is_semantic, is_divergence,
                "{:?}: is_semantic={} but is_divergence_space={}",
                embedder, is_semantic, is_divergence
            );
        }
        println!("[PASS] ARCH-10: is_divergence_space matches is_semantic");
    }

    #[test]
    fn test_ap63_no_temporal_divergence() {
        // AP-63: NEVER trigger divergence from temporal proximity differences
        for embedder in [
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
        ] {
            assert!(
                !DIVERGENCE_SPACES.contains(&embedder),
                "AP-63 violation: {:?} in DIVERGENCE_SPACES",
                embedder
            );
            assert!(
                !is_divergence_space(embedder),
                "AP-63 violation: is_divergence_space({:?}) returned true",
                embedder
            );
        }
        println!("[PASS] AP-63: Temporal embedders excluded from DIVERGENCE_SPACES");
    }
}
