---
namespace: reports
agent_id: codex
timestamp: 2026-05-12T15:40:00Z
created: 2026-05-12T15:40:00Z
updated: 2026-05-12T15:40:00Z
status: active
tags:
  - phase-a-d
  - readiness-audit
  - mejepa
  - dda
  - shift-subscriber
  - full-state-verification
---

# Phase A-D Readiness Audit and Live DDA Fix

## User request

The operator asked for a root-cause readiness audit of all Phase A-D work before moving forward, with web research through Exa MCP, no mocks/fallbacks/workarounds, and full state verification. Required directives were read:

- `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`
- `/home/cabdru/contextgraph/docs2/hardeningprompt.md`
- `/home/cabdru/contextgraph/docs2/flagprompt.md`
- `/home/cabdru/leapablememory/docs2/infisical-secrets-guide.md`
- `/home/cabdru/leapablememory/docs2/aiwonder-system.md`

No live secrets were needed.

## Readiness verdict

The repo is not fully ready to move past Phase A-D as of this audit. The concrete live DDA persistence gap in Phase D was fixed and fully state-verified, but the docs/evidence still show remaining gates:

- Full pretrained model artifact FSV and full Language x EntityType routing forward-output FSV remain open for active content embedders.
- Trained prediction heads do not yet consume persisted DDA/neural features.
- Production cache pruning/size-bound enforcement and daemon hit-rate telemetry remain open.
- Production calibration/OOD/latency evidence and broad static CI gate still have tracked gaps.
- Phase B label analyzer hardening, UTML/flywheel gaps, and bootstrap release checkpoint are not closed.

Umbrella gate issue opened: https://github.com/ChrisRoyse/contextgraph/issues/15

## Best-practice research

Exa MCP research was performed for streaming feature pipelines, RocksDB durability, and online/offline feature consistency. Findings used:

- RocksDB `WriteOptions::set_sync(true)` documents that sync writes flush OS buffers before writes are considered complete; disabling WAL risks data loss; ignoring missing column families can silently skip intended writes.
- AWS SageMaker Feature Store docs describe online/offline feature stores and the need for consistent feature processing to reduce training-serving skew.
- Supplementary streaming feature-pipeline guidance reinforced idempotent writes and not advancing offsets until downstream writes are durably committed.

Design applied: the live subscriber now computes the DDA rows before prediction/watermark side effects, persists through the existing synchronous DDA writer, and relies on immediate RocksDB readback inside `persist_dda_signals`.

## Root cause fixed

Before this change, `ShiftSubscriber::process_shift` wrote predictions, panels, oracle/cert rows, and watermarks, but never called the DDA compute/persist path. `CF_MEJEPA_DDA_SIGNALS` could remain empty even after a successful live shift stream.

Changed files:

- `crates/context-graph-mejepa-shift-subscriber/Cargo.toml`
- `crates/context-graph-mejepa-shift-subscriber/src/error.rs`
- `crates/context-graph-mejepa-shift-subscriber/src/models.rs`
- `crates/context-graph-mejepa-shift-subscriber/src/runner.rs`
- `crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs`
- `crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs`
- `Cargo.lock`
- Futurebuild docs/specs were aligned to distinguish shipped live algorithmic DDA rows from remaining full pretrained/routing and trained-head consumption gates.

Implementation notes:

- `process_shift` is async and computes DDA rows before live prediction persistence.
- The live path uses real `AlgorithmicEmbedderForward` slots E2/E3/E4/E9 and `EmbedderCache`; temporal embedder inputs include explicit `epoch:<seconds>` so there is no current-time fallback.
- Empty post-change chunks fail closed with `MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT` before any prediction, DDA, or watermark state is written.
- `ShiftOutcome::Predicted` now exposes `dda_signal_count`.
- Runner supervision still catches async panics.

## Full state verification

Source of truth:

- Live subscriber inference DB: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/infer-db`
- DDA rows: RocksDB `CF_MEJEPA_DDA_SIGNALS`, keyed by `(panel_id, chunk_id)`
- Embedder cache: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/mejepa-embedder-cache`
- Watermark/prediction/oracle/cert evidence:
  - `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/watermark.json`
  - `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/observe-evidence.json`
  - `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/edge-cases.json`
  - `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/manifest.json`

Happy path manual FSV after a 100-shift stream:

- Before: `prediction_count=0`, `dda_signal_count=0`, `oracle_verdict_count=0`, `phase7_train_cert_count=0`
- After readback: `prediction_count=100`, `dda_signal_count=100`, `oracle_verdict_count=20`, `phase7_train_cert_count=20`
- Sample DDA rows read from RocksDB:
  - `answer_100.py#0`, `embedder_count=4`, per-embedder cosine `[1.0, 1.0, 1.0, 1.0]`, nonzero pairwise/blind-spot arrays
  - `answer_001.py#0`, `embedder_count=4`, per-embedder cosine `[1.0, 1.0, 1.0, 1.0]`, nonzero pairwise/blind-spot arrays
- Embedder cache physical evidence: 400 files for 100 chunks x 4 algorithmic embedders.

Edge cases manually simulated and read back from source of truth:

1. Non-code shift:
   - Before counts all zero.
   - Outcome skipped with `no_oracle_signal`.
   - After counts stayed zero for prediction/DDA/oracle/cert.
   - Watermark advanced to `01J00000000000000000101`.
2. Empty post-shift chunk:
   - Before counts all zero.
   - Outcome failed closed with `MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT`.
   - After counts stayed zero.
   - Watermark readback remained `{}`.
3. Low L-step oracle:
   - Before counts all zero.
   - Outcome processed/predicted with `dda_signal_count=1`, `observed=false`, `dropped_l_step_below_threshold=1`.
   - After readback: `prediction_count=1`, `dda_signal_count=1`, `oracle_verdict_count=1`, `phase7_train_cert_count=1`.
   - Watermark advanced to `01J00000000000000000103`.

## Verification commands run

Passed:

- `cargo check -p context-graph-mejepa-shift-subscriber`
- `cargo test -p context-graph-mejepa-shift-subscriber -- --nocapture`
- `cargo test -p context-graph-mejepa-shift-subscriber --all-targets -- --nocapture`
- `cargo clippy -p context-graph-mejepa-shift-subscriber --all-targets -- -D warnings`
- `cargo test -p context-graph-mejepa-train dda -- --nocapture`
- `cargo test -p context-graph-mejepa-embedders cache -- --nocapture`
- `cargo run -p context-graph-mejepa-shift-subscriber --example shift_subscriber_fsv`
- `cargo run -p context-graph-mejepa --example phase_b_prediction_fsv`
- `cargo run -p context-graph-mejepa-train --example phase_d_dda_fsv`
- `cargo run -p context-graph-mejepa-embedders --example phase_d_embedder_cache_fsv`
- `cargo run -p context-graph-mejepa-instruments --example phase_d_stdlib_registry_fsv`
- `git diff --check`

## Existing dirty worktree context

The worktree had user/doc edits before this pass:

- `docs/futurebuild/02_CURRENT_STATE_HONEST.md`
- `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`
- `docs/futurebuild/13_TASK_INDEX.md`
- `docs/futurebuild/tasks/enrichment/_all_tasks.md`

These were not reverted. The final worktree also includes targeted code, FSV, and documentation updates from this pass.
