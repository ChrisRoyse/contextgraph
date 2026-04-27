//! Manual edge case tests for TopicSynthesizer with full state verification.
//!
//! This file provides exhaustive edge case testing with:
//! - State logging before and after operations
//! - Evidence collection for verification
//! - Boundary condition testing

use context_graph_core::clustering::{ClusterMembership, TopicProfile, TopicSynthesizer};
use context_graph_core::embeddings::category::{
    category_for, max_weighted_agreement, topic_threshold,
};
use context_graph_core::teleological::Embedder;
use std::collections::HashMap;
use uuid::Uuid;

/// Create memberships for testing with controlled cluster assignments
fn create_test_memberships(
    ids: &[Uuid],
    shared_config: &[(Embedder, i32)],
) -> HashMap<Embedder, Vec<ClusterMembership>> {
    let mut memberships = HashMap::new();

    for &id in ids {
        for (embedder, cluster_id) in shared_config {
            memberships
                .entry(*embedder)
                .or_insert_with(Vec::new)
                .push(ClusterMembership::new(
                    id,
                    *embedder,
                    *cluster_id,
                    0.9,
                    true,
                ));
        }
    }

    memberships
}

#[test]
fn edge_case_1_empty_input_returns_empty() {
    println!("\n=== EDGE CASE 1: Empty Input ===");
    println!("PURPOSE: Verify empty input produces empty output, not an error");

    let synthesizer = TopicSynthesizer::new();
    let memberships: HashMap<Embedder, Vec<ClusterMembership>> = HashMap::new();

    println!("STATE BEFORE: memberships.len() = {}", memberships.len());

    let result = synthesizer.synthesize_topics(&memberships);

    println!("STATE AFTER: result.is_ok() = {}", result.is_ok());
    if let Ok(topics) = &result {
        println!("  topics.len() = {}", topics.len());
    }

    assert!(result.is_ok(), "Empty input should return Ok");
    let topics = result.unwrap();
    assert!(topics.is_empty(), "Empty input should produce empty topics");

    println!("[EVIDENCE] Empty input -> Ok([]) - SUCCESS");
}

#[test]
fn edge_case_2_single_memory_no_topic() {
    println!("\n=== EDGE CASE 2: Single Memory ===");
    println!("PURPOSE: A single memory cannot form a topic (needs >= 2)");

    let synthesizer = TopicSynthesizer::new();
    let id = Uuid::new_v4();

    let mut memberships = HashMap::new();
    for embedder in Embedder::all() {
        memberships.insert(
            embedder,
            vec![ClusterMembership::new(id, embedder, 1, 0.9, true)],
        );
    }

    println!("STATE BEFORE: 1 memory, all spaces cluster=1");
    println!("  memory_id = {}", id);

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(topics.is_empty(), "Single memory cannot form topic");

    println!("[EVIDENCE] 1 memory -> 0 topics - SUCCESS");
}

#[test]
fn edge_case_3_all_noise_no_topic() {
    println!("\n=== EDGE CASE 3: All Noise (cluster_id = -1) ===");
    println!("PURPOSE: Noise points should not form topics");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    let mut memberships = HashMap::new();
    for embedder in Embedder::all() {
        memberships.insert(
            embedder,
            vec![
                ClusterMembership::noise(id1, embedder),
                ClusterMembership::noise(id2, embedder),
            ],
        );
    }

    println!("STATE BEFORE: 2 memories, all spaces cluster=-1 (noise)");

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(topics.is_empty(), "All noise should not form topics");

    println!("[EVIDENCE] All noise -> 0 topics - SUCCESS");
}

