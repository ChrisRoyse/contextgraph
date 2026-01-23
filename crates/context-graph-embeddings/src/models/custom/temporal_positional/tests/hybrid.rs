//! Integration tests for hybrid E4 session+position encoding mode.
//!
//! These tests verify that the hybrid mode correctly combines session signatures
//! with position encodings to enable both session clustering and position ordering.

use crate::models::custom::temporal_positional::TemporalPositionalModel;
use crate::traits::EmbeddingModel;
use crate::types::ModelInput;

use super::cosine_similarity;

/// Create a text input with instruction.
fn text_input(instruction: &str) -> ModelInput {
    ModelInput::text_with_instruction("test", instruction).expect("Failed to create input")
}

// =============================================================================
// HYBRID MODE BASIC TESTS
// =============================================================================

#[tokio::test]
async fn test_hybrid_embedding_dimension() {
    let model = TemporalPositionalModel::new();
    assert!(model.is_hybrid_mode(), "Default should be hybrid mode");

    let input = text_input("session:abc123 sequence:42");
    let emb = model.embed(&input).await.unwrap();

    assert_eq!(emb.vector.len(), 512, "Hybrid embedding should be 512D");
}

#[tokio::test]
async fn test_hybrid_embedding_normalized() {
    let model = TemporalPositionalModel::new();
    let input = text_input("session:test-session sequence:1");
    let emb = model.embed(&input).await.unwrap();

    let norm: f32 = emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-4,
        "Hybrid embedding should be L2 normalized, got norm: {}",
        norm
    );
}

#[tokio::test]
async fn test_legacy_mode_still_works() {
    let model = TemporalPositionalModel::with_hybrid_mode(false);
    assert!(!model.is_hybrid_mode(), "Should be legacy mode");

    let input = text_input("sequence:42");
    let emb = model.embed(&input).await.unwrap();

    assert_eq!(emb.vector.len(), 512, "Legacy embedding should be 512D");

    let norm: f32 = emb.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "Should be normalized");
}

// =============================================================================
// SESSION CLUSTERING TESTS
// =============================================================================

#[tokio::test]
async fn test_same_session_similar() {
    let model = TemporalPositionalModel::new();

    // Same session, different positions
    let emb1 = model.embed(&text_input("session:abc123 sequence:1")).await.unwrap();
    let emb2 = model.embed(&text_input("session:abc123 sequence:2")).await.unwrap();

    let sim = cosine_similarity(&emb1.vector, &emb2.vector);
    assert!(
        sim > 0.7,
        "Same session should have high similarity: {}",
        sim
    );
}

#[tokio::test]
async fn test_different_session_dissimilar() {
    let model = TemporalPositionalModel::new();

    // Different sessions, same position
    let emb1 = model.embed(&text_input("session:abc123 sequence:1")).await.unwrap();
    let emb2 = model.embed(&text_input("session:xyz789 sequence:1")).await.unwrap();

    let sim = cosine_similarity(&emb1.vector, &emb2.vector);
    assert!(
        sim < 0.6,
        "Different sessions should have lower similarity: {}",
        sim
    );
}

#[tokio::test]
async fn test_session_separation_ratio() {
    let model = TemporalPositionalModel::new();

    // Generate embeddings for two sessions
    let mut session1_embs = Vec::new();
    for i in 0..5 {
        let emb = model
            .embed(&text_input(&format!("session:session1 sequence:{}", i)))
            .await
            .unwrap();
        session1_embs.push(emb);
    }

    let mut session2_embs = Vec::new();
    for i in 0..5 {
        let emb = model
            .embed(&text_input(&format!("session:session2 sequence:{}", i)))
            .await
            .unwrap();
        session2_embs.push(emb);
    }

    // Compute intra-session similarity (same session pairs)
    let mut intra_sims = Vec::new();
    for i in 0..session1_embs.len() {
        for j in (i + 1)..session1_embs.len() {
            intra_sims.push(cosine_similarity(
                &session1_embs[i].vector,
                &session1_embs[j].vector,
            ));
        }
    }
    let avg_intra: f32 = intra_sims.iter().sum::<f32>() / intra_sims.len() as f32;

    // Compute inter-session similarity (different session pairs)
    let mut inter_sims = Vec::new();
    for emb1 in &session1_embs {
        for emb2 in &session2_embs {
            inter_sims.push(cosine_similarity(&emb1.vector, &emb2.vector));
        }
    }
    let avg_inter: f32 = inter_sims.iter().sum::<f32>() / inter_sims.len() as f32;

    // Session separation ratio should be >= 1.5 (intra should be higher than inter)
    let separation_ratio = avg_intra / avg_inter.max(0.01);
    assert!(
        separation_ratio >= 1.5,
        "Session separation ratio should be >= 1.5, got: {} (intra={}, inter={})",
        separation_ratio,
        avg_intra,
        avg_inter
    );
}

// =============================================================================
// POSITION ORDERING TESTS (WITHIN SESSION)
// =============================================================================

