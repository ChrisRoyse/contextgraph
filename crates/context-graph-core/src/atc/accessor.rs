//! Unified threshold access by name.
//!
//! Enables dynamic threshold lookup for MCP tools and subsystems
//! that need to access thresholds by string name rather than field access.

use super::{AdaptiveThresholdCalibration, Domain};

/// All threshold names supported by the system (19 thresholds).
pub const THRESHOLD_NAMES: &[&str] = &[
    // Existing fields (5 behavioral thresholds, excluding confidence_bias)
    "theta_opt",
    "theta_acc",
    "theta_warn",
    "theta_dup",
    "theta_edge",
    // Coherence thresholds (3)
    "theta_gate",
    "theta_hypersync",
    "theta_fragmentation",
    // Layer thresholds (2)
    "theta_memory_sim",
    "theta_consolidation",
    // Dream thresholds (3)
    "theta_dream_activity",
    "theta_semantic_leap",
    "theta_shortcut_conf",
    // Classification thresholds (1)
    "theta_classification",
    // Autonomous thresholds (4)
    "theta_obsolescence_low",
    "theta_obsolescence_high",
    "theta_obsolescence_mid",
    "theta_drift_slope",
];

/// Unified threshold access by name.
///
/// Enables both static (field access) and dynamic (name-based) threshold retrieval.
pub trait ThresholdAccessor {
    /// Get threshold value by name for a domain.
    /// Returns `None` if threshold name is unknown.
    fn get_threshold(&self, name: &str, domain: Domain) -> Option<f32>;

    /// Get threshold with fallback to General domain if domain-specific unavailable.
    fn get_threshold_or_general(&self, name: &str, domain: Domain) -> f32;

    /// Observe threshold usage for EWMA drift tracking (Level 1).
    fn observe_threshold_usage(&mut self, name: &str, value: f32);

    /// List all available threshold names.
    fn list_threshold_names() -> &'static [&'static str];
}

impl ThresholdAccessor for AdaptiveThresholdCalibration {
    fn get_threshold(&self, name: &str, domain: Domain) -> Option<f32> {
        let thresholds = self.get_domain_thresholds(domain)?;

        Some(match name {
            // Existing fields
            "theta_opt" => thresholds.theta_opt,
            "theta_acc" => thresholds.theta_acc,
            "theta_warn" => thresholds.theta_warn,
            "theta_dup" => thresholds.theta_dup,
            "theta_edge" => thresholds.theta_edge,
            // Coherence thresholds
            "theta_gate" => thresholds.theta_gate,
            "theta_hypersync" => thresholds.theta_hypersync,
            "theta_fragmentation" => thresholds.theta_fragmentation,
            // Layer thresholds
            "theta_memory_sim" => thresholds.theta_memory_sim,
            "theta_consolidation" => thresholds.theta_consolidation,
            // Dream thresholds
            "theta_dream_activity" => thresholds.theta_dream_activity,
            "theta_semantic_leap" => thresholds.theta_semantic_leap,
            "theta_shortcut_conf" => thresholds.theta_shortcut_conf,
            // Classification thresholds
            "theta_classification" => thresholds.theta_classification,
            // Autonomous thresholds
            "theta_obsolescence_low" => thresholds.theta_obsolescence_low,
            "theta_obsolescence_high" => thresholds.theta_obsolescence_high,
            "theta_obsolescence_mid" => thresholds.theta_obsolescence_mid,
            "theta_drift_slope" => thresholds.theta_drift_slope,
            _ => {
                tracing::warn!(threshold_name = %name, "Unknown threshold name requested");
                return None;
            }
        })
    }

    fn get_threshold_or_general(&self, name: &str, domain: Domain) -> f32 {
        self.get_threshold(name, domain)
            .or_else(|| self.get_threshold(name, Domain::General))
            .unwrap_or_else(|| {
                tracing::error!(threshold_name = %name, "Unknown threshold, returning 0.5 as fallback");
                0.5
            })
    }

    fn observe_threshold_usage(&mut self, name: &str, value: f32) {
        self.observe_threshold(name, value);
    }

