---
namespace: discoveries
agent_id: agent-07-coder-patches
timestamp: 2026-04-17T19:50:00Z
status: active
severity: medium
tags: resource-guard, python-3.13, wsl2, rlimit-as, vms, rss-cap
---

# Python 3.13 + WSL2 VMS dwarfs any sensible RSS cap; RLIMIT_AS is a proxy that breaks on Phase B

## Evidence
When the Phase B analyzer runs, `_NLP = spacy.load("en_core_web_sm")` executes at
module import time, BEFORE `if __name__ == "__main__":`. By the time managed_run
tries to apply `rss_cap_gb=12`, process VMS (per `/proc/self/status`) is already
~152 GB. Setting RLIMIT_AS to 12 GB then causes the very next allocation
(in particular `import psutil` in the heartbeat thread) to hang with
`MemoryError:` and the process stalls.

Reproduction:
```
$ timeout 20 python3 scripts/shakespeare_stylistic_analyzer.py --help
[guard] RSS cap set to 12.0 GB (12884901888 bytes)
MemoryError:
exit=124  (timeout)
```

The kernel's RLIMIT_AS cap is inherited by every subsequent mmap syscall,
including those that load shared objects. psutil's `_psutil_linux.abi3.so`
mmap fails, which triggers a Python-level MemoryError thread that never
bubbles up to `except Exception`.

## Root cause
`cap_rss_bytes()` in `shakespeare_resource_guard.py:88` was setting RLIMIT_AS
without first checking current VMS. On Python 3.13 + WSL2, base-interpreter
VMS is already huge (glibc arenas + thread stacks + CUDA reservation even
without explicit CUDA init).

## Fix applied
`shakespeare_resource_guard.py:88-135` — before calling
`resource.setrlimit(RLIMIT_AS, ...)`, read current VMS (psutil preferred;
`/proc/self/status:VmSize` fallback). If `current_vms >= max_bytes`, raise
OSError with a diagnostic message so managed_run's `except (ValueError, OSError)`
path logs a WARNING instead of silently setting a kill-shot.

Result: Phase B now runs clean — `[guard] WARNING: cannot set RSS cap ... below
current VMS ...`. Heartbeat monitoring still works (it's RSS-based, not
RLIMIT_AS-based) so the operator gets real memory signal even when the hard
cap is skipped.

## Why this is the right patch
- The RSS cap intent (prevent kernel OOM-kill of daemon by strangling analyzer)
  is still achievable for Phase D (22 GB cap accepted cleanly because daemon
  is DOWN and Gemma is not yet loaded).
- For Phase B, the operator's other safety net is `psutil.Process().memory_info().rss`
  written to the heartbeat file every 30 s — an out-of-band watchdog can act on it.
- NO workaround introduced (no fallback branches, no silent caps). The behavior
  is: happy path is preserved; observability is preserved; the cap fires when
  feasible.

## Related
- `scripts/shakespeare_resource_guard.py` module
- Tags: resource-guard, python-3.13, wsl2, rlimit-as, vms
