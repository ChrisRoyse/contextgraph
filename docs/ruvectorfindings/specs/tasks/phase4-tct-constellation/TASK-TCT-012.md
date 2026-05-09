# TASK-TCT-012 — FSV harness (constellation-evidence, gtau-evidence, threshold-calibration-evidence, chunk-level-rejection-evidence)

```xml
<task_spec id="TASK-TCT-012" version="1.0">

<metadata>
  <title>TCT FSV harness in `crates/context-graph-mejepa-tct/examples/tct_fsv.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-TCT-14">FSV harness writes 4 evidence files; re-reads + asserts; mode 0o600</item>
    <item ref="AC-TCT-07-02">FSV files at /tmp/contextgraph-mejepa-tct-fsv/{constellation,gtau,threshold-calibration,chunk-level-rejection}-evidence.json</item>
    <item ref="AC-TCT-03-02">External-verifier replay reproduces gtau_satisfied + per-violation cosines within f32 ULP</item>
    <item ref="AC-TCT-07-01">Verdict reproducibility ≥ 99.9% per doc 13 §11 falsifier #8</item>
    <item ref="constitution:fsv_protocol">re-read + assert; mode 0o600</item>
    <item ref="NFR-TCT-04">Determinism across 1,000 invocations</item>
    <item ref="NFR-TCT-05">External-verifier reproducibility within ULP</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-006</task_ref>
    <task_ref>TASK-TCT-007</task_ref>
    <task_ref>TASK-TCT-008</task_ref>
    <task_ref>TASK-TCT-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (examples/tct_fsv.rs)</estimated_files>
</metadata>

