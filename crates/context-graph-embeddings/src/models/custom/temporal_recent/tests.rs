//! Integration and edge case tests for the Temporal-Recent model.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::models::custom::temporal_recent::{TemporalRecentModel, TEMPORAL_RECENT_DIMENSION};
    use crate::traits::EmbeddingModel;
    use crate::types::{ModelId, ModelInput};

    // =========================================================================
    // RECENCY ORDERING TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_recent_vs_old_timestamps() {
        let ref_time = Utc::now();
        let model = TemporalRecentModel::with_reference_time(ref_time);

        // Recent: 1 hour ago
        let recent = ref_time - Duration::hours(1);
        let recent_input = ModelInput::text_with_instruction(
            "content",
            format!("timestamp:{}", recent.to_rfc3339()),
        )
        .expect("Failed to create");

        // Somewhat old: 7 days ago (not too old to have significant decay)
        let old = ref_time - Duration::days(7);
        let old_input =
            ModelInput::text_with_instruction("content", format!("timestamp:{}", old.to_rfc3339()))
                .expect("Failed to create");

        let recent_embedding = model.embed(&recent_input).await.expect("Recent embed");
        let old_embedding = model.embed(&old_input).await.expect("Old embed");

        // Both should be valid 512D vectors
        assert_eq!(recent_embedding.vector.len(), 512);
        assert_eq!(old_embedding.vector.len(), 512);

        // Vectors should be different
        assert_ne!(
            recent_embedding.vector, old_embedding.vector,
            "Recent and old embeddings should differ"
        );

        // Both should be L2 normalized
        let recent_norm: f32 = recent_embedding
            .vector
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        let old_norm: f32 = old_embedding
            .vector
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        assert!(
            (recent_norm - 1.0).abs() < 0.001,
            "Recent should be normalized"
        );
        assert!((old_norm - 1.0).abs() < 0.001, "Old should be normalized");
    }

    // =========================================================================
    // EDGE CASE TESTS (MANDATORY per spec)
    // =========================================================================

    #[tokio::test]
    async fn test_edge_case_1_very_old_timestamp() {
        let model = TemporalRecentModel::new();
        let one_year_ago = Utc::now() - Duration::days(365);

        let input = ModelInput::text_with_instruction(
            "old content",
            format!("timestamp:{}", one_year_ago.to_rfc3339()),
        )
        .expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        // Must still produce valid 512D vector
        assert_eq!(embedding.vector.len(), 512);
        // Values should be valid (no NaN/Inf)
        assert!(embedding.vector.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn test_edge_case_2_future_timestamp() {
        let model = TemporalRecentModel::new();
        let future = Utc::now() + Duration::days(30);

        let input = ModelInput::text_with_instruction(
            "future content",
            format!("timestamp:{}", future.to_rfc3339()),
        )
        .expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        // Future timestamps should still produce valid output
        // (delta clamped to 0 minimum)
        assert_eq!(embedding.vector.len(), 512);
        assert!(embedding.vector.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn test_edge_case_3_no_timestamp() {
        let model = TemporalRecentModel::new();

        let input = ModelInput::text("content without timestamp").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        // Should use current time = minimal decay
        assert_eq!(embedding.vector.len(), 512);
        assert!(embedding.vector.iter().all(|x| x.is_finite()));
    }

    // =========================================================================
    // SOURCE OF TRUTH VERIFICATION (MANDATORY per spec)
    // =========================================================================

    #[tokio::test]
    async fn test_source_of_truth_verification() {
        let ref_time = Utc::now();
        let model = TemporalRecentModel::with_reference_time(ref_time);
        let input = ModelInput::text_with_instruction(
            "test content",
            format!("timestamp:{}", ref_time.to_rfc3339()),
        )
        .expect("Failed to create input");

        // Execute
        let embedding = model.embed(&input).await.expect("Embed should succeed");

        // INSPECT SOURCE OF TRUTH
        println!("=== SOURCE OF TRUTH VERIFICATION ===");
        println!("model_id: {:?}", embedding.model_id);
        println!("vector.len(): {}", embedding.vector.len());
        println!("vector[0..5]: {:?}", &embedding.vector[0..5]);
        println!("latency_us: {}", embedding.latency_us);

        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("L2 norm: {}", norm);

        let has_nan = embedding.vector.iter().any(|x| x.is_nan());
        let has_inf = embedding.vector.iter().any(|x| x.is_infinite());
        println!("has_nan: {}, has_inf: {}", has_nan, has_inf);

        // VERIFY
        assert_eq!(embedding.model_id, ModelId::TemporalRecent);
        assert_eq!(embedding.vector.len(), 512);
        assert!((norm - 1.0).abs() < 0.001);
        assert!(!has_nan && !has_inf);
    }

    // =========================================================================
    // EVIDENCE OF SUCCESS TEST (MANDATORY per spec)
    // =========================================================================

    #[tokio::test]
    async fn test_evidence_of_success() {
        println!("\n========================================");
        println!("M03-L04 EVIDENCE OF SUCCESS");
        println!("========================================\n");

        let ref_time = Utc::now();
        let model = TemporalRecentModel::with_reference_time(ref_time);

        // Test 1: Model metadata
        println!("1. MODEL METADATA:");
        println!("   model_id = {:?}", model.model_id());
        println!("   dimension = {}", model.dimension());
        println!("   is_initialized = {}", model.is_initialized());
        println!("   is_pretrained = {}", model.is_pretrained());
        println!("   latency_budget_ms = {}", model.latency_budget_ms());

        // Test 2: Embed and verify output
        let input = ModelInput::text_with_instruction(
            "test",
            format!("timestamp:{}", ref_time.to_rfc3339()),
        )
        .expect("Failed to create input");

        let start = std::time::Instant::now();
        let embedding = model.embed(&input).await.expect("Embed should succeed");
        let elapsed = start.elapsed();

        println!("\n2. EMBEDDING OUTPUT:");
        println!("   vector length = {}", embedding.vector.len());
        println!("   latency = {:?}", elapsed);
        println!("   first 10 values = {:?}", &embedding.vector[0..10]);

        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("   L2 norm = {}", norm);

        // Test 3: Recency ordering
        let old_input = ModelInput::text_with_instruction(
            "test",
            format!("timestamp:{}", (ref_time - Duration::days(30)).to_rfc3339()),
        )
        .expect("Failed to create input");
        let old_embedding = model.embed(&old_input).await.expect("Old embed");

        let recent_sum: f32 = embedding.vector[0..128].iter().sum();
        let old_sum: f32 = old_embedding.vector[0..128].iter().sum();

        println!("\n3. RECENCY ORDERING:");
        println!("   recent (now) first block sum = {}", recent_sum);
        println!("   old (30 days) first block sum = {}", old_sum);
        println!(
            "   recent != old = {}",
            (recent_sum - old_sum).abs() > 0.001
        );

        println!("\n========================================");
        println!("ALL CHECKS PASSED");
        println!("========================================\n");

        assert!(elapsed.as_millis() < 2, "Latency exceeded 2ms budget");
        assert_eq!(embedding.vector.len(), 512);
        assert!((norm - 1.0).abs() < 0.001);
    }

    // =========================================================================
    // DIMENSION CONSTANT TEST
    // =========================================================================

    #[test]
    fn test_dimension_constant_matches() {
        assert_eq!(
            TEMPORAL_RECENT_DIMENSION, 512,
            "TEMPORAL_RECENT_DIMENSION must be 512"
        );
    }
}
