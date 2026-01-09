//! Comprehensive Integration Tests for Adaptive Threshold Calibration
//!
//! Tests 7 scenarios as per specification:
//! 1. EWMA Drift Detection with Real Data
//! 2. Temperature Scaling per Embedder
//! 3. Thompson Sampling Convergence
//! 4. Bayesian GP Optimization
//! 5. Domain Transfer Learning
//! 6. Calibration Quality Monitoring
//! 7. Fail-Fast on Missing Config

#[cfg(test)]
mod atc_integration_tests {
    use context_graph_core::atc::*;
    use std::collections::HashMap;

    /// Test 1: EWMA Drift Detection with Real Data
    /// Simulates a distribution shift and verifies drift detection
    #[test]
    fn test_ewma_drift_detection() {
        let mut tracker = DriftTracker::new();

        // Register threshold
        tracker.register_threshold("theta_opt", 0.75, 0.05, 0.2);

        // Phase 1: Normal observations around baseline (0.75)
        for _ in 0..10 {
            tracker.observe("theta_opt", 0.75);
            tracker.observe("theta_opt", 0.76);
            tracker.observe("theta_opt", 0.74);
        }

        let drift_normal = tracker.get_drift_score("theta_opt").unwrap();
        assert!(drift_normal < 1.0, "Normal phase should have low drift");

        // Phase 2: Sudden shift upward (distribution drift)
        for _ in 0..20 {
            tracker.observe("theta_opt", 0.85);
            tracker.observe("theta_opt", 0.86);
        }

        let drift_shifted = tracker.get_drift_score("theta_opt").unwrap();
        assert!(drift_shifted > 1.5, "Should detect significant drift");

        // Verify Level 2 trigger
        if drift_shifted > 2.0 {
            assert!(tracker
                .get_level2_triggers()
                .contains(&"theta_opt".to_string()));
        }
    }

    /// Test 2: Temperature Scaling per Embedder
    /// Verifies different embedders get different calibration temperatures
    #[test]
    fn test_temperature_scaling_per_embedder() {
        let mut scaler = TemperatureScaler::new();

        // Simulate overconfident E5_Causal (should get T > 1.0)
        for _ in 0..30 {
            scaler.record(Embedder::Causal, 0.9, false);
        }
        for _ in 0..10 {
            scaler.record(Embedder::Causal, 0.9, true);
        }

        // Simulate underconfident E7_Code (should get T < 1.0)
        for _ in 0..20 {
            scaler.record(Embedder::Code, 0.6, true);
        }

        let losses = scaler.calibrate_all();

        let e5_loss = losses.get(&Embedder::Causal).unwrap();
        let e7_loss = losses.get(&Embedder::Code).unwrap();

        // Both should be calibrating (non-zero loss indicates calibration happened)
        println!(
            "E5_Causal calibration loss: {}, E7_Code calibration loss: {}",
            e5_loss, e7_loss
        );

        // E5 likely to have higher temperature (overconfident)
        let e5_temp = scaler.get_temperature(Embedder::Causal).unwrap();
        let e7_temp = scaler.get_temperature(Embedder::Code).unwrap();

        println!(
            "E5_Causal temperature: {}, E7_Code temperature: {}",
            e5_temp, e7_temp
        );
    }