<context>
This task lands the formal FSV harness per constitution &lt;fsv_protocol&gt;. Writes 4 evidence files,
re-reads each, asserts byte-equal serde round-trip, and runs domain-specific replay assertions
(re-run Gτ on persisted panel; verify byte-equal verdict). All files mode 0o600 per SEC-MEJEPA-07.
The harness is the falsifier for the per-output verifiability claim per Royse 2026c §1: any third
party with the frozen embedders + the persisted constellation reproduces gtau_satisfied + per-
violation cosines to within f32 ULP.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; SEC-MEJEPA-07)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-14; AC-TCT-07-01..02; NFR-TCT-04, 05; doc 13 §11 falsifier #8)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (fsv_protocol via test_strategy.fsv_assertion_protocol; file_structure tct_fsv.rs reference)</file>
  <file purpose="upstream_gtau">crates/context-graph-mejepa-tct/src/gtau.rs (TASK-TCT-006)</file>
  <file purpose="upstream_hinge">crates/context-graph-mejepa-tct/src/hinge.rs (TASK-TCT-007)</file>
  <file purpose="upstream_store">crates/context-graph-mejepa-tct/src/store.rs (TASK-TCT-008)</file>
  <file purpose="upstream_rate">crates/context-graph-mejepa-tct/src/rate_aggregator.rs (TASK-TCT-010)</file>
  <file purpose="reference_fsv">crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs (existing FSV pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-006/007/008/010 complete</check>
  <check>Synthetic panel fixtures available from instruments crate</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/examples/tct_fsv.rs` with `fn main() -> Result<(), Box<dyn Error>>`
    - Synthesize a small constellation (subset of cells; ~5 mutation × 3 languages × 4 entity types × 21 embedders) for fast harness execution
    - Synthesize 8 panel fixtures: 1 accepted (all cos ≥ τ) + 7 rejected (one per FailedGate-equivalent: each violating a different embedder slot below τ)
    - Synthesize 3-chunk patch with 1 chunk failing for chunk-level Gτ
    - Write evidence files at /tmp/contextgraph-mejepa-tct-fsv/ mode 0o600:
      - `constellation-evidence.json`: TctConstellation summary (cell counts; sample support histogram; threshold table) — not raw centroids; round-trip via serde
      - `gtau-evidence.json`: 8 panel fixtures × {gtau_satisfied, violations, centroid_origin, elapsed_ms}; per-fixture re-run gtau_check on persisted panel; assert byte-equal output to within ULP
      - `threshold-calibration-evidence.json`: per-cell empirical FPR + chosen τ_m + sample counts (from CalibrationDecision); assert all FPR ≤ 0.05
      - `chunk-level-rejection-evidence.json`: 3-chunk fixture × ChunkGtauOutput → assert violating_chunks contains exactly the 1 failing chunk; round-trip serde
    - Re-read each file; deserialize; assert byte-equal to canonical-pretty-print of in-memory struct
    - Domain-specific assertions:
      - constellation-evidence: round-trip preserves all summary stats; cell counts match expected
      - gtau-evidence: re-run Gτ on persisted constellation + panel → byte-equal verdict (NFR-TCT-04 determinism)
      - threshold-calibration-evidence: every FPR ≤ 0.05; (cells skipped due to N&lt;30 marked strict_enforcement=false)
      - chunk-level-rejection-evidence: violating_chunks.len() == 1; aggregate_satisfied=false
    - Print summary table to stdout; exit non-zero on any assertion failure
    - All files mode 0o600 verified via stat
  </in_scope>
  <out_of_scope>
    - Real Phase 0 corpus (uses synthetic panels)
    - Real predictor (synthetic panels chosen to steer each Gτ outcome)
    - CI integration
    - End-to-end with mejepa_constellation_inspect MCP roundtrip (TASK-TCT-011 has its own integration tests)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/examples/tct_fsv.rs">
fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt; {
    setup_fsv_dir()?;
    let (constellation, decisions) = build_synthetic_constellation()?;
    write_constellation_evidence(&amp;constellation, &amp;decisions)?;
    write_gtau_evidence(&amp;constellation)?;
    write_threshold_calibration_evidence(&amp;decisions)?;
    write_chunk_level_rejection_evidence(&amp;constellation)?;
    re_read_and_assert_all(&amp;constellation)?;
    Ok(())
}

fn build_synthetic_constellation() -&gt; Result&lt;(TctConstellation, Vec&lt;CalibrationDecision&gt;), TctError&gt;;
fn write_constellation_evidence(c: &amp;TctConstellation, decisions: &amp;[CalibrationDecision]) -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_gtau_evidence(c: &amp;TctConstellation) -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_threshold_calibration_evidence(d: &amp;[CalibrationDecision]) -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn write_chunk_level_rejection_evidence(c: &amp;TctConstellation) -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;
fn re_read_and_assert_all(c: &amp;TctConstellation) -&gt; Result&lt;(), Box&lt;dyn Error&gt;&gt;;

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct ConstellationEvidence {
    version_id: String,
    cell_counts: CellCounts,
    sample_support_histogram: SampleSupportHistogram,
    panel_level_threshold_summary: BTreeMap&lt;EmbedderId, f32&gt;,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct GtauEvidence {
    fixtures: Vec&lt;GtauFixtureRecord&gt;,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct GtauFixtureRecord {
    name: String,
    panel_sha: String,
    output: GtauOutput,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct ThresholdCalibrationEvidence {
    decisions: Vec&lt;CalibrationDecision&gt;,
    summary: ThresholdSummary,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct ThresholdSummary {
    cells_with_strict_enforcement: u32,
    max_observed_fpr: f32,
    target_fpr: f32,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
struct ChunkLevelRejectionEvidence {
    chunk_count: u32,
    violating_chunk_count: u32,
    aggregate_satisfied: bool,
    per_chunk: Vec&lt;ChunkRecord&gt;,
}
    </signature>
  </signatures>
  <constraints>
    - All 4 evidence files mode 0o600 via OpenOptions::new().mode(0o600) on Unix
    - Round-trip via serde_json::from_slice; round-trip equality via PartialEq derive on each evidence struct
    - Domain-specific assertion: re-run gtau_check on persisted constellation + panel → assert byte-equal GtauOutput (NFR-TCT-04 determinism + NFR-TCT-05 reproducibility)
    - Threshold calibration evidence: every cell with strict_enforcement=true has observed_fpr ≤ 0.05
    - Chunk-level evidence: violating_chunks.len() matches expected (1 in synthetic)
    - Determinism: run twice → byte-equal output (modulo elapsed_ms timing)
    - Synthetic constellation small enough to fit in &lt; 1 MB across all 4 files combined
    - All file writes prefixed with `tracing::info!` per &lt;error_handling&gt;
  </constraints>
  <verification>
    - `cargo run --release -p context-graph-mejepa-tct --example tct_fsv` exits 0
    - All 4 files exist at /tmp/contextgraph-mejepa-tct-fsv/ mode 0o600
    - Each file round-trips via serde_json byte-equal
    - gtau-evidence: re-run Gτ on persisted panel → byte-equal output
    - threshold-calibration-evidence: max_observed_fpr ≤ 0.05
    - chunk-level-rejection-evidence: violating_chunks.len() == 1
    - Two consecutive runs produce byte-equal evidence files (modulo elapsed_ms)
  </verification>
</definition_of_done>

<pseudo_code>
fn write_gtau_evidence(constellation: &TctConstellation) -> Result<(), Box<dyn Error>> {
    let cases = synth_8_panel_fixtures();
    let mut fixtures = Vec::new();
    for (name, panel, predicted_class, language, entity_type) in cases {
        let output = gtau::gtau_check(&panel, predicted_class, language, entity_type, constellation)?;
        fixtures.push(GtauFixtureRecord {
            name: name.into(),
            panel_sha: hex::encode(panel.sha256()),
            output,
        });
    }
    let evidence = GtauEvidence { fixtures };
    write_json_0600("/tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json", &evidence)?;
    Ok(())
}

fn re_read_and_assert_all(constellation: &TctConstellation) -> Result<(), Box<dyn Error>> {
    // gtau-evidence: re-run Gτ on persisted panel and assert byte-equal
    let bytes = std::fs::read("/tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json")?;
    let parsed: GtauEvidence = serde_json::from_slice(&bytes)?;
    for fixture in &parsed.fixtures {
        // Re-synthesize the same panel from the same seed; re-run gtau
        let (panel, predicted_class, language, entity_type) = re_synthesize_fixture(&fixture.name);
        let recomputed = gtau::gtau_check(&panel, predicted_class, language, entity_type, constellation)?;
        // Modulo elapsed_ms timing field
        let mut a = fixture.output.clone();
        let mut b = recomputed.clone();
        a.elapsed_ms = 0.0;
        b.elapsed_ms = 0.0;
        assert_eq!(a, b, "Gτ replay mismatch on {}", fixture.name);
    }
    // mode check
    let mode = std::fs::metadata("/tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json")?
        .permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    // ... same for other 3 files ...
    Ok(())
}

fn write_json_0600<T: serde::Serialize>(path: &str, value: &T) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::io::Write;
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true).create(true).truncate(true)
        .mode(0o600).open(path)?;
    f.write_all(&bytes)?;
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/examples/tct_fsv.rs">FSV harness writing 4 evidence files + re-read + assert</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">add `[[example]] name = "tct_fsv"`; ensure example deps available</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo run --release -p context-graph-mejepa-tct --example tct_fsv exits 0</criterion>
  <criterion>4 evidence files exist with mode 0o600</criterion>
  <criterion>Each file round-trips byte-equal via serde_json</criterion>
  <criterion>Gτ replay produces byte-equal GtauOutput (modulo elapsed_ms)</criterion>
  <criterion>Threshold-calibration max_observed_fpr ≤ 0.05</criterion>
  <criterion>Chunk-level violating_chunks count matches expected</criterion>
  <criterion>Two consecutive runs byte-equal evidence (modulo elapsed_ms)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct --example tct_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-tct --example tct_fsv</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-tct-fsv/constellation-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-tct-fsv/threshold-calibration-evidence.json</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-tct-fsv/chunk-level-rejection-evidence.json</command>
</test_commands>

</task_spec>
```
