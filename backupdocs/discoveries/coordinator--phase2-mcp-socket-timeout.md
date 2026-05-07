---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-20T12:20:00Z
status: resolved
severity: medium
tags: discovery, phase-2, mcp, socket-timeout, resilience
---

# Discovery: Phase 2 Crashed on MCP Socket Read Timeout at 7,318 Records

## What Happened

Phase 2 ran from 2026-04-18 09:42 CDT to 2026-04-20 ~12:18 CDT (~51 hours elapsed) processing 7,318 records before crashing with:

```
Traceback (most recent call last):
  File "/home/cabdru/contextgraph/scripts/shakespeare_v4_generate_responses.py", line 340, in main
    store = mcp.tool_call("store_memory", {...})
  File "/home/cabdru/contextgraph/scripts/shakespeare_v4_generate_responses.py", line 122, in _call_raw
    line = self.f.readline()
  File "/home/cabdru/miniconda3/lib/python3.13/socket.py", line 719, in readinto
    return self._sock.recv_into(b)
TimeoutError: timed out
```

**Blast radius:** zero data loss — `.partial` file preserved. Recovery cost: ~3 min restart + ~10s Gemma/LoRA reload from disk cache.

## Root Cause

`MCPClient` sets `timeout=30.0` on `socket.create_connection(...)`. This applies to BOTH connection AND subsequent read/write operations. Daemon's `store_memory` does full 13-embedder inference (+ async worker updates); a single response occasionally exceeds 30s under load.

Probable triggers:
- Background HNSW compaction on the daemon
- Growing RocksDB write buffer → slower batched writes
- Disk I/O pressure (partial has 22 MB now, RocksDB growing)
- Transient GPU contention with Gemma generation (both share GPU 0)

## State at Crash

- 7,318 records in `shakespeare_v4_sft.jsonl.partial` (81.3%)
- 6,861 accepted, pass rate ~97.9%
- Last category: `conversational` (217/700 done)
- Remaining: 1,682 records (conversational balance + creative_abstract + foreign_lingual + adversarial)

## Recovery Action

Restarted Phase 2 via `run_in_background: true`. Script auto-resumes from `.partial` (skips 7,318 already-processed prompt_ids). Daemon remained UP throughout — didn't need restart.

## Longer-Term Fix Options

1. **Increase socket timeout to 120s** — most robust, won't crash on normal latency spikes.
2. **Catch TimeoutError and retry once** — adds complexity, one retry could still fail.
3. **Reset socket state on TimeoutError** — if daemon is actually fine, reconnect and re-issue the call.
4. **Leave as-is** — 51h uptime before crash, `.partial` resume works. Restarting every 50h is acceptable cost.

**Decision**: leave timeout at 30s for now. If this crashes again within the remaining 1,682 records, tune to 120s. Otherwise no change — the fail-fast + resume-safe pattern is working.

## Lessons

1. **MCP read timeout must account for daemon's slowest reasonable response**, not its average. 30s was too tight when daemon does 13-embedder inference + background work. Consider 60-120s for long-run batch scripts.
2. **Traceback notification preceded the failed-task notification by ~2 minutes** — log has Traceback, but the background-task status only flips to "failed" when the parent bash exits. Don't wait for the official failure — check partial freshness instead.
3. **Resume-safe `.partial` pattern has now paid off TWICE** in this project (daemon orphan kill at record 140, MCP timeout at record 7,318). This is the load-bearing pattern that makes 50-hour runs actually feasible.

## Related

- `memory/discoveries/coordinator--daemon-orphan-kills-phase2.md` — first incident (daemon side)
- `tmp/SITREP_2026-04-18_171128.md` — mid-run handoff
- Log: `/tmp/v4-phase2-full.log`
- Output: `~/.contextgraph/training/shakespeare_v4_sft.jsonl.partial`
