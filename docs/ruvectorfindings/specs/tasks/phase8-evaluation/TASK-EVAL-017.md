## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**Decided scope for this task:**
1. New example binary `crates/context-graph-mejepa/examples/eval_phase8_fsv.rs` (clean-room, provenance comment per CLAUDE.md `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).`).
2. The example creates a tempdir-backed RocksDB, opens all required CFs (sister TASK-EVAL-003 declares them), constructs `EvalRunner` against synthetic-but-valid holdout (`mejepa::fixtures::synthetic_holdout`), runs end-to-end + active-learning + graph + weekly-report + ship-gate.
3. Writes 16 evidence files to `/tmp/contextgraph-mejepa-evaluation-fsv/` mode `0o600`:
   - `holdout-eval.json` — output of TASK-EVAL-004
   - `weekly-report.json` — output of TASK-EVAL-015
   - `active-learning-queue.json` — output of TASK-EVAL-009/014
   - `ship-gate.json` — output of TASK-EVAL-008
   - `patch-similarity-graph.json` — output of TASK-EVAL-010
   - `regression.json` — output of TASK-EVAL-012
   - `ood.json` — output of TASK-EVAL-006
   - `q1q2q3.json` — output of TASK-EVAL-012
   - `state-transfer.json` — output of TASK-EVAL-011 (+ Phase-7-not-shipped degradation)
   - `aux-head-distillation.json` — output of TASK-EVAL-011
   - `open-research-questions.json` — output of TASK-EVAL-001/015
   - `leakage-check.json` — output of TASK-EVAL-004 leakage path
   - `bayesian-shrinkage.json` — output of TASK-EVAL-012
   - `provenance.json` — every persisted record's provenance fields
   - `latency.json` — TASK-EVAL-016 measured wall times per phase
   - `mcp-roundtrip.json` — output of TASK-EVAL-013/014 MCP tool roundtrip
4. Top-level `manifest.json` records SHA-256 of every evidence file plus the synthetic fixture SHA, code_version, and wall-clock timestamps.
5. After writing each file, READ IT BACK and `assert!(read_back == in_memory)` byte-equal.
6. NO mock data — use real `RocksDbInferStore` against a tempdir.

**FSV for this task:** the example IS the FSV. After running, `/tmp/contextgraph-mejepa-evaluation-fsv/` contains 16 evidence files with mode 0600; `manifest.json` SHA-256 column matches a fresh `sha256sum`.

---

# TASK-EVAL-017 — FSV harness writing 16 evidence files at /tmp/contextgraph-mejepa-evaluation-fsv/

