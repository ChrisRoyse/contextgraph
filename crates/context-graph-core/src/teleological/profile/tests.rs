//! Tests for teleological profile components.

#[cfg(test)]
mod tests {
    use crate::teleological::profile::{FusionStrategy, TeleologicalProfile};
    use crate::teleological::types::NUM_EMBEDDERS;

    #[test]
    fn test_profile_new() {
        let profile = TeleologicalProfile::new(
            "test",
            "Test Profile",
            crate::teleological::profile::TaskType::General,
        );

        assert_eq!(profile.id.as_str(), "test");
        assert_eq!(profile.name, "Test Profile");
        assert_eq!(
            profile.task_type,
            crate::teleological::profile::TaskType::General
        );
        assert_eq!(profile.sample_count, 0);
        assert!(!profile.is_system);

        // Weights should be uniform
        let expected_weight = 1.0 / NUM_EMBEDDERS as f32;
        for &w in profile.embedding_weights.iter() {
            assert!((w - expected_weight).abs() < 0.001);
        }
    }

    #[test]
    fn test_profile_system() {
        let profile =
            TeleologicalProfile::system(crate::teleological::profile::TaskType::CodeSearch);

        assert!(profile.is_system);
        assert!(profile.id.as_str().contains("system"));

        // Primary embedders should have higher weights
        let primary = crate::teleological::profile::TaskType::CodeSearch.primary_embedders();
        for &idx in primary {
            assert!(
                profile.embedding_weights[idx] > 0.1,
                "Primary embedder {} should have high weight",
                idx
            );
        }
    }

    #[test]
    fn test_profile_normalize_weights() {
        let mut profile = TeleologicalProfile::new(
            "test",
            "Test",
            crate::teleological::profile::TaskType::General,
        );

        // Set non-normalized weights
        profile.embedding_weights = [1.0; NUM_EMBEDDERS];
        profile.normalize_weights();

        let sum: f32 = profile.embedding_weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_profile_similarity() {
        let p1 = TeleologicalProfile::code_implementation();
        let p2 = TeleologicalProfile::code_implementation();

        let sim = p1.similarity(&p2);
        assert!((sim - 1.0).abs() < 0.001);

        let p3 = TeleologicalProfile::conceptual_research();
        let sim2 = p1.similarity(&p3);
        assert!(sim2 < 0.95); // Should be different
    }

    #[test]
    fn test_profile_serialization() {
        let profile = TeleologicalProfile::code_implementation();

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: TeleologicalProfile = serde_json::from_str(&json).unwrap();

        assert_eq!(profile.id, deserialized.id);
        assert_eq!(profile.embedding_weights, deserialized.embedding_weights);

        // Also verify FusionStrategy roundtrip
        let strategy = FusionStrategy::TuckerDecomposition { ranks: (2, 3, 64) };
        let json = serde_json::to_string(&strategy).unwrap();
        let deser_strategy: FusionStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(strategy, deser_strategy);
    }
}
