---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:30:00Z
status: active
severity: medium
tags: phase-d, phase-c, phase-b, rss, heartbeat, observability
---

# All three scripts lack RSS cap and per-30s memory heartbeat sidecar

## Evidence
No `resource.setrlimit` anywhere:
```
$ rg -n 'resource\.|setrlimit|RLIMIT' scripts/shakespeare_*.py
(no matches)
```

No periodic memory logging (only per-event progress lines that track
record counts, not RSS/VRAM).

## Why It Matters
In the crash described in `blockers/agent-04-coder-analyzer--oom-kills-daemon.md`,
the host went from OK to OOM-killed in MINUTES. An RSS-cap would have
caused the offending script to MemoryError and exit cleanly, keeping
the daemon and training data safe.

A per-30s memory heartbeat (RSS, VMS, torch cuda allocated/reserved,
GPU util) written to a sidecar file would have made post-mortem
analysis trivial: we'd know exactly which script hit which level at
what time.

## Fix Recommendation

### Shared module shakespeare_resource_guard.py
```python
import os, sys, time, threading, resource
from pathlib import Path

def cap_rss(max_bytes: int) -> None:
    """Soft-cap the address space; subsequent allocations raise MemoryError
    instead of OOM-killing. RLIMIT_AS on Linux covers virtual memory incl.
    mmap'd regions."""
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    new_soft = min(max_bytes, hard if hard != resource.RLIM_INFINITY else max_bytes)
    resource.setrlimit(resource.RLIMIT_AS, (new_soft, hard))

class MemoryHeartbeat(threading.Thread):
    """Emit per-30s memory metrics to a sidecar file."""
    def __init__(self, path: Path, period: float = 30.0):
        super().__init__(daemon=True, name="mem-heartbeat")
        self._path = path
        self._period = period
        self._stop = threading.Event()
        self._start_ts = time.time()
        self._pid = os.getpid()

    def stop(self): self._stop.set()

    def run(self):
        import json
        self._path.parent.mkdir(parents=True, exist_ok=True)
        with self._path.open("a", encoding="utf-8") as f:
            while not self._stop.is_set():
                row = self._sample()
                f.write(json.dumps(row) + "\n")
                f.flush()
                self._stop.wait(self._period)

    def _sample(self) -> dict:
        import psutil
        p = psutil.Process(self._pid)
        mem = p.memory_info()
        d = {
            "ts": time.time(),
            "elapsed_s": round(time.time() - self._start_ts, 1),
            "pid": self._pid,
            "rss_mb": round(mem.rss / 1e6, 1),
            "vms_mb": round(mem.vms / 1e6, 1),
        }
        try:
            import torch
            if torch.cuda.is_available():
                d["cuda_alloc_gb"] = round(torch.cuda.memory_allocated() / 1e9, 2)
                d["cuda_reserved_gb"] = round(torch.cuda.memory_reserved() / 1e9, 2)
        except ImportError:
            pass
        try:
            host = psutil.virtual_memory()
            d["host_used_gb"] = round((host.total - host.available) / 1e9, 1)
            d["host_total_gb"] = round(host.total / 1e9, 1)
        except Exception:
            pass
        return d
```

### Usage per script:
```python
from shakespeare_resource_guard import managed_run

with managed_run(
    script_name="shakespeare_sft_augmenter",
    rss_cap_bytes=30 * 1024**3,   # 30GB
    heartbeat_path=Path("/tmp/shake_sft_aug_mem.jsonl"),
    forget_on_exit=lambda: scorer.close() if scorer else None,
    unload_torch=lambda: gemma.unload() if gemma else None,
):
    main()
```

## Related
- `./memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md`
- Tags: rss, heartbeat, observability