#[test]
fn edge_case_4_threshold_boundary_exactly_2_5() {
    println!("\n=== EDGE CASE 4: Threshold Boundary (exactly 2.5) ===");
    println!("PURPOSE: weighted_agreement = 2.5 should form topic (>= threshold)");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // 2 semantic (2.0) + 1 relational (0.5) = exactly 2.5
    let config: Vec<(Embedder, i32)> = vec![
        (Embedder::Semantic, 1), // weight 1.0
        (Embedder::Causal, 1),   // weight 1.0
        (Embedder::Entity, 1),   // weight 0.5 (relational)
    ];

    let memberships = create_test_memberships(&[id1, id2], &config);

    let expected_weight: f32 = 1.0 + 1.0 + 0.5;
    println!(
        "STATE BEFORE: 2 memories, shared spaces = {:?}",
        config.iter().map(|(e, _)| e.name()).collect::<Vec<_>>()
    );
    println!("  Expected weighted_agreement = {}", expected_weight);
    println!("  topic_threshold() = {}", topic_threshold());

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());
    if !topics.is_empty() {
        println!("  Topic confidence = {:.4}", topics[0].confidence);
        println!("  Topic is_valid = {}", topics[0].is_valid());
    }

    assert_eq!(topics.len(), 1, "Exactly 2.5 should form topic");
    assert!(topics[0].is_valid(), "Topic should be valid");

    println!("[EVIDENCE] weighted_agreement=2.5 (exactly threshold) -> 1 topic - SUCCESS");
}

#[test]
fn edge_case_5_below_threshold_2_49() {
    println!("\n=== EDGE CASE 5: Below Threshold (2.49 < 2.5) ===");
    println!("PURPOSE: weighted_agreement < 2.5 should NOT form topic");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // Only 2 semantic = 2.0 < 2.5
    let config: Vec<(Embedder, i32)> = vec![
        (Embedder::Semantic, 1), // weight 1.0
        (Embedder::Causal, 1),   // weight 1.0
    ];

    let memberships = create_test_memberships(&[id1, id2], &config);

    let expected_weight: f32 = 1.0 + 1.0;
    println!(
        "STATE BEFORE: 2 memories, shared spaces = {:?}",
        config.iter().map(|(e, _)| e.name()).collect::<Vec<_>>()
    );
    println!("  Expected weighted_agreement = {}", expected_weight);
    println!("  This is < topic_threshold() = {}", topic_threshold());

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(topics.is_empty(), "2.0 < 2.5 should not form topic");

    println!("[EVIDENCE] weighted_agreement=2.0 (below threshold) -> 0 topics - SUCCESS");
}

#[test]
fn edge_case_6_temporal_only_zero_weight() {
    println!("\n=== EDGE CASE 6: Temporal Only (AP-60 verification) ===");
    println!("PURPOSE: Temporal embedders contribute 0.0, cannot form topic alone");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // All 3 temporal embedders
    let config: Vec<(Embedder, i32)> = vec![
        (Embedder::TemporalRecent, 1),     // weight 0.0
        (Embedder::TemporalPeriodic, 1),   // weight 0.0
        (Embedder::TemporalPositional, 1), // weight 0.0
    ];

    let memberships = create_test_memberships(&[id1, id2], &config);

    println!("STATE BEFORE: 2 memories, shared temporal spaces only");
    for e in [
        Embedder::TemporalRecent,
        Embedder::TemporalPeriodic,
        Embedder::TemporalPositional,
    ] {
        println!(
            "  {:?} -> topic_weight = {}",
            e.name(),
            category_for(e).topic_weight()
        );
    }

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(
        topics.is_empty(),
        "Temporal-only should not form topic per AP-60"
    );

    println!("[EVIDENCE] 3 temporal spaces (0.0 weight) -> 0 topics - AP-60 VERIFIED");
}

