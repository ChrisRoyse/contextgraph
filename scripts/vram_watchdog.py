#!/usr/bin/env python3
"""VRAM Safety Watchdog for Shakespeare Resurrection Pipeline.

Monitors RTX 5090 VRAM usage every POLL_INTERVAL_S seconds via pynvml (reliable)
and kills the newest GPU process if total VRAM exceeds HARD_KILL_GB.

Source of truth rationale:
- pynvml / nvidia-smi: reliable on RTX 5090 + WSL2 + driver 595.71, system-wide view
- torch.cuda.mem_get_info: UNRELIABLE on WSL2 — only sees current process, NOT other procs
  See: memory/discoveries/agent-s2-sherlock-vram--nvidia-smi-is-reliable-torch-memgetinfo-is-not.md

Usage:
    python3 scripts/vram_watchdog.py                 # run until Ctrl+C
    python3 scripts/vram_watchdog.py --dry-run       # monitor but do NOT kill
    python3 scripts/vram_watchdog.py --threshold-gb 29.5

Exit codes:
    0  normal shutdown (SIGINT/SIGTERM)
    1  initialization failure (no CUDA, pynvml missing)
    2  a kill event happened and --exit-on-kill is set
"""
from __future__ import annotations
import argparse
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

# Hardware constant (RTX 5090)
TOTAL_VRAM_GB = 31.84          # nvidia-smi-reported total; do NOT use pynvml's 34.19
# Thresholds
HARD_KILL_GB = 30.0            # kill newest GPU process above this
WARN_GB = 28.0                 # log WARN above this
POLL_INTERVAL_S = 10.0         # matches user spec: every 10 s
# Process whitelist: NEVER kill these even if VRAM exceeds threshold
# (these are the critical services; if they bloat beyond budget the problem is elsewhere)
WHITELIST_CMDS = {
    # intentionally empty — all GPU processes may be killed in order of newest first
    # to whitelist something add its exact /proc/<pid>/comm here, e.g. "Xorg"
}
# Filesystem
LOGFILE = Path("/tmp/vram_watchdog.log")
MAX_LOG_BYTES = 10 * 1024 * 1024
LOG_BACKUPS = 3


@dataclass(frozen=True)
class GpuProc:
    pid: int
    used_mib: int
    cmd: str
    start_epoch: float


def log(msg: str, level: str = "INFO") -> None:
    line = f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] [{level}] vram_watchdog: {msg}"
    print(line, file=sys.stderr, flush=True)
    try:
        rotate_log_if_needed(LOGFILE, MAX_LOG_BYTES, LOG_BACKUPS)
        with LOGFILE.open("a") as f:
            f.write(line + "\n")
    except OSError:
        pass


def rotate_log_if_needed(path: Path, max_bytes: int, backups: int) -> None:
    """Bound append-only watchdog logs without deleting the active file."""
    try:
        if not path.exists() or path.stat().st_size < max_bytes:
            return
        oldest = path.with_name(f"{path.name}.{backups}")
        if oldest.exists():
            oldest.unlink()
        for idx in range(backups - 1, 0, -1):
            src = path.with_name(f"{path.name}.{idx}")
            if src.exists():
                src.rename(path.with_name(f"{path.name}.{idx + 1}"))
        path.rename(path.with_name(f"{path.name}.1"))
    except OSError:
        pass


