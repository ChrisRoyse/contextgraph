//! Core tests for lifecycle manager (constructors, accessors, utilities).

#[cfg(test)]
mod tests {
    use crate::config::LifecycleConfig;
    use crate::lifecycle::manager::LifecycleManager;
    use crate::lifecycle::stage::LifecycleStage;

    fn test_config() -> LifecycleConfig {
        LifecycleConfig::default()
    }

    #[test]
    fn test_new() {
        let config = test_config();
        let manager = LifecycleManager::new(&config);

        assert_eq!(manager.interaction_count(), 0);
        assert_eq!(manager.current_stage(), LifecycleStage::Infancy);
        assert!(manager.is_auto_transition_enabled());
    }

    #[test]
    fn test_with_count() {
        let config = test_config();

        let manager = LifecycleManager::with_count(&config, 100);
        assert_eq!(manager.interaction_count(), 100);
        assert_eq!(manager.current_stage(), LifecycleStage::Growth);

        let manager = LifecycleManager::with_count(&config, 600);
        assert_eq!(manager.current_stage(), LifecycleStage::Maturity);
    }

    #[test]
    fn test_current_weights() {
        let config = test_config();

        let manager = LifecycleManager::new(&config);
        let weights = manager.current_weights();
        assert!((weights.lambda_s() - 0.7).abs() < 0.001);

        let manager = LifecycleManager::with_count(&config, 100);
        let weights = manager.current_weights();
        assert!((weights.lambda_s() - 0.5).abs() < 0.001);

        let manager = LifecycleManager::with_count(&config, 600);
        let weights = manager.current_weights();
        assert!((weights.lambda_s() - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_interpolated_weights() {
        let config = test_config();
        let manager = LifecycleManager::with_count(&config, 55);

        let weights = manager.interpolated_weights(&config);
        // Should be between Infancy (0.7) and Growth (0.5)
        assert!(weights.lambda_s() >= 0.5);
        assert!(weights.lambda_s() <= 0.7);
    }

    #[test]
    fn test_current_stance() {
        let config = test_config();

        let manager = LifecycleManager::new(&config);
        assert_eq!(manager.current_stance(), "capture-novelty");

        let manager = LifecycleManager::with_count(&config, 100);
        assert_eq!(manager.current_stance(), "balanced");

        let manager = LifecycleManager::with_count(&config, 600);
        assert_eq!(manager.current_stance(), "curation-coherence");
    }

    #[test]
    fn test_auto_transition_toggle() {
        let config = test_config();
        let mut manager = LifecycleManager::new(&config);

        assert!(manager.is_auto_transition_enabled());

        manager.set_auto_transition(false);
        assert!(!manager.is_auto_transition_enabled());

        // With auto transition disabled, stage shouldn't change
        manager.increment_by(100);
        assert_eq!(manager.current_stage(), LifecycleStage::Infancy); // Still Infancy
    }

    #[test]
    fn test_smooth_transitions_toggle() {
        let config = test_config();
        let mut manager = LifecycleManager::new(&config);

        assert!(manager.is_smooth_transitions_enabled());

        manager.set_smooth_transitions(false);
        assert!(!manager.is_smooth_transitions_enabled());
    }

    #[test]
    fn test_stage_progress() {
        let config = test_config();

        // 25 out of 50 = 50% through Infancy
        let manager = LifecycleManager::with_count(&config, 25);
        assert!((manager.stage_progress() - 0.5).abs() < 0.01);

        // 0 out of 50 = 0% through Infancy
        let manager = LifecycleManager::new(&config);
        assert!((manager.stage_progress() - 0.0).abs() < 0.01);

        // 275 out of 450 range (50-500) = ~50% through Growth
        let manager = LifecycleManager::with_count(&config, 275);
        assert!((manager.stage_progress() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_interactions_until_next_stage() {
        let config = test_config();

        let manager = LifecycleManager::with_count(&config, 40);
        assert_eq!(manager.interactions_until_next_stage(), Some(10)); // 50 - 40

        let manager = LifecycleManager::with_count(&config, 400);
        assert_eq!(manager.interactions_until_next_stage(), Some(100)); // 500 - 400

        let manager = LifecycleManager::with_count(&config, 600);
        assert_eq!(manager.interactions_until_next_stage(), None); // Already in Maturity
    }

    #[test]
    fn test_summary() {
        let config = test_config();
        let manager = LifecycleManager::with_count(&config, 100);

        let summary = manager.summary();
        assert!(summary.contains("Growth"));
        assert!(summary.contains("100"));
        assert!(summary.contains("balanced"));
    }

    #[test]
    fn test_default() {
        let manager = LifecycleManager::default();

        assert_eq!(manager.interaction_count(), 0);
        assert_eq!(manager.current_stage(), LifecycleStage::Infancy);
    }

    #[test]
    fn test_serialization() {
        let config = test_config();
        let manager = LifecycleManager::with_count(&config, 100);

        let json = serde_json::to_string(&manager).unwrap();
        let deserialized: LifecycleManager = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.interaction_count(), 100);
        assert_eq!(deserialized.current_stage(), LifecycleStage::Growth);
    }

    #[test]
    fn test_clone() {
        let config = test_config();
        let manager = LifecycleManager::with_count(&config, 100);
        let cloned = manager.clone();

        assert_eq!(cloned.interaction_count(), 100);
        assert_eq!(cloned.current_stage(), LifecycleStage::Growth);
    }

    #[test]
    fn test_debug() {
        let config = test_config();
        let manager = LifecycleManager::new(&config);
        let debug = format!("{:?}", manager);

        assert!(debug.contains("LifecycleManager"));
        assert!(debug.contains("interaction_count"));
        assert!(debug.contains("current_stage"));
    }

    // ========== TASK-METAUTL-P2-001: Lambda Override Persistence Tests ==========

    #[test]
    fn test_lambda_override_persistence_roundtrip() {
        // TASK-METAUTL-P2-001: Verify lambda_override survives serialization
        use crate::lifecycle::lambda::LifecycleLambdaWeights;

        let config = test_config();
        let mut manager = LifecycleManager::new(&config);

        // Set override
        let override_weights = LifecycleLambdaWeights::new(0.4, 0.6).unwrap();
        manager.set_lambda_override(override_weights);

        // Serialize
        let json = serde_json::to_string(&manager).expect("serialize");

        // Verify override is in JSON
        assert!(json.contains("lambda_override"), "lambda_override should be in JSON");

        // Deserialize
        let restored: LifecycleManager = serde_json::from_str(&json).expect("deserialize");

        // Verify override preserved
        assert!(restored.has_lambda_override(), "override should be preserved");
        let restored_weights = restored.get_effective_weights();
        assert!((restored_weights.lambda_s() - 0.4).abs() < 1e-6, "lambda_s should be 0.4");
        assert!((restored_weights.lambda_c() - 0.6).abs() < 1e-6, "lambda_c should be 0.6");
    }

    #[test]
    fn test_lambda_override_backwards_compatibility() {
        // TASK-METAUTL-P2-001: Old JSON without lambda_override field deserializes correctly
        let json_without_override = r#"{
            "interaction_count": 100,
            "current_stage": "growth",
            "auto_transition": true,
            "transition_hysteresis": 10,
            "last_transition_count": 50,
            "smooth_transitions": false,
            "smoothing_window": 5
        }"#;

        let manager: LifecycleManager = serde_json::from_str(json_without_override)
            .expect("deserialize old format");

        // Should not have override (defaults to None)
        assert!(!manager.has_lambda_override(), "override should default to None");
    }

    #[test]
    fn test_lambda_override_none_not_serialized() {
        // TASK-METAUTL-P2-001: None override should not appear in JSON (cleaner output)
        let config = test_config();
        let manager = LifecycleManager::new(&config);

        let json = serde_json::to_string(&manager).expect("serialize");

        // When None, field should be omitted
        assert!(!json.contains("lambda_override"), "lambda_override should be omitted when None");
    }
}
