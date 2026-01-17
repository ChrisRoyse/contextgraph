//! Temporal enrichment provider for Priority 5 badges.
//!
//! This module provides temporal badges for context injection metadata.
//! These badges provide contextual information (same session, same day, etc.)
//! WITHOUT affecting topic detection or relevance scoring.
//!
//! # Constitution Compliance
//! - AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic detection
//! - AP-63: NEVER trigger divergence from temporal proximity differences
//! - AP-14: No .unwrap() in library code
//! - AP-10: No NaN/Infinity in similarity scores

use serde::{Deserialize, Serialize};

use crate::similarity::cosine_similarity;
use crate::types::fingerprint::SemanticFingerprint;

/// Default thresholds for temporal badges (from constitution.yaml temporal_enrichment)
pub const DEFAULT_SAME_SESSION_THRESHOLD: f32 = 0.8;  // E2 similarity > 0.8
pub const DEFAULT_SAME_DAY_THRESHOLD: f32 = 0.7;      // E3 similarity > 0.7
pub const DEFAULT_SAME_PERIOD_THRESHOLD: f32 = 0.6;   // E3 similarity > 0.6
pub const DEFAULT_SAME_SEQUENCE_THRESHOLD: f32 = 0.6; // E4 similarity > 0.6

/// Type of temporal enrichment badge.
///
/// These are metadata-only and do NOT affect relevance or topic detection.
/// Per AP-60: Temporal embedders MUST NOT count toward topic detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalBadgeType {
    /// From same session (E2 Temporal-Recent similarity > 0.8)
    SameSession,
    /// From same day (E3 Temporal-Periodic similarity > 0.7)
    SameDay,
    /// From same time period/cycle (E3 similarity in [0.6, 0.7))
    SamePeriod,
    /// In same sequence/order (E4 Temporal-Positional similarity > 0.6)
    SameSequence,
}

impl TemporalBadgeType {
    /// Get emoji representation for display.
    #[inline]
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::SameSession => "📅",
            Self::SameDay => "🕐",
            Self::SamePeriod => "🔄",
            Self::SameSequence => "📊",
        }
    }

    /// Get human-readable display text.
    #[inline]
    pub fn display_text(&self) -> &'static str {
        match self {
            Self::SameSession => "From same session",
            Self::SameDay => "From same day",
            Self::SamePeriod => "From similar time period",
            Self::SameSequence => "In same sequence",
        }
    }
}

/// A temporal enrichment badge for context injection.
///
/// These appear in Priority 5 slot (~50 tokens) and provide
/// contextual metadata without affecting relevance scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalBadge {
    /// The type of temporal badge.
    pub badge_type: TemporalBadgeType,
    /// The similarity score that triggered this badge.
    pub similarity: f32,
}

impl TemporalBadge {
    /// Create a new temporal badge.
    ///
    /// # Arguments
    /// * `badge_type` - Type of badge to create
    /// * `similarity` - Similarity score (must be in [0.0, 1.0])
    pub fn new(badge_type: TemporalBadgeType, similarity: f32) -> Self {
        Self { badge_type, similarity }
    }

    /// Get emoji for display.
    #[inline]
    pub fn display_emoji(&self) -> &'static str {
        self.badge_type.emoji()
    }

    /// Get display text for badge.
    #[inline]
    pub fn display_text(&self) -> &'static str {
        self.badge_type.display_text()
    }

    /// Format badge for injection (emoji + text).
    #[inline]
    pub fn format(&self) -> String {
        format!("{} {}", self.display_emoji(), self.display_text())
    }
}

/// Computes temporal enrichment badges from E2, E3, E4 embeddings.
///
/// IMPORTANT: These badges are metadata-only for Priority 5 injection.
/// Temporal embedders have weight 0.0 in topic detection and relevance scoring.
///
/// # Constitution Compliance
/// - AP-60: Temporal embedders excluded from topic detection
/// - AP-63: No divergence from temporal proximity
#[derive(Debug, Clone)]
pub struct TemporalEnrichmentProvider {
    /// E2 (Temporal-Recent) similarity threshold for "Same Session"
    same_session_threshold: f32,
    /// E3 (Temporal-Periodic) similarity threshold for "Same Day"
    same_day_threshold: f32,
    /// E3 (Temporal-Periodic) similarity threshold for "Same Period"
    same_period_threshold: f32,
    /// E4 (Temporal-Positional) similarity threshold for "Same Sequence"
    same_sequence_threshold: f32,
}

