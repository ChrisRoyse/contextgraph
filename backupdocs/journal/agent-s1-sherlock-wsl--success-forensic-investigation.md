---
namespace: journal
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:45:00Z
status: active
tags: journal, success, wsl2, forensic, oom
---

# Journal: WSL2 crash forensic investigation — completed

## What I did
1. Read mission brief, guard architecture decision log, stylistic schema, prior agent
   discoveries (spacy/cupy, messages index), SMAP protocol.
2. Inspected `/etc/wsl.conf` (bare — only `systemd=true`) and `/etc/fstab` (unconfigured).
3. Surveyed live memory/swap/meminfo/swappiness/overcommit settings.
4. Enumerated running claude/node/npm/python/MCP processes; aggregated 10.5 GB user RSS
   CURRENTLY (post-reboot).
5. Confirmed the kernel dmesg is per-boot only, but **systemd-journal retained the
   previous boot** — journalctl --list-boots showed boot `-1` from 10:42 to 11:54:55.
6. Pulled the final minutes of the prior boot's journal and found the **OOM killer
   dump** at 11:53:53 UTC+CDT with a full `Mem-Info` snapshot.
7. Verified: four Python processes simultaneously at 14–30 GB RSS, swap exhausted,
   44 MB free in normal zone.
8. Identified stale PID files (.claude-flow/daemon.pid from 2026-02-15,
   .claude/scheduled_tasks.lock from 2026-04-13).
9. Verified no systemd-oomd and no earlyoom installed.

## Key observations
- Crash was HOST RAM OOM, NOT VRAM OOM. VRAM was tight but not the proximate trigger.
  The proximate trigger was a 339 GiB virtual / 28 GiB resident Python process unable to
  get an order-4 page for a 9p readahead, falling into oom-killer with swap exhausted.
- Multiple simultaneous Python phases — evidence that the pipeline had no enforced
  sequencing or orphan-reaping. Augmenter does stop daemon cleanly, but prior-phase
  Python leftovers were never killed before the next started.
- Four-minute stall between page-alloc failure and OOM reaper — long enough that
  Windows' vmmem watchdog likely terminated the VM (perceived as "crash").

## Memory files written
- discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md
- discoveries/agent-s1-sherlock-wsl--stale-pid-files.md
- discoveries/agent-s1-sherlock-wsl--no-userspace-oom-manager.md
- decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md
- patterns/agent-s1-sherlock-wsl--safe-startup-sequence.md
- journal/agent-s1-sherlock-wsl--success-forensic-investigation.md (this file)
- reports/agent-s1-sherlock-wsl--final-report.md (next)

## Confidence
HIGH. Forensic evidence is from a persistent systemd-journal (not transient dmesg) and
is reproducible via `journalctl -b -1 --since "2026-04-17 11:53:40"`.
