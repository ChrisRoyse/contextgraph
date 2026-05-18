---
namespace: discoveries
created: 2026-05-09T04:02:04Z
updated: 2026-05-09T04:02:04Z
status: active
tags: mejepa, instruments, panel, oracle, fail-closed, serde, fsv
---

# ME-JEPA Instruments Must Reject Zero-Evidence State

## What Happened
The new `context-graph-mejepa-instruments` scaffold originally had three fail-open paths:

- `EOracleInstrument::encode()` accepted `OracleVerdict { per_test: [], exception: None }` and produced a zero vector.
- `PanelBuilder::build()` returned an all-zero panel with `filled_mask = 0`.
- `Panel` derived serde directly, so malformed persisted JSON could deserialize with the wrong vector length, non-finite values, or unknown filled-mask bits.

## Why It Matters
For ME-JEPA, zero is a meaningful slot value but also the natural value of missing data. Accepting an empty oracle
verdict or empty panel would make missing test-run reality indistinguishable from a real encoded state and could
poison `CF_MEJEPA_PANELS` once storage wiring lands.

## Fix Applied
The hardened crate now:

- Rejects empty no-exception oracle verdicts, whitespace/control-character test ids, invalid runtime sentinels, and test counts above 1,000,000.
- Makes `PanelBuilder::build()` return `InstrumentResult<Panel>` and rejects `filled_mask = 0`.
- Makes `Panel` fields private and deserializes through `#[serde(try_from = "PanelSerde", into = "PanelSerde")]`, validating `PANEL_DIM`, finite values, nonzero mask, and valid slot bits.
- Updates the FSV example to write `/tmp/contextgraph-mejepa-instruments-fsv/panel.json` and prove malformed persisted panels reject during independent readback.

## Related
- `crates/context-graph-mejepa-instruments/src/e_oracle.rs`
- `crates/context-graph-mejepa-instruments/src/lib.rs`
- `crates/context-graph-mejepa-instruments/examples/instruments_fsv.rs`
- Source of truth: `/tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json`
