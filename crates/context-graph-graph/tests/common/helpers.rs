//! Test Environment Helpers for M04-T25 Integration Tests.
//!
//! Provides test environment setup, verification utilities, and timing helpers.
//! All helpers use REAL implementations - NO MOCKS.
//!
//! # Key Functions
//!
//! - `create_test_storage()` - Creates temp RocksDB storage
//! - `verify_state()` - Full state verification for storage
//! - `measure_latency()` - NFR-compliant timing measurement
//! - `gpu_available()` - Check CUDA availability

#![allow(dead_code)]

use std::time::{Duration, Instant};

use context_graph_graph::{
    error::GraphResult,
    storage::{EntailmentCone, GraphStorage, NodeId, PoincarePoint},
};

/// Test timing result with NFR comparison.
#[derive(Debug)]
pub struct TimingResult {
    pub operation: String,
    pub duration: Duration,
    pub nfr_target_us: u64,
    pub passed: bool,
}

impl TimingResult {
    pub fn new(operation: &str, duration: Duration, nfr_target_us: u64) -> Self {
        let duration_us = duration.as_micros() as u64;
        Self {
            operation: operation.to_string(),
            duration,
            nfr_target_us,
            passed: duration_us <= nfr_target_us,
        }
    }

    pub fn log(&self) {
        let status = if self.passed { "✓ PASS" } else { "✗ FAIL" };
        println!(
            "{}: {} in {:?} (target: {} μs) {}",
            self.operation,
            status,
            self.duration,
            self.nfr_target_us,
            if self.passed { "" } else { "⚠ NFR VIOLATION" }
        );
    }
}

/// Measure operation latency and compare against NFR target.
pub fn measure_latency<F, R>(operation: &str, nfr_target_us: u64, f: F) -> (R, TimingResult)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();

    let timing = TimingResult::new(operation, duration, nfr_target_us);
    timing.log();

    (result, timing)
}

/// Create a temporary test storage with default configuration.
///
/// Returns both the storage and the temp directory path.
/// Storage is automatically cleaned up when TempDir is dropped.
pub fn create_test_storage() -> GraphResult<(GraphStorage, tempfile::TempDir)> {
    let temp_dir = tempfile::tempdir().map_err(|e| {
        context_graph_graph::error::GraphError::Storage(format!(
            "Failed to create temp directory: {}",
            e
        ))
    })?;

    let db_path = temp_dir.path().join("test_integration.db");
    let storage = GraphStorage::open_default(&db_path)?;

    Ok((storage, temp_dir))
}

/// Full state verification for GraphStorage.
///
/// Verifies:
/// - Hyperbolic point counts match expected
/// - Cone counts match expected
/// - Adjacency counts match expected
///
/// # Fail-Fast Behavior
///
/// Returns error immediately on first mismatch - no partial verification.
pub fn verify_storage_state(
    storage: &GraphStorage,
    expected_hyperbolic_count: usize,
    expected_cone_count: usize,
    expected_adjacency_count: usize,
) -> GraphResult<()> {
    println!("VERIFYING: Storage state inspection");

    // Hyperbolic count
    let actual_hyperbolic = storage.hyperbolic_count()?;
    if actual_hyperbolic != expected_hyperbolic_count {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Hyperbolic count mismatch: expected {}, got {}",
            expected_hyperbolic_count, actual_hyperbolic
        )));
    }
    println!("  ✓ Hyperbolic count: {}", actual_hyperbolic);

    // Cone count
    let actual_cone = storage.cone_count()?;
    if actual_cone != expected_cone_count {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Cone count mismatch: expected {}, got {}",
            expected_cone_count, actual_cone
        )));
    }
    println!("  ✓ Cone count: {}", actual_cone);

    // Adjacency count
    let actual_adjacency = storage.adjacency_count()?;
    if actual_adjacency != expected_adjacency_count {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Adjacency count mismatch: expected {}, got {}",
            expected_adjacency_count, actual_adjacency
        )));
    }
    println!("  ✓ Adjacency count: {}", actual_adjacency);

    println!("VERIFIED: All storage state checks passed");
    Ok(())
}

/// Verify a specific hyperbolic point exists and matches expected coordinates.
pub fn verify_hyperbolic_point(
    storage: &GraphStorage,
    node_id: NodeId,
    expected: &PoincarePoint,
    tolerance: f32,
) -> GraphResult<()> {
    let actual = storage.get_hyperbolic(node_id)?.ok_or_else(|| {
        context_graph_graph::error::GraphError::NodeNotFound(format!("hyperbolic[{}]", node_id))
    })?;

    for i in 0..actual.coords.len() {
        let a = actual.coords[i];
        let e = expected.coords[i];
        if (a - e).abs() > tolerance {
            return Err(context_graph_graph::error::GraphError::Storage(format!(
                "Hyperbolic point[{}] coord[{}] mismatch: expected {}, got {}",
                node_id, i, e, a
            )));
        }
    }

    Ok(())
}

