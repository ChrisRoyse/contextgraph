#!/usr/bin/env bash
#
# cgreality-tick.sh — Phase 21 external watcher
#
# Periodic tick (suggested cadence: every 30 min via systemd timer or cron).
# When called:
#   1. read the active runtime root from ~/.cache/cgreality/active_runtime_root
#   2. find the latest attempt-summary.json under it
#   3. skip if a recommendation already exists for this attempt
#   4. skip if today's optimizer cost exceeds the daily cap
#   5. otherwise launch a single-shot Claude Code optimizer turn
#
# Environment variables:
#   ANTHROPIC_API_KEY         — required for headless Claude Code (unless using
#                                operator's interactive login)
#   CGREALITY_DAILY_COST_CAP_USD — soft daily ceiling (default 20)
#   CGREALITY_MAX_TURNS         — single-shot turn cap (default 25)
#   CGREALITY_MODEL             — claude model id (default opus)
#   SIMULATE_NO_CLAUDE          — if set, skip the claude exec (for AI-verifiable
#                                  end-state)
#
# Source of truth on every decision:
#   ~/.cache/cgreality/active_runtime_root
#   <runtime_root>/<run>/<task>/<attempt>/attempt-summary.json
#   <runtime_root>/<run>/<task>/<attempt>/claude-code-optimizer/recommendation-*.json
#
# Exit codes:
#   0 — tick handled cleanly (whether claude was launched or skipped)
#   1 — broken environment or missing required tool

set -euo pipefail

ROOT="${CONTEXTGRAPH_ROOT:-$HOME/contextgraph}"
cd "$ROOT" || { echo "cgreality-tick: cannot cd to $ROOT" >&2; exit 1; }

ACTIVE_FILE="$HOME/.cache/cgreality/active_runtime_root"
[ -f "$ACTIVE_FILE" ] || { echo "cgreality-tick: no active runtime root" >&2; exit 0; }

ACTIVE="$(cat "$ACTIVE_FILE" 2>/dev/null | tr -d '[:space:]')"
[ -n "$ACTIVE" ] || { echo "cgreality-tick: active runtime root file empty" >&2; exit 0; }
[ -d "$ACTIVE" ] || { echo "cgreality-tick: $ACTIVE does not exist on disk" >&2; exit 0; }

LATEST_ATTEMPT="$(find "$ACTIVE" -name attempt-summary.json -type f 2>/dev/null | sort -V | tail -n 1 || echo "")"
[ -n "$LATEST_ATTEMPT" ] || { echo "cgreality-tick: no attempt-summary.json found" >&2; exit 0; }

ATTEMPT_DIR="$(dirname "$LATEST_ATTEMPT")"

# Skip if a recommendation already exists for this attempt
if find "$ATTEMPT_DIR/claude-code-optimizer" -name 'recommendation-*.json' -type f 2>/dev/null | grep -q .; then
  echo "cgreality-tick: recommendation already exists at $ATTEMPT_DIR/claude-code-optimizer" >&2
  exit 0
fi

# Cost cap — sum cost_estimate_usd across all recommendations under the active root
DAILY_CAP="${CGREALITY_DAILY_COST_CAP_USD:-20}"
TODAY_TOTAL_USD="$(
  find "$ACTIVE" -name 'recommendation-*.json' -type f -newermt "$(date -u +%F)" -print0 2>/dev/null \
    | xargs -0 -r jq -s 'map(.cost_estimate_usd // 0) | add // 0' 2>/dev/null \
    || echo 0
)"
# numeric guard against jq returning empty/null
TODAY_TOTAL_USD="${TODAY_TOTAL_USD:-0}"
if awk -v t="$TODAY_TOTAL_USD" -v c="$DAILY_CAP" 'BEGIN { exit !(t+0 >= c+0) }'; then
  WATCH_DIR="$ACTIVE/cgreality-watcher"
  mkdir -p "$WATCH_DIR"
  printf '{"event":"cost_cap_reached","today_total_usd":%s,"daily_cap_usd":%s,"unix":%s,"attempt":"%s"}\n' \
    "$TODAY_TOTAL_USD" "$DAILY_CAP" "$(date +%s)" "$ATTEMPT_DIR" \
    > "$WATCH_DIR/cost-cap-$(date +%s).json"
  echo "cgreality-tick: cost cap reached ($TODAY_TOTAL_USD >= $DAILY_CAP) — skipping" >&2
  exit 0
fi

# Persist a no-op trace for AI-verifiable simulation paths
WATCH_DIR="$ACTIVE/cgreality-watcher"
mkdir -p "$WATCH_DIR"
TICK_TRACE="$WATCH_DIR/tick-$(date +%s).json"
cat > "$TICK_TRACE" <<EOF
{
  "event": "tick_fired",
  "attempt_summary": "$LATEST_ATTEMPT",
  "today_total_usd": $TODAY_TOTAL_USD,
  "daily_cap_usd": $DAILY_CAP,
  "unix": $(date +%s),
  "simulate_no_claude": "${SIMULATE_NO_CLAUDE:-0}"
}
EOF

if [ -n "${SIMULATE_NO_CLAUDE:-}" ]; then
  echo "cgreality-tick: SIMULATE_NO_CLAUDE set — skipping claude exec; trace=$TICK_TRACE"
  exit 0
fi

if ! command -v claude >/dev/null 2>&1; then
  echo "cgreality-tick: claude CLI not on PATH" >&2
  exit 1
fi

MAX_TURNS="${CGREALITY_MAX_TURNS:-25}"
MODEL="${CGREALITY_MODEL:-opus}"

exec claude -p "/ccreality-iterate" --model "$MODEL" --max-turns "$MAX_TURNS"
