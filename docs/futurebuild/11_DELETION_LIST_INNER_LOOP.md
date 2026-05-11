# 11 — Deletion List: Inner-Loop Residue and Other Retirements

> **Purpose.** Concrete enumeration of code, configs, docs, hooks, agents, MCP tools, decision files, and skills that should be removed (or marked retired) so the codebase reflects the post-pivot single-outer-loop reality. Per the operator directive: *"we can actually remove any reality loop stuff all together. the reality loop system is solely to get training data for the me jepa system. … delete the inner loop entirely and everything having to do with it."*
>
> **Risk discipline.** Not everything on this list should be deleted with `git rm` — some items should be kept as historical artifacts with `status: superseded` per CLAUDE.md §6. The first column tells you which.

---

## 1. Decision rule

| Status | Meaning |
|---|---|
| **DELETE** | hard-remove from the working tree; the artifact has zero forward purpose AND no historical reference value |
| **SUPERSEDE** | mark `status: superseded` in frontmatter (or its in-doc analogue); KEEP for provenance per CLAUDE.md §6 |
| **RETIRE** | mark code path as fail-closed (`MEJEPA_*_RETIRED` error); keep the symbol so old callers get a clear error; do NOT delete |
| **KEEP** | retain as-is — already correct or still load-bearing |

---

## 2. Inner-actor LLM residue

| Item | Path | Action | Notes |
|---|---|---|---|
| Local Qwen3.6-27B GGUF model | `~/.cache/cgreality/models/` and any `models/qwen*` | already DELETED 2026-05-09 | confirm absence; no rebuild |
| Local Hermes-2-Pro GGUF | same as above | already DELETED 2026-05-09 | confirm |
| Local Qwen3-Coder-30B-A3B-MoE GGUF | per `memory/decisions/agent-77-...md` | already DELETED 2026-05-09 | confirm |
| `config/ccreality_model_pins.json` (if it still exists) | `config/` | DELETE | inner-actor pin file; no forward purpose |
| `crates/context-graph-cli/src/bin/reality-loop.rs` | crate bin | already DELETED 2026-05-09 | confirm absence; CLI inventory should not list `reality-loop` binary |
| `target/release/reality-loop` (build artifact) | `target/release/` | DELETE next clean rebuild | builds skip it; remove stale artifact |
| `mcp__cgreality__reality_run_attempt` MCP tool | `crates/context-graph-mcp/src/handlers/tools/reality_loop/` | already RETIRED (fails closed `CCREALITY_ENGINE_RETIRED`) | keep the failure path; do not re-enable |
| `cc-reality-harness-editor` agent | `.claude/agents/` | RETIRE (mark frontmatter `status: retired`) | agent definition can stay as historical doc; do not invoke |
| `cc-reality-diagnostician` agent | `.claude/agents/` | RETIRE | same |
| `cc-reality-reward-analyst` agent | `.claude/agents/` | RETIRE | same |
| `cc-reality-source-of-truth-readback` agent | `.claude/agents/` | KEEP — still useful for futurebuild source-of-truth verification | repurpose as a generic readback agent |
| `cc-reality-swe-failure-triage` agent | `.claude/agents/` | KEEP — useful for SWE-bench Docker eval triage in capture mode | scope changes from inner-actor to outer-Claude-Code attempts |

---

## 3. Path-policy / governance residue

| Item | Path | Action | Notes |
|---|---|---|---|
| Path-policy gate code in MCP server | `crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs::ensure_path_allowed` | KEEP gutted form | already gutted to "reject empty/control-char paths only" 2026-05-07; current state is correct |
| Pre-edit hook governance script | `scripts/cc/hook_pre_tool_use_edit.sh` | KEEP | hook records SHA for provenance; that is correct |
| Old Mode A/B docstrings in `loop.md` | `.claude/loop.md` | REPLACE per `14_NEW_LOOP_MD.md` | full file replacement |
| Per-attempt wallclock budget | `crates/context-graph-cli/...` | DELETE if remaining; otherwise no-op | inner-actor scoped; not applicable to outer Claude Code |
| Champion / promotion of "best inner attempt" | `crates/context-graph-...` | already retired | confirm absence |
| Tool-result history cap (4 KB) | applied to `UserPromptSubmit` injection | KEEP | applies to prediction-injection block; still relevant |

---

## 4. Memory / SMAP residue

