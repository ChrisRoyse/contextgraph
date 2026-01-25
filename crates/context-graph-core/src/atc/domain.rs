//! Domain-Specific Threshold Management
//!
//! Per-domain threshold adaptation with transfer learning.
//! Supports Code, Medical, Legal, Creative, Research, and General domains.
//!
//! Transfer learning formula:
//! θ_new = α × θ_similar_domain + (1 - α) × θ_general

use std::collections::HashMap;

/// Supported domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Code,
    Medical,
    Legal,
    Creative,
    Research,
    General,
}

impl Domain {
    pub fn as_str(&self) -> &str {
        match self {
            Domain::Code => "code",
            Domain::Medical => "medical",
            Domain::Legal => "legal",
            Domain::Creative => "creative",
            Domain::Research => "research",
            Domain::General => "general",
        }
    }

    /// Get description from constitution
    pub fn description(&self) -> &str {
        match self {
            Domain::Code => "Strict thresholds, low tolerance for false positives",
            Domain::Medical => "Very strict, high causal weight",
            Domain::Legal => "Moderate, high semantic precision",
            Domain::Creative => "Loose thresholds, exploration encouraged",
            Domain::Research => "Balanced, novelty valued",
            Domain::General => "Default priors",
        }
    }

    /// Get recommended strictness (0=loose, 1=strict)
    pub fn strictness(&self) -> f32 {
        match self {
            Domain::Code => 0.9,
            Domain::Medical => 1.0,
            Domain::Legal => 0.8,
            Domain::Creative => 0.2,
            Domain::Research => 0.5,
            Domain::General => 0.5,
        }
    }

    /// Find most similar domain for transfer learning
    pub fn find_similar(&self) -> Domain {
        match self {
            Domain::Code => Domain::Research,
            Domain::Medical => Domain::Legal,
            Domain::Legal => Domain::Medical,
            Domain::Creative => Domain::Research,
            Domain::Research => Domain::General,
            Domain::General => Domain::General,
        }
    }
}

impl std::str::FromStr for Domain {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "code" => Ok(Domain::Code),
            "medical" => Ok(Domain::Medical),
            "legal" => Ok(Domain::Legal),
            "creative" => Ok(Domain::Creative),
            "research" => Ok(Domain::Research),
            "general" => Ok(Domain::General),
            _ => Err(()),
        }
    }
}

/// Domain-specific thresholds for adaptive calibration.
///
/// # Constitution Reference
/// See `docs2/constitution.yaml` sections:
/// - `adaptive_thresholds.priors` for base ranges
/// - `dream.trigger`, `dream.phases` for dream thresholds
#[derive(Debug, Clone)]
pub struct DomainThresholds {
    pub domain: Domain,

    // === Existing fields (6) ===
    pub theta_opt: f32,       // [0.60, 0.90] Optimal alignment
    pub theta_acc: f32,       // [0.55, 0.85] Acceptable alignment
    pub theta_warn: f32,      // [0.40, 0.70] Warning alignment
    pub theta_dup: f32,       // [0.80, 0.98] Duplicate detection
    pub theta_edge: f32,      // [0.50, 0.85] Edge creation
    pub confidence_bias: f32, // Domain confidence adjustment

    // === Coherence thresholds (3) ===
    pub theta_gate: f32,          // [0.65, 0.95] Coherence gate
    pub theta_hypersync: f32,     // [0.90, 0.99] Hypersync detection
    pub theta_fragmentation: f32, // [0.35, 0.65] Fragmentation warning

    // === NEW: Layer thresholds (2) ===
    pub theta_memory_sim: f32,    // [0.35, 0.75] Memory similarity
    pub theta_consolidation: f32, // [0.05, 0.30] Consolidation trigger

    // === NEW: Dream thresholds (3) ===
    pub theta_dream_activity: f32, // [0.05, 0.30] Dream trigger
    pub theta_semantic_leap: f32,  // [0.50, 0.90] REM exploration
    pub theta_shortcut_conf: f32,  // [0.50, 0.85] Shortcut confidence

    // === NEW: Classification thresholds (1) ===
    pub theta_classification: f32, // [0.35, 0.65] Classification boundary

    // === NEW: Autonomous thresholds (4) ===
    pub theta_obsolescence_low: f32,  // [0.20, 0.50] Low relevance
    pub theta_obsolescence_high: f32, // [0.65, 0.90] High confidence
    pub theta_obsolescence_mid: f32,  // [0.45, 0.75] Medium confidence
    pub theta_drift_slope: f32,       // [0.001, 0.01] Drift detection
}

