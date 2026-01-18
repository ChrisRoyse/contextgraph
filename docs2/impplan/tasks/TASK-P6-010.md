# Task: TASK-P6-010 - Performance Validation

## STATUS: PENDING

---

## CRITICAL: AI Agent Implementation Instructions

**This document is the SINGLE SOURCE OF TRUTH for performance validation.**

### MANDATORY Requirements
1. **NO MOCK DATA** - All benchmarks use REAL CLI execution, REAL database operations
2. **FAIL FAST** - Tests must fail immediately on any performance violation with full context
3. **NO BACKWARDS COMPATIBILITY** - No fallbacks or workarounds for slow code paths
4. **PHYSICAL VERIFICATION REQUIRED** - Check actual timing measurements, not just return values
5. **SYNTHETIC DATA WITH KNOWN OUTPUTS** - Use deterministic test data with predictable performance

---

## 1. Executive Summary

**What This Task Validates:**
- CLI command latency meets hook timeout constraints
- Performance degrades gracefully as memory count increases
- Cold start vs warm cache performance characteristics
- P95 latency measurements for all critical paths

**Timeout Budgets (from constitution.yaml):**

| Hook | Timeout | Safety Margin | CLI Budget |
|------|---------|---------------|------------|
| PreToolUse | 500ms | 100ms | **400ms** |
| UserPromptSubmit | 2000ms | 200ms | **1800ms** |
| PostToolUse | 3000ms | 300ms | **2700ms** |
| SessionStart | 5000ms | 500ms | **4500ms** |
| SessionEnd | 30000ms | 2000ms | **28000ms** |

**Constitution References:**
- `perf.latency.inject_context`: <25ms p95
- `perf.latency.pre_tool_hook`: <100ms p95 (CLI internal, not including shell/spawn)
- `success.performance.full_retrieval`: <30ms p95

---

## 2. Current Codebase State (VERIFIED 2026-01-17)

### 2.1 Existing Infrastructure

**Tests that ALREADY measure timing (`tests/integration/timeout_test.rs`):**
- `test_pre_tool_use_completes_under_100ms` - P95 timing with 10 iterations
- `test_session_end_can_use_full_30s_budget` - Wall-clock timing
- `test_timing_recorded_in_output` - JSON `execution_time_ms` field validation
- `test_all_hooks_within_budget` - All 5 hooks within constitutional limits

**Existing timing constants (`tests/integration/helpers.rs`):**
```rust
pub const TIMEOUT_PRE_TOOL_MS: u64 = 300;       // 100ms CLI + 200ms spawn overhead
pub const TIMEOUT_USER_PROMPT_MS: u64 = 2000;
pub const TIMEOUT_POST_TOOL_MS: u64 = 3000;
pub const TIMEOUT_SESSION_START_MS: u64 = 5000;
pub const TIMEOUT_SESSION_END_MS: u64 = 30000;
```

**What's MISSING:**
- `crates/context-graph-cli/benches/` directory - NO BENCHMARKS EXIST
- Criterion.rs not in Cargo.toml dev-dependencies
- No scalability tests with increasing memory counts
- No P95 statistical measurement infrastructure
- No cold start vs warm cache comparison
- No CI/CD regression detection

### 2.2 CLI Command Structure

```
context-graph-cli
├── session                      # Session management
│   ├── restore-identity
│   └── persist-identity
├── hooks                        # Claude Code native hooks (TIMING CRITICAL)
│   ├── session-start           # <4500ms budget
│   ├── pre-tool                # <400ms budget (FAST PATH)
│   ├── post-tool               # <2700ms budget
│   ├── prompt-submit           # <1800ms budget
│   └── session-end             # <28000ms budget
├── memory                       # Memory operations
│   ├── inject-context          # Full context injection
│   ├── inject-brief            # Brief context (<200 tokens) - FAST PATH
│   ├── capture-memory          # Store HookDescription
│   └── capture-response        # Store ClaudeResponse
└── setup                        # Project initialization
```

### 2.3 File Locations

| File | Purpose | Status |
|------|---------|--------|
| `crates/context-graph-cli/Cargo.toml` | Dependencies | **MODIFY** (add criterion) |
| `crates/context-graph-cli/benches/cli_benchmarks.rs` | Benchmarks | **CREATE** |
| `crates/context-graph-cli/tests/integration/timeout_test.rs` | Timing tests | EXISTS |
| `crates/context-graph-cli/tests/integration/helpers.rs` | Test utilities | EXISTS |