#[tokio::test]
async fn test_position_ordering_preserved() {
    let model = TemporalPositionalModel::new();

    // Same session, positions 1, 2, 10
    let emb1 = model.embed(&text_input("session:sess1 sequence:1")).await.unwrap();
    let emb2 = model.embed(&text_input("session:sess1 sequence:2")).await.unwrap();
    let emb10 = model.embed(&text_input("session:sess1 sequence:10")).await.unwrap();

    let sim_12 = cosine_similarity(&emb1.vector, &emb2.vector);
    let sim_110 = cosine_similarity(&emb1.vector, &emb10.vector);

    // Adjacent positions should be more similar than distant positions
    assert!(
        sim_12 > sim_110,
        "Adjacent positions should be more similar: sim(1,2)={} vs sim(1,10)={}",
        sim_12,
        sim_110
    );
}

#[tokio::test]
async fn test_intra_session_ordering_accuracy() {
    let model = TemporalPositionalModel::new();

    // Generate 10 positions in same session
    let positions: Vec<u64> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let mut embs = Vec::new();
    for pos in &positions {
        let emb = model
            .embed(&text_input(&format!("session:ordering-test sequence:{}", pos)))
            .await
            .unwrap();
        embs.push(emb);
    }

    // Check if ordering is preserved (earlier positions more similar to anchor than later ones)
    // Using position 0 as anchor
    let anchor = &embs[0];
    let mut correct_pairs = 0;
    let mut total_pairs = 0;

    for i in 1..embs.len() {
        for j in (i + 1)..embs.len() {
            let sim_i = cosine_similarity(&anchor.vector, &embs[i].vector);
            let sim_j = cosine_similarity(&anchor.vector, &embs[j].vector);

            // Position i is closer to anchor (0) than position j, so sim_i should be >= sim_j
            if sim_i >= sim_j {
                correct_pairs += 1;
            }
            total_pairs += 1;
        }
    }

    let accuracy = correct_pairs as f64 / total_pairs as f64;
    assert!(
        accuracy >= 0.8,
        "Intra-session ordering accuracy should be >= 80%, got: {:.1}%",
        accuracy * 100.0
    );
}

// =============================================================================
// BACKWARD COMPATIBILITY TESTS
// =============================================================================

#[tokio::test]
async fn test_backward_compatible_no_session() {
    let model = TemporalPositionalModel::new();

    // Legacy format without session should work (uses sentinel)
    let input = text_input("sequence:42");
    let emb = model.embed(&input).await.unwrap();

    assert_eq!(emb.vector.len(), 512);
}

#[tokio::test]
async fn test_backward_compatible_timestamp() {
    let model = TemporalPositionalModel::new();

    // Legacy timestamp format
    let input = text_input("timestamp:2024-01-15T10:30:00Z");
    let emb = model.embed(&input).await.unwrap();

    assert_eq!(emb.vector.len(), 512);
}

#[tokio::test]
async fn test_backward_compatible_no_instruction() {
    let model = TemporalPositionalModel::new();

    // No instruction - should use current time
    let input = ModelInput::text("test").expect("Failed to create input");
    let emb = model.embed(&input).await.unwrap();

    assert_eq!(emb.vector.len(), 512);
}

// =============================================================================
// DETERMINISM TESTS
// =============================================================================

#[tokio::test]
async fn test_hybrid_deterministic() {
    let model = TemporalPositionalModel::new();

    let input = text_input("session:determinism-test sequence:42");
    let emb1 = model.embed(&input).await.unwrap();
    let emb2 = model.embed(&input).await.unwrap();

    // Embeddings should be identical
    for (a, b) in emb1.vector.iter().zip(&emb2.vector) {
        assert!(
            (a - b).abs() < 1e-6,
            "Same input should produce identical embedding"
        );
    }
}

#[tokio::test]
async fn test_different_text_same_instruction_identical() {
    let model = TemporalPositionalModel::new();

    // E4 only uses the instruction field, text content should be ignored
    let input1 =
        ModelInput::text_with_instruction("some text", "session:test sequence:1").unwrap();
    let input2 =
        ModelInput::text_with_instruction("different text", "session:test sequence:1").unwrap();

    let emb1 = model.embed(&input1).await.unwrap();
    let emb2 = model.embed(&input2).await.unwrap();

    for (a, b) in emb1.vector.iter().zip(&emb2.vector) {
        assert!((a - b).abs() < 1e-6, "Same instruction should match");
    }
}

// =============================================================================
// UUID SESSION ID TESTS
// =============================================================================

#[tokio::test]
async fn test_uuid_session_ids() {
    let model = TemporalPositionalModel::new();

    let uuid1 = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
    let uuid2 = "11111111-2222-3333-4444-555555555555";

    let emb1 = model
        .embed(&text_input(&format!("session:{} sequence:1", uuid1)))
        .await
        .unwrap();
    let emb2 = model
        .embed(&text_input(&format!("session:{} sequence:1", uuid2)))
        .await
        .unwrap();

    let sim = cosine_similarity(&emb1.vector, &emb2.vector);
    assert!(
        sim < 0.6,
        "Different UUID sessions should be orthogonal: {}",
        sim
    );
}

