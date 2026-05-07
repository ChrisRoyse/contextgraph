---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:08:00Z
status: active
severity: low
tags: exp2, extract-embeddings, fail-loud, success, source-of-truth
---

# Success: extract_embeddings.py is rigorously fail-loud and reads source-of-truth

## What's good
- Every embedder has a per-key dim check + finiteness check + L2-norm > 0 check (`vector_from_record`, lines 75-93). Hits L1/L3 of the validation hierarchy.
- `store_memories` result is asserted to have `storedCount == len(batch)` AND `embedderCount == 14`. (line 132)
- `get_memory_fingerprint` asserts all 14 embedders are present per fingerprint. (lines 139-142)
- `export_training_corpus` is called and `errors == 0` is checked. (line 162-163)
- `count_training_records` is checked before AND after; mismatch raises Exp2Error. Goes beyond return-value trust.
- `clearExisting=True` on export — destructive but in a temp session, so OK per spec §7.
- `np.savez_compressed` saves `manifest_sha256` alongside arrays — supports cross-machine FSV (anchor in chain).

## Tiny issues
- `DENSE_MAP` (line 30) is an identity mapping — purely documentational. Could simplify to a set, but the redundancy is harmless.
- Hardcoded batch size `64` (line 116) is not configurable from yaml. Operator may want to tune. Minor.
- `vector_from_record` raises on zero-norm. C10 says "Embedder degeneration (zero-norm vectors): rejected." Compliant.
- `e6_sparse_top_k` and `e13_splade_top_k` are post-processed via `sparse_top_k` (sort by `|v|`, take top 128, pad with 0s). This matches `04_data_corpus.md` §3 ("top-128 weight + L2 norm + PCA-32"). The sparse_top_k doesn't apply L2 norm at this stage — the L2 normalization happens later in fit_projections. **OK.**
- `token_mean` mean-pools 128-dim per-token vectors. Matches spec §3 row E12.
- The token embeddings use `EXPECTED_DIMS["e12_token_pooled"]` (128) — correct.

## Structural compliance
File matches spec `05_technical_spec.md` §7 (extraction strategy) and §9 (extraction script). ALL 14 embedders are extracted. ALL pass through fail-loud assertions before being saved.

## Cross-check
This module won't run without a live MCP daemon AND store_memories tool returning `embedderCount == 14`. Agent-02 cannot test it without the daemon and weights. Agent-03 (sherlock) is the right place to live-verify.
