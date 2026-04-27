//! CLI Performance Benchmarks (TASK-P6-010)
//!
//! This file contains Criterion benchmarks for measuring CLI command latency
//! against constitutional performance budgets.
//!
//! # Timeout Budgets (from constitution.yaml)
//! | Hook              | Timeout | Safety Margin | CLI Budget |
//! |-------------------|---------|---------------|------------|
//! | PreToolUse        | 500ms   | 100ms         | 400ms      |
//! | UserPromptSubmit  | 2000ms  | 200ms         | 1800ms     |
//! | PostToolUse       | 3000ms  | 300ms         | 2700ms     |
//! | SessionStart      | 5000ms  | 500ms         | 4500ms     |
//! | SessionEnd        | 30000ms | 2000ms        | 28000ms    |
//!
//! # Architecture
//! - Benchmarks execute the real CLI binary (NO MOCKS)
//! - Uses tempdir for isolated database per benchmark
//! - Measures wall-clock time including process spawn overhead
//!
//! # Usage
//! ```bash
//! # Build release first (REQUIRED for accurate benchmarks)
//! cargo build --release --package context-graph-cli
//!
//! # Run benchmarks
//! cargo bench --package context-graph-cli
//!
//! # View HTML reports
//! open target/criterion/inject_brief/memories/100/report/index.html
//! ```
//!
//! # Constitution References
//! - perf.latency.inject_context: <25ms p95 (internal), <1800ms (full CLI)
//! - perf.latency.pre_tool_hook: <100ms p95 (internal), <400ms (full CLI)
//! - AP-14: No .unwrap() in library code (use .expect())

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

// =============================================================================
// BENCHMARK RUNNER
// =============================================================================

struct BenchmarkRunner {
    #[allow(dead_code)]
    temp_dir: TempDir, // Keep alive to prevent cleanup
    db_path: PathBuf,
    cli_binary: PathBuf,
    session_id: String,
}

impl BenchmarkRunner {
    fn new(prefix: &str) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_path_buf();

        // Find CLI binary (prefer release)
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let release_bin = workspace_root.join("target/release/context-graph-cli");
        let debug_bin = workspace_root.join("target/debug/context-graph-cli");

        let cli_binary = if release_bin.exists() {
            release_bin
        } else if debug_bin.exists() {
            eprintln!(
                "WARNING: Using debug build. For accurate benchmarks, run:\n  cargo build --release -p context-graph-cli"
            );
            debug_bin
        } else {
            panic!(
                "CLI binary not found. Run:\n  cargo build --release -p context-graph-cli\n\
                 Looked for:\n  - {}\n  - {}",
                release_bin.display(),
                debug_bin.display()
            );
        };

        let session_id = format!("bench-{}-{}", prefix, uuid::Uuid::new_v4());