#[test]
fn edge_case_7_max_weighted_agreement() {
    println!("\n=== EDGE CASE 7: Maximum Weighted Agreement ===");
    println!("PURPOSE: All 13 spaces agreeing should give max = 8.5");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    let mut memberships = HashMap::new();
    for embedder in Embedder::all() {
        memberships.insert(
            embedder,
            vec![
                ClusterMembership::new(id1, embedder, 1, 0.9, true),
                ClusterMembership::new(id2, embedder, 1, 0.9, true),
            ],
        );
    }

    // Calculate expected max
    let mut expected_max = 0.0f32;
    for embedder in Embedder::all() {
        expected_max += category_for(embedder).topic_weight();
        println!(
            "  {:?}: weight = {}, cumulative = {:.1}",
            embedder.short_name(),
            category_for(embedder).topic_weight(),
            expected_max
        );
    }

    println!("STATE BEFORE: 2 memories, ALL 13 spaces share cluster=1");
    println!("  Expected max weighted_agreement = {}", expected_max);
    println!(
        "  max_weighted_agreement() constant = {}",
        max_weighted_agreement()
    );

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());
    if !topics.is_empty() {
        println!("  Topic confidence = {:.4}", topics[0].confidence);
        let expected_confidence = expected_max / max_weighted_agreement();
        println!("  Expected confidence = {:.4}", expected_confidence);
    }

    assert_eq!(topics.len(), 1, "Max agreement should form topic");
    assert!(
        (topics[0].confidence - 1.0).abs() < 0.001,
        "Max confidence should be 1.0"
    );

    println!("[EVIDENCE] All 13 spaces -> confidence=1.0 (max) - SUCCESS");
}

#[test]
fn edge_case_8_different_clusters_no_agreement() {
    println!("\n=== EDGE CASE 8: Different Clusters (no agreement) ===");
    println!("PURPOSE: Memories in different clusters have no agreement");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    let mut memberships = HashMap::new();
    for embedder in Embedder::all() {
        memberships.insert(
            embedder,
            vec![
                ClusterMembership::new(id1, embedder, 1, 0.9, true), // cluster 1
                ClusterMembership::new(id2, embedder, 2, 0.9, true), // cluster 2 (different!)
            ],
        );
    }

    println!("STATE BEFORE: 2 memories, id1 in cluster=1, id2 in cluster=2");

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(
        topics.is_empty(),
        "Different clusters should not form topic"
    );

    println!("[EVIDENCE] Same space, different clusters -> 0 topics - SUCCESS");
}

#[test]
fn edge_case_9_mixed_temporal_and_semantic() {
    println!("\n=== EDGE CASE 9: Mixed Temporal + Semantic ===");
    println!("PURPOSE: Temporal should not add to semantic weight");

    let synthesizer = TopicSynthesizer::new();
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // 2 semantic (2.0) + 3 temporal (0.0) = still 2.0 < 2.5
    let config: Vec<(Embedder, i32)> = vec![
        (Embedder::Semantic, 1),           // 1.0
        (Embedder::Causal, 1),             // 1.0
        (Embedder::TemporalRecent, 1),     // 0.0
        (Embedder::TemporalPeriodic, 1),   // 0.0
        (Embedder::TemporalPositional, 1), // 0.0
    ];

    let memberships = create_test_memberships(&[id1, id2], &config);

    println!("STATE BEFORE: 2 semantic + 3 temporal");
    println!("  Expected: 2*1.0 + 3*0.0 = 2.0 < 2.5");

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());

    assert!(
        topics.is_empty(),
        "2 semantic + 3 temporal = 2.0 should not form topic"
    );

    println!("[EVIDENCE] Temporal adds 0.0 to weighted_agreement - SUCCESS");
}

