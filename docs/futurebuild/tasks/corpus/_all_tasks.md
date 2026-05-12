# Corpus domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

## TASK-CORPUS-001 — Per-language mutation operators

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mejepa-corpus/src/generic_language_ops.rs` and dispatched via `apply_mutation_for_language` for Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP. Source-of-truth FSV is the generated non-Python corpus at `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/`, where every operator/language row is validated by the corresponding real toolchain report.

- **Phase:** C · **Effort:** 4-5d · **Blocked by:** none · **Spec:** existing SPEC-CORPUS-functional + 04_DDA_TCT_CODE_ENRICHMENT.md §7
- **Goal:** Port the 8 Python mutation categories (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) to Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP.
- **Outputs:** generic non-Python mutation dispatch plus parse-preserving candidate validation; regression tests for the real failures discovered during corpus generation; per-row toolchain reports under each generated fixture corpus.
- **DoD:** for each (language, operator) pair, generate mutations against fixture programs; verify syntax/toolchain check produces expected pass/fail per category; fail closed on mismatch; read back the mutation, verdict, report JSON, and corpus index from source of truth.
- **FSV:** `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/verify-evidence.json`, `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/stats.json`, and read-only RocksDB inspection of `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/oracle-store`.
- **Boundary:** invalid syntax and operator no-site cases fail closed; output outside `/mnt/d/contextgraph/corpus/` fails before writing; `--tasks-per-language 0` fails before writing.

## TASK-CORPUS-002 — 300×8 SWE-bench Lite Python corpus generation

**Status:** complete (2026-05-12). Source of truth: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json` with `passes=true`, `entry_count=2400`, `full_phase0_complete=true`, `mutation_cf_count=2400`, `verdict_cf_count=2400`, `provenance_cf_count=1`, `oracle_sample_count=20`, and `failures=[]`.

- **Phase:** C · **Effort:** 2-3d ops · **Blocked by:** none
- **Goal:** Run the full Phase 0 corpus generator against the 300 SWE-bench Lite Python tasks × 8 mutation categories = 2,400 corpus entries. Each entry: `(task_id, mutation_kind, mutated_source, oracle_verdict)` persisted to `CF_MEJEPA_MUTATION_CORPUS`.
- **Outputs:** populated D-backed corpus directory `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-<date>/`; `oracle-store/` RocksDB CFs; `index.json`; `stats.json`; `verify-evidence.json`; per-instance per-category Docker eval artifacts.
- **DoD:** all 2,400 entries persisted with valid `OracleVerdict` (FAIL_TO_PASS, PASS_TO_FAIL, etc.); `assert_no_test_patch_leakage` passes; `oracle-store/` readback shows 2,400 mutation rows, 2,400 verdict rows, and 1 provenance row; corpus paths outside `/mnt/d/contextgraph/corpus/` fail before writing.
- **FSV:** `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-<date>/verify-evidence.json` plus an independent RocksDB inspector readback saved beside it.
- **Boundary:** Docker image/evaluator failure is a hard failure with actionable error logging. Do not mark the full corpus complete with skipped rows.

## TASK-CORPUS-003 — Per-language fixture corpora (~50 tasks per language)

**Status:** complete (2026-05-12). Source of truth: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/verify-evidence.json` with `passes=true`, `entry_count=5120`, `mutation_cf_count=5120`, `verdict_cf_count=5120`, `provenance_cf_count=1`, and `toolchain_report_count=5120`.

- **Phase:** C · **Effort:** 1-2d · **Blocked by:** TASK-CORPUS-001
- **Goal:** For each non-Python language (10 langs), generate deterministic fixture tasks and run all 8 mutation categories through TASK-CORPUS-001's per-language operators.
- **Outputs:** `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` with `fixture-manifest.json`, `index.json`, `stats.json`, `verify-evidence.json`, `mutations/`, `toolchain-reports/`, `toolchain-work/`, and `oracle-store/`.
- **DoD:** each language has ≥500 corpus entries with verified real-toolchain outcomes. Final state has 512 entries per language and 64 entries per `(language × category)` cell.
- **FSV:** independent readback confirmed 5,120 mutation files, 5,120 toolchain report JSON files, 5,120 mutation CF rows, 5,120 verdict CF rows, and 1 provenance CF row.

## TASK-CORPUS-004 — Stoer-Wagner mincut split applied to new corpora

**Status:** complete (2026-05-12). Source of truth: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json` with `passes=true`, Python entry count 2,400, non-Python entry count 5,120, combined task count 940, bucket sizes `[752, 94, 94]`, and leakage check passed.

- **Phase:** C · **Effort:** 0.25d · **Blocked by:** TASK-CORPUS-002, TASK-CORPUS-003
- **Goal:** Apply the corpus split function to the new combined corpus (Python + 10 other languages) producing train / calibration / holdout per `docs/ruvectorfindings/specs/_README.md` Phase 0.
- **Outputs:** split assignments persisted in `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`; `assert_no_test_patch_leakage` validation report included in the evidence.
- **DoD:** clean split with zero cross-cut leakage; ratios ~80/10/10 over 940 tasks.
- **FSV:** `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`