        Self {
            temp_dir,
            db_path,
            cli_binary,
            session_id,
        }
    }

    fn setup_session(&self) -> bool {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let input = serde_json::json!({
            "hook_type": "session_start",
            "session_id": self.session_id,
            "timestamp_ms": timestamp_ms,
            "payload": {
                "type": "session_start",
                "data": { "cwd": "/tmp", "source": "benchmark" }
            }
        });

        let mut child = Command::new(&self.cli_binary)
            .args([
                "hooks",
                "session-start",
                "--session-id",
                &self.session_id,
                "--stdin",
                "--format",
                "json",
            ])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn session-start");

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(input.to_string().as_bytes())
                .expect("Write failed");
        }
        drop(child.stdin.take());

        let output = child.wait_with_output().expect("Wait failed");
        if !output.status.success() {
            eprintln!(
                "Session start failed:\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return false;
        }
        true
    }

    fn seed_memories(&self, count: usize) {
        for i in 0..count {
            let content = format!(
                "Benchmark memory {}: Implemented HDBSCAN clustering with {} nodes for topic detection",
                i,
                i * 100
            );

            let timestamp_ms = chrono::Utc::now().timestamp_millis();
            let input = serde_json::json!({
                "hook_type": "post_tool_use",
                "session_id": self.session_id,
                "timestamp_ms": timestamp_ms,
                "payload": {
                    "type": "post_tool_use",
                    "data": {
                        "tool_name": "Write",
                        "tool_input": { "file_path": format!("/src/file_{}.rs", i) },
                        "tool_response": content,
                        "tool_use_id": format!("tu-seed-{:04}", i)
                    }
                }
            });

            let mut child = Command::new(&self.cli_binary)
                .args([
                    "hooks",
                    "post-tool",
                    "--session-id",
                    &self.session_id,
                    "--tool-name",
                    "Write",
                    "--success",
                    "true",
                    "--stdin",
                    "true",
                    "--format",
                    "json",
                ])
                .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn post-tool");

            if let Some(ref mut stdin) = child.stdin {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .expect("Write failed");
            }
            drop(child.stdin.take());

            let _ = child.wait_with_output();
        }
    }

    fn run_inject_brief(&self) -> Duration {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let input = serde_json::json!({
            "hook_type": "pre_tool_use",
            "session_id": self.session_id,
            "timestamp_ms": timestamp_ms,
            "payload": {
                "type": "pre_tool_use",
                "data": {
                    "tool_name": "Read",
                    "tool_input": { "file_path": "/tmp/benchmark.txt" },
                    "tool_use_id": format!("tu-bench-{}", uuid::Uuid::new_v4())
                }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args([
                "hooks",
                "pre-tool",
                "--session-id",
                &self.session_id,
                "--tool-name",
                "Read",
                "--fast-path",
                "true",
                "--stdin",
                "true",
                "--format",
                "json",
            ])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn pre-tool");

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(input.to_string().as_bytes())
                .expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }

    fn run_inject_context(&self, query: &str) -> Duration {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let input = serde_json::json!({
            "hook_type": "user_prompt_submit",
            "session_id": self.session_id,
            "timestamp_ms": timestamp_ms,
            "payload": {
                "type": "user_prompt_submit",
                "data": { "prompt": query, "context": [] }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args([
                "hooks",
                "prompt-submit",
                "--session-id",
                &self.session_id,
                "--stdin",
                "true",
                "--format",
                "json",
            ])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn prompt-submit");

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(input.to_string().as_bytes())
                .expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }

    fn run_capture_memory(&self, content: &str) -> Duration {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let input = serde_json::json!({
            "hook_type": "post_tool_use",
            "session_id": self.session_id,
            "timestamp_ms": timestamp_ms,
            "payload": {
                "type": "post_tool_use",
                "data": {
                    "tool_name": "Edit",
                    "tool_input": { "file_path": "/tmp/capture.rs" },
                    "tool_response": content,
                    "tool_use_id": format!("tu-capture-{}", uuid::Uuid::new_v4())
                }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args([
                "hooks",
                "post-tool",
                "--session-id",
                &self.session_id,
                "--tool-name",
                "Edit",
                "--success",
                "true",
                "--stdin",
                "true",
                "--format",
                "json",
            ])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn post-tool");

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(input.to_string().as_bytes())
                .expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }

    fn run_session_start(&self) -> Duration {
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        // Use a unique session ID for each measurement to avoid conflicts
        let temp_session_id = format!("bench-start-{}", uuid::Uuid::new_v4());
        let input = serde_json::json!({
            "hook_type": "session_start",
            "session_id": temp_session_id,
            "timestamp_ms": timestamp_ms,
            "payload": {
                "type": "session_start",
                "data": { "cwd": "/tmp", "source": "benchmark" }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args([
                "hooks",
                "session-start",
                "--session-id",
                &temp_session_id,
                "--stdin",
                "--format",
                "json",
            ])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn session-start");

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(input.to_string().as_bytes())
                .expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }
}

// =============================================================================
// P95 MEASUREMENT HELPER
// =============================================================================

fn calculate_p95(times: &[u64]) -> u64 {
    if times.is_empty() {
        return 0;
    }
    let mut sorted = times.to_vec();
    sorted.sort();
    let p95_idx = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    sorted[p95_idx.min(sorted.len().saturating_sub(1))]
}

// =============================================================================
// INJECT-BRIEF BENCHMARKS
// =============================================================================

fn benchmark_inject_brief(c: &mut Criterion) {
    let mut group = c.benchmark_group("inject_brief");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(30);

    for memory_count in [0, 10, 100].iter() {
        let runner = BenchmarkRunner::new(&format!("inject-brief-{}", memory_count));
        if !runner.setup_session() {
            panic!("Failed to setup session for benchmark");
        }
        runner.seed_memories(*memory_count);

        group.bench_with_input(
            BenchmarkId::new("memories", memory_count),
            memory_count,
            |b, _| {
                b.iter(|| black_box(runner.run_inject_brief()));
            },
        );
    }

    group.finish();
}

// =============================================================================
// INJECT-CONTEXT BENCHMARKS
// =============================================================================

fn benchmark_inject_context(c: &mut Criterion) {
    let mut group = c.benchmark_group("inject_context");
    group.measurement_time(Duration::from_secs(45));
    group.sample_size(20);

    for memory_count in [0, 10, 100].iter() {
        let runner = BenchmarkRunner::new(&format!("inject-ctx-{}", memory_count));
        if !runner.setup_session() {
            panic!("Failed to setup session for benchmark");
        }
        runner.seed_memories(*memory_count);

        group.bench_with_input(
            BenchmarkId::new("memories", memory_count),
            memory_count,
            |b, _| {
                b.iter(|| black_box(runner.run_inject_context("clustering algorithm")));
            },
        );
    }

    group.finish();
}

// =============================================================================
// CAPTURE-MEMORY BENCHMARKS
// =============================================================================

fn benchmark_capture_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_memory");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);

    for memory_count in [0, 10, 100].iter() {
        let runner = BenchmarkRunner::new(&format!("capture-{}", memory_count));
        if !runner.setup_session() {
            panic!("Failed to setup session for benchmark");
        }
        runner.seed_memories(*memory_count);

        group.bench_with_input(
            BenchmarkId::new("memories", memory_count),
            memory_count,
            |b, _| {
                b.iter(|| {
                    black_box(
                        runner.run_capture_memory(
                            "Benchmark capture content for testing performance",
                        ),
                    )
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// SESSION-START BENCHMARK
// =============================================================================

fn benchmark_session_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_start");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20);

    let runner = BenchmarkRunner::new("session-start");

    group.bench_function("cold_start", |b| {
        b.iter(|| black_box(runner.run_session_start()));
    });

    group.finish();
}

// =============================================================================
// SCALABILITY BENCHMARK
// =============================================================================

fn benchmark_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(15);

    let memory_counts = vec![10, 100];
    let mut latencies: Vec<(usize, u64)> = Vec::new();

    for memory_count in &memory_counts {
        let runner = BenchmarkRunner::new(&format!("scale-{}", memory_count));
        if !runner.setup_session() {
            panic!("Failed to setup session for scalability benchmark");
        }
        runner.seed_memories(*memory_count);

        // Collect multiple samples for P95
        let mut times = Vec::new();
        for _ in 0..20 {
            let duration = runner.run_inject_context("clustering algorithm");
            times.push(duration.as_millis() as u64);
        }

        let p95 = calculate_p95(&times);
        latencies.push((*memory_count, p95));

        group.bench_with_input(
            BenchmarkId::new("inject_context", memory_count),
            memory_count,
            |b, _| {
                b.iter(|| black_box(runner.run_inject_context("test query")));
            },
        );
    }

    // Verify graceful degradation: 10x memories should be <2x latency
    if latencies.len() >= 2 {
        let ratio = latencies[1].1 as f64 / latencies[0].1.max(1) as f64;
        println!("\n=== Scalability Analysis ===");
        println!("10 memories: {}ms p95", latencies[0].1);
        println!("100 memories: {}ms p95", latencies[1].1);
        println!("Ratio (100/10): {:.2}x", ratio);

        // Warn but don't fail - Criterion will track regression over time
        if ratio >= 2.0 {
            eprintln!(
                "WARNING: Scalability concern - latency increased {:.2}x when memories 10x.\n\
                 10 memories: {}ms, 100 memories: {}ms",
                ratio, latencies[0].1, latencies[1].1
            );
        }
    }

    group.finish();
}

// =============================================================================
// COLD START VS WARM BENCHMARK
// =============================================================================

fn benchmark_cold_warm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_warm");
    group.sample_size(15);

    // Cold start: fresh database
    let runner = BenchmarkRunner::new("cold-warm");
    if !runner.setup_session() {
        panic!("Failed to setup session for cold/warm benchmark");
    }

    let cold_start = runner.run_inject_context("cold start query");

    // Warm runs
    let mut warm_times = Vec::new();
    for _ in 0..10 {
        let duration = runner.run_inject_context("warm query");
        warm_times.push(duration.as_millis() as u64);
    }
    let warm_avg = if warm_times.is_empty() {
        1
    } else {
        warm_times.iter().sum::<u64>() / warm_times.len() as u64
    };

    let ratio = cold_start.as_millis() as f64 / warm_avg.max(1) as f64;

    println!("\n=== Cold vs Warm Analysis ===");
    println!("Cold start: {}ms", cold_start.as_millis());
    println!("Warm average: {}ms", warm_avg);
    println!("Cold/Warm ratio: {:.2}x", ratio);

    // Warn but don't fail - Criterion will track regression over time
    if ratio >= 5.0 {
        eprintln!(
            "WARNING: Cold start may be too slow relative to warm.\n\
             Cold: {}ms, Warm: {}ms, Ratio: {:.2}x (limit: 5.0x)",
            cold_start.as_millis(),
            warm_avg,
            ratio
        );
    }

    group.bench_function("warm_inject_context", |b| {
        b.iter(|| black_box(runner.run_inject_context("warm query")));
    });

    group.finish();
}

// =============================================================================
// CRITERION GROUPS
// =============================================================================

criterion_group!(
    benches,
    benchmark_inject_brief,
    benchmark_inject_context,
    benchmark_capture_memory,
    benchmark_session_start,
    benchmark_scalability,
    benchmark_cold_warm_comparison
);
criterion_main!(benches);

// =============================================================================
// PERFORMANCE VALIDATION TESTS (Non-Criterion)
// =============================================================================

// Note: This module is only compiled during `cargo test`, not `cargo bench`.
// The warnings about "unused" items are false positives during bench compilation.
#[cfg(test)]
#[allow(dead_code, unused_imports)]
mod performance_tests {
    use super::BenchmarkRunner;

    // =============================================================================
    // TIMEOUT BUDGETS (Constitution-compliant)
    // =============================================================================

    /// Full CLI budget (including ~200ms spawn overhead)
    const INJECT_BRIEF_BUDGET_MS: u64 = 400;
    const INJECT_CONTEXT_BUDGET_MS: u64 = 1800;
    const CAPTURE_MEMORY_BUDGET_MS: u64 = 2700;
    const SESSION_START_BUDGET_MS: u64 = 4500;

    // =============================================================================
    // STATISTICS HELPERS (test-only)
    // =============================================================================

    fn calculate_p95(times: &[u64]) -> u64 {
        if times.is_empty() {
            return 0;
        }
        let mut sorted = times.to_vec();
        sorted.sort();
        let p95_idx = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        sorted[p95_idx.min(sorted.len().saturating_sub(1))]
    }

    fn calculate_stats(times: &[u64]) -> (u64, u64, u64, u64, u64) {
        if times.is_empty() {
            return (0, 0, 0, 0, 0);
        }
        let mut sorted = times.to_vec();
        sorted.sort();
        let sum: u64 = sorted.iter().sum();
        let avg = sum / sorted.len() as u64;
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let p95_idx = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let p95 = sorted[p95_idx.min(sorted.len().saturating_sub(1))];
        (avg, min, max, p95, sorted.len() as u64)
    }

    /// Test that inject-brief P95 is under budget
    ///
    /// Budget: 400ms (includes ~200ms process spawn overhead)
    /// Constitution: pre_tool_hook <100ms p95 (internal)
    #[test]
    fn test_inject_brief_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-brief");
        if !runner.setup_session() {
            panic!("Failed to setup session for P95 test");
        }
        runner.seed_memories(100);

        let mut times = Vec::new();
        for _ in 0..50 {
            let duration = runner.run_inject_brief();
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, count) = calculate_stats(&times);

        println!("\n=== inject-brief P95 Test ===");
        println!("Samples: {}", count);
        println!("Min: {}ms, Max: {}ms, Avg: {}ms", min, max, avg);
        println!("P95: {}ms (budget: {}ms)", p95, INJECT_BRIEF_BUDGET_MS);

        assert!(
            p95 < INJECT_BRIEF_BUDGET_MS,
            "P95 VIOLATION: inject-brief P95 {}ms exceeds budget {}ms\n\
             Stats: min={}ms, max={}ms, avg={}ms, count={}\n\
             All times: {:?}",
            p95,
            INJECT_BRIEF_BUDGET_MS,
            min,
            max,
            avg,
            count,
            times
        );
    }

    /// Test that inject-context P95 is under budget
    ///
    /// Budget: 1800ms (UserPromptSubmit hook)
    #[test]
    fn test_inject_context_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-context");
        if !runner.setup_session() {
            panic!("Failed to setup session for P95 test");
        }
        runner.seed_memories(100);

        let mut times = Vec::new();
        for _ in 0..30 {
            let duration = runner.run_inject_context("test query");
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, count) = calculate_stats(&times);

        println!("\n=== inject-context P95 Test ===");
        println!("Samples: {}", count);
        println!("Min: {}ms, Max: {}ms, Avg: {}ms", min, max, avg);
        println!("P95: {}ms (budget: {}ms)", p95, INJECT_CONTEXT_BUDGET_MS);

        assert!(
            p95 < INJECT_CONTEXT_BUDGET_MS,
            "P95 VIOLATION: inject-context P95 {}ms exceeds budget {}ms\n\
             Stats: min={}ms, max={}ms, avg={}ms, count={}\n\
             All times: {:?}",
            p95,
            INJECT_CONTEXT_BUDGET_MS,
            min,
            max,
            avg,
            count,
            times
        );
    }

    /// Test that capture-memory P95 is under budget
    ///
    /// Budget: 2700ms (PostToolUse hook)
    #[test]
    fn test_capture_memory_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-capture");
        if !runner.setup_session() {
            panic!("Failed to setup session for P95 test");
        }
        runner.seed_memories(100);

        let mut times = Vec::new();
        for _ in 0..30 {
            let duration = runner.run_capture_memory("Test capture content for benchmarking");
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, count) = calculate_stats(&times);

        println!("\n=== capture-memory P95 Test ===");
        println!("Samples: {}", count);
        println!("Min: {}ms, Max: {}ms, Avg: {}ms", min, max, avg);
        println!("P95: {}ms (budget: {}ms)", p95, CAPTURE_MEMORY_BUDGET_MS);

        assert!(
            p95 < CAPTURE_MEMORY_BUDGET_MS,
            "P95 VIOLATION: capture-memory P95 {}ms exceeds budget {}ms\n\
             Stats: min={}ms, max={}ms, avg={}ms, count={}\n\
             All times: {:?}",
            p95,
            CAPTURE_MEMORY_BUDGET_MS,
            min,
            max,
            avg,
            count,
            times
        );
    }

    /// Test that session-start P95 is under budget
    ///
    /// Budget: 4500ms (SessionStart hook)
    #[test]
    fn test_session_start_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-session-start");

        let mut times = Vec::new();
        for _ in 0..20 {
            let duration = runner.run_session_start();
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, count) = calculate_stats(&times);

        println!("\n=== session-start P95 Test ===");
        println!("Samples: {}", count);
        println!("Min: {}ms, Max: {}ms, Avg: {}ms", min, max, avg);
        println!("P95: {}ms (budget: {}ms)", p95, SESSION_START_BUDGET_MS);

        assert!(
            p95 < SESSION_START_BUDGET_MS,
            "P95 VIOLATION: session-start P95 {}ms exceeds budget {}ms\n\
             Stats: min={}ms, max={}ms, avg={}ms, count={}\n\
             All times: {:?}",
            p95,
            SESSION_START_BUDGET_MS,
            min,
            max,
            avg,
            count,
            times
        );
    }

    /// Test scalability: 10x memories should result in <2x latency increase
    #[test]
    fn test_scalability_graceful_degradation() {
        // Collect P95 for 10 memories
        let runner_10 = BenchmarkRunner::new("scale-10");
        if !runner_10.setup_session() {
            panic!("Failed to setup session for scalability test (10 memories)");
        }
        runner_10.seed_memories(10);

        let mut times_10 = Vec::new();
        for _ in 0..20 {
            let duration = runner_10.run_inject_context("scalability test");
            times_10.push(duration.as_millis() as u64);
        }
        let p95_10 = calculate_p95(&times_10);

        // Collect P95 for 100 memories
        let runner_100 = BenchmarkRunner::new("scale-100");
        if !runner_100.setup_session() {
            panic!("Failed to setup session for scalability test (100 memories)");
        }
        runner_100.seed_memories(100);

        let mut times_100 = Vec::new();
        for _ in 0..20 {
            let duration = runner_100.run_inject_context("scalability test");
            times_100.push(duration.as_millis() as u64);
        }
        let p95_100 = calculate_p95(&times_100);

        let ratio = p95_100 as f64 / p95_10.max(1) as f64;

        println!("\n=== Scalability Test ===");
        println!("10 memories P95: {}ms", p95_10);
        println!("100 memories P95: {}ms", p95_100);
        println!("Ratio: {:.2}x (limit: 2.0x)", ratio);

        assert!(
            ratio < 2.0,
            "SCALABILITY VIOLATION: Latency should not increase >2x when memories 10x.\n\
             10 memories: {}ms p95, 100 memories: {}ms p95, ratio: {:.2}x",
            p95_10,
            p95_100,
            ratio
        );
    }

    /// Test cold start vs warm performance
    #[test]
    fn test_cold_vs_warm_ratio() {
        let runner = BenchmarkRunner::new("cold-warm-test");
        if !runner.setup_session() {
            panic!("Failed to setup session for cold/warm test");
        }

        // Cold start (first run after session setup)
        let cold_start = runner.run_inject_context("cold start query");

        // Warm runs (subsequent runs with cached state)
        let mut warm_times = Vec::new();
        for _ in 0..10 {
            let duration = runner.run_inject_context("warm query");
            warm_times.push(duration.as_millis() as u64);
        }
        let warm_avg = if warm_times.is_empty() {
            1
        } else {
            warm_times.iter().sum::<u64>() / warm_times.len() as u64
        };

        let ratio = cold_start.as_millis() as f64 / warm_avg.max(1) as f64;

        println!("\n=== Cold vs Warm Test ===");
        println!("Cold start: {}ms", cold_start.as_millis());
        println!("Warm average: {}ms", warm_avg);
        println!("Ratio: {:.2}x (limit: 5.0x)", ratio);

        assert!(
            ratio < 5.0,
            "COLD START VIOLATION: Cold start should not be >5x slower than warm.\n\
             Cold: {}ms, Warm: {}ms, Ratio: {:.2}x",
            cold_start.as_millis(),
            warm_avg,
            ratio
        );
    }

    /// Edge case: Empty database (0 memories)
    #[test]
    fn test_edge_case_empty_database() {
        let runner = BenchmarkRunner::new("edge-empty");
        if !runner.setup_session() {
            panic!("Failed to setup session for empty DB test");
        }
        // No memories seeded

        let mut times = Vec::new();
        for _ in 0..10 {
            let duration = runner.run_inject_brief();
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, _) = calculate_stats(&times);

        println!("\n=== Empty Database Edge Case ===");
        println!(
            "Min: {}ms, Max: {}ms, Avg: {}ms, P95: {}ms",
            min, max, avg, p95
        );

        // Should be faster than with 100 memories
        assert!(
            p95 < INJECT_BRIEF_BUDGET_MS,
            "Empty DB should be within budget. P95: {}ms, Budget: {}ms",
            p95,
            INJECT_BRIEF_BUDGET_MS
        );
    }

    /// Edge case: Maximum memory count (stress test with 500 memories)
    #[test]
    #[ignore] // Run explicitly: cargo test --test cli_benchmarks -- --ignored
    fn test_edge_case_max_memories() {
        let runner = BenchmarkRunner::new("edge-max");
        if !runner.setup_session() {
            panic!("Failed to setup session for max memories test");
        }
        runner.seed_memories(500);

        let mut times = Vec::new();
        for _ in 0..10 {
            let duration = runner.run_inject_context("max memory test");
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, _) = calculate_stats(&times);

        println!("\n=== Max Memories (500) Edge Case ===");
        println!(
            "Min: {}ms, Max: {}ms, Avg: {}ms, P95: {}ms",
            min, max, avg, p95
        );

        // Should still be within budget
        assert!(
            p95 < INJECT_CONTEXT_BUDGET_MS,
            "500 memories should be within budget. P95: {}ms, Budget: {}ms",
            p95,
            INJECT_CONTEXT_BUDGET_MS
        );
    }
}
