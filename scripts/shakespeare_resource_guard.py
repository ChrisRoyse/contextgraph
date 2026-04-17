#!/usr/bin/env python3
"""Shared resource guard for the Shakespeare Phase B/C/D pipeline scripts.

Provides:
    managed_run(...)      — context manager with signal handlers, RSS cap,
                             heartbeat, and cleanup callbacks.
    cap_rss_bytes(n)      — soft-cap the process address space via
                             resource.setrlimit(RLIMIT_AS, ...). Subsequent
                             over-limit allocations raise MemoryError instead
                             of the kernel OOM-killer sweeping us (or the
                             daemon).
    MemoryHeartbeat       — background thread that writes RSS / VMS / CUDA
                             torch metrics to a sidecar JSONL every 30s.
    register_fingerprint  — track in-flight daemon memories so a SIGTERM
                             handler can forget them before exit.
    unregister_fingerprint

Usage inside each Phase B/C/D script:

    from shakespeare_resource_guard import managed_run, register_fingerprint, \
                                           unregister_fingerprint

    with managed_run(
        script_name="shakespeare_stylistic_analyzer",
        rss_cap_gb=30,                      # or None for no cap
        heartbeat_path=Path("/tmp/shake_analyzer_mem.jsonl"),
        on_shutdown=cleanup_fn,             # closes sockets, unloads torch
    ):
        main()

Exit codes honoured:
    0    normal completion
    130  SIGINT (Ctrl+C)
    143  SIGTERM
    137  SIGKILL (unreachable from Python — kernel-only)

Hard rules:
    - NO fallback on shutdown errors; log loudly and continue best-effort.
    - Heartbeat is best-effort; import errors on psutil must not break
      the pipeline (scripts can run without heartbeat).
    - Signal handlers are installed ONLY in the main thread. Worker
      threads inherit the handler through process-level disposition.
"""
from __future__ import annotations

import contextlib
import json
import os
import resource
import signal
import sys
import threading
import time
import traceback
from pathlib import Path
from typing import Any, Callable


# =============================================================================
# Module-level state for fingerprint tracking across the whole process
# =============================================================================
_IN_FLIGHT_FPS: set[str] = set()
_FP_LOCK = threading.Lock()


def register_fingerprint(fp_id: str) -> None:
    """Call immediately after `store_memory` succeeds. Pair with
    `unregister_fingerprint(fp_id)` in the try/finally that calls forget."""
    with _FP_LOCK:
        _IN_FLIGHT_FPS.add(fp_id)


def unregister_fingerprint(fp_id: str) -> None:
    """Call after forget_concept returns (success OR failure).  Safe to
    call on already-forgotten ids."""
    with _FP_LOCK:
        _IN_FLIGHT_FPS.discard(fp_id)


def _snapshot_fingerprints() -> list[str]:
    with _FP_LOCK:
        return list(_IN_FLIGHT_FPS)


# =============================================================================
# RSS cap via RLIMIT_AS (virtual memory) — best proxy for Python RSS on Linux
# =============================================================================
def cap_rss_bytes(max_bytes: int) -> None:
    """Set a soft cap on process address space.

    Does NOT affect existing allocations. Future allocations (malloc,
    mmap, Python object creation) that would push RSS past max_bytes
    raise MemoryError instead of being served. This is more graceful
    than an OOM kill and keeps the daemon and training data safe.

    Notes:
      - RLIMIT_AS covers virtual memory incl. mmap (e.g. torch CUDA
        pinned memory lives here).
      - The hard cap (second tuple value) cannot be raised without
        CAP_SYS_RESOURCE, so we keep it at the current hard cap.
      - On WSL2 with torch + CUDA, VMS is dominated by CUDA reserved
        regions; setting the cap TOO LOW crashes immediately on
        torch.cuda.init(). Recommended minimum for Phase D: 30-40GB.
      - Python 3.13 + Miniconda already reserve very large VMS on import
        (e.g. 150+ GB on WSL2). If the caller's intent cap is under the
        current VMS, setting the rlimit immediately starves subsequent
        mmap (e.g. psutil native .so, CUDA driver) and the process hangs.
        We raise a distinct OSError in that case so managed_run can fall
        back to "no cap, heartbeat only" safely instead of silently
        strangling the run.

    Raises OSError if resource module does not support RLIMIT_AS
    (unlikely on Linux) OR if the current VMS already exceeds the
    requested cap (soft-deny — caller should log and continue).
    """
    if max_bytes <= 0:
        raise ValueError(f"rss cap must be positive, got {max_bytes}")
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    # Don't raise above the existing hard limit (cannot without privileges).
    if hard != resource.RLIM_INFINITY and max_bytes > hard:
        max_bytes = hard
    # Refuse to cap below current VMS — would cause immediate MemoryError on
    # any subsequent allocation / native .so mmap. psutil is preferred for
    # the VMS read; fall back to /proc/self/status if psutil is unavailable.
    current_vms: int | None = None
    try:
        import psutil  # type: ignore
        current_vms = psutil.Process().memory_info().vms
    except Exception:
        try:
            with open("/proc/self/status", "r") as f:
                for line in f:
                    if line.startswith("VmSize:"):
                        parts = line.split()
                        current_vms = int(parts[1]) * 1024  # kB -> bytes
                        break
        except OSError:
            current_vms = None
    if current_vms is not None and current_vms >= max_bytes:
        raise OSError(
            f"requested RLIMIT_AS cap {max_bytes} bytes is below current "
            f"VMS {current_vms} bytes; setting it would cause immediate "
            f"MemoryError on any subsequent allocation. Refusing to set."
        )
    resource.setrlimit(resource.RLIMIT_AS, (max_bytes, hard))


