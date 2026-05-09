# TASK-INFER-014 — FSV harness (verify-evidence.json, conformal-calibration-evidence.json, ood-evidence.json, latency-histogram.json)

```xml
<task_spec id="TASK-INFER-014" version="1.0">

<metadata>
  <title>Inference FSV harness in `crates/context-graph-mejepa/examples/inference_fsv.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-INFER-18">Performance + coverage + AUC budgets verified empirically</item>
    <item ref="component:fsv_protocol">verify-evidence + conformal-calibration-evidence + ood-evidence + latency-histogram</item>
    <item ref="constitution:fsv_protocol">re-read + assert protocol; mode 0o600</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-011</task_ref>
    <task_ref>TASK-INFER-013</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (examples/inference_fsv.rs)</estimated_files>
</metadata>

<context>
This task lands the formal FSV harness per constitution &lt;fsv_protocol&gt;: every shipped primitive
ships with a forensic source verification example binary that writes evidence to
/tmp/contextgraph-mejepa-infer-fsv/ and re-reads + asserts before reporting success. The harness
exercises ALL FailedGate variants (one example each) plus an Approve example, persists conformal
calibration histogram + tau, persists ROC curve points + AUC, persists 1,000-call latency histogram,
all at mode 0o600.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; SEC-MEJEPA-07 mode 0o600)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-18; AC-INFER-01-04 latency; AC-INFER-02-01)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (fsv_protocol block; testing notes)</file>
  <file purpose="upstream_cli">crates/context-graph-mejepa/src/cli.rs (TASK-INFER-013 — InferTestReport pattern)</file>
  <file purpose="upstream_handler">crates/context-graph-mcp/src/handlers/tools/mejepa.rs (TASK-INFER-011 — handler entry point)</file>
  <file purpose="upstream_compiler">crates/context-graph-mejepa/src/compiler.rs (verify path)</file>
  <file purpose="reference_fsv">crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs (existing FSV harness pattern to mirror)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-011 complete; verify path callable end-to-end</check>
  <check>TASK-INFER-013 complete; CLI driver functional</check>
  <check>Mock predictor/frozen_target/oracle_head/constellation usable for synthetic boundary cases</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/examples/inference_fsv.rs` with `fn main() -> Result<(), Box<dyn Error>>`
    - Synthesize 8 fixture bundles, one per VerifyVerdict outcome: 1 Approve + 7 EscalateToHuman (one per FailedGate variant: OutOfDistribution, LowConfidence, PredictedFailure, GrangerRejection, WitnessChainBroken, SourceShaDrift, ConstellationGuardRejected)
    - Run verify() on each fixture; collect (input_panel_sha, predicted_oracle_pass, ood_score, outcome_set, granger_attestations, gates_passed, failed_gate, latency_ms) per call
    - Write `/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json` containing all 8 records
    - Write `/tmp/contextgraph-mejepa-infer-fsv/conformal-calibration-evidence.json` containing histogram of calibration non-conformity scores (50 buckets), tau picked, alpha, empirical coverage on calibration set (sanity, ~1-α), empirical coverage on held-out subset (target 0.88-0.92), min_samples_per_stratum
    - Write `/tmp/contextgraph-mejepa-infer-fsv/ood-evidence.json` containing ROC curve points (50 thresholds), AUC, sigma_squared, in-distribution histogram + OOD histogram
    - Write `/tmp/contextgraph-mejepa-infer-fsv/latency-histogram.json` containing 1,000-call p50 / p90 / p99 / p99.9 + cold-vs-warm split + per-gate latency contribution (Gate 6 SHA, Gate 5 Witness, Gate 1 OOD, Gate 2 PredFail, Gate 3 LowConf, Gate 4 Granger, Gate 7 TCT)
    - Re-read each evidence file; deserialize via `serde_json::from_slice`; assert byte-equal to canonical-key-order pretty-print of in-memory struct (constitution &lt;fsv_protocol&gt; "re-reads + asserts before reporting success")
    - For each file, after writing assert mode == 0o600 via `std::fs::metadata().permissions().mode() & 0o777 == 0o600` (Unix only; cfg-gate non-Unix to skip)
    - Domain-specific assertions:
      - verify-evidence: re-run verify() on the persisted bundle; assert byte-equal verdict
      - conformal-calibration-evidence: re-compute tau from histogram; assert ULP-equal to persisted tau
      - ood-evidence: re-compute AUC from raw scores; assert ULP-equal to persisted auc; assert auc > 0.85
      - latency-histogram: assert p50 < 100ms (per AC-INFER-01-04)
    - Print summary table to stdout; exit non-zero on any assertion failure
    - Use `examples/inference_fsv.rs` per constitution &lt;file_organization&gt; "FSV harness binaries under examples/&lt;name&gt;_fsv.rs per crate"
  </in_scope>
  <out_of_scope>
    - Real Phase 2 predictor — uses MockPredictor that produces deterministic synthetic outputs steering each FailedGate
    - CI integration — separate task downstream
    - Cross-platform mode bits — Unix only via cfg(unix)
    - Full SWE-bench Lite oracle — handled by Phase 0 corpus
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/examples/inference_fsv.rs">
fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt; {
    setup_fsv_dir()?;          // mkdir 0700
    write_verify_evidence()?;  // 8 fixtures: 1 Approve + 7 FailedGate
    write_conformal_calibration_evidence()?;
    write_ood_evidence()?;
    write_latency_histogram()?;
    re_read_and_assert_all()?;
    Ok(())
}

fn setup_fsv_dir() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_verify_evidence() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_conformal_calibration_evidence() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_ood_evidence() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_latency_histogram() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn re_read_and_assert_all() -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct VerifyEvidence {
    fixtures: Vec&lt;VerifyFixtureRecord&gt;,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct VerifyFixtureRecord {
    case_name: String,
    input_panel_sha: String,
    predicted_oracle_pass: f32,
    ood_score: f32,
    outcome_set: Vec&lt;context_graph_mejepa::types::OracleOutcome&gt;,
    granger_attestations: std::collections::BTreeMap&lt;String, f32&gt;,
    gates_passed: u8,
    failed_gate: Option&lt;serde_json::Value&gt;,
    latency_ms: f32,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct ConformalCalibrationEvidence {
    score_histogram: Vec&lt;u32&gt;,    // 50 buckets
    tau: f32,
    alpha: f32,
    empirical_coverage_calibration: f32,
    empirical_coverage_holdout: f32,
    min_samples_per_stratum: usize,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct OodEvidence {
    roc_points: Vec&lt;(f32, f32)&gt;,   // 50 (fpr, tpr) points
    auc: f32,
    sigma_squared: f32,
    in_dist_histogram: Vec&lt;u32&gt;,
    ood_histogram: Vec&lt;u32&gt;,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct LatencyHistogram {
    samples_ms: Vec&lt;f32&gt;,
    p50_ms: f32,
    p90_ms: f32,
    p99_ms: f32,
    p999_ms: f32,
    cold_p50_ms: f32,
    warm_p50_ms: f32,
    per_gate_p50_ms: std::collections::BTreeMap&lt;String, f32&gt;,
}
    </signature>
  </signatures>
  <constraints>
    - All 4 evidence files mode 0o600 verified via stat after write
    - Re-read via serde_json::from_slice; round-trip equality asserted (canonical key order via BTreeMap not HashMap for granger)
    - Domain-specific assertions per file run AFTER serde round-trip
    - On any assertion failure, exit code != 0 with structured stderr message identifying the failed assertion
    - Determinism: run twice → byte-equal evidence files (modulo timing samples; the histogram bucket counts MUST match)
    - Synthetic fixtures use seeded RNG so they reproduce
    - 1,000 verify calls for latency are over the same fixture (instruments cached path) per AC-INFER-01-04 to satisfy p50 < 100ms
    - Cold p50 measured by clearing instrument cache between calls; warm p50 measured with cache populated
  </constraints>
  <verification>
    - `cargo run --release -p context-graph-mejepa --example inference_fsv` succeeds with exit 0
    - All 4 files exist at /tmp/contextgraph-mejepa-infer-fsv/ with mode 0o600
    - `cargo test -p context-graph-mejepa --example inference_fsv` passes (if any tests inline)
    - Re-reading each file deserializes successfully and round-trips byte-equal
    - latency-histogram p50 < 100ms
    - ood-evidence auc > 0.85
    - verify-evidence has exactly 8 fixtures with all 7 FailedGate variants represented + 1 Approve
  </verification>
</definition_of_done>

<pseudo_code>
fn write_verify_evidence() -> Result<(), Box<dyn Error>> {
    let cases = [
        ("approve",                  build_approve_fixture()),
        ("ood",                      build_ood_fixture()),
        ("low_confidence",           build_low_confidence_fixture()),
        ("predicted_failure",        build_predicted_failure_fixture()),
        ("granger_rejection",        build_granger_rejection_fixture()),
        ("witness_chain_broken",     build_witness_chain_broken_fixture()),
        ("source_sha_drift",         build_source_sha_drift_fixture()),
        ("constellation_guard",      build_constellation_guard_rejected_fixture()),
    ];
    let mut fixtures = Vec::new();
    for (name, (bundle, ctx)) in cases {
        let t0 = Instant::now();
        let verdict = compiler.verify(&bundle, &ctx)?;
        let lat_ms = t0.elapsed().as_secs_f32() * 1000.0;
        fixtures.push(record_from_verdict(name, &bundle, &verdict, lat_ms));
    }
    let evidence = VerifyEvidence { fixtures };
    write_json_0600("/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json", &evidence)?;
    Ok(())
}

fn write_json_0600<T: serde::Serialize>(path: &str, value: &T) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::OpenOptionsExt;
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true).create(true).truncate(true)
        .mode(0o600).open(path)?;
    f.write_all(&bytes)?;
    Ok(())
}

fn re_read_and_assert_all() -> Result<(), Box<dyn Error>> {
    // verify-evidence
    let bytes = std::fs::read("/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json")?;
    let parsed: VerifyEvidence = serde_json::from_slice(&bytes)?;
    let written: VerifyEvidence = recompute_from_in_memory(); // fresh copy for compare
    assert_eq!(parsed, written, "verify-evidence round-trip mismatch");
    let mode = std::fs::metadata("/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json")?
        .permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "verify-evidence mode {:o} != 0o600", mode);
    // ... same for other 3 files ...

    // domain-specific
    let lat: LatencyHistogram = serde_json::from_slice(&std::fs::read("/tmp/contextgraph-mejepa-infer-fsv/latency-histogram.json")?)?;
    assert!(lat.p50_ms < 100.0, "p50_ms {} >= 100ms", lat.p50_ms);
    let ood: OodEvidence = serde_json::from_slice(&std::fs::read("/tmp/contextgraph-mejepa-infer-fsv/ood-evidence.json")?)?;
    assert!(ood.auc > 0.85, "AUC {} <= 0.85", ood.auc);
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/examples/inference_fsv.rs">FSV harness writing 4 evidence files + re-read + assert</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `[[example]] name = "inference_fsv"`; ensure example deps available</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo run --release -p context-graph-mejepa --example inference_fsv exits 0</criterion>
  <criterion>All 4 evidence files exist with mode 0o600</criterion>
  <criterion>verify-evidence has 8 fixtures (1 Approve + 7 FailedGate variants)</criterion>
  <criterion>Each file round-trips byte-equal via serde_json</criterion>
  <criterion>p50 latency &lt; 100ms (instruments cached)</criterion>
  <criterion>OOD AUC &gt; 0.85</criterion>
  <criterion>Empirical coverage on holdout in [0.88, 0.92]</criterion>
  <criterion>Two consecutive runs produce byte-equal histograms (modulo timing fields)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --example inference_fsv</command>
  <command>cargo run --release -p context-graph-mejepa --example inference_fsv</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-infer-fsv/conformal-calibration-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-infer-fsv/ood-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-infer-fsv/latency-histogram.json</command>
</test_commands>

</task_spec>
```
