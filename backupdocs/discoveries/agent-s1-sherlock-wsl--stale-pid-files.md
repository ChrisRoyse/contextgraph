---
namespace: discoveries
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:15:00Z
status: active
severity: medium
tags: wsl2, pid-files, orphans, stale-locks, hygiene
---

# Stale PID / Lock Files Surviving Across Reboots

## What Happened
Current inspection (post-reboot) shows:

| File | Contents | Status |
|------|---------:|--------|
| `/home/cabdru/contextgraph/contextgraph_data/mcp.pid` | 17221 | ALIVE (fresh daemon after reboot; self-healed) |
| `/home/cabdru/contextgraph/.claude-flow/daemon.pid` | 4216 | **DEAD** — orphan from 2026-02-15, never cleaned up |
| `/home/cabdru/contextgraph/.claude/scheduled_tasks.lock` | `{"pid":88546,...acquiredAt:1776139691335}` | **STALE** — from 2026-04-13, PID gone |

## Why It Matters
When training scripts probe for "is the daemon running" by reading a PID file and calling
`kill -0 $pid`, a stale PID can lead to false positives (the PID got recycled to a
different process) or false negatives (the PID is gone but the port is still owned).

The Shakespeare scripts' `ensure_daemon.sh` uses port-based detection, so it's safe, but
`.claude-flow/daemon.pid` is a booby trap for any agent that grabs it via the claude-flow
CLI. The `scheduled_tasks.lock` is a coordination lock — if any code path honors it and
sees PID 88546 "owns" the lock, work silently waits forever.

## Recommended Cleanup (immediate)
```bash
# Safe to delete: all reference PIDs that no longer exist
rm -f /home/cabdru/contextgraph/.claude-flow/daemon.pid
rm -f /home/cabdru/contextgraph/.claude/scheduled_tasks.lock
```

Leave `contextgraph_data/mcp.pid` alone — the daemon rewrites it on start.

## Pattern to Establish (for future scripts)
Any long-lived service MUST:
1. Write PID to PID file atomically (temp-file-and-rename).
2. On startup, check existing PID file — if PID is dead (`kill -0` fails), remove and proceed.
3. On clean shutdown, `atexit`/`trap` removes own PID file.
4. NEVER hold a lock with ONLY a PID number — include start timestamp and verify via
   `/proc/<pid>/stat` starttime field to rule out PID recycling.

## Related
- Tags: pid-files, orphans, hygiene, startup-sequence
