---
namespace: reports
agent_id: agent-s1-sherlock-wsl
timestamp: 2026-04-17T17:55:00Z
status: active
tags: report, final, agent-s1, wsl2, forensic, oom, process-management
---

# Final Report: agent-s1-sherlock-wsl

## Task Completed: yes

## Summary
Investigated the WSL2 crash that occurred 2026-04-17 at approximately 11:54 CDT during
Shakespeare Resurrection training. Scope was restricted to WSL2-level resource management
and process hygiene (VRAM allocation is Agent S2's, script-level leaks are Agent S3's).

**Verdict**: the crash was a **host-RAM global OOM inside the WSL2 VM**, not a VRAM
exhaustion event. At the moment of collapse, four concurrent Python processes
(presumed pipeline phases and/or Gemma loaders) each held 14–30 GiB resident RAM,
swap was fully exhausted, and only 44 MiB remained free in the Normal zone. The
kernel's page allocator stalled for ≥4 minutes trying to satisfy an order-4 allocation
for a 9p filesystem readahead — long enough that Windows' vmmem watchdog terminated
the VM, which the user perceived as a system crash.

The evidence is preserved in `systemd-journal` (persistent across WSL reboots) and can
be reproduced verbatim by:
```bash
journalctl -b -1 --since "2026-04-17 11:53:40" --until "2026-04-17 11:54:00"
```

The root causes are (ranked by contribution):
1. Multiple pipeline phases ran CONCURRENTLY with no enforced sequencing or orphan-reap.
2. `vm.overcommit_memory=1` (ALWAYS overcommit) allowed Python to claim 339 GiB virtual
   memory without objection, which became real pages under dataset-iterator pressure.
3. NO userspace OOM manager (neither systemd-oomd nor earlyoom installed) — kernel OOM
   fires too late.
4. `/etc/wsl.conf` has only `systemd=true`; no Windows-side `.wslconfig` memory cap.
5. Eight simultaneous `claude` CLIs + eight `npm exec @playwright/mcp` added a ~5–10 GB
   baseline before training even started.

## Files Created or Modified in the Repo
None. (This is a forensic investigation; fixes are documented in SMAP decisions and
patterns, to be implemented by the coordinator.)

## Files Written to ./memory/
- `discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md` — full forensic analysis
  of the OOM event, with kernel trace excerpts.
- `discoveries/agent-s1-sherlock-wsl--stale-pid-files.md` — three stale PID/lock files
  identified on the host.
- `discoveries/agent-s1-sherlock-wsl--no-userspace-oom-manager.md` — no earlyoom or
  systemd-oomd present; recommendation to install earlyoom.
- `decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md` — full recommended
  contents for `%USERPROFILE%\.wslconfig` and `/etc/wsl.conf` with rationale.
- `patterns/agent-s1-sherlock-wsl--safe-startup-sequence.md` — 7-step numbered startup
  sequence with RSS budget, subprocess watchdog, Python-side `resource` module guard.
- `journal/agent-s1-sherlock-wsl--success-forensic-investigation.md` — execution log.
- `reports/agent-s1-sherlock-wsl--final-report.md` — this file.

## Decisions Made
- **Adopt earlyoom, not systemd-oomd.** earlyoom's PID-cmdline filtering (`--avoid` /
  `--prefer`) directly matches the requirement "protect daemon, kill training child".
  Rationale: systemd-oomd requires cgroup v2 PSI data which is sparser inside WSL's
  Hyper-V kernel and is more complex to configure for this case.
- **Run the pipeline via a wrapper shell script**, not by invoking Python directly.
  The wrapper enforces ulimit, runs a per-phase watchdog, and cleans up orphans — all
  things that cannot be enforced from inside the Python process itself.
- **Sequence phases, do not parallelise.** Phases A/B/C/G need daemon up; D/E/F need
  daemon down. No overlap, no exceptions.

## Discoveries
- systemd-journal is PERSISTENT across WSL reboots on this system (`SystemMaxUse` default
  or similar). dmesg is wiped per boot, but `journalctl -b -1` retrieves the prior boot.
  Impact: future crash investigations can use the journal as the source of truth.
- `hv_balloon: Max. dynamic memory size: 125952 MB` — WSL2 can balloon to 126 GB, but
  balloon latency under sudden pressure leaves the VM visibly unresponsive for seconds.
