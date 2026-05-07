---
namespace: discoveries
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:20:00Z
status: active
severity: high
tags: wsl2, oom-manager, systemd-oomd, earlyoom, defense-in-depth
---

# No Userspace OOM Manager Installed — Kernel OOM Killer Is Too Late

## What Happened
Verification:
```bash
systemctl status systemd-oomd   # Unit systemd-oomd.service could not be found
systemctl status earlyoom       # Unit earlyoom.service could not be found
cat /proc/sys/vm/overcommit_memory   # 1 (always overcommit)
cat /proc/sys/vm/swappiness          # 60 (kernel prefers to kill over aggressive swap)
```

There is NO PSI-based (Pressure Stall Information) userspace OOM manager. Linux kernel
OOM killer is the only last resort, and it only fires AFTER pages-free dips under
zone-min for an extended stall — by then, the VM is already unresponsive for minutes,
during which Windows' vmmem watchdog can kill the entire WSL instance (perceived as
"crash").

## Why It Matters
In the crash forensics, the kernel took >4 minutes of page-allocation failures before
the OOM killer fired. A userspace manager reading `/proc/pressure/memory` can act
at 5–10 % PSI memory-pressure, well before the kernel enters its reclaim death spiral.

## Fix Options (ranked)
1. **Install `systemd-oomd`** (preferred — already integrated with systemd on Ubuntu 24+):
   ```bash
   sudo apt install systemd-oomd
   sudo systemctl enable --now systemd-oomd
   sudo mkdir -p /etc/systemd/oomd.conf.d
   cat <<'EOF' | sudo tee /etc/systemd/oomd.conf.d/training.conf
   [OOM]
   SwapUsedLimit=90%
   DefaultMemoryPressureLimit=65%
   DefaultMemoryPressureDurationSec=30s
   EOF
   ```
   Then mark the user slice for oversight:
   ```bash
   sudo systemctl edit user-1000.slice
   # [Slice]
   # ManagedOOMSwap=kill
   # ManagedOOMMemoryPressure=kill
   # ManagedOOMMemoryPressureLimit=80%
   ```

2. **Install `earlyoom`** (lightweight, PSI-agnostic, works on older systems):
   ```bash
   sudo apt install earlyoom
   # /etc/default/earlyoom:
   #   EARLYOOM_ARGS="-m 7 -s 7 --avoid '^(dockerd|context-graph-mcp|systemd)$' --prefer '^(python|python3|chrome)$'"
   sudo systemctl enable --now earlyoom
   ```
   This kills least-important processes when RAM+SWAP both drop below 7 %, before the
   kernel gets into distress. `--prefer '^python'` biases kills away from the daemon
   and toward the training python process, which is the right behavior.

## Recommendation
**Adopt earlyoom** for this environment. It is simpler than systemd-oomd (which requires
cgroup v2 ManagedOOM settings and is sensitive to WSL's lack of full cgroup PSI data),
and its `--prefer` / `--avoid` filtering directly matches the need: protect the daemon,
kill the training child when it goes rogue.

## Related
- See also: ./memory/discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md
- Tags: oom, earlyoom, systemd-oomd, defense-in-depth