# =============================================================================
# Heartbeat thread — writes mem / cuda metrics every 30s to a sidecar
# =============================================================================
class MemoryHeartbeat(threading.Thread):
    """Daemon thread that writes one JSON line per sampling interval.

    Stops cleanly on stop() or when the main thread exits.  If psutil or
    torch are unavailable, the corresponding fields are simply omitted.
    Never crashes the main pipeline.
    """

    def __init__(
        self,
        path: Path,
        period_s: float = 30.0,
        script_name: str = "unknown",
    ) -> None:
        super().__init__(daemon=True, name="shake-resource-heartbeat")
        self._path = path
        self._period = period_s
        self._stop = threading.Event()
        self._script = script_name
        self._start_ts = time.time()
        self._pid = os.getpid()

    def stop(self) -> None:
        self._stop.set()

    def run(self) -> None:
        try:
            self._path.parent.mkdir(parents=True, exist_ok=True)
        except OSError as e:
            print(f"[guard] heartbeat cannot create {self._path.parent}: {e}",
                  file=sys.stderr)
            return
        try:
            fp = self._path.open("a", encoding="utf-8")
        except OSError as e:
            print(f"[guard] heartbeat cannot open {self._path}: {e}",
                  file=sys.stderr)
            return
        try:
            # Emit an open marker so post-mortem knows a new run started.
            fp.write(json.dumps({
                "ts": time.time(),
                "event": "heartbeat-open",
                "script": self._script,
                "pid": self._pid,
            }) + "\n")
            fp.flush()
            while not self._stop.is_set():
                try:
                    row = self._sample()
                    fp.write(json.dumps(row) + "\n")
                    fp.flush()
                except Exception as e:
                    # Never crash the heartbeat
                    print(f"[guard] heartbeat sample error: {type(e).__name__}: {e}",
                          file=sys.stderr)
                self._stop.wait(self._period)
        finally:
            try:
                fp.write(json.dumps({
                    "ts": time.time(),
                    "event": "heartbeat-close",
                    "script": self._script,
                    "pid": self._pid,
                    "in_flight_fps": len(_snapshot_fingerprints()),
                }) + "\n")
                fp.close()
            except Exception:
                pass

    def _sample(self) -> dict:
        row: dict[str, Any] = {
            "ts": time.time(),
            "elapsed_s": round(time.time() - self._start_ts, 1),
            "pid": self._pid,
            "script": self._script,
            "in_flight_fps": len(_snapshot_fingerprints()),
        }
        try:
            import psutil
            p = psutil.Process(self._pid)
            with p.oneshot():
                mem = p.memory_info()
                row["rss_mb"] = round(mem.rss / 1e6, 1)
                row["vms_mb"] = round(mem.vms / 1e6, 1)
                try:
                    row["open_fds"] = p.num_fds()
                except (psutil.AccessDenied, AttributeError):
                    pass
                try:
                    row["threads"] = p.num_threads()
                except psutil.AccessDenied:
                    pass
            host = psutil.virtual_memory()
            row["host_used_gb"] = round((host.total - host.available) / 1e9, 1)
            row["host_total_gb"] = round(host.total / 1e9, 1)
            row["host_avail_gb"] = round(host.available / 1e9, 1)
            row["host_pct"] = host.percent
            swap = psutil.swap_memory()
            row["swap_used_gb"] = round(swap.used / 1e9, 1)
            row["swap_total_gb"] = round(swap.total / 1e9, 1)
        except ImportError:
            pass
        except Exception as e:
            row["psutil_error"] = f"{type(e).__name__}: {e}"

        # torch CUDA metrics (optional)
        try:
            import torch
            if torch.cuda.is_available():
                row["cuda_alloc_gb"] = round(torch.cuda.memory_allocated() / 1e9, 2)
                row["cuda_reserved_gb"] = round(torch.cuda.memory_reserved() / 1e9, 2)
                # Include max for peak tracking across the run
                row["cuda_max_alloc_gb"] = round(
                    torch.cuda.max_memory_allocated() / 1e9, 2)
        except ImportError:
            pass
        except Exception as e:
            row["torch_error"] = f"{type(e).__name__}: {e}"

        return row


