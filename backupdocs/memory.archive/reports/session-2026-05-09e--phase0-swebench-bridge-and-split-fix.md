---
namespace: reports
agent_id: claude-code
created: 2026-05-09T02:50:00Z
updated: 2026-05-09T02:50:00Z
status: active
tags: report, session, mejepa, phase-0, swebench-oracle-bridge, split-fix, fsv
---

# Session Report: 2026-05-09e — Phase 0 SWE-bench oracle bridge + split fix + E2E FSV

## Task
Close the three Phase 0 / Phase 1 gaps named in the prior turn's report:
1. Phase 0 still needs the official SWE-bench Docker evaluator/batch run.
2. Phase 1 still needs the remaining deferred instruments.
3. Split is `[195, 77, 28]` instead of clean 80/10/10.

## Outcome: partial — gaps #1 (bridge) and #3 (split) closed; gap #2 deferred to next turn

The SWE-bench harness oracle bridge ships, the corpus split fix ships,
and both have end-to-end FSV against real Docker. The 8-mutation
corpus generator + CLI + Phase 1 instrument extension defer to next
turn so this turn delivers a tight, well-tested vertical instead of a
half-finished horizontal sweep.

## What shipped

### 1. Phase 0 SWE-bench Lite harness oracle bridge — `crates/context-graph-mejepa-corpus/src/swebench.rs` (NEW, 1306 lines incl. 18 unit tests)

