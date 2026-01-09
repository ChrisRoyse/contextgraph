//! Misalignment detection and flags.
//!
//! Provides flags for detecting specific misalignment patterns that indicate
//! structural issues in the goal alignment, not just low scores.
//!
//! # Misalignment Patterns
//!
//! - `tactical_without_strategic`: High Tactical alignment but low Strategic
//! - `divergent_hierarchy`: Child goals diverge from parent alignment direction
//! - `below_threshold`: Any goal below the Critical threshold
//! - `inconsistent_alignment`: High variance across embedding spaces

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Flags indicating specific misalignment patterns.
///
/// These flags go beyond simple threshold checks to identify structural
/// issues in goal alignment that require intervention.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MisalignmentFlags {
    /// High Tactical alignment but low Strategic alignment.
    ///
    /// Indicates short-term focus without strategic direction.
    /// Threshold: Tactical > 0.7 AND Strategic < 0.5
    pub tactical_without_strategic: bool,

    /// Child goals diverge from parent alignment direction.
    ///
    /// Indicates hierarchical inconsistency where children don't
    /// serve their parent goals.
    pub divergent_hierarchy: bool,

    /// At least one goal below the Critical threshold (< 0.55).
    pub below_threshold: bool,

    /// High variance in alignment across embedding spaces.
    ///
    /// Indicates the fingerprint aligns well in some spaces but
    /// poorly in others - inconsistent understanding.
    /// Threshold: alignment std_dev > 0.2
    pub inconsistent_alignment: bool,

    /// List of goal IDs that are critically misaligned.
    pub critical_goals: Vec<Uuid>,

    /// List of goal IDs that are in warning state.
    pub warning_goals: Vec<Uuid>,

    /// Alignment variance across embedding spaces (for debugging).
    pub alignment_variance: f32,

    /// Details about divergent hierarchy (parent -> child pairs).
    pub divergent_pairs: Vec<(Uuid, Uuid)>,
}

impl MisalignmentFlags {
    /// Create empty flags (no misalignment detected).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if any misalignment is detected.
    #[inline]
    pub fn has_any(&self) -> bool {
        self.tactical_without_strategic
            || self.divergent_hierarchy
            || self.below_threshold
            || self.inconsistent_alignment
    }

    /// Check if critical intervention is needed.
    ///
    /// Returns true if below_threshold or divergent_hierarchy is set,
    /// as these indicate serious structural issues.
    #[inline]
    pub fn needs_intervention(&self) -> bool {
        self.below_threshold || self.divergent_hierarchy
    }

    /// Count of active flags.
    pub fn flag_count(&self) -> usize {
        let mut count = 0;
        if self.tactical_without_strategic {
            count += 1;
        }
        if self.divergent_hierarchy {
            count += 1;
        }
        if self.below_threshold {
            count += 1;
        }
        if self.inconsistent_alignment {
            count += 1;
        }
        count
    }

    /// Get severity level (0 = none, 1 = warning, 2 = critical).
    pub fn severity(&self) -> u8 {
        if self.below_threshold || self.divergent_hierarchy {
            2 // Critical
        } else if self.tactical_without_strategic || self.inconsistent_alignment {
            1 // Warning
        } else {
            0 // None
        }
    }

    /// Set the below_threshold flag and add critical goal.
    pub fn mark_below_threshold(&mut self, goal_id: Uuid) {
        self.below_threshold = true;
        if !self.critical_goals.contains(&goal_id) {
            self.critical_goals.push(goal_id);
        }
    }

    /// Add a warning-level goal.
    pub fn mark_warning(&mut self, goal_id: Uuid) {
        if !self.warning_goals.contains(&goal_id) {
            self.warning_goals.push(goal_id);
        }
    }

    /// Set divergent hierarchy with a specific parent-child pair.
    pub fn mark_divergent(&mut self, parent_id: Uuid, child_id: Uuid) {
        self.divergent_hierarchy = true;
        self.divergent_pairs.push((parent_id, child_id));
    }

    /// Set inconsistent alignment with variance.
    pub fn mark_inconsistent(&mut self, variance: f32) {
        self.inconsistent_alignment = true;
        self.alignment_variance = variance;
    }

