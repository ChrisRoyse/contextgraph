#!/bin/bash
# Safe restart of the Context Graph daemon + vram_watchdog after a WSL reboot.
# Run this once after `wsl --shutdown` or host reboot, before launching Phase C/D/E.
#
# What it does:
#   1. Verifies Phase B output is still present on disk (ext4 survives reboot)
#   2. Verifies earlyoom + sysctl hardening is active
#   3. Cleans any stale PID/lock files (survives crash path too)
#   4. Starts daemon; blocks until listening on 3100 (max 5 min)
#   5. Starts vram_watchdog in background
#   6. Prints a one-line READY / NOT-READY verdict
#
# Exit 0 = ready for Phase C
# Exit 1 = something's broken; DO NOT launch Phase C

set -u

REPO=/home/cabdru/contextgraph
BIN="$REPO/target/release/context-graph-mcp"
PIDFILE="/tmp/context-graph-daemon.pid"
DAEMON_LOG="/tmp/cg-daemon.log"
WATCHDOG_LOG="/tmp/vram_watchdog.log"
PHASE_B_OUT="$HOME/.contextgraph/training/shakespeare_stylistic_tags.jsonl"
MAX_WAIT=300   # seconds to wait for daemon to bind 3100

ok()   { printf "\033[32m[ok]\033[0m %s\n" "$*"; }
warn() { printf "\033[33m[warn]\033[0m %s\n" "$*"; }
fail() { printf "\033[31m[fail]\033[0m %s\n" "$*"; exit 1; }

echo "=== Context Graph pipeline restart ==="
echo "time: $(date -Iseconds)"
echo ""

# --- 1. Phase B output present ---
if [ ! -f "$PHASE_B_OUT" ]; then
  fail "Phase B output missing: $PHASE_B_OUT — did the VHD survive? Check ext4."
fi
PB_LINES=$(wc -l < "$PHASE_B_OUT")
if [ "$PB_LINES" -lt 7925 ]; then
  warn "Phase B output has $PB_LINES lines, expected 7925. Spot-check before Phase C."
else
  ok "Phase B output OK: $PB_LINES / 7925 records"
fi

# --- 2. earlyoom + sysctl ---
if systemctl is-active --quiet earlyoom; then
  ok "earlyoom active"
else
  fail "earlyoom not active — run: sudo systemctl start earlyoom"
fi

SYSCTL_OK=1
[ "$(sysctl -n vm.overcommit_memory)" = "2" ] || { warn "vm.overcommit_memory is not 2"; SYSCTL_OK=0; }
[ "$(sysctl -n vm.swappiness)" = "10" ]       || { warn "vm.swappiness is not 10"; SYSCTL_OK=0; }
[ "$(sysctl -n vm.overcommit_ratio)" = "90" ] || { warn "vm.overcommit_ratio is not 90"; SYSCTL_OK=0; }
[ "$(sysctl -n vm.dirty_ratio)" = "10" ]      || { warn "vm.dirty_ratio is not 10"; SYSCTL_OK=0; }
[ "$(sysctl -n vm.dirty_background_ratio)" = "5" ] || { warn "vm.dirty_background_ratio is not 5"; SYSCTL_OK=0; }
if [ "$SYSCTL_OK" = "1" ]; then
  ok "sysctl tuning (2/90/10/10/5) applied"
else
  fail "sysctl tuning not applied — run: sudo sysctl -p /etc/sysctl.d/99-contextgraph-training.conf"
fi

# --- 3. Clean stale PID/lock files ---
for f in "$PIDFILE" \
         "$REPO/.claude-flow/daemon.pid" \
         "$REPO/.claude/scheduled_tasks.lock" \
         "$REPO/contextgraph_data/mcp.pid"; do
  [ -f "$f" ] || continue
  pid=$(head -c 32 "$f" 2>/dev/null | grep -oE '^[0-9]+' | head -1 || true)
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    warn "keeping $f (PID $pid still alive)"
  else
    ok "removed stale $f"
    rm -f "$f"
  fi
done

# --- 4. Daemon already listening? ---
if ss -lnt 2>/dev/null | grep -q ':3100 '; then
  ok "daemon already listening on 3100"
else
  # Kill any zombie daemon processes that aren't listening
  if pgrep -f 'context-graph-mcp --daemon' >/dev/null; then
    warn "found context-graph-mcp processes but no listener — killing"
    pkill -9 -f 'context-graph-mcp --daemon' 2>/dev/null
    sleep 2
  fi

  # Start fresh
  [ -x "$BIN" ] || fail "binary not found or not executable: $BIN"
  echo "[restart] launching daemon — model load takes 30-60s..."
  cd "$REPO"
  RUST_LOG=info nohup "$BIN" --daemon-server-only --daemon-port 3100 > "$DAEMON_LOG" 2>&1 &
  DPID=$!
  echo "$DPID" > "$PIDFILE"

  # Wait for LISTEN
  elapsed=0
  while [ $elapsed -lt $MAX_WAIT ]; do
    if ss -lnt 2>/dev/null | grep -q ':3100 '; then
      ok "daemon listening on 3100 after ${elapsed}s (pid=$DPID)"
      break
    fi
    if ! kill -0 "$DPID" 2>/dev/null; then
      fail "daemon exited during startup — check $DAEMON_LOG"
    fi
    sleep 5
    elapsed=$((elapsed + 5))
    echo "[restart]   still loading... ${elapsed}s"
  done
  if [ $elapsed -ge $MAX_WAIT ]; then
    fail "daemon did not bind 3100 within ${MAX_WAIT}s — check $DAEMON_LOG"
  fi
fi

# --- 5. vram_watchdog ---
if pgrep -f vram_watchdog.py >/dev/null; then
  ok "vram_watchdog already running"
else
  cd "$REPO"
  nohup python3 scripts/vram_watchdog.py --threshold-gb 30 --warn-gb 28 --interval 10 > "$WATCHDOG_LOG" 2>&1 &
  WPID=$!
  sleep 2
  if kill -0 "$WPID" 2>/dev/null; then
    ok "vram_watchdog started (pid=$WPID)"
  else
    fail "vram_watchdog exited immediately — check $WATCHDOG_LOG"
  fi
fi

# --- 6. Summary ---
echo ""
echo "=== READY for Phase C ==="
echo "Phase B output: $PHASE_B_OUT ($PB_LINES records)"
ss -lntp 2>/dev/null | grep ':3100 ' | head -1
nvidia-smi --query-gpu=memory.used,memory.free,memory.total --format=csv,noheader | head -1
free -h | head -2
echo ""
echo "Next step (when told): python3 scripts/shakespeare_build_dpo_style_pairs.py"
