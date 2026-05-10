# v4.2 — CAMD-DPO Foundation

Implementation of **Method 1 + Method 2** from `docs/V4_2_NOVEL_METHODS.md` —
Constellation-Anchored Multi-View DPO (CAMD-DPO) pair mining and constellation
reward function.

Nothing here trains a model yet. This layer produces the **training signal**
that downstream DPO consumes. Everything else (EGTL, MVAG, DPO Trainer
integration) depends on this foundation being correct.

## Components

| File | What it is |
|------|------------|
| `camd_lib.py` | Reusable library — ConstellationScorer, CAMDPairFilter, data classes |
| `mine_camd_pairs.py` | CLI that mines DPO pairs from CF_CONTRASTIVE_PAIRS and writes JSONL |
| `validate_camd.py` | 3-part validation: reward sanity, pair discrimination, stats |

## Prerequisites

1. Context Graph daemon running on `127.0.0.1:3100`:
   ```bash
   CONTEXTGRAPH_DATA_ROOT=/mnt/d/contextgraph \
   CONTEXT_GRAPH_STORAGE_PATH=/mnt/d/contextgraph/storage/contextgraph-rocksdb \
     ./target/release/context-graph-mcp --daemon-server-only --daemon-port 3100
   ```

2. CF_CONTRASTIVE_PAIRS must be populated. As of 2026-04-21, `count_contrastive_pairs`
   reports **13,465 pairs** already mined during corpus ingestion. If empty, run:
   ```bash
   # via daemon's mine_contrastive_pairs tool
   ```

3. Sonnets constellation `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` must exist
   (313 members, coherence 0.9036).

## Quickstart

```bash
# 1. Verify the reward function discriminates
python3 scripts/v4_2/camd_lib.py
# Expected: Shakespeare style_score ≥ 0.45, modern ≤ 0.40, gap ≥ 0.15

# 2. Mine ~25 CAMD pairs for smoke test
python3 scripts/v4_2/mine_camd_pairs.py \
    --target 25 \
    --max-input 500 \
    --max-text-len 8000 \
    --output /tmp/camd_smoke.jsonl

# 3. Validate the mined pairs
python3 scripts/v4_2/validate_camd.py --pairs /tmp/camd_smoke.jsonl

# 4. Production mining run (15-25h; plan accordingly)
python3 scripts/v4_2/mine_camd_pairs.py \
    --target 5000 \
    --max-input 13500 \
    --max-text-len 8000 \
    --output ~/.contextgraph/training/v4_2_camd_pairs.jsonl
```

## Why this is novel

Public DPO pair construction uses one of three strategies, all of which have
known failure modes:

1. **Judge-LLM ranking** (e.g., UltraFeedback, PairRM) — $$$, noisy, biased
   towards the judge's own preferences, non-reproducible.
2. **Heuristic edit-distance** — cheap but produces pairs where rejected is a
   corrupted version of chosen, not a genuinely different record.
3. **On-policy sampling from the model being trained** — feedback loop
   reinforces model artifacts; can cause mode collapse.

**CAMD-DPO uses none of these.** It uses the Sonnets constellation — a
pre-computed centroid + covariance in 13-embedder space — as an **absolute,
deterministic reference anchor**. Pairs are mined from the 13,465 existing
contrastive pairs where:

- **E1 (semantic) similarity is HIGH** → chosen and rejected mean the same thing
- **E12 (ColBERT) + E13 (SPLADE) similarity is LOW** → different style
- **Constellation distance gap is LARGE** → one side is substantially closer
  to Shakespeare than the other

The side with the higher style score becomes `chosen`; the other becomes
`rejected`. No LLM judge required.

## Reward function: what is "style score"?

Combined weighted cosine-to-constellation-centroid across the 5
style-discriminating embedders:

| Embedder | Weight | Role |
|----------|--------|------|
| E12 (ColBERT) | 0.30 | Per-token late-interaction — strongest style signal |
| E13 (SPLADE) | 0.25 | Sparse lexical expansion — archaic vocabulary detector |
| E7 (code structure) | 0.15 | Syntax pattern (low for prose, discriminative for verse/meter) |
| E9 (HDC) | 0.15 | Holographic structural robustness |
| E10 (paraphrase) | 0.15 | Asymmetric paraphrase |

These weights are the same as the production dashboard's guard score (mirror
of `shakespeare_chat_dashboard.py::STYLE_EMBEDDER_WEIGHTS`), so what we train
against matches what we enforce at inference time.

## Tuning notes

Based on smoke-test stats (first 500 pairs from CF_CONTRASTIVE_PAIRS):

- **Text length distribution**: median ~3500 chars, p95 ~9500 chars. The
  default `--max-text-len 8000` keeps ~63% of pairs. `--max-text-len 3000`
  keeps only 26% (but those tend to be sonnet-sized).
- **Style similarity distribution**: most pairs have moderate style-similarity.
  `--max-style-sim 0.60` is strict; 0.70 roughly triples yield.
- **Acceptance rate** at loose thresholds (8000/0.70/0.10): ~5-10% of input
  pairs pass all filters. Tight thresholds (3000/0.60/0.15): ~0.5-1%.

Rule of thumb: to get 5000 pairs, plan on processing ~50k-100k contrastive
pairs (we currently have 13k, so you may need to run `mine_contrastive_pairs`
with larger `maxPairs` first to grow the pool).

## DPO output format

One JSON per line, TRL-compatible:

```json
{
  "prompt": "Compose a passage in the voice of William Shakespeare.",
  "chosen":  "Shall I compare thee to a summer's day? ...",
  "rejected": "The weather today is pretty nice. ...",
  "meta": {
    "chosen_id": "...", "rejected_id": "...",
    "chosen_style_score": 0.62, "rejected_style_score": 0.38,
    "score_gap": 0.24,
    "chosen_per_embedder": [0.94, 0.56, ...],
    "rejected_per_embedder": [0.93, 0.51, ...],
    "anomaly_kind": "STYLE_INVERSION",
    "e1_similarity": 0.91, "style_similarity": 0.42,
    "constellation_id": "18ef2460-9ab6-44fc-8d7e-8d7513174ac8"
  }
}
```

The `meta` block is retained so downstream training scripts can apply
additional weighting (e.g., EGTL per-token loss from per_embedder profile,
or confidence weighting from score gap).

## Known limitations + follow-ups

- **Scoring is slow** — ~2-3 seconds per candidate via RPC round-trip. A
  5k-pair mining run takes ~5-6 hours on a warm daemon. Speedups:
  parallelize via multiple Rpc connections (daemon handles concurrent
  clients), or batch scoring if the daemon adds a bulk API.

- **Prompt templates are generic** — 8 stock Shakespeare prompts, rotated.
  Better prompts should come from E11 entity extraction (topic-specific) —
  this is Method 9 in `V4_2_NOVEL_METHODS.md` and is a future enhancement.

- **Single constellation anchor** — everything pivots on Sonnets (313 members).
  Future: multi-anchor (Sonnets + Hamlet + Henry plays) with per-anchor
  weighting for different request types.

- **No counterfactual filter yet** — Method 12 from the plan (reject pairs
  where the chosen would be identical under a modernized prompt) requires
  integrating the causal_simulator agent. Deferred.

## See also

- `docs/V4_2_NOVEL_METHODS.md` — full plan with all 15 methods
- `tmp/SITREP_2026-04-21_151147.md` — v4.1 failure analysis that motivated v4.2
- `scripts/shakespeare_chat_dashboard.py` — reference implementation of
  STYLE_EMBEDDER_WEIGHTS and guard logic
- `scripts/shakespeare_build_dpo_style_pairs.py` — v2.1's pair builder
  (text-substitution approach); CAMD is an upgrade