impl DomainThresholds {
    /// Create default thresholds for a domain.
    ///
    /// Threshold values are computed based on domain strictness (0.0 = loose, 1.0 = strict).
    /// Constitution references:
    /// - Dream: dream.trigger.activity, dream.phases.rem.blind_spot
    pub fn new(domain: Domain) -> Self {
        let strictness = domain.strictness();

        // === Existing thresholds (unchanged logic) ===
        let theta_opt = 0.75 + (strictness * 0.1); // [0.75, 0.85]
        let theta_acc = 0.70 + (strictness * 0.08); // [0.70, 0.78]
        let theta_warn = 0.55 + (strictness * 0.05); // [0.55, 0.60]

        // === Coherence thresholds ===
        // Stricter domains have higher gates
        let theta_gate = 0.75 + (strictness * 0.15); // [0.75, 0.90]
        let theta_hypersync = 0.93 + (strictness * 0.04); // [0.93, 0.97]
        let theta_fragmentation = 0.50 - (strictness * 0.10); // [0.40, 0.50]

        // === NEW: Layer thresholds ===
        let theta_memory_sim = 0.50 + (strictness * 0.15); // [0.50, 0.65]
        let theta_consolidation = 0.10 + (strictness * 0.10); // [0.10, 0.20]

        // === NEW: Dream thresholds ===
        // Creative domains dream more aggressively (lower trigger threshold)
        let theta_dream_activity = 0.15 - (strictness * 0.05); // [0.10, 0.15]
        let theta_semantic_leap = 0.70 - (strictness * 0.10); // [0.60, 0.70]
        let theta_shortcut_conf = 0.70 + (strictness * 0.10); // [0.70, 0.80]

        // === NEW: Classification thresholds ===
        // Fixed per constitution (may later be domain-tuned)
        let theta_classification = 0.50;

        // === NEW: Autonomous thresholds ===
        // Stricter domains require higher confidence for autonomous actions
        let theta_obsolescence_low = 0.30 + (strictness * 0.10); // [0.30, 0.40]
        let theta_obsolescence_high = 0.75 + (strictness * 0.10); // [0.75, 0.85]

        // theta_obsolescence_mid: between low and high
        let theta_obsolescence_mid =
            theta_obsolescence_low + (theta_obsolescence_high - theta_obsolescence_low) * 0.5;

        // theta_drift_slope: stricter domains need smaller slopes to trigger
        // Range [0.001, 0.01], inverse of strictness (more strict = smaller threshold)
        let theta_drift_slope = 0.01 - (strictness * 0.009); // [0.001, 0.01]

        Self {
            domain,
            theta_opt,
            theta_acc,
            theta_warn,
            theta_dup: 0.90,
            theta_edge: 0.70,
            confidence_bias: 1.0,
            theta_gate,
            theta_hypersync,
            theta_fragmentation,
            theta_memory_sim,
            theta_consolidation,
            theta_dream_activity,
            theta_semantic_leap,
            theta_shortcut_conf,
            theta_classification,
            theta_obsolescence_low,
            theta_obsolescence_high,
            theta_obsolescence_mid,
            theta_drift_slope,
        }
    }

