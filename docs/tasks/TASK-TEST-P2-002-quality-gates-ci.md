# TASK-TEST-P2-002: Add Quality Gates to CI Pipeline

## Task Metadata

```xml
<task_spec id="TASK-TEST-P2-002" version="1.0">
<metadata>
  <title>Add Quality Gates and Performance Benchmarks to CI Pipeline</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>2</sequence>
  <implements>
    <item>PRD: Quality Gates enforcement (constitution.yaml perf.quality)</item>
    <item>PRD: Performance target validation (constitution.yaml perf.latency)</item>
    <item>Sherlock-10 GAP 3: Quality Gate Enforcement Not Automated</item>
    <item>Sherlock-10 GAP 4: Performance Targets Not Enforced in CI</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TEST-P2-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>
```

---

## Context

This task addresses a critical gap identified in Sherlock-10 analysis: quality metrics and performance benchmarks exist in the codebase but are NOT enforced in CI. This means:

1. **Quality Drift**: UTL average could drop below 0.6 undetected
2. **Coherence Regression**: Recovery could exceed 10s undetected
3. **Security Degradation**: Attack detection could fall below 95% undetected
4. **Performance Regression**: inject_context P95 could exceed 40ms, tool P99 could exceed 60ms

The existing CI pipeline (`/.github/workflows/ci.yml`) runs tests, linting, coverage, and security audits but lacks:
- Quality gate validation job
- Performance benchmark job with regression detection
- Automated failure on threshold violations

---

## Input Context Files

```xml
<input_context_files>
  <file purpose="Current CI configuration to modify">.github/workflows/ci.yml</file>
  <file purpose="Quality thresholds from constitution">docs2/constitution.yaml</file>
  <file purpose="Performance targets from PRD">docs2/contextprd.md</file>
  <file purpose="Existing UTL benchmarks">crates/context-graph-utl/benches/utl_bench.rs</file>
  <file purpose="Existing graph benchmarks">crates/context-graph-graph/benches/benchmark_suite/main.rs</file>
  <file purpose="Monitoring trait with health metrics">crates/context-graph-core/src/monitoring.rs</file>
  <file purpose="Sherlock-10 gap analysis">docs/sherlock-10-integration-tests.md</file>
</input_context_files>
```

---

## Prerequisites

```xml
<prerequisites>
  <check>TASK-TEST-P2-001 complete (chaos tests implemented)</check>
  <check>Current CI passes (check, test, coverage, docs jobs)</check>
  <check>Criterion benchmark infrastructure available in crates</check>
  <check>SystemMonitor trait has health_metrics methods</check>
</prerequisites>
```

---

## Scope

```xml
<scope>
  <in_scope>
    - Add quality-gates job to CI workflow
    - Add performance-benchmarks job to CI workflow
    - Create quality gate validation test module
    - Create benchmark baseline comparison script
    - Configure CI to fail on quality gate violations
    - Configure CI to fail on performance regressions (>5%)
  </in_scope>
  <out_of_scope>
    - Implementing actual monitoring backends (existing stubs acceptable for CI)
    - GPU-specific benchmarks (CPU-only for CI)
    - Long-running stability tests (handled in TASK-TEST-P2-003)
    - Dream cycle validation (separate task)
  </out_of_scope>
</scope>
```

---

## Quality Gate Thresholds (from constitution.yaml)

| Metric | Target | Enforcement |
|--------|--------|-------------|
| UTL average | >0.6 | CI fails if below |
| Coherence recovery | <10s | CI fails if above |
| Attack detection | >95% | CI fails if below |
| False positive rate | <2% | CI fails if above |
| Information loss | <15% | CI warns if above |
| Compression ratio | >60% | CI warns if above |

---

## Performance Thresholds (from contextprd.md and constitution.yaml)

| Metric | Target | Enforcement |
|--------|--------|-------------|
| inject_context P95 | <40ms | CI fails if above |
| Any tool P99 | <60ms | CI fails if above |
| compute_learning_magnitude | <100us | CI fails if above |
| Full UTL computation P99 | <10ms | CI fails if above |
| Surprise calculation P99 | <5ms | CI fails if above |
| Hopfield cache | <1ms | CI fails if above |
| Poincare single | <10us | CI fails if above |
| Poincare 1k batch | <100ms CPU | CI fails if above |

---

## Definition of Done

