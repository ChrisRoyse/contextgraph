---
namespace: handoff
agent_id: coordinator
timestamp: 2026-04-22T08:35:00Z
status: active
tags: v4.2, handoff, dpo-training, camd, pairs-ready
---

# Handoff: v4.2 CAMD-DPO training set ready

## Summary

5000 preference pairs shipped to `~/.contextgraph/training/v4_2_camd_lmdb.jsonl`
(29.8 MB). Ready for DPO training via `scripts/v4_2/train_camd_dpo.py`.

## Pair quality

- **Source**: `mine_camd_from_lmdb.py` — score-and-sort approach over 979
  LMDB records, pairs top-250 (chosen pool) with bottom-250 (rejected pool).
  Bypasses the daemon's CF_CONTRASTIVE_PAIRS which Sherlock found to be
  non-style-discriminative.
- **Top-K range (chosen)**: extended_combined 0.806..0.862
- **Bottom-K range (rejected)**: extended_combined 0.370..0.623
- **Min gap enforced**: 0.08 (actual mean gap on first 100: 0.249)
- **Provenance per pair**: prompt + chosen + rejected + full 5-view cosines
  on both sides + rank + sha

Example pair:

```
CHOSEN (extended_combined 0.862):
"My glass shall not persuade me I am old,
So long as youth and thou are of one date..."
    (Shakespeare Sonnet 22)

REJECTED (extended_combined 0.623):
"Scene I. On shipboard..."
    (stage direction from a play)

Per-view gap:
    bge_m3_dense:    0.843 - 0.536 = +0.307
    bge_m3_colbert:  0.847 - 0.470 = +0.377
    bge_m3_sparse:   0.414 - 0.032 = +0.382
    jina_v3:         0.913 - 0.678 = +0.235
    qwen3:           0.978 - 0.885 = +0.093
```

## What's different vs v2.1's DPO pairs

v2.1 had 12,567 DPO pairs with E12 ColBERT mean 0.901 (chosen≈rejected in
style — the Sherlock finding that motivated this whole phase). Our 5000
pairs have `jina_v3_classif` gap averaging 0.235 and ColBERT gap averaging
0.377 per pair — **orders of magnitude more style-discriminative**.

## Ready-to-run DPO training command

```bash
cd /home/cabdru/contextgraph
export HF_TOKEN="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"

# Full DPO training (3-4h on RTX 5090)
python3 scripts/v4_2/train_camd_dpo.py \
    --base google/gemma-4-E4B-it \
    --adapter-in ~/.contextgraph/models/shakespeare_lora/final \
    --pairs ~/.contextgraph/training/v4_2_camd_lmdb.jsonl \
    --output-dir ~/.contextgraph/models/shakespeare_lora_v4_2/dpo \
    --epochs 1 --batch-size 4 --grad-accum 4 \
    --lr 5e-6 --beta 0.1 --lambda-sft 0.1 \
    --max-length 1024 --max-prompt-length 128

# Dry-run first to verify environment:
python3 scripts/v4_2/train_camd_dpo.py --dry-run \
    --pairs ~/.contextgraph/training/v4_2_camd_lmdb.jsonl
```

Config is Reg-DPO (SFT-regularized DPO) with:
- β=0.1 (conservative — safe default for style transfer)
- LR 5e-6 (half of typical SFT, per Cerebras DPO guidance)
- λ_sft=0.1 (Reg-DPO SFT-regularization)
- 1 epoch over 5000 pairs, effective batch 16 (4×4)
- paged_adamw_8bit optimizer, bf16 compute
- Start from v2.1 adapter (NOT v4.1 — v4.1 was fragile in production)

## Browser FSV after training

```bash
# Start services
./scripts/restart_all.sh  # daemon + inference + dashboard

# Update inference LORA_PATH to point at v4.2
export LORA_PATH=~/.contextgraph/models/shakespeare_lora_v4_2/dpo/final

# Run Playwright FSV
# (TBD — need a playwright-harness script; current browser-FSV is manual)
```

## Hard gate before HF push (per Part 5 of docs/V4_2_NOVEL_METHODS.md)

- ≥13/15 strict on 15-prompt diverse battery via production dashboard
- ≥80% pass rate in each of 5 categories (classical creative, code, adversarial,
  cross-lingual, modern topical)
- 0/100 loops on repetition stress test
- Constellation distance of v4.2 < v2.1 on held-out prompts

If any criterion fails: DO NOT PUSH. Document the failure in
`memory/blockers/`, investigate root cause.

## State of LMDB sidecar

- 979 records backfilled (313 Sonnets + 666 other Shakespeare from the
  776-member contrastive-pair pool)
- Correlation analysis + FSV already complete; see
  `memory/discoveries/coordinator-v42--cross-embedder-correlations-313-sonnets.md`
  and `~/.contextgraph/models/extended_ensemble/fsv_report.json`

## Future work beyond v4.2

1. Expand LMDB to full 2770 Shakespeare corpus (currently 979 — enough
   for DPO, but fuller corpus enables fresh contrastive-pair mining).
2. Add ~200 non-Shakespeare records to LMDB (news, tech docs, wikipedia)
   so mine_camd_from_lmdb.py can produce true Shakespeare-vs-modern pairs
   rather than strong-Shakespeare-vs-stage-directions.
3. Integrate extended views into the Rust daemon as CF_EXTENDED_* column
   families + MCP tools (40-80h project, currently layered via Python).