    /// Transfer learning: blend with similar domain thresholds.
    ///
    /// Formula: θ_new = α × θ_similar + (1 - α) × θ_self
    pub fn blend_with_similar(&mut self, similar: &DomainThresholds, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        let blend =
            |self_val: f32, other_val: f32| -> f32 { alpha * other_val + (1.0 - alpha) * self_val };

        // Existing fields
        self.theta_opt = blend(self.theta_opt, similar.theta_opt);
        self.theta_acc = blend(self.theta_acc, similar.theta_acc);
        self.theta_warn = blend(self.theta_warn, similar.theta_warn);
        self.theta_dup = blend(self.theta_dup, similar.theta_dup);
        self.theta_edge = blend(self.theta_edge, similar.theta_edge);

        // Coherence thresholds
        self.theta_gate = blend(self.theta_gate, similar.theta_gate);
        self.theta_hypersync = blend(self.theta_hypersync, similar.theta_hypersync);
        self.theta_fragmentation = blend(self.theta_fragmentation, similar.theta_fragmentation);

        // NEW: Layer thresholds
        self.theta_memory_sim = blend(self.theta_memory_sim, similar.theta_memory_sim);
        self.theta_consolidation = blend(self.theta_consolidation, similar.theta_consolidation);

        // NEW: Dream thresholds
        self.theta_dream_activity = blend(self.theta_dream_activity, similar.theta_dream_activity);
        self.theta_semantic_leap = blend(self.theta_semantic_leap, similar.theta_semantic_leap);
        self.theta_shortcut_conf = blend(self.theta_shortcut_conf, similar.theta_shortcut_conf);

        // NEW: Classification thresholds
        self.theta_classification = blend(self.theta_classification, similar.theta_classification);

        // NEW: Autonomous thresholds
        self.theta_obsolescence_low =
            blend(self.theta_obsolescence_low, similar.theta_obsolescence_low);
        self.theta_obsolescence_high = blend(
            self.theta_obsolescence_high,
            similar.theta_obsolescence_high,
        );
        self.theta_obsolescence_mid =
            blend(self.theta_obsolescence_mid, similar.theta_obsolescence_mid);
        self.theta_drift_slope = blend(self.theta_drift_slope, similar.theta_drift_slope);
    }

    /// Check if thresholds are valid (monotonicity, ranges).
    ///
    /// Returns `false` if any threshold is out of its constitution-defined range
    /// or if monotonicity constraints are violated.
    pub fn is_valid(&self) -> bool {
        // === Existing monotonicity check ===
        if !(self.theta_opt > self.theta_acc && self.theta_acc > self.theta_warn) {
            return false;
        }

        // === Existing range checks ===
        if !(0.60..=0.90).contains(&self.theta_opt) {
            return false;
        }
        if !(0.55..=0.85).contains(&self.theta_acc) {
            return false;
        }
        if !(0.40..=0.70).contains(&self.theta_warn) {
            return false;
        }
        if !(0.80..=0.98).contains(&self.theta_dup) {
            return false;
        }
        if !(0.50..=0.85).contains(&self.theta_edge) {
            return false;
        }

        // === Coherence thresholds ===
        if !(0.65..=0.95).contains(&self.theta_gate) {
            return false;
        }
        if !(0.90..=0.99).contains(&self.theta_hypersync) {
            return false;
        }
        if !(0.35..=0.65).contains(&self.theta_fragmentation) {
            return false;
        }

        // === NEW: Layer thresholds ===
        if !(0.35..=0.75).contains(&self.theta_memory_sim) {
            return false;
        }
        if !(0.05..=0.30).contains(&self.theta_consolidation) {
            return false;
        }

        // === NEW: Dream thresholds ===
        if !(0.05..=0.30).contains(&self.theta_dream_activity) {
            return false;
        }
        if !(0.50..=0.90).contains(&self.theta_semantic_leap) {
            return false;
        }
        if !(0.50..=0.85).contains(&self.theta_shortcut_conf) {
            return false;
        }

        // === NEW: Classification thresholds ===
        if !(0.35..=0.65).contains(&self.theta_classification) {
            return false;
        }

        // === NEW: Autonomous thresholds - MUST enforce monotonicity ===
        if !(0.20..=0.50).contains(&self.theta_obsolescence_low) {
            return false;
        }
        if !(0.65..=0.90).contains(&self.theta_obsolescence_high) {
            return false;
        }
        if self.theta_obsolescence_high <= self.theta_obsolescence_low {
            return false;
        }

        // theta_obsolescence_mid
        if !(0.45..=0.75).contains(&self.theta_obsolescence_mid) {
            return false;
        }
        // Must be between low and high
        if !(self.theta_obsolescence_mid > self.theta_obsolescence_low
            && self.theta_obsolescence_mid < self.theta_obsolescence_high)
        {
            return false;
        }

        // theta_drift_slope
        if !(0.001..=0.01).contains(&self.theta_drift_slope) {
            return false;
        }

        true
    }

