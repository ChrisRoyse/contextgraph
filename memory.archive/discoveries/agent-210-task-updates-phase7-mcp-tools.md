---
namespace: discoveries
agent_id: agent-210-task-updater
timestamp: 2026-05-10T00:00:00Z
status: active
tags: phase7-mcp-tools, task-updates, audit, mandate-block, sha-provenance
files:
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md
    pre_sha: 58ac7e631df40b484beaa5f4e2007d7ec1edc8586e9d19a383bb67b902b2affd
    post_sha: bef9c80fd6b9e37e33489171bcf405421a1cc652c8c5a34b909ee29e52c7496c
    audit_finding: "Assumes sub-module tree mejepa/{mod,schemas,verify,predict_latest,observe_shift,subscriber_status,capture_audit}.rs; reality is flat file mejepa_tools.rs. 5-tool scope is outdated (now 18 planned tools, 8 shipped, 10 pending). Recommends full rewrite of scope/files/signatures/verification blocks."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-002.md
    pre_sha: 6aeeeb92eda20c1d16d124b31340425e9c3c7abc502cc725021d41c81394c15d
    post_sha: 7e4f5aec60dcde43585b0a7b278c72229a002dab6f63bba2c74844d20e3619e6
    audit_finding: "mejepa_verify ALREADY SHIPPED as call_mejepa_verify (line ~76) in flat file mejepa_tools.rs. Task should pivot to audit-and-close-gaps rather than create new shim."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-003.md
    pre_sha: 7fad80d19d25fca33048bc52223237ced065186ba7b1953ec81368d52d8bd602
    post_sha: ec0bfaa57a8bcfa89b0083364010d933423935764d2d75d10028c3043409daeb
    audit_finding: "mejepa_predict_latest ALREADY SHIPPED as call_mejepa_predict_latest (line ~151) in flat file mejepa_tools.rs. Writer side (TASK-SHIFT-007 CF_MEJEPA_LIVE_PREDICTIONS) may not yet populate rows; verify."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-004.md
    pre_sha: 7013b54415f303041136fb03ee00c86d931d0628f8df08d0a9c83b01773c25ed
    post_sha: c19ba8843248a5fae6db5bdf7c1f843b3c9b568dc28ca400feca7b8b1b0427e9
    audit_finding: "mejepa_observe_shift PENDING. Blocked on Phase 7 shift-subscriber crate (does not exist). Sub-module tree assumption wrong; should ship as new call_mejepa_observe_shift in flat file."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-005.md
    pre_sha: 4c237a06b07add69151de438bde6d7a452dc13cf20b1f62c5ed2df2ce85770d3
    post_sha: 5fe1be41e8158beb31935221335ef0689ac2ce806eecec778a048aa86925b98b
    audit_finding: "mejepa_subscriber_status PENDING. Blocked on TASK-SHIFT-002 (SubscriberMetrics) + TASK-SHIFT-008 (WatermarkWriter). Sub-module tree assumption wrong."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-006.md
    pre_sha: 9598d129d845478cc8f9b381f1350ae5be029da3be93aabe9b32ac2e3b88b251
    post_sha: c682057efc4e17224f02af1dea7e940ae19d643a102a04fd117c0c6ab3d6cfc9
    audit_finding: "mejepa_capture_audit PENDING. Blocked on 4 RocksDB CF writer paths (TASK-SHIFT-002, TASK-SHIFT-007) + CaptureAuditBundle type. Sub-module tree assumption wrong."
  - path: docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-007.md
    pre_sha: e54452d9e63732242f215f554fd44a7c5f999717ed1d72c8a0ee006e96701819
    post_sha: d20ccc6a7a12996be64c18bce319fc0e8cfa529aae2cc9cb267e245b6c05847a
    audit_finding: "FSV harness scoped to '5 mejepa_* tools'; post-pivot reality is 18 planned tools (8 shipped + 10 pending). Should be re-scoped to cover 8 shipped tools NOW + separate Phase 6/7/8 FSV tasks as pending tools ship."
---

# Phase 7 MCP-tools TASK file audit (agent-210)

## Scope

Prepended the verbatim "🔍 AI-Agent Implementation Mandate (2026-05-10)" block from `memory/handoff/agent-208-coordinator--ai-agent-mandate-block-template.md` to all 7 TASK-MCP-*.md files in `docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/`, and added a per-file `## 📋 2026-05-10 STATE AUDIT — REQUIRES REWORK` sub-section that documents the actual MCP tool surface vs. what each task assumes.

## Reality probed

- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` — flat file, 5 shipped handlers: `call_mejepa_verify` (~76), `call_mejepa_predict_latest` (~151), `call_mejepa_constellation_inspect` (~222), `call_mejepa_heal_status` (~273), `call_mejepa_rollback_to` (~308).
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` — 3 additional shipped handlers: `call_mejepa_load_learner_state` (~1753), `call_mejepa_routing_lookup` (~1795), `call_mejepa_vram_budget_report` (~1817).
- `crates/context-graph-mcp/src/tools/names.rs` — `MEJEPA_*` name constants (8 currently).
- `crates/context-graph-mcp/src/handlers/tools/mejepa/` — does NOT exist (sub-module tree was never created).
- `crates/context-graph-mejepa-shift-subscriber/` — does NOT exist (Phase 7 wiring).

## Tool-status matrix

SHIPPED (8): `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`.

PENDING (10): `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit` (Phase 7); `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress` (Phase 6); `mejepa_eval_run`, `mejepa_eval_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_active_learning_label` (Phase 8).

## Cross-cutting drift across all 7 tasks

1. Module structure assumption is wrong (sub-module tree vs. flat file).
2. Tool count assumption is wrong (5 in originals vs. 18 in post-pivot plan).
3. TASK-MCP-001's dispatcher skeleton was never built — the dispatch in the codebase routes through whichever existing `handlers/tools/mod.rs` mechanism is in place, not the planned `mejepa::dispatch`.
4. 2 of the 5 original Phase 7 tools (`verify`, `predict_latest`) are already shipped (in their flat-file form); 3 (`observe_shift`, `subscriber_status`, `capture_audit`) remain blocked on Phase 7 shift-subscriber crate.

## Recommended downstream action

Re-author tasks 001-007 to reflect the flat-file layout, expand scope to 18 tools across Phase 6/7/8, and split TASK-MCP-007 into per-phase FSV evidence tasks.