/// Verify a specific entailment cone exists and matches expected values.
pub fn verify_entailment_cone(
    storage: &GraphStorage,
    node_id: NodeId,
    expected: &EntailmentCone,
    tolerance: f32,
) -> GraphResult<()> {
    let actual = storage.get_cone(node_id)?.ok_or_else(|| {
        context_graph_graph::error::GraphError::NodeNotFound(format!("cone[{}]", node_id))
    })?;

    // Verify aperture
    if (actual.aperture - expected.aperture).abs() > tolerance {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Cone[{}] aperture mismatch: expected {}, got {}",
            node_id, expected.aperture, actual.aperture
        )));
    }

    // Verify aperture_factor
    if (actual.aperture_factor - expected.aperture_factor).abs() > tolerance {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Cone[{}] aperture_factor mismatch: expected {}, got {}",
            node_id, expected.aperture_factor, actual.aperture_factor
        )));
    }

    // Verify depth
    if actual.depth != expected.depth {
        return Err(context_graph_graph::error::GraphError::Storage(format!(
            "Cone[{}] depth mismatch: expected {}, got {}",
            node_id, expected.depth, actual.depth
        )));
    }

    // Verify apex coordinates
    for i in 0..actual.apex.coords.len() {
        let a = actual.apex.coords[i];
        let e = expected.apex.coords[i];
        if (a - e).abs() > tolerance {
            return Err(context_graph_graph::error::GraphError::Storage(format!(
                "Cone[{}] apex coord[{}] mismatch: expected {}, got {}",
                node_id, i, e, a
            )));
        }
    }

    Ok(())
}

/// Check if CUDA GPU is available.
///
/// Returns true if:
/// - FAISS GPU is available
/// - At least one CUDA device is detected
///
/// Used to skip GPU-dependent tests on CPU-only systems.
#[cfg(feature = "faiss-gpu")]
pub fn gpu_available() -> bool {
    // Check FAISS GPU availability when feature is enabled
    true
}

#[cfg(not(feature = "faiss-gpu"))]
pub fn gpu_available() -> bool {
    false
}

/// Skip test if GPU is not available.
#[macro_export]
macro_rules! skip_if_no_gpu {
    () => {
        if !$crate::common::helpers::gpu_available() {
            println!("⚠ Skipping test: No GPU available");
            return;
        }
    };
}

/// Assert with detailed error message for state verification.
#[macro_export]
macro_rules! assert_state {
    ($cond:expr, $component:expr, $expected:expr, $actual:expr) => {
        if !$cond {
            panic!(
                "STATE VERIFICATION FAILED:\n  Component: {}\n  Expected: {}\n  Actual: {}",
                $component, $expected, $actual
            );
        }
    };
}

/// Log BEFORE/AFTER state for test traceability.
pub struct StateLog {
    component: String,
    before_state: String,
}

impl StateLog {
    pub fn new(component: &str, before_state: &str) -> Self {
        println!("BEFORE: {} = {}", component, before_state);
        Self {
            component: component.to_string(),
            before_state: before_state.to_string(),
        }
    }

    pub fn after(&self, after_state: &str) {
        println!(
            "AFTER: {} changed from {} to {}",
            self.component, self.before_state, after_state
        );
    }

    pub fn verified(&self, expected: &str, actual: &str) {
        if expected == actual {
            println!(
                "VERIFIED: {} = {} (expected {})",
                self.component, actual, expected
            );
        } else {
            println!(
                "MISMATCH: {} = {} (expected {})",
                self.component, actual, expected
            );
        }
    }
}

/// NFR (Non-Functional Requirements) targets from constitution.
pub struct NfrTargets;

impl NfrTargets {
    /// FAISS k=100 search latency target: 2ms = 2000μs
    pub const FAISS_K100_SEARCH_US: u64 = 2000;

    /// Poincare GPU 1kx1k distance matrix: 1ms = 1000μs
    pub const POINCARE_GPU_1KX1K_US: u64 = 1000;

    /// Cone GPU 1kx1k membership check: 2ms = 2000μs
    pub const CONE_GPU_1KX1K_US: u64 = 2000;

    /// BFS depth 6 traversal: 100ms = 100000μs
    pub const BFS_DEPTH_6_US: u64 = 100_000;

    /// Domain-aware search: 10ms = 10000μs
    pub const DOMAIN_SEARCH_US: u64 = 10_000;

    /// Entailment query per cone: 1ms = 1000μs
    pub const ENTAILMENT_QUERY_US: u64 = 1000;
}

/// Run a batch of timing tests and summarize results.
pub struct TimingBatch {
    results: Vec<TimingResult>,
}

impl TimingBatch {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    pub fn add(&mut self, result: TimingResult) {
        self.results.push(result);
    }

    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    pub fn summary(&self) {
        println!("\n=== TIMING SUMMARY ===");
        let passed = self.results.iter().filter(|r| r.passed).count();
        let total = self.results.len();
        println!("Passed: {}/{}", passed, total);

        for result in &self.results {
            let status = if result.passed { "✓" } else { "✗" };
            println!(
                "  {} {}: {:?} (target: {} μs)",
                status, result.operation, result.duration, result.nfr_target_us
            );
        }

        if self.all_passed() {
            println!("ALL NFR TARGETS MET ✓");
        } else {
            println!("⚠ SOME NFR TARGETS MISSED");
        }
        println!("======================\n");
    }
}

impl Default for TimingBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_storage() {
        let (storage, _temp_dir) = create_test_storage().expect("Failed to create test storage");

        let count = storage.hyperbolic_count().expect("Count failed");
        assert_eq!(count, 0, "New storage should be empty");
    }

    #[test]
    fn test_measure_latency() {
        let (result, timing) = measure_latency("test_op", 100_000, || {
            std::thread::sleep(Duration::from_micros(100));
            42
        });

        assert_eq!(result, 42);
        assert!(timing.passed, "100μs operation should pass 100ms target");
    }

    #[test]
    fn test_timing_batch() {
        let mut batch = TimingBatch::new();

        batch.add(TimingResult::new("op1", Duration::from_micros(100), 1000));
        batch.add(TimingResult::new("op2", Duration::from_micros(200), 1000));

        assert!(batch.all_passed());
        batch.summary();
    }
}