    fn list_threshold_names() -> &'static [&'static str] {
        THRESHOLD_NAMES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_threshold_known_names() {
        let atc = AdaptiveThresholdCalibration::new();

        for name in THRESHOLD_NAMES {
            let value = atc.get_threshold(name, Domain::General);
            assert!(value.is_some(), "Threshold {} should exist", name);
            let v = value.unwrap();
            assert!(
                (0.0..=1.0).contains(&v),
                "Threshold {} = {} out of [0,1]",
                name,
                v
            );
        }
    }

    #[test]
    fn test_get_threshold_unknown_returns_none() {
        let atc = AdaptiveThresholdCalibration::new();
        let result = atc.get_threshold("theta_nonexistent", Domain::General);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_threshold_or_general_fallback() {
        let atc = AdaptiveThresholdCalibration::new();
        let value = atc.get_threshold_or_general("theta_opt", Domain::Code);
        assert!((0.60..=0.90).contains(&value));
    }

    #[test]
    fn test_domain_differences() {
        let atc = AdaptiveThresholdCalibration::new();

        let code_gate = atc.get_threshold("theta_gate", Domain::Code).unwrap();
        let creative_gate = atc.get_threshold("theta_gate", Domain::Creative).unwrap();

        // Code (strictness=0.9) should have higher gate than Creative (strictness=0.2)
        assert!(
            code_gate > creative_gate,
            "Code gate {} should be > Creative gate {}",
            code_gate,
            creative_gate
        );
    }

    #[test]
    fn test_list_threshold_names() {
        let names = AdaptiveThresholdCalibration::list_threshold_names();
        assert_eq!(
            names.len(),
            18,
            "Expected 18 threshold names, got {}",
            names.len()
        );
        assert!(names.contains(&"theta_opt"));
        assert!(names.contains(&"theta_gate"));
        assert!(names.contains(&"theta_obsolescence_high"));
        assert!(names.contains(&"theta_obsolescence_mid"));
        assert!(names.contains(&"theta_drift_slope"));
    }

    #[test]
    fn test_all_domains_have_all_thresholds() {
        let atc = AdaptiveThresholdCalibration::new();

        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            for name in THRESHOLD_NAMES {
                let value = atc.get_threshold(name, domain);
                assert!(
                    value.is_some(),
                    "Domain {:?} missing threshold {}",
                    domain,
                    name
                );
            }
        }
    }

    #[test]
    fn test_strictness_affects_gwt_thresholds() {
        let atc = AdaptiveThresholdCalibration::new();

        // Medical is strictest (1.0), Creative is loosest (0.2)
        let medical_gate = atc.get_threshold("theta_gate", Domain::Medical).unwrap();
        let creative_gate = atc.get_threshold("theta_gate", Domain::Creative).unwrap();
        let medical_hypersync = atc
            .get_threshold("theta_hypersync", Domain::Medical)
            .unwrap();
        let creative_hypersync = atc
            .get_threshold("theta_hypersync", Domain::Creative)
            .unwrap();

        assert!(
            medical_gate > creative_gate,
            "Medical gate {} should be > Creative gate {}",
            medical_gate,
            creative_gate
        );
        assert!(
            medical_hypersync > creative_hypersync,
            "Medical hypersync {} should be > Creative hypersync {}",
            medical_hypersync,
            creative_hypersync
        );
    }

    #[test]
    fn test_dream_thresholds_inverse_strictness() {
        let atc = AdaptiveThresholdCalibration::new();

        // Creative should dream more aggressively (lower threshold)
        let medical_dream = atc
            .get_threshold("theta_dream_activity", Domain::Medical)
            .unwrap();
        let creative_dream = atc
            .get_threshold("theta_dream_activity", Domain::Creative)
            .unwrap();

        assert!(
            creative_dream > medical_dream,
            "Creative dream {} should be > Medical dream {} (lower strictness = more dreaming)",
            creative_dream,
            medical_dream
        );
    }

    #[test]
    fn test_obsolescence_monotonicity() {
        let atc = AdaptiveThresholdCalibration::new();

        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            let low = atc.get_threshold("theta_obsolescence_low", domain).unwrap();
            let high = atc
                .get_threshold("theta_obsolescence_high", domain)
                .unwrap();

            assert!(
                high > low,
                "{:?}: obsolescence_high {} should be > obsolescence_low {}",
                domain,
                high,
                low
            );
        }
    }
}