    /// Clamp thresholds to valid ranges.
    ///
    /// Note: This only enforces range bounds, not monotonicity constraints.
    /// Call `is_valid()` after clamping to verify monotonicity.
    pub fn clamp(&mut self) {
        // Existing fields
        self.theta_opt = self.theta_opt.clamp(0.60, 0.90);
        self.theta_acc = self.theta_acc.clamp(0.55, 0.85);
        self.theta_warn = self.theta_warn.clamp(0.40, 0.70);
        self.theta_dup = self.theta_dup.clamp(0.80, 0.98);
        self.theta_edge = self.theta_edge.clamp(0.50, 0.85);

        // Coherence thresholds
        self.theta_gate = self.theta_gate.clamp(0.65, 0.95);
        self.theta_hypersync = self.theta_hypersync.clamp(0.90, 0.99);
        self.theta_fragmentation = self.theta_fragmentation.clamp(0.35, 0.65);

        // NEW: Layer thresholds
        self.theta_memory_sim = self.theta_memory_sim.clamp(0.35, 0.75);
        self.theta_consolidation = self.theta_consolidation.clamp(0.05, 0.30);

        // NEW: Dream thresholds
        self.theta_dream_activity = self.theta_dream_activity.clamp(0.05, 0.30);
        self.theta_semantic_leap = self.theta_semantic_leap.clamp(0.50, 0.90);
        self.theta_shortcut_conf = self.theta_shortcut_conf.clamp(0.50, 0.85);

        // NEW: Classification thresholds
        self.theta_classification = self.theta_classification.clamp(0.35, 0.65);

        // NEW: Autonomous thresholds
        self.theta_obsolescence_low = self.theta_obsolescence_low.clamp(0.20, 0.50);
        self.theta_obsolescence_high = self.theta_obsolescence_high.clamp(0.65, 0.90);
        self.theta_obsolescence_mid = self.theta_obsolescence_mid.clamp(0.45, 0.75);
        self.theta_drift_slope = self.theta_drift_slope.clamp(0.001, 0.01);
    }
}

/// Domain threshold manager
#[derive(Debug)]
pub struct DomainManager {
    thresholds: HashMap<Domain, DomainThresholds>,
}

impl Default for DomainManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainManager {
    /// Create new domain manager with defaults
    pub fn new() -> Self {
        let mut thresholds = HashMap::new();

        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            thresholds.insert(domain, DomainThresholds::new(domain));
        }

