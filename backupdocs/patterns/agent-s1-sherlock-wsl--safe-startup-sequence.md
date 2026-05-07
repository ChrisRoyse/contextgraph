---
namespace: patterns
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:40:00Z
status: active
severity: high
tags: startup-sequence, training, guard, python-subprocess, watchdog, rss-budget
---

# Safe Training Startup Sequence — WSL2 Process Hygiene Pattern

This pattern MUST be followed when launching any Shakespeare pipeline phase (B/C/D/E/F/G).
It guarantees: (1) one GPU-intensive process at a time, (2) bounded Python RSS, (3) clean
teardown of prior processes, (4) watchdog-based memory abort.

## The Sequence (numbered shell steps)

```bash
# ===== 0. PRE-FLIGHT CHECK (run as user, from /home/cabdru/contextgraph) =====

# 0.1 Confirm /etc/wsl.conf has systemd=true and autoMemoryReclaim=gradual
grep -q 'systemd=true' /etc/wsl.conf || { echo "FATAL: systemd not enabled"; exit 1; }

# 0.2 Confirm kernel has the tight vm.overcommit setting
[ "$(cat /proc/sys/vm/overcommit_memory)" = "2" ] \
  || { echo "WARN: overcommit still 1; edit .wslconfig kernelCommandLine"; }

# 0.3 Confirm earlyoom or systemd-oomd is active
systemctl is-active --quiet earlyoom || systemctl is-active --quiet systemd-oomd \
  || { echo "FATAL: no userspace OOM manager; install earlyoom first"; exit 1; }

# 0.4 Clean up stale PID/lock files (idempotent)
for f in .claude-flow/daemon.pid .claude/scheduled_tasks.lock \
         contextgraph_data/mcp.pid; do
  [ -f "$f" ] || continue
  pid=$(head -c 32 "$f" | grep -oE '^[0-9]+' | head -1 || true)
  if [ -n "$pid" ] && ! kill -0 "$pid" 2>/dev/null; then
    echo "[cleanup] removing stale $f (PID $pid dead)"
    rm -f "$f"
  fi
done

# ===== 1. STOP EVERY HEAVY CLAUDE/NODE INSTANCE EXCEPT ONE =====
#
# Rationale: each `claude` CLI is 500-900 MB, each `npm exec` is 100 MB.
# Eight of them running in parallel is 5-10 GB before ML starts. Kill them
# all except the terminal you're in.

MY_PPID=$(ps -o ppid= -p $$)  # parent of current shell
for pid in $(pgrep -f '^(claude|npm exec|node .*@(playwright|claude))' 2>/dev/null); do
  # Don't kill ourselves or our parent
  [ "$pid" = "$$" ] && continue
  [ "$pid" = "$MY_PPID" ] && continue
  # Get process ancestor chain - if it's in my session, leave it alone
  ps -o pid= -p "$pid" >/dev/null 2>&1 && kill -TERM "$pid" 2>/dev/null || true
done
sleep 3
# Hard-kill holdouts
pgrep -f '^(claude|npm exec)' 2>/dev/null | \
  while read pid; do [ "$pid" = "$$" ] && continue; kill -KILL "$pid" 2>/dev/null; done

# ===== 2. STOP ANY ORPHAN PYTHON FROM PRIOR PHASES =====
#
# Any prior pipeline process holding the old dataset in RAM is dangerous.
# Only kill our own user's python processes whose cmdline matches shakespeare_*.

for pid in $(pgrep -fu "$USER" 'python.*shakespeare_' 2>/dev/null); do
  echo "[cleanup] terminating prior shakespeare python PID $pid"
  kill -TERM "$pid" 2>/dev/null || true
done
sleep 5
# SIGKILL fallback
for pid in $(pgrep -fu "$USER" 'python.*shakespeare_' 2>/dev/null); do
  kill -KILL "$pid" 2>/dev/null || true
done

# ===== 3. DETERMINE WHICH PHASE NEEDS GPU (mutually exclusive with daemon) =====

PHASE="${1:?usage: start-training <A|B|C|D|E|F|G>}"
case "$PHASE" in
  A|B|C|G) DAEMON_NEEDED=1; GEMMA_NEEDED=0 ;;
  D)       DAEMON_NEEDED=0; GEMMA_NEEDED=1 ;;
  E|F)     DAEMON_NEEDED=0; GEMMA_NEEDED=0 ;;  # trainer owns GPU entirely
  *) echo "unknown phase $PHASE"; exit 1 ;;
esac

# ===== 4. GPU-EXCLUSIVE SLOT — DAEMON DOWN BEFORE GEMMA/TRAINER =====

if [ "$DAEMON_NEEDED" = "0" ]; then
  # 4a. Graceful stop via daemon SIGTERM
  if fuser 3100/tcp 2>/dev/null; then
    echo "[setup] stopping daemon on :3100"
    fuser -k -TERM 3100/tcp 2>/dev/null || true
    sleep 10  # daemon cleans up, writes state
    # SIGKILL fallback
    fuser -k -KILL 3100/tcp 2>/dev/null || true
    sleep 2
  fi
  # 4b. Verify GPU is actually free
  USED_MIB=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | head -1)
  if [ "${USED_MIB:-0}" -gt 2048 ]; then
    echo "FATAL: GPU still has ${USED_MIB} MiB after daemon stop. Check nvidia-smi."
    nvidia-smi
    exit 2
  fi
fi

if [ "$DAEMON_NEEDED" = "1" ]; then
  # Ensure daemon is up
  bash /home/cabdru/contextgraph/scripts/ensure_daemon.sh || {
    echo "FATAL: daemon failed to come up"; exit 3; }
fi

# ===== 5. SPAWN PHASE SCRIPT WITH RSS LIMIT + TIMEOUT WRAPPER =====

# Determine safe RSS cap per phase:
#   B/C (CPU-bound analysis, constellation scoring) — 16 GB
#   D   (Gemma loader, one-off generation)          — 22 GB
#   E/F (LoRA trainer)                              — 26 GB
case "$PHASE" in
  B|C) RSS_CAP_GB=16 ;;
  D)   RSS_CAP_GB=22 ;;
  E|F) RSS_CAP_GB=26 ;;
  A|G) RSS_CAP_GB=8  ;;
esac

# Convert GB -> kB for ulimit
RSS_CAP_KB=$(( RSS_CAP_GB * 1024 * 1024 ))

# Run the phase in a subshell with:
#   ulimit -v : virtual memory cap (hard, kills malloc)
#   ulimit -m : RSS soft cap
#   timeout   : wall clock ceiling
# The trainer/augmenter should internally do torch.cuda.empty_cache() on exit.

(
  ulimit -v "$RSS_CAP_KB"
  ulimit -m "$RSS_CAP_KB"
  # cd into project so imports work
  cd /home/cabdru/contextgraph
  exec timeout --kill-after=60 10h \
    python3 scripts/shakespeare_phase_${PHASE,,}.py "$@"
) &
PHASE_PID=$!
echo "[start] phase $PHASE PID=$PHASE_PID RSS_CAP=${RSS_CAP_GB} GB"

# ===== 6. WATCHDOG — abort if RSS exceeds budget for >60s =====

(
  WATCH_INTERVAL=30          # seconds between samples
  OVER_GRACE=2               # # of consecutive samples over budget before kill
  OVER_COUNT=0
  while kill -0 "$PHASE_PID" 2>/dev/null; do
    sleep "$WATCH_INTERVAL"
    # Aggregate RSS of the phase process tree
    RSS_KB=$(ps --no-headers -o rss= --ppid "$PHASE_PID" -p "$PHASE_PID" 2>/dev/null \
             | awk '{sum+=$1} END {print sum+0}')
    RSS_GB=$(( RSS_KB / 1048576 ))
    # Also sample system-wide available RAM as a safety net
    AVAIL_GB=$(awk '/MemAvailable/ {print int($2/1048576)}' /proc/meminfo)
    echo "[watchdog] phase=$PHASE pid=$PHASE_PID rss=${RSS_GB}GB avail=${AVAIL_GB}GB"
    if [ "$RSS_GB" -gt "$RSS_CAP_GB" ] || [ "$AVAIL_GB" -lt 8 ]; then
      OVER_COUNT=$(( OVER_COUNT + 1 ))
      if [ "$OVER_COUNT" -ge "$OVER_GRACE" ]; then
        echo "[watchdog] BUDGET EXCEEDED — sending SIGTERM to $PHASE_PID"
        kill -TERM "$PHASE_PID" 2>/dev/null
        sleep 10
        kill -KILL "$PHASE_PID" 2>/dev/null
        exit 99
      fi
    else
      OVER_COUNT=0
    fi
  done
) &
WATCHDOG_PID=$!

# ===== 7. WAIT AND CLEAN UP =====

wait "$PHASE_PID"
PHASE_EXIT=$?
echo "[done] phase $PHASE exited with $PHASE_EXIT"

kill "$WATCHDOG_PID" 2>/dev/null

# If phase owned the GPU, restart daemon now so subsequent phases see it
if [ "$DAEMON_NEEDED" = "0" ] && [ "$PHASE_EXIT" = "0" ]; then
  echo "[teardown] waiting 20s for VRAM to settle, then restarting daemon"
  sleep 20
  # Confirm GPU is clear
  for i in 1 2 3 4 5; do
    USED_MIB=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | head -1)
    [ "${USED_MIB:-0}" -lt 2048 ] && break
    echo "[teardown] GPU still busy (${USED_MIB} MiB), waiting..."
    sleep 10
  done
  bash /home/cabdru/contextgraph/scripts/ensure_daemon.sh
fi

exit $PHASE_EXIT
```

