---
namespace: reports
agent_id: codex
created: 2026-05-09T09:20:00Z
updated: 2026-05-09T09:20:00Z
timestamp: 2026-05-09T09:20:00Z
status: active
tags: report, session, mejepa, phase-0, phase-1, fsv
---

# Session Report: Phase 0/1 ME-JEPA implementation + FSV

## Task

Fully implement the Phase 0 `context-graph-mejepa-corpus` and Phase 1
`context-graph-mejepa-instruments` gaps, using first-principles root-cause
analysis, best-practice research, fail-closed behavior, and Full State
Verification against real sources of truth.

## Research applied

- SWE-bench official docs define JSONL predictions with `instance_id`,
  `model_name_or_path`, and `model_patch`, and the harness writes per-instance
  artifacts under `logs/run_evaluation/<run_id>/<model>/<instance_id>/`.
- Tree-sitter official docs describe `ERROR` and `MISSING` recovery nodes, so
  Phase 1 code instruments reject recovered/error parses instead of treating
  them as valid signal.
- Clap derive docs support a typed subcommand CLI; the corpus CLI uses this
  path rather than ad hoc argument parsing.

## Root causes

- Phase 0 had operators, splits, store, and a one-task bridge, but not the
  documented operator-facing `generate`/`verify`/`stats` command.
- Phase 0 verification originally trusted absolute index paths. A copied
  corpus could verify against the original store and patch files.
- Phase 0 validation could create partial state for preflight errors.
- Phase 1 could represent a 5,120-d panel but did not have real encoders for
  all 15 slots.

## Implemented

- Added `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`:
  `generate`, `verify`, and `stats`.
- Added unified-diff mutation bridge
  `crates/context-graph-mejepa-corpus/src/patch_mutation.rs`.
- Corpus index hardening:
  - corpus-relative `store_path` and `patch_path`;
  - absolute/path-escaping index paths rejected;
  - `complete=false` until all requested entries and stats are written;
  - `verify` checks patch file hash, RocksDB mutation row hash, RocksDB verdict
    hash, mutation CF count, and verdict CF count.
- Added Phase 1 deterministic typed encoders for all remaining panel slots:
  `features`, `e_code`, `e_text`, `e_trace`, `e_runtime`, `e_reasoning`,
  and `e_scalars`.
- Extended `phase1_extended_fsv` to build/read a full 15-slot panel and record
  before/after state plus edge-case outcomes.

## Full State Verification

### Phase 0 happy path

Source of truth:
`/tmp/contextgraph-mejepa-corpus-cli-fsv-final`

- Trigger:
  `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- generate --output /tmp/contextgraph-mejepa-corpus-cli-fsv-final --instance-id sympy__sympy-20590 --category known_good --run-id-prefix mejepaFinalFsv --instance-timeout-secs 1800 --overall-timeout-secs 3600`
- `index.json` readback:
  `complete=true`, `store_path="oracle-store"`,
  `patch_path="mutations/sympy__sympy-20590/known_good.patch"`,
  `split_assignment_count=300`.
- SWE-bench report readback:
  `patch_successfully_applied=true`, `resolved=true`,
  FAIL_TO_PASS success `["test_immutable"]`, PASS_TO_PASS count `21`.
- `verify-evidence.json`:
  `passes=true`, `entry_count=1`, `mutation_cf_count=1`,
  `verdict_cf_count=1`.
- `stats.json`:
  `total_entries=1`, `by_category.known_good=1`, `by_oracle.pass=1`.

### Phase 0 edge cases

- Invalid category:
  `not_a_category` failed closed and
  `/tmp/contextgraph-mejepa-corpus-edge-invalid-category2` was not created.
- No mutation site:
  `subtle_flip` on `sympy__sympy-20590` failed closed with
  `MEJEPA_CORPUS_NO_MUTATION_SITE` and
  `/tmp/contextgraph-mejepa-corpus-edge-no-site2` was not created.
- Tampered copy:
  `/tmp/contextgraph-mejepa-corpus-cli-fsv-final-tampered/verify-evidence.json`
  has `passes=false` with a `patch hash mismatch`.
- Concurrent RocksDB opens:
  parallel `stats` and `verify` caused RocksDB lock refusal instead of
  inconsistent reads; rerunning serially passed.

### Phase 1 happy path and edges

Source of truth:
`/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended`

- Trigger:
  `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`
- `panel-all-slots.json` readback:
  `data.len()=5120`, `filled_mask=32767`.
- Evidence readback:
  `passes=true`, `panel_dim=5120`, `filled_slot_count=15`,
  `edge_case_count=11`.
- Edge cases include empty/tampered witness chain, blank static source,
  diagnostic line out of bounds, missing required slot, code parse error,
  unsupported language, empty trace, invalid runtime coverage, blank reasoning,
  and invalid scalar health.

## Automated verification

- `cargo fmt --all`
- `cargo test -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments`
  passed: 87 corpus tests, 44 instrument tests.
- `cargo clippy -p context-graph-mejepa-corpus -p context-graph-mejepa-instruments --all-targets -- -D warnings`
  passed.

## Residual scope

- The full 300-task × 8-category SWE-bench batch was not run in this session;
  the implementation now has the CLI and verification gates for it, but the
  actual batch is an operational multi-hour Docker run.
- Current code instruments are Python/tree-sitter backed and reject
  unsupported languages. Multi-language AST chunker/instrument expansion is
  still separate Phase 1/Doc 14 scope.

