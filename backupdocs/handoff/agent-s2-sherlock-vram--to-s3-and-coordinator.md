---
namespace: handoff
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:46:00Z
status: active
tags: handoff, vram, watchdog, phase-d, phase-g
---

# Handoff to S3 (script memory patterns) and the Coordinator

## TL;DR
- **Real daemon VRAM is 17.34 GB**, not 20.7 GB as the plan assumed. All phases fit within budget with safe headroom.
- **Phase G has 3.0 GB headroom**, NOT a 0.4 GB shortfall. The plan's math used stale/conservative daemon estimates.
- **The existing Phase D stagger (daemon DOWN → Gemma bf16 → daemon UP for scoring) is correct.** Keep it. No code change required.
- **Watchdog ready**: `scripts/vram_watchdog.py` — use pynvml, kills newest process over 30 GB.
- **Monitoring flip-of-truth**: use pynvml/nvidia-smi, NOT torch.cuda.mem_get_info (which silently misses other processes on WSL2).

## For S3 (script memory patterns)
When S3 audits script-level memory patterns:
- Every Python script that holds a CUDA model MUST call `torch.cuda.empty_cache()` and `torch.cuda.synchronize()` before Python exit
- The augmenter already does this correctly in `GemmaGenerator.unload()`
- The inference server currently does NOT explicitly unload — it relies on process exit; that's fine because inference is its own process
- Training script (`shakespeare_full_pipeline.py`) loads model at phase2_sft and reuses it in phase3_dpo. That's VRAM-efficient but requires phase E and F to be separate invocations in Resurrection (the mission plan splits E and F, which is good; keep them separate).

## For the Coordinator — operational launch commands

### Start the watchdog BEFORE any phase:
```bash
nohup python3 /home/cabdru/contextgraph/scripts/vram_watchdog.py \
    --threshold-gb 30.0 --warn-gb 28.0 --interval 10 \
    > /tmp/vram_watchdog.stdout 2>&1 &
echo $! > /tmp/vram_watchdog.pid
```

### Between Phase B/C and Phase D:
```bash
# Stop daemon for Phase D.1 (Gemma load)
kill -TERM $(cat /tmp/context-graph-daemon.pid)
# Wait for VRAM to drop (pynvml check), then launch augmenter with --auto-daemon
python3 scripts/shakespeare_sft_augmenter.py --auto-daemon
```

### Between Phase D and Phase E (training):
```bash
# Daemon is already up from D.2 scoring; stop it now
kill -TERM $(cat /tmp/context-graph-daemon.pid)
# Kill any leftover inference server or Ollama
pkill -9 ollama 2>/dev/null || true
pkill -f shakespeare_inference_server 2>/dev/null || true
# Wait until VRAM < 2 GB, then start training
python3 scripts/shakespeare_full_pipeline.py
```

### Between Phase F and Phase G:
```bash
# Training done; restart daemon first, THEN inference
bash /home/cabdru/contextgraph/scripts/ensure_daemon.sh
# wait for daemon warm-up (~20-30 s)
HF_TOKEN=... python3 scripts/shakespeare_inference_server.py --port 8766 &
# verify both are up and watchdog sees ~28.8 GB total usage
```

### Watchdog shutdown (after Phase G validation):
```bash
kill -TERM $(cat /tmp/vram_watchdog.pid)
```

## Open Questions (not blockers)
1. Should the augmenter script ALSO use 4-bit Gemma to shave Phase D from 11.5 to 4.2 GB? 
   - **My call: no, not required.** Current pattern works. Change only if you need to parallelize the Gemma gen with daemon scoring, which would break the existing clean staging.
2. Should Phase G ever try to use the fp16 Gemma instead of 4-bit? 
   - **No.** 4-bit Gemma inference is already production-tested and scores 5/5 on FSV. 
3. Is there ever a case where we'd need E7 (Code embedder, 5.8 GB) disabled? 
   - **Not for this mission.** Shakespeare structural matching uses E7.

## Files I Wrote
### Memory
- `memory/discoveries/agent-s2-sherlock-vram--nvidia-smi-is-reliable-torch-memgetinfo-is-not.md`
- `memory/discoveries/agent-s2-sherlock-vram--sparse-embedders-live-in-vram-no-cpu-savings-available.md`
- `memory/discoveries/agent-s2-sherlock-vram--daemon-has-no-per-embedder-disable-flag.md`
- `memory/discoveries/agent-s2-sherlock-vram--gemma-4bit-and-sdpa-footprint-analysis.md`
- `memory/decisions/agent-s2-sherlock-vram--phase-budget-manifest.md`
- `memory/journal/agent-s2-sherlock-vram--success-vram-investigation-complete.md`
- `memory/handoff/agent-s2-sherlock-vram--to-s3-and-coordinator.md` (this file)
- `memory/reports/agent-s2-sherlock-vram--final-report.md`

### Repo
- `scripts/vram_watchdog.py` — the safety watchdog
