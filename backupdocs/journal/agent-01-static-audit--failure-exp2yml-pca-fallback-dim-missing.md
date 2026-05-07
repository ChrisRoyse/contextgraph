---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T22:56:00Z
status: active
severity: medium
tags: exp2, hyperparameter-drift, pca, fallback, spec-mismatch
---

# Defect: exp2.yml drops the pre-registered `pca.fallback_dim: 64`

## Where
`scripts/exp2/exp2.yml:15-19` — pca block

## Spec
- `docs/exp2/03_methodology.md` §6:
  ```
  pca:
    target_dim: 32
    fallback_dim: 64
    variance_threshold: 0.85
  ```
- `docs/exp2/05_technical_spec.md` §6 also lists `fallback_dim: 64`.
- `docs/exp2/03_methodology.md` §3a explicitly requires "if any falls below 0.70, that embedder is upgraded to PCA-64".

## Observed
`exp2.yml` lines 15-19:
```
pca:
  fit_corpus: artefacts/exp2/pca_training_corpus_v1.json
  fit_corpus_count: 1000
  target_dim: 32
  variance_threshold: 0.85
```
No `fallback_dim`. So when the PCA fallback path is invoked (variance < 0.70) the fit code in `fit_projections.py` either uses a hardcoded value or fails. Need to inspect `fit_projections.py` to see what happens.

## Why it matters
Constitution C5 forbids hyperparams floating around without a written record. The fallback is an explicit methodology branch; it must be in the hashed yaml or it cannot be hashed alongside the run.

## Recommended fix
Restore `fallback_dim: 64` to `pca:` block in `exp2.yml`.

## Cross-check
Agent-02 should run a test where one synthetic embedder has < 0.70 explained variance at d=32 and verify `fit_projections.py` falls back correctly without reading from yaml.
