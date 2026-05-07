---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-22T01:30:00Z
status: active
tags: v4.2, correlations, embedders, redundancy, orthogonality, discoveries, sonnets
---

# Discovery: Cross-embedder correlation atlas on 313 Sonnets (17 views)

## Summary

After backfilling all 313 Sonnets constellation members through the 4 new
extended embedders (BGE-M3 dense, BGE-M3 ColBERT, Jina-v3 classification,
Qwen3-instruct) and computing per-view cosines against the Sonnets centroid,
the full 17×17 Pearson correlation matrix reveals which views are redundant,
which are orthogonal, and where we gain fresh signal. E11 is dead (NaN) and
excluded from analysis.

Raw JSON: `~/.contextgraph/models/extended_ensemble/correlation_findings.json`

## Top-line findings

### Redundant daemon pairs (|r| > 0.9 — candidates for retirement or merging)

| View A | View B | r |
|--------|--------|---|
| E2_recency | E3_periodic | +0.988 |
| E1_semantic | E8_graph | +0.957 |
| E2_recency | E4_sequential | +0.934 |

E2/E3/E4 are all temporal sinusoidal PE embedders; r ≥ 0.93 across the pair
means they encode essentially the same timestamp signal for our corpus.
E1 and E8 diverged by design (semantic vs graph-topology) but at r=0.96
are nearly the same axis in practice — suggests the K-NN graph tracks
semantic neighborhood very tightly.

### Orthogonal daemon-vs-extended pairs (|r| < 0.3 — extended adds NEW signal)

15 cross-block pairs. The strongest novelty comes from:

| Daemon view | Extended view | r |
|-------------|---------------|---|
| E4_sequential | X14_bge_m3_dense | −0.013 |
| E3_periodic | X14_bge_m3_dense | −0.023 |
| E2_recency | X14_bge_m3_dense | −0.023 |
| E9_hdc | X16_jina_v3_classif | +0.074 |

The extended embedders are essentially **orthogonal to the temporal axes** —
which makes sense: they don't see timestamps, only content. This means the
new views contribute genuinely new signal, not a rehash of temporal position.

E9 HDC has near-zero correlation with Jina-v3 classification (r=+0.074) —
HDC's symbolic binding and Jina's classification-LoRA-tuned signal are
measuring fundamentally different things.

### Strongest daemon-extended correlations (cross-content-axes)

| Daemon view | Extended view | r |
|-------------|---------------|---|
| E1_semantic | X14_bge_m3_dense | +0.708 |
| E10_paraphrase | X14_bge_m3_dense | +0.687 |
| E12_colbert | X14_bge_m3_dense | (implicit) |
| E8_graph | X14_bge_m3_dense | +0.640 |
| E5_causal | X14_bge_m3_dense | +0.639 |

All cross-block correlations are **below 0.75**, meaning no extended view
is a simple copy of any single daemon view. The extended ensemble genuinely
adds information to the 17-view stack.

## Per-axis discrimination strength on Sonnets reference

| Axis | Sonnets mean | Sonnets std |
|------|--------------|-------------|
| X17_qwen3_instruct | +0.968 | 0.011 |
| E10_paraphrase | +0.910 | 0.011 |
| E1_semantic | +0.903 | 0.011 |
| E8_graph | +0.902 | 0.011 |
| X16_jina_v3_classif | +0.891 | 0.023 |
| E12_colbert | +0.822 | 0.059 |
| E7_code | +0.800 | 0.029 |
| X14_bge_m3_dense | +0.796 | 0.026 |
| X15_bge_m3_colbert | +0.788 | 0.033 |
| E5_causal | +0.716 | 0.050 |
| ... |

**Qwen3-instruct has the highest Sonnets-reference mean (0.968) with the
lowest std (0.011)**. This means the Sonnets cluster together VERY TIGHTLY
in Qwen3's instruction-aware embedding space. Anything falling below ~0.945
(i.e., 2σ below the Sonnets mean) is outside the Shakespeare manifold
by Qwen3's standards — a useful guard threshold.

## Implications for v4.2 CAMD-DPO

1. **Drop E2/E3/E4 from fusion** when targeting style (they're redundant
   with each other AND orthogonal to style signal). Use E2 alone when you
   need temporal, skip when you need style.
2. **Favor X17 (Qwen3) + X16 (Jina) for style scoring** over E12 (ColBERT)
   and E13 (SPLADE). Extended views are more style-selective for our domain.
3. **New anomaly-kind candidate**: "high E1 semantic + low X17 Qwen3" —
   same meaning but NOT Shakespeare-style. This is precisely the CAMD
   pair criterion we've been wanting.
4. **Reranker dominates as a binary Shakespeare-yes/no signal**: the logit
   gap between Shakespeare and modern is +6.67, vs 0.22 for the cosine-based
   combined reward. Use reranker for the yes/no gate, combined reward for
   the ordinal DPO loss.

## Why this matters

Public datasets can't generate this kind of analysis — nobody else has
multi-view embeddings labeled at this granularity for a single style domain.
The 15 orthogonal daemon-extended pairs mean our extended layer isn't
bolting on more of the same signal; it's genuinely expanding the signal
space, specifically into style-sensitive dimensions that the daemon's
legacy views under-sample.

## Related

- `memory/discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md`
  — the original finding that motivated extended embedders
- `memory/decisions/coordinator-v42-extended-persistence--lmdb-sidecar.md`
  — architectural decision
- `~/.contextgraph/models/extended_ensemble/correlation_findings.json`
  — raw JSON for downstream analysis
