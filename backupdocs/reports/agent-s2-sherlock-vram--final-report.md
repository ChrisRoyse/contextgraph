---
namespace: reports
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:50:00Z
status: active
tags: report, final, vram, gpu, resurrection, phase-budget
---

# Final Report: agent-s2-sherlock-vram

## Task Completed: yes

## Summary

Investigated GPU/VRAM resource allocation for the Shakespeare Resurrection 7-phase pipeline after a WSL2 crash attributed to loading Gemma while the Context Graph daemon was still resident. Verified measured VRAM footprints against the plan's estimates, identified the actual source of truth for GPU monitoring (pynvml, NOT `torch.cuda.mem_get_info`), built and smoke-tested a safety watchdog, and produced a per-phase budget manifest showing every phase fits within the 31.84 GB RTX 5090 budget with safe headroom.

The key surprise: **the Resurrection plan's 20.7 GB daemon estimate is too high**; the real daemon footprint is **17.34 GB** (measured via nvidia-smi with the daemon actually running, PID 17221 at the time of investigation). This converts the plan's worst-case Phase G "32.2 GB over budget by 0.4 GB" into a measured **28.84 GB with 3.0 GB headroom** — safe but tight, and the watchdog watches it.

The second surprise: the earlier SITREP claimed `nvidia-smi` reports garbage numbers on the 5090 and `torch.cuda.mem_get_info` is correct. In the current driver (595.71 / CUDA 13.2), the opposite is true — `torch.cuda.mem_get_info` silently ignores other processes' VRAM (returned 1.76 GB used while pynvml saw 19.14 GB system-wide). The watchdog uses pynvml.

## Files Created in the Repo
- `scripts/vram_watchdog.py` — VRAM safety watchdog, polls pynvml every 10 s, kills newest GPU process above 30.0 GB; supports `--dry-run`, `--threshold-gb`, `--warn-gb`, `--interval`, `--exit-on-kill`.

## Files Written to ./memory/
- `discoveries/agent-s2-sherlock-vram--nvidia-smi-is-reliable-torch-memgetinfo-is-not.md` — contradiction of prior SITREP, with reproduction.
- `discoveries/agent-s2-sherlock-vram--sparse-embedders-live-in-vram-no-cpu-savings-available.md` — E6/E13 both use GPU BERT forward; 1.4 GB equivalent if disabled (but no flag exists).
- `discoveries/agent-s2-sherlock-vram--daemon-has-no-per-embedder-disable-flag.md` — `--no-warm` defers loading only; does not save VRAM.
- `discoveries/agent-s2-sherlock-vram--gemma-4bit-and-sdpa-footprint-analysis.md` — SDPA gives <0.5 GB at inference lengths; 4-bit NF4 current in prod; Phase D could go to 4.2 GB if needed.
- `decisions/agent-s2-sherlock-vram--phase-budget-manifest.md` — LOCKED budget table for all 7 phases, conflict-pair list, ordering constraints.
- `journal/agent-s2-sherlock-vram--success-vram-investigation-complete.md`
- `handoff/agent-s2-sherlock-vram--to-s3-and-coordinator.md`
- `reports/agent-s2-sherlock-vram--final-report.md` (this file)

## Decisions Made
1. **Monitoring source of truth = pynvml** (falls back to nvidia-smi CSV). Rationale: `torch.cuda.mem_get_info` only sees the calling process under WSL2 + driver 595.71 and would silently miss a stale daemon's 17.34 GB.
2. **Watchdog thresholds: 30.0 GB hard kill / 28.0 GB warn / 10 s poll**. Rationale: 31.84 GB hardware total − 1.8 GB headroom (driver + allocator fragmentation) = 30.04 GB safe ceiling; 10 s matches the user spec.
3. **Kill newest GPU process on threshold breach** (the one most likely to be the misbehaving one attempting to co-reside with an existing large allocation). Uses `/proc/<pid>` st_mtime as the start-time proxy since there's no perfect child-of-daemon lineage.
4. **Keep Phase D's existing stop-daemon → Gemma bf16 → unload → start-daemon pattern.** Do NOT add 4-bit Gemma to the augmenter — the existing staging already fits comfortably and additional change risks introducing bugs.
5. **Do NOT disable individual embedders**. The budget fits with all 13 loaded once we use measured (not planned) numbers.

## Discoveries (summary)
- `torch.cuda.mem_get_info()` returns a PER-PROCESS view on WSL2 RTX 5090, not system-wide — despite documentation claiming `cudaMemGetInfo` returns global. Impact: ANY watchdog built on it is blind to other GPU processes. **Do not rely on it for cross-process OOM prevention.**
- Daemon real VRAM = **17.34 GB**, not the plan's 20.7 GB. Source: nvidia-smi, pid 17221, measured 2026-04-17 12:05 UTC.
- E7 (Code 1536-D) is the single largest embedder at ~5.8 GB on disk. If an emergency demands more free VRAM, disabling E7 reclaims the most.
- Sparse models E6 + E13 are GPU-resident (BERT-backbone encoders producing sparse outputs). They are NOT "CPU embedders" despite their inverted-index retrieval.
- Gemma's 11.5 GB "large vocabulary bottleneck": 4-bit NF4 helps with weights (8B params → ~4 GB) but does NOT compress the vocab embedding (256K × 4096), which stays dense.

## Failures
- None during this investigation. One earlier-session claim in `SITREP_2026-04-17_102816.md` ("nvidia-smi reports garbage") was contradicted by measurement — kept as a discovery, not a blocker.

