# Trigger & Invocation Policy

Defines when Claude Code wakes up, who pulls the trigger, and how the optimizer is launched. Replaces the Rust-side `decide_deepseek_optimizer_trigger` function (deleted per [04_EXCISION_PLAN](04_EXCISION_PLAN.md)) with a model that puts the decision where the decision is made: in Claude Code itself, recorded via MCP.

## Core distinction: trigger ≠ invocation

- **Trigger** = the policy decision that an optimization turn *should* run for a given attempt (e.g. "official SWE not resolved + reward regressed → trigger fires").
- **Invocation** = the mechanism that actually starts a Claude Code session (operator types `/loop`, or cron runs `claude -p "/loop"`, or the engine writes a trigger artifact and an external watcher acts on it).

These are decoupled. The trigger artifact can be written without Claude Code running; an invocation can happen without a fresh trigger. Both events are recorded.

## Trigger policies

Same set as before excision, with an added policy for the new control flow:

| Policy | Fires when |
|--------|------------|
| `disabled` | Never. Operator handles all optimization manually. |
| `every_attempt` | After every attempt completes, regardless of outcome. |
| `failure` | After any attempt where official SWE-bench did not resolve OR local validation failed OR a repeated patch was detected. |
| `failure_or_regression` | (default) `failure` policy + reward regression beyond threshold + reward unavailability. |
| `manual_operator` | Operator-driven; trigger only when operator runs `/ccreality-iterate` or `/loop`. The MCP `optimizer_record_decision` tool sets `should_run: true` regardless of signals. |

Configured per-run via the engine's `--optimizer-trigger-policy` flag. Default `failure_or_regression`. The engine writes the policy into the run-summary; the MCP server reads it when minting the trigger decision.

## Where the decision is made

**Old flow (DeepSeek era):** the engine evaluated signals at end of attempt, wrote `deepseek-optimizer-trigger-decision.json`, then conditionally spawned the OpenAI/DeepSeek HTTP call.

**New flow (ccreality):** the engine just writes the raw signals into `attempt-summary.json` and `reality-signal-packet.json`. When Claude Code starts an optimization turn, the FIRST thing the `/loop` skill does is call `mcp__cgreality__optimizer_record_decision`, passing in:
- The attempt to react to (latest by default).
- The configured policy (read from the run-summary).
- The agent's calculated `should_run` boolean (computed from the signals via the same logic as the old Rust function, now lifted into the MCP server).
- The reasons.

The MCP server writes the decision to `<runtime_root>/<task>/attempt-N/trigger-decision.json` AND `<runtime_root>/<task>/attempt-N/claude-code-optimizer/decision-turn-NN.json`. Both with the same content; the latter is per-turn so multiple turns get separate records.

## Why move the decision

1. **Symmetry with `optimizer_record_recommendation`**: every optimizer artifact (decision and recommendation) is now produced by the same MCP layer. Easier to audit.
2. **Bypass-able from the operator side**: when the operator uses `manual_operator` policy, the decision tool simply sets `should_run: true`. No special engine path needed.
3. **Allows model-aware decisions**: the MCP server can incorporate per-attempt context the engine can't easily compute (e.g. "the previous turn's recommendation said `next_gemma_prompt_delta` was X; did the engine actually run with that delta? if not, don't trigger again").

## Invocation methods (in order of preference)

### A. Interactive operator session (always available)

The operator runs:
```bash
claude --model opus --permission-mode acceptEdits
```

Then types `/ccreality-iterate` or `/loop 5m` interactively. SessionStart hook injects the runtime root. UserPromptSubmit injects trigger info. Operator watches.

This is the **default** for development and debugging.

### B. Self-paced single-shot

Operator (or a cron job) runs:
```bash
claude -p "/ccreality-iterate" --model opus
```

Single turn. Recommendation is recorded. Process exits.

### C. Self-paced loop (overnight)

Operator runs:
```bash
claude -p "/loop" --model opus --max-turns 50
```

`/loop` self-paces via `ScheduleWakeup`. Turn cap (`--max-turns`) bounds cost. Process exits when cap is hit.

### D. Fixed-interval loop (long-running watch)

Operator runs:
```bash
claude --model opus --permission-mode acceptEdits
> /loop 30m
```

Re-fires every 30 min in the operator's session. Best for "watch this overnight from my laptop" mode. Limited by Claude Code's 7-day recurring-task lifetime.

### E. External-watcher cron (production)

