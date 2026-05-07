---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:20:00Z
status: active
severity: high
tags: exp2, audit-chain, pair-results, schema-violation, c7
---

# DEFECT: per-pair row schema violates the audit-chain shape from `03_methodology.md` §13

## Where
`scripts/exp2/run_estimators.py:77-97` — the `row` dict written for each pair.

## Spec
`docs/exp2/03_methodology.md` §13 audit-chain row example:
```json
{
  "embedder_j": "E1_Semantic",
  "embedder_j_weight_sha256": "...",
  "embedder_k": "E10_Multimodal",
  "embedder_k_weight_sha256": "...",
  "pca_j_sha256": "...",
  "pca_k_sha256": "...",
  "corpus_manifest_sha256": "...",
  "infonce_estimate_nats": 0.847,
  ...
  "ksg_estimate_nats": 0.812,
  ...
  "agreement_nats": 0.035,
  "agreement_pass": true,
  "redundancy": 0.514,
  "code_commit_sha": "...",
  "hyperparams_sha256": "...",
  "run_seed": 42,
  "run_timestamp": "2026-04-XX T XX:XX:XX UTC"
}
```

## Observed (run_estimators.run_pair lines 77-97)
The implementation emits:
- pair, embedder_j, embedder_k
- infonce_estimate_nats, infonce_ci_low, infonce_ci_high
- infonce_train_loss_final, infonce_val_loss_final, infonce_train_val_gap, infonce_stopped_step, infonce_critic_path
- ksg_estimate_nats, ksg_ci_low, ksg_ci_high
- ksg_bootstrap_duplicate_policy
- agreement_nats, agreement_pass
- run_seed, run_timestamp

**Missing per §13**:
1. `embedder_j_weight_sha256` — no embedder weight hash
2. `embedder_k_weight_sha256` — no embedder weight hash
3. `pca_j_sha256` — no PCA model hash for embedder j
4. `pca_k_sha256` — no PCA model hash for embedder k
5. `corpus_manifest_sha256` — no corpus manifest hash
6. `redundancy` — no redundancy = MI / min(H_j, H_k) field
7. `code_commit_sha` — no git commit hash
8. `hyperparams_sha256` — no exp2.yml hash

## Why this is HIGH-severity
- 8 of the 18 spec'd scalars are missing, including ALL the provenance chain hashes.
- C7 ("audit chain first") demands every artefact carry these provenance items.
- A reviewer reading a single pair JSON cannot verify which embedder weights / which corpus / which code / which hyperparams produced this row.
- The per_pair_table.csv (`08_output_deliverables.md` §3) consumer does NOT include redundancy column either, so this propagates downstream.

## Impact
- T-024 (audit chain finalize) will not be able to build the chain from the pair rows because the rows don't carry the parent hashes.
- Replay test (`fsv.compare_replay`) misses `redundancy` field (already flagged separately).
- N_eff cannot compute redundancy from rows because rows don't have the field; today `effective_rank.n_eff_redundancy_adjusted` recomputes it from MI matrix and entropy at line 31 (which works, but means the per-pair row's contribution is silent).

## Recommended fix
Have `run_pair` build:
- `corpus_manifest_sha256`: from extracted_log or read separately.
- `embedder_*_weight_sha256`: from `verification_log.json["weight_hashes"]`.
- `pca_*_sha256`: from `pca_validation_report.json` (already includes per-key sha256).
- `code_commit_sha`: same as audit_chain._git_commit.
- `hyperparams_sha256`: sha256_file("scripts/exp2/exp2.yml").
- `redundancy`: compute as `min(1.0, MI / min(H_j, H_k))` after entropies are known; or write later when entropies are available.

## Cross-check
Static; no runtime needed. Severity HIGH because it blocks T-024 audit-chain finalization.
