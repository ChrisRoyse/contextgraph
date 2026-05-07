---
namespace: discoveries
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:10:00Z
status: active
severity: high
tags: wsl2, oom-killer, crash-root-cause, forensic, python-leak
---

# WSL2 Crash Root Cause — Host RAM OOM, Not VRAM OOM

## Verdict (Forensic Finding)
The WSL2 VM hang at 2026-04-17 11:53:53 was caused by a **global host-RAM OOM** inside the
Linux VM, not by RTX 5090 VRAM exhaustion. Four concurrent Python processes + eight claude
CLIs + MCP daemons consumed over 100 GB of anonymous RAM and exhausted the 32 GB swap
before the kernel OOM killer was able to recover a responsive system.

## Source of Truth — systemd-journal (persistent across WSL reboots)
Run this to reproduce:
```
journalctl -b -1 --since "2026-04-17 11:53:40" --until "2026-04-17 11:53:55"
```

Key kernel lines (verbatim):
```
Apr 17 11:53:53 kernel: python: page allocation failure: order:4, ... cpuset=init.scope
Apr 17 11:53:53 kernel: Mem-Info:
                         active_anon:3660237 inactive_anon:27376542
                         free:131510
Apr 17 11:53:53 kernel: Node 0 active_anon:14640948kB inactive_anon:109506168kB
Apr 17 11:53:53 kernel: Node 0 Normal free:45164kB min:43636kB     <-- 44 MB FREE, 42 MB MIN
Apr 17 11:53:53 kernel: Free swap  = 0kB
Apr 17 11:53:53 kernel: Total swap = 33554432kB                     <-- all 32 GB swap consumed
Apr 17 11:53:53 kernel: oom-kill:constraint=CONSTRAINT_NONE, global_oom, task=python, pid=10431
Apr 17 11:53:53 kernel: Out of memory: Killed process 10431 (python)
                         total-vm:356159424kB,  <-- 339 GiB VIRTUAL
                         anon-rss:29577012kB    <-- 28.2 GiB RESIDENT
```

Top memory hogs at crash time (RSS in GB):
| PID     | Command | RSS (GB) | Note |
|---------|---------|---------:|------|
| 10431   | python  | 28.2     | OOM-killed (presumed Gemma loader / SFT trainer) |
| 1282476 | python  | 29.6     | concurrent pipeline phase |
| 1308807 | python  | 29.4     | concurrent pipeline phase |
| 1754353 | python  | 14.0     | concurrent pipeline phase |
| 881553  | claude  | ~4.5     | agent CLI |
| (7 more claude+node) | | ~25 combined | |

**Aggregated UID 1000 RSS: ~85 GB across 190 PIDs** — against a Node-0-Normal zone of
~117 GB managed, leaving 44 MB free, 0 swap. The system was in a hard page-alloc stall for
4+ minutes before the OOM reaper cleaned up — long enough that Windows' vmmem watchdog
likely terminated the VM for unresponsiveness, which is what the user observed as a
"crash".

## Why VRAM Was NOT the Cause
- GPU VRAM at current reboot: 17.3 GB used (daemon only). Pre-crash, Gemma + daemon would
  have been ~28-31 GB — TIGHT but not the proximate trigger.
- CUDA OOM would have thrown `CUDA out of memory` from `torch` and **killed only the
  Python process**, not the whole VM. What actually happened was a **kernel-level
  `__alloc_pages_slowpath` failure during a file-page readahead on the 9p /mnt/c
  mount** — i.e. Python was trying to read from /mnt/c/... and the kernel couldn't
  allocate ANY order-4 (64 KB contiguous) pages for the 9p fcall buffer.

## Contributing Factors (ranked)
1. **Multiple Python phases ran CONCURRENTLY** — four processes each holding a large
   fraction of the 35 MB–300 MB dataset in RAM, possibly each with a transformer model.
   The mission plan explicitly sequences D/E/F with "daemon DOWN for Gemma", but nothing
   enforced that previous-phase Python processes were reaped before the next started.
2. **vm.overcommit_memory = 1 (ALWAYS OVERCOMMIT)** — the kernel granted 339 GB virtual
   space to a single process without objection. Combined with Python's arena allocator
   and torch's pinned-memory staging buffers, this turned into actual resident memory
   under dataset iteration pressure.
3. **No userspace OOM manager** — no systemd-oomd, no earlyoom. WSL's kernel OOM killer
   is the only defense and fires too late.
4. **swap exhausted before kernel could reclaim** — swappiness=60 under heavy anonymous
   memory pressure plus 100GB of anon pages left the kernel with no reclaim targets.
5. **8+ concurrent claude CLI instances** — each Node.js process held 500-900 MB RSS.
   Aggregated claude+node baseline was ~5-10 GB before training started.
6. **Missing /etc/wsl.conf memory governance** — current file only sets `systemd=true`.
   No memory cap (Windows-side .wslconfig would also need work).

## Why Hardware Is "Far Above" the Requirement but Crashed Anyway
128 GB physical DDR5 is plenty. But:
- Windows keeps ~3-6 GB for itself.
- WSL2 `hv_balloon: Max. dynamic memory size: 125952 MB` — WSL can grow up to 126 GB but
  only if it asks Hyper-V for it. Under sudden allocation pressure, ballooning latency
  (seconds) can starve the VM before Windows delivers more pages.
- No .wslconfig memory/swap tuning means the VM uses dynamic defaults which are tuned for
  desktop-use VMs, not ML pipelines.

## Related
- See also: ./memory/context/mission-resurrection-plan.md (phase D/E/F daemon-down requirement)
- See also: ./memory/decisions/coordinator--guard-architecture.md (fail-fast doctrine)
- Tags: wsl2, oom-killer, resurrection, python-leak, systemd-oomd