```xml
<task_spec id="TASK-EVAL-017" version="1.0">

<metadata>
  <title>Implement FSV harness in `crates/context-graph-mejepa/examples/eval_fsv.rs` writing all 16 evidence files at `/tmp/contextgraph-mejepa-evaluation-fsv/` with mode 0o600 + re-read-and-assert</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>17</sequence>
  <implements>
    <item ref="REQ-EVAL-15">Phase 8 DoD CLI command verified end-to-end</item>
    <item ref="REQ-EVAL-16">Performance budget verified (&lt; 30 min p50 wall-clock recorded in latency-evidence.json)</item>
    <item ref="REQ-EVAL-17">Determinism — byte-equal canonical bytes across 3 invocations on identical fixture</item>
    <item ref="constitution#fsv_protocol">re-read-and-assert mandatory; mode 0o600; real data only</item>
    <item ref="testing_requirements[fsv_evidence_paths]">/tmp/contextgraph-mejepa-evaluation-fsv/{holdout-eval,weekly-report,active-learning-queue,ship-gate,patch-similarity-graph,regression,ood,q1q2q3,state-transfer,aux-head-distillation,open-research-questions,leakage-check,bayesian-shrinkage,provenance,latency,mcp-roundtrip}.json</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-013</task_ref><!-- mejepa_eval_run + ship_gate_status MCP -->
    <task_ref>TASK-EVAL-014</task_ref><!-- active_learning queue + label MCP -->
    <task_ref>TASK-EVAL-016</task_ref><!-- eval-run + eval-build-graph CLI subcommands -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 created (eval_fsv.rs + fsv_fixtures.rs); 1 modified (Cargo.toml [[example]] entry)</estimated_files>
</metadata>

<context>
This task lands the **canonical FSV harness** for the EVALUATION domain — a `cargo run --release --example eval_fsv` binary that writes all 16 documented evidence files to `/tmp/contextgraph-mejepa-evaluation-fsv/`, each with mode 0o600, each re-read-and-asserted before reporting success. Per `&lt;fsv_protocol&gt;` real data only — no mock embedders, real RocksDB writes (in-memory ephemeral CFs), real fixture corpus seeded with deterministic SplitMix64. Trait collaborators (predictor, ood detector, gtau checker, e_diff embedder, corpus appender) use the test trait impls established in TASK-EVAL-004, 010, 011 (deterministic outputs from real inputs).

The 16 evidence files are listed verbatim in TECH-EVALUATION (and SPEC-EVALUATION testing_requirements) — each captures one slice of the eval pipeline:

1. `holdout-eval-evidence.json` — `EvalReport` core slice (overall_correlation, per_category, per_language, per_cell sample).
2. `weekly-report.json` — copy of the report at canonical filename for sanity check.
3. `active-learning-queue.json` — 1 enqueued (signal_value &gt; 0.6) + 1 OOD-rejected escalation + queue snapshot per TECH-EVALUATION.
4. `ship-gate-evidence.json` — passing case + 8 failing cases (one per condition) + regression case.
5. `patch-similarity-graph.json` — 3-task fixture; expected 3 edges with cosines.
6. `regression-evidence.json` — synthesized 3-pct drop on (subtle_flip, typescript) cell; expected `MEJEPA_EVAL_SHIP_GATE_REGRESSION` blocker.
7. `ood-evidence.json` — per-language OOD AUC + 4-week drift fixture.
8. `q1q2q3-evidence.json` — 1 panel passing all 3 + 1 panel failing each; rolled-up rates.
9. `state-transfer-evidence.json` — Wasserstein-1 + M(t) on synthetic holdout/live + train_certs.
10. `aux-head-distillation-evidence.json` — synthetic train_certs with 3 heads in consensus + 2 diverged.
11. `open-research-questions.json` — 8 seeded entries with mitigation_state=Active.
12. `leakage-check-evidence.json` — 1 passing case + 1 violating case (duplicate task across train+holdout) → expected `MEJEPA_EVAL_HOLDOUT_LEAKAGE`.
13. `bayesian-shrinkage-evidence.json` — synthetic 5-sample (over_engineer, ruby) cell shrunk to ruby-aggregate.
14. `provenance-evidence.json` — corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout_split_sha all populated and consistent across the report.
15. `latency-evidence.json` — wall-clock for full eval + build_patch_similarity_graph; assert &lt; 30-min budget headroom.
16. `mcp-roundtrip-evidence.json` — MCP-call EvalReport vs in-process-call EvalReport; SHA-256 of canonical bytes equal.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; testing_requirements fsv_evidence_paths)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-15..17; AC-EVAL-* + EC-EVAL-* fixture cases)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (16 evidence files listed verbatim; testing_requirements section)</file>
  <file purpose="prior_task">TASK-EVAL-013, TASK-EVAL-014 (MCP), TASK-EVAL-016 (CLI)</file>
  <file purpose="cross_domain_pattern">crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs (FSV harness pattern from Phase 0)</file>
  <file purpose="cross_domain_pattern">crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs (Phase 7 FSV harness pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-013, 014, 016 shipped: MCP tools + CLI subcommands callable</check>
  <check>Test trait impls available from prior tasks (predictor, e_diff, gtau, ood, corpus_appender)</check>
  <check>`tempfile` workspace dev-dep available for ephemeral RocksDB</check>
  <check>`anyhow` workspace dev-dep available for harness errors</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/examples/eval_fsv.rs` — main harness binary:
      ```rust
      #[tokio::main]
      async fn main() -&gt; anyhow::Result&lt;()&gt; {
          let evidence_dir = std::path::Path::new("/tmp/contextgraph-mejepa-evaluation-fsv/");
          std::fs::create_dir_all(evidence_dir)?;
          let fixture = build_fsv_fixture()?;
          let runner = build_eval_runner_with_test_traits(&fixture).await?;

          // 16 writers (each writes one evidence file with mode 0o600)
          write_holdout_eval_evidence(evidence_dir, &runner, &fixture)?;
          write_weekly_report_evidence(evidence_dir, &runner, &fixture)?;
          write_active_learning_queue_evidence(evidence_dir, &runner, &fixture)?;
          write_ship_gate_evidence(evidence_dir, &runner, &fixture)?;
          write_patch_similarity_graph_evidence(evidence_dir, &runner, &fixture)?;
          write_regression_evidence(evidence_dir, &runner, &fixture)?;
          write_ood_evidence(evidence_dir, &runner, &fixture)?;
          write_q1q2q3_evidence(evidence_dir, &runner, &fixture)?;
          write_state_transfer_evidence(evidence_dir, &runner, &fixture)?;
          write_aux_head_distillation_evidence(evidence_dir, &runner, &fixture)?;
          write_open_research_questions_evidence(evidence_dir, &runner, &fixture)?;
          write_leakage_check_evidence(evidence_dir, &runner, &fixture)?;
          write_bayesian_shrinkage_evidence(evidence_dir, &runner, &fixture)?;
          write_provenance_evidence(evidence_dir, &runner, &fixture)?;
          write_latency_evidence(evidence_dir, &runner, &fixture)?;
          write_mcp_roundtrip_evidence(evidence_dir, &runner, &fixture).await?;

          assert_re_read_invariants(evidence_dir)?;
          println!("FSV-OK: evaluation");
          Ok(())
      }
      ```
    - Create `crates/context-graph-mejepa/examples/fsv_fixtures.rs` — `FsvFixture` builder + helpers:
      - `pub struct FsvFixture { rocksdb: tempfile::TempDir, holdout: HoldoutDataset, calibration: CalibrationDataset, corpus: Corpus, predictor: Arc<TestPredictor>, ... }`
      - `pub fn build_fsv_fixture() -&gt; anyhow::Result&lt;FsvFixture&gt;` — synthesizes:
        - 240 holdout panels (30 per language × 8 languages of the 11; 3 unsupported intentionally short to test EmptyHoldoutLanguage path)
        - 60 calibration panels
        - 100 live predictions (≥ 100 → state_transfer enabled)
        - 1000 train_certs with synthetic ΔΩ × ΔΞ
        - 3-task corpus for graph build
        - 2-week prior report fixture for regression detection
        - Synthetic OOD panels per language (≥ 50 each)
        - SplitMix64 seeded; identical fixture across runs
    - Each `write_*_evidence` helper:
      1. Constructs the slice (e.g., calls `runner.run_holdout_eval(7, true)`, captures specific fields)
      2. Serializes via `serde_json::to_vec_pretty` with sorted keys (BTreeMap-backed)
      3. Writes to `evidence_dir.join(filename)` with mode `0o600` via `std::os::unix::fs::OpenOptionsExt::mode(0o600)` (cfg(unix); fallback warning on non-Unix)
      4. Returns `Result<(), anyhow::Error>`
    - `fn assert_re_read_invariants(dir: &Path) -&gt; anyhow::Result<()>`:
      - For each of the 16 filenames:
        1. Assert file exists
        2. Assert mode bits `& 0o777 == 0o600` (Unix only)
        3. Assert file is parseable as JSON (deserialize roundtrip)
        4. Per-file invariants:
           - holdout-eval-evidence: `overall_correlation` finite + clamped [-1, 1]
           - weekly-report: filename matches `mejepa-weekly-report-YYYY-MM-DD.json` pattern
           - active-learning-queue: at least 1 entry with signal_value ≥ 0.6; at least 1 escalation
           - ship-gate-evidence: includes both passing AND failing case payloads
           - patch-similarity-graph: edge_count ≥ 1; cosines ≥ 0.7
           - regression: contains `MEJEPA_EVAL_SHIP_GATE_REGRESSION` blocker
           - ood: per-language AUC entries finite [0, 1]
           - q1q2q3: q1 + q2 + q3 all finite [0, 1]
           - state-transfer: T finite [0, 1]; m_t_estimate finite [0, 1]
           - aux-head-distillation: 3 in consensus + 2 diverged
           - open-research-questions: exactly 8 entries Q1..Q8
           - leakage-check: contains `MEJEPA_EVAL_HOLDOUT_LEAKAGE` failure case
           - bayesian-shrinkage: contains `(over_engineer, ruby)` shrunk note
           - provenance: all 6 SHA/version fields present and non-empty
           - latency: wall_clock_seconds finite ≤ 1800.0 (30-min budget)
           - mcp-roundtrip: in_process_hash == mcp_hash (SHA-256 byte-equal)
      - Failure of any assertion exits non-zero
    - Add **determinism test**: 3 invocations of the harness produce byte-equal `holdout-eval-evidence.json` (modulo `report_date` and `wall_clock_seconds` fields stripped)
    - Add `[[example]]` entries to `crates/context-graph-mejepa/Cargo.toml`:
      ```toml
      [[example]]
      name = "eval_fsv"
      path = "examples/eval_fsv.rs"

      [[example]]
      name = "fsv_fixtures"  # not strictly an example, but co-located helper
      path = "examples/fsv_fixtures.rs"
      ```
      Actually `fsv_fixtures.rs` is included as a `mod` from `eval_fsv.rs` (no separate example bin), per the SHIFT-013 pattern.
  </in_scope>
  <out_of_scope>
    - Production weights — test trait impls are deterministic stand-ins per TECH-EVALUATION testing pattern
    - Real Docker oracle — fixture pre-populates CF_MEJEPA_ORACLE_VERDICTS
    - Real Phase 7 shift-subscriber — fixture pre-populates CF_MEJEPA_LIVE_PREDICTIONS with 100 entries
    - Real candle-cuda — test E_Diff encoder produces deterministic embeddings via SHA-256-based PRG
    - The `verify` CLI from TASK-SHIFT-014 (Phase 7 FSV — separate harness)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/examples/eval_fsv.rs">
