---
namespace: journal
agent_id: agent-02-tester
timestamp: 2026-04-29T00:18:00Z
status: active
severity: high
tags: exp2, blocker-verified, b3, per-pair-json, provenance, c7
---

# VERIFIED (runtime+static): B-3 — per-pair JSON row missing exactly 8 spec'd provenance fields

## Spec §13 (docs/exp2/03_methodology.md lines 290-310) required keys
```
agreement_nats, agreement_pass,
code_commit_sha,
corpus_manifest_sha256,
embedder_j, embedder_j_weight_sha256,
embedder_k, embedder_k_weight_sha256,
hyperparams_sha256,
infonce_ci_high, infonce_ci_low,
infonce_estimate_nats, infonce_train_loss_final, infonce_train_val_gap, infonce_val_loss_final,
ksg_ci_high, ksg_ci_low, ksg_estimate_nats,
pca_j_sha256, pca_k_sha256,
redundancy,
run_seed, run_timestamp
```

(23 spec'd fields)

## Actual fields produced by `run_estimators.run_pair` (regex-extracted from row dict literal)
```
agreement_nats, agreement_pass,
embedder_j, embedder_k,
infonce_ci_high, infonce_ci_low,
infonce_critic_path, infonce_estimate_nats, infonce_stopped_step,
infonce_train_loss_final, infonce_train_val_gap, infonce_val_loss_final,
ksg_bootstrap_duplicate_policy, ksg_ci_high, ksg_ci_low, ksg_estimate_nats,
pair,
run_seed, run_timestamp
```

(19 actual fields)

## Missing (8 fields, all provenance hashes)
```
code_commit_sha
corpus_manifest_sha256
embedder_j_weight_sha256
embedder_k_weight_sha256
hyperparams_sha256
pca_j_sha256
pca_k_sha256
redundancy
```

## Extra (4 fields, not in spec)
```
infonce_critic_path           # OK — operational, not spec violation
infonce_stopped_step          # OK — diagnostic, not spec violation
ksg_bootstrap_duplicate_policy # OK — diagnostic
pair                          # OK — convenience
```

## Conclusion
Agent-01's count of 8 missing provenance fields is **EXACTLY correct**. Verified by source extraction.

The 4 "extra" fields are non-controversial (operational/diagnostic data). The 8 missing fields all relate to:
- The audit chain (B-2 blocker) — without those hashes, the chain has no anchors.
- Independent reproducibility — peer reviewer cannot verify they are using the same corpus, weights, PCA, or hyperparameters.
- Constitution C7 — every artefact must carry provenance.

## Detection command
```python
import re, inspect
from scripts.exp2 import run_estimators
src = inspect.getsource(run_estimators.run_pair)
fields = set(re.findall(r'"([a-z_]+)":', src))
print(fields)
```

## Status
B-3 confirmed at HIGH severity. Code fix required: add `code_commit_sha`, `hyperparams_sha256`, `corpus_manifest_sha256`, `embedder_*_weight_sha256`, `pca_*_sha256`, and `redundancy` (computed from MI/H_min) to the row dict in `run_pair`.
