# Session Report: ME-JEPA Python Parser Harness FSV

Date: 2026-05-09
Agent focus: Python-only hardening for the ME-JEPA prediction system.

## Task

Investigate the AST/parser harness for the ME-JEPA prediction system, identify root causes, research best practices, fix robustly without fallbacks, update documentation, and manually verify final state by reading the physical source of truth.

## Outcome

Completed for the Python ME-JEPA corpus harness.

The current Python mutation harness is fail-closed around parseability and mutation-site integrity. The FSV example now uses real SWE-bench Lite task manifests instead of hard-coded synthetic split data, and the generated evidence file records source-of-truth readbacks for generated Python files, compiled bytecode, edge-case errors, and split assignments.

## Files Touched

- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs`
  - Replaced hard-coded synthetic split corpus construction with real manifest loading from `tasks/swebench-lite`.
  - Recorded the real task manifest directory in split FSV evidence.
  - Kept validation fail-closed: missing `instance_id`, missing `repo`, or empty manifest directory returns an FSV error.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
  - Documents the current Python parser-backed harness state, FSV source of truth, py_compile verification, and remaining Phase 0 gaps.
- `memory/context/mejepa-implementation-initiative.md`
  - Records current Phase 0 Python harness and FSV evidence state.
- `memory/discoveries/2026-05-09--mejepa-python-parser-harness-fsv-source-of-truth.md`
  - Records the FSV source-of-truth discovery.

## Root Causes

1. Historical parser dependency failure: `python_lex.rs` used `tree_sitter` and `tree_sitter_python` before the crate manifest carried those dependencies. Current HEAD contains the dependency fix.
2. Historical stale FSV invariant: the delete-test-call check previously expected empty replacement after the operator was changed to replace assertion lines with `pass`. Current FSV now asserts the actual intended persisted state.
3. Current issue fixed this session: split FSV used a hard-coded synthetic corpus whose task counts drifted from its own `assert_eq!(total, 300)`. This violated the no-mock-data requirement and made the split evidence less meaningful than the real task manifests.

## Research Applied

- Python `ast.parse` and compile semantics: parsing can raise `SyntaxError`, and compilation can still fail after parsing; FSV therefore verifies generated sources through `py_compile`.
- Python lexical/f-string semantics: f-string replacement fields are expressions; scanners must avoid literal text but keep replacement fields visible to code-oriented mutation operators.
- Tree-sitter parser behavior: parse trees can contain syntax errors; parser-backed harnesses must inspect `has_error()`/error nodes and fail closed.

## Verification

Commands completed successfully:

- `cargo test -p context-graph-mejepa-corpus`
  - Passed: 48 tests before concurrent additions.
- `cargo test --release -p context-graph-mejepa-corpus`
  - Passed after current changes: 50 tests.
- `cargo run --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
  - Passed and wrote source-of-truth evidence.

Source of truth:

- Evidence JSON: `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
  - Size: 21,542 bytes
  - SHA-256: `sha256:f4772960e5ce6f0241647b874d3371ddf5aa45259c2c7108b4bca5cc75107664`
- Split assignment CSV: `/tmp/contextgraph-mejepa-corpus-fsv/split-assignments.csv`
  - Size: 13,735 bytes
  - SHA-256: `sha256:a6292cacdd3f75b6d6ae460303e565bdbe801958fe4ac3bd8fcb545280060c4d`
- Real task manifest count readback: 300 JSON files in `tasks/swebench-lite`
- Split evidence readback:
  - `corpus_size`: 300
  - `repo_count`: 12
  - `bucket_sizes`: `[195, 77, 28]`
  - `bucket_fractions`: `[0.65, 0.25666666666666665, 0.09333333333333334]`
  - `cross_cut_weights`: `[0.0, 0.0, 0.0]`
  - `leakage_check_passed`: true
  - CSV rows: 300
  - Unique task IDs: 300

Happy-path physical verification:

- Generated Python sources were written under `/tmp/contextgraph-mejepa-corpus-fsv/generated_sources`.
- Expected-good generated Python files were compiled with `py_compile`.
- Expected `.pyc` artifacts were read back under `/tmp/contextgraph-mejepa-corpus-fsv/pyc`.
- Delete-test-call mutation source-of-truth readback:
  - Original text: `        assert score >= 0.0\n`
  - Replacement text: `        pass\n`
  - The generated output compiled successfully.

Boundary and edge-case audit:

- Empty input rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Wrong-file mutation without alternate source rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Subtle flip with no boolean operator rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
- Subtle flip docstring-only source rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
- Off-by-one floats-only source rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
- Off-by-one docstring-only source rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
- Delete-test-call with no assertions rejected with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
- Empty split corpus rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Single-repo degenerate split rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Split fractions that do not sum to one rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Duplicate task IDs rejected with `MEJEPA_CORPUS_INVALID_INPUT`.
- Whitespace repo IDs rejected with `MEJEPA_CORPUS_INVALID_INPUT`.

Known limitation from this session:

- `cargo test --release -p context-graph-core ast_chunker` failed before test execution due rustc/LLVM being killed while compiling `hnsw_rs` under release mode. A debug retry was blocked by concurrent cargo activity and was stopped to avoid leaving a process running. This was not a parser-harness regression, but core AST chunker verification should be rerun in a less memory-constrained or uncontended build window.

## Next Resume Point

Continue Python ME-JEPA work by connecting parser-harness evidence into the outer reality loop scoring path, then add durable RocksDB/graph source-of-truth checks for prediction outcomes. Keep using real SWE-bench Lite manifests and compiled/generated artifacts as verification anchors; do not reintroduce synthetic split plans.
