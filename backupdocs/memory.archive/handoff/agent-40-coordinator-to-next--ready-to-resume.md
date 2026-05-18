---
namespace: handoff
agent_id: agent-40-coordinator
timestamp: 2026-05-07T14:22:00Z
status: active
tags: handoff, ready-to-resume, fresh-tick, post-gate-removal
---

# Handoff: Ready to resume `/loop` on a fresh Claude Code instance

## TL;DR for the next agent

You are the FIRST `/loop` tick after a heavy infrastructure session. The reality loop is ready. **Drip a fresh attempt for `astropy__astropy-12907` and proceed normally.** Do not be surprised by the lack of a formal `recommendation-turn-40.json` — turn-40 was operator-driven infrastructure (path-policy removal + SMAP bootstrap + CLAUDE.md/loop.md refactor) and recorded in `./memory/reports/agent-40-coordinator--turn-40-final-report.md` instead.

## What's different from before this session

1. **Path-policy gate is FULLY REMOVED** at all 3 layers (hook, MCP server, CLI engine). You can edit any project-relative path that isn't in the §8 BEHAVIORAL deny set (CLAUDE.md). The MCP harness tools accept any project-relative path; the AI is trusted to honor the deny set.
2. **SMAP `./memory/` + `./agents/` is the team-side narrative**. Read `./memory/context/{mission-brief,current-phase,constraints}.md` first. Pair every recommendation JSON with a SMAP report at `./memory/reports/agent-{N}-coordinator--turn-{N}-final-report.md`.
3. **MCP singleton takeover (turn 39)** ensures `/mcp reconnect` cannot accumulate zombie MCPs.
4. **MCP→CLI shim (commits 4072dcf3 / d48102e6)** means changes to `crates/context-graph-cli/**` propagate to the next MCP harness call automatically — NO `/mcp reconnect` needed.
5. **CLAUDE.md and `.claude/loop.md` were both fully refactored** in this session. They are the canonical source for the role contract and the per-tick procedure. Read them.

## What you should do FIRST this turn

1. `mcp__cgreality__reality_latest_root` — confirm reality (active runtime root, run_id, last_attempt_number).
2. `cat ./memory/context/current-phase.md` — pick up where turn-40 left off.
3. Run the freshness probe (loop.md §4.6): `stat -c '%y %n' target/release/{context-graph-mcp,context-graph-cli,reality-loop}` + `ps -eo pid,etime,cmd | grep -E 'reality-loop attempt|context-graph-mcp' | grep -v grep`. Expect: 1 MCP, no engine attempts in flight, fresh binaries.
4. `ls /tmp/cgreality-prod-postfix-1778124110/rust_reality_1778158179562449993_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/` — confirm last-known agent-tool state (round-01 read_problem_reality completed; no round 2 minted before kill).

## Then evaluate SKIP conditions (§6.1 of loop.md)

None of these should apply on this tick:
1. All resolved? No — astropy__astropy-12907 has no champion yet.
2. Cost cap? Fresh session — no.
3. External drift? Run `mcp__cgreality__reality_shift_compare_to_my_view` to confirm.
4. Recent drip in flight? No engine PID alive (operator killed it earlier this session).
5. Trigger override? Check `mcp__cgreality__reality_trigger_decision` — should not block.

## Then drip + diagnose + edit

Per §6.2-6.9 of loop.md. The HIGHEST-PRIORITY queued harness fix is the **per-round wallclock cap** (see `current-phase.md` §"Queued harness work" #1). This is a generic intervention — the inner-actor silent-hang affects any SWE-bench task, so the fix is mergeable now.

If the diagnostician (after the new attempt completes) flags a different failure mode, address that instead. The wallclock cap is queued, not mandatory for this tick.

## Final action

Pair the formal `mcp__cgreality__optimizer_record_recommendation` (Stop-hook-enforced) with `./memory/reports/agent-{N}-coordinator--turn-{N}-final-report.md` (SMAP, honor-system).

## Things you do NOT need to worry about

- MCP zombies: takeover singleton handles this.
- Hook gate denying your edits: gone.
- MCP server rejecting harness paths: gone.
- CLI engine rejecting harness paths: gone.
- Whether to do `/mcp reconnect` after a `crates/context-graph-cli/**` rebuild: NO, the shim re-resolves the binary on every call.

## Things you DO still need to worry about

- Path discipline: don't edit `.env`, `swebench/**`, `tests/oracle_**`, reward specs, verifiers (BEHAVIORAL — CLAUDE.md §8).
- Single-writer rule: only `cc-reality-harness-editor` performs harness source edits, one invocation per turn.
- GENERIC-ONLY: every harness change must apply to ANY SWE-bench task. Mental check before merging.
- Cost cap: <$0.50 per turn.
- `/mcp reconnect` IS still required for MCP-protocol-layer changes (`crates/context-graph-mcp/src/handlers/`, `protocol.rs`, `tools/`, `main.rs`). Surface in `remaining_risks` when applicable.

Good hunting.