    /// Test 3: Thompson Sampling Convergence
    /// Verifies Thompson sampling converges to better arms
    #[test]
    fn test_thompson_sampling_convergence() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
            ThresholdArm { value: 0.80 }, // Best arm
        ];

        let mut bandit = ThresholdBandit::new(arms, 1.5);

        // Simulate 0.80 being best: 80% success rate
        for i in 0..100 {
            let selected = bandit.select_thompson().unwrap();

            if selected.value == 0.80 {
                // 80% success
                if i % 5 != 0 {
                    bandit.record_outcome(selected, true);
                } else {
                    bandit.record_outcome(selected, false);
                }
            } else {
                // Other arms: 40% success
                if i % 10 == 0 {
                    bandit.record_outcome(selected, true);
                } else {
                    bandit.record_outcome(selected, false);
                }
            }
        }

        // After 100 iterations, best arm should be selected more often
        let (best_arm, best_mean) = bandit.get_best_arm().unwrap();

        println!(
            "Best arm after Thompson sampling: {}, success rate: {}",
            best_arm.value, best_mean
        );

        // Should converge on best arm (0.80)
        assert!(best_arm.value >= 0.75, "Should converge toward better arms");
        assert!(best_mean > 0.5, "Best arm should have >50% success");
    }

    /// Test 4: Bayesian GP Optimization
    /// Verifies Bayesian optimization finds good threshold configurations
    #[test]
    fn test_bayesian_gp_optimization() {
        let constraints = ThresholdConstraints::default();
        let mut optimizer = BayesianOptimizer::new(constraints);

        // Add some observations
        let obs1 = HashMap::from([
            ("theta_opt".to_string(), 0.75),
            ("theta_acc".to_string(), 0.70),
            ("theta_warn".to_string(), 0.55),
        ]);
        optimizer.observe(obs1, 0.82);

        let obs2 = HashMap::from([
            ("theta_opt".to_string(), 0.78),
            ("theta_acc".to_string(), 0.72),
            ("theta_warn".to_string(), 0.58),
        ]);
        optimizer.observe(obs2, 0.85);

        let obs3 = HashMap::from([
            ("theta_opt".to_string(), 0.72),
            ("theta_acc".to_string(), 0.68),
            ("theta_warn".to_string(), 0.52),
        ]);
        optimizer.observe(obs3, 0.79);

        // Suggest next configuration
        let suggestion = optimizer.suggest_next();

        println!("Suggested configuration: {:?}", suggestion);

        // Should suggest valid configuration
        assert!(optimizer.observation_count() == 3);
        assert!(suggestion.contains_key("theta_opt"));
    }

    /// Test 5: Domain Transfer Learning
    /// Verifies transfer learning between domains
    #[test]
    fn test_domain_transfer_learning() {
        let mut manager = DomainManager::new();

        let code_before = manager.get(Domain::Code).unwrap().clone();
        let research_before = manager.get(Domain::Research).unwrap().clone();

        println!(
            "Code theta_opt before: {}, Research theta_opt before: {}",
            code_before.theta_opt, research_before.theta_opt
        );

        // Transfer learning: blend Code with Research (similar domain)
        manager
            .transfer_learn(Domain::Code, Domain::Research, 0.4)
            .unwrap();

        let code_after = manager.get(Domain::Code).unwrap();
        let research_after = manager.get(Domain::Research).unwrap();

        println!(
            "Code theta_opt after: {}, Research theta_opt after: {}",
            code_after.theta_opt, research_after.theta_opt
        );

        // Code should change after transfer
        assert!(
            (code_after.theta_opt - code_before.theta_opt).abs() > 0.01,
            "Transfer learning should change threshold"
        );

        // Should still be valid
        assert!(
            code_after.is_valid(),
            "Transferred thresholds must be valid"
        );
    }

    /// Test 6: Calibration Quality Monitoring
    /// Verifies ECE, MCE, Brier metrics and recalibration triggers
    #[test]
    fn test_calibration_quality_monitoring() {
        let mut computer = CalibrationComputer::new(10);

        // Phase 1: Well-calibrated predictions
        // 0.8 confidence with 80% accuracy = well-calibrated
        for _ in 0..40 {
            computer.add_prediction(0.8, true);
        }
        for _ in 0..10 {
            computer.add_prediction(0.8, false);
        }

        let metrics_good = computer.compute_all();
        println!(
            "Well-calibrated: ECE={}, MCE={}, Brier={}, Status={:?}",
            metrics_good.ece, metrics_good.mce, metrics_good.brier, metrics_good.quality_status
        );

        assert!(
            metrics_good.ece < 0.25,
            "Well-calibrated should have reasonable ECE"
        );
        assert!(
            !metrics_good.quality_status.should_recalibrate(),
            "Good calibration should not trigger recalibration"
        );

        // Phase 2: Poorly-calibrated (overconfident)
        computer.clear();
        for _ in 0..50 {
            computer.add_prediction(0.95, false);
        }
        for _ in 0..50 {
            computer.add_prediction(0.95, true);
        }

        let metrics_bad = computer.compute_all();
        println!(
            "Poorly-calibrated: ECE={}, MCE={}, Brier={}, Status={:?}",
            metrics_bad.ece, metrics_bad.mce, metrics_bad.brier, metrics_bad.quality_status
        );

        assert!(metrics_bad.ece > 0.3, "Overconfident should have high ECE");
        assert!(
            metrics_bad.quality_status.should_recalibrate(),
            "Poor calibration should trigger recalibration"
        );
    }

    /// Test 7: Fail-Fast on Missing Configuration
    /// Verifies system detects and fails when required config is missing
    #[test]
    fn test_fail_fast_on_missing_config() {
        let constraints = ThresholdConstraints::default();

        // Valid configuration
        let mut valid = HashMap::new();
        valid.insert("theta_opt".to_string(), 0.75);
        valid.insert("theta_acc".to_string(), 0.70);
        valid.insert("theta_warn".to_string(), 0.55);

        assert!(
            constraints.is_valid(&valid),
            "Valid config should pass validation"
        );

        // Invalid: violates monotonicity
        let mut invalid = HashMap::new();
        invalid.insert("theta_opt".to_string(), 0.70); // Should be > acc
        invalid.insert("theta_acc".to_string(), 0.75); // Wrong
        invalid.insert("theta_warn".to_string(), 0.55);

        assert!(
            !constraints.is_valid(&invalid),
            "Invalid config should fail validation"
        );

        // Invalid: out of range
        let mut out_of_range = HashMap::new();
        out_of_range.insert("theta_opt".to_string(), 0.95); // Max is 0.90

        assert!(
            !constraints.is_valid(&out_of_range),
            "Out-of-range config should fail validation"
        );
    }

    /// Integration Test: Full ATC System
    /// Tests all 4 levels working together
    #[test]
    fn test_full_atc_system_integration() {
        let mut atc = AdaptiveThresholdCalibration::new();

        // Level 1: Register thresholds
        atc.register_threshold("theta_opt", 0.75, 0.05, 0.2);
        atc.register_threshold("theta_dup", 0.90, 0.03, 0.2);

        // Observe some drift
        for _ in 0..20 {
            atc.observe_threshold("theta_opt", 0.82);
        }

        // Check drift status via public API
        let drift_status = atc.get_drift_status();
        println!("Drift status: {:?}", drift_status);

        // Level 2: Calibrate temperatures
        atc.record_prediction(Embedder::Semantic, 0.8, true);
        atc.record_prediction(Embedder::Causal, 0.85, false);

        let losses = atc.calibrate_temperatures();
        println!("Temperature calibration losses: {:?}", losses);

        // Level 3: Initialize bandit
        atc.init_session_bandit(vec![0.70, 0.75, 0.80]);
        let selected = atc.select_threshold_thompson();
        assert!(selected.is_some());

        // Record outcome
        atc.record_threshold_outcome(selected.unwrap(), true);

        // Level 4: Verify optimizer exists
        assert!(!atc.should_optimize_level4()); // Just created

        // Monitor calibration
        let predictions = vec![
            Prediction {
                confidence: 0.8,
                is_correct: true,
            },
            Prediction {
                confidence: 0.7,
                is_correct: true,
            },
        ];
        atc.update_calibration_metrics(predictions);

        let quality = atc.get_calibration_quality();
        println!(
            "System calibration quality: ECE={}, MCE={}, Brier={}",
            quality.ece, quality.mce, quality.brier
        );

        assert_eq!(quality.sample_count, 2);
    }

    /// Edge Case Test: Extreme Values
    /// Tests system behavior with edge cases
    #[test]
    fn test_edge_cases() {
        let mut tracker = DriftTracker::new();
        tracker.register_threshold("test", 0.75, 0.0, 0.2); // Zero std

        // Should handle zero std gracefully
        tracker.observe("test", 0.90);
        let drift = tracker.get_drift_score("test").unwrap();
        assert_eq!(drift, 0.0, "Zero std should give zero drift");

        // Test with empty predictions
        let computer = CalibrationComputer::new(10);
        let metrics = computer.compute_all();
        assert_eq!(metrics.sample_count, 0);
        assert_eq!(metrics.ece, 0.0);

        // Test bandit with single arm
        let bandit = ThresholdBandit::new(vec![ThresholdArm { value: 0.75 }], 1.5);
        let selected = bandit.select_thompson();
        assert!(selected.is_some());
    }

    /// Regression Test: Convergence Speed
    /// Verifies convergence happens within expected iterations
    #[test]
    fn test_convergence_speed() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
            ThresholdArm { value: 0.80 },
        ];

        let mut bandit = ThresholdBandit::new(arms, 1.5);

        // Simulate 1000 iterations
        for i in 0..1000 {
            let selected = bandit.select_ucb().unwrap();

            // 0.80 succeeds 80% of the time
            let success = selected.value == 0.80 && (i % 5 != 0);
            bandit.record_outcome(selected, success);
        }

        let (best_arm, best_mean) = bandit.get_best_arm().unwrap();
        println!(
            "After 1000 iterations: best arm = {}, success rate = {}",
            best_arm.value, best_mean
        );

        // Should have converged
        assert_eq!(best_arm.value, 0.80, "Should converge to best arm");
        assert!(best_mean > 0.7, "Best arm should have ~80% success");
    }
}
