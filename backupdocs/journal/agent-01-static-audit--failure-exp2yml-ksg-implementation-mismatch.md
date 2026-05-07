---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T22:55:00Z
status: active
severity: medium
tags: exp2, hyperparameter-drift, ksg, implementation, spec-mismatch
---

# Defect: exp2.yml ksg.implementation deviates from pre-registered methodology

## Where
`scripts/exp2/exp2.yml:42`

## Spec
- `docs/exp2/03_methodology.md` §6 pre-registered `implementation: npeet` (and §5c says "We use `npeet` (NumPy Entropy Estimation Toolbox, Steeg lab) **or implement directly with `sklearn.neighbors.NearestNeighbors`**" — fallback acknowledged, but the pre-registered yaml said `npeet`).
- `docs/exp2/05_technical_spec.md` §6 (config example) line 279 says `implementation: npeet`.

## Observed
`exp2.yml` line 42: `implementation: sklearn_direct`.

## Severity assessment
This is **medium**, not high, because:
- `03_methodology.md` §5c explicitly allows the sklearn-based implementation as a fallback if npeet "proves unstable".
- The KSG-1 estimator is a well-defined non-parametric algorithm; correctness of the implementation can be verified independently against synthetic ground truth.
- However: per Constitution C5 ("Pre-registered hyperparams") and C7 ("hyperparams_sha256 logged"), the deviation must be acknowledged. The pre-registered name was `npeet`. The drift was **not documented** in lessons.md or any decisions/ file.

## Why it matters
Constitution C5 forbids tuning hyperparams to make headline numbers look better. Even an honest implementation switch must be logged with rationale. Otherwise an external reviewer comparing the published `hyperparams_sha256` to the pre-registered yaml (per C17, OSF) sees a mismatch.

## Recommended fix
Either:
1. Update `03_methodology.md` §6 + `05_technical_spec.md` §6 to say `implementation: sklearn_direct` and document the rationale (npeet pin had numerical instability or was missing from requirements.txt).
2. Switch back to `npeet` in `requirements.txt` and `ksg.py`.

## Cross-check
`scripts/exp2/requirements.txt` may not pin npeet. To confirm, agent-02 should `cat scripts/exp2/requirements.txt` and check for npeet.