```xml
<definition_of_done>
  <signatures>
    <signature file=".github/workflows/ci.yml">
      # New job: quality-gates
      quality-gates:
        name: Quality Gates
        runs-on: ubuntu-latest
        needs: test
        steps:
          - name: Checkout repository
            uses: actions/checkout@v4
          - name: Install Rust toolchain
            uses: dtolnay/rust-action@stable
          - name: Run quality gate checks
            run: cargo test --package context-graph-utl --test quality_gates -- --nocapture
          - name: Validate quality metrics
            run: |
              cargo run --package context-graph-mcp --bin quality-validator
    </signature>

    <signature file=".github/workflows/ci.yml">
      # New job: benchmarks
      benchmarks:
        name: Performance Benchmarks
        runs-on: ubuntu-latest
        needs: test
        steps:
          - name: Run benchmarks
            run: cargo bench --workspace -- --save-baseline ci
          - name: Check for regressions
            run: |
              # Compare against main baseline
              # Fail if any benchmark regresses >5%
    </signature>

    <signature file="crates/context-graph-utl/tests/quality_gates.rs">
      //! Quality gate enforcement tests
      //!
      //! These tests verify that quality metrics meet constitution.yaml requirements:
      //! - UTL average: >0.6
      //! - Coherence recovery: <10s
      //! - Attack detection: >95%
      //! - False positive: <2%

      #[tokio::test]
      async fn test_utl_average_meets_threshold() { ... }

      #[tokio::test]
      async fn test_coherence_recovery_within_limit() { ... }

      #[tokio::test]
      async fn test_attack_detection_rate_sufficient() { ... }

      #[tokio::test]
      async fn test_false_positive_rate_acceptable() { ... }
    </signature>

    <signature file="scripts/benchmark-check.sh">
      #!/bin/bash
      # Benchmark regression check script
      # Compares current benchmarks against baseline
      # Exits with 1 if regression >5%
    </signature>
  </signatures>

  <constraints>
    - CI job must complete in <10 minutes
    - Quality gates must use actual monitoring infrastructure, not hardcoded values
    - Benchmark comparison must use criterion-compare or similar
    - All thresholds must match constitution.yaml exactly
    - No GPU requirements (CPU-only for CI runners)
    - Cache cargo registry for performance
    - Quality gates job must depend on test job passing
    - Benchmarks job may run in parallel with quality gates
  </constraints>

  <verification>
    - cargo test --package context-graph-utl --test quality_gates passes
    - cargo bench --workspace completes without error
    - CI workflow validates correctly with act (local runner)
    - Quality gate failure correctly fails CI when thresholds violated
    - Benchmark regression correctly fails CI when >5% slower
  </verification>
</definition_of_done>
```

---

## Pseudo Code

### 1. Quality Gates Test Module (`crates/context-graph-utl/tests/quality_gates.rs`)

```rust
quality_gates.rs:
  const UTL_THRESHOLD: f64 = 0.6;
  const COHERENCE_RECOVERY_LIMIT_SECS: u64 = 10;
  const ATTACK_DETECTION_THRESHOLD: f64 = 0.95;
  const FALSE_POSITIVE_LIMIT: f64 = 0.02;

  test_utl_average_meets_threshold:
    - Create UtlProcessor with defaults
    - Run sample computations across representative inputs
    - Calculate average UTL value
    - Assert average > UTL_THRESHOLD

  test_coherence_recovery_within_limit:
    - Create CoherenceTracker
    - Simulate coherence disruption
    - Measure time to recover
    - Assert recovery_time < COHERENCE_RECOVERY_LIMIT_SECS

  test_attack_detection_rate_sufficient:
    - Load adversarial test patterns (from SEC-04)
    - Run detection against all patterns
    - Calculate detection rate
    - Assert rate >= ATTACK_DETECTION_THRESHOLD

  test_false_positive_rate_acceptable:
    - Load benign test patterns
    - Run detection against all patterns
    - Calculate false positive rate
    - Assert rate < FALSE_POSITIVE_LIMIT
```

### 2. CI Workflow Updates (`.github/workflows/ci.yml`)

```yaml
ci.yml additions:

  quality-gates:
    name: Quality Gates
    runs-on: ubuntu-latest
    needs: test
    steps:
      - Checkout
      - Install Rust
      - Cache cargo
      - Run quality gate tests:
          cargo test --package context-graph-utl --test quality_gates -- --nocapture
      - If any test fails, job fails

  benchmarks:
    name: Performance Benchmarks
    runs-on: ubuntu-latest
    needs: check
    steps:
      - Checkout
      - Install Rust
      - Cache cargo
      - Download previous baseline (if exists)
      - Run benchmarks:
          cargo bench --workspace -- --save-baseline current
      - Compare baselines:
          ./scripts/benchmark-check.sh
      - Upload new baseline as artifact
      - If regression >5%, job fails
```