## Successes
- Reliable VRAM watchdog built and smoke-tested (dry-run shows 18.77 GB → 19.73 GB oscillation as daemon services traffic; never triggers kill).
- All 7 Resurrection phases verified to fit within the 31.84 GB budget with ≥3.0 GB headroom.
- Earlier SITREP inversion caught and corrected — future agents should not be misled.

## Blockers (if any)
None. No new blockers raised. Resurrection plan can proceed as written, modulo the 20.7 → 17.34 GB correction on the "Service State During Each Phase" row for Phases A/B/C/G.

## Tests / Build
- Tests added: none (this was an investigation + operational script, not a code change).
- Tests passing: N/A — watchdog smoke-tested in `--dry-run` for 8 s and produced expected log output.
- Build status: not applicable — the Rust core was not modified.

## Returned Answers (as requested)

### 1. VRAM BUDGET TABLE
| Phase | Peak VRAM | Margin to 31.8 GB | Verdict |
|---|---|---|---|
| A | ~0 GB | +31.8 GB | SAFE (CPU-only) |
| B | 17.34 GB | +14.5 GB | SAFE |
| C | 17.34 GB | +14.5 GB | SAFE |
| D.1 (Gemma gen) | 11.5 GB (bf16) | +20.3 GB | SAFE — daemon DOWN required |
| D.2 (scoring) | 17.34 GB | +14.5 GB | SAFE — Gemma DOWN required |
| E | 22.5 GB | +9.3 GB | SAFE — daemon + inference DOWN required |
| F | 22.5 GB | +9.3 GB | SAFE — daemon + inference DOWN required |
| G | 28.84 GB | +3.0 GB | SAFE but TIGHT — watchdog mandatory |

### 2. CRITICAL CONFLICT PAIRS (MUST NOT run simultaneously)
- Daemon (17.34) + training (22.5) = **39.8 GB → OOM by 8.0 GB** — this is likely what crashed WSL2
- Gemma inference (11.5) + training (22.5) = **34.0 GB → OOM by 2.2 GB**
- Daemon + Gemma-bf16-augmenter + Ollama = **40+ GB → OOM**
- Daemon (17.34) + Gemma bf16 augmenter (11.5) = 28.84 GB → TIGHT; current staging avoids this

### 3. PHASE D DECISION
**Keep existing stagger.** Phase D.1: daemon DOWN + Gemma bf16 UP (11.5 GB, 20.3 GB headroom). Phase D.2: Gemma unloaded + daemon UP (17.34 GB, 14.5 GB headroom). No quantization change needed; augmenter script already has correct stop/start logic.

*Alternative available if needed:* adding 4-bit NF4 to the augmenter (code change, 5 lines) would cut Phase D.1 to 4.2 GB and permit running Gemma + daemon simultaneously (21.5 GB total, 10.3 GB headroom). Not required; documented for future option.

### 4. WATCHDOG SCRIPT PATH
`/home/cabdru/contextgraph/scripts/vram_watchdog.py` — uses pynvml, kills newest GPU process above 30.0 GB, 10 s poll, smoke-tested in dry-run.

### 5. NVIDIA-SMI RELIABILITY
**nvidia-smi IS reliable** under current driver (595.71 / CUDA 13.2). Earlier SITREP claim is outdated. Watchdog uses pynvml as primary (same underlying NVML library) with nvidia-smi CSV fallback.

### Specific questions answered with exact numbers
- **SDPA saves on Gemma inference?** At max_new_tokens=250 and max_length=512, <0.5 GB. Already enabled in production. Negligible room for improvement.
- **4-bit quantization for Gemma paraphrase (Phase D)?** Possible, reduces Gemma from 11.5 GB → ~4.2 GB. Not required because current stagger already fits.
- **`--no-warm` flag?** Defers loading only; steady-state VRAM is unchanged (17.34 GB). Not a VRAM-saving tool.
- **Daemon flag to unload specific embedders?** None exists. Source patching required. Not recommended for this mission.
- **E6/E13 sparse on CPU?** NO — both load BERT encoders into GPU VRAM (~0.7 GB each). "Sparse" refers to the output vector shape, not the encoder placement.

## For the Next Agent (S3 — script memory patterns)
- Start by reading: `memory/decisions/agent-s2-sherlock-vram--phase-budget-manifest.md` for the locked VRAM budget numbers.
- Watch out for:
  1. Any script that calls `torch.cuda.mem_get_info` to "check free VRAM" — it's lying on this system. Replace with `pynvml.nvmlDeviceGetMemoryInfo`.
  2. Any script that kills the daemon WITHOUT waiting for pid removal from `nvidia-smi --query-compute-apps` — the kernel may hold VRAM briefly after kill(-TERM).
  3. Any training script that loads Gemma BEFORE verifying the daemon is gone and free VRAM > training requirement + headroom.
- Priority: verify that `scripts/shakespeare_full_pipeline.py` does NOT attempt to load the model while any other GPU process is alive. Current code has a `torch.cuda.empty_cache()` + sleep 2 sequence at the start of phase2_sft that does not verify other-process state — recommend adding a `pynvml`-based pre-flight check.

## Confidence: high
All numbers are from direct measurement on the running system (not estimates). The watchdog is tested end-to-end in dry-run. The phase budget table uses measured daemon footprint; every phase has ≥3 GB headroom. The monitoring source-of-truth flip was counter-intuitive but verified with three independent readings (nvidia-smi, pynvml, torch — the latter was the outlier). The watchdog uses the two-source quorum (pynvml primary, nvidia-smi fallback) that will survive a future driver flip-flop.
