# Session Report: Phase 0/1 hardening

Date: 2026-05-09

## Scope

Harden and verify ME-JEPA Phase 0 and Phase 1 state using the sitrep docs and
Full State Verification discipline.

## Changed

- Hardened `crates/context-graph-mejepa-corpus/src/oracle.rs`:
  - xfailed/xpassed are tracked as raw pytest outcome kinds for summary
    validation while still mapping to canonical E_Oracle skip/fail buckets.
  - non-zero raw outcome counts now require matching summary fields, so
    incomplete pytest summaries fail closed.
- Fixed strict-Clippy loop findings in Phase 1 tests/static-analysis code.
- Updated sitrep docs to reflect that:
  - local Phase 0 command/Docker oracle wrapper and crate-local RocksDB store
    are shipped;
  - E_Witness and E_StaticAnalysis are shipped;
  - Phase 0's remaining blocker is the official SWE-bench Docker batch/full
    corpus generation;
  - Phase 1's remaining blocker is the 11 unimplemented instruments.

## Tests

- `cargo test -p context-graph-mejepa-instruments`
  - 40 passed.
- `cargo test -p context-graph-mejepa-corpus`
  - 59 passed.
- `cargo clippy -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments --all-targets -- -D warnings`
  - passed.

## FSV Evidence

- `cargo run -p context-graph-mejepa-corpus --example oracle_fsv`
  - Evidence: `/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json`
  - Trigger: real `python3 -m pytest`.
  - Report SoT: `/tmp/contextgraph-mejepa-corpus-fsv/oracle/pytest-report.json`.
  - RocksDB SoT: `/tmp/contextgraph-mejepa-corpus-fsv/oracle/rocksdb`.
  - Readback counts: `mejepa_mutation_corpus = 1`,
    `mejepa_oracle_verdicts = 1`.
  - Pytest summary: 1 pass, 1 fail, 1 skip, 0 errors, 3 total.
  - Boundaries: empty report, mismatched summary, stale report, bad store key.
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
  - Evidence: `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`.
- `cargo run -p context-graph-mejepa-corpus --example split_fsv`
  - Evidence: `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`.
  - 300 tasks across 12 repos; bucket sizes 195/77/28; cross-cut weights all 0.
- `cargo run -p context-graph-mejepa-instruments --example instruments_fsv`
  - Evidence: `/tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json`.
  - Panel SoT: `/tmp/contextgraph-mejepa-instruments-fsv/panel.json`.
  - Witness SoT: `/tmp/contextgraph-mejepa-instruments-fsv/witness_chain.bin`.
  - Static-analysis SoT:
    `/tmp/contextgraph-mejepa-instruments-fsv/static_analysis_input.json`.
- `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`
  - Evidence:
    `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`.

## Residual Risk

- Phase 0 is not doc-09-complete until official SWE-bench task images are run
  and the 2,400-entry corpus is generated and verified.
- Phase 1 is not doc-09-complete until the remaining 11 instruments ship and
  the linear-probe gate is run on the real corpus.
- The local oracle wrapper validates Docker availability, but the official
  SWE-bench images were not present locally and were not pulled/run in this
  hardening pass.