use std::path::Path;

#[tokio::main]
async fn main() -&gt; anyhow::Result&lt;()&gt;;

fn build_fsv_fixture() -&gt; anyhow::Result&lt;FsvFixture&gt;;
async fn build_eval_runner_with_test_traits(fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;EvalRunner&gt;;

fn write_holdout_eval_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_weekly_report_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_active_learning_queue_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_ship_gate_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_patch_similarity_graph_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_regression_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_ood_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_q1q2q3_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_state_transfer_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_aux_head_distillation_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_open_research_questions_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_leakage_check_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_bayesian_shrinkage_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_provenance_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
fn write_latency_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
async fn write_mcp_roundtrip_evidence(dir: &amp;Path, runner: &amp;EvalRunner, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;

fn assert_re_read_invariants(dir: &amp;Path) -&gt; anyhow::Result&lt;()&gt;;
    </signature>
  </signatures>
  <constraints>
    - All 16 files written with mode 0o600 (Unix); cfg-gated; non-Unix prints stderr warning per SEC-MEJEPA-07
    - Re-read-and-assert is mandatory per `&lt;fsv_protocol&gt;`; binary exits non-zero on any assertion failure
    - JSON serialization deterministic via `serde_json::to_vec_pretty` with BTreeMap-backed maps + sorted keys
    - Synthetic ULIDs use a counted SplitMix64 PRNG so the fixture is byte-stable across runs
    - Determinism test: 3 invocations produce byte-equal `holdout-eval-evidence.json` (modulo wall-clock-stripped fields)
    - Real RocksDB writes via `tempfile::TempDir` (NOT in-memory mock) — exercises the real CF persistence path
    - File length budget: `eval_fsv.rs` &lt; 500 lines; split helpers into `fsv_fixtures.rs` if needed
    - The 16th file `mcp-roundtrip-evidence.json` requires bringing up an in-process MCP server stub OR mocking the JSON-RPC layer; for v1 we test in-process EvalRunner vs MCP-handler-call-as-function-call equivalence (NOT a separate server process); both produce a serialized EvalReport whose `hash_for_determinism()` MUST be byte-equal
    - Total wall-clock for the FSV harness MUST be &lt; 5 min on RTX 5090 (so it can run on every release-build CI)
  </constraints>
  <verification>
    - `cargo run --release --example eval_fsv -p context-graph-mejepa` exits 0
    - All 16 evidence files present at `/tmp/contextgraph-mejepa-evaluation-fsv/`
    - Each file has mode 0o600 on Unix
    - Each file deserializes cleanly as JSON
    - Each file passes its documented per-file invariant check
    - 3 invocations produce byte-equal stripped output (determinism)
    - Total wall-clock &lt; 5 min on RTX 5090 hardware
    - Stdout last line == `FSV-OK: evaluation`
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/examples/eval_fsv.rs:
  #[tokio::main]
  async fn main() -&gt; anyhow::Result&lt;()&gt; {
      let evidence_dir = std::path::Path::new("/tmp/contextgraph-mejepa-evaluation-fsv/");
      std::fs::create_dir_all(evidence_dir)?;
      let fixture = build_fsv_fixture()?;
      let runner = build_eval_runner_with_test_traits(&fixture).await?;

      let writers: Vec&lt;(&amp;str, Box&lt;dyn FnOnce(&amp;Path, &amp;EvalRunner, &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;&gt;)&gt; = vec![
          ("holdout-eval-evidence.json", Box::new(write_holdout_eval_evidence)),
          ("weekly-report.json",         Box::new(write_weekly_report_evidence)),
          // ... 14 more ...
      ];
      for (name, w) in writers { w(evidence_dir, &runner, &fixture)?; }

      // mcp-roundtrip is async
      write_mcp_roundtrip_evidence(evidence_dir, &runner, &fixture).await?;

      assert_re_read_invariants(evidence_dir)?;
      println!("FSV-OK: evaluation");
      Ok(())
  }

  fn write_with_mode_0o600(path: &Path, bytes: &[u8]) -&gt; anyhow::Result&lt;()&gt; {
      use std::fs::OpenOptions;
      let mut opts = OpenOptions::new();
      opts.create(true).write(true).truncate(true);
      #[cfg(unix)]
      {
          use std::os::unix::fs::OpenOptionsExt;
          opts.mode(0o600);
      }
      let mut file = opts.open(path)?;
      use std::io::Write;
      file.write_all(bytes)?;
      Ok(())
  }

  fn assert_re_read_invariants(dir: &Path) -&gt; anyhow::Result&lt;()&gt; {
      let required = [
          "holdout-eval-evidence.json", "weekly-report.json",
          "active-learning-queue.json", "ship-gate-evidence.json",
          "patch-similarity-graph.json", "regression-evidence.json",
          "ood-evidence.json", "q1q2q3-evidence.json",
          "state-transfer-evidence.json", "aux-head-distillation-evidence.json",
          "open-research-questions.json", "leakage-check-evidence.json",
          "bayesian-shrinkage-evidence.json", "provenance-evidence.json",
          "latency-evidence.json", "mcp-roundtrip-evidence.json",
      ];
      for f in required {
          let path = dir.join(f);
          let meta = std::fs::metadata(&path)?;
          #[cfg(unix)]
          {
              use std::os::unix::fs::PermissionsExt;
              anyhow::ensure!(meta.permissions().mode() & 0o777 == 0o600,
                  "evidence file {f} not 0o600");
          }
          let bytes = std::fs::read(&path)?;
          let v: serde_json::Value = serde_json::from_slice(&bytes)?;
          // per-file invariants
          match f {
              "holdout-eval-evidence.json" =&gt; {
                  let oc = v.get("overall_correlation").and_then(|x| x.as_f64()).unwrap_or(f64::NAN);
                  anyhow::ensure!(oc.is_finite() && (-1.0..=1.0).contains(&oc), "overall_correlation invalid");
              }
              "open-research-questions.json" =&gt; {
                  let arr = v.as_array().ok_or_else(|| anyhow::anyhow!("not array"))?;
                  anyhow::ensure!(arr.len() == 8, "expected exactly 8 research questions");
              }
              "regression-evidence.json" =&gt; {
                  let s = v.to_string();
                  anyhow::ensure!(s.contains("MEJEPA_EVAL_SHIP_GATE_REGRESSION"), "missing regression blocker");
              }
              "leakage-check-evidence.json" =&gt; {
                  let s = v.to_string();
                  anyhow::ensure!(s.contains("MEJEPA_EVAL_HOLDOUT_LEAKAGE"), "missing leakage failure case");
              }
              "mcp-roundtrip-evidence.json" =&gt; {
                  let in_proc = v.get("in_process_hash").and_then(|x| x.as_str()).unwrap_or("");
                  let mcp = v.get("mcp_hash").and_then(|x| x.as_str()).unwrap_or("");
                  anyhow::ensure!(!in_proc.is_empty() && in_proc == mcp, "mcp roundtrip hash mismatch");
              }
              "latency-evidence.json" =&gt; {
                  let secs = v.get("wall_clock_seconds").and_then(|x| x.as_f64()).unwrap_or(f64::NAN);
                  anyhow::ensure!(secs.is_finite() && secs &lt;= 1800.0, "wall_clock exceeds 30-min budget");
              }
              _ =&gt; {}
          }
      }
      Ok(())
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/examples/eval_fsv.rs">FSV harness binary writing 16 evidence files + assert_re_read_invariants</file>
  <file path="crates/context-graph-mejepa/examples/fsv_fixtures.rs">FsvFixture builder + 240-panel holdout + 100-live + train_certs + 3-task corpus + 2-week prior report + helpers (mod-included from eval_fsv.rs)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `[[example]] name = "eval_fsv" path = "examples/eval_fsv.rs"`; add `anyhow`, `tempfile` to [dev-dependencies] if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo run --release --example eval_fsv -p context-graph-mejepa exits 0</criterion>
  <criterion>16 evidence files present at /tmp/contextgraph-mejepa-evaluation-fsv/</criterion>
  <criterion>Each file mode 0o600 on Unix</criterion>
  <criterion>Each file deserializes cleanly as JSON</criterion>
  <criterion>holdout-eval-evidence.overall_correlation finite [-1, 1]</criterion>
  <criterion>open-research-questions.json has exactly 8 entries</criterion>
  <criterion>regression-evidence.json contains MEJEPA_EVAL_SHIP_GATE_REGRESSION</criterion>
  <criterion>leakage-check-evidence.json contains MEJEPA_EVAL_HOLDOUT_LEAKAGE failure case</criterion>
  <criterion>ship-gate-evidence.json includes both passing AND failing case payloads</criterion>
  <criterion>state-transfer-evidence.json T and m_t_estimate finite [0, 1]</criterion>
  <criterion>aux-head-distillation-evidence.json has 3 in_consensus + 2 diverged</criterion>
  <criterion>provenance-evidence.json has all 6 SHA/version fields populated</criterion>
  <criterion>latency-evidence.json wall_clock_seconds &lt; 1800</criterion>
  <criterion>mcp-roundtrip in-process and MCP hashes byte-equal</criterion>
  <criterion>3 invocations produce byte-equal stripped output (determinism)</criterion>
  <criterion>Total wall-clock &lt; 5 min on RTX 5090</criterion>
  <criterion>Stdout last line == FSV-OK: evaluation</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release --example eval_fsv -p context-graph-mejepa</command>
  <command>cargo run --release --example eval_fsv -p context-graph-mejepa</command>
  <command>ls -la /tmp/contextgraph-mejepa-evaluation-fsv/</command>
  <command>for f in /tmp/contextgraph-mejepa-evaluation-fsv/*.json; do stat -c '%a %n' "$f"; done</command>
  <command>cargo clippy --release --example eval_fsv -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