The `memory/` SMAP tree has many decisions and reports that reference inner-actor concepts. Per CLAUDE.md §6 + §10.3, **do not delete** — mark with `status: superseded` and add a one-line `superseded_by:` pointer to the corresponding futurebuild decision.

| Item | Path | Action | Replacement reference |
|---|---|---|---|
| `agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md` | `memory/decisions/` | SUPERSEDE | `2026-05-09--single-outer-loop-mejepa-no-local-llm.md` |
| `agent-79-harness-editor--tool-result-history-cap-tightened-to-4kb.md` | `memory/decisions/` | KEEP | still applies to prediction-injection size |
| `agent-73-harness-editor--wallclock-budget-raise.md` | `memory/decisions/` | SUPERSEDE | n/a — wallclock concept retired |
| `agent-58-harness-editor--post-apply-action-message-clarification.md` | `memory/decisions/` | SUPERSEDE | apply-action protocol no longer exists |
| `agent-47-harness-editor--per-attempt-wallclock-budget.md` | `memory/decisions/` | SUPERSEDE | concept retired |
| `agent-42-harness-editor--nvidia-smi-na-sentinel-tolerance.md` | `memory/decisions/` | KEEP | nvidia-smi handling still applies to VRAM telemetry |
| `agent-111-coordinator--reality-run-attempt-fire-and-forget.md` | `memory/decisions/` | SUPERSEDE | fire-and-forget engine spawn retired |
| `agent-72-coordinator--docker-permission-denied-blocks-official-eval.md` | `memory/blockers/` | KEEP if active | Docker permission still relevant for capture mode |
| All `memory/context/current-phase.md` referencing inner-actor | `memory/context/` | UPDATE | rewrite per current state once daemon mode ships |
| All `memory/reports/agent-*--turn-*` | `memory/reports/` | KEEP | append-only; per CLAUDE.md §6 never delete |

---

## 5. Documentation residue

| Item | Path | Action | Notes |
|---|---|---|---|
| `docs/ccreality/` (21 docs describing pre-pivot two-loop architecture) | `docs/ccreality/` | KEEP as historical reference | per `CLAUDE.md §1` ("retained as historical reference"); do not edit |
| `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md` | `docs/ccreality/` | KEEP | the reality-shift principle is still operative; cited from new docs |
| Any `# Mode A` / `# Mode B` references in older planning docs | various | LEAVE for historical context | do not retroactively rewrite history |
| `docs/ruvectorfindings/` master plan + 09–16 hardening docs | `docs/ruvectorfindings/` | KEEP | accurate; futurebuild extends, doesn't replace |
| `docs/ruvectorfindings/specs/` (constitution, functional, technical, tasks) | `docs/ruvectorfindings/specs/` | KEEP | the canonical spec hierarchy for the shipped substrate |
| `docs/futurebuild/` (this directory) | `docs/futurebuild/` | KEEP — this is the new canon for forward work | |

---

## 6. Code paths to mark `MEJEPA_*_RETIRED`

| Symbol | Path | Action | Retirement code |
|---|---|---|---|
| `mcp__cgreality__reality_run_attempt` | MCP server | already RETIRED | `CCREALITY_ENGINE_RETIRED` |
| `mcp__cgreality__reality_loop_*` legacy tools (any not in the active 19-tool ME-JEPA surface) | MCP server | RETIRE | per-tool-specific code |
| Any `Champion` / `ChampionState` types still referenced | various | KEEP if used by `champion_state_get/promote` MCP tools | otherwise DELETE |
| `mcp__cgreality__champion_state_get` and `_promote` MCP tools | MCP server | KEEP — repurpose for ME-JEPA model variant pinning | the "champion" now refers to the active ME-JEPA model, not an inner-actor patch |
| `crates/context-graph-cli` `reality-loop` subcommand if present | CLI | DELETE | confirm absence |

---

## 7. Skills (`.claude/skills/`)

| Skill | Action | Notes |
|---|---|---|
| `/ccreality-iterate` | RETIRE — supersede with `/loop` | one canonical skill |
| `/ccreality-smoke` | KEEP — repurpose | now verifies daemon mode + shift subscriber + GC scheduler boot |
| `/ccreality-trigger-check` | RETIRE | inner-actor drip trigger; concept gone |
| `/ccreality-readback` | KEEP | drift detection on past recommendations is still useful |
| `/loop` | REFACTOR per `14_NEW_LOOP_MD.md` | the canonical per-tick skill |

