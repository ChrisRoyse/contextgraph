---
namespace: journal
agent_id: coordinator
timestamp: 2026-04-18T08:50:00Z
status: active
tags: journal, v4, phase-2, generation, background-task
---

# Journal — v4 Phase 2 Response Generation Launched

## State at launch

- Commit: `3479ff48` (v4 prompt bank + infra), pushed to origin/main.
- v3 LoRA: deleted (user requested).
- Daemon: UP on 3100, 13 embedders loaded, ~17.5 GB VRAM.
- Gemma-4-E4B + v2.1 LoRA: loads in ~10s, takes ~11.5 GB VRAM. Combined with daemon: ~29 GB of 31.8 GB VRAM. Tight, no room for anything else.
- Inference server NOT started — Phase 2 loads LoRA directly in Python script for efficiency (no HTTP overhead).

## Smoke test results (v1, MAX_LEN=900, MAX_ATTEMPTS=3)

- 20 prompts, **55% accepted**, 45% REJECTED_FILTER (all `too_long(1000+)`)
- Rate: 0.02 rec/s → ETA 125 hr for 9K. Too slow.

## Smoke retest (v2, MAX_LEN=1500, MAX_ATTEMPTS=2)

- 20 prompts, **95% accepted**, 0% filter-rejected, 5% score-rejected (0.472 < 0.50 threshold)
- Rate: 0.05 rec/s → ETA ~50 hr for 9K. Acceptable.
- Style score distribution: min=0.501, median=0.568, max=0.699 (all above 0.50)
- Length distribution: min=218, median=445, max=1468 chars
- Attempts: 17/19 on first attempt (89%), 2/19 needed retry

## Full run launch

```bash
python3 scripts/shakespeare_v4_generate_responses.py --heartbeat-every 100
# logging to /tmp/v4-phase2-full.log
# output to ~/.contextgraph/training/shakespeare_v4_sft.jsonl (via .partial)
```

ETA: ~50 hours (actual may be faster once Gemma + LoRA caches stabilize).

## Expected final corpus

At 95% pass rate: **~8,550 accepted training pairs** in `shakespeare_v4_sft.jsonl`.

## Risks

1. **VRAM tightness** (29/31.8 GB) — any memory growth could OOM. If OOM strikes, options are:
   - Reduce Gemma max_new_tokens further (currently 360)
   - Set max_seq_length explicit cap in HF generate config
   - Load Gemma in 4-bit but offload some layers to CPU (slow)
2. **Daemon stability over 50h** — prior session showed daemon PPID=1 orphan kill. This run uses run_in_background bash shell for parent, but bash could die.
3. **Host crash** — 50h is long. Single host crash loses all progress. Mitigation: `.partial` resume-safe, script will pick up where it left off on restart.
4. **Storage growth** — each stored+forgotten memory still leaves RocksDB entries. 9K memories × 13 embeddings could add ~500 MB to the 4.4 GB DB. Not critical.

## Resume command (if crash)

```bash
# Just re-run — will skip already-processed prompt_ids via .partial scan
python3 scripts/shakespeare_v4_generate_responses.py --heartbeat-every 100 > /tmp/v4-phase2-full.log 2>&1 &
```

## Next agent's job (Phase 3: SFT training)

After Phase 2 completes (ETA 2026-04-20):
1. Verify accepted >= 7,000 records
2. Strip REJECTED_* records, retain only ACCEPTED → `shakespeare_v4_sft_final.jsonl`
3. Kill daemon (frees 17 GB VRAM for training)
4. Run rank-256 SFT training on the corpus (~2-4 hours)
5. FSV the resulting model against v2.1 baseline (5-prompt test harness from v3)
6. If v4 ≥ v2.1 → publish as v4.0