## Python-Side Companion (use inside scripts)

Each phase script SHOULD declare its own in-process memory guardrail using the `resource`
module, BEFORE loading heavy libraries:

```python
import resource, os, signal, sys, atexit

# 1. Install a hard RSS cap that matches the outer ulimit (defense in depth).
#    The shell already set ulimit -v; this reassures future readers AND lets
#    the script skip loads when it detects a constrained environment.
RSS_CAP_BYTES = int(os.environ.get("PHASE_RSS_CAP_GB", "24")) * 1024**3
try:
    # AS = address space = virtual memory
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    if soft == resource.RLIM_INFINITY or soft > RSS_CAP_BYTES:
        resource.setrlimit(resource.RLIMIT_AS, (RSS_CAP_BYTES, hard))
except ValueError:
    # outer ulimit was stricter; that is fine — keep it
    pass

# 2. atexit handler — torch cleanup + explicit exit code
def _cleanup():
    try:
        import torch, gc
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
            torch.cuda.synchronize()
    except Exception:
        pass
atexit.register(_cleanup)

# 3. Signal handler — parent watchdog can SIGTERM cleanly
def _sigterm(signum, frame):
    sys.stderr.write("[phase] received SIGTERM — cleaning up\n")
    _cleanup()
    sys.exit(143)  # 128 + 15
signal.signal(signal.SIGTERM, _sigterm)
signal.signal(signal.SIGINT,  _sigterm)
```

## Hard Rules (non-negotiable)
1. NEVER run two GPU-owning processes simultaneously. Daemon stop → Gemma load (or
   trainer start) → Gemma unload → daemon restart.
2. NEVER launch the next phase while the previous phase's Python is alive. Always
   `pgrep`-kill orphans at the top of each phase.
3. NEVER rely on the kernel OOM killer. The watchdog + ulimit + earlyoom form a
   defence in depth; each layer catches different failure modes.
4. ALWAYS invoke phase scripts via the wrapper above, not directly via `python3`.

## Related
- See also: ./memory/discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md
- See also: ./memory/decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md
- See also: ./memory/decisions/coordinator--guard-architecture.md (D3 ensure_daemon.sh)
- Tags: startup-sequence, watchdog, ulimit, rss-budget, python-hygiene