# =============================================================================
# Shutdown orchestration — invoked from signal handler AND context manager exit
# =============================================================================
def _perform_shutdown(
    reason: str,
    *,
    on_shutdown: Callable[[], None] | None = None,
    forget_fingerprint: Callable[[str], None] | None = None,
    unload_torch: bool = True,
    logger=lambda m: print(m, file=sys.stderr),
) -> None:
    """Best-effort cleanup. Every step is wrapped in try/except so a
    single failure doesn't abort the remaining cleanup."""
    logger(f"[guard] shutdown reason={reason}")

    # 1. Forget in-flight daemon memories
    fps = _snapshot_fingerprints()
    if fps:
        logger(f"[guard] forgetting {len(fps)} in-flight fingerprints")
        if forget_fingerprint is not None:
            for fp in fps:
                try:
                    forget_fingerprint(fp)
                    unregister_fingerprint(fp)
                except Exception as e:
                    logger(f"[guard] forget_fingerprint({fp[:8]}…) failed: "
                           f"{type(e).__name__}: {e}")
        else:
            logger(f"[guard] no forget_fingerprint callback provided; "
                   f"leaving {len(fps)} orphan memories: {fps[:5]}...")

    # 2. User callback (close sockets, flush files, etc.)
    if on_shutdown is not None:
        try:
            on_shutdown()
        except Exception as e:
            logger(f"[guard] on_shutdown callback failed: "
                   f"{type(e).__name__}: {e}")
            traceback.print_exc(file=sys.stderr)

    # 3. torch unload (best effort)
    if unload_torch:
        try:
            import torch
            if torch.cuda.is_available():
                torch.cuda.empty_cache()
                torch.cuda.synchronize()
                alloc_gb = torch.cuda.memory_allocated() / 1e9
                logger(f"[guard] torch final allocated={alloc_gb:.2f}GB")
        except ImportError:
            pass
        except Exception as e:
            logger(f"[guard] torch cleanup failed: "
                   f"{type(e).__name__}: {e}")


