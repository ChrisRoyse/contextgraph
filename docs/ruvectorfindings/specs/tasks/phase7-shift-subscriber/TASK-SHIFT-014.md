<task_spec id="TASK-SHIFT-014" version="1.0">
<metadata>
  <title>Surface — FSV harness: synthetic shift stream end-to-end + Phase 7 DoD `verify` CLI</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-SHIFT-15">Phase 7 DoD verification command: `cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./candidate.patch` reports `panel_materialization &lt; 1 s`, `me_jepa_verify_verdict`, `me_jepa_predicts_match_oracle: true` on a 5 % holdout sample, `corpus_entry_added: 1`, `total_corpus_size: previous + 1`</item>
    <item ref="REQ-SHIFT-10">FSV evidence on every release-build CI run; mode 0o600</item>
    <item ref="TC-SHIFT-10">Phase 7 DoD verification e2e test (verbatim doc 09 §7 Phase 7 DoD)</item>
    <item ref="EC-SHIFT-13">5 % holdout sample disagreement on 2 of 5 surfaces a non-zero exit + diagnostic blocker — the harness MUST produce that evidence path</item>
    <item ref="constitution#fsv_protocol">re-read-and-assert before reporting success; mode 0o600 on every evidence file</item>
    <item ref="testing_requirements[fsv_evidence_paths]">/tmp/contextgraph-mejepa-shift-subscriber-fsv/{watermark,observe-evidence,predict-latency-evidence,mcp-tool-roundtrip-evidence}.json</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
    <task_ref>TASK-SHIFT-004</task_ref>
    <task_ref>TASK-SHIFT-005</task_ref>
    <task_ref>TASK-SHIFT-006</task_ref>
    <task_ref>TASK-SHIFT-007</task_ref>
    <task_ref>TASK-SHIFT-008</task_ref>
    <task_ref>TASK-SHIFT-009</task_ref>
    <task_ref>TASK-SHIFT-010</task_ref>
    <task_ref>TASK-SHIFT-011</task_ref>
    <task_ref>TASK-SHIFT-012</task_ref>
    <task_ref>TASK-SHIFT-013</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 CLI bin + 1 verify-driver helper + 1 fixture-stream synthesizer reuse from TASK-SHIFT-013</estimated_files>
</metadata>

