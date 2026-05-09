# TASK-INSTR-023 — `phase1_extended_fsv` harness extending shipped panel.json with all 14 instruments × 4 boundary cases

```xml
<task_spec id="TASK-INSTR-023" version="1.0">

<metadata>
  <title>phase1_extended_fsv — extend shipped panel.json FSV harness with all 14 instruments × 4 boundary cases (all_pass / mixed / crash_assertion_error / crash_type_error)</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>23</sequence>
  <implements>
    <item ref="REQ-INSTR-20">FSV harness (4 boundary cases × N instruments + panel.json)</item>
    <item ref="AC-INSTR-05-01">Per-boundary-case files; re-read + assertion before reporting success</item>
    <item ref="AC-INSTR-05-02">panel.json deserializes via try_from gate</item>
    <item ref="REQ-INSTR-22">panel materialization perf budgets profiled (latency_p50_p99.json)</item>
    <item ref="constitution:fsv_protocol">Every shipped primitive ships an FSV evidence harness</item>
    <item ref="constitution:SEC-MEJEPA-07">FSV files mode 0o600</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-020</task_ref>
    <task_ref>TASK-INSTR-021</task_ref>
    <task_ref>TASK-INSTR-022</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file modified (existing example), 0-1 files created</estimated_files>
</metadata>

<context>
The existing `examples/phase1_extended_fsv.rs` (per memory/project_phase0_mejepa_mutation_corpus.md
2026-05-09d entry) ships evidence for E_Oracle / E_Witness / E_StaticAnalysis (3 instruments × 4
boundary cases + panel.json 46 KB). This task extends it to cover ALL 14 named instruments + the
Scalars aggregator across all 4 boundary cases (all_pass / mixed / crash_assertion_error /
crash_type_error), plus the full 5,120-d panel.json, plus the latency profile for REQ-INSTR-22.

Per constitution `<fsv_protocol>` + REQ-INSTR-20 + AC-INSTR-05-01: write evidence to
`/tmp/contextgraph-mejepa-instruments-fsv/` with mode 0o600 (SEC-MEJEPA-07), re-read each file,
assert dim correctness + finiteness + deserialize round-trip, then report success.

The 4 boundary cases:

1. **all_pass**: Every test passes; no exceptions; runtime walltime tight; reasoning happy-path; etc.
2. **mixed**: Some pass, some fail, some skip; partial coverage delta; mixed reasoning events
3. **crash_assertion_error**: Top-level AssertionError; per_test_outcomes empty; runtime exit_code=1
4. **crash_type_error**: Top-level TypeError; runtime peak_rss elevated; reasoning includes ToolResult error
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-20, REQ-INSTR-22, AC-INSTR-05-01/02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (test_strategy fsv + performance)</file>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol, SEC-MEJEPA-07, fsv_evidence_paths)</file>
  <file purpose="shipped_fsv">crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs (existing)</file>
  <file purpose="materialize">crates/context-graph-mejepa-instruments/src/materialize.rs (TASK-INSTR-020)</file>
  <file purpose="panel_json">crates/context-graph-mejepa-instruments/src/panel_json.rs (TASK-INSTR-022)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-020 done (materialize_panel ships)</check>
  <check>TASK-INSTR-021 done (linear-probe binaries available — they share the corpus loader)</check>
  <check>TASK-INSTR-022 done (panel_to_json + panel_from_json + EmbedderPins available)</check>
  <check>/tmp writable; mode 0o600 setting available on Unix host</check>
  <check>Phase 0 mutation corpus accessible OR synthetic boundary fixtures provided</check>
</prerequisites>

<scope>
  <in_scope>
    - Extend `crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs` (or create a new `phase1_full_panel_fsv.rs`)
    - For each of 4 boundary cases × all 14 named instruments + Scalars:
      - Build typed input fixture (OracleVerdict, WitnessChainInput, CodeInstrumentInput, RuntimeInput, ReasoningInput, etc.)
      - Encode via the corresponding instrument
      - Write `e_x_{boundary}_{input,encoded}.json` with mode 0o600
      - Re-read each file; assert dim correctness + finiteness + deserialize round-trip
    - For each of the 4 boundary cases:
      - Materialize panel[t=2] (full panel via `materialize_panel(TimeStep::T2, ctx, registry)`)
      - Serialize via `panel_to_json` with provenance (code_version=git rev-parse HEAD; embedder_versions from current models_config.toml)
      - Write to `/tmp/contextgraph-mejepa-instruments-fsv/panel_{boundary}.json` mode 0o600
      - Re-read via `panel_from_json` + assert SHA-256 round-trip + deserialize round-trip
    - Latency profile: time each materialize_panel call; write histogram to `/tmp/contextgraph-mejepa-instruments-fsv/latency_p50_p99.json`
    - Aggregate report `phase1_extended_fsv_report.json`: per-instrument pass/fail + per-boundary panel pass/fail + latency p50/p99
    - Print human-readable summary on stdout
    - Exit code 0 on full success; non-zero on any assertion failure (with file path + first failing assertion)
  </in_scope>
  <out_of_scope>
    - Linear-probe binaries (TASK-INSTR-021 — separate)
    - Phase 7 mejepa-shift-subscriber (Phase 7 spec)
    - Per-language FSV across all 11 languages (covered partly in TASK-INSTR-008..011 + TASK-INSTR-014; aggregate at panel level here only for primary language)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs">
fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt; {
    let evidence_dir = Path::new("/tmp/contextgraph-mejepa-instruments-fsv");
    std::fs::create_dir_all(evidence_dir)?;

    let registry = build_instrument_registry()?;
    let pins = EmbedderPins::from_config(Path::new("config/models_config.toml")).ok();

    let mut report = FsvReport::default();
    for boundary in [Boundary::AllPass, Boundary::Mixed, Boundary::CrashAssertionError, Boundary::CrashTypeError] {
        let ctx = build_attempt_context_for_boundary(boundary)?;

        // Per-instrument evidence
        for slot in InstrumentSlot::all() {
            let (input_json, encoded_json) = encode_instrument_for_evidence(slot, &amp;ctx, &amp;registry)?;
            write_evidence_secure(&amp;evidence_dir.join(format!("{}_{}_input.json", slot.slug(), boundary.slug())), &amp;input_json)?;
            write_evidence_secure(&amp;evidence_dir.join(format!("{}_{}_encoded.json", slot.slug(), boundary.slug())), &amp;encoded_json)?;
            verify_evidence_roundtrip(&amp;input_json, &amp;encoded_json, slot)?;
            report.record_instrument(slot, boundary, true);
        }

        // Full panel evidence
        let t0 = std::time::Instant::now();
        let result = materialize_panel(TimeStep::T2, &amp;ctx, &amp;registry)?;
        report.record_latency(boundary, t0.elapsed());
        let provenance = build_provenance()?;
        let panel_json = panel_to_json(&amp;result.panel, &amp;provenance, 2, &amp;ctx.attempt_id)?;
        let panel_path = evidence_dir.join(format!("panel_{}.json", boundary.slug()));
        write_evidence_secure(&amp;panel_path, &amp;panel_json)?;
        if let Some(p) = &amp;pins {
            let _round = panel_from_json(&amp;panel_json, p)?;
        }
        report.record_panel(boundary, true);
    }

    write_evidence_secure(&amp;evidence_dir.join("latency_p50_p99.json"), &amp;report.latency_histogram())?;
    write_evidence_secure(&amp;evidence_dir.join("phase1_extended_fsv_report.json"), &amp;report)?;
    println!("{}", report.summary());
    if !report.all_passed() { std::process::exit(1); }
    Ok(())
}
    </signature>
  </signatures>
  <constraints>
    - Mode 0o600 on every written file (per SEC-MEJEPA-07; constitution coding_standards fsv_protocol)
    - Re-read every file before reporting success (per constitution fsv_protocol)
    - 4 boundary cases × 15 slots × 2 files = 120 instrument-level files + 4 panel files + 2 aggregate files = 126 files
    - panel_{boundary}.json deserializes via panel_from_json round-trip
    - Latency histogram p50 + p99 recorded
    - Exit code non-zero on any assertion failure (no silent success)
    - File path printed on every assertion failure
    - File &lt; 500 lines (split helpers into a sibling module if needed)
    - Determinism: same code_version + same embedder_versions -&gt; byte-equal evidence
  </constraints>
  <verification>
    - cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv
    - All 126 evidence files present in /tmp/contextgraph-mejepa-instruments-fsv/
    - stat() reports mode 0o600 on every file
    - panel_{boundary}.json deserializes without error
    - latency_p50_p99.json present with valid JSON
    - phase1_extended_fsv_report.json reports all_passed = true
    - Exit code 0 on success; non-zero if any case fails
  </verification>
</definition_of_done>

<pseudo_code>
phase1_extended_fsv.rs:

  fn write_evidence_secure(path, value) {
      let mut opts = OpenOptions::new();
      opts.create(true).write(true).truncate(true);
      #[cfg(unix)] opts.mode(0o600);
      let file = opts.open(path)?;
      serde_json::to_writer_pretty(file, value)?;
      // Re-read to confirm
      let bytes = std::fs::read(path)?;
      let _: Value = serde_json::from_slice(&amp;bytes)?;
      Ok(())
  }

  fn build_attempt_context_for_boundary(b: Boundary) -&gt; Result&lt;AttemptContext&gt; {
      match b {
          Boundary::AllPass =&gt; AttemptContext {
              oracle_verdict: Some(OracleVerdict { per_test_outcomes: vec![100x Pass], exception_class: None, total_test_count: 100 }),
              runtime_input: Some(RuntimeInput { walltime_ns_p50: 1_000_000, ..., determinism_flag: true }),
              reasoning_input: Some(ReasoningInput { events: happy_path_cot() }),
              ...
          },
          Boundary::Mixed =&gt; AttemptContext { ..., per_test_outcomes: 60 Pass + 30 Fail + 10 Skip, ... },
          Boundary::CrashAssertionError =&gt; AttemptContext { ..., exception_class: AssertionError, per_test_outcomes: empty, ... },
          Boundary::CrashTypeError =&gt; AttemptContext { ..., exception_class: TypeError, ..., reasoning_input: tool_error_cot() },
      }
  }

  fn verify_evidence_roundtrip(input_json, encoded_json, slot) {
      // Re-read (already in write helper); also assert encoded length matches slot.dim()
      let encoded: Vec&lt;f32&gt; = serde_json::from_value(encoded_json["vector"].clone())?;
      assert_eq!(encoded.len(), slot.dim());
      assert!(encoded.iter().all(|x| x.is_finite()));
  }

  struct FsvReport {
      instrument_results: HashMap&lt;(InstrumentSlot, Boundary), bool&gt;,
      panel_results: HashMap&lt;Boundary, bool&gt;,
      latency_ns: HashMap&lt;Boundary, u64&gt;,
  }
</pseudo_code>

<files_to_create>
  <!-- optional: examples/phase1_extended_fsv_helpers.rs sibling -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs">Extend from 3 instruments × 4 cases to 15 slots × 4 cases + full panel + latency profile + secure writes</file>
</files_to_modify>

<validation_criteria>
  <criterion>15 slots × 4 boundary cases × 2 files (input + encoded) = 120 files written</criterion>
  <criterion>4 panel_{boundary}.json files written with full 5,120-d data</criterion>
  <criterion>latency_p50_p99.json + phase1_extended_fsv_report.json written</criterion>
  <criterion>All 126 files have mode 0o600</criterion>
  <criterion>Each file re-read + asserted before report; failure prints file path</criterion>
  <criterion>panel_{boundary}.json round-trips via panel_to_json/panel_from_json</criterion>
  <criterion>Latency p99 &lt; 500 ms cold per NFR-INSTR-01</criterion>
  <criterion>Exit code 0 on full success; non-zero on any failure</criterion>
  <criterion>Determinism: re-running produces byte-equal evidence files</criterion>
  <criterion>cargo build --release --example phase1_extended_fsv clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>file under 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments --example phase1_extended_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv</command>
  <command>ls -la /tmp/contextgraph-mejepa-instruments-fsv/ | wc -l</command>
  <command>find /tmp/contextgraph-mejepa-instruments-fsv/ -type f -not -perm 600 -print</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
