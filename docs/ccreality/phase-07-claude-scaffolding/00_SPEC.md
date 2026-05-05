# Phase 7 — Specification

## Objectives

1. Replace the empty `.claude/settings.json` with the full ccreality config: model, permissions allow/deny, env vars, hooks block.
2. Create `.mcp.json` declaring the `cgreality` MCP server.
3. Drop `.claude/loop.md` with the operator loop prompt.
4. Drop 5 subagent files under `.claude/agents/`.
5. Drop 4 skill `SKILL.md` files under `.claude/skills/`.

After this phase, opening Claude Code in this project loads the entire ccreality runtime: MCP tools available, hooks firing, subagents discoverable, skills typeable.

## Scope

In scope: the 11+ files listed in [README.md](README.md).

Out of scope:
- The CLAUDE.md addendum (Phase 8).
- Any operator-local `settings.local.json` change (operator's choice).
- Any modification to existing `.claude/hooks/*.sh` (they belong to a different system).

## Deliverables

| File | Source spec | What it does |
|------|-------------|--------------|
| `.claude/settings.json` | [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) | Project Claude Code config: model, permissions, env, hooks |
| `.mcp.json` | [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) §".mcp.json" | Declares cgreality MCP server |
| `.claude/loop.md` | [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md) §"File: .claude/loop.md" | The /loop default prompt |
| `.claude/agents/cc-reality-diagnostician.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"1. cc-reality-diagnostician" | Read-only failure diagnoser |
| `.claude/agents/cc-reality-harness-editor.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"2. cc-reality-harness-editor" | Single source-editor |
| `.claude/agents/cc-reality-reward-analyst.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"3. cc-reality-reward-analyst" | Reward signal analyst |
| `.claude/agents/cc-reality-source-of-truth-readback.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"4. cc-reality-source-of-truth-readback" | SHA verifier |
| `.claude/agents/cc-reality-swe-failure-triage.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"5. cc-reality-swe-failure-triage" | Docker/eval triage |
| `.claude/skills/ccreality-iterate/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"1. ccreality-iterate" | One full optimizer turn |
| `.claude/skills/ccreality-smoke/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"2. ccreality-smoke" | Synthetic verification |
| `.claude/skills/ccreality-trigger-check/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"3. ccreality-trigger-check" | Read-only status |
| `.claude/skills/ccreality-readback/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"4. ccreality-readback" | Drift detection |

## Path-policy validation

Every file path the executing AI agent writes during this phase must be allowed by the governed-paths policy from Phase 5. Cross-check before writing:

- `.claude/settings.json` — allowed (`.claude/settings.json` is in allow-list... actually it isn't currently; we should add it). **Action:** verify the governed-paths policy includes `.claude/settings.json` and `.mcp.json` in the allow-list. If not, add them now (this is a policy update, not a Phase 7 file).

Actually, the agent doing this phase is human-driven (the operator setting up ccreality), not the optimizer Claude Code session itself. So path policy doesn't apply. But once ccreality is up and running, future optimizer turns CAN edit these files, so the allow-list should include them.

Update `config/ccreality_governed_paths.json` to include:
```
".claude/settings.json",
".mcp.json"
```
in the allow list. This is a small Phase 5 amendment, applied here.

## Time budget

1–2 hours, mostly transcription from the upstream specs:
- Tasks 7.1–7.3 (settings.json, .mcp.json, loop.md): 30 min.
- Task 7.4 (5 subagent files): 30 min.
- Task 7.5 (4 skill files): 30 min.
- Task 7.6 (verification + iteration on JSON syntax): 15 min.