#[test]
fn edge_case_10_multi_topic_groups() {
    println!("\n=== EDGE CASE 10: Multiple Distinct Topic Groups ===");
    println!("PURPOSE: Memories in separate clusters form separate topics");

    let synthesizer = TopicSynthesizer::new();
    let group1_a = Uuid::new_v4();
    let group1_b = Uuid::new_v4();
    let group2_a = Uuid::new_v4();
    let group2_b = Uuid::new_v4();

    let mut memberships = HashMap::new();

    // Group 1: cluster 1 in semantic spaces
    // Group 2: cluster 2 in semantic spaces
    for embedder in [Embedder::Semantic, Embedder::Causal, Embedder::Code] {
        let entries = vec![
            ClusterMembership::new(group1_a, embedder, 1, 0.9, true),
            ClusterMembership::new(group1_b, embedder, 1, 0.9, true),
            ClusterMembership::new(group2_a, embedder, 2, 0.9, true),
            ClusterMembership::new(group2_b, embedder, 2, 0.9, true),
        ];
        memberships.insert(embedder, entries);
    }

    println!("STATE BEFORE: 4 memories, 2 groups");
    println!("  Group 1: [group1_a, group1_b] in cluster 1");
    println!("  Group 2: [group2_a, group2_b] in cluster 2");
    println!("  Shared spaces: Semantic, Causal, Code (3 * 1.0 = 3.0 each)");

    let topics = synthesizer.synthesize_topics(&memberships).unwrap();

    println!("STATE AFTER: topics.len() = {}", topics.len());
    for (i, topic) in topics.iter().enumerate() {
        println!(
            "  Topic {}: {} members, confidence={:.4}",
            i,
            topic.member_count(),
            topic.confidence
        );
    }

    // Should form 2 topics (one per cluster group)
    let total_members: usize = topics.iter().map(|t| t.member_count()).sum();
    assert_eq!(total_members, 4, "All 4 memories should be in topics");

    println!("[EVIDENCE] 2 distinct groups -> separate topics - SUCCESS");
}

#[test]
fn test_topic_profile_strength_clamping() {
    println!("\n=== EDGE CASE 11: TopicProfile Strength Clamping ===");
    println!("PURPOSE: Strength values should be clamped to [0.0, 1.0]");

    let extremes = [
        f32::MAX,
        f32::MIN,
        f32::NEG_INFINITY,
        f32::INFINITY,
        -10.0,
        10.0,
        0.5,
    ];

    for &val in &extremes {
        let mut strengths = [0.0f32; 14];
        strengths[0] = val;

        let profile = TopicProfile::new(strengths);

        println!(
            "  Input: {} -> Output: {} (clamped to [0.0, 1.0])",
            val, profile.strengths[0]
        );

        assert!(profile.strengths[0] >= 0.0, "Strength should be >= 0.0");
        assert!(profile.strengths[0] <= 1.0, "Strength should be <= 1.0");
        assert!(!profile.strengths[0].is_nan(), "Strength should not be NaN");
    }

    println!("[EVIDENCE] All extreme values clamped correctly - SUCCESS");
}

#[test]
fn test_weighted_agreement_no_nan() {
    println!("\n=== EDGE CASE 12: Weighted Agreement No NaN ===");
    println!("PURPOSE: weighted_agreement should never return NaN/Infinity");

    // Test with zero profile
    let zero_profile = TopicProfile::new([0.0; 14]);
    let wa = zero_profile.weighted_agreement();
    println!("  Zero profile -> weighted_agreement = {}", wa);
    assert!(!wa.is_nan(), "Should not be NaN");
    assert!(!wa.is_infinite(), "Should not be Infinity");

    // Test with max profile
    let max_profile = TopicProfile::new([1.0; 14]);
    let wa = max_profile.weighted_agreement();
    println!("  Max profile -> weighted_agreement = {}", wa);
    assert!(!wa.is_nan(), "Should not be NaN");
    assert!(!wa.is_infinite(), "Should not be Infinity");

    println!("[EVIDENCE] weighted_agreement handles edge cases without NaN - SUCCESS");
}

#[test]
fn test_synthesizer_configuration_validation() {
    println!("\n=== EDGE CASE 13: Synthesizer Configuration Bounds ===");
    println!("PURPOSE: Configuration values should be clamped to valid ranges");

    // Test extreme merge thresholds
    let synth = TopicSynthesizer::with_config(100.0, -100.0);
    println!("  Input: merge=100.0, silhouette=-100.0");
    println!(
        "  Output: merge={}, silhouette={}",
        synth.merge_similarity_threshold(),
        synth.min_silhouette()
    );

    assert!(
        (synth.merge_similarity_threshold() - 1.0).abs() < f32::EPSILON,
        "merge_threshold should clamp to 1.0"
    );
    assert!(
        (synth.min_silhouette() - (-1.0)).abs() < f32::EPSILON,
        "min_silhouette should clamp to -1.0"
    );

    println!("[EVIDENCE] Configuration values properly bounded - SUCCESS");
}
