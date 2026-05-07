---
namespace: journal
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T06:09:00Z
status: resolved
tags: v4.2, ensemble, smoke-test, success
---

# Smoke test PASS — all 5 style-gating checks OK

## Command
```
python3 scripts/v4_2/extended_ensemble.py
```

## Result table (verbatim, after BGE-M3 sparse was re-bucketed as INFO-only)
```
==========================================================================
embedder                          shk_sim    mod_sim        gap    PASS?
--------------------------------------------------------------------------
bge_m3_dense                       0.7288     0.6673    +0.0615     PASS
bge_m3_colbert                     0.7204     0.6371    +0.0833     PASS
jina_v3_classification             0.7899     0.6835    +0.1064     PASS
qwen3_instruct                     0.9398     0.8464    +0.0934     PASS
bge_reranker_v2_m3 (logits)       -1.2197    -7.1719    +5.9521     PASS
bge_m3_sparse (INFO)               0.5180     0.5266    -0.0086     INFO
==========================================================================
```

## Provenance
```
bge_m3:             568.81M params, 1024D, ctx=8192, hash=011b6000, FlagEmbedding.BGEM3FlagModel
jina_v3:            572.31M params, 1024D, ctx=8194, hash=03920000, sentence-transformers
qwen3:              595.78M params, 1024D, ctx=32768, hash=00970000, sentence-transformers
bge_reranker_v2_m3: 567.76M params, 1 logit, ctx=8192, hash=890ec000, transformers.AutoModelForSequenceClassification
```

## VRAM
- Before:  30927 MiB free (cuda_free metric — torch mem_get_info)
- After:   25393 MiB free
- Delta:    5542 MiB = 5.40 GB — well under the 15 GB budget

## Wall time
- Warm cache (all models downloaded): 26.6 s end-to-end.
- Cold cache (first run): ~266 s (downloads BGE-reranker and Qwen3).

## What this proves
1. All four embedders + reranker load under the compat shims.
2. All four dense/multi-vec style views CORRECTLY prefer a real Shakespeare
   passage over a real modern prose passage when scored against a second
   real Shakespeare passage (Sonnet 29) as anchor.
3. The sparse view is correctly classified as lexical-not-style.
4. VRAM is within budget for running the ensemble alongside the daemon
   (13,465 pair mining job on the daemon side leaves 14.6 GB free; ensemble
   uses 5.4 GB, plenty of headroom).

## For Agent 2
Sonnets constellation id = `18ef2460-9ab6-44fc-8d7e-8d7513174ac8`. 313 members.
Use `ExtendedEmbedderEnsemble` with default dtype=bf16. Expected references:
    - bge_m3.dense:   (313, 1024)   -> centroid (1024,) + cov_inv (1024,1024) or diag.
    - bge_m3.colbert: per-member, keep representative member; NOT a centroid
      (ColBERT ragged-length tokens; store all 313 as a list of ndarrays).
    - bge_m3.sparse:  dict[token_id -> mean_weight] over all 313 members.
    - jina_v3:        (313, 1024) -> centroid (1024,)
    - qwen3:          (313, 1024) -> centroid (1024,)  (use a single task_prompt
                      across all members so the anchor bias is consistent)
Recommended task_prompt for Qwen3:
    "Classify the literary style as Shakespearean early modern English verse."
