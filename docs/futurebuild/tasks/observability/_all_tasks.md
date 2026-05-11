# Observability domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

OBS-005 has its own detailed file. The rest:

## TASK-OBS-001 — Weekly evaluation production feed/dashboard/runbook

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-HEAL-002, TASK-UTML-009 · **Spec:** SPEC-OBSERVABILITY REQ-OBS-FB-01 + TECH-OBSERVABILITY §3
- **Goal:** Turn the shipped daemon weekly-eval scheduler into a production operator surface backed by real holdout panels and dashboard/runbook commands.
- **What exists:** `crates/context-graph-mcp/src/daemon.rs::run_weekly_eval_scheduler` runs weekly, calls `EvalRunner`, persists `CF_MEJEPA_EVAL_REPORTS`, and writes `/mnt/d/contextgraph/exports/eval/<date>/weekly.{md,json}`.
- **Outputs still needed:** production holdout feed setup, dashboard/CLI aggregation, and runbook proof for weekly operations.
- **DoD:** triggered weekly against a populated production holdout feed; report contains all required sections (per-cell ship-gate, drift events, agent feedback summary, etc.) and is visible through the operator dashboard/runbook.

## TASK-OBS-002 — Pairwise MI heatmap renderer

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** none · **Spec:** SPEC-OBSERVABILITY REQ-OBS-FB-02
- **Goal:** Render `pairwise_mi_matrix.csv` to markdown table + PNG heatmap.
- **Outputs:** `crates/context-graph-mejepa/src/eval/mi_heatmap_renderer.rs` (NEW); Python helper `scripts/heatmap_render.py`.
- **DoD:** renders weekly; PNG visually inspectable.

## TASK-OBS-003 — Daemon status aggregator MCP tool

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-FLYWHEEL-001
- **Goal:** `mejepa_daemon_status` aggregates subscriber + heal + quota + VRAM in one call.
- **Outputs:** `mejepa_tools.rs::handle_daemon_status`.
- **DoD:** p50 < 100ms; output structured.

## TASK-OBS-004 — Per-prediction-class running calibration tracker

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-PREDICT-005
- **Goal:** Rolling-window ECE per head; expose via `mejepa_ship_gate_status` extension.
- **Outputs:** `eval/per_head_calibration_tracker.rs` (NEW).
- **DoD:** synthetic predictions with known calibration error reproduce ECE within ±2%.

## TASK-OBS-006 — FSV path migration sweep

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** none
- **Goal:** Update each `crates/*/examples/*_fsv.rs` to write evidence under `/mnt/d/contextgraph/fsv/<crate>-fsv/` instead of `/tmp/contextgraph-*-fsv/`.
- **Outputs:** mechanical sweep across ~30 FSV examples.
- **DoD:** `grep -r "/tmp/contextgraph-" crates/*/examples/` returns 0 matches.

## TASK-OBS-007 — Per-slice quota observability extension

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** none · **Spec:** SPEC-OBSERVABILITY REQ-OBS-FB-08
- **Goal:** Extend `mejepa_quota_status` response with per-slice breakdown per `TECH-OBSERVABILITY §6`.
- **Outputs:** `slice_quota_breakdown.rs` (NEW); extended MCP response.
- **DoD:** sum of slice GB matches total; matches `du -sh` for that root.

## TASK-OBS-008 — WSL GPU telemetry hardening

- **Phase:** F · **Effort:** 1-2d · **Blocked by:** none · **Spec:** SPEC-OBSERVABILITY REQ-OBS-FB-05 + TECH-OBSERVABILITY §5
- **Goal:** Hardened `nvidia-smi` wrapper with retry + N/A detection + WSL2 quirks.
- **Outputs:** `crates/context-graph-mcp/src/telemetry/gpu_wsl.rs` (NEW).
- **DoD:** N/A field reported as `unavailable: true`, not -1; transient timeouts retried.

## TASK-OBS-009 — `mejepa_inspect_prediction` MCP tool

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** Returns full provenance trace for a specific prediction.
- **Outputs:** new MCP handler.
- **DoD:** trace includes weight version, constellation version, calibration version, contributing chunks with vectors.

## TASK-OBS-010 — Active-learning operator CLI

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** none
- **Goal:** Per `TECH-OBSERVABILITY §8`. CLI subcommands: list, label, dismiss.
- **Outputs:** `crates/context-graph-cli/src/commands/mejepa_active_learning.rs` (NEW).
- **DoD:** all three subcommands work; round-trip with queue state verified.

## TASK-OBS-011 — Prediction replay tool (D5)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-OBS-009 · **Spec:** SPEC-API REQ-API-03 (in `24_API_STABILITY.md §5`)
- **Goal:** `context-graph-cli mejepa replay --prediction-id <id>` reproduces a historical prediction byte-for-byte.
- **Outputs:** `crates/context-graph-cli/src/commands/mejepa_replay.rs` (NEW).
- **DoD:** replay of 10 historical predictions reproduces identical RealityPrediction (or differs by exactly the new fields added since).

## TASK-OBS-012 — Reasoning-trace renderer (D6)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-OBS-009
- **Goal:** Render `mejepa_inspect_prediction` output as HTML / markdown for operator review.
- **Outputs:** `scripts/render_reasoning_trace.py` (NEW).
- **DoD:** HTML output contains contributing-signals table + saliency heatmap + counterfactual.

## TASK-OBS-013 — System cost telemetry (D7)

- **Phase:** F · **Effort:** 1d · **Blocked by:** none
- **Goal:** Track CUDA cycles + agent token cost of feedback events + RocksDB write volume.
- **Outputs:** `crates/context-graph-mejepa/src/telemetry/system_cost.rs` (NEW); weekly report extension.
- **DoD:** weekly report includes cost breakdown.

## TASK-OBS-014 — Health & readiness endpoints (D15)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-FLYWHEEL-001
- **Goal:** Lightweight HTTP probes (NOT for client API; for systemd / k8s health checks). `/health` returns 200 if daemon process alive; `/ready` returns 200 if all supervised tasks healthy.
- **Outputs:** new HTTP listener thread (opt-in via config).
- **DoD:** `curl localhost:9111/health` returns 200; `/ready` returns 503 when scheduler is flapping.

## TASK-OBS-015 — Log rotation policy (D16)

- **Phase:** F · **Effort:** 0.25d
- **Goal:** systemd-journald-style rotation for `/mnt/d/contextgraph/logs/*.log`. Daily rotation; keep 30 days.
- **Outputs:** `scripts/systemd/mejepa-log-rotate.timer` + service.
- **DoD:** logs rotate daily; old logs deleted at 30 days.

## TASK-OBS-016 — Executive summary renderer (D18)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-OBS-001
- **Goal:** Render `RealityPrediction` as a 1-paragraph executive summary for non-technical operators.
- **Outputs:** `scripts/render_executive_summary.py` (NEW).
- **DoD:** synthetic complex prediction → ≤ 200-word summary capturing key concerns.
