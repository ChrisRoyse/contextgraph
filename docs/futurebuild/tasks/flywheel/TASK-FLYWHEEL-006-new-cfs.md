---
task_id: TASK-FLYWHEEL-006
title: Register CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_FAILURE_EXEMPLARS, CF_MEJEPA_AGENT_FEEDBACK
phase: A
status: complete
blocked_by: []
blocks: [TASK-PREDICT-002, TASK-ENRICH-002]
effort: 0.5d
spec_refs: [SPEC-FLYWHEEL.md REQ-FLYWHEEL-10, TECH-FLYWHEEL.md §6]
---

## Goal

Add three new column families to the shared CF registry in `context-graph-mejepa-cf`:

- `CF_MEJEPA_DDA_SIGNALS` — DDA pairwise + cosine signal block per chunk per panel
- `CF_MEJEPA_FAILURE_EXEMPLARS` — per-failure-mode reverse index for retrieval
- `CF_MEJEPA_AGENT_FEEDBACK` — append-only agent feedback ledger

Wire each into the storage opener so they're created on first daemon startup. Define the value structs (`DdaSignals`, `Vec<ExemplarRef>`, `SurpriseEvent`) with serde JSON for forward compatibility.

## Inputs

- `crates/context-graph-mejepa-cf/src/lib.rs` — existing CF registry
- `crates/context-graph-storage/src/lib.rs` — existing storage opener (for CF list bootstrap)
- `docs/futurebuild/specs/technical/TECH-FLYWHEEL.md` §6 (CF schemas)

## Outputs

- `crates/context-graph-mejepa-cf/src/lib.rs` — extended with 3 new CF constants + struct definitions
- `crates/context-graph-mejepa/src/types.rs` — extended with `DdaSignals`, `ExemplarRef`, related enums
- FSV: `crates/context-graph-mejepa-cf/examples/cf_round_trip_fsv.rs`

## Definition of done

1. `Storage::open` creates the 3 new CFs on a fresh storage root.
2. Round-trip serialize/deserialize for each value struct via serde JSON (lossless).
3. `cargo build --release -p context-graph-mejepa-cf` + `cargo test -p context-graph-mejepa-cf` pass.
4. FSV: `cargo run --release -p context-graph-mejepa-cf --example cf_round_trip_fsv` writes `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json` with one of each kind written, read back, and field-by-field compared.

## FSV evidence path

`/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json`

## Boundary cases

- DdaSignals with empty embedder set (zero active embedders configured) → length 0 vecs OK
- ExemplarRef list with > 100 entries → no truncation at CF layer; consumer paginates
- SurpriseEvent with empty `agent_explanation` → permitted; serialized as empty string
- Concurrent writes to `CF_MEJEPA_AGENT_FEEDBACK` from multiple agents → CF write is atomic per RocksDB; no extra locking needed

## Verification contract

Inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md).

## Completion record (2026-05-11)

- **`crates/context-graph-mejepa-cf/src/lib.rs`** — added `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK` constants + `FLYWHEEL_CFS` slice; extended `all_hygiene_referenced_cfs()` chain; 2 new tests.
- **`crates/context-graph-mejepa/src/types.rs`** — added value types: `PredictionId`/`FeedbackId`/`PanelId`/`WitnessHash`/`AgentId` newtypes, `SurpriseSeverity`/`FeedbackKind`/`FailureModeClass` enums, `DdaSignals`/`ExemplarRef`/`SurpriseEvent`/`ActualOutcome`/`EmbedderViolationSignature` structs with `validate()`. Re-uses `OracleVerdict` from `mejepa-instruments`.
- **`crates/context-graph-mejepa/examples/cf_round_trip_fsv.rs`** — FSV harness (the path differs from the original Outputs section because `mejepa-cf` is intentionally dependency-free; `mejepa` is where rocksdb + value types live). All happy-path round-trips and 4 boundary cases pass.
- **`crates/context-graph-mejepa-hygiene/tests/flywheel_cfs_registered_test.rs`** — integration test confirming `open_hygiene_rocksdb` creates the 3 CFs on a fresh root AND across close-reopen.
- **FSV evidence**: `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json` with `all_passed: true`; real RocksDB SST files at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/rocks-1778516317859/000009.sst` and siblings.
- **Session report**: `./memory/reports/session-2026-05-11--task-flywheel-006-new-cfs.md`
- **Decision**: kept uppercase CF string convention matching the rest of `mejepa-cf`; spec illustrations used lowercase. See `./memory/discoveries/2026-05-11--mejepa-cf-uppercase-string-convention.md`.
