---
namespace: agents
purpose: agent roster for the ccreality outer reality-loop optimizer pipeline
last_updated: 2026-05-07T13:50:00Z
---

# Agent Roster

This directory holds SMAP-style agent-role definitions for the ccreality reality-loop optimizer. Each `.md` file documents one role — its purpose, model, tool surface, and the SMAP namespaces it reads/writes.

The canonical Claude Code agent definitions live at `.claude/agents/cc-reality-*.md`. The files here are SMAP-style narrative descriptions intended for cross-agent orientation; they cross-reference the canonical definitions but do not replace them.

## Active roles

| Role | Subagent type | Model | R/W | Purpose |
|------|---------------|-------|-----|---------|
| coordinator | (Claude Code itself) | opus | RW | The outer reality-loop optimizer. Drives `/loop` ticks, dispatches subagents, records recommendations. Single entity — there is only one coordinator per session. |
| diagnostician | `cc-reality-diagnostician` | sonnet | R | Read-only failure-class triage. Returns structured Markdown describing root cause + intervention surface. Spawn via `Agent` tool. |
| harness-editor | `cc-reality-harness-editor` | opus | RW | The SINGLE writer for harness source edits. Opens window → applies smallest viable edit → cargo check → mints a harness-transition record. **Never spawn in parallel with another writer.** |
| readback | `cc-reality-source-of-truth-readback` | sonnet | R | Re-reads each claimed-changed file via `mcp__cgreality__reality_replay_artifact` and verifies SHAs match the editor's report. |
| reward-analyst | `cc-reality-reward-analyst` | sonnet | R | When reward signal is unavailable or grounding fails, proposes new reward features as a structured proposal. **Never edits.** |
| swe-failure-triage | `cc-reality-swe-failure-triage` | sonnet | R | Triages official SWE-bench Docker eval failures (Docker timeout, env issues, oracle mismatch, dependency problems). Read-only. |

## SMAP integration

When the coordinator runs an iteration, every spawned subagent inherits the SMAP discipline:

1. **READ**: peek at `./memory/` (context, decisions, discoveries, blockers, handoff) before starting actual work. Even read-only agents benefit from this.
2. **WORK**: write `.md` files into the right namespace as artifacts emerge.
3. **REPORT**: drop a final report into `./memory/reports/` with full disclosure of failures and successes — this is the LAST action.

The coordinator's recommendation (`recommendation-turn-NN.json`) is the **SWE-bench-side** audit trail; SMAP's `./memory/` is the **team-side** narrative. Both must agree but capture different cuts of the same reality.

## Spawning convention

Use Claude Code's `Agent` tool with `subagent_type` matching the canonical `.claude/agents/cc-reality-*.md` definitions. Each subagent inherits this project's CLAUDE.md and (now) the `./memory/` shared mind.

Naming convention for SMAP file paths inside this project:

- `agent-00-coordinator` — the outer optimizer (Claude Code itself).
- `agent-NN-{role}` — pattern for any other agent. `NN` is the per-session sequential turn number for that role.

## Files in this directory

- `README.md` (this file) — roster index and SMAP integration notes.

Add per-role narrative files (`coordinator.md`, `diagnostician.md`, etc.) only when the canonical `.claude/agents/cc-reality-*.md` lacks SMAP-specific guidance worth duplicating.