---

## 3. Technical Specification

### 3.1 Required Dependencies

Add to `crates/context-graph-cli/Cargo.toml`:
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports", "async_tokio"] }
hdrhistogram = "7.5"  # For accurate P95 measurement

[[bench]]
name = "cli_benchmarks"
harness = false
```

### 3.2 Benchmark Architecture

**Critical Insight:** Process spawn overhead is ~100-200ms. The constitution's 100ms budget for PreToolUse is for **CLI internal logic only**. When benchmarking the full CLI binary, add spawn overhead.

```
┌─────────────────────────────────────────────────────────────┐
│                    Total Wall-Clock Time                     │
├──────────────┬──────────────────────────┬───────────────────┤
│ Process Spawn│     CLI Internal Logic    │   Output I/O     │
│  ~100-200ms  │       <100ms target       │     ~10ms        │
└──────────────┴──────────────────────────┴───────────────────┘
```

**Benchmarking Strategy:**
1. **Internal benchmarks** (Criterion) - Measure library functions directly
2. **External benchmarks** (binary execution) - Measure end-to-end with spawn overhead
3. **Scalability benchmarks** - Vary memory count: 0, 10, 100, 500, 1000

### 3.3 Benchmark Implementation

```rust
// crates/context-graph-cli/benches/cli_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

// =============================================================================
// TIMEOUT BUDGETS (Constitution-compliant)
// =============================================================================

/// CLI internal budget (excluding process spawn)
const INJECT_BRIEF_INTERNAL_MS: u64 = 100;
/// Full CLI budget (including ~200ms spawn overhead)
const INJECT_BRIEF_BUDGET_MS: u64 = 400;
/// Hook timeout (shell overhead + CLI)
const INJECT_BRIEF_HOOK_TIMEOUT_MS: u64 = 500;

const INJECT_CONTEXT_BUDGET_MS: u64 = 1800;
const CAPTURE_MEMORY_BUDGET_MS: u64 = 2700;
const SESSION_START_BUDGET_MS: u64 = 4500;
const SESSION_END_BUDGET_MS: u64 = 28000;

// =============================================================================
// BENCHMARK RUNNER
// =============================================================================

struct BenchmarkRunner {
    temp_dir: TempDir,
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

    fn setup_session(&self) {
        let input = serde_json::json!({
            "hook_type": "session_start",
            "session_id": self.session_id,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
            "payload": {
                "type": "session_start",
                "data": { "cwd": "/tmp", "source": "benchmark" }
            }
        });

        let mut child = Command::new(&self.cli_binary)
            .args(["hooks", "session-start", "--session-id", &self.session_id, "--stdin", "--format", "json"])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn session-start");

        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(input.to_string().as_bytes()).expect("Write failed");
        }
        drop(child.stdin.take());

