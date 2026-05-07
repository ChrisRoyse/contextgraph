---
namespace: discoveries
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:10:00Z
status: active
severity: high
tags: gpu, vram, monitoring, rtx5090, torch, nvidia-smi, pynvml, observability
---

# nvidia-smi is reliable; torch.cuda.mem_get_info is NOT (contradicts prior SITREP)

## What Happened

Prior SITREP (2026-04-17_102816) claimed:
> `nvidia-smi` reports garbage VRAM numbers on the 5090 driver ("17592181882455 MiB")
> `torch.cuda.mem_get_info()` gives correct readings

Direct measurement on 2026-04-17 12:10 contradicts this:

| Source | Total VRAM | Used VRAM | Notes |
|---|---|---|---|
| `nvidia-smi` (text) | 32,607 MiB (31.84 GB) | 17,330 MiB (16.92 GB) | Matches daemon footprint |
| `nvidia-smi --query-gpu=memory.used` | 32,607 | 17,340 | Structured output, clean integer |
| `pynvml` | 34.19 GB | 19.14 GB | Matches daemon, slightly higher due to pinned/reserved accounting |
| `torch.cuda.mem_get_info()` (other-process view) | 34.19 GB | **1.76 GB** | WRONG — does NOT see daemon's 17.34 GB |

### Reproduction (in a separate Python process)
```python
import torch
free, total = torch.cuda.mem_get_info(0)  # returns only 1.76 GB used
# ...allocate 2 GB tensor...
# torch.mem_get_info jumps to 3.89 GB used — clearly only counts THIS process
# nvidia-smi concurrently reports 21+ GB used system-wide
```

## Why It Matters

A safety watchdog built on `torch.cuda.mem_get_info()` from outside the training process **will not see the training process's allocation** and will fail to protect against OOM.

## Decision for this mission
**Use `pynvml` (or `nvidia-smi --query-gpu=memory.used`) as the source of truth for system-wide VRAM monitoring.** Both agree within 2 GB (allocator bookkeeping difference). Only use `torch.cuda.memory_allocated()` for the CURRENT process's allocation.

## Nuance — why the earlier SITREP may have been right AT THE TIME
Earlier SITREP could have been observing a driver bug that shipped in a specific version. The `nvidia-smi` currently running (Driver 595.71 / CUDA 13.2) reports sensible numbers. If a driver update reintroduces the bug, the watchdog should fall back to pynvml (same underlying NVML library that nvidia-smi uses) — NOT to torch.

## Related
- Previous SITREP incorrect claim: `tmp/SITREP_2026-04-17_102816.md` "RTX 5090 + CUDA 13 Quirks"
- Hardware: `docs2/systemspecs.md` — RTX 5090 with 31.84 GB VRAM
- Watchdog implementation: `scripts/vram_watchdog.py`
