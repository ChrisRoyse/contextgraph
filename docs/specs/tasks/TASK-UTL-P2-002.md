# TASK-UTL-P2-002: Add ClusterFit Benchmark with Performance Regression Detection

```xml
<task_spec id="TASK-UTL-P2-002" version="2.0">
<metadata>
  <title>Add ClusterFit Benchmark with Performance Regression Detection</title>
  <status>COMPLETED-WITH-FINDINGS</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <priority>P2</priority>
  <implements>
    <requirement_ref>SPEC-UTL-002</requirement_ref>
    <requirement_ref>UTL-002</requirement_ref>
  </implements>
  <depends_on>TASK-UTL-P2-001</depends_on>
  <estimated_complexity>low</estimated_complexity>
  <verified_by>SHERLOCK-HOLMES-FORENSIC-AUDIT-2026-01-12</verified_by>
</metadata>

<context>
The ClusterFit module computes silhouette coefficients for the UTL coherence formula:

  delta_C = 0.4 * Connectivity + 0.4 * ClusterFit + 0.2 * Consistency

ClusterFit is called frequently during context graph operations, making performance critical.

Current benchmark status: IMPLEMENTED and FUNCTIONING
Current performance: ~3.87ms mean latency (1000 embeddings, 1536 dimensions)
Target performance: < 2ms p95 latency

**FINDING: Performance target NOT MET - Current implementation is 1.94x slower than target**
</context>

<constitution_rules>
  <rule id="UTL-002">delta_C = 0.4*Connectivity + 0.4*ClusterFit + 0.2*Consistency</rule>
  <rule id="AP-10">No NaN/Infinity in outputs (verified implicitly during benchmark)</rule>
</constitution_rules>

<current_implementation_status>
  <file path="crates/context-graph-utl/benches/cluster_fit_bench.rs" status="VERIFIED">
    - Benchmark file exists with 6 benchmark functions
    - Uses criterion with html_reports feature
    - Deterministic data generation via generate_embedding/generate_cluster
    - All benchmark groups implemented: target, scaling, comparison, batch
  </file>
  <file path="crates/context-graph-utl/Cargo.toml" status="VERIFIED">
    - criterion = { version = "0.5", features = ["html_reports"] } in dev-dependencies
    - [[bench]] name = "cluster_fit_bench" harness = false at lines 47-49
  </file>
</current_implementation_status>

<state_verification>
  <source_of_truth>
    <location>crates/context-graph-utl/benches/cluster_fit_bench.rs</location>
    <expected>Benchmark compiles and runs, measures compute_cluster_fit performance</expected>
    <verified>YES - Benchmark executes successfully</verified>
    <evidence>
      cargo bench output:
      cluster_fit_1000_embeddings_1536d
        time:   [3.8460 ms 3.8724 ms 3.9018 ms]
    </evidence>
  </source_of_truth>

  <execute_and_inspect>
    <command>cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_fit_1000 -- --noplot</command>
    <result>
      Benchmark executes successfully.
      Mean latency: 3.87ms
      Target: < 2ms
      Verdict: TARGET NOT MET (1.94x over target)
    </result>
    <timestamp>2026-01-12</timestamp>
  </execute_and_inspect>

  <edge_cases_tested>
    <case name="build_benchmarks">cargo build -p context-graph-utl --benches - PASSES</case>
    <case name="run_benchmarks">cargo bench -p context-graph-utl --bench cluster_fit_bench - PASSES</case>
    <case name="scaling_cluster">cluster_fit_cluster_scaling group - 10/50/100/500/1000 sizes</case>
    <case name="scaling_dimension">cluster_fit_dimension_scaling group - 384/768/1536/3072 dims</case>
    <case name="metric_comparison">cluster_fit_distance_metrics - Cosine/Euclidean/Manhattan</case>
    <case name="sampling">cluster_fit_sampling - Tests max_sample_size impact</case>
  </edge_cases_tested>

  <evidence_of_success>
    <metric name="benchmark_compiles">cargo build --benches - SUCCESS</metric>
    <metric name="benchmark_runs">cargo bench cluster_fit_1000 - SUCCESS</metric>
    <metric name="performance_measured">3.87ms mean latency captured</metric>
    <metric name="criterion_reports">HTML reports generated in target/criterion/</metric>
  </evidence_of_success>

  <performance_gap_analysis>
    <current>3.87ms mean latency</current>
    <target>2.0ms p95 latency</target>
    <gap>1.87ms (94% over target)</gap>
    <likely_causes>
      - O(n*m) distance computations for 1000 same_cluster + 500 nearest_cluster
      - No SIMD optimization for distance calculations
      - Sampling not engaged (cluster size = max_sample_size = 1000)
    </likely_causes>
    <recommended_optimizations>
      - Enable SIMD vectorization for distance functions
      - Lower default max_sample_size to 500 for production
      - Consider centroid-based approximation for large clusters
    </recommended_optimizations>
  </performance_gap_analysis>
</state_verification>

<manual_test_design>
  <test_case id="benchmark-build-verify">
    <command>cargo build -p context-graph-utl --benches</command>
    <expected_output>Compiles without errors</expected_output>
    <verification>Exit code 0, "Finished" message</verification>
  </test_case>

  <test_case id="benchmark-primary-target">
    <input>
      query = generate_embedding(1536, 42)
      same_cluster = generate_cluster(1000, 1536, 100)
      nearest_cluster = generate_cluster(500, 1536, 10000)
      config = ClusterFitConfig::default() // Cosine
    </input>
    <expected_output>
      - Benchmark runs successfully
      - Mean latency measured and reported
      - Target: < 2ms p95
    </expected_output>
    <actual_output>
      - Mean: 3.87ms
      - Target: NOT MET
    </actual_output>
    <verification>criterion outputs timing statistics</verification>
  </test_case>

  <test_case id="benchmark-scaling-linear">
    <input>Cluster sizes: 10, 50, 100, 500, 1000</input>
    <expected_output>O(n) scaling with cluster size</expected_output>
    <verification>Throughput (elements/sec) should remain roughly constant</verification>
  </test_case>
</manual_test_design>

<backwards_compatibility>
  <policy>ABSOLUTELY NO BACKWARDS COMPATIBILITY for invalid states</policy>
  <enforcement>
    - Benchmark MUST fail fast if compute_cluster_fit returns NaN/Infinity
    - black_box prevents optimizer from eliminating invalid computation
    - If results are invalid, benchmark timings would be meaningless
  </enforcement>
</backwards_compatibility>

<input_context_files>
  <file purpose="cluster_fit_compute" path="crates/context-graph-utl/src/coherence/cluster_fit/compute.rs">
    Main compute_cluster_fit function to benchmark
  </file>
  <file purpose="cluster_fit_types" path="crates/context-graph-utl/src/coherence/cluster_fit/types.rs">
    ClusterFitConfig, ClusterContext, DistanceMetric types
  </file>
  <file purpose="benchmark_file" path="crates/context-graph-utl/benches/cluster_fit_bench.rs">
    Benchmark implementation with 6 benchmark functions
  </file>
  <file purpose="cargo_config" path="crates/context-graph-utl/Cargo.toml">
    Benchmark configuration with [[bench]] entry
  </file>
</input_context_files>

<definition_of_done>
  <signatures status="ALL_IMPLEMENTED">
    <signature file="cluster_fit_bench.rs" type="benchmark_function" status="EXISTS">
fn bench_cluster_fit_1000_embeddings(c: &amp;mut Criterion) {
    // IMPLEMENTED at lines 44-64
    // Single query against cluster of 1000 members
    // Target: < 2ms p95 latency (NOT MET - actual 3.87ms)
}
    </signature>
    <signature file="cluster_fit_bench.rs" type="benchmark_group" status="EXISTS">
fn bench_cluster_fit_cluster_scaling(c: &amp;mut Criterion) {
    // IMPLEMENTED at lines 70-98
    // Scaling across cluster sizes: 10, 50, 100, 500, 1000
}
    </signature>
    <signature file="cluster_fit_bench.rs" type="benchmark_group" status="EXISTS">
fn bench_cluster_fit_dimension_scaling(c: &amp;mut Criterion) {
    // IMPLEMENTED at lines 104-128
    // Scaling across dimensions: 384, 768, 1536, 3072
}
    </signature>
    <signature file="cluster_fit_bench.rs" type="benchmark_group" status="EXISTS">
fn bench_cluster_fit_distance_metrics(c: &amp;mut Criterion) {
    // IMPLEMENTED at lines 134-164
    // Compare Cosine vs Euclidean vs Manhattan performance
}
    </signature>
    <signature file="Cargo.toml" type="bench_entry" status="EXISTS">
[[bench]]
name = "cluster_fit_bench"
harness = false
    </signature>
  </signatures>
  <constraints status="PARTIALLY_MET">
    <constraint status="NOT_MET">Target: < 2ms p95 for 1000 embeddings at 1536 dimensions (Actual: 3.87ms)</constraint>
    <constraint status="MET">Use deterministic data generation (same seed = same results)</constraint>
    <constraint status="MET">Include throughput metrics (elements/second)</constraint>
    <constraint status="MET">Benchmark compiles and runs with `cargo bench`</constraint>
    <constraint status="MET">Use black_box to prevent optimizer from eliminating computation</constraint>
  </constraints>
  <verification status="BENCHMARK_WORKS_PERFORMANCE_NOT_MET">
    <command>cargo bench -p context-graph-utl --bench cluster_fit_bench -- --noplot</command>
    <assertion status="VERIFIED">Benchmark compiles and runs successfully</assertion>
    <assertion status="FAILED">cluster_fit_1000_embeddings mean latency < 2ms (Actual: 3.87ms)</assertion>
    <assertion status="VERIFIED">Criterion HTML reports generated in target/criterion/</assertion>
  </verification>
</definition_of_done>

<validation_criteria status="PARTIALLY_VERIFIED">
  <criterion id="VC-1" status="VERIFIED">Benchmark compiles with `cargo build -p context-graph-utl --benches`</criterion>
  <criterion id="VC-2" status="VERIFIED">Benchmark runs with `cargo bench -p context-graph-utl --bench cluster_fit_bench`</criterion>
  <criterion id="VC-3" status="VERIFIED">cluster_fit_1000_embeddings_1536d mean latency is measured (3.87ms)</criterion>
  <criterion id="VC-4" status="VERIFIED">Criterion HTML reports generated in target/criterion/</criterion>
  <criterion id="VC-5" status="TO_VERIFY">Scaling benchmarks show expected O(n) scaling with cluster size</criterion>
  <criterion id="VC-6" status="VERIFIED">Target latency < 2ms documented in benchmark comments</criterion>
  <criterion id="VC-7" status="FAILED">Target performance met - ACTUAL: 3.87ms, TARGET: < 2ms</criterion>
</validation_criteria>

<test_commands>
  <command description="Build benchmarks">cargo build -p context-graph-utl --benches</command>
  <command description="Run all ClusterFit benchmarks">cargo bench -p context-graph-utl --bench cluster_fit_bench -- --noplot</command>
  <command description="Run primary target benchmark">cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_fit_1000 -- --noplot</command>
  <command description="Run scaling benchmarks">cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_scaling -- --noplot</command>
  <command description="Save baseline">cargo bench --bench cluster_fit_bench -- --save-baseline main</command>
</test_commands>

<error_handling>
  <error case="Benchmark fails to compile">
    Check ClusterFit module exports in lib.rs. compute_cluster_fit must be public.
    VERIFIED: Exports work correctly via `pub use cluster_fit::{...}`
  </error>
  <error case="Benchmark times out">
    Reduce sample_size or iterations. Check for accidental O(n^2) behavior.
  </error>
  <error case="Performance regression detected">
    Criterion will report "regressed" - investigate algorithm changes or data generation.
  </error>
</error_handling>

<followup_tasks>
  <task id="OPT-001" priority="HIGH">
    <title>Optimize ClusterFit to meet < 2ms target</title>
    <description>Current performance 3.87ms is 1.94x over target. Optimization required.</description>
    <suggested_approaches>
      - SIMD vectorization for distance calculations (cosine_distance hot path)
      - Reduce default max_sample_size from 1000 to 500
      - Implement centroid-based approximation for clusters > 500 members
      - Consider parallel distance computation with rayon
    </suggested_approaches>
  </task>
</followup_tasks>

<notes>
  <note>
    Performance target of < 2ms p95 is based on ClusterFit being called in the UTL hot path.
    This allows ~500 ClusterFit calculations per second per thread.
    Current performance (3.87ms) allows only ~258 calculations per second.
  </note>
  <note>
    The benchmark uses deterministic data generation via sin() functions for reproducibility.
    Different seeds produce different but consistent vectors.
  </note>
  <note>
    Criterion automatically detects performance regressions by comparing against baseline.
    Run `cargo bench --bench cluster_fit_bench -- --save-baseline main` to save baseline.
  </note>
  <note>
    The max_sample_size parameter in ClusterFitConfig prevents O(n^2) explosion for
    very large clusters. Default is 1000, which the sampling benchmark validates.
  </note>
  <note>
    Module visibility: Benchmark imports from `context_graph_utl::coherence::{...}` which
    works correctly because of pub use re-exports in coherence/mod.rs.
  </note>
</notes>
</task_spec>
```