    /// Human-readable summary of detected issues.
    pub fn summary(&self) -> String {
        if !self.has_any() {
            return "No misalignment detected".to_string();
        }

        let mut issues = Vec::new();

        if self.below_threshold {
            issues.push(format!(
                "Critical: {} goal(s) below threshold",
                self.critical_goals.len()
            ));
        }
        if self.divergent_hierarchy {
            issues.push(format!(
                "Divergent: {} parent-child pair(s) misaligned",
                self.divergent_pairs.len()
            ));
        }
        if self.tactical_without_strategic {
            issues.push("Tactical focus without strategic direction".to_string());
        }
        if self.inconsistent_alignment {
            issues.push(format!(
                "Inconsistent alignment (variance: {:.3})",
                self.alignment_variance
            ));
        }

        issues.join("; ")
    }
}

/// Thresholds for detecting misalignment patterns.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MisalignmentThresholds {
    /// Tactical alignment must exceed this for tactical_without_strategic.
    pub tactical_high: f32,

    /// Strategic alignment must be below this for tactical_without_strategic.
    pub strategic_low: f32,

    /// Parent-child alignment delta threshold for divergent_hierarchy.
    /// If child alignment < parent alignment - divergence_delta, flag it.
    pub divergence_delta: f32,

    /// Variance threshold for inconsistent_alignment.
    pub variance_threshold: f32,
}

impl Default for MisalignmentThresholds {
    fn default() -> Self {
        Self {
            tactical_high: 0.7,
            strategic_low: 0.5,
            divergence_delta: 0.25,
            variance_threshold: 0.2,
        }
    }
}

impl MisalignmentThresholds {
    /// Check if tactical-without-strategic pattern applies.
    pub fn is_tactical_without_strategic(&self, tactical: f32, strategic: f32) -> bool {
        tactical > self.tactical_high && strategic < self.strategic_low
    }

    /// Check if parent-child pair is divergent.
    pub fn is_divergent(&self, parent_alignment: f32, child_alignment: f32) -> bool {
        child_alignment < parent_alignment - self.divergence_delta
    }

