---
namespace: decisions
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:30:00Z
status: active
severity: high
tags: vram, budget, manifest, phase, resurrection
---

# VRAM Budget Manifest — Shakespeare Resurrection Pipeline

## Hardware Constants
- **Total VRAM**: 32,607 MiB = **31.84 GB** (nvidia-smi), 34.19 GB (pynvml). Use the lower **31.8 GB** for ALL budgeting.
- **Safety headroom**: 1.8 GB minimum (nvidia-smi overhead + driver reservations + allocator fragmentation)
- **Hard kill threshold**: 30.0 GB system-wide used → kill newest GPU process
- **Warning threshold**: 28.0 GB system-wide used → log WARN
- **Monitoring source of truth**: `pynvml.nvmlDeviceGetMemoryInfo` (NOT `torch.cuda.mem_get_info`)

## Measured Service Footprints (2026-04-17)
| Service | VRAM (measured) | Plan estimate | Notes |
|---|---|---|---|
| Context Graph daemon (13 embedders, warm) | **17.34 GB** | 20.7 GB | Plan was conservative — real is lower |
| Gemma-4-E4B-it bf16 (augmenter Phase D) | ~11.5 GB | 11.5 GB | Matches |
| Gemma-4-E4B-it 4-bit + LoRA (inference) | **11.5 GB** | 11.5 GB | Surprising: 4-bit didn't help as much as expected; large vocab dominates |
| Gemma-4-E4B-it 4-bit alone (no LoRA, Phase D) | ~4.2 GB | — | If we enable 4-bit in augmenter |
| LoRA training (batch_size=1, grad_accum=32, rank=256) | **~22.5 GB** | 22.5 GB | Matches |

## Phase-by-Phase Budget Table

| Phase | Daemon | Inference | Training | Gemma Aug | Peak VRAM | Headroom | Verdict |
|---|---|---|---|---|---|---|---|
| **A** Hotfix (commit) | off | off | — | — | **~0 GB** | +31.8 GB | SAFE (CPU only) |
| **B** Stylistic analysis | UP (17.34) | off | — | — | **17.34 GB** | +14.5 GB | SAFE |
| **C** DPO pairs + constellation scoring | UP (17.34) | off | — | — | **17.34 GB** | +14.5 GB | SAFE |
| **D.1** Gemma paraphrase generation | DOWN | off | — | 11.5 GB (bf16) or 4.2 GB (4-bit) | **11.5 / 4.2 GB** | +20.3 / +27.6 GB | SAFE — daemon MUST be down |
| **D.2** Constellation scoring pass | UP (17.34) | off | — | off | **17.34 GB** | +14.5 GB | SAFE — Gemma MUST be unloaded |
| **E** SFT retrain | DOWN | DOWN | 22.5 GB | off | **22.5 GB** | +9.3 GB | SAFE |
| **F** DPO retrain | DOWN | DOWN | 22.5 GB | off | **22.5 GB** | +9.3 GB | SAFE |
| **G** Validation (both services up) | UP (17.34) | UP (11.5) | — | — | **28.84 GB** | **+3.0 GB** | SAFE but TIGHT — enable watchdog |

## Critical Conflict Pairs — MUST NOT run simultaneously
1. **Gemma inference server (11.5 GB) + training (22.5 GB)** = 34.0 GB → **OOM (+2.2 GB over)**
2. **Gemma inference server (11.5 GB) + Gemma augmenter bf16 (11.5 GB) + daemon (17.3 GB)** = 40.3 GB → **OOM (+8.5 GB over)**
3. **Daemon (17.3 GB) + training (22.5 GB)** = 39.8 GB → **OOM (+8.0 GB over)**  [This is what crashed WSL2]
4. **Daemon (17.3 GB) + Gemma bf16 augmenter (11.5 GB)** = 28.8 GB → TIGHT (+3.0 GB headroom); works with 4-bit Gemma augmenter (21.5 GB, +10.3 GB) but current augmenter uses bf16. **Keep the stagger.**
5. **Daemon (17.3 GB) + Gemma bf16 augmenter + Ollama** = 40+ GB → **OOM**; Ollama MUST be killed before Phase D

## Locked Phase Ordering (NO violations)

```
A → B → C → [stop daemon] → D.1 (Gemma up) → [unload Gemma] → [start daemon] → D.2 (scoring) →
  [stop daemon] → E → [stop training] → F → [stop training] → [start daemon] → [start inference] → G
```

## Between-phase Transitions (VRAM reset)
Every inter-phase transition MUST:
1. Call `torch.cuda.empty_cache()` in the exiting Python process
2. Call `torch.cuda.synchronize()` before exit
3. Verify the GPU process list via `nvidia-smi --query-compute-apps=pid --format=csv,noheader` is empty (or contains only the expected next-phase process)
4. Optionally wait for `pynvml` reading to drop below 2.0 GB before starting the next phase

## Pre-Flight Checks (script must run before each phase)
- Current pynvml `used` reading
- Running GPU processes (pid + RSS)
- Free VRAM compared to the next-phase minimum requirement
- Daemon/inference/ollama/training process status
- FAIL FAST if the phase would exceed 30.0 GB peak

## Source of Truth
- Hardware spec: `docs2/systemspecs.md` — RTX 5090, 31.84 GB
- Mission plan: `memory/context/mission-resurrection-plan.md` — Service State During Each Phase table
- Monitoring discovery: `memory/discoveries/agent-s2-sherlock-vram--nvidia-smi-is-reliable-torch-memgetinfo-is-not.md`
- Watchdog: `scripts/vram_watchdog.py`