---

## 8. Hook references in `.claude/settings.json`

| Hook | Action | Notes |
|---|---|---|
| `SessionStart` | UPDATE injection text | replace inner-actor language with "futurebuild active runtime root" |
| `UserPromptSubmit` | EXTEND with prediction-injection per `08_LIVE_PREDICTION_AGENT_API.md §3` | new feature |
| `PreToolUse Edit/Write` | KEEP | SHA recording still correct |
| `PostToolUse Edit/Write` | KEEP | shift-log append + cargo check still correct |
| `Stop` | KEEP | `optimizer_record_recommendation` requirement still correct |
| `PreCompact` | KEEP | SMAP narrative dump still correct |

---

## 9. Tasks manifest

| Item | Path | Action | Notes |
|---|---|---|---|
| `tasks/swebench-lite/*.json` (300 task manifests) | `tasks/swebench-lite/` | KEEP | feeds Phase 0 corpus + capture-mode work; never inner-actor-only |
| `tasks/` other | `tasks/` | review per file | most are workstream-scoped task lists; most KEEP |

---

## 10. CLAUDE.md updates

CLAUDE.md §11 already reflects the post-pivot single-loop architecture. Three small updates needed:

1. **§11.11 (Operator-driven skills)**: mark `/ccreality-trigger-check` as retired; mark `/loop` as canonical (already correct in spirit; tighten language).
2. **§13 (Support)**: replace `19_MIGRATION_RUNBOOK.md` reference with a pointer to `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` once the new plan is the working document.
3. **§3 (Directory layout)**: add `docs/futurebuild/` to the directory layout block.

These are 3-5 line edits; covered by `TASK-LOOP-002`.

---

## 11. `agents/README.md` updates

The SMAP roster doc lists agents. Update:

| Agent | Action |
|---|---|
| `cc-reality-harness-editor` | mark `status: retired`; keep as historical reference |
| `cc-reality-diagnostician` | mark `status: retired` |
| `cc-reality-reward-analyst` | mark `status: retired` |
| `cc-reality-source-of-truth-readback` | KEEP (still useful for source-of-truth verification) |
| `cc-reality-swe-failure-triage` | KEEP (repurposed for capture-mode SWE-bench triage) |
| All other agents | review per `agents/README.md`; most KEEP |

---

## 12. Migration order (do these in this sequence)

1. **Apply `14_NEW_LOOP_MD.md` content to `.claude/loop.md`** (`TASK-LOOP-001`).
2. **Update CLAUDE.md §11.11 + §13 + §3** (`TASK-LOOP-002`).
3. **Mark retired agents in `agents/README.md`** (part of `TASK-LOOP-002`).
4. **Mark superseded SMAP decisions** with frontmatter `status: superseded` + `superseded_by:` pointer (`TASK-LOOP-003`).
5. **Delete or retire unused MCP tools** (`TASK-LOOP-004`).
6. **Confirm absence of inner-actor model files on disk** (`TASK-LOOP-005`).
7. **Delete `target/release/reality-loop` if it lingers** (`TASK-LOOP-006`).
8. **Add C-drive lint to CI** to prevent regression (`TASK-OBS-005`).

Each task is small (5-30 minutes); they can be batched into a single PR titled "futurebuild: post-pivot residue cleanup".

---

## 13. What this deletion list does NOT touch

- The 13 active embedders (E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17). All KEEP.
- The 15 instrument slots. All KEEP.
- The 19 live ME-JEPA MCP tools. All KEEP.
- The 8 Python mutation operators. All KEEP.
- The shipped Phase 0–8 substrate. All KEEP.
- The witness chain + ed25519 signing + Merkle compression. All KEEP.
- The hooks layer at the `.claude/` level. KEEP (extend via `08_LIVE_PREDICTION_AGENT_API.md §3`).
- The SMAP shared-mind protocol. KEEP — all of `memory/` is the team narrative; CLAUDE.md §6 forbids deletion.
- The `docs/ccreality/` historical docs. KEEP — historical reference per CLAUDE.md §1.
- The `docs/ruvectorfindings/` design docs and specs. KEEP — accurate and load-bearing.

The deletion list is small and surgical. The futurebuild plan is mostly *additive*; the only removals are the dead inner-actor scaffolding and stale legacy tool surfaces.