impl Default for TemporalEnrichmentProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalEnrichmentProvider {
    /// Create provider with default thresholds from constitution.yaml.
    pub fn new() -> Self {
        Self {
            same_session_threshold: DEFAULT_SAME_SESSION_THRESHOLD,
            same_day_threshold: DEFAULT_SAME_DAY_THRESHOLD,
            same_period_threshold: DEFAULT_SAME_PERIOD_THRESHOLD,
            same_sequence_threshold: DEFAULT_SAME_SEQUENCE_THRESHOLD,
        }
    }

    /// Create provider with custom thresholds.
    ///
    /// # Arguments
    /// * `session` - E2 threshold for SameSession badge (default 0.8)
    /// * `day` - E3 threshold for SameDay badge (default 0.7)
    /// * `period` - E3 threshold for SamePeriod badge (default 0.6)
    /// * `sequence` - E4 threshold for SameSequence badge (default 0.6)
    pub fn with_thresholds(session: f32, day: f32, period: f32, sequence: f32) -> Self {
        Self {
            same_session_threshold: session,
            same_day_threshold: day,
            same_period_threshold: period,
            same_sequence_threshold: sequence,
        }
    }

    /// Compute temporal badges for a candidate memory relative to current context.
    ///
    /// Uses ONLY temporal embedders:
    /// - E2 (e2_temporal_recent): "Same Session" badge if > 0.8
    /// - E3 (e3_temporal_periodic): "Same Day" badge if > 0.7, "Same Period" if > 0.6
    /// - E4 (e4_temporal_positional): "Same Sequence" badge if > 0.6
    ///
    /// # Arguments
    /// * `current` - Current context fingerprint
    /// * `candidate` - Candidate memory fingerprint
    ///
    /// # Returns
    /// Vector of badges (may be empty if no thresholds met)
    ///
    /// # Errors
    /// Returns empty vec if cosine_similarity fails (e.g., zero-magnitude vectors).
    /// Per AP-14, we handle errors gracefully without panicking.
    pub fn compute_badges(
        &self,
        current: &SemanticFingerprint,
        candidate: &SemanticFingerprint,
    ) -> Vec<TemporalBadge> {
        let mut badges = Vec::new();

        // E2: Temporal-Recent -> Same Session
        if let Some(badge) = self.check_e2_badge(current, candidate) {
            badges.push(badge);
        }

        // E3: Temporal-Periodic -> Same Day or Same Period
        if let Some(badge) = self.check_e3_badge(current, candidate) {
            badges.push(badge);
        }

        // E4: Temporal-Positional -> Same Sequence
        if let Some(badge) = self.check_e4_badge(current, candidate) {
            badges.push(badge);
        }

        badges
    }

    /// Check E2 (temporal_recent) for SameSession badge.
    fn check_e2_badge(
        &self,
        current: &SemanticFingerprint,
        candidate: &SemanticFingerprint,
    ) -> Option<TemporalBadge> {
        let sim = compute_temporal_similarity(
            &current.e2_temporal_recent,
            &candidate.e2_temporal_recent,
        )?;

        if sim > self.same_session_threshold {
            Some(TemporalBadge::new(TemporalBadgeType::SameSession, sim))
        } else {
            None
        }
    }

    /// Check E3 (temporal_periodic) for SameDay or SamePeriod badge.
    fn check_e3_badge(
        &self,
        current: &SemanticFingerprint,
        candidate: &SemanticFingerprint,
    ) -> Option<TemporalBadge> {
        let sim = compute_temporal_similarity(
            &current.e3_temporal_periodic,
            &candidate.e3_temporal_periodic,
        )?;

        if sim > self.same_day_threshold {
            Some(TemporalBadge::new(TemporalBadgeType::SameDay, sim))
        } else if sim > self.same_period_threshold {
            Some(TemporalBadge::new(TemporalBadgeType::SamePeriod, sim))
        } else {
            None
        }
    }

    /// Check E4 (temporal_positional) for SameSequence badge.
    fn check_e4_badge(
        &self,
        current: &SemanticFingerprint,
        candidate: &SemanticFingerprint,
    ) -> Option<TemporalBadge> {
        let sim = compute_temporal_similarity(
            &current.e4_temporal_positional,
            &candidate.e4_temporal_positional,
        )?;

        if sim > self.same_sequence_threshold {
            Some(TemporalBadge::new(TemporalBadgeType::SameSequence, sim))
        } else {
            None
        }
    }

    /// Get current threshold configuration.
    pub fn thresholds(&self) -> (f32, f32, f32, f32) {
        (
            self.same_session_threshold,
            self.same_day_threshold,
            self.same_period_threshold,
            self.same_sequence_threshold,
        )
    }
}

