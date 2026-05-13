---
namespace: reports
agent_id: agent-codex
created: 2026-05-13T04:46:00-05:00
updated: 2026-05-13T04:46:00-05:00
status: active
tags: phase-f,api,deprecation,mcp,fsv,issue-88
---

# Session Report: 2026-05-13 — Phase F Deprecation Call Sites

## Task: #88 TASK-API-006 deprecation framework call-site audit
## Outcome: complete

## Summary
Wired the retired cgreality MCP tool responses through the Phase F deprecation framework. The normal `tools/call` dispatcher path and the legacy `--mode reality-loop` stdio path now share the same retired-tool metadata, so default behavior returns the existing `CCREALITY_ENGINE_RETIRED` MCP error result with a `warnings[]` entry, while `CONTEXT_GRAPH_DEPRECATION_LEVEL=error` returns a hard JSON-RPC error.

Added the required static call-site inventory at `scripts/ci/phase_f_deprecation_call_sites.md` and extended the deprecation FSV harness to verify all retired tools, the inventory readback, warning injection, error escalation, and missing-notice fail-closed behavior.

## Files Created or Modified in the Repo
- `crates/context-graph-mcp/src/deprecation.rs` — centralized retired cgreality tool metadata and helper.
- `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs` — normal MCP retired-tool responses now apply deprecation warnings/errors.
- `crates/context-graph-mcp/src/main.rs` — legacy `--mode reality-loop` retired-tool response path now applies the same helper.
- `crates/context-graph-mcp/examples/phase_f_deprecation_framework_fsv.rs` — FSV now covers TASK-API-006 call-site behavior and inventory readback.
- `scripts/ci/phase_f_deprecation_call_sites.md` — required retired-tool inventory.
- `crates/context-graph-mejepa/src/lib.rs` — compile unblocker for concurrent #84: re-export `system_cost::*`.
- `crates/context-graph-mejepa/src/toolchain_detect.rs` — compile unblocker for concurrent #85: `ToolchainMissingDiagnostic` derives `Deserialize`.
- `crates/context-graph-mcp/tests/toolchain_startup_gate.rs` — compile unblocker for concurrent #85: hoisted `lock_count` out of `json!`.

## Files Written or Updated in ./memory/
- `reports/session-2026-05-13--phase-f-deprecation-call-sites.md` — this report.

## Decisions Made
- Centralized retired cgreality deprecation metadata in `deprecation.rs` instead of duplicating the list in the handler and legacy stdio path.
- Used `2026-08-09` as the `remove_after` date: 90 days after the 2026-05-09 ME-JEPA pivot retirement.

## Discoveries
- The retired-tool path existed in two live call surfaces: `handlers/tools/dispatch.rs` via `mejepa_phase7_tools.rs`, and the older `run_reality_loop_stdio()` path in `main.rs`.
- Concurrent #84/#85/#86 edits were present in the shared worktree and initially blocked MCP compilation; overlap notes were posted on #84 and #85 before minimal unblockers.

## Failures
- Initial targeted tests failed before reaching #88 due to concurrent unfinished edits: missing `SystemCostCounters` root export, missing `Deserialize` for `ToolchainMissingDiagnostic`, and a `json!` macro syntax error in #85's startup-gate test.

## Successes
- Retired cgreality tools have structured deprecation notices and default warnings.
- Error escalation returns a hard JSON-RPC error for deprecated retired tools.
- FSV readback proved 5 happy-path cases and 5 boundary cases.

## Open Blockers
- None for #88. The worktree still contains unrelated/in-progress Phase F follow-up edits from other issues.

## Tests / Build
- Tests added: retired cgreality notice/unit behavior and retired-tool JSON-RPC error escalation.
- Tests passing: yes.
- Build status: passing.
- Clippy: clean.

## FSV evidence
- `/mnt/d/contextgraph/fsv/phase-f-deprecation-framework-fsv/deprecation_framework_fsv.json` — sha256:c5d480d00f0c4da59ea8adeae83ed116706c8a3810c340d4b53006a5f8c05f19
- Inventory file sha256:282cd9ff91aa2d388bb5cf299f4ff6435ff67d4beacf173ca9f62458345f26d5

## GitHub issues touched
- #88 — claimed, discovery-commented, resolved.
- #84 — overlap note for `SystemCostCounters` compile unblocker.
- #85 — overlap/build-blocker notes for `main.rs` and startup-gate test/diagnostic compile unblockers.

## Resume Here Next Session
- Start by checking open `phase:f-followup` issues and avoid #65 unless explicitly released by its owner.
- Watch out for: current worktree has mixed concurrent edits; do not assume every dirty file belongs to this session.
- Priority: continue unclaimed Phase F follow-ups after #88 closes.
- Open question: whether the concurrent #84/#85/#86 work will be committed together or split by issue owner.

## Confidence: high
The #88 path has source-of-truth FSV, targeted and full lib tests, release build, and clippy evidence. The only residual risk is coordination noise from concurrent dirty files owned by other Phase F follow-up issues.