A systemd timer or cron runs every 30 min:
```bash
#!/usr/bin/env bash
# /usr/local/bin/cgreality-tick.sh
set -euo pipefail
ROOT="$HOME/contextgraph"
cd "$ROOT"
ACTIVE="$(cat ~/.cache/cgreality/active_runtime_root 2>/dev/null || echo '')"
if [ -z "$ACTIVE" ]; then exit 0; fi
LATEST_ATTEMPT="$(find "$ACTIVE" -name attempt-summary.json | sort -V | tail -n 1 || echo '')"
if [ -z "$LATEST_ATTEMPT" ]; then exit 0; fi
# Skip if a recommendation already exists for this attempt
ATTEMPT_DIR="$(dirname "$LATEST_ATTEMPT")"
if find "$ATTEMPT_DIR/claude-code-optimizer" -name 'recommendation-*.json' 2>/dev/null | grep -q .; then exit 0; fi
# Fire single-shot
exec claude -p "/ccreality-iterate" --model opus --max-turns 25
```

This is the **production** entry point. The watcher writes nothing; it just decides whether to spawn Claude Code. The decision artifact is written by the MCP server inside the spawned session.

The watcher is the only piece outside the trio (engine, MCP, operator). It is a 20-line shell script — simple enough to audit. Not committed to the project; lives in `/usr/local/bin/` or as a systemd unit on the operator's machine.

## Trigger evaluation pseudocode (lifted from old Rust into MCP)

```python
def should_optimize(attempt_summary, prev_attempt_summary, policy, threshold):
    officially_unresolved = attempt_summary.resolved is not True
    validation_failed = attempt_summary.validation.status != "passed"
    repeated_patch = attempt_summary.repeated_patch
    current_reward = scalar_from(attempt_summary.reality_signal_compact)
    prev_reward = scalar_from(prev_attempt_summary.reality_signal_compact) if prev_attempt_summary else None
    reward_regression = (prev_reward is not None
                         and current_reward is not None
                         and prev_reward - current_reward > threshold)
    reward_unavailable = current_reward is None

    if policy == "disabled":      return (False, [])
    if policy == "every_attempt": return (True, ["policy=every_attempt"])
    if policy == "failure":
        return (officially_unresolved or validation_failed or repeated_patch,
                reasons_from([officially_unresolved, validation_failed, repeated_patch]))
    if policy == "failure_or_regression":
        return (officially_unresolved or validation_failed or repeated_patch
                or reward_regression or reward_unavailable,
                reasons_from([...]))
    if policy == "manual_operator":
        return (True, ["policy=manual_operator"])
    raise ValueError("invalid policy")
```

This logic moves from `reality-loop.rs:decide_deepseek_optimizer_trigger` to the new MCP module's `optimizer_record_decision` handler.

## When NOT to trigger

- `should_run` returns false → record a `no_change` recommendation with the policy reasons; exit. Do not spend a turn on resolved attempts.
- Last attempt is still running → the engine writes an interim status; the MCP server detects it and refuses to record a decision (returns `CCREALITY_ATTEMPT_IN_PROGRESS`). The operator should wait.
- The latest official report already places the target instance in `resolved_ids` → record `no_change` with reason `"already resolved per official report; nothing to optimize"`.
- A previous turn's recommendation is `error` and the operator hasn't acknowledged → the MCP server writes the decision but flags it; the operator gets a notification.

## When to escalate

If three consecutive turns record `blocked` for the same diagnosis, the recommendation should include `"requires human inspection"` in `remaining_risks`. The operator's status board (or a manual `/ccreality-readback` invocation) surfaces this.

## Concurrency

Only one Claude Code session per runtime root at a time. Enforced by an exclusive flock on `~/.cache/cgreality/active_runtime_root.lock` taken at SessionStart hook. If a second session tries to start while the first holds the lock, SessionStart emits `additionalContext` saying "another optimizer session is active; this session will run read-only." The shift principle naturally tolerates this: the second session perceives the first's shifts via `reality_shift_log`.

## Auth

For headless/cron use:
- `ANTHROPIC_API_KEY` exported in the watcher's environment.
- The `.env` file is in the deny list, so Claude Code can never read it; the watcher reads its own credentials from `/etc/cgreality/credentials` (chmod 600, root:operator) or from the operator's shell profile.

For interactive use:
- `claude.ai` login (via `claude` first-launch flow). Subscription billing.

Both work. Interactive is preferred for development; API key for unattended runs because it's more deterministic.
