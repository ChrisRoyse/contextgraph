---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:08:00Z
status: active
severity: high
tags: signals, sigterm, cleanup, orphan-memories, vram-leak
---

# NONE of the three scripts install SIGTERM/SIGINT handlers — Ctrl+C leaks

## Evidence
Grep across all three scripts:
```
$ rg -n 'signal\.|SIGTERM|SIGINT|atexit' scripts/shakespeare_stylistic_analyzer.py \
    scripts/shakespeare_build_dpo_style_pairs.py scripts/shakespeare_sft_augmenter.py
(no matches)
```

Only `KeyboardInterrupt` is caught at `scripts/shakespeare_sft_augmenter.py:2812`:
```python
    except KeyboardInterrupt:
        log("interrupted by user", level="WARN")
        sys.exit(130)
```
And `shakespeare_build_dpo_style_pairs.py` has NO KeyboardInterrupt catch at all.
`shakespeare_stylistic_analyzer.py` has NO KeyboardInterrupt catch.

## Why It Matters
When WSL hits RAM pressure, the kernel OOM-killer sends SIGKILL (uncatchable)
OR the operator sends Ctrl+C (SIGINT). Today's behavior:

1. **Phase B (analyzer)**: In-flight `store_memory` that hasn't reached the
   `forget_concept` line leaves a memory in the daemon. Each failure adds
   one memory slot. Over 7,925 records, even a 1% interrupt rate = 79
   orphaned guard-check memories → 79 × 13 embedders = 1,027 extra HNSW
   vectors → ~4-6MB wasted VRAM per run that accumulates across restarts.

2. **Phase C (DPO)**: Every `rec` may have 1 chosen + up to 4 rejected
   fingerprints in flight. Interrupt mid-variant = 1-4 orphan daemon
   memories × 7,925 records × restart attempts. Even in the best case
   (try/finally is exited before SIGTERM), the chosen_fp lives for the
   entire variant loop (~4 variants × ~2s scoring = 8s window) — plenty
   of room to orphan.

3. **Phase D (augmenter)**: WORST. Gemma is loaded into VRAM (~11.5GB).
   SIGKILL leaves:
   - CUDA context held until process table entry reaped
   - ~11.5GB VRAM unreleased until nvidia-smi reports "dead process"
   - Any in-flight `store_memory` orphaned in daemon

## The try/finally Pairing IS Correct
To the scripts' credit, each one has proper try/finally around
store_memory → forget_concept:
- Analyzer: `shakespeare_stylistic_analyzer.py:407-441` (DaemonClient.score_passage)
- DPO: `shakespeare_build_dpo_style_pairs.py:770-777` (chosen_fp); 770-772 (rejected_fp)
- Augmenter: `shakespeare_sft_augmenter.py:1991-2032` (ConstellationScorer.score)

BUT: Python's try/finally does NOT run if the process is SIGKILL-ed or
SIGTERM-ed without handler. finally DOES run on SIGINT (raises
KeyboardInterrupt) — BUT ONLY if the thread is waiting on Python-level
work. A thread blocked in `socket.recv()` will not see the KeyboardInterrupt
until the recv returns or the socket timeouts (currently 30-60s).

## Fix Recommendation
Install signal handlers at the top of each script OR (better) via a
shared `shakespeare_resource_guard.managed_run()` context manager:

```python
import signal, sys

_SHUTDOWN = threading.Event()
_IN_FLIGHT_FPS: set[str] = set()   # fingerprintIds not yet forgotten
_IN_FLIGHT_LOCK = threading.Lock()
_SCORER_REF = {"scorer": None}  # settable pointer

def _handle_sigterm(signum, frame):
    print(f"[guard] received signal {signum}; forgetting {len(_IN_FLIGHT_FPS)} "
          f"in-flight fingerprints, unloading torch, flushing writers",
          file=sys.stderr)
    _SHUTDOWN.set()
    sc = _SCORER_REF.get("scorer")
    if sc is not None:
        for fp in list(_IN_FLIGHT_FPS):
            try:
                sc.forget(fp)
            except Exception:
                pass
    # Attempt torch unload if available
    try:
        import torch
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
    except ImportError:
        pass
    sys.exit(130 if signum == signal.SIGINT else 143)

signal.signal(signal.SIGTERM, _handle_sigterm)
signal.signal(signal.SIGINT, _handle_sigterm)
```

And in the scorer wrappers: on entry register `fp_id`, on exit deregister.

## Related
- `./memory/decisions/coordinator--guard-architecture.md` (D7: try/finally pairing)
- Tags: signals, cleanup, daemon-memories
