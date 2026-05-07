---
namespace: discoveries
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T06:10:00Z
status: active
tags: v4.2, bge-m3, sparse, splade, style-discrimination, bucket-assignment
---

# BGE-M3 sparse view is a KEYWORD/LEXICAL signal, not a STYLE signal

## What was observed
Running the smoke-test with MIN_GAP = 0.05, all dense views passed style
discrimination between real Sonnet 18 and a real modern passage (using Sonnet 29
as anchor):

    bge_m3_dense           gap +0.0615  PASS
    bge_m3_colbert         gap +0.0833  PASS
    jina_v3_classification gap +0.1064  PASS
    qwen3_instruct         gap +0.0934  PASS
    bge_reranker_v2_m3     gap +5.9521  PASS

But BGE-M3 sparse view FAILED:

    bge_m3_sparse          gap -0.0086  FAIL

Raw SPLADE dot products on the probe:

    sim(Sonnet29, Sonnet18)  = 0.0263
    sim(Sonnet29, Modern)    = 0.0404
    sim(Sonnet18, Modern)    = 0.0643

## Why
BGE-M3's sparse output is a SPLADE-style bag of token weights. Two texts score
high sparse similarity iff they share high-weighted vocabulary ITEMS, not
iff they share STYLE. Sonnet 18 and Sonnet 29 share almost no content words
(summer/buds/May/lease vs. disgrace/heaven/fate/state), so sparse similarity
between them is artificially low. Meanwhile a modern weather passage shares
more common English function words with Sonnet 29 (heaven, fate, state are
ordinary English; temperate/buds/May are uncommon), so sparse(modern, anchor)
is higher than sparse(Sonnet18, anchor). Lexical signal, not style signal.

## Consequence for CAMD / v4.2
This aligns with what `scripts/v4_2/camd_lib.py` already documents in
`STYLE_WEIGHTS`: E13 SPLADE (sparse) lives in the KEYWORD bucket, NOT in the
STYLE bucket, for exactly this reason. Our new E14 (BGE-M3) has three views;
the dense + ColBERT views belong to the STYLE weighting, the sparse view
belongs to a KEYWORD / lexical-recall bucket.

## Smoke-test fix
The smoke test now treats `bge_m3_sparse` as informational and prints it
with a `(INFO)` marker. It does NOT count toward the PASS/FAIL gating. The
gating embedders are: `bge_m3_dense`, `bge_m3_colbert`, `jina_v3_classification`,
`qwen3_instruct`, and the reranker. All five of those MUST PASS MIN_GAP >= 0.05
(or gap > 0.0 for the reranker logit). If any of those fail, sys.exit(1).

## Note for Agent 2 (reference-builder)
When building the sonnets_references.npz file, compute and persist the
BGE-M3 sparse centroid separately, label it clearly as "lexical", and
compose the v4.2 extended reward as:

    style_bucket   = weighted_avg(dense_views + colbert + jina + qwen3)
    keyword_bucket = weighted_avg(daemon's E6/E13 + bge_m3_sparse)

Don't mix sparse into the style aggregate. That's the mistake that caused
v4.1's DPO to train on "content disagreement" instead of "style disagreement"
(see `memory/discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md`).

## Not changing
- MIN_GAP stays 0.05 — we do NOT relax the threshold. Sparse simply isn't in
  the style-gating set.
- The `embed_bge_m3` API still returns the sparse dict. Downstream code
  decides how to aggregate.