- The `init.scope` cgroup hosts all user processes on this config; when it hits OOM,
  child processes die in an unpredictable order, risking the daemon.
- Four-Python-phase simultaneous memory residency in the OOM dump strongly implies
  the scripts do NOT kill prior-phase Python processes; they assume prior phase is
  complete when the next starts. No cross-check.
- Current GPU VRAM usage post-reboot: 17.3 GiB (daemon alone). Pre-crash was likely
  28–31 GiB (daemon + Gemma load overlapping). This is relevant to Agent S2 but does
  NOT explain this crash.

## Failures
None during the forensic investigation itself.

## Successes
- Pulled the prior-boot OOM kernel trace despite dmesg being wiped. Key insight: always
  check `journalctl -b -1` before declaring crash evidence lost.
- Identified global_oom (not cgroup OOM) — rules out cgroup-v2 accounting bugs as the
  trigger; focuses the fix on whole-VM memory discipline.

## Blockers
None. The fixes are straightforward to apply.

## Tests / Build
- Not applicable. No code was modified.

## For the Next Agent (Coordinator)

### TOP-3 preventive measures (exact paths + contents)

1. **Windows-side `.wslconfig`** — file path `C:\Users\hotra\.wslconfig` (edit from
   Windows; WSL cannot write here). Full contents specified in
   `./memory/decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md` (section
   "File 1"). Critical lines:
   ```ini
   [wsl2]
   memory=112GB
   swap=32GB
   swapFile=C:\\wsl-swap\\swap.vhdx
   processors=30
   kernelCommandLine=vm.swappiness=10 vm.dirty_ratio=10 vm.dirty_background_ratio=5 vm.overcommit_memory=2 vm.overcommit_ratio=90
   networkingMode=mirrored
   ```
   Apply with `wsl --shutdown` from PowerShell after write.

2. **Install earlyoom** — inside WSL:
   ```bash
   sudo apt install earlyoom
   sudo sh -c 'cat > /etc/default/earlyoom <<EOF
   EARLYOOM_ARGS="-m 8 -s 8 --avoid ^(dockerd|context-graph-mcp|systemd|init)\$ --prefer ^(python|python3|chrome|node)\$"
   EOF'
   sudo systemctl enable --now earlyoom
   systemctl status earlyoom
   ```
   This kills least-important processes when RAM+SWAP drops below 8 % — long before the
   kernel OOM killer takes a 4-minute stall.

3. **Upgrade `/etc/wsl.conf` and install boot cleanup script** — full contents in
   `./memory/decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md` (section
   "File 2"). Critical additions:
   ```ini
   [boot]
   systemd=true
   command="/usr/local/sbin/wsl-boot-cleanup.sh"
   [experimental]
   autoMemoryReclaim=gradual
   sparseVhd=true
   ```
   The cleanup script reaps stale PID files at every distro boot, preventing the
   "PID 4216 dead since 2026-02-15" orphan situation.

### STARTUP SEQUENCE (numbered shell for safe training run)
Full version in `./memory/patterns/agent-s1-sherlock-wsl--safe-startup-sequence.md`.
Condensed:
```bash
# Run from /home/cabdru/contextgraph, on each training phase launch:

# 1) Verify guards are in place
[ "$(cat /proc/sys/vm/overcommit_memory)" = "2" ] || { echo "Abort: .wslconfig not applied"; exit 1; }
systemctl is-active --quiet earlyoom || { echo "Abort: earlyoom not running"; exit 1; }

# 2) Kill orphan claude/node/mcp (keep only current terminal's parent)
pgrep -f '^(claude|npm exec)$' | grep -v "^$$\$" | xargs -r kill -TERM; sleep 3
pgrep -f '^(claude|npm exec)$' | xargs -r kill -KILL 2>/dev/null

# 3) Kill any prior shakespeare python
pgrep -fu "$USER" 'python.*shakespeare_' | xargs -r kill -TERM; sleep 5
pgrep -fu "$USER" 'python.*shakespeare_' | xargs -r kill -KILL 2>/dev/null

# 4) Stop daemon if this phase owns the GPU (D/E/F)
case "$PHASE" in
  D|E|F)  fuser -k -TERM 3100/tcp 2>/dev/null; sleep 10
          fuser -k -KILL 3100/tcp 2>/dev/null; sleep 2
          USED=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | head -1)
          [ "$USED" -gt 2048 ] && { echo "Abort: GPU still ${USED}MiB"; exit 2; } ;;
esac

# 5) Start daemon if this phase needs it (A/B/C/G)
case "$PHASE" in A|B|C|G)  bash scripts/ensure_daemon.sh || exit 3 ;; esac

# 6) Run phase with ulimit + watchdog (see pattern file for full watchdog)
RSS_CAP_GB=22
(
  ulimit -v $(( RSS_CAP_GB * 1024 * 1024 ))
  exec timeout --kill-after=60 10h python3 scripts/shakespeare_phase_${PHASE,,}.py "$@"
) &
PHASE_PID=$!

# Watchdog (background) samples RSS every 30s and kills if >budget for 2 samples
# or if system MemAvailable drops below 8 GB. See pattern file.

wait "$PHASE_PID"
```