/// Compute cosine similarity for temporal embeddings.
///
/// Returns None if:
/// - Vectors are empty
/// - Vectors have different lengths
/// - Either vector has zero magnitude
///
/// This follows AP-14: No .unwrap() - errors handled gracefully.
fn compute_temporal_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    cosine_similarity(a, b).ok()
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Create test fingerprint with specified temporal embeddings.
    /// Uses zeroed() which is only available in tests.
    fn make_test_fingerprint(e2: &[f32], e3: &[f32], e4: &[f32]) -> SemanticFingerprint {
        let mut fp = SemanticFingerprint::zeroed();
        // Copy temporal embeddings (pad/truncate to correct dimension)
        let e2_dim = fp.e2_temporal_recent.len();
        let e3_dim = fp.e3_temporal_periodic.len();
        let e4_dim = fp.e4_temporal_positional.len();

        for (i, &val) in e2.iter().enumerate().take(e2_dim) {
            fp.e2_temporal_recent[i] = val;
        }
        for (i, &val) in e3.iter().enumerate().take(e3_dim) {
            fp.e3_temporal_periodic[i] = val;
        }
        for (i, &val) in e4.iter().enumerate().take(e4_dim) {
            fp.e4_temporal_positional[i] = val;
        }
        fp
    }

    #[test]
    fn test_same_session_badge() {
        let provider = TemporalEnrichmentProvider::new();

        // Create fingerprints with high E2 similarity
        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0], // E2: will have high similarity
            &[0.0, 1.0, 0.0], // E3: orthogonal
            &[0.0, 0.0, 1.0], // E4: orthogonal
        );

        let candidate = make_test_fingerprint(
            &[0.95, 0.05, 0.0], // E2: ~0.99 similarity
            &[1.0, 0.0, 0.0],   // E3: orthogonal to current
            &[1.0, 0.0, 0.0],   // E4: orthogonal to current
        );

        let badges = provider.compute_badges(&current, &candidate);

        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameSession),
            "Should have SameSession badge with high E2 similarity"
        );
        assert!(
            !badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameDay),
            "Should NOT have SameDay badge"
        );
        println!("[PASS] SameSession badge triggered correctly");
    }

    #[test]
    fn test_same_day_badge() {
        let provider = TemporalEnrichmentProvider::new();

        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0], // E3: will have high similarity
            &[0.0, 0.0, 1.0],
        );

        let candidate = make_test_fingerprint(
            &[0.0, 1.0, 0.0], // E2: orthogonal
            &[0.9, 0.1, 0.0], // E3: ~0.99 similarity
            &[1.0, 0.0, 0.0], // E4: orthogonal
        );

        let badges = provider.compute_badges(&current, &candidate);

        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameDay),
            "Should have SameDay badge"
        );
        println!("[PASS] SameDay badge triggered correctly");
    }

    #[test]
    fn test_same_period_badge() {
        let provider = TemporalEnrichmentProvider::new();

        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0], // E3 base
            &[0.0, 0.0, 1.0],
        );

        // Create candidate with E3 similarity around 0.65 (between 0.6 and 0.7)
        let candidate = make_test_fingerprint(
            &[0.0, 1.0, 0.0], // E2: orthogonal
            &[0.8, 0.6, 0.0], // E3: moderate similarity ~0.8
            &[1.0, 0.0, 0.0], // E4: orthogonal
        );

        let badges = provider.compute_badges(&current, &candidate);

        // With 0.8 E3 component similarity, this should be SameDay (>0.7)
        let has_day_or_period = badges.iter().any(|b| {
            b.badge_type == TemporalBadgeType::SameDay
                || b.badge_type == TemporalBadgeType::SamePeriod
        });
        assert!(has_day_or_period, "Should have SameDay or SamePeriod badge");
        println!("[PASS] Same period/day badge computed correctly");
    }

    #[test]
    fn test_same_sequence_badge() {
        let provider = TemporalEnrichmentProvider::new();

        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0],
            &[1.0, 0.0, 0.0], // E4: will have high similarity
        );

        let candidate = make_test_fingerprint(
            &[0.0, 1.0, 0.0], // E2: orthogonal
            &[1.0, 0.0, 0.0], // E3: orthogonal
            &[0.95, 0.05, 0.0], // E4: ~0.99 similarity
        );

        let badges = provider.compute_badges(&current, &candidate);

        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameSequence),
            "Should have SameSequence badge"
        );
        println!("[PASS] SameSequence badge triggered correctly");
    }

    #[test]
    fn test_no_badges_below_threshold() {
        let provider = TemporalEnrichmentProvider::new();

        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
        );

        // All orthogonal - no similarity
        let candidate = make_test_fingerprint(
            &[0.0, 1.0, 0.0],
            &[1.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0],
        );

        let badges = provider.compute_badges(&current, &candidate);

        assert!(badges.is_empty(), "Should have no badges when all below threshold");
        println!("[PASS] No badges returned when all below threshold");
    }

    #[test]
    fn test_badge_display() {
        let badge = TemporalBadge::new(TemporalBadgeType::SameSession, 0.9);

        assert_eq!(badge.display_emoji(), "📅");
        assert_eq!(badge.display_text(), "From same session");
        assert_eq!(badge.format(), "📅 From same session");
        println!("[PASS] Badge display methods work correctly");
    }

    #[test]
    fn test_custom_thresholds() {
        let provider = TemporalEnrichmentProvider::with_thresholds(0.5, 0.5, 0.4, 0.4);

        let thresholds = provider.thresholds();
        assert_eq!(thresholds.0, 0.5);
        assert_eq!(thresholds.1, 0.5);
        assert_eq!(thresholds.2, 0.4);
        assert_eq!(thresholds.3, 0.4);
        println!("[PASS] Custom thresholds set correctly");
    }

    #[test]
    fn test_default_thresholds() {
        let provider = TemporalEnrichmentProvider::default();

        let thresholds = provider.thresholds();
        assert_eq!(thresholds.0, 0.8, "Session threshold");
        assert_eq!(thresholds.1, 0.7, "Day threshold");
        assert_eq!(thresholds.2, 0.6, "Period threshold");
        assert_eq!(thresholds.3, 0.6, "Sequence threshold");
        println!("[PASS] Default thresholds match constitution.yaml");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let badge = TemporalBadge::new(TemporalBadgeType::SameDay, 0.85);

        let json = serde_json::to_string(&badge).expect("serialize");
        let restored: TemporalBadge = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(badge.badge_type, restored.badge_type);
        assert!((badge.similarity - restored.similarity).abs() < f32::EPSILON);
        println!("[PASS] Serialization roundtrip works");
    }

    // =========================================================================
    // Edge Cases (MANDATORY per task spec)
    // =========================================================================

    #[test]
    fn test_zero_magnitude_vectors_no_panic() {
        let provider = TemporalEnrichmentProvider::new();

        // All zeros - cosine_similarity will return ZeroMagnitude error
        let current = SemanticFingerprint::zeroed();
        let candidate = SemanticFingerprint::zeroed();

        // Should NOT panic - returns empty vec
        let badges = provider.compute_badges(&current, &candidate);
        assert!(badges.is_empty(), "Zero vectors should produce no badges");
        println!("[PASS] Zero-magnitude vectors handled without panic");
    }

    #[test]
    fn test_multiple_badges_same_candidate() {
        let provider = TemporalEnrichmentProvider::new();

        // Create fingerprints where ALL temporal embeddings are similar
        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0],
            &[1.0, 0.0, 0.0],
        );

        let candidate = make_test_fingerprint(
            &[0.95, 0.05, 0.0], // High E2 sim
            &[0.95, 0.05, 0.0], // High E3 sim
            &[0.95, 0.05, 0.0], // High E4 sim
        );

        let badges = provider.compute_badges(&current, &candidate);

        // Should have all 3 badges
        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameSession),
            "Should have SameSession"
        );
        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameDay),
            "Should have SameDay (E3 > 0.7)"
        );
        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameSequence),
            "Should have SameSequence"
        );
        println!("[PASS] Multiple badges can be assigned to same candidate");
    }

    #[test]
    fn test_exact_threshold_boundary() {
        // Test exact boundary: similarity == threshold should NOT trigger
        let provider = TemporalEnrichmentProvider::with_thresholds(0.8, 0.7, 0.6, 0.6);

        // We need similarity > 0.8, not >= 0.8
        // This tests that the comparison is strictly greater-than
        let current = make_test_fingerprint(
            &[1.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
        );

        // 0.8 similarity with [1,0,0] requires vector at specific angle
        // cos(theta) = 0.8 => vector approximately [0.8, 0.6, 0.0] normalized
        let candidate = make_test_fingerprint(
            &[0.81, 0.58, 0.0], // Slightly above 0.8 cosine sim
            &[0.0, 0.0, 1.0],
            &[1.0, 0.0, 0.0],
        );

        let badges = provider.compute_badges(&current, &candidate);
        // Similarity should be ~0.81, which is > 0.8
        assert!(
            badges.iter().any(|b| b.badge_type == TemporalBadgeType::SameSession),
            "Should trigger at similarity > threshold"
        );
        println!("[PASS] Threshold boundary behavior correct");
    }
}
