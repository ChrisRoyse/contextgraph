//! Cognitive Pulse Example
//!
//! Demonstrates CognitivePulse creation, action computation, and state monitoring.
//! This example shows how the cognitive pulse system works without requiring a database.
//!
//! Run with: `cargo run --package context-graph-core --example cognitive_pulse`

use context_graph_core::types::{CognitivePulse, EmotionalState, SuggestedAction};

fn main() {
    println!("=== Cognitive Pulse Examples ===\n");

    // Example 1: Crisis state (high entropy, low coherence)
    println!("--- Example 1: Crisis State ---");
    let crisis = CognitivePulse::from_values(0.85, 0.25);
    println!("Crisis State:");
    println!("  Entropy: {:.2}", crisis.entropy);
    println!("  Coherence: {:.2}", crisis.coherence);
    println!("  Suggested Action: {:?}", crisis.suggested_action);
    println!(
        "  Action Description: {}",
        crisis.suggested_action.description()
    );
    println!("  Is Healthy: {}", crisis.is_healthy());
    assert_eq!(crisis.suggested_action, SuggestedAction::Stabilize);
    println!("  ✓ Correctly identified as Stabilize action\n");

    // Example 2: Ready state (low entropy, high coherence)
    println!("--- Example 2: Ready State ---");
    let ready = CognitivePulse::from_values(0.2, 0.85);
    println!("Ready State:");
    println!("  Entropy: {:.2}", ready.entropy);
    println!("  Coherence: {:.2}", ready.coherence);
    println!("  Suggested Action: {:?}", ready.suggested_action);
    println!("  Is Healthy: {}", ready.is_healthy());
    assert_eq!(ready.suggested_action, SuggestedAction::Ready);
    println!("  ✓ Correctly identified as Ready action\n");

    // Example 3: Exploration state (high entropy, high coherence)
    println!("--- Example 3: Exploration State ---");
    let explore = CognitivePulse::from_values(0.7, 0.6);
    println!("Exploration State:");
    println!("  Entropy: {:.2}", explore.entropy);
    println!("  Coherence: {:.2}", explore.coherence);
    println!("  Suggested Action: {:?}", explore.suggested_action);
    println!(
        "  Action Description: {}",
        explore.suggested_action.description()
    );
    assert_eq!(explore.suggested_action, SuggestedAction::Explore);
    println!("  ✓ Correctly identified as Explore action\n");

    // Example 4: Consolidation state (low entropy, low coherence)
    println!("--- Example 4: Consolidation State ---");
    let consolidate = CognitivePulse::from_values(0.3, 0.3);
    println!("Consolidation State:");
    println!("  Entropy: {:.2}", consolidate.entropy);
    println!("  Coherence: {:.2}", consolidate.coherence);
    println!("  Suggested Action: {:?}", consolidate.suggested_action);
    assert_eq!(consolidate.suggested_action, SuggestedAction::Consolidate);
    println!("  ✓ Correctly identified as Consolidate action\n");

    // Example 5: Blending two pulses
    println!("--- Example 5: Blending Pulses ---");
    let blended = crisis.blend(&ready, 0.5);
    println!("Blended State (50/50 crisis + ready):");
    println!(
        "  Original Crisis: entropy={:.2}, coherence={:.2}",
        crisis.entropy, crisis.coherence
    );
    println!(
        "  Original Ready:  entropy={:.2}, coherence={:.2}",
        ready.entropy, ready.coherence
    );
    println!(
        "  Blended Result:  entropy={:.2}, coherence={:.2}",
        blended.entropy, blended.coherence
    );
    println!("  Suggested Action: {:?}", blended.suggested_action);
    // Midpoint values: (0.85+0.2)/2 = 0.525 entropy, (0.25+0.85)/2 = 0.55 coherence
    assert!((blended.entropy - 0.525).abs() < 0.01);
    assert!((blended.coherence - 0.55).abs() < 0.01);
    println!("  ✓ Blend correctly interpolated values\n");

    // Example 6: Update pulse dynamically
    println!("--- Example 6: Dynamic Update ---");
    let mut dynamic = CognitivePulse::default();
    println!("Dynamic Update:");
    println!(
        "  Initial: entropy={:.2}, coherence={:.2}, action={:?}",
        dynamic.entropy, dynamic.coherence, dynamic.suggested_action
    );
    assert_eq!(dynamic.entropy, 0.5);
    assert_eq!(dynamic.coherence, 0.5);

    dynamic.update(0.3, -0.2); // increase entropy, decrease coherence
    println!(
        "  After update(+0.3, -0.2): entropy={:.2}, coherence={:.2}, action={:?}",
        dynamic.entropy, dynamic.coherence, dynamic.suggested_action
    );
    assert_eq!(dynamic.entropy, 0.8);
    assert_eq!(dynamic.coherence, 0.3);
    println!("  Coherence delta: {:.2}", dynamic.coherence_delta);
    println!("  ✓ Update correctly modified values and recomputed action\n");

    // Example 7: Pulse with emotional state
    println!("--- Example 7: Emotional State ---");
    let focused = CognitivePulse::with_emotion(0.4, 0.7, EmotionalState::Focused, None);
    println!("Pulse with Focused emotional state:");
    println!("  Entropy: {:.2}", focused.entropy);
    println!("  Coherence: {:.2}", focused.coherence);
    println!(
        "  Emotional Weight: {:.2} (Focused = 1.3x)",
        focused.emotional_weight
    );
    println!("  Suggested Action: {:?}", focused.suggested_action);
    assert_eq!(focused.emotional_weight, 1.3);
    println!("  ✓ Emotional weight correctly derived from EmotionalState\n");

    // Example 8: All SuggestedAction variants
    println!("--- Example 8: All Action Variants ---");
    let actions = [
        SuggestedAction::Ready,
        SuggestedAction::Continue,
        SuggestedAction::Explore,
        SuggestedAction::Consolidate,
        SuggestedAction::Prune,
        SuggestedAction::Stabilize,
        SuggestedAction::Review,
    ];
    for action in actions {
        println!("  {:?}: {}", action, action.description());
    }
    println!();

    // Example 9: JSON serialization
    println!("--- Example 9: JSON Serialization ---");
    let json = serde_json::to_string_pretty(&ready).expect("Failed to serialize");
    println!("JSON Output:\n{}", json);

    // Verify round-trip
    let parsed: CognitivePulse = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(parsed.entropy, ready.entropy);
    assert_eq!(parsed.coherence, ready.coherence);
    assert_eq!(parsed.suggested_action, ready.suggested_action);
    println!("\n  ✓ JSON round-trip successful\n");

    // Example 10: Health check across states
    println!("--- Example 10: Health Check Summary ---");
    let states = [
        ("Crisis", crisis.is_healthy()),
        ("Ready", ready.is_healthy()),
        ("Explore", explore.is_healthy()),
        ("Consolidate", consolidate.is_healthy()),
    ];
    for (name, healthy) in states {
        let status = if healthy {
            "✓ Healthy"
        } else {
            "✗ Unhealthy"
        };
        println!("  {}: {}", name, status);
    }

    println!("\n=== All Examples Completed Successfully ===");
}
