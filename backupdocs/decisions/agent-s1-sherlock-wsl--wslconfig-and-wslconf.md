---
namespace: decisions
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:30:00Z
status: active
severity: high
tags: wsl2, wslconfig, wsl-conf, configuration, memory-cap, swap
---

# WSL Configuration — Locked Settings

Two configuration files govern WSL2 resource exposure. Both must be set correctly. The
first is **Windows-side** (cannot be written from inside WSL); the second is **Linux-side**.

## File 1: `%USERPROFILE%\.wslconfig` (Windows path)
On this system: `C:\Users\hotra\.wslconfig`

Recommended contents (WSL must be shut down with `wsl --shutdown` before edits take
effect — do this when no training is running):

```ini
[wsl2]
# --- Memory governance -------------------------------------------------------
# Hard cap the VM to leave Windows ~8 GB headroom. Prevents the VM from
# ballooning into territory where Windows itself starts paging and declares
# the VM unresponsive.
memory=112GB

# Provision the full swap. With memory=112GB + swap=32GB, even a runaway
# training job has ~140 GB before the kernel OOM kills it — enough time for
# earlyoom (PSI-aware) to intervene.
swap=32GB
swapFile=C:\\wsl-swap\\swap.vhdx

# --- CPU governance ----------------------------------------------------------
# 9950X3D has 16c/32t; reserve 2 threads for Windows + claude-code IDE.
processors=30

# --- Networking --------------------------------------------------------------
# Mirrored mode plays better with Windows firewall for dashboard:8765 / 3100 / 8766
networkingMode=mirrored
dnsTunneling=true
firewall=true
autoProxy=true
localhostForwarding=true

# --- Kernel tuning ----------------------------------------------------------
# Cap dirty ratios to reduce the risk of a multi-second writeback stall during
# page reclaim. Lowering swappiness reduces the kernel's preference for
# swapping over killing when anon pages are overwhelming.
kernelCommandLine=vm.swappiness=10 vm.dirty_ratio=10 vm.dirty_background_ratio=5 vm.overcommit_memory=2 vm.overcommit_ratio=90

# --- Power settings ---------------------------------------------------------
# Page-reporting frees unused pages back to the host quickly — helps Windows
# reclaim when the VM is idle between phases.
pageReporting=true
# DO NOT set guiApplications=false — WSLg is used; but idleThreshold trims
# quickly on idle.
```

Key rationale:
- `memory=112GB` — prevent the VM from grabbing more than 87 % of physical and starving
  Windows (which, if starved, will pause the VM).
- `swap=32GB` + dedicated `swapFile` — keeps swap off the default virtual disk.
- `vm.overcommit_memory=2` (in kernelCommandLine) — switches Linux from "always grant"
  to "strict accounting": a malloc for 100 GB will fail with ENOMEM instead of succeeding
  and crashing later. With `overcommit_ratio=90`, the accounting ceiling is
  `swap + 90% * RAM ≈ 32 + 100 = 132 GB` — matches what the system can actually deliver.
- `vm.swappiness=10` — don't thrash to swap before the kernel decides; better to spill
  to swap gradually under real pressure.
- `dnsTunneling=true` + `autoProxy=true` — WSL2 2.0+ defaults; improves reliability for
  HuggingFace / pip under corporate proxies.

## File 2: `/etc/wsl.conf` (inside the distro)
Current (minimal):
```ini
[boot]
systemd=true
```

Recommended upgrade (write with sudo; takes effect after `wsl --shutdown` + reopen):

```ini
[boot]
systemd=true
# Cleanup command runs once per distro launch. Reaps stale PID/lock files
# from /tmp and our project dirs BEFORE any service starts.
command="/usr/local/sbin/wsl-boot-cleanup.sh"

[automount]
# Disable metadata/UID on /mnt/c to reduce 9p overhead during large-file reads
# from Windows filesystem. Training data should live on the ext4 side of WSL
# (~/.contextgraph/), not /mnt/c, so metadata is unneeded.
enabled=true
options="metadata,uid=1000,gid=1000,umask=22,fmask=11,case=off"
mountFsTab=false

[network]
generateHosts=true
generateResolvConf=true
hostname=cabdru

[user]
default=cabdru

[interop]
enabled=true
appendWindowsPath=false

[experimental]
# Reclaim memory aggressively when idle (WSL 0.82+)
autoMemoryReclaim=gradual
sparseVhd=true
```

The boot `command=` script to create (`/usr/local/sbin/wsl-boot-cleanup.sh`):

```bash
#!/bin/bash
# Clean up stale PID files left from prior crashes. Fail-safe: never removes
# a PID file whose PID is actually alive.
set -u
log(){ logger -t wsl-boot-cleanup "$*"; }

for f in \
  /home/cabdru/contextgraph/.claude-flow/daemon.pid \
  /home/cabdru/contextgraph/.claude/scheduled_tasks.lock \
  /home/cabdru/contextgraph/contextgraph_data/mcp.pid; do
  [ -f "$f" ] || continue
  pid=$(head -c 32 "$f" | grep -oE '^[0-9]+' | head -1 || true)
  [ -z "$pid" ] && { log "removing malformed $f"; rm -f "$f"; continue; }
  if ! kill -0 "$pid" 2>/dev/null; then
    log "removing stale $f (PID $pid gone)"
    rm -f "$f"
  fi
done

# Clear tmpfs training breadcrumbs that are older than boot
find /tmp -maxdepth 2 -name 'shake_*.log' -mmin +60 -delete 2>/dev/null || true
find /tmp -maxdepth 2 -name '.shakespeare_*' -mmin +60 -delete 2>/dev/null || true

exit 0
```

Make it executable: `sudo install -m 0755 wsl-boot-cleanup.sh /usr/local/sbin/`.

## Activation
After writing both files on Windows and Linux sides:
```
# FROM WINDOWS PowerShell (as Administrator)
wsl --shutdown
# wait ~10 seconds, then reopen terminal
```

Verify inside WSL after reboot:
```bash
grep -E 'overcommit_memory|swappiness|dirty_ratio' /proc/sys/vm/* 2>/dev/null
# Expected: overcommit_memory=2, swappiness=10, dirty_ratio=10
free -h       # memory line should cap at ~112 GiB
swapon --show # 32 GB on swap.vhdx
```

## Related
- See also: ./memory/discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md
- Tags: wsl2, configuration, memory, swap
