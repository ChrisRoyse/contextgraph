//! Performance Tests
//!
//! Tests for UTL computation latency requirements

use context_graph_utl::processor::UtlProcessor;
use std::time::Instant;

use super::helpers::{generate_context, generate_embedding};

// =============================================================================
// PERFORMANCE TESTS
// =============================================================================

#[test]
fn test_performance_under_10ms() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Performance test content for UTL processing with various words.";
    let embedding = generate_embedding(1536, 42);
    let context = generate_context(50, 1536, 100);

    let start = Instant::now();
    let signal = processor.compute_learning(content, &embedding, &context);
    let elapsed = start.elapsed();

    assert!(signal.is_ok(), "computation must succeed");
    assert!(
        elapsed.as_millis() < 10,
        "Computation took {}ms, expected < 10ms",
        elapsed.as_millis()
    );

    // Also verify latency is tracked in signal
    let signal = signal.unwrap();
    assert!(
        signal.latency_us < 10_000,
        "Reported latency {}us exceeds 10ms",
        signal.latency_us
    );
}

#[test]
fn test_performance_large_context() {
    let mut processor = UtlProcessor::with_defaults();

    let content = "Large context performance test";
    let embedding = generate_embedding(1536, 42);
    let context = generate_context(100, 1536, 100);

    let start = Instant::now();
    let signal = processor.compute_learning(content, &embedding, &context);
    let elapsed = start.elapsed();

    assert!(signal.is_ok());
    // Allow more time for large context, but should still be reasonable
    assert!(
        elapsed.as_millis() < 50,
        "Large context took {}ms, expected < 50ms",
        elapsed.as_millis()
    );
}
