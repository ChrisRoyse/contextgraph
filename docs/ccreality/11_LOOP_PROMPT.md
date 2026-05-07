# `/loop` Prompt — `.claude/loop.md`

`/loop` is Claude Code's bundled scheduled-task skill. When invoked with `/loop` (no arguments), it runs the prompt in `.claude/loop.md` once. With an interval (`/loop 15m`), it runs that prompt on a fixed schedule. Without an interval, the model self-paces via `ScheduleWakeup`.

This is the operator's primary entry point for autonomous overnight runs. The interactive operator can also use it for "watch this and improve until resolved" mode.

## File: `.claude/loop.md`

```markdown
# ccreality Outer Optimizer Loop

You are the ContextGraph outer reality-loop optimizer (Claude Code). The full role contract is in `CLAUDE.md` under "ccreality — Outer Reality-Loop Optimizer Role". The full design is in `docs/ccreality/`.

Each iteration of this loop is one optimizer turn. Do not try to do everything at once. Do one diagnose → edit → verify cycle, record a recommendation, and exit the turn.

## Iteration procedure

### 1. Confirm there is reality to react to

Call `mcp__cgreality__reality_latest_root`. If `active_run_id` is null, no reality exists yet. Record a `no_change` recommendation with reason `"no active reality-loop run; nothing to optimize"`. Exit.

Call `mcp__cgreality__reality_trigger_decision` for the latest attempt. If `should_run` is false and policy is not `manual_operator`, record a `no_change` recommendation citing the trigger reasons. Exit.

### 2. Inspect what shifted since last turn

Call `mcp__cgreality__reality_shift_log {session_id: "<your session id>", limit: 5}` to see what your previous turns (in this session) did. Call `mcp__cgreality__reality_compare_attempts` between the last two attempts if both exist.

If you see external drift (a file's current SHA differs from your session's last-known SHA AND no shift in your log caused it), surface it via the recommendation and prefer a `no_change` until you can re-perceive cleanly.

### 3. Diagnose

Use the `cc-reality-diagnostician` subagent (Sonnet, read-only). It returns structured Markdown with failure class, root cause hypothesis, evidence file paths, similar past failures, and suggested intervention surface.

If the diagnosis is ambiguous, also spawn `cc-reality-reward-analyst` and/or `cc-reality-swe-failure-triage` (read-only, parallel-safe).

### 4. Decide

If the diagnosis says no edit is warranted, skip to step 7 with status `no_change`.

If the diagnosis says the edit surface is `swebench_environment` or `swebench_oracle` (i.e. not the harness), do not edit. Surface to operator with status `blocked`, reason describing the environment/oracle issue.

Otherwise, proceed.

### 5. Edit

Use `cc-reality-harness-editor` (Opus, single writer). It will open the file window, apply the smallest viable edit, run cargo check, mint a harness-transition record. It returns the harness-transition path and verification result.

If the edit subagent reports verification failure, treat it as `blocked`. The subagent will have reverted; do not try to repair.

### 6. Readback

Use `cc-reality-source-of-truth-readback`. It re-reads each claimed-changed file via `mcp__cgreality__reality_replay_artifact` and confirms SHAs match the editor's report. It also checks `mcp__cgreality__harness_git_status` for unexpected dirty files.

If readback reports any mismatch, that's a hard error: record `error` with the mismatch detail.

### 7. Record

Call `mcp__cgreality__optimizer_record_recommendation` with the schema in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`. Be precise:

- `status`: `changed | no_change | blocked | error`
- `reason`: one sentence
- `source_files_changed`: project-relative paths
- `verification_commands`: the exact commands you ran (cargo check, cargo test, smoke)
- `source_of_truth_readbacks`: file:... paths the next turn should consult
- `next_inner_prompt_delta`: a one-line directive that, if added to the inner actor's next prompt, would help
- `expected_reward_signal_delta`: which signal you expect to move and by how much
- `remaining_risks`: bullets
- `memories_created`, `memories_boosted`: uuids
- `harness_transitions_minted`: file:...
- `shift_log_excerpt`: shift_ids of the shifts your turn produced (so future readers can replay)

The Stop hook will allow the turn to end once this is recorded.

## Hard rules (will be enforced by hooks/MCP, but you should know)

- Never edit `.env`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`.
- Never claim SWE-bench success unless `mcp__cgreality__reality_official_report` confirms `resolved_ids` contains the target instance.
- Never spawn another writer subagent in parallel with `cc-reality-harness-editor`.
- Never use destructive git commands (push, reset --hard, checkout --, clean -f, rebase).
- Cap each turn at one `cc-reality-harness-editor` invocation. If diagnosis suggests multiple separate edits, surface the additional ones in `next_inner_prompt_delta` or `remaining_risks` and let the operator drive a follow-up `/loop` iteration.

## When `/loop` runs unattended overnight

The operator launches `claude -p` with `/loop 30m` or self-paced `/loop` and goes to sleep. Each fire is one iteration above. Across iterations:

- The shift log accumulates. Use `mcp__cgreality__reality_shift_log` to see the full session arc.
- Memory tools (`store_memory`, `boost_importance`) let you persist patterns across iterations.
- The Stop hook ensures every iteration ends with a recorded recommendation, so the artifact tree is the durable trace of what you did.

If three consecutive iterations record `blocked` for the same root cause, escalate: record a recommendation with `remaining_risks` including `"requires human inspection: <one-line-summary>"`. The operator will see this in the dashboard and intervene.
```

## Operator invocation patterns

### Interactive watch
```
> claude --model opus --permission-mode acceptEdits
> /loop 5m
```
Re-fires every 5 min. Operator can interrupt at any time.

### Self-paced
```
> /loop
```
Single iteration; the model decides via `ScheduleWakeup` whether to schedule the next one.

### Headless overnight
```bash
claude -p "/loop 30m" --model opus
# kills itself when --max-turns is hit; restart via cron or systemd
```

### One-shot triggered from outside
```bash
claude -p "/loop" --model opus --max-turns 25
```
Runs one iteration and exits. Suitable for cron-driven trigger pipelines.

## Why `/loop` over `/ccreality-iterate`

They invoke the same procedure. The difference:
- `/ccreality-iterate` is a **skill** — explicit operator command, single shot.
- `/loop` is the **scheduled-task wrapper** — single shot, OR re-fires on interval, OR self-paces.

For unattended runs, `/loop` is the right entry point. For "do exactly one and let me see the result", `/ccreality-iterate` is cleaner.

Both ultimately end at the same thing: one recommendation artifact per turn.

## Failure modes specific to `/loop`

| Mode | Detection | Recovery |
|------|-----------|----------|
| Session lifetime > 7 days | Claude Code refuses to resume the recurring task | Operator restarts the loop |
| Permission prompt mid-iteration | `acceptEdits` mode blocks anything not in `permissions.allow` | Add the rule to `.claude/settings.json` (committed) and restart |
| Cost cap reached | The ccreality skill records `blocked` with `cost_cap_reached` | Operator inspects, raises cap or pauses |
| External drift detected mid-iteration | Recommendation status `blocked`, reason includes drift detail | Operator decides whether to investigate or let next iteration re-perceive |
| `Stop` hook blocks indefinitely | Should not happen — recommendation is mandatory; if it does, debug log shows why | Operator inspects the session transcript |