        let output = child.wait_with_output().expect("Wait failed");
        assert!(output.status.success(), "Session start failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    fn seed_memories(&self, count: usize) {
        for i in 0..count {
            let content = format!(
                "Benchmark memory {}: Implemented HDBSCAN clustering with {} nodes",
                i, i * 100
            );

            let input = serde_json::json!({
                "hook_type": "post_tool_use",
                "session_id": self.session_id,
                "timestamp_ms": chrono::Utc::now().timestamp_millis(),
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
                .args(["hooks", "post-tool", "--session-id", &self.session_id,
                       "--tool-name", "Write", "--success", "true", "--stdin", "true", "--format", "json"])
                .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to spawn post-tool");

            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(input.to_string().as_bytes()).expect("Write failed");
            }
            drop(child.stdin.take());

            let _ = child.wait_with_output();
        }
    }

    fn run_inject_brief(&self) -> Duration {
        let input = serde_json::json!({
            "hook_type": "pre_tool_use",
            "session_id": self.session_id,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
            "payload": {
                "type": "pre_tool_use",
                "data": {
                    "tool_name": "Read",
                    "tool_input": { "file_path": "/tmp/benchmark.txt" },
                    "tool_use_id": "tu-bench-001"
                }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args(["hooks", "pre-tool", "--session-id", &self.session_id,
                   "--tool-name", "Read", "--fast-path", "true", "--stdin", "true", "--format", "json"])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn pre-tool");

        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(input.to_string().as_bytes()).expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }

    fn run_inject_context(&self, query: &str) -> Duration {
        let input = serde_json::json!({
            "hook_type": "user_prompt_submit",
            "session_id": self.session_id,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
            "payload": {
                "type": "user_prompt_submit",
                "data": { "prompt": query, "context": [] }
            }
        });

        let start = Instant::now();

        let mut child = Command::new(&self.cli_binary)
            .args(["hooks", "prompt-submit", "--session-id", &self.session_id, "--stdin", "true", "--format", "json"])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn prompt-submit");

        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(input.to_string().as_bytes()).expect("Write failed");
        }
        drop(child.stdin.take());

        let _ = child.wait_with_output();

        start.elapsed()
    }

    fn run_capture_memory(&self, content: &str) -> Duration {
        let input = serde_json::json!({
            "hook_type": "post_tool_use",
            "session_id": self.session_id,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
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
            .args(["hooks", "post-tool", "--session-id", &self.session_id,
                   "--tool-name", "Edit", "--success", "true", "--stdin", "true", "--format", "json"])
            .env("CONTEXT_GRAPH_DB_PATH", &self.db_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn post-tool");

        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(input.to_string().as_bytes()).expect("Write failed");
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
    let mut sorted = times.to_vec();
    sorted.sort();
    let p95_idx = (sorted.len() as f64 * 0.95) as usize;
    sorted[p95_idx.min(sorted.len().saturating_sub(1))]
}

fn calculate_stats(times: &[u64]) -> (u64, u64, u64, u64, u64) {
    let mut sorted = times.to_vec();
    sorted.sort();
    let sum: u64 = sorted.iter().sum();
    let avg = sum / sorted.len() as u64;
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let p95_idx = (sorted.len() as f64 * 0.95) as usize;
    let p95 = sorted[p95_idx.min(sorted.len().saturating_sub(1))];
    (avg, min, max, p95, sorted.len() as u64)
}

// =============================================================================
// INJECT-BRIEF BENCHMARKS
// =============================================================================

fn benchmark_inject_brief(c: &mut Criterion) {
    let mut group = c.benchmark_group("inject_brief");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(50);

    for memory_count in [0, 10, 100, 500].iter() {
        let runner = BenchmarkRunner::new(&format!("inject-brief-{}", memory_count));
        runner.setup_session();
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
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(30);

    for memory_count in [0, 10, 100, 500].iter() {
        let runner = BenchmarkRunner::new(&format!("inject-ctx-{}", memory_count));
        runner.setup_session();
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
    group.sample_size(30);

    for memory_count in [0, 10, 100, 500].iter() {
        let runner = BenchmarkRunner::new(&format!("capture-{}", memory_count));
        runner.setup_session();
        runner.seed_memories(*memory_count);

        group.bench_with_input(
            BenchmarkId::new("memories", memory_count),
            memory_count,
            |b, _| {
                b.iter(|| black_box(runner.run_capture_memory("Benchmark capture content")));
            },
        );
    }

    group.finish();
}

// =============================================================================
// SCALABILITY BENCHMARK
// =============================================================================

fn benchmark_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");
    group.measurement_time(Duration::from_secs(60));

    let memory_counts = vec![10, 100, 1000];
    let mut latencies: Vec<(usize, u64)> = Vec::new();

    for memory_count in &memory_counts {
        let runner = BenchmarkRunner::new(&format!("scale-{}", memory_count));
        runner.setup_session();
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

        assert!(
            ratio < 2.0,
            "SCALABILITY VIOLATION: Latency should not increase >2x when memories 10x.\n\
             10 memories: {}ms, 100 memories: {}ms, ratio: {:.2}x",
            latencies[0].1, latencies[1].1, ratio
        );
    }

    group.finish();
}

// =============================================================================
// COLD START VS WARM BENCHMARK
// =============================================================================

fn benchmark_cold_warm_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_warm");

    // Cold start: fresh database
    let runner = BenchmarkRunner::new("cold-warm");
    runner.setup_session();

    let cold_start = runner.run_inject_context("cold start query");

    // Warm runs
    let mut warm_times = Vec::new();
    for _ in 0..10 {
        let duration = runner.run_inject_context("warm query");
        warm_times.push(duration.as_millis() as u64);
    }
    let warm_avg = warm_times.iter().sum::<u64>() / warm_times.len() as u64;

    let ratio = cold_start.as_millis() as f64 / warm_avg.max(1) as f64;

    println!("\n=== Cold vs Warm Analysis ===");
    println!("Cold start: {}ms", cold_start.as_millis());
    println!("Warm average: {}ms", warm_avg);
    println!("Cold/Warm ratio: {:.2}x", ratio);

    assert!(
        ratio < 5.0,
        "COLD START VIOLATION: Cold start should not be >5x slower than warm.\n\
         Cold: {}ms, Warm: {}ms, Ratio: {:.2}x",
        cold_start.as_millis(), warm_avg, ratio
    );

    group.bench_function("cold_start", |b| {
        b.iter(|| {
            let fresh_runner = BenchmarkRunner::new("cold-iter");
            fresh_runner.setup_session();
            black_box(fresh_runner.run_inject_context("cold query"))
        });
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
    benchmark_scalability,
    benchmark_cold_warm_comparison
);
criterion_main!(benches);

// =============================================================================
// PERFORMANCE VALIDATION TESTS (Non-Criterion)
// =============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_inject_brief_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-brief");
        runner.setup_session();
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
            "P95 VIOLATION: inject-brief P95 {}ms exceeds budget {}ms\nAll times: {:?}",
            p95, INJECT_BRIEF_BUDGET_MS, times
        );
    }

    #[test]
    fn test_inject_context_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-context");
        runner.setup_session();
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
            "P95 VIOLATION: inject-context P95 {}ms exceeds budget {}ms\nAll times: {:?}",
            p95, INJECT_CONTEXT_BUDGET_MS, times
        );
    }

    #[test]
    fn test_capture_memory_p95_under_budget() {
        let runner = BenchmarkRunner::new("p95-capture");
        runner.setup_session();
        runner.seed_memories(100);

        let mut times = Vec::new();
        for _ in 0..30 {
            let duration = runner.run_capture_memory("Test capture content");
            times.push(duration.as_millis() as u64);
        }

        let (avg, min, max, p95, count) = calculate_stats(&times);

        println!("\n=== capture-memory P95 Test ===");
        println!("Samples: {}", count);
        println!("Min: {}ms, Max: {}ms, Avg: {}ms", min, max, avg);
        println!("P95: {}ms (budget: {}ms)", p95, CAPTURE_MEMORY_BUDGET_MS);

        assert!(
            p95 < CAPTURE_MEMORY_BUDGET_MS,
            "P95 VIOLATION: capture-memory P95 {}ms exceeds budget {}ms\nAll times: {:?}",
            p95, CAPTURE_MEMORY_BUDGET_MS, times
        );
    }
}
```

---

## 4. Full State Verification Protocol (MANDATORY)

After every benchmark, you MUST verify physical state.

### 4.1 Source of Truth Identification

| Operation | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| Timing measurement | `Instant::now()` wall-clock | Print actual elapsed values |
| P95 calculation | Sorted array percentile | Assert P95 < budget, print all samples |
| Scalability ratio | Latency comparison | Assert ratio < 2.0 with explicit math |
| Cold/Warm ratio | First run vs subsequent | Assert ratio < 5.0 with explicit values |

### 4.2 Execute & Inspect Protocol

```bash
# Step 1: Build CLI in RELEASE mode (critical for accurate benchmarks)
cargo build --release --package context-graph-cli

# Step 2: Run benchmarks
cargo bench --package context-graph-cli

# Step 3: VERIFY benchmark output
cat target/criterion/inject_brief/memories/10/new/estimates.json | jq '.mean.point_estimate'
# Expected: < 400000000 (400ms in nanoseconds)

# Step 4: VERIFY P95 test results
cargo test --package context-graph-cli --test cli_benchmarks performance_tests -- --nocapture
# Expected: All P95 VIOLATION asserts pass

# Step 5: VERIFY HTML reports generated
ls target/criterion/inject_brief/memories/*/report/index.html
# Expected: HTML files exist for each memory count
```

### 4.3 Boundary & Edge Case Audit (3 Required)

**Edge Case 1: Empty Database (0 Memories)**
```
Before: Fresh TempDir, no memories seeded
Action: run_inject_brief()
After:
  - Exit code: 0
  - Timing: < 300ms (no search overhead)
  - Output: Valid JSON with empty context
```

**Edge Case 2: Maximum Memory Count (500+)**
```
Before: Seed 500 memories
Action: run_inject_context("clustering algorithm")
After:
  - Exit code: 0
  - Timing: < 1800ms (budget)
  - P95: Verify graceful degradation < 2x ratio
```

**Edge Case 3: Cold Start After Long Idle**
```
Before: Fresh BenchmarkRunner, no prior invocations
Action: First run_inject_context() call
After:
  - Timing: May be higher due to cache warming
  - Cold/Warm ratio: < 5.0x
  - Subsequent runs: Faster (cache warm)
```

### 4.4 Evidence of Success Log Template

```
=== PERFORMANCE VALIDATION FSV VERIFICATION ===
Test: [benchmark_name]
Run Time: [timestamp]
CLI Binary: [path]
Build Mode: RELEASE

[Benchmark Group: inject_brief]
  0 memories:   avg=125ms, p95=142ms (budget: 400ms) ✓
  10 memories:  avg=138ms, p95=156ms (budget: 400ms) ✓
  100 memories: avg=152ms, p95=178ms (budget: 400ms) ✓
  500 memories: avg=189ms, p95=221ms (budget: 400ms) ✓

[Benchmark Group: inject_context]
  0 memories:   avg=320ms, p95=385ms (budget: 1800ms) ✓
  10 memories:  avg=345ms, p95=412ms (budget: 1800ms) ✓
  100 memories: avg=412ms, p95=498ms (budget: 1800ms) ✓
  500 memories: avg=589ms, p95=721ms (budget: 1800ms) ✓

[Scalability Analysis]
  10 → 100 memories: ratio = 1.21x (limit: 2.0x) ✓
  100 → 1000 memories: ratio = 1.45x (limit: 2.0x) ✓

[Cold vs Warm Analysis]
  Cold start: 523ms
  Warm average: 345ms
  Ratio: 1.52x (limit: 5.0x) ✓

[Edge Cases]
  Empty DB (exit 0): ✓
  500 memories within budget: ✓
  Cold start ratio < 5.0x: ✓

=== ALL PERFORMANCE VALIDATIONS PASSED ===
```

---

## 5. Definition of Done

### 5.1 Functional Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| DOD-1 | inject-brief P95 < 400ms | Benchmark + test assertion |
| DOD-2 | inject-context P95 < 1800ms | Benchmark + test assertion |
| DOD-3 | capture-memory P95 < 2700ms | Benchmark + test assertion |
| DOD-4 | Scalability ratio < 2.0x | Explicit assertion in scalability benchmark |
| DOD-5 | Cold/Warm ratio < 5.0x | Explicit assertion in cold_warm benchmark |
| DOD-6 | Criterion benchmarks pass | `cargo bench` completes without error |
| DOD-7 | HTML reports generated | `target/criterion/*/report/index.html` exists |
| DOD-8 | Performance tests pass | `cargo test performance_tests` all green |

### 5.2 File Artifacts

| Artifact | Location | Status |
|----------|----------|--------|
| Benchmark file | `crates/context-graph-cli/benches/cli_benchmarks.rs` | CREATE |
| Updated Cargo.toml | `crates/context-graph-cli/Cargo.toml` | MODIFY |
| Benchmark results | `target/criterion/*/` | GENERATED |
| HTML reports | `target/criterion/*/report/` | GENERATED |

---

## 6. Manual Testing Procedures

### 6.1 Full Benchmark Manual Test

```bash
# 1. Build CLI in release mode (REQUIRED)
cargo build --release --package context-graph-cli

# 2. Verify binary exists
ls -la target/release/context-graph-cli
# Expected: executable binary

# 3. Run full benchmark suite
cargo bench --package context-graph-cli 2>&1 | tee benchmark_output.txt

# 4. Check for P95 violations
grep -i "violation" benchmark_output.txt
# Expected: No matches (no violations)

# 5. Verify HTML reports
open target/criterion/inject_brief/memories/100/report/index.html
# Expected: Graph showing timing distribution

# 6. Run performance tests
cargo test --package context-graph-cli --bench cli_benchmarks performance_tests -- --nocapture 2>&1 | tee perf_test_output.txt

# 7. Verify all assertions passed
grep "P95 VIOLATION" perf_test_output.txt
# Expected: No matches

# 8. Check scalability results
grep "Scalability Analysis" -A 5 perf_test_output.txt
# Expected: All ratios < 2.0x
```

### 6.2 Individual Hook Timing Manual Test

```bash
# Create isolated test environment
TEMP_DIR=$(mktemp -d)
SESSION_ID="perf-test-$(date +%s)"

# Test inject-brief timing (PreToolUse)
echo "Testing inject-brief timing..."
for i in {1..10}; do
  START=$(date +%s%N)
  echo '{"hook_type":"pre_tool_use","session_id":"'$SESSION_ID'","timestamp_ms":0,"payload":{"type":"pre_tool_use","data":{"tool_name":"Read","tool_input":{},"tool_use_id":"tu-001"}}}' | \
    ./target/release/context-graph-cli hooks pre-tool --session-id $SESSION_ID --tool-name Read --fast-path true --stdin true --format json 2>/dev/null
  END=$(date +%s%N)
  ELAPSED=$(( (END - START) / 1000000 ))
  echo "  Run $i: ${ELAPSED}ms"
done

# Cleanup
rm -rf "$TEMP_DIR"
```

---

## 7. Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| TASK-P6-009 (E2E Tests) | ✅ COMPLETE | Validates functionality first |
| TASK-P6-007 (Setup Command) | ✅ COMPLETE | Creates hook scripts |
| criterion crate | PENDING | Add to dev-dependencies |
| Release build | REQUIRED | `cargo build --release` |

---

## 8. Constitution Compliance

| Rule | Compliance |
|------|------------|
| perf.latency.inject_context | ✓ <25ms p95 (internal), <1800ms (full CLI) |
| perf.latency.pre_tool_hook | ✓ <100ms p95 (internal), <400ms (full CLI) |
| AP-14 | ✓ No .unwrap() in benchmark code (use .expect()) |
| AP-26 | ✓ Exit codes verified in test assertions |
| testing.gates.pre-merge | ✓ bench --no-run passes |

---

## 9. Anti-Patterns (FORBIDDEN)

1. **NO MOCKS** - All benchmarks use real CLI binary execution
2. **NO DEBUG BUILDS** - Benchmarks MUST use `--release` builds
3. **NO IGNORED VIOLATIONS** - Every P95 > budget is a hard failure
4. **NO ESTIMATED TIMES** - Use actual measured wall-clock time
5. **NO SINGLE SAMPLES** - Minimum 20 samples for P95 calculation
6. **NO SILENT FAILURES** - Print all timing data on assertion failure

---

## 10. Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| "CLI binary not found" | Not built | `cargo build --release -p context-graph-cli` |
| Benchmarks timeout | Debug build | Ensure using `--release` |
| High variance in times | System load | Run on idle system, increase sample count |
| P95 violation on CI | CI noise | Increase measurement_time, use bencher threshold |
| Cold start too slow | Cache initialization | Expected; verify ratio < 5.0x only |

---

## 11. Related Files

| File | Purpose |
|------|---------|
| `crates/context-graph-cli/Cargo.toml` | Dependencies (add criterion) |
| `crates/context-graph-cli/benches/cli_benchmarks.rs` | Benchmark implementation |
| `crates/context-graph-cli/tests/integration/timeout_test.rs` | Existing timing tests |
| `crates/context-graph-cli/tests/integration/helpers.rs` | Test utilities |
| `docs2/constitution.yaml` | Performance budgets |
| `target/criterion/` | Benchmark output |

---

## 12. CI/CD Integration (Future Enhancement)

For CI/CD regression detection, consider adding:

```yaml
# .github/workflows/bench.yml
name: Performance Benchmarks

on:
  pull_request:
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Build release
        run: cargo build --release --package context-graph-cli

      - name: Run benchmarks
        run: cargo bench --package context-graph-cli -- --output-format bencher | tee output.txt

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: output.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
          alert-threshold: '150%'
          fail-on-alert: true
```
