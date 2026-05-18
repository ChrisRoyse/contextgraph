# Discovery: Phase 0/1 hardening root state

Date: 2026-05-09

## Root Findings

- Phase 0's hard blocker is no longer local oracle/store code. The tree now
  has `context_graph_mejepa_corpus::oracle` with command/Docker wrappers,
  strict pytest report parsing, timeout/stale-report handling, and crate-local
  RocksDB CFs:
  - `mejepa_mutation_corpus`
  - `mejepa_oracle_verdicts`
- The remaining Phase 0 blocker is operational: pull/run official SWE-bench
  Lite Docker task images and execute the full 300 tasks × 8 categories batch.
- Phase 1 now has 3 concrete instruments:
  - `E_Oracle`
  - `E_Witness`
  - `E_StaticAnalysis`
- The remaining Phase 1 blocker is breadth, not scaffold correctness: 11
  instruments are still deferred, especially E_Test/E_Problem/E_CommitMsg,
  E_Diff, E_AST/E_CFG/E_DataFlow, E_TypeGraph, E_Trace/E_Runtime/E_Reasoning,
  and Scalars.

## Hardening Applied

- Tightened pytest parser handling for xfailed/xpassed so raw pytest summary
  counts are validated against raw outcome kinds instead of collapsed
  canonical pass/fail/skip buckets.
- Changed summary validation so non-zero raw outcomes require the matching
  summary field, preventing truncated summaries from silently passing.
- Re-ran strict Clippy for the Phase 0/1 crates and fixed resulting loop
  lints.
- Updated sitrep docs to stop calling E_Witness/E_StaticAnalysis and the local
  oracle/store path deferred.

## FSV Evidence

- Phase 0 oracle/store:
  `/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json`
  - real `python3 -m pytest` trigger
  - report read from `/tmp/contextgraph-mejepa-corpus-fsv/oracle/pytest-report.json`
  - RocksDB reopened and both CFs scanned
- Phase 0 mutation/split:
  `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
  `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`
- Phase 1 instruments:
  `/tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json`
  `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`

## Verification Commands

- `cargo test -p context-graph-mejepa-instruments`
- `cargo test -p context-graph-mejepa-corpus`
- `cargo clippy -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments --all-targets -- -D warnings`
- `cargo run -p context-graph-mejepa-instruments --example instruments_fsv`
- `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
- `cargo run -p context-graph-mejepa-corpus --example split_fsv`
- `cargo run -p context-graph-mejepa-corpus --example oracle_fsv`