def get_vram_used_gb() -> float:
    """System-wide VRAM used in GB via pynvml. Falls back to nvidia-smi CSV if pynvml missing."""
    try:
        import pynvml  # type: ignore
        pynvml.nvmlInit()
        h = pynvml.nvmlDeviceGetHandleByIndex(0)
        m = pynvml.nvmlDeviceGetMemoryInfo(h)
        return m.used / 1e9
    except Exception as e:
        log(f"pynvml failed ({e}); falling back to nvidia-smi", level="WARN")
        try:
            out = subprocess.run(
                ["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                capture_output=True, text=True, timeout=5.0, check=True)
            # returns MiB as integer-string
            mib = int(out.stdout.strip().splitlines()[0])
            return (mib * 1024 * 1024) / 1e9
        except Exception as e2:
            log(f"nvidia-smi also failed ({e2}); reporting 0.0", level="ERROR")
            return 0.0


def list_gpu_processes() -> list[GpuProc]:
    """Return list of processes using GPU compute, sorted by start time (oldest first).
    Uses nvidia-smi pmon for compute-app listing."""
    try:
        # --query-compute-apps gives pid + used_memory (MiB)
        out = subprocess.run(
            ["nvidia-smi", "--query-compute-apps=pid,used_memory",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5.0, check=True)
    except Exception as e:
        log(f"nvidia-smi --query-compute-apps failed: {e}", level="ERROR")
        return []
    procs: list[GpuProc] = []
    for raw in out.stdout.strip().splitlines():
        parts = [p.strip() for p in raw.split(",")]
        if len(parts) < 2:
            continue
        try:
            pid = int(parts[0])
            used_mib = int(parts[1])
        except ValueError:
            continue
        # Get cmdline + start time
        try:
            cmd = Path(f"/proc/{pid}/comm").read_text().strip()
        except OSError:
            cmd = f"pid-{pid}"
        try:
            start_epoch = Path(f"/proc/{pid}").stat().st_mtime
        except OSError:
            start_epoch = 0.0
        procs.append(GpuProc(pid=pid, used_mib=used_mib, cmd=cmd, start_epoch=start_epoch))
    # Sort by start_epoch ASCENDING; newest is last
    procs.sort(key=lambda p: p.start_epoch)
    return procs


def kill_newest_non_whitelisted(procs: list[GpuProc], dry_run: bool) -> GpuProc | None:
    """Kill the newest GPU process (highest start_epoch) not in whitelist.
    Returns the killed process, or None if none killed."""
    for p in reversed(procs):  # newest first
        if p.cmd in WHITELIST_CMDS:
            continue
        log(f"KILL target: pid={p.pid} cmd={p.cmd} used={p.used_mib} MiB "
            f"started={time.strftime('%H:%M:%S', time.localtime(p.start_epoch))}",
            level="CRITICAL")
        if dry_run:
            log("dry-run: NOT killing", level="CRITICAL")
            return p
        try:
            subprocess.run(["kill", "-TERM", str(p.pid)], check=False, timeout=3.0)
            # give 5s to exit gracefully
            time.sleep(5.0)
            if Path(f"/proc/{p.pid}").exists():
                log(f"pid {p.pid} did not exit after SIGTERM — sending SIGKILL", level="CRITICAL")
                subprocess.run(["kill", "-KILL", str(p.pid)], check=False, timeout=3.0)
        except Exception as e:
            log(f"kill command failed: {e}", level="ERROR")
            return None
        return p
    log("no killable (non-whitelisted) processes found", level="ERROR")
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--threshold-gb", type=float, default=HARD_KILL_GB,
                    help=f"Hard-kill threshold in GB (default {HARD_KILL_GB})")
    ap.add_argument("--warn-gb", type=float, default=WARN_GB,
                    help=f"Warning threshold in GB (default {WARN_GB})")
    ap.add_argument("--interval", type=float, default=POLL_INTERVAL_S,
                    help=f"Poll interval in seconds (default {POLL_INTERVAL_S})")
    ap.add_argument("--dry-run", action="store_true",
                    help="Log what WOULD be killed but do not actually kill")
    ap.add_argument("--exit-on-kill", action="store_true",
                    help="Exit with code 2 after a kill event (for CI testing)")
    args = ap.parse_args()

    stop = {"flag": False}

    def handle(sig, _frame):
        log(f"received signal {sig}, shutting down")
        stop["flag"] = True

    signal.signal(signal.SIGINT, handle)
    signal.signal(signal.SIGTERM, handle)

    log(f"started: threshold={args.threshold_gb} GB warn={args.warn_gb} GB "
        f"interval={args.interval}s dry_run={args.dry_run}")
    log(f"hardware: {TOTAL_VRAM_GB} GB total (RTX 5090)")

    kills_performed = 0
    try:
        while not stop["flag"]:
            used = get_vram_used_gb()
            if used >= args.threshold_gb:
                procs = list_gpu_processes()
                log(f"VRAM {used:.2f} GB >= {args.threshold_gb} GB threshold; "
                    f"{len(procs)} GPU processes", level="CRITICAL")
                for p in procs:
                    log(f"  pid={p.pid} used={p.used_mib} MiB cmd={p.cmd}", level="CRITICAL")
                killed = kill_newest_non_whitelisted(procs, args.dry_run)
                if killed is not None:
                    kills_performed += 1
                    log(f"kill #{kills_performed}: pid={killed.pid} cmd={killed.cmd}",
                        level="CRITICAL")
                    if args.exit_on_kill:
                        return 2
                # don't sleep immediately — re-check next iteration
            elif used >= args.warn_gb:
                log(f"VRAM {used:.2f} GB >= {args.warn_gb} GB warning threshold", level="WARN")
            else:
                log(f"VRAM {used:.2f} GB / {TOTAL_VRAM_GB} GB "
                    f"({100*used/TOTAL_VRAM_GB:.1f}% — {args.threshold_gb-used:.2f} GB to kill)",
                    level="INFO")
            # Sleep in small increments to react to SIGINT quickly
            slept = 0.0
            while slept < args.interval and not stop["flag"]:
                time.sleep(min(0.5, args.interval - slept))
                slept += 0.5
    except Exception as e:
        log(f"fatal error: {e}", level="ERROR")
        return 1
    log(f"shutdown clean — {kills_performed} kill events this session")
    return 0


if __name__ == "__main__":
    sys.exit(main())
