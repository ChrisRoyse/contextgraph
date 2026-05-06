# Phase 21 — Production Ops (External Watcher, Cost Cap, Alerts)

**Goal:** the loop runs unattended for hours/days. Operator is paged only on real escalations. Cost is bounded.
**Effort:** 1 day.
**Risk:** low — packaging existing artifacts behind a small watcher script.

## External-watcher cron

Per `docs/ccreality/14_TRIGGER_AND_INVOCATION_POLICY.md` Section E, a 20-line shell script invokes Claude Code single-shot when there is reality to react to.

File: `scripts/cc/cgreality-tick.sh` (committed; mode 0755).

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="$HOME/contextgraph"
cd "$ROOT"

ACTIVE="$(cat ~/.cache/cgreality/active_runtime_root 2>/dev/null || echo '')"
[ -n "$ACTIVE" ] || exit 0

LATEST_ATTEMPT="$(find "$ACTIVE" -name attempt-summary.json | sort -V | tail -n 1 || echo '')"
[ -n "$LATEST_ATTEMPT" ] || exit 0

ATTEMPT_DIR="$(dirname "$LATEST_ATTEMPT")"
# Skip if a recommendation already exists for this attempt
if find "$ATTEMPT_DIR/claude-code-optimizer" -name 'recommendation-*.json' 2>/dev/null | grep -q .; then
  exit 0
fi

# Cost cap check (env override)
TODAY_TOTAL_USD="$(./target/debug/context-graph-cli ccreality cost-summary --today --json | jq -r .total_usd 2>/dev/null || echo 0)"
DAILY_CAP="${CGREALITY_DAILY_COST_CAP_USD:-20}"
if (( $(echo "$TODAY_TOTAL_USD >= $DAILY_CAP" | bc -l) )); then
  echo "cost cap reached ($TODAY_TOTAL_USD >= $DAILY_CAP) — skipping" >&2
  exit 0
fi

# Fire single-shot
exec claude -p "/ccreality-iterate" --model opus --max-turns 25
```

Operator installs as a systemd timer (recommended) or cron entry:

```
# /etc/systemd/system/cgreality-tick.service
[Service]
Type=oneshot
WorkingDirectory=/home/cabdru/contextgraph
ExecStart=/home/cabdru/contextgraph/scripts/cc/cgreality-tick.sh
Environment=ANTHROPIC_API_KEY=...
Environment=CGREALITY_DAILY_COST_CAP_USD=20

# /etc/systemd/system/cgreality-tick.timer
[Unit]
Description=cgreality optimizer tick
[Timer]
OnCalendar=*:0/30
Persistent=true
[Install]
WantedBy=timers.target
```

`scripts/cc/cgreality-tick.systemd-example.{service,timer}` are committed alongside the script as templates. Real install lives in `/etc/systemd/system/` and is operator-side.

## Cost-summary CLI command

`context-graph-cli ccreality cost-summary` (new subcommand):

```
context-graph-cli ccreality cost-summary --today --json
```

Output:
```json
{
  "schema_version": 1,
  "window": "today_utc",
  "active_runtime_root": "/tmp/.../layer5-skill-runtime",
  "recommendation_count": 12,
  "total_usd": 4.27,
  "per_session": [
    { "session_id": "sess-...", "count": 5, "usd": 1.91 },
    ...
  ],
  "source_of_truth": "file:..."
}
```

Reads only `cost_estimate_usd` from each `recommendation-*.json` in the active runtime root. The number is what the model self-reported; not authoritative for billing but sufficient as a rate limiter.

## Alerts

`scripts/cc/cgreality-alert.sh` is invoked by the engine when:
- Three consecutive recommendations record `blocked` for the same diagnosis cluster.
- `incident_packet` count for one (model, task) exceeds 5.
- A single attempt's wall-clock exceeded 60 minutes.

The script does NOT itself send alerts; it writes `<runtime_root>/cgreality-alerts/<unix>-<reason>.json`. The operator's environment routes those to Slack / email / their preferred channel via a separate cron tail or webhook.

A new MCP tool `mcp__cgreality__alerts_list` lets the optimizer (and the dashboard) read the current alerts queue.

## Permission setup recap

Per `docs/ccreality/06_SETTINGS_JSON_SPEC.md`, `.claude/settings.json` already lists the allowed tools. Operator double-checks before going unattended:

- `mcp__cgreality__*` is allowed.
- `Bash` is allowed only for the deterministic command allowlist (cargo, rustfmt, git status/diff/log/show, ollama ps, nvidia-smi).
- `Edit/Write` is allowed but PreToolUse hook enforces governed-paths policy.
- `WebFetch` and `WebSearch` are NOT allowed in unattended mode.
- `Stop` hook blocks until `optimizer_record_recommendation`.

## What "running smoothly overnight" looks like

After a 12-hour overnight run with the watcher firing every 30 min:
- 24+ optimizer turns recorded.
- 5–10 harness edits applied (and verified).
- 1–3 candidate champions proposed.
- Dashboard reality_score has trended upward.
- Total cost under $20.
- Zero `ANTHROPIC_API_KEY` errors, zero hook failures.

If any of those go sideways (most likely: cost spike from a chatty turn), the operator inspects the latest recommendation and takes manual action.

## AI-verifiable end-state for Phase 21

```bash
test -x scripts/cc/cgreality-tick.sh
bash -n scripts/cc/cgreality-tick.sh

target/debug/context-graph-cli ccreality cost-summary --today --json | jq .total_usd

# simulate the watcher firing without Claude Code attached:
SIMULATE_NO_CLAUDE=1 bash scripts/cc/cgreality-tick.sh
# expected: exits 0; writes a noop trace artifact under <runtime_root>/cgreality-watcher/
```

## Out of scope

- Cloud-hosted scheduler (Anthropic-side `/schedule`, Vercel cron, etc.). Local systemd is the supported path.
- Alerting transports beyond writing `alerts/*.json`. Operator chooses transport.
- Multi-host orchestration.
