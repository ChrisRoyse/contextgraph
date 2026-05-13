# Phase F Deprecation Call Sites

Source of truth: `crates/context-graph-mcp/src/deprecation.rs`.

This inventory is for TASK-API-006. Every listed retired MCP entry point must
route through `apply_retired_cgreality_deprecation()` before returning to the
caller. Default behavior adds `warnings[]`; `CONTEXT_GRAPH_DEPRECATION_LEVEL=error`
returns a hard JSON-RPC error.

| Tool | Deprecated since | Remove after | Error code | Replacement |
|---|---:|---:|---|---|
| `mcp__cgreality__reality_run_attempt` | 2026-05-09 | 2026-08-09 | `CCREALITY_ENGINE_RETIRED` | `mcp__cgreality__mejepa_verify`, `mcp__cgreality__mejepa_predict_latest`, or `mcp__cgreality__mejepa_observe_shift` |
| `mcp__cgreality__reality_alter_file` | 2026-05-09 | 2026-08-09 | `CCREALITY_ENGINE_RETIRED` | `mcp__cgreality__mejepa_verify`, `mcp__cgreality__mejepa_predict_latest`, or `mcp__cgreality__mejepa_observe_shift` |
| `mcp__cgreality__reality_alter_run` | 2026-05-09 | 2026-08-09 | `CCREALITY_ENGINE_RETIRED` | `mcp__cgreality__mejepa_verify`, `mcp__cgreality__mejepa_predict_latest`, or `mcp__cgreality__mejepa_observe_shift` |
| `mcp__cgreality__reality_alter_apply` | 2026-05-09 | 2026-08-09 | `CCREALITY_ENGINE_RETIRED` | `mcp__cgreality__mejepa_verify`, `mcp__cgreality__mejepa_predict_latest`, or `mcp__cgreality__mejepa_observe_shift` |
| `mcp__cgreality__reality_drip_attempt` | 2026-05-09 | 2026-08-09 | `CCREALITY_ENGINE_RETIRED` | `mcp__cgreality__mejepa_verify`, `mcp__cgreality__mejepa_predict_latest`, or `mcp__cgreality__mejepa_observe_shift` |

Verified call sites:

| File | Call site |
|---|---|
| `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs` | Normal MCP `tools/call` dispatcher retired-tool result |
| `crates/context-graph-mcp/src/main.rs` | Legacy `--mode reality-loop` stdio retired-tool result |