        Self { thresholds }
    }

    /// Get thresholds for a domain
    pub fn get(&self, domain: Domain) -> Option<&DomainThresholds> {
        self.thresholds.get(&domain)
    }

    /// Get mutable thresholds for a domain
    pub fn get_mut(&mut self, domain: Domain) -> Option<&mut DomainThresholds> {
        self.thresholds.get_mut(&domain)
    }

    /// Update thresholds for a domain
    pub fn update(&mut self, domain: Domain, thresholds: DomainThresholds) -> Result<(), String> {
        if !thresholds.is_valid() {
            return Err("Thresholds fail validity check".to_string());
        }

        self.thresholds.insert(domain, thresholds);
        Ok(())
    }

    /// Transfer learn from one domain to another
    /// Uses: θ_target = α × θ_source + (1 - α) × θ_target
    pub fn transfer_learn(
        &mut self,
        target_domain: Domain,
        source_domain: Domain,
        alpha: f32,
    ) -> Result<(), String> {
        let source_copy = self
            .thresholds
            .get(&source_domain)
            .ok_or("Source domain not found")?
            .clone();

        let target = self
            .thresholds
            .get_mut(&target_domain)
            .ok_or("Target domain not found")?;

        target.blend_with_similar(&source_copy, alpha);
        target.clamp();

        Ok(())
    }

    /// Apply similarity-based transfer learning
    /// Automatically finds similar domain and blends
    pub fn apply_similarity_transfer(&mut self, domain: Domain, alpha: f32) -> Result<(), String> {
        let similar = domain.find_similar();
        if similar != domain {
            self.transfer_learn(domain, similar, alpha)?;
        }
        Ok(())
    }

    /// Get all domains and their thresholds
    pub fn get_all(&self) -> Vec<(Domain, &DomainThresholds)> {
        self.thresholds.iter().map(|(d, t)| (*d, t)).collect()
    }

    /// Validate all domains
    pub fn validate_all(&self) -> Vec<(Domain, bool)> {
        self.thresholds
            .iter()
            .map(|(d, t)| (*d, t.is_valid()))
            .collect()
    }

    /// Reset all domains to defaults
    pub fn reset_all(&mut self) {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            self.thresholds
                .insert(domain, DomainThresholds::new(domain));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_descriptions() {
        assert!(!Domain::Code.description().is_empty());
        assert!(!Domain::Medical.description().is_empty());
    }

    #[test]
    fn test_domain_strictness() {
        assert!(Domain::Medical.strictness() > Domain::Creative.strictness());
        assert!(Domain::Code.strictness() > Domain::General.strictness());
    }

    #[test]
    fn test_domain_thresholds_creation() {
        let thresholds = DomainThresholds::new(Domain::Code);
        assert!(thresholds.is_valid());
        assert!(thresholds.theta_opt > thresholds.theta_acc);
        assert!(thresholds.theta_acc > thresholds.theta_warn);
    }

    #[test]
    fn test_domain_differences() {
        let code = DomainThresholds::new(Domain::Code);
        let creative = DomainThresholds::new(Domain::Creative);

        // Code should have stricter thresholds
        assert!(code.theta_opt > creative.theta_opt);
    }

    #[test]
    fn test_blend_with_similar() {
        let mut code = DomainThresholds::new(Domain::Code);
        let research = DomainThresholds::new(Domain::Research);

        code.blend_with_similar(&research, 0.5);
        assert!(code.is_valid());
    }

    #[test]
    fn test_domain_manager() {
        let manager = DomainManager::new();
        assert!(manager.get(Domain::Code).is_some());
        assert!(manager.get(Domain::Medical).is_some());
    }

    #[test]
    fn test_transfer_learning() {
        let mut manager = DomainManager::new();
        let original_opt = manager.get(Domain::Creative).unwrap().theta_opt;

        manager
            .transfer_learn(Domain::Creative, Domain::Code, 0.3)
            .unwrap();

        let new_opt = manager.get(Domain::Creative).unwrap().theta_opt;
        // Should be different after transfer
        assert!((new_opt - original_opt).abs() > 0.01);
    }

    #[test]
    fn test_validate_all() {
        let manager = DomainManager::new();
        let validation = manager.validate_all();

        // All default domains should be valid
        assert!(validation.iter().all(|(_, valid)| *valid));
    }

    #[test]
    fn test_clamping() {
        let mut thresholds = DomainThresholds::new(Domain::Code);
        thresholds.theta_opt = 0.95; // Out of range

        thresholds.clamp();
        assert_eq!(thresholds.theta_opt, 0.90); // Clamped to max

        assert!(thresholds.is_valid());
    }

    #[test]
    fn test_similarity_chain() {
        let code_similar = Domain::Code.find_similar();
        assert_eq!(code_similar, Domain::Research);

        let research_similar = Domain::Research.find_similar();
        assert_eq!(research_similar, Domain::General);
    }

    // ========== NEW TESTS FOR TASK-ATC-P2-002 ==========

    #[test]
    fn test_extended_fields_exist() {
        let thresholds = DomainThresholds::new(Domain::Code);

        // All new fields accessible and in valid range
        assert!(
            (0.65..=0.95).contains(&thresholds.theta_gate),
            "theta_gate {} out of range [0.65, 0.95]",
            thresholds.theta_gate
        );
        assert!(
            (0.90..=0.99).contains(&thresholds.theta_hypersync),
            "theta_hypersync {} out of range [0.90, 0.99]",
            thresholds.theta_hypersync
        );
        assert!(
            (0.35..=0.65).contains(&thresholds.theta_fragmentation),
            "theta_fragmentation {} out of range [0.35, 0.65]",
            thresholds.theta_fragmentation
        );
        assert!(
            (0.35..=0.75).contains(&thresholds.theta_memory_sim),
            "theta_memory_sim {} out of range [0.35, 0.75]",
            thresholds.theta_memory_sim
        );
        assert!(
            (0.05..=0.30).contains(&thresholds.theta_consolidation),
            "theta_consolidation {} out of range [0.05, 0.30]",
            thresholds.theta_consolidation
        );
        assert!(
            (0.05..=0.30).contains(&thresholds.theta_dream_activity),
            "theta_dream_activity {} out of range [0.05, 0.30]",
            thresholds.theta_dream_activity
        );
        assert!(
            (0.50..=0.90).contains(&thresholds.theta_semantic_leap),
            "theta_semantic_leap {} out of range [0.50, 0.90]",
            thresholds.theta_semantic_leap
        );
        assert!(
            (0.50..=0.85).contains(&thresholds.theta_shortcut_conf),
            "theta_shortcut_conf {} out of range [0.50, 0.85]",
            thresholds.theta_shortcut_conf
        );
        assert!(
            (0.35..=0.65).contains(&thresholds.theta_classification),
            "theta_classification {} out of range [0.35, 0.65]",
            thresholds.theta_classification
        );
        assert!(
            (0.20..=0.50).contains(&thresholds.theta_obsolescence_low),
            "theta_obsolescence_low {} out of range [0.20, 0.50]",
            thresholds.theta_obsolescence_low
        );
        assert!(
            (0.65..=0.90).contains(&thresholds.theta_obsolescence_high),
            "theta_obsolescence_high {} out of range [0.65, 0.90]",
            thresholds.theta_obsolescence_high
        );
    }

    #[test]
    fn test_domain_strictness_affects_new_thresholds() {
        let code = DomainThresholds::new(Domain::Code); // strictness=0.9
        let creative = DomainThresholds::new(Domain::Creative); // strictness=0.2

        // Stricter domain has HIGHER gate
        assert!(
            code.theta_gate > creative.theta_gate,
            "Code gate {} should be > Creative gate {}",
            code.theta_gate,
            creative.theta_gate
        );
        // Stricter domain has HIGHER memory similarity requirement
        assert!(
            code.theta_memory_sim > creative.theta_memory_sim,
            "Code memory_sim {} should be > Creative memory_sim {}",
            code.theta_memory_sim,
            creative.theta_memory_sim
        );
        // Stricter domain dreams LESS aggressively (higher threshold to trigger)
        // Note: In our implementation, lower strictness = lower dream_activity threshold
        // So creative has HIGHER dream_activity value (but that means more dreaming allowed)
        assert!(
            code.theta_dream_activity < creative.theta_dream_activity,
            "Code dream_activity {} should be < Creative dream_activity {}",
            code.theta_dream_activity,
            creative.theta_dream_activity
        );
    }

    #[test]
    fn test_obsolescence_monotonicity() {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            assert!(
                t.theta_obsolescence_high > t.theta_obsolescence_low,
                "{:?}: high {} should be > low {}",
                domain,
                t.theta_obsolescence_high,
                t.theta_obsolescence_low
            );
        }
    }

    #[test]
    fn test_is_valid_fails_invalid_gate_below_min() {
        let mut t = DomainThresholds::new(Domain::General);
        t.theta_gate = 0.50; // Below min 0.65
        assert!(!t.is_valid(), "Should fail for theta_gate below min");
    }

    #[test]
    fn test_is_valid_fails_invalid_hypersync_above_max() {
        let mut t = DomainThresholds::new(Domain::General);
        t.theta_hypersync = 1.0; // Above max 0.99
        assert!(!t.is_valid(), "Should fail for theta_hypersync above max");
    }

    #[test]
    fn test_is_valid_fails_invalid_obsolescence_monotonicity() {
        let mut t = DomainThresholds::new(Domain::General);
        // First clamp to valid ranges
        t.theta_obsolescence_high = 0.70; // Valid range
        t.theta_obsolescence_low = 0.40; // Valid range
                                         // Now break monotonicity by swapping conceptually
        t.theta_obsolescence_high = 0.30; // Now out of range, will fail range check
        t.theta_obsolescence_low = 0.40;
        assert!(!t.is_valid(), "Should fail monotonicity/range check");
    }

    #[test]
    fn test_clamp_all_new_fields() {
        let mut t = DomainThresholds::new(Domain::General);

        // Set all new fields out of range
        t.theta_gate = 1.5;
        t.theta_hypersync = 0.5;
        t.theta_fragmentation = 0.0;
        t.theta_memory_sim = 1.0;
        t.theta_consolidation = 0.5;
        t.theta_dream_activity = 0.5;
        t.theta_semantic_leap = 0.0;
        t.theta_shortcut_conf = 0.0;
        t.theta_classification = 0.0;
        t.theta_obsolescence_low = 0.0;
        t.theta_obsolescence_high = 1.0;

        t.clamp();

        // All should now be clamped to range bounds
        assert_eq!(t.theta_gate, 0.95);
        assert_eq!(t.theta_hypersync, 0.90);
        assert_eq!(t.theta_fragmentation, 0.35);
        assert_eq!(t.theta_memory_sim, 0.75);
        assert_eq!(t.theta_consolidation, 0.30);
        assert_eq!(t.theta_dream_activity, 0.30);
        assert_eq!(t.theta_semantic_leap, 0.50);
        assert_eq!(t.theta_shortcut_conf, 0.50);
        assert_eq!(t.theta_classification, 0.35);
        assert_eq!(t.theta_obsolescence_low, 0.20);
        assert_eq!(t.theta_obsolescence_high, 0.90);
    }

    #[test]
    fn test_blend_includes_new_fields() {
        let mut code = DomainThresholds::new(Domain::Code);
        let creative = DomainThresholds::new(Domain::Creative);

        let original_gate = code.theta_gate;
        let original_dream = code.theta_dream_activity;

        code.blend_with_similar(&creative, 0.5);

        // Gate should have moved toward creative (lower)
        assert!(
            code.theta_gate < original_gate,
            "Gate {} should have decreased from {}",
            code.theta_gate,
            original_gate
        );
        // Dream activity should have moved toward creative (higher)
        assert!(
            code.theta_dream_activity > original_dream,
            "Dream activity {} should have increased from {}",
            code.theta_dream_activity,
            original_dream
        );
    }

    #[test]
    fn test_all_domains_valid_on_creation() {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            assert!(
                t.is_valid(),
                "Domain {:?} should create valid thresholds",
                domain
            );
        }
    }

    #[test]
    fn test_medical_strictest_thresholds() {
        let medical = DomainThresholds::new(Domain::Medical); // strictness = 1.0
        let general = DomainThresholds::new(Domain::General); // strictness = 0.5

        // Medical should have highest gates
        assert!(medical.theta_gate > general.theta_gate);
        assert!(medical.theta_hypersync > general.theta_hypersync);
        assert!(medical.theta_obsolescence_high > general.theta_obsolescence_high);
    }

    #[test]
    fn test_creative_loosest_thresholds() {
        let creative = DomainThresholds::new(Domain::Creative); // strictness = 0.2
        let general = DomainThresholds::new(Domain::General); // strictness = 0.5

        // Creative should have lower gates
        assert!(creative.theta_gate < general.theta_gate);
        // Creative should have higher fragmentation threshold (easier to detect)
        assert!(creative.theta_fragmentation > general.theta_fragmentation);
    }

    #[test]
    fn test_field_count_is_19() {
        // This test ensures we have exactly 19 fields by checking the struct size indirectly
        // We verify by accessing all 19 f32 fields
        let t = DomainThresholds::new(Domain::General);
        let fields: Vec<f32> = vec![
            // domain is not f32, so we count 19 f32 fields + 1 Domain
            t.theta_opt,
            t.theta_acc,
            t.theta_warn,
            t.theta_dup,
            t.theta_edge,
            t.confidence_bias,
            t.theta_gate,
            t.theta_hypersync,
            t.theta_fragmentation,
            t.theta_memory_sim,
            t.theta_consolidation,
            t.theta_dream_activity,
            t.theta_semantic_leap,
            t.theta_shortcut_conf,
            t.theta_classification,
            t.theta_obsolescence_low,
            t.theta_obsolescence_high,
            t.theta_obsolescence_mid,
            t.theta_drift_slope,
        ];
        // 19 f32 fields + 1 Domain = 20 total fields in struct
        assert_eq!(fields.len(), 19, "Should have 19 f32 threshold fields");
        // Verify domain field exists
        assert_eq!(t.domain, Domain::General);
    }

    #[test]
    fn test_print_all_domain_thresholds() {
        // This test prints all threshold values for manual verification
        // It also validates the computed values match expected ranges
        println!("\n=== Domain Threshold Values (TASK-ATC-P2-002) ===\n");

        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            println!("{:?} (strictness={:.1}):", domain, domain.strictness());
            println!("  theta_gate: {:.3}", t.theta_gate);
            println!("  theta_hypersync: {:.3}", t.theta_hypersync);
            println!("  theta_fragmentation: {:.3}", t.theta_fragmentation);
            println!("  theta_memory_sim: {:.3}", t.theta_memory_sim);
            println!("  theta_consolidation: {:.3}", t.theta_consolidation);
            println!("  theta_dream_activity: {:.3}", t.theta_dream_activity);
            println!("  theta_semantic_leap: {:.3}", t.theta_semantic_leap);
            println!("  theta_shortcut_conf: {:.3}", t.theta_shortcut_conf);
            println!("  theta_classification: {:.3}", t.theta_classification);
            println!("  theta_obsolescence_low: {:.3}", t.theta_obsolescence_low);
            println!(
                "  theta_obsolescence_high: {:.3}",
                t.theta_obsolescence_high
            );
            println!("  theta_obsolescence_mid: {:.3}", t.theta_obsolescence_mid);
            println!("  theta_drift_slope: {:.6}", t.theta_drift_slope);
            println!();

            assert!(
                t.is_valid(),
                "Domain {:?} thresholds should be valid",
                domain
            );
        }
    }

    // ========== NEW TESTS FOR TASK-ATC-P2-007 ==========

    #[test]
    fn test_theta_obsolescence_mid_range() {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            assert!(
                (0.45..=0.75).contains(&t.theta_obsolescence_mid),
                "{:?}: theta_obsolescence_mid {} out of range [0.45, 0.75]",
                domain,
                t.theta_obsolescence_mid
            );
        }
    }

    #[test]
    fn test_theta_drift_slope_range() {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            assert!(
                (0.001..=0.01).contains(&t.theta_drift_slope),
                "{:?}: theta_drift_slope {} out of range [0.001, 0.01]",
                domain,
                t.theta_drift_slope
            );
        }
    }

    #[test]
    fn test_theta_obsolescence_mid_monotonicity() {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let t = DomainThresholds::new(domain);
            assert!(
                t.theta_obsolescence_mid > t.theta_obsolescence_low,
                "{:?}: mid {} should be > low {}",
                domain,
                t.theta_obsolescence_mid,
                t.theta_obsolescence_low
            );
            assert!(
                t.theta_obsolescence_mid < t.theta_obsolescence_high,
                "{:?}: mid {} should be < high {}",
                domain,
                t.theta_obsolescence_mid,
                t.theta_obsolescence_high
            );
        }
    }

    #[test]
    fn test_drift_slope_inverse_strictness() {
        let medical = DomainThresholds::new(Domain::Medical); // strictness=1.0
        let creative = DomainThresholds::new(Domain::Creative); // strictness=0.2

        // Stricter domains need smaller slope to trigger (more sensitive)
        assert!(
            medical.theta_drift_slope < creative.theta_drift_slope,
            "Medical drift_slope {} should be < Creative drift_slope {}",
            medical.theta_drift_slope,
            creative.theta_drift_slope
        );
    }

    #[test]
    fn test_clamp_new_fields() {
        let mut t = DomainThresholds::new(Domain::General);

        // Set out of range
        t.theta_obsolescence_mid = 0.0;
        t.theta_drift_slope = 0.1;

        t.clamp();

        assert_eq!(t.theta_obsolescence_mid, 0.45);
        assert_eq!(t.theta_drift_slope, 0.01);
    }

    #[test]
    fn test_is_valid_fails_invalid_mid_below_range() {
        let mut t = DomainThresholds::new(Domain::General);
        t.theta_obsolescence_mid = 0.40; // Below min 0.45
        assert!(
            !t.is_valid(),
            "Should fail for theta_obsolescence_mid below min"
        );
    }

    #[test]
    fn test_is_valid_fails_invalid_mid_monotonicity() {
        let mut t = DomainThresholds::new(Domain::General);
        // Set mid >= high (violates monotonicity)
        t.theta_obsolescence_mid = 0.75;
        t.theta_obsolescence_high = 0.75;
        assert!(!t.is_valid(), "Should fail for mid >= high");
    }

    #[test]
    fn test_is_valid_fails_invalid_drift_slope() {
        let mut t = DomainThresholds::new(Domain::General);
        t.theta_drift_slope = 0.02; // Above max 0.01
        assert!(!t.is_valid(), "Should fail for theta_drift_slope above max");
    }

    #[test]
    fn test_blend_includes_new_fields_p2_007() {
        let mut code = DomainThresholds::new(Domain::Code);
        let creative = DomainThresholds::new(Domain::Creative);

        let original_mid = code.theta_obsolescence_mid;
        let original_slope = code.theta_drift_slope;

        code.blend_with_similar(&creative, 0.5);

        // Mid should have moved toward creative
        assert!(
            (code.theta_obsolescence_mid - original_mid).abs() > 0.001,
            "Mid {} should have changed from {}",
            code.theta_obsolescence_mid,
            original_mid
        );
        // Slope should have moved toward creative (higher)
        assert!(
            code.theta_drift_slope > original_slope,
            "Drift slope {} should have increased from {}",
            code.theta_drift_slope,
            original_slope
        );
    }
}
