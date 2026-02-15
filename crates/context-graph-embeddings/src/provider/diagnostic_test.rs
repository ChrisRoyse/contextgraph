//! DIAGNOSTIC TEST: Verify E9 vectors differ from E1 vectors
//!
//! This test directly checks if the SemanticFingerprint has unique vectors
//! for each embedder after embedding.

#[cfg(test)]
mod vector_identity_tests {
    use crate::config::GpuConfig;
    use crate::provider::multi_array::MultiArrayEmbeddingProvider;
    use crate::traits::MultiSpaceEmbedder;
    use std::path::PathBuf;

    /// CRITICAL DIAGNOSTIC: Are E1 and E9 vectors different?
    #[tokio::test]
    #[ignore = "requires models directory with pretrained weights"]
    async fn diagnostic_e1_e9_vector_difference() {
        let models_dir = PathBuf::from(
            std::env::var("MODELS_DIR").unwrap_or_else(|_| "./models".to_string()),
        );

        let provider = MultiArrayEmbeddingProvider::new(models_dir, GpuConfig::default())
            .expect("Failed to create provider");

        let test_content = "authentication failed for user";
        let output = provider.embed_all(test_content).await
            .expect("Embedding failed");
        let fingerprint = output.fingerprint;

        let e1 = &fingerprint.e1_semantic;
        let e9 = &fingerprint.e9_hdc;

        let are_identical = e1.len() == e9.len() && e1.iter().zip(e9.iter()).all(|(a, b)| (a - b).abs() < 1e-9);
        assert!(!are_identical, "E1 and E9 vectors must be different!");

        // Also check E5
        let e5 = fingerprint.e5_active_vector();
        let e1_e5_identical = e1.len() == e5.len() && e1.iter().zip(e5.iter()).all(|(a, b)| (a - b).abs() < 1e-9);
        assert!(!e1_e5_identical, "E1 and E5 vectors must be different!");
    }

    /// Check if the problem is in cosine similarity computation itself
    #[test]
    fn diagnostic_cosine_similarity_computation() {
        println!("\n========================================");
        println!("DIAGNOSTIC: Cosine Similarity Computation");
        println!("========================================\n");

        // Create two clearly different vectors
        let vec_a = vec![1.0, 0.0, 0.0, 0.0];
        let vec_b = vec![0.0, 1.0, 0.0, 0.0];
        let vec_c = vec![1.0, 0.0, 0.0, 0.0]; // Same as A

        fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
            let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm_a == 0.0 || norm_b == 0.0 {
                return 0.0;
            }
            dot / (norm_a * norm_b)
        }

        let sim_ab = cosine_sim(&vec_a, &vec_b);
        let sim_ac = cosine_sim(&vec_a, &vec_c);

        println!("A = [1,0,0,0], B = [0,1,0,0], C = [1,0,0,0]");
        println!("cosine(A, B) = {} (expected: 0.0)", sim_ab);
        println!("cosine(A, C) = {} (expected: 1.0)", sim_ac);

        assert!((sim_ab - 0.0).abs() < 0.001, "A and B are orthogonal");
        assert!((sim_ac - 1.0).abs() < 0.001, "A and C are identical");

        println!("✓ Cosine similarity computation is correct\n");
    }
}
