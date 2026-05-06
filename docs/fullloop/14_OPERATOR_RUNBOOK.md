# Operator Runbook — How to Actually Drive the Loop

This is the day-one + day-N field guide for the human operator. It is intentionally short: most of the work is delegated to /loop and the watcher.

## Prerequisites

- ContextGraph repo at `~/contextgraph`, branch `ccreality-cutover`.
- Rust toolchain installed (`rust-toolchain.toml`).
- Local Ollama with `gemma4:e4b` pulled (single-model loop; the previous multi-model plan was retired).
- Docker Desktop (or compatible daemon) for the official SWE-bench evaluator.
- ANTHROPIC_API_KEY exported in the shell where Claude Code or the watcher runs (unless using `claude.ai` interactive login).
- `claude` CLI installed.

Verify with:
```bash
cargo build --release -p context-graph-cli --bin reality-loop -p context-graph-cli --bin context-graph -p context-graph-mcp --bin context-graph-mcp
ollama list | grep gemma4
docker version
claude --version
```

## Day-1: smoke + first real attempt

### 1. Smoke

```bash
bash scripts/cc/tests/smoke.sh
```

All layers PASS → plumbing healthy.

### 2. Set the active runtime root

```bash
RUNTIME_ROOT=/tmp/ccreality-prod-2
mkdir -p ~/.cache/cgreality
echo "$RUNTIME_ROOT" > ~/.cache/cgreality/active_runtime_root

target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --runtime-root "$RUNTIME_ROOT" \
  --model gemma4:e4b \
  --official-eval-each-attempt \
  --max-attempts 1
```

### 3. Inspect

```bash
claude --model opus --permission-mode acceptEdits
# in the session:
> /ccreality-trigger-check
> /ccreality-iterate
```

Read the recommendation that lands at:
```
$RUNTIME_ROOT/<task>/<model>/attempt-000001/claude-code-optimizer/recommendation-turn-01.json
```

## Day-N: sustained `/loop`

### Interactive (operator watches)

```bash
claude --model opus --permission-mode acceptEdits
> /loop 15m
```

Re-fires every 15 min. Operator can interrupt at any time. Reads each recommendation as it lands.

### Headless overnight

Either:
```bash
claude -p "/loop 30m" --model opus --max-turns 50
```
…or install the systemd timer from Phase 21.

## What to watch

A healthy run at 24 hours:
- Reality-score (`<run_root>/reality-scoreboard/reality-scoreboard.md`) is up.
- Champion-state has ≥ 1 candidate champion.
- Attempt scoreboard shows one or more `official_resolved: true` rows.
- Cost: under daily cap (`context-graph-cli ccreality cost-summary --today`).

A struggling run at 24 hours:
- Same diagnosis cited 5+ times across recommendations → harness edits aren't sticking; manual inspection needed.
- Reality-score flat or declining → /ccreality-readback to confirm prior recommendations still match disk; potentially revert a kept experiment.
- Cost approaching cap → verify no recommendation is over-spawning subagents.

## Manual override patterns

### Pause the loop

Stop the timer / interactive session. Recommendations are append-only; nothing corrupts.

### Revert a kept experiment

```bash
# look up the experiment
mcp__cgreality__experiment_registry_list status_filter=kept

# inspect the recommendation that produced it
cat <run_root>/<task>/attempt-N/claude-code-optimizer/recommendation-turn-XX.json

# git revert the harness change (cite shift_log_excerpt)
git log --oneline | grep ccreality
git revert <commit>

# update the experiment registry
mcp__cgreality__experiment_registry_update_outcome experiment_id=exp-... outcome=discarded reasoning="post-hoc revert: <reason>"
```

### Force a fresh attempt without the optimizer

```bash
target/release/reality-loop attempt --runtime-root <root> --task-manifest <task> --max-attempts 1 --official-eval-each-attempt
```

The optimizer will see a new attempt next /loop tick.

## When to escalate to "stop autonomous run"

- Three consecutive `blocked` recommendations cite the same root cause.
- A single recommendation reports `external_drift_detected` against a file the operator did not edit.
- Cost cap reached unexpectedly (single-day spike).
- A new failure class appears that no diagnostician category matches.

In any case: stop the loop, inspect the latest 3 recommendations, fix root cause manually, restart.

## Where to look when things break

| Symptom | First file to read |
|---------|---------------------|
| Engine won't start | `<runtime_root>/failures/*.json` |
| `claude` CLI errors | session transcript at `~/.claude/projects/<project>/sessions/<session>.jsonl` |
| Hook errors | `<runtime_root>/cgreality-hook-events.jsonl` |
| MCP errors | `<runtime_root>/cgreality-mcp-events.jsonl` |
| Recommendation not landing | `<runtime_root>/<task>/<model>/attempt-N/claude-code-optimizer/` |
| Reward suspicious | `<runtime_root>/reality-optimizer/reward-shadow/correlation/*.json` |
| Cross-model behaving differently | `<runtime_root>/<task>/cross-model-summary.json` |

## Cardinal rules (operator-side)

1. Never edit `gemmafrontend/reward_spec.verified_outcome.json`. The shadow framework is the only path.
2. Never edit `swebench/`, `tests/oracle_**`, dataset fixtures, or anything else in the deny list.
3. Never `git push --force` or `git reset --hard` against `ccreality-cutover`.
4. Never claim resolution without `mcp__cgreality__reality_official_report` confirming `resolved_ids` contains the instance.
5. Always commit harness edits incrementally; never one giant commit.
