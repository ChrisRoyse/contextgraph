---
namespace: discoveries
agent_id: agent-211-task-updater
timestamp: 2026-05-10T00:00:00Z
status: active
tags: phase7, shift-subscriber, task-update, mandate-block, audit, drift
---

# Phase 7 shift-subscriber task-file mandate-block update + state audit

## Scope

Prepended the verbatim "🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)" block from `memory/handoff/agent-208-coordinator--ai-agent-mandate-block-template.md` to all 14 TASK-SHIFT-*.md files in `docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/`, immediately followed by a per-task `## 📋 2026-05-10 STATE AUDIT` subsection capturing drift findings against the current tree. The mandate block lands BEFORE the `<task_spec ...>` opening tag per coordinator instructions.

## Files updated (14)

| File | Pre-SHA | Post-SHA |
|------|---------|----------|
| TASK-SHIFT-001.md | a974866d0b70430020a145b13b2d27220f1806e1df17f5f36003fdbe1ed0c952 | c222dfb2c3b14befacf69a60e46fc82c82006efa30cc0eef5ad562e8e1a14096 |
| TASK-SHIFT-002.md | 3e79577476e6a24b5b0732879075005e644fb4ed4c44eb29c104de47be5b5e68 | 79654417d34ec28c78956a09418f41df2a7018bcba63f374203d21ee41d3fccc |
| TASK-SHIFT-003.md | 3b7fc13622a3d5ad7f93cca1bb260cd065ff84f71b5f69417336469e4160ab60 | 46a81f4f8c9359a955678baaaaf5880a1201f68517c29e22f993e3ebdc2fbe50 |
| TASK-SHIFT-004.md | ad0a223fda7affc94e7afcba3fdeb63eed3a317c7c7983bd2137cde6656faca3 | 9a24f41907d08b859573850fbc07b3a84301e474a872d076430d85d7e1bf836a |
| TASK-SHIFT-005.md | 2cd25a60fd9e4f6bd65010283101d9020c0021564b635a5fed8b34a02502d9ad | f006cef0f38857859545f79936cd79cc00938383c612b6514a1fe4cd9fc37363 |
| TASK-SHIFT-006.md | 5104fff0f1da8e8c99f49f9c0486a85609b6f3fd29e80b2ab69316c040222680 | fd9b7d2882a7bc74826e1b48113a7bcb63a8f2c5b4f9b9edff8907695a2f3c02 |
| TASK-SHIFT-007.md | 3b7795fcf88df11cf64cd60ea1fb8276b185d5e26b4011a3f168e10f2b21cb24 | 65b949b276f1a256d42ec8c9f7ccb8583e180fd81c749a0ca392741a43bfa48c |
| TASK-SHIFT-008.md | c4c9ed4f3897d14ed6e424233db52c36a826098b3a69257e841e94200b44b6cc | 1b3c93a28039081ca79be43bb14f6d6cf8a0ed9a8750e516d79e6ec07b5d967b |
| TASK-SHIFT-009.md | b9280cc077ab185a779b1f87371ee1d6baa639b176d8a2b8ddbeeba7f939f9a5 | 3a46d3a0d2a974f4acda9a9fda11d91391d7db2367b5337643d40a2b87258e62 |
| TASK-SHIFT-010.md | b22c97ac4301c849609deee5f874300ba3617e1ab5eacc1c642d5f03792ebbc8 | 3dcbabdd633d872ec05bd1ad96e4a0a8be029c0babac95ba6139919c9b15f930 |
| TASK-SHIFT-011.md | c146810c9b65ad2afb9845dec24ba149e1b5566c86ce2142241a782add24346e | 3e49ac5a0f47dea0a5b4894c99b965e55608cfc29d5a5cb27b702323dfe81e2f |
| TASK-SHIFT-012.md | 684c92de31bcf68f0e804617d0aaa2cc9d0f9f297937abf3883a275893a01a5b | d6279f579813d6aef85defa21ca5b1bee3e8366866acf3b2f659f542681d6070 |
| TASK-SHIFT-013.md | 5e396e77d80b5a8d9501fe97077273d1773f3a61aa36029794db5e9626b27b2e | 17b35d96ca54200413304f3996c03f9a6373c8ec44036d70aaa35b28b264e5a4 |
| TASK-SHIFT-014.md | 890d33f379dc4a48e538bd1b776f9cb0de4b5887a5c0b4c281f7ef6483b96fcd | 6185cb794ff5473ed1bd47d3b9284fec0061d172bf0bbe3858952811b0211021 |

## Drift findings (rolled up across 14 tasks)

1. **Subscriber crate is genuinely absent.** `crates/context-graph-mejepa-shift-subscriber/` does NOT exist on disk. The coordinator's pre-flagged finding (TASK-SHIFT-001..014 accurate) is confirmed. Every task that proposes "create new file under that crate" is still load-bearing — none of them have been pre-empted by a parallel commit.

2. **MCP tool surface partially shipped.** `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` (flat file, NOT a `mejepa/` sub-tree as some earlier specs assumed) already hosts 5 MCP tools — `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`. The 3 MCP tools the subscriber's downstream consumers expect (`mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`) are PENDING — their constants are absent from `crates/context-graph-mcp/src/tools/names.rs`. Flagged in TASK-SHIFT-001 / TASK-SHIFT-011 audit blocks.

3. **`retired.rs` does NOT exist yet.** `crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs` is absent. TASK-SHIFT-003's "verify or create" branch becomes "create". The retired-tool sentinel `CCREALITY_ENGINE_RETIRED` is already used elsewhere (per 2026-05-09 commits `77cd98c5` / `7a718785`), so the sentinel literal is shared — only the centralized handler module needs creating.

4. **Reality-shift JSONL writer SHIPPED.** `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs` exists and is the upstream producer the subscriber's tail-reader (TASK-SHIFT-005) will consume from. JSONL schema in that producer is the source of truth for `ShiftEntry` parsing — re-grep when implementing.

5. **Cross-cutting concrete types still stubbed.** `RealityPrediction`, `VerifyVerdict`, `OracleOutcome`, `AuxHeadPrediction`, `CounterfactualResult` remain `serde_json::Value` typedefs in TASK-SHIFT-002 — SPEC-INFERENCE / SPEC-PREDICTOR / SPEC-CORPUS concrete types have not shipped in their owning crates.

6. **`MeJepaCompiler::compile` trait surface PENDING.** TASK-SHIFT-014 expects `MeJepaCompiler::compile(&PatchBundle, &TaskContext)` per TASK-INFER-003; the closest current API is the `call_mejepa_predict_latest` MCP tool body — not a Rust trait method. Implementation of TASK-SHIFT-014 needs either a deterministic-verify shim or to wait on TASK-INFER-003.

7. **Phase 0 holdout materialization status unverified.** `./data/mejepa-corpus/holdout/` may not yet exist as an operator-facing directory; Phase 0 split shipped JSON assignments at `/tmp/contextgraph-mejepa-corpus-fsv/split-assignments.json`. TASK-SHIFT-014's verify CLI assumes the materialized directory — audit before running.

## Output policy honored

- Mandate block inserted verbatim at top of each file, before `<task_spec>`.
- Each audit subsection cites the specific evidence (paths, line numbers, commit SHAs) that justified the drift call.
- No `<task_spec>` body content was modified — only prepended sections were added.
- One Edit per file.
- All 14 files share the same mandate block bytes (modulo trailing audit subsection variance).