<context>
TASK-SHIFT-013 wires the FSV harness for the subscriber crate itself (writing 4 evidence files at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`). TASK-SHIFT-014 lands the **Phase 7 DoD `verify` subcommand** at the top-level `context-graph-mejepa` crate — the verbatim doc 09 §7 Phase 7 DoD entrypoint operators run to demonstrate that the shift-subscriber feedback loop closes end-to-end. This binary drives a synthetic shift stream against the subscriber, materializes a panel for the requested task, runs `predict()` + `verify()` through the inference compiler, samples 5 % of the holdout to check oracle agreement, appends one corpus entry through the SPEC-CORPUS writer, and asserts the four documented invariants from REQ-SHIFT-15. The 4 FSV evidence files from TASK-SHIFT-013 MUST already exist at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`; this task asserts their presence + adds nothing to that directory itself (the verify command's own evidence is the stdout report block + exit code 0/1/2).

The `verify` subcommand is the last piece of Phase 7. After it ships, an operator can drop a candidate patch, run the verbatim doc-09 command, and get a deterministic verdict on (a) whether the predictor agrees with the oracle on a 5 % sample and (b) whether the corpus row landed durably.
</context>

<input_context_files>
  <file purpose="dod_verbatim">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md (§7 Phase 7 DoD)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (REQ-SHIFT-15; TC-SHIFT-10; EC-SHIFT-13; US-SHIFT-10 i_want_to verbatim)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md (verify command behavior; corpus writer cross-cut)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-013.md (FSV evidence file producer this task asserts on)</file>
  <file purpose="cross_domain">crates/context-graph-mejepa-corpus/src/oracle_store.rs (corpus appender; corpus_entry_added counter source)</file>
  <file purpose="cross_domain_predictor">crates/context-graph-mejepa/src/compiler.rs (TASK-INFER-003 MeJepaCompiler::compile)</file>
  <file purpose="fsv_protocol">docs/ruvectorfindings/specs/constitution.md#fsv_protocol</file>
  <file purpose="evidence_paths">docs/ruvectorfindings/specs/constitution.md#testing_requirements[fsv_evidence_paths]</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001..013 complete</check>
  <check>TASK-SHIFT-013 has produced /tmp/contextgraph-mejepa-shift-subscriber-fsv/{watermark,observe-evidence,predict-latency-evidence,mcp-tool-roundtrip-evidence}.json on the same CI run (or the operator runs SHIFT-013 first locally; verify command fails closed if missing)</check>
  <check>SPEC-INFERENCE TASK-INFER-003 shipped: `MeJepaCompiler::compile(&PatchBundle, &TaskContext) -&gt; Result&lt;RealityPrediction, _&gt;` callable</check>
  <check>SPEC-CORPUS oracle store is openable as a writer for `corpus_entry_added` increment</check>
  <check>Holdout split materialized at `./data/mejepa-corpus/holdout/` (Phase 0 deliverable) — 5 % sample read from this directory</check>
  <check>`clap` v4 workspace dep available for subcommand parsing</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa/src/bin/mejepa.rs` — top-level binary with `clap::Subcommand` enum gaining a new `Verify` variant alongside any pre-existing subcommands
    - `crates/context-graph-mejepa/src/verify_cli.rs` — driver implementing the 6-step DoD body:
      1. **Parse + canonicalize**: `--task astropy__astropy-12907` (validate against task manifest); `--patch ./candidate.patch` (canonicalize, reject paths with `..` per SEC-MEJEPA-01)
      2. **Materialize panel**: load the 21-instrument panel for `task_id` via the same materialization path the subscriber uses (TASK-SHIFT-007); record `panel_materialization_ms` via `Instant::now()` delta. Assert `&lt; 1_000 ms` per REQ-SHIFT-15.
      3. **Predict**: `MeJepaCompiler::compile(&patch_bundle, &task_context) -&gt; RealityPrediction { verdict, predicted_oracle_pass, conformal_set, ood_score }`. Capture as `me_jepa_verify_verdict`.
      4. **5 % holdout sample agreement**: load the holdout split at `./data/mejepa-corpus/holdout/`; deterministic 5 % SplitMix64-seeded subsample (seed = SHA-256(`task_id`) → u64 first 8 bytes). For each sampled task, call compile() and read the actual oracle verdict from CF_MEJEPA_ORACLE_VERDICTS keyed by `(task_id, MutationCategory::KnownGood)`. Compute `agreement_count / sample_size`; assert `&gt;= 1.0` (every sampled task must match oracle) per REQ-SHIFT-15 strict reading. If `&lt; 1.0`, set `me_jepa_predicts_match_oracle: false` and the `disagreed_tasks` blocker list per EC-SHIFT-13. Exit code 1 on any disagreement.
      5. **Append corpus entry**: write the (panel, prediction, oracle) tuple to the corpus through the SPEC-CORPUS writer; capture `corpus_entry_added: 1`; read back `total_corpus_size` from the same writer's count API. Both before + after counts go into the report.
      6. **Re-read-and-assert FSV preconditions**: open each of the 4 TASK-SHIFT-013 evidence files; assert (a) the file exists; (b) on Unix, mode bits `& 0o777 == 0o600`; (c) `watermark.json` records `consumed &gt;= 100`; (d) `predict-latency-evidence.json` reports finite `p50_ms` and `p99_ms`; (e) `observe-evidence.json` contains at least one `observed` and one `dropped` example record; (f) `mcp-tool-roundtrip-evidence.json` (produced by sister phase TASK-MCP-007) is present and parseable. Any assertion failure exits 2.
    - **Stdout output** is a single JSON object on the FINAL line (preceding lines are tracing logs) with the canonical fields from REQ-SHIFT-15:
      ```json
      {
        "task": "astropy__astropy-12907",
        "panel_materialization_ms": 873,
        "me_jepa_verify_verdict": "approve",
        "me_jepa_predicts_match_oracle": true,
        "agreement_sample_size": 12,
        "agreement_count": 12,
        "corpus_entry_added": 1,
        "total_corpus_size_before": 2400,
        "total_corpus_size_after": 2401,
        "fsv_evidence_dir": "/tmp/contextgraph-mejepa-shift-subscriber-fsv/",
        "fsv_evidence_files_present": 4,
        "exit_code": 0,
        "blockers": []
      }
      ```
      Stdout's last line MUST be a single-line `serde_json::to_string` (NOT pretty) for log-grep simplicity; preceding tracing must be on stderr.
    - **Exit codes**: 0 on full success; 1 on `me_jepa_predicts_match_oracle: false` (oracle disagreement); 2 on FSV evidence missing or panel materialization &gt; 1 s budget breach.
    - **Configuration overrides** for testing: `--corpus-root &lt;path&gt;` (default `./data/mejepa-corpus/`); `--evidence-dir &lt;path&gt;` (default `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`); `--holdout-sample-pct &lt;float in [0.01, 1.0]&gt;` (default 0.05).
    - **Determinism**: SplitMix64 seeded from SHA-256(task_id) — identical input → identical sample. The agreement-count and sample-size MUST be byte-stable across re-runs.
    - **Tests**:
      - integration test `verify_succeeds_when_full_oracle_agreement` (mock predictor returns oracle-matching predictions for all 12 sampled tasks)
      - integration test `verify_fails_with_exit_1_on_2_of_5_disagreement` (mocked predictor disagrees on 2 sampled tasks; covers EC-SHIFT-13)
      - integration test `verify_fails_with_exit_2_on_missing_fsv_evidence` (delete one of the 4 evidence files; expect exit 2 + structured blocker)
      - integration test `verify_fails_with_exit_2_on_panel_materialization_over_1s` (inject 1.5 s artificial delay)
      - unit test `splitmix64_seed_is_deterministic_per_task_id` (3 invocations same seed produce same sample indices)
      - unit test `task_id_validator_rejects_unknown_task` (non-manifest `task` argument)
      - unit test `patch_path_rejects_traversal` (`--patch ../../etc/passwd` rejected)
  </in_scope>
  <out_of_scope>
    - Production of the 4 FSV evidence files (TASK-SHIFT-013 owns)
    - The `mcp-tool-roundtrip-evidence.json` file (TASK-MCP-007 owns; this task only asserts presence)
    - Real predictor weights (test trait impls / a small CPU candle stub are acceptable; FSV in CI uses the shipped predictor)
    - Real Docker oracle execution (oracle verdicts read from CF_MEJEPA_ORACLE_VERDICTS, populated by Phase 0)
    - Phase 8 holdout-split FSV (TASK-EVAL-017 owns)
    - MCP tool surface for the verify command (would require a sister phase task; this is CLI-only per doc 09 §7 wording)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/bin/mejepa.rs">
      use clap::{Parser, Subcommand};

      #[derive(Parser)]
      #[command(name = "mejepa", version, about = "ME-JEPA-Code reality compiler CLI")]
      struct Cli {
          #[command(subcommand)]
          cmd: Cmd,
      }

      #[derive(Subcommand)]
      enum Cmd {
          /// Phase 7 DoD verification command (REQ-SHIFT-15)
          Verify {
              #[arg(long)]
              task: String,
              #[arg(long, value_parser = clap::value_parser!(std::path::PathBuf))]
              patch: std::path::PathBuf,
              #[arg(long, default_value = "./data/mejepa-corpus/")]
              corpus_root: std::path::PathBuf,
              #[arg(long, default_value = "/tmp/contextgraph-mejepa-shift-subscriber-fsv/")]
              evidence_dir: std::path::PathBuf,
              #[arg(long, default_value_t = 0.05)]
              holdout_sample_pct: f32,
          },
      }

      fn main() -&gt; std::process::ExitCode {
          let cli = Cli::parse();
          match cli.cmd {
              Cmd::Verify { task, patch, corpus_root, evidence_dir, holdout_sample_pct } =&gt;
                  context_graph_mejepa::verify_cli::run_verify(
                      &amp;task, &amp;patch, &amp;corpus_root, &amp;evidence_dir, holdout_sample_pct
                  ),
          }
      }
    </signature>
    <signature file="crates/context-graph-mejepa/src/verify_cli.rs">
      use std::path::Path;
      use std::process::ExitCode;
      use serde::Serialize;

      #[derive(Debug, Serialize)]
      pub struct VerifyReport {
          pub task: String,
          pub panel_materialization_ms: u64,
          pub me_jepa_verify_verdict: String,
          pub me_jepa_predicts_match_oracle: bool,
          pub agreement_sample_size: usize,
          pub agreement_count: usize,
          pub corpus_entry_added: u32,
          pub total_corpus_size_before: u64,
          pub total_corpus_size_after: u64,
          pub fsv_evidence_dir: String,
          pub fsv_evidence_files_present: u8,
          pub exit_code: u8,
          pub blockers: Vec&lt;String&gt;,
      }

      pub fn run_verify(
          task: &amp;str,
          patch: &amp;Path,
          corpus_root: &amp;Path,
          evidence_dir: &amp;Path,
          holdout_sample_pct: f32,
      ) -&gt; ExitCode;

      fn validate_task_id(task: &amp;str) -&gt; anyhow::Result&lt;()&gt;;
      fn canonicalize_patch_path(p: &amp;Path) -&gt; anyhow::Result&lt;std::path::PathBuf&gt;;
      fn materialize_panel_for_task(task: &amp;str) -&gt; anyhow::Result&lt;(crate::Panel, u64)&gt;;
      fn sample_holdout_5pct(task_seed: &amp;str, pct: f32, holdout_dir: &amp;Path) -&gt; anyhow::Result&lt;Vec&lt;String&gt;&gt;;
      fn check_agreement(sample: &amp;[String]) -&gt; anyhow::Result&lt;(usize, Vec&lt;String&gt;)&gt;;
      fn append_corpus_entry(corpus_root: &amp;Path) -&gt; anyhow::Result&lt;(u64, u64)&gt;;
      fn assert_fsv_preconditions(evidence_dir: &amp;Path) -&gt; anyhow::Result&lt;u8&gt;;
    </signature>
  </signatures>

  <constraints>
    - Patch path canonicalized via `Path::canonicalize()` then checked against repo root prefix; reject if canonical form is outside `corpus_root`'s parent or contains `..` segments (SEC-MEJEPA-01)
    - Task-id validator reads the SWE-bench Lite manifest at `tasks/swebench-lite/*.json` (existing) and rejects any task not in the manifest with `MEJEPA_VERIFY_UNKNOWN_TASK`
    - Panel materialization budget: hard fail (exit 2) if `panel_materialization_ms &gt; 1_000` per REQ-SHIFT-15 verbatim
    - Sampling determinism: `SplitMix64::new(u64::from_le_bytes(sha256(task_id)[..8].try_into()?))` — no `rand::thread_rng()` anywhere
    - Sample size: `max(1, (holdout_count * holdout_sample_pct).round() as usize)` (avoid zero on tiny holdouts)
    - JSON output on stdout is single-line `serde_json::to_string`; tracing/log goes to stderr only (clean separation for grep)
    - Exit code matches `report.exit_code` (0 / 1 / 2 only); other codes invalid
    - Blockers list populated with SCREAMING_SNAKE-tagged entries: `MEJEPA_VERIFY_FSV_EVIDENCE_MISSING:&lt;file&gt;`, `MEJEPA_VERIFY_PANEL_LATENCY_EXCEEDED:ms=&lt;n&gt;`, `MEJEPA_VERIFY_ORACLE_DISAGREEMENT:&lt;tasks&gt;`
    - File length budget: `verify_cli.rs` &lt; 500 lines per CLAUDE.md §4; split helpers into a `verify_cli/` sub-module if needed
    - No `unwrap_or` on confidence-related fields
    - `--patch` opens RO + reads SHA-256 + records into the corpus row (no editing the candidate patch on disk)
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa --bin mejepa` succeeds with zero warnings
    - `cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./tests/fixtures/dummy_candidate.patch` exits 0 on a populated test corpus
    - The same command exits 1 with `me_jepa_predicts_match_oracle: false` when the test predictor disagrees on ≥ 1 sampled task
    - The same command exits 2 when one of the 4 FSV evidence files is missing or has wrong mode
    - `cargo test -p context-graph-mejepa verify_cli` shows ≥ 7 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - The single-line stdout JSON parses cleanly via `serde_json::from_str::&lt;VerifyReport&gt;`
    - Determinism test: 3 invocations with identical inputs produce byte-equal `agreement_sample_size` and identical `disagreed_tasks` ordering
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/verify_cli.rs:
  pub fn run_verify(task, patch, corpus_root, evidence_dir, holdout_sample_pct) -&gt; ExitCode {
      let mut blockers: Vec&lt;String&gt; = vec![];
      // step 1: validate
      if let Err(e) = validate_task_id(task) {
          eprintln!("MEJEPA_VERIFY_UNKNOWN_TASK: {e}");
          return ExitCode::from(2);
      }
      let canonical_patch = match canonicalize_patch_path(patch) {
          Ok(p) =&gt; p,
          Err(e) =&gt; { eprintln!("MEJEPA_VERIFY_PATCH_PATH_INVALID: {e}"); return ExitCode::from(2); }
      };
      // step 2: materialize panel
      let (_panel, panel_ms) = match materialize_panel_for_task(task) {
          Ok(x) =&gt; x,
          Err(e) =&gt; { eprintln!("MEJEPA_VERIFY_PANEL_MATERIALIZATION_FAIL: {e}"); return ExitCode::from(2); }
      };
      if panel_ms &gt; 1_000 {
          blockers.push(format!("MEJEPA_VERIFY_PANEL_LATENCY_EXCEEDED:ms={panel_ms}"));
      }
      // step 3: predict
      let verdict = run_compile_predict(&canonical_patch).unwrap_or_else(|e| {
          eprintln!("MEJEPA_VERIFY_PREDICT_FAIL: {e}");
          "error".into()
      });
      // step 4: sample agreement
      let sample = sample_holdout_5pct(task, holdout_sample_pct, &corpus_root.join("holdout/")).unwrap_or_default();
      let (agreement_count, disagreed) = check_agreement(&sample).unwrap_or((0, vec![]));
      let me_jepa_predicts_match_oracle = disagreed.is_empty() && !sample.is_empty();
      if !disagreed.is_empty() {
          blockers.push(format!("MEJEPA_VERIFY_ORACLE_DISAGREEMENT:{}", disagreed.join(",")));
      }
      // step 5: append corpus
      let (before, after) = append_corpus_entry(corpus_root).unwrap_or((0, 0));
      // step 6: assert FSV preconditions
      let evidence_files_present = match assert_fsv_preconditions(evidence_dir) {
          Ok(n) =&gt; n,
          Err(e) =&gt; { blockers.push(format!("MEJEPA_VERIFY_FSV_EVIDENCE_MISSING:{e}")); 0 }
      };

      let exit_code = if !blockers.is_empty() {
          if blockers.iter().any(|b| b.starts_with("MEJEPA_VERIFY_ORACLE_DISAGREEMENT")) { 1 } else { 2 }
      } else { 0 };

      let report = VerifyReport {
          task: task.into(),
          panel_materialization_ms: panel_ms,
          me_jepa_verify_verdict: verdict,
          me_jepa_predicts_match_oracle,
          agreement_sample_size: sample.len(),
          agreement_count,
          corpus_entry_added: 1,
          total_corpus_size_before: before,
          total_corpus_size_after: after,
          fsv_evidence_dir: evidence_dir.display().to_string(),
          fsv_evidence_files_present: evidence_files_present,
          exit_code,
          blockers,
      };
      println!("{}", serde_json::to_string(&report).unwrap_or_default());
      ExitCode::from(exit_code)
  }

  fn assert_fsv_preconditions(dir: &Path) -&gt; anyhow::Result&lt;u8&gt; {
      let required = ["watermark.json", "observe-evidence.json",
                      "predict-latency-evidence.json", "mcp-tool-roundtrip-evidence.json"];
      let mut count = 0u8;
      for f in required {
          let p = dir.join(f);
          let meta = std::fs::metadata(&p)?;
          #[cfg(unix)]
          {
              use std::os::unix::fs::PermissionsExt;
              if meta.permissions().mode() & 0o777 != 0o600 {
                  anyhow::bail!("evidence file {f} not 0o600");
              }
          }
          count += 1;
      }
      Ok(count)
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/bin/mejepa.rs">top-level CLI bin with `verify` subcommand</file>
  <file path="crates/context-graph-mejepa/src/verify_cli.rs">VerifyReport struct + run_verify driver + 6-step DoD body + tests</file>
  <file path="crates/context-graph-mejepa/tests/fixtures/dummy_candidate.patch">tiny no-op patch fixture for integration tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod verify_cli;`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `clap = { workspace = true, features = ["derive"] }`, `anyhow = { workspace = true }`, `tracing = { workspace = true }` to deps; add `[[bin]] name = "mejepa" path = "src/bin/mejepa.rs"`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa --bin mejepa succeeds</criterion>
  <criterion>cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./tests/fixtures/dummy_candidate.patch exits 0 on populated corpus</criterion>
  <criterion>panel_materialization_ms &lt; 1_000 in the success path</criterion>
  <criterion>me_jepa_predicts_match_oracle == true on full-agreement test fixture</criterion>
  <criterion>corpus_entry_added == 1 and total_corpus_size_after == total_corpus_size_before + 1</criterion>
  <criterion>FSV evidence preconditions: 4 files exist with mode 0o600 on Unix</criterion>
  <criterion>Exit code 1 on oracle disagreement; exit code 2 on missing evidence or latency breach</criterion>
  <criterion>Stdout last line is a single-line JSON serializing cleanly to VerifyReport</criterion>
  <criterion>Determinism: 3 identical-input invocations produce byte-equal sample size + disagreed-tasks ordering</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --bin mejepa</command>
  <command>cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./crates/context-graph-mejepa/tests/fixtures/dummy_candidate.patch</command>
  <command>cargo test -p context-graph-mejepa verify_cli</command>
  <command>cargo clippy -p context-graph-mejepa --bin mejepa -- -D warnings</command>
</test_commands>
</task_spec>