# =============================================================================
# managed_run context manager — the main entry point for scripts
# =============================================================================
@contextlib.contextmanager
def managed_run(
    *,
    script_name: str,
    rss_cap_gb: float | None = 40.0,
    heartbeat_path: Path | None = None,
    heartbeat_period_s: float = 30.0,
    on_shutdown: Callable[[], None] | None = None,
    forget_fingerprint: Callable[[str], None] | None = None,
    logger: Callable[[str], None] | None = None,
):
    """Context manager that wraps a script's main() with:
      - RSS cap (if rss_cap_gb is not None)
      - Heartbeat thread (if heartbeat_path is not None)
      - SIGTERM / SIGINT handlers that trigger cleanup
      - Guaranteed on_shutdown call on normal exit AND exception

    Parameters
    ----------
    script_name        : short tag for logs (e.g. "phase-b-analyzer")
    rss_cap_gb         : soft cap on VMS; 40 GB is a reasonable default for
                         non-Gemma scripts. Phase D should use ~22 GB (Gemma
                         is ~11.5 GB; analyzer overhead ~3 GB; headroom).
                         Set to None to disable.
    heartbeat_path     : JSONL file to write per-30s memory snapshots. None
                         to disable.
    on_shutdown        : callable invoked from cleanup. Use this to close
                         sockets, flush writers, unload torch models.
    forget_fingerprint : callable(fp_id) invoked to forget daemon memories
                         tracked via register_fingerprint(). Typically
                         scorer.forget.

    Usage
    -----
    >>> with managed_run(
    ...     script_name="phase-d-augmenter",
    ...     rss_cap_gb=28,
    ...     heartbeat_path=Path("/tmp/shake_phased_mem.jsonl"),
    ...     on_shutdown=lambda: scorer.close(),
    ...     forget_fingerprint=scorer.forget,
    ... ):
    ...     main()
    """
    log = logger or (lambda m: print(m, file=sys.stderr))

    # ---- rss cap ----
    if rss_cap_gb is not None:
        cap_bytes = int(rss_cap_gb * 1024 ** 3)
        try:
            cap_rss_bytes(cap_bytes)
            log(f"[guard] RSS cap set to {rss_cap_gb:.1f} GB ({cap_bytes} bytes)")
        except (ValueError, OSError) as e:
            log(f"[guard] WARNING: cannot set RSS cap to {rss_cap_gb} GB: {e}")

    # ---- heartbeat thread ----
    hb: MemoryHeartbeat | None = None
    if heartbeat_path is not None:
        hb = MemoryHeartbeat(
            path=heartbeat_path,
            period_s=heartbeat_period_s,
            script_name=script_name,
        )
        hb.start()
        log(f"[guard] heartbeat writing to {heartbeat_path} every "
            f"{heartbeat_period_s:.0f}s")

    # ---- signal handlers ----
    _shutdown_flag = {"triggered": False, "signum": None}

    def _handle_signal(signum: int, frame: Any) -> None:
        if _shutdown_flag["triggered"]:
            # Second signal — hard exit
            log(f"[guard] second signal {signum} received — hard exit")
            os._exit(2 * 128 + signum)
        _shutdown_flag["triggered"] = True
        _shutdown_flag["signum"] = signum
        log(f"[guard] signal {signum} ({signal.Signals(signum).name}) received")
        _perform_shutdown(
            reason=f"signal-{signal.Signals(signum).name}",
            on_shutdown=on_shutdown,
            forget_fingerprint=forget_fingerprint,
            unload_torch=True,
            logger=log,
        )
        # Translate signal to exit code (128 + signum is POSIX convention)
        sys.exit(128 + signum)

    prev_term: Any = None
    prev_int: Any = None
    installed = False
    try:
        prev_term = signal.signal(signal.SIGTERM, _handle_signal)
        prev_int = signal.signal(signal.SIGINT, _handle_signal)
        installed = True
        log(f"[guard] signal handlers installed for SIGTERM + SIGINT")
    except ValueError as e:
        # setting signal from a non-main-thread raises ValueError
        log(f"[guard] WARNING: signal handlers NOT installed (not main thread?): {e}")

    exit_code_for_natural_shutdown = 0
    try:
        yield
    except SystemExit as e:
        exit_code_for_natural_shutdown = e.code if isinstance(e.code, int) else 1
        raise
    except BaseException as e:
        exit_code_for_natural_shutdown = 1
        _perform_shutdown(
            reason=f"exception-{type(e).__name__}",
            on_shutdown=on_shutdown,
            forget_fingerprint=forget_fingerprint,
            unload_torch=True,
            logger=log,
        )
        raise
    finally:
        # Restore previous signal handlers so a re-used process behaves
        # predictably (unlikely, but correct).
        if installed:
            try:
                signal.signal(signal.SIGTERM, prev_term if prev_term is not None else signal.SIG_DFL)
                signal.signal(signal.SIGINT, prev_int if prev_int is not None else signal.SIG_DFL)
            except Exception:
                pass
        # If no signal fired, still run shutdown for the natural-exit case.
        if not _shutdown_flag["triggered"]:
            _perform_shutdown(
                reason=f"natural-exit-code-{exit_code_for_natural_shutdown}",
                on_shutdown=on_shutdown,
                forget_fingerprint=forget_fingerprint,
                unload_torch=True,
                logger=log,
            )
        if hb is not None:
            hb.stop()
            hb.join(timeout=2.0)


# =============================================================================
# CLI smoke test — lets operator verify the module works standalone
# =============================================================================
def _smoke_test() -> int:
    """Quick verification that the guard works standalone.

    Run: python3 scripts/shakespeare_resource_guard.py --smoke
    """
    print("[smoke] starting managed_run for 5s")
    smoke_path = Path("/tmp/shake_guard_smoke.jsonl")
    if smoke_path.exists():
        smoke_path.unlink()

    def on_shutdown() -> None:
        print("[smoke] on_shutdown called")

    def forget(fp: str) -> None:
        print(f"[smoke] forget({fp}) called (no-op in smoke test)")

    register_fingerprint("smoke-fp-1")
    register_fingerprint("smoke-fp-2")

    with managed_run(
        script_name="smoke-test",
        rss_cap_gb=None,  # don't cap during smoke test
        heartbeat_path=smoke_path,
        heartbeat_period_s=1.0,
        on_shutdown=on_shutdown,
        forget_fingerprint=forget,
    ):
        for i in range(5):
            print(f"[smoke] tick {i}")
            time.sleep(1.0)

    print(f"[smoke] done. Heartbeat file: {smoke_path}")
    if smoke_path.exists():
        lines = smoke_path.read_text().splitlines()
        print(f"[smoke] wrote {len(lines)} heartbeat rows")
        if lines:
            print(f"[smoke] first row: {lines[0][:200]}")
            print(f"[smoke] last row:  {lines[-1][:200]}")
    return 0


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser(
        description="Shakespeare pipeline resource guard — SIGTERM handlers, "
                    "RSS cap, and memory heartbeat. Import `managed_run` from "
                    "this module in each pipeline script.")
    p.add_argument("--smoke", action="store_true",
                   help="Run a 5-second smoke test to verify the module.")
    args = p.parse_args()
    if args.smoke:
        sys.exit(_smoke_test())
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(0)