// =============================================================================
// MODEL CONFIGURATION TESTS
// =============================================================================

#[tokio::test]
async fn test_with_hybrid_mode_true() {
    let model = TemporalPositionalModel::with_hybrid_mode(true);
    assert!(model.is_hybrid_mode());
    assert_eq!(model.base(), 10000.0);
}

#[tokio::test]
async fn test_with_hybrid_mode_false() {
    let model = TemporalPositionalModel::with_hybrid_mode(false);
    assert!(!model.is_hybrid_mode());
    assert_eq!(model.base(), 10000.0);
}

#[tokio::test]
async fn test_with_base_keeps_hybrid_mode() {
    let model = TemporalPositionalModel::with_base(5000.0).unwrap();
    assert!(
        model.is_hybrid_mode(),
        "with_base should preserve hybrid default"
    );
    assert_eq!(model.base(), 5000.0);
}

#[tokio::test]
async fn test_with_base_and_hybrid_mode() {
    let model = TemporalPositionalModel::with_base_and_hybrid_mode(5000.0, false).unwrap();
    assert!(!model.is_hybrid_mode());
    assert_eq!(model.base(), 5000.0);
}

// =============================================================================
// SESSION SIGNATURE COMPONENT ISOLATION TESTS
// =============================================================================

/// Helper to compute cosine similarity of two slices (handles non-normalized vectors).
fn slice_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Test that different sessions produce different session signatures.
///
/// This is the critical integration test for the e4_instruction() fix.
/// It verifies that when different session_ids are passed, the E4 model
/// produces genuinely different session signature components (first 256D).
#[tokio::test]
async fn test_different_sessions_produce_different_signatures() {
    let model = TemporalPositionalModel::new();
    assert!(model.is_hybrid_mode());

    // Two embeddings with DIFFERENT session_ids but SAME sequence position
    let emb1 = model
        .embed(&text_input("session:session-A sequence:1"))
        .await
        .unwrap();
    let emb2 = model
        .embed(&text_input("session:session-B sequence:1"))
        .await
        .unwrap();

    let vec1 = &emb1.vector;
    let vec2 = &emb2.vector;

    // Extract session signature components (first 256D)
    let sig1 = &vec1[0..256];
    let sig2 = &vec2[0..256];

    // Session signatures should be DIFFERENT for different sessions
    let sig_similarity = slice_cosine_similarity(sig1, sig2);

    assert!(
        sig_similarity < 0.5,
        "Session signatures should be different for different sessions, got similarity: {}",
        sig_similarity
    );

    // Position components (last 256D) should be IDENTICAL for same sequence
    let pos1 = &vec1[256..512];
    let pos2 = &vec2[256..512];

    let pos_similarity = slice_cosine_similarity(pos1, pos2);

    assert!(
        pos_similarity > 0.99,
        "Position encodings should be identical for same sequence, got similarity: {}",
        pos_similarity
    );
}

/// Test that same session with different positions has similar session signature but different position.
#[tokio::test]
async fn test_same_session_similar_signature_different_position() {
    let model = TemporalPositionalModel::new();

    // SAME session, DIFFERENT positions
    let emb1 = model
        .embed(&text_input("session:shared-session sequence:1"))
        .await
        .unwrap();
    let emb2 = model
        .embed(&text_input("session:shared-session sequence:100"))
        .await
        .unwrap();

    let vec1 = &emb1.vector;
    let vec2 = &emb2.vector;

    // Session signatures (first 256D) should be IDENTICAL for same session
    let sig1 = &vec1[0..256];
    let sig2 = &vec2[0..256];

    let sig_similarity = slice_cosine_similarity(sig1, sig2);

    assert!(
        sig_similarity > 0.99,
        "Session signatures should be identical for same session, got similarity: {}",
        sig_similarity
    );

    // Position components (last 256D) should be DIFFERENT for different sequences
    let pos1 = &vec1[256..512];
    let pos2 = &vec2[256..512];

    let pos_similarity = slice_cosine_similarity(pos1, pos2);

    // Positions 1 vs 100 should be quite different
    assert!(
        pos_similarity < 0.8,
        "Position encodings should differ for distant sequences, got similarity: {}",
        pos_similarity
    );
}

/// Test that missing session_id produces sentinel signature (backward compatibility).
#[tokio::test]
async fn test_missing_session_produces_sentinel() {
    let model = TemporalPositionalModel::new();

    // No session_id in instruction (legacy format)
    let emb1 = model.embed(&text_input("sequence:1")).await.unwrap();
    let emb2 = model.embed(&text_input("sequence:2")).await.unwrap();

    let vec1 = &emb1.vector;
    let vec2 = &emb2.vector;

    // Session signatures should be IDENTICAL (both use sentinel)
    let sig1 = &vec1[0..256];
    let sig2 = &vec2[0..256];

    let sig_similarity = slice_cosine_similarity(sig1, sig2);

    assert!(
        sig_similarity > 0.99,
        "Both should use sentinel session signature, got similarity: {}",
        sig_similarity
    );
}
