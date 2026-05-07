---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:31:00Z
status: active
severity: low
tags: exp2, tests, extraction-contract, success
---

# Success: test_extraction_contract.py exercises sparse top-K and token mean-pool paths

## What's good
- `training_record_fixture` provides plausible synthetic record matching the extract_embeddings.py expectations.
- Tests:
  - `vector_from_record(rec, "e1_semantic")` shape (1024,) — dense path
  - `vector_from_record(rec, "e6_sparse_top_k")` shape (128,) — sparse path
  - `vector_from_record(rec, "e13_splade_top_k")` shape (128,) — sparse path
  - `token_mean(rec)` shape (128,) — pool path
- `np.allclose(pooled, 2.0)` — sanity-checks the mean of [1.0]*128 + [3.0]*128 = 2.0. Strong correctness check.

## What's missing
- No test for `e12_token_pooled` via `vector_from_record` (only via `token_mean`).
- No test for the failure paths: missing key, wrong dim, zero-norm vector, non-finite values.
- No test for the L2-normalize semantics (extract_embeddings doesn't L2-normalize; `fit_projections.apply_pca_models` does).

## Compliance
Contract is correct; coverage is partial. Acceptable for T-006 (the extraction script exists with shape/finite checks); insufficient for full-methodology validation.
