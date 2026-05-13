---
type: deletion_list_audit
status: complete
task: TASK-LOOP-008
issue: 136
created: 2026-05-13
---

# Phase H Deletion List Audit

## Summary

Audit source: `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md`.

Every directive below is marked `APPLIED` or `SKIPPED` with a reason. `SKIPPED` means the deletion list itself says the item should be kept, retained as historical provenance, or is outside the active forward runtime. Unresolved count: 0.

Tracked commit: `6260bdf0` (`docs(loop): audit deletion list closure`).
FSV artifact: `/mnt/d/contextgraph/fsv/phase-h-deletion-list-audit-fsv/deletion_list_audit_fsv.json`.
FSV sha256: `9450ba900c49669728235f395e74b03ed0faa7ecef97fcebbc34c6d1f935a171`.
Final readback: `build_release_sha=6260bdf0`, `all_passed=true`, `readback_equal=true`.

## Audit Entries

| ID | Deletion-list directive | Result | Reason / source of truth |
|---|---|---|---|
| DL-001 | Local Qwen3.6 GGUF model | APPLIED | Absent by `scripts/audit/no_inner_actor_models.sh`; FSV #134. |
| DL-002 | Local Hermes GGUF model | APPLIED | Absent by `scripts/audit/no_inner_actor_models.sh`; FSV #134. |
| DL-003 | Local Qwen3-Coder GGUF model | APPLIED | Absent by path-name audit; FSV #134. |
| DL-004 | Orphaned scripts/config retired local LLM references | APPLIED | `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh`; active disallowed count 0; FSV #140. |
| DL-005 | `config/ccreality_model_pins.json` | APPLIED | File is absent from `config/`. |
| DL-006 | `crates/context-graph-cli/src/bin/reality-loop.rs` | APPLIED | Source file absent; Cargo metadata has no `reality-loop` bin; FSV #135. |
| DL-007 | `target/release/reality-loop` | APPLIED | Stale release binary absent and isolated release build does not produce it; FSV #135. |
| DL-008 | `mcp__cgreality__reality_run_attempt` | APPLIED | Retired compatibility endpoint returns `CCREALITY_ENGINE_RETIRED`; FSV #133 and #139. |
| DL-009 | `cc-reality-harness-editor` agent | APPLIED | `.claude/agents/cc-reality-harness-editor.md` frontmatter has `status: retired`; FSV #138. |
| DL-010 | `cc-reality-diagnostician` agent | APPLIED | `.claude/agents/cc-reality-diagnostician.md` frontmatter has `status: retired`; FSV #138. |
| DL-011 | `cc-reality-reward-analyst` agent | APPLIED | `.claude/agents/cc-reality-reward-analyst.md` frontmatter has `status: retired`; FSV #138. |
| DL-012 | `cc-reality-source-of-truth-readback` agent | SKIPPED | Kept active by deletion-list rule as generic post-edit source-of-truth verifier. |
| DL-013 | `cc-reality-swe-failure-triage` agent | SKIPPED | Kept active by deletion-list rule for capture-mode SWE-bench evaluator triage. |
| DL-014 | Path-policy gate code | SKIPPED | Kept in gutted form; active code rejects only empty/control-character paths. |
| DL-015 | Pre-edit hook governance script | SKIPPED | Kept because SHA provenance recording is still load-bearing. |
| DL-016 | Old Mode A/B docstrings in `.claude/loop.md` | APPLIED | `.claude/loop.md` is the post-pivot canonical loop content from `14_NEW_LOOP_MD.md`. |
| DL-017 | Per-attempt wallclock budget | APPLIED | Inner-actor scoped implementation is absent; legacy SMAP decisions superseded in #132. |
| DL-018 | Champion/promotion of best inner attempt | APPLIED | Inner-attempt champion path is retired; remaining champion APIs are ME-JEPA model-variant concepts. |
| DL-019 | Tool-result history cap | SKIPPED | Kept because prediction-injection/history-size control remains relevant. |
| DL-020 | `agent-77` inner-actor model swap decision | APPLIED | `status: superseded`, `superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md`; FSV #132. |
| DL-021 | `agent-79` tool-result history cap decision | SKIPPED | Kept active because the size-cap concept still applies outside the retired local actor. |
| DL-022 | `agent-73` wallclock budget raise decision | APPLIED | Superseded by the single-outer-loop pivot decision; FSV #132. |
| DL-023 | `agent-58` post-apply action message decision | APPLIED | Superseded by the single-outer-loop pivot decision; FSV #132. |
| DL-024 | `agent-47` per-attempt wallclock budget decision | APPLIED | Superseded by the single-outer-loop pivot decision; FSV #132. |
| DL-025 | `agent-42` nvidia-smi N/A tolerance decision | SKIPPED | Kept because VRAM telemetry remains active. |
| DL-026 | `agent-111` fire-and-forget decision | APPLIED | Superseded by the single-outer-loop pivot decision; FSV #132. |
| DL-027 | `agent-72` Docker permission blocker | SKIPPED | Kept if active because Docker/SWE-bench capture remains relevant. |
| DL-028 | `memory/context/current-phase.md` inner-actor state | APPLIED | Rewritten to current Phase G/H post-pivot state during #136. |
| DL-029 | `memory/reports/agent-*--turn-*` | SKIPPED | Kept append-only per SMAP provenance rule. |
| DL-030 | `docs/ccreality/` historical docs | SKIPPED | Kept as historical reference by deletion-list rule. |
| DL-031 | `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md` | SKIPPED | Kept because the reality-shift principle still applies. |
| DL-032 | Historical Mode A/B planning docs | SKIPPED | Kept as historical context; no retroactive rewrite. |
| DL-033 | `docs/ruvectorfindings/` design docs | SKIPPED | Kept because futurebuild extends them. |
| DL-034 | `docs/ruvectorfindings/specs/` hierarchy | SKIPPED | Kept as canonical shipped substrate specs. |
| DL-035 | `docs/futurebuild/` | SKIPPED | Kept as new forward canon. |
| DL-036 | Retire `mcp__cgreality__reality_run_attempt` | APPLIED | Compatibility endpoint is unadvertised from active tools and fails closed; FSV #133/#139. |
| DL-037 | Retire legacy `mcp__cgreality__reality_*` tools outside active surface | APPLIED | Five retired compatibility names registered only in deprecation layer and fail closed; FSV #133/#139. |
| DL-038 | Champion/ChampionState types | SKIPPED | Kept only where used by ME-JEPA model-variant champion tools. |
| DL-039 | `mcp__cgreality__champion_state_get/promote` | SKIPPED | Kept and repurposed for ME-JEPA model variant pinning. |
| DL-040 | CLI `reality-loop` subcommand if present | APPLIED | No `reality-loop` Cargo target or CLI binary source remains; FSV #135. |
| DL-041 | `/ccreality-iterate` skill | APPLIED | `.claude/skills/ccreality-iterate/SKILL.md` now has `status: retired` and points to `.claude/loop.md`. |
| DL-042 | `/ccreality-smoke` skill | SKIPPED | Kept and already repurposed as post-pivot retirement smoke. |
| DL-043 | `/ccreality-trigger-check` skill | APPLIED | `.claude/skills/ccreality-trigger-check/SKILL.md` now has `status: retired`. |
| DL-044 | `/ccreality-readback` skill | SKIPPED | Kept but updated so it no longer suggests retired iteration skills. |
| DL-045 | `/loop` skill/content | APPLIED | `.claude/loop.md` is canonical post-pivot loop content. |
| DL-046 | `.claude/settings.json` SessionStart text | APPLIED | Hook points at `scripts/cc/hook_session_start.sh`, which emits active runtime root context. |
| DL-047 | UserPromptSubmit prediction injection | APPLIED | Settings run both prompt-submit hook and `hook_user_prompt_submit_inject_prediction.sh`. |
| DL-048 | PreToolUse Edit/Write hook | SKIPPED | Kept for pre-edit SHA recording. |
| DL-049 | PostToolUse Edit/Write hook | SKIPPED | Kept for shift-log append and post-edit verification. |
| DL-050 | Stop hook | SKIPPED | Kept because `optimizer_record_recommendation` remains required. |
| DL-051 | PreCompact hook | SKIPPED | Kept for SMAP narrative dump. |
| DL-052 | `tasks/swebench-lite/*.json` | SKIPPED | Kept because it feeds Phase 0 corpus and capture-mode work. |
| DL-053 | Other `tasks/` files | SKIPPED | Kept unless future task-specific review finds obsolete workstream files. |
| DL-054 | CLAUDE.md operator-driven skills text | APPLIED | No active `ccreality-trigger-check`/old skill guidance remains in CLAUDE.md. |
| DL-055 | CLAUDE.md support pointer | APPLIED | Support section points at futurebuild plan. |
| DL-056 | CLAUDE.md directory layout | APPLIED | Directory layout includes `docs/futurebuild/`. |
| DL-057 | CLAUDE.md inner-loop references | APPLIED | Remaining references are retirement/deny/fail-closed guidance only; FSV #137. |
| DL-058 | `.claude/agents/README.md` harness-editor row | APPLIED | Roster marks retired. |
| DL-059 | `.claude/agents/README.md` diagnostician row | APPLIED | Roster marks retired. |
| DL-060 | `.claude/agents/README.md` reward-analyst row | APPLIED | Roster marks retired. |
| DL-061 | `.claude/agents/README.md` source-of-truth-readback row | SKIPPED | Kept active for generic source-of-truth verification. |
| DL-062 | `.claude/agents/README.md` swe-failure-triage row | SKIPPED | Kept active for capture-mode triage. |
| DL-063 | All other agents | SKIPPED | No deletion-list action required; outside ccreality retired helper set. |
| DL-064 | Migration step apply `14_NEW_LOOP_MD.md` | APPLIED | `.claude/loop.md` is post-pivot canonical content. |
| DL-065 | Migration step update CLAUDE.md and roster | APPLIED | Covered by #137 and #138. |
| DL-066 | Migration step mark retired agents | APPLIED | Covered by #138. |
| DL-067 | Migration step supersede SMAP decisions | APPLIED | Covered by #132. |
| DL-068 | Migration step delete or retire unused MCP tools | APPLIED | Covered by #133 and #139. |
| DL-069 | Migration step confirm model file absence | APPLIED | Covered by #134. |
| DL-070 | Migration step delete stale release binary | APPLIED | Covered by #135. |
| DL-071 | Migration step C-drive lint | APPLIED | Covered by `TASK-OBS-005`, complete 2026-05-12. |
| DL-072 | Active embedders not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-073 | Instrument slots not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-074 | Live ME-JEPA MCP tools not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-075 | Python mutation operators not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-076 | Phase 0-8 substrate not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-077 | Witness chain/signing/Merkle compression not touched | SKIPPED | Explicit keep boundary; no Phase H change. |
| DL-078 | `.claude/` hooks layer not deleted | SKIPPED | Kept and extended by prediction-injection path. |
| DL-079 | SMAP memory protocol not deleted | SKIPPED | Memory tree retained; targeted stale current-phase context updated. |
| DL-080 | `docs/ccreality/` not deleted | SKIPPED | Kept as historical reference. |
| DL-081 | `docs/ruvectorfindings/` not deleted | SKIPPED | Kept as accurate load-bearing design docs. |

## Deterministic Spot Checks

The FSV uses seed `TASK-LOOP-008-2026-05-13` and checks these three deletion-list entries:

1. `DL-005`: `config/ccreality_model_pins.json` is absent.
2. `DL-008`: the retired MCP compatibility registry includes `mcp__cgreality__reality_run_attempt` and the retired response source contains `CCREALITY_ENGINE_RETIRED`.
3. `DL-041`: `/ccreality-iterate` has retired skill frontmatter and no active retired-tool workflow body.

## Verification

FSV artifact path after commit rerun: `/mnt/d/contextgraph/fsv/phase-h-deletion-list-audit-fsv/deletion_list_audit_fsv.json`.