### 3. Benchmark Check Script (`scripts/benchmark-check.sh`)

```bash
benchmark-check.sh:

  REGRESSION_THRESHOLD=5  # percent

  parse_criterion_results:
    - Read target/criterion/**/new/estimates.json
    - Extract mean times for each benchmark

  compare_with_baseline:
    - Read baseline from artifact or file
    - For each benchmark:
        - Calculate percent change
        - If change > REGRESSION_THRESHOLD:
            - Print warning with benchmark name and change
            - Set exit_code = 1

  generate_report:
    - Output markdown summary of all benchmarks
    - Highlight regressions in red
    - Highlight improvements in green

  exit with exit_code
```

---

## Files to Create

```xml
<files_to_create>
  <file path="crates/context-graph-utl/tests/quality_gates.rs">
    Quality gate enforcement tests validating constitution.yaml thresholds
  </file>
  <file path="scripts/benchmark-check.sh">
    Bash script to compare Criterion benchmark results against baseline
    with 5% regression threshold
  </file>
  <file path="scripts/quality-validator.rs">
    Binary that runs health_metrics checks and exits with proper codes
  </file>
</files_to_create>
```

---

## Files to Modify

```xml
<files_to_modify>
  <file path=".github/workflows/ci.yml">
    Add quality-gates job (after test job)
    Add benchmarks job (after check job)
    Ensure proper job dependencies
    Add artifact caching for benchmark baselines
  </file>
  <file path="crates/context-graph-utl/Cargo.toml">
    Add dev-dependency for quality_gates test if needed
  </file>
</files_to_modify>
```

---

## Validation Criteria

```xml
<validation_criteria>
  <criterion>Quality gates job exists in CI and runs on every PR</criterion>
  <criterion>Benchmarks job exists in CI and runs on every PR</criterion>
  <criterion>UTL average threshold (0.6) is enforced - test fails if below</criterion>
  <criterion>Coherence recovery threshold (10s) is enforced - test fails if above</criterion>
  <criterion>Attack detection threshold (95%) is enforced - test fails if below</criterion>
  <criterion>False positive threshold (2%) is enforced - test fails if above</criterion>
  <criterion>Performance regression detection works with 5% threshold</criterion>
  <criterion>CI fails correctly when quality gate violated (verified with intentional failure)</criterion>
  <criterion>CI fails correctly when benchmark regresses (verified with intentional slowdown)</criterion>
  <criterion>All thresholds match constitution.yaml exactly</criterion>
  <criterion>Jobs complete in under 10 minutes each</criterion>
</validation_criteria>
```

---

## Test Commands

```xml
<test_commands>
  <command>cargo test --package context-graph-utl --test quality_gates</command>
  <command>cargo bench --package context-graph-utl</command>
  <command>cargo bench --package context-graph-graph</command>
  <command>bash scripts/benchmark-check.sh</command>
  <command>act -j quality-gates (local CI test with act)</command>
  <command>act -j benchmarks (local CI test with act)</command>
</test_commands>
```

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Benchmark noise causes false failures | Medium | Medium | Use statistical comparison, require 3+ consecutive failures |
| CI runner performance varies | Medium | Low | Use relative thresholds (regression %), not absolute |
| Quality gate tests too slow | Low | Low | Parallelize tests, use sampling for large datasets |
| Stub monitors return fixed values | Medium | High | Ensure tests use realistic mock data, not fixed returns |

---

## Success Metrics

After implementation:
- Every PR automatically validates quality gates
- Performance regressions >5% are automatically caught
- UTL drift below 0.6 is immediately flagged
- CI provides clear failure messages with specific threshold violations
- Zero quality regressions ship to main branch undetected

---

## Appendix: Threshold Reference

From `docs2/constitution.yaml` line 233:
```yaml
quality: {
  utl_avg: ">0.6",
  coherence_recovery: "<10s",
  attack_detection: ">95%",
  false_positive: "<2%",
  info_loss: "<15%",
  compression: ">60%"
}
```

From `docs2/contextprd.md` lines 788-800:
```
| inject_context P95 | <40ms |
| Any tool P99 | <60ms |
| UTL avg | >0.6 |
```

From `crates/context-graph-utl/benches/utl_bench.rs` lines 4-8:
```rust
//! Performance targets (per constitution.yaml and CUDA Report):
//! - compute_learning_magnitude: <100μs mean
//! - Full UTL computation: <10ms P99
//! - Surprise calculation: <5ms P99
//! - Emotional weight: <1ms mean
//! - Coherence tracking: <2ms mean
```

</task_spec>
