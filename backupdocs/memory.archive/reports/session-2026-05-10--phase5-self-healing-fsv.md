---
namespace: reports
created: 2026-05-10T17:45:00-05:00
updated: 2026-05-10T17:45:00-05:00
status: active
tags: mejepa, phase5, self-healing, fsv, rocksdb
---

# Phase 5 Self-Healing FSV Session

Implemented/verified the ME-JEPA Phase 5 self-healing path: compiler observe delegates to the healing pipeline, the release CLI exposes `heal-drill`, the drill writes durable FSV evidence under `/tmp/contextgraph-mejepa-self-healing-fsv`, RocksDB self-healing CFs are created/read back, and MCP status/rollback helpers are tested against persisted state.

Final happy-path drill:

- Command: `cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift hard --seed 42`
- Result: `exit_code=0`, hard drift detected at 700 observations, final empirical coverage `0.82`, mode winner `C`, LoRA refresh succeeded, witness integrity violation detected, Fisher rank `64/64`.
- FSV files: all five JSON evidence files present with mode `0600`.
- RocksDB readback: `CF_MEJEPA_FISHER_SNAPSHOTS=5`, `CF_MEJEPA_DRIFT_WINDOW=700`, `CF_MEJEPA_DRIFT_HISTORY=1`, `CF_MEJEPA_HEAL_REPORTS=1`, `CF_MEJEPA_DISTILL_STEPS=700`, `CF_MEJEPA_ACTIVE_POINTERS=1`, `CF_MEJEPA_WEIGHT_BLOBS=3`, `CF_MEJEPA_SHIFT_WATERMARK=1`.

Manual edge checks:

- Invalid boundary: `--max-observations 699` failed with `MEJEPA_HEAL_INVALID_STATE` and wrote no FSV state.
- Soft drift: final coverage `0.86`, severity `soft`, 700 drift rows, mode winner `C`, exit `0`.
- Catastrophic drift: final coverage `0.75142854`, severity `catastrophic`, integrity evidence paused the status after corruption, exit `0`.

Validation commands passed:

- `cargo test -p context-graph-mejepa`
- `cargo test -p context-graph-mejepa-train learning_signal`
- `cargo check -p context-graph-mcp`
- `cargo test -p context-graph-mcp test_total_tool_count_and_no_duplicates`
- `cargo test -p context-graph-mcp mejepa_heal_status_and_rollback_read_persisted_source_of_truth`
- `cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv -- --inject-drift hard --seed 42`

Manual MCP stdio startup was attempted with `--no-warm`, but the default MCP server still fails before tool calls when local embedding model assets are absent. The Phase 5 MCP handler logic is covered by direct persisted-state tests instead of masking that asset error.