- `SwebenchPrediction` / `SwebenchPredictionMode { Gold, Custom(...) }` — canonical predictions JSONL row.
- `SwebenchOracleConfig` with `defaults_for(...)` (defaults: `princeton-nlp/SWE-bench_Lite` / `test` / `swebench` / 1800s instance / 3600s overall).
- `run_swebench_lite_oracle(config) -> SwebenchOracleResult` — invokes `python -m swebench.harness.run_evaluation` for ONE instance, kills on overall_timeout, fails closed on harness exit ≠ 0.
- `parse_swebench_instance_report(instance_id, text) -> SwebenchInstanceReport` — fail-closed on multi-key root, instance_id mismatch, missing required bools, malformed `tests_status`.
- `swebench_report_to_verdict(report, exception_hint) -> OracleVerdict` — converts FAIL_TO_PASS/PASS_TO_PASS/FAIL_TO_FAIL/PASS_TO_FAIL `success`/`failure` lists to canonical `Vec<PerTestOutcome>` consumed by `EOracleInstrument`. Mapping verified against `swebench/harness/grading.py::get_eval_tests_report` semantics.
- `extract_exception_class(text)` — word-boundary scan of `test_output.txt` for the dominant Python exception class (precedence-ordered: AssertionError → TypeError → ValueError → KeyError → IndexError → AttributeError → ImportError → NameError → RuntimeError).
- Docker-name-safe `run_id` validator — rejects `+`, spaces, path separators up front. (Caught by FSV the first time the run failed: `docker.errors.APIError: 400` because my run_id used `epoch+1234s` and Docker's container-name regex is `[a-zA-Z0-9][a-zA-Z0-9_.-]+`.)

### 2. End-to-end FSV — `crates/context-graph-mejepa-corpus/examples/swebench_oracle_fsv.rs` (NEW, 380 lines)

Trigger: real `python -m swebench.harness.run_evaluation` against the local `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` Docker image (3.75 GB).

Source-of-truth files written:
- `<run_root>/gold.<run_id>.json` (overall report)
- `<run_root>/logs/run_evaluation/<run_id>/{gold|model}/sympy__sympy-20590/{report.json, run_instance.log, test_output.txt, patch.diff}`
- `<run_root>/oracle-store/{*.sst, CURRENT, IDENTITY, LOCK, LOG}` — physical RocksDB
- `/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`

Verified outcomes:
- **Gold mode**: harness duration 23704 ms; instance_report.resolved=true; tests_status.FAIL_TO_PASS.success=[`test_immutable`]; PASS_TO_PASS.success=21 tests; verdict.per_test=22 rows all Pass; verdict.exception=None; RocksDB put → flush → reopen → get returns identical OracleVerdict.
- **Custom mode** (using gold patch as model_patch): harness duration 23041 ms; resolved=true; predictions JSONL written and read back as the file SoT under `<run_root>/predictions.jsonl`; same 22 per-test rows all Pass.
- Reparse-then-reconvert determinism: parsing the on-disk report.json a second time and re-running `swebench_report_to_verdict` yields identical OracleVerdict bytes.

### 3. Split fix — `crates/context-graph-mejepa-corpus/src/split.rs`

New `SplitMode { InstanceAtomic, RepoAtomic }` enum.

- `InstanceAtomic` (NEW, default) — per-task placement via seed-driven shuffle + lowest-current/target-ratio bin-pack. Achieves clean **240/30/30** on SWE-bench Lite (300 tasks). Leakage-free at the test_patch level because every Lite instance has a unique test_patch SHA.
- `RepoAtomic` — original 2026-05-08 design, every task in a repo lands in the same bucket. Structurally bounded to ~`[195, 77, 28]` on Lite because django=38% / sympy=25.7% (no whole-repo subset sums to 240/30/30). Locked into a regression test.
- New `assert_no_test_patch_leakage(assignments, sha_for)` — explicit second-line check that no two assignments share a test_patch SHA. Use this with the manifest's `official_test_patch_sha256` for the InstanceAtomic invariant.

Two new tests lock the dataset-shape constraint into the suite:
- `instance_atomic_achieves_clean_80_10_10_on_swe_bench_lite_shape` — synthesises Lite's 12-repo distribution, asserts `[240, 30, 30]`.
- `repo_atomic_locks_in_dataset_shape_constraint_on_swe_bench_lite` — same shape, asserts `train < 240` and total = 300.

`split_fsv.rs` and `mejepa_corpus_fsv.rs` updated to test BOTH modes; `split-evidence.json` now records `repo_atomic.bucket_sizes=[195,77,28]` and `instance_atomic.bucket_sizes=[240,30,30]` side-by-side.

### 4. `tasks/swebench-lite/` source-of-truth restoration

Restored `scripts/swebench_lite_ops.py` from HEAD (deleted at session start; 28592 bytes, untracked but committed in `f2ebf36c`). Confirmed `preflight` passes: 300 dataset rows ⇄ 300 manifests, `swebench_version=4.1.0`, `datasets_version=4.8.5`, `docker_py_version=7.1.0`, Docker reachable.

## Files Created or Modified in the Repo

- `crates/context-graph-mejepa-corpus/src/swebench.rs` — NEW (1306 lines including 18 unit tests).
- `crates/context-graph-mejepa-corpus/src/lib.rs` — wire `pub mod swebench`.
- `crates/context-graph-mejepa-corpus/src/oracle.rs` — make error constructors + `wait_with_timeout` + `tail_text` `pub(crate)`.
- `crates/context-graph-mejepa-corpus/src/split.rs` — add `SplitMode`, `split_corpus_instance_atomic`, `assert_no_test_patch_leakage`; refactor `split_corpus` dispatch.
- `crates/context-graph-mejepa-corpus/src/split_tests.rs` — pin `RepoAtomic` on existing tests; add 4 new tests.
- `crates/context-graph-mejepa-corpus/examples/swebench_oracle_fsv.rs` — NEW E2E FSV (380 lines).
- `crates/context-graph-mejepa-corpus/examples/split_fsv.rs` — verify both modes; rename single_repo edge case for clarity.
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs` — pin RepoAtomic on existing assertions.
- `scripts/swebench_lite_ops.py` — restored from HEAD.
- `docs/ruvectorfindings/sitrep/01_status_overview.md` — bumped test totals 229→251; new oracle bridge row; updated split row + FSV evidence list.

## Files Written or Updated in `./memory/`

- `reports/session-2026-05-09e--phase0-swebench-bridge-and-split-fix.md` — this report.

## Decisions Made

- **InstanceAtomic becomes the default split mode** (post-2026-05-09e). Rationale: ME-JEPA needs balanced train/cal/holdout for stable training; instance-level test_patch atomicity is the load-bearing leakage invariant; repo-atomic is over-conservative on Lite's skewed distribution. RepoAtomic stays available behind a flag for callers who explicitly need cross-task generalization separation. **Lock**: do not silently flip the default back without surfacing in `./memory/decisions/`.
- **Oracle bridge invokes the official harness via subprocess, not via Python C-API.** Rationale: the harness has heavy Python-side state (datasets caching, Docker SDK, log parsers per repo); a clean subprocess boundary keeps the Rust crate from inheriting all that. The cost is 100-200ms of Python startup per instance, which is dwarfed by 20+ seconds of Docker test execution. **Lock**: keep the bridge subprocess-based.
- **Predictions JSONL is one prediction per file**, written into `<run_root>/predictions.jsonl`. The harness can take many predictions in one JSONL, but for ME-JEPA's per-(task, mutation) corpus generation each invocation runs ONE prediction with `--max_workers 1`. This keeps RocksDB writes cleanly serialized per (task, mutation_category) key. **Lock**: do not switch to multi-prediction JSONL without considering the OracleStore key invariant.

## Discoveries

- **Docker container-name regex bites SWE-bench harness run_id**: the harness embeds `run_id` directly into container names. Docker's regex is `[a-zA-Z0-9][a-zA-Z0-9_.-]+`. Symptoms: `docker.errors.APIError: 400 Client Error ... Bad Request ("Invalid container name ...")` with no clear surface from the harness's error path. Fix: validate up front in Rust. Recorded in `memory/discoveries/swebench_run_id_must_be_docker_safe.md` (this turn).
- **SWE-bench Lite distribution is mathematically incompatible with repo-atomic 80/10/10**: django=114 (38%), sympy=77 (25.7%), top 2 repos = 63.7%. No whole-repo subset sums to 240/30/30; the LPT bin-pack achieves the closest-attainable [195, 77, 28]. The right fix is to relax the atomicity unit, not to hand-pick fractions.
- **The harness writes the per-instance `report.json` under `logs/run_evaluation/<run_id>/<model_name_or_path>/<instance_id>/report.json`**, NOT under `<run_root>/<instance_id>/report.json`. The artifact-namespace dir is `gold` for `--predictions_path gold` and the prediction's `model_name_or_path` field for custom predictions.
- **`get_eval_tests_report` always emits all four buckets** (FAIL_TO_PASS/PASS_TO_PASS/FAIL_TO_FAIL/PASS_TO_FAIL) but F2F and P2F are empty when `calculate_to_fail=False` (the harness default). The Rust parser treats all four as required keys with possibly-empty arrays.
- **The "success"/"failure" labels inside transition buckets refer to whether the test ended in the EXPECTED state**, not whether it passed. F2F.failure means a test that was failing on base now passes on patched (extra credit). The verdict converter maps to final-state Pass/Fail accordingly.

## Failures (with the lesson)

- **First gold-mode FSV invocation produced `report.json missing` despite the harness running for 19 seconds.** Root cause: my `iso_now()` returned `epoch+1778312175s` and Docker rejected the container name with HTTP 400. Lesson: always validate run_id against Docker's container-name regex BEFORE invoking the harness. Fix shipped: `validate_config` rejects non-`[a-zA-Z0-9][a-zA-Z0-9_.-]+` run_ids; `iso_now()` uses `epoch.{secs}` instead.
- **`scripts/swebench_lite_ops.py` was deleted from the working tree** between session start (where SessionStart status said `?? scripts/swebench_lite_ops.py` untracked) and the first attempt to run it. Restored from `git restore`. Lesson: don't trust the session-start status snapshot for "is this file present"; use `git ls-tree HEAD <path>` + `ls -la <path>` to check both committed and on-disk state.

## Successes

- **Gold mode E2E**: 22/22 per-test rows Pass, resolved=true, RocksDB round-trip identical. ~24 seconds wall-clock against real Docker.
- **Custom mode E2E**: predictions JSONL written, read back, harness consumed it, produced same outcome as gold. Proves the JSONL writer + path computation is correct.
- **InstanceAtomic split**: synthesized Lite distribution, deterministic seed, exactly `[240, 30, 30]` produced.
- **All 4 mejepa-corpus FSVs still pass**: split, oracle, oracle_store, mejepa_corpus.
- **81/81 mejepa-corpus tests + 40/40 instruments tests + clippy clean + fmt clean.**

## Open Blockers

None.

## Tests / Build

- Tests added: 22 in mejepa-corpus (18 swebench + 4 split). Total now 81.
- Tests passing: yes — 81/81 mejepa-corpus, 40/40 instruments, all FSVs.
- Build status: passing (`cargo build --release` for the touched crates).
- Clippy: clean with `-D warnings`.
- Fmt: clean with `--check`.

## Resume Here Next Session

1. **Phase 0 corpus generator + CLI** (`mejepa-corpus generate/verify`) — the bridge is in place. Build the diff-aware mutation pipeline (apply 8 mutation operators to gold patches loaded from the HF dataset) and wire it into a CLI. ~2-3 days.
2. **Phase 1 instruments** (E_Test, E_Problem, E_CommitMsg) — the lowest-risk Phase 1 deliverables that reuse E1. ~1-2 days each.
3. **Multi-language ast_chunker extension** — Python first since SWE-bench Lite is Python-only.

## Confidence: high
The SWE-bench oracle bridge has been verified end-to-end against real Docker with FSV evidence on disk (`/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`). The split fix has explicit regression tests. The deferred work is well-scoped.