### Start by reading
- `./memory/discoveries/agent-s1-sherlock-wsl--oom-killer-verdict.md` for the full
  forensic evidence.
- `./memory/patterns/agent-s1-sherlock-wsl--safe-startup-sequence.md` for the complete
  wrapper including watchdog, Python-side `resource` module integration, and teardown.
- `./memory/decisions/agent-s1-sherlock-wsl--wslconfig-and-wslconf.md` for exact
  config contents.

### Watch out for
- `wsl --shutdown` terminates ALL WSL distros and ALL running claude-code sessions.
  Schedule .wslconfig application when user is not mid-session.
- Changing `vm.overcommit_memory` from 1 to 2 will cause previously-succeeding
  allocations to fail with ENOMEM. Any Python code that assumed infinite virtual
  memory will now raise `MemoryError`. That is the POINT — fail fast and loud, not
  hang the VM.
- earlyoom's `--prefer` pattern MUST not include `claude` without thought — we want
  earlyoom to kill training children, not the terminal. The recommended
  `--prefer ^(python|python3|chrome|node)$` could kill VS Code server (node). Tune
  down to `--prefer ^python` if that becomes an issue.

### Priority
1. Install earlyoom (5 minutes, inside WSL, no reboot needed). IMMEDIATE.
2. Apply `.wslconfig` + `wsl --shutdown` (requires scheduled downtime). HIGH.
3. Integrate startup-sequence wrapper into pipeline runner script. HIGH.
4. Add Python-side `resource`/`atexit`/`signal` guard to each phase script. MEDIUM.

## What I Could NOT Verify
Full transparency on gaps:
- **Cannot read `C:\Users\hotra\.wslconfig`** from inside WSL — only documented what it
  SHOULD contain. If a file exists there with different settings, it will override my
  recommendations. User should `type %USERPROFILE%\.wslconfig` on Windows to confirm.
- **Cannot reconstruct exactly which Python scripts** were running as the four OOM-dump
  PIDs — the kernel only logged `Comm=python` (truncated). The journal does not log
  cmdlines. Possible culprits based on PID ordering and context: simultaneous
  `shakespeare_stylistic_analyzer.py` + `shakespeare_build_dpo_style_pairs.py` +
  `shakespeare_sft_augmenter.py` + maybe a lingering `shakespeare_train_lora.py` from
  a prior attempt.
- **Cannot confirm vmmem watchdog fired**. The journal contains the OOM event and
  subsequent OOM reaper, then a 20-second silence, then systemd_resolved "Clock change
  detected" (clock change is a Windows-side marker for VM-level state change). No
  explicit "vmmem killed VM" log entry on the Linux side, because Linux doesn't see
  it coming.
- **Cannot access dmesg from the prior boot** — WSL wipes it. Only journalctl
  retained the crash record.
- **The stale `.claude-flow/daemon.pid` from 2026-02-15** predates the crash by 2
  months, so it did not cause the crash. It's a hygiene issue, not a contributor.

## Confidence: HIGH
The OOM event is documented verbatim in kernel logs with timestamps matching the user's
reported crash window. The Mem-Info snapshot, swap exhaustion, and task list of
memory consumers are all in-line evidence. The diagnosis is "host-RAM OOM triggered by
concurrent Python pipelines" with very high certainty. The preventive measures
(earlyoom, .wslconfig memory cap, startup-sequence wrapper) directly address each
contributing factor identified in the evidence.

Where confidence drops: WHICH pipeline scripts were the four Python processes (medium
confidence — can be narrowed by Agent S3 who examines scripts) and whether vmmem was
the final killer vs the VM simply hanging on 9p readahead stalls (low consequence — the
fix is the same either way).