    /// Check if alignment variance indicates inconsistency.
    pub fn is_inconsistent(&self, variance: f32) -> bool {
        variance > self.variance_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_misalignment_flags_empty() {
        let flags = MisalignmentFlags::empty();
        assert!(!flags.has_any());
        assert!(!flags.needs_intervention());
        assert_eq!(flags.flag_count(), 0);
        assert_eq!(flags.severity(), 0);
        assert_eq!(flags.summary(), "No misalignment detected");

        println!("[VERIFIED] MisalignmentFlags::empty creates clean state");
    }

    #[test]
    fn test_misalignment_flags_below_threshold() {
        let mut flags = MisalignmentFlags::empty();
        flags.mark_below_threshold(Uuid::new_v4());

        assert!(flags.has_any());
        assert!(flags.needs_intervention());
        assert!(flags.below_threshold);
        assert_eq!(flags.critical_goals.len(), 1);
        assert_eq!(flags.severity(), 2); // Critical

        println!("[VERIFIED] below_threshold flag triggers intervention");
        println!("  - summary: {}", flags.summary());
    }

    #[test]
    fn test_misalignment_flags_divergent_hierarchy() {
        let mut flags = MisalignmentFlags::empty();
        flags.mark_divergent(Uuid::new_v4(), Uuid::new_v4());

        assert!(flags.has_any());
        assert!(flags.needs_intervention());
        assert!(flags.divergent_hierarchy);
        assert_eq!(flags.divergent_pairs.len(), 1);
        assert_eq!(flags.severity(), 2); // Critical

        println!("[VERIFIED] divergent_hierarchy flag triggers intervention");
        println!("  - summary: {}", flags.summary());
    }

    #[test]
    fn test_misalignment_flags_tactical_without_strategic() {
        let mut flags = MisalignmentFlags::empty();
        flags.tactical_without_strategic = true;

        assert!(flags.has_any());
        assert!(!flags.needs_intervention()); // Warning only
        assert_eq!(flags.severity(), 1); // Warning

        println!("[VERIFIED] tactical_without_strategic is warning-level");
    }

    #[test]
    fn test_misalignment_flags_inconsistent() {
        let mut flags = MisalignmentFlags::empty();
        flags.mark_inconsistent(0.35);

        assert!(flags.has_any());
        assert!(!flags.needs_intervention()); // Warning only
        assert!(flags.inconsistent_alignment);
        assert_eq!(flags.alignment_variance, 0.35);
        assert_eq!(flags.severity(), 1); // Warning

        println!("[VERIFIED] inconsistent_alignment is warning-level");
        println!("  - summary: {}", flags.summary());
    }

    #[test]
    fn test_misalignment_flags_multiple() {
        let mut flags = MisalignmentFlags::empty();
        flags.mark_below_threshold(Uuid::new_v4());
        flags.mark_below_threshold(Uuid::new_v4());
        flags.mark_warning(Uuid::new_v4());
        flags.mark_divergent(Uuid::new_v4(), Uuid::new_v4());
        flags.tactical_without_strategic = true;

        assert_eq!(flags.flag_count(), 3); // below_threshold, divergent, tactical_without_strategic
                                           // Note: Each Uuid::new_v4() creates a unique ID, so no duplicates
        assert_eq!(flags.critical_goals.len(), 2);
        assert_eq!(flags.warning_goals.len(), 1);
        assert_eq!(flags.severity(), 2);

        println!("[VERIFIED] Multiple flags accumulate correctly");
        println!("  - flag_count: {}", flags.flag_count());
        println!("  - summary: {}", flags.summary());
    }

    #[test]
    fn test_misalignment_thresholds_default() {
        let thresholds = MisalignmentThresholds::default();
        assert_eq!(thresholds.tactical_high, 0.7);
        assert_eq!(thresholds.strategic_low, 0.5);
        assert_eq!(thresholds.divergence_delta, 0.25);
        assert_eq!(thresholds.variance_threshold, 0.2);

        println!("[VERIFIED] MisalignmentThresholds::default values match spec");
    }

    #[test]
    fn test_misalignment_thresholds_tactical_without_strategic() {
        let thresholds = MisalignmentThresholds::default();

        // Should trigger: tactical > 0.7 AND strategic < 0.5
        assert!(thresholds.is_tactical_without_strategic(0.8, 0.4));

        // Should not trigger: tactical not high enough
        assert!(!thresholds.is_tactical_without_strategic(0.6, 0.4));

        // Should not trigger: strategic not low enough
        assert!(!thresholds.is_tactical_without_strategic(0.8, 0.6));

        println!("[VERIFIED] tactical_without_strategic detection works");
    }

    #[test]
    fn test_misalignment_thresholds_divergent() {
        let thresholds = MisalignmentThresholds::default();

        // Parent: 0.8, Child: 0.5 -> delta = 0.3 > 0.25 -> divergent
        assert!(thresholds.is_divergent(0.8, 0.5));

        // Parent: 0.8, Child: 0.7 -> delta = 0.1 < 0.25 -> not divergent
        assert!(!thresholds.is_divergent(0.8, 0.7));

        // Parent: 0.5, Child: 0.6 -> child > parent -> not divergent
        assert!(!thresholds.is_divergent(0.5, 0.6));

        println!("[VERIFIED] divergent hierarchy detection works");
    }

    #[test]
    fn test_misalignment_thresholds_inconsistent() {
        let thresholds = MisalignmentThresholds::default();

        // Variance 0.25 > 0.2 -> inconsistent
        assert!(thresholds.is_inconsistent(0.25));

        // Variance 0.15 < 0.2 -> consistent
        assert!(!thresholds.is_inconsistent(0.15));

        println!("[VERIFIED] inconsistent alignment detection works");
    }

    #[test]
    fn test_misalignment_flags_no_duplicates() {
        let mut flags = MisalignmentFlags::empty();
        let same_id = Uuid::new_v4();
        flags.mark_below_threshold(same_id);
        flags.mark_below_threshold(same_id); // Duplicate
        flags.mark_below_threshold(Uuid::new_v4());

        assert_eq!(flags.critical_goals.len(), 2); // No duplicate

        println!("[VERIFIED] Duplicate goal IDs are not added");
    }
}
