# Corpus domain — all atomic tasks

## TASK-CORPUS-001 — Per-language mutation operators

- **Phase:** C · **Effort:** 4-5d · **Blocked by:** none · **Spec:** existing SPEC-CORPUS-functional + 04_DDA_TCT_CODE_ENRICHMENT.md §7
- **Goal:** Port the 8 Python mutation operators (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) to Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP. Each per-language op uses tree-sitter for AST-aware mutations and the corresponding compiler for syntax verification.
- **Outputs:** `crates/context-graph-mejepa-corpus/src/<language>/{known_good,subtle_flip,off_by_one,swap_variable,delete_test_call,wrong_file,over_engineer,compile_error}.rs` × 10 languages.
- **DoD:** for each (language, operator) pair, generate 50 mutations against fixture programs; verify syntax check produces expected pass/fail per category; round-trip evidence at `/mnt/d/contextgraph/fsv/corpus-fsv/<language>/<operator>.json`.
- **FSV:** per-(language, operator) FSV files (80 total).
- **Boundary:** language with no boolean operators (e.g., comments-only file) for SubtleFlip → `MEJEPA_CORPUS_OPERATOR_INAPPLICABLE` (existing pattern).

## TASK-CORPUS-002 — 300×8 SWE-bench Lite Python corpus generation

- **Phase:** C · **Effort:** 2-3d ops · **Blocked by:** none
- **Goal:** Run the full Phase 0 corpus generator against the 300 SWE-bench Lite Python tasks × 8 mutation categories = 2,400 corpus entries. Each entry: `(task_id, mutation_kind, mutated_source, oracle_verdict)` persisted to `CF_MEJEPA_MUTATION_CORPUS`.
- **Outputs:** populated CFs; per-instance per-category Docker eval results; report at `/mnt/d/contextgraph/exports/corpus/300x8.json`.
- **DoD:** all 2,400 entries persisted with valid `OracleVerdict` (FAIL_TO_PASS, PASS_TO_FAIL, etc.); `assert_no_test_patch_leakage` passes; runtime ~16h wall (parallel across cores).
- **FSV:** `/mnt/d/contextgraph/fsv/corpus-fsv/300x8_summary.json`
- **Boundary:** Docker pull failure for an instance → retry × 3, then mark instance as `MEJEPA_CORPUS_DOCKER_UNAVAILABLE` and skip; corpus generation continues; partial coverage acceptable.

## TASK-CORPUS-003 — Per-language fixture corpora (~50 tasks per language)

- **Phase:** C · **Effort:** 1-2d · **Blocked by:** TASK-CORPUS-001
- **Goal:** For each non-Python language (10 langs), curate ~50 fixture tasks (small synthetic + scraped public CI logs) and generate corpora using TASK-CORPUS-001's per-language operators.
- **Outputs:** `crates/context-graph-mejepa-corpus/fixtures/<language>/*.json` (~50 per language); populated CFs.
- **DoD:** each language has ≥ 30 corpus entries with verified oracle outcomes; per-(language × category) cell counts ≥ 5.

## TASK-CORPUS-004 — Stoer-Wagner mincut split applied to new corpora

- **Phase:** C · **Effort:** 0.25d · **Blocked by:** TASK-CORPUS-002, TASK-CORPUS-003
- **Goal:** Apply the existing `context-graph-mincut` split function to the new combined corpus (Python + 10 other languages) producing train / calibration / holdout per `docs/ruvectorfindings/specs/_README.md` Phase 0.
- **Outputs:** split assignments persisted; `assert_no_test_patch_leakage` validation report.
- **DoD:** clean split with zero cross-cut leakage; ratios ~80/10/10 (relaxed for sparse cells per Bayesian shrinkage policy).
- **FSV:** `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`
