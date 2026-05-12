# SPEC-OBSERVABILITY — Functional Specification

> **Level 2.** Defines WHAT the production observability surface must do.
> Owns: REQ-OBS-FB-01..13.
> Implementation reference: `12_IMPLEMENTATION_PHASES.md §F`. Extends Phase 8 SPEC-EVAL (shipped).

---

## User stories

- **As an operator**, I want a weekly report I can read in 5 minutes that tells me if ME-JEPA is on track.
- **As an operator**, I want live conformal coverage tracking so I notice if calibration drifts.
- **As an SRE**, I want WSL GPU telemetry that doesn't spuriously report N/A so my dashboards are reliable.
- **As a developer**, I want to inspect a specific prediction's full provenance trace.

## Requirements

### REQ-OBS-FB-01 — Weekly evaluation cron

A scheduled task SHALL run `mejepa_eval_run` weekly (default Monday 06:00 UTC) and write a report at `/mnt/d/contextgraph/exports/eval/<date>/weekly.md` with:
- Per-(category × language) prediction-oracle correlation
- Per-cell ship-gate status (≥0.95 met / not met)
- Active-learning queue summary (top-N items, by-kind breakdown)
- Agent-feedback summary (Confirmed / Surprise / Omission / Calibration counts; top 5 surprises)
- Drift events of the week
- Promotions / rollbacks of the week
- Storage utilization
- Catastrophic events (if any) at the top

### REQ-OBS-FB-02 — Pairwise MI heatmap

A weekly task SHALL render the pairwise MI matrix from `pairwise_mi_matrix.csv` to a markdown table embeddable in the weekly report, plus a PNG heatmap at `/mnt/d/contextgraph/exports/pairwise-mi/<date>.png`.

### REQ-OBS-FB-03 — Daemon status aggregator MCP tool

`mejepa_daemon_status` SHALL aggregate `mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`, `mejepa_vram_budget_report` into one response for fast operator inspection.

### REQ-OBS-FB-04 — Per-prediction-class running calibration tracker

The system SHALL maintain a rolling-window ECE per prediction head (10 heads) and surface in `mejepa_ship_gate_status` per-head.

### REQ-OBS-FB-05 — WSL GPU telemetry hardening

`nvidia-smi`-based GPU telemetry SHALL handle:
- N/A sentinels (don't return -1; mark `unavailable: true` in the response)
- Transient timeouts (retry with backoff up to 3 times)
- WSL2 GPU passthrough quirks (per `memory/decisions/agent-42-harness-editor--nvidia-smi-na-sentinel-tolerance.md`)

### REQ-OBS-FB-06 — `mejepa_inspect_prediction` MCP tool

`mejepa_inspect_prediction(prediction_id)` SHALL return the full provenance trace: witness_hash, predictor version, calibration version, contributing chunks (with their per-embedder vectors), TCT cells consulted, conformal interval computation steps.

### REQ-OBS-FB-07 — Active-learning operator CLI

`cargo run -p context-graph-cli mejepa active-learning [list|label|dismiss] [--top-n N]` SHALL wrap `mejepa_active_learning_queue` for easier operator interaction.

### REQ-OBS-FB-08 — Per-slice quota observability

`mejepa_quota_status` response SHALL break down disk usage by slice (RocksDB / shift-log / cold / replay / logs / fsv / exports) so operator sees where space is going.

### REQ-OBS-FB-09 — C-drive lint in CI

CI SHALL include a lint step that grep-asserts no source file introduces a hardcoded path matching `/home/`, `~/.cache/`, or `/tmp/contextgraph-` outside allowed exceptions (source tree itself; tempfile-wrapped scratch).

### REQ-OBS-FB-10 — FSV path migration sweep

A one-time sweep SHALL update each `crates/*/examples/*_fsv.rs` to write evidence under `/mnt/d/contextgraph/fsv/<crate>-fsv/` instead of `/tmp/contextgraph-*-fsv/`.

### REQ-OBS-FB-11 — Operator alert escalation

Catastrophic-severity alerts SHALL be visible at the top of the weekly report AND optionally sent via a configurable webhook (default: none — purely report-based).

### REQ-OBS-FB-12 — Live shift-subscriber metrics endpoint

`mejepa_subscriber_status` SHALL include latency p50/p99/max histograms over the last 1000 shifts, plus lag-alert state.

### REQ-OBS-FB-13 — Production telemetry source-of-truth windows

The system SHALL persist `ProductionTelemetryWindow` rows to `CF_MEJEPA_PRODUCTION_TELEMETRY` for each production evaluation window. Each row SHALL include:
- window id, source path, corpus SHA, sample count, and timestamp range
- conformal ECE, OOD AUC, Gτ pass rate, prediction-oracle agreement, ship-gate target, and ship-gate pass flag
- p50/p95/p99/max prediction latency
- CUDA-driver VRAM free/required/total bytes when CUDA is required

The ship-gate pass flag SHALL be derived from `prediction_oracle_agreement >= ship_gate_target`; stored contradictions SHALL fail closed. VRAM totals SHALL fail closed if free exceeds total, required is zero, or required exceeds total.

## Acceptance criteria

1. After one week of operation, `/mnt/d/contextgraph/exports/eval/<date>/weekly.md` exists and contains all required sections.
2. `mejepa_daemon_status` returns subscriber + heal + quota + VRAM in a single call < 100ms.
3. `nvidia-smi` returning "N/A" for any field → `mejepa_vram_budget_report` reports `unavailable: true` for that field, not a fake value.
4. `mejepa_inspect_prediction` for a recent prediction returns the full provenance with all referenced artifacts present.
5. CI lint catches a deliberately-introduced `/tmp/contextgraph-test/` path in source.
6. A production telemetry FSV writes final plus rolling windows, flushes/drops/reopens RocksDB, independently reads back every `CF_MEJEPA_PRODUCTION_TELEMETRY` row, and proves direct iterator count equals the expected window count.

## Edge cases

- **Weekly cron fires while a full retrain is in flight**: report writer waits up to 5 min for retrain to finish; if not, writes report with retrain marker noted.
- **Active-learning queue empty for two weeks**: report still writes (with empty queue section); not a problem.
- **GPU not present (CPU-only run)**: VRAM telemetry returns `gpu_present: false`; daemon refuses to load CUDA-required embedders.
- **Invalid production telemetry row**: empty window id, zero samples, non-monotonic latency, contradictory ship-gate flag, or impossible VRAM accounting fails validation before a RocksDB write.

## Non-functional

- Weekly report write < 10 min total.
- Heatmap render < 5s.
- Daemon status aggregator p50 < 100ms.
- Active-learning CLI p50 < 1s.
