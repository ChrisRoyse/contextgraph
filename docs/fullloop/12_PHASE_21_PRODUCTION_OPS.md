# Phase 21 — Production Ops (External Watcher, Alerts)

**Goal:** the loop runs unattended for hours/days. Operator is paged only on real escalations.
**Effort:** 1 day.
**Risk:** low — packaging existing artifacts behind a small watcher script.

## External-watcher cron

Per `docs/ccreality/14_TRIGGER_AND_INVOCATION_POLICY.md` Section E, a small shell script invokes Claude Code single-shot when there is reality to react to.

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

## No spend caps

The system runs locally on operator hardware; there are no per-turn or per-day spend limits. The watcher script does NOT enforce any cost cap.

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
- `Edit/Write` is allowed but PreToolUse hook records SHAs (the deny gate was removed 2026-05-07; path discipline is behavioral).
- `WebFetch` and `WebSearch` are NOT allowed in unattended mode.
- `Stop` hook blocks until `optimizer_record_recommendation`.

## What "running smoothly overnight" looks like

After a 12-hour overnight run with the watcher firing every 30 min:
- 24+ optimizer turns recorded.
- 5–10 harness edits applied (and verified).
- 1–3 candidate champions proposed.
- Dashboard reality_score has trended upward.
- Zero `ANTHROPIC_API_KEY` errors, zero hook failures.

If any of those go sideways, the operator inspects the latest recommendation and takes manual action.

## AI-verifiable end-state for Phase 21

```bash
test -x scripts/cc/cgreality-tick.sh
bash -n scripts/cc/cgreality-tick.sh

# simulate the watcher firing without Claude Code attached:
SIMULATE_NO_CLAUDE=1 bash scripts/cc/cgreality-tick.sh
# expected: exits 0; writes a noop trace artifact under <runtime_root>/cgreality-watcher/
```

## Out of scope

- Cloud-hosted scheduler (Anthropic-side `/schedule`, Vercel cron, etc.). Local systemd is the supported path.
- Alerting transports beyond writing `alerts/*.json`. Operator chooses transport.
- Multi-host orchestration.
