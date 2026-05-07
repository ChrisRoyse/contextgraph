---
namespace: journal
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:45:00Z
status: active
tags: journal, success, vram, investigation, watchdog
---

# VRAM Investigation — Complete

## What I Did
1. Read systemspecs, resurrection plan, both SITREPs, full pipeline script, inference server, ensure_daemon.sh, SMAP protocol (as required).
2. Queried real GPU state via `nvidia-smi`, `torch.cuda.mem_get_info`, and `pynvml`.
3. Discovered CRITICAL inversion of prior SITREP claim: `nvidia-smi` IS reliable; `torch.cuda.mem_get_info` is NOT (see discovery file).
4. Inspected daemon CLI: confirmed `--no-warm` defers but doesn't save VRAM; no per-embedder disable flag.
5. Inspected sparse model code: E6/E13 live fully in VRAM (no CPU savings available).
6. Inspected augmenter: already has stop/start daemon pattern for Phase D — exactly correct.
7. Measured on-disk embedder sizes (12.1 GB total) vs runtime VRAM (17.34 GB) → ~5.2 GB overhead for HNSW/workspaces.
8. Wrote the VRAM budget manifest with measured numbers (real daemon = 17.34 GB, plan assumed 20.7 GB — plan was conservative).
9. Built and smoke-tested the watchdog (`scripts/vram_watchdog.py`) using pynvml as source of truth.

## Key Numbers
| Metric | Value |
|---|---|
| RTX 5090 total VRAM | 31.84 GB (nvidia-smi) / 34.19 GB (pynvml — includes mapped) |
| Daemon measured usage | 17.34 GB (plan said 20.7 — plan was conservative) |
| Gemma bf16 | ~11.5 GB |
| Gemma 4-bit NF4 (current inference) | 11.5 GB total with LoRA (4.2 GB base) |
| Training LoRA rank-256 | ~22.5 GB |
| Phase G peak (real) | 17.34 + 11.5 = 28.84 GB (3.0 GB headroom — SAFE, not "0.4 GB over") |

## Key Decisions Made
1. Use pynvml (not torch.cuda.mem_get_info) as monitoring source of truth.
2. Keep Phase D's existing stagger (daemon down → Gemma → daemon up for scoring).
3. Keep existing inference server's 4-bit NF4 + SDPA configuration for Phase G.
4. Set watchdog hard-kill = 30.0 GB, warn = 28.0 GB, interval = 10 s.
5. Do NOT disable individual embedders (not worth the source-patching effort; budget fits as-is).