---

## Summary

| Field | Value |
|-------|-------|
| Task ID | TASK-UTL-P2-002 |
| Title | Add ClusterFit Benchmark with Performance Regression Detection |
| Status | **COMPLETED-WITH-FINDINGS** |
| Layer | Foundation |
| Priority | P2 (Medium) |
| Complexity | Low |
| Files Modified | 1 (Cargo.toml) |
| Files Created | 1 (cluster_fit_bench.rs) |

## Benchmark Results (2026-01-12)

| Benchmark | Mean Latency | Target | Status |
|-----------|--------------|--------|--------|
| cluster_fit_1000_embeddings_1536d | 3.87ms | < 2ms | **FAILS TARGET** |

## Performance Gap Analysis

| Metric | Value |
|--------|-------|
| Current Performance | 3.87ms mean |
| Target Performance | < 2ms p95 |
| Gap | 1.87ms (94% over target) |
| Throughput Impact | 258/sec actual vs 500/sec target |

## Benchmarks Implemented

1. `cluster_fit_1000_embeddings_1536d` - Primary target (FAILS: 3.87ms vs 2ms target)
2. `cluster_fit_cluster_scaling` - 10/50/100/500/1000 cluster sizes
3. `cluster_fit_dimension_scaling` - 384/768/1536/3072 dimensions
4. `cluster_fit_distance_metrics` - Cosine/Euclidean/Manhattan comparison
5. `cluster_fit_batch_100_queries` - Batch processing
6. `cluster_fit_sampling` - Sampling threshold impact

## Constitution Compliance

- **UTL-002**: ClusterFit is part of delta_C formula, performance matters
- **AP-10**: Benchmarks implicitly verify no NaN/Infinity (would cause incorrect results)

## Follow-up Required

**OPT-001: Optimize ClusterFit to meet < 2ms target**

Suggested optimizations:
1. SIMD vectorization for cosine_distance (hot path)
2. Reduce default max_sample_size from 1000 to 500
3. Centroid-based approximation for large clusters
4. Parallel distance computation with rayon

## Verification Commands

```bash
# Build benchmarks
cargo build -p context-graph-utl --benches

# Run all benchmarks
cargo bench -p context-graph-utl --bench cluster_fit_bench -- --noplot

# Run primary target only
cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_fit_1000 -- --noplot

# Save baseline for regression detection
cargo bench --bench cluster_fit_bench -- --save-baseline main
```
