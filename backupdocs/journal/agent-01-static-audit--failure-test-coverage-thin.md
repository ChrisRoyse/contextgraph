---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:29:00Z
status: active
severity: medium
tags: exp2, tests, coverage, pytest, t-013, t-011
---

# Defect: pytest suite has only 5 tests across 4 files; misses critical T-011/T-013 coverage

## Where
`scripts/exp2/tests/`

## Inventory
- `test_estimators.py` (3 tests):
  - `test_bootstrap_ci_is_deterministic_on_known_mean` — bootstrap determinism on Gaussian mean
  - `test_ksg_recovers_low_dimensional_gaussian_mi` — KSG at d=1, rho=0.5
  - `test_ksg_independence_near_zero` — KSG at d=1, rho=0
- `test_effective_rank.py` (2 tests):
  - `test_redundancy_adjusted_count_known_three_embedder_cases` — independent + fully redundant
  - `test_spectrum_and_schur_are_finite_on_positive_matrix`
- `test_extraction_contract.py` (1 test):
  - `test_training_record_vector_contract` — shapes from synthetic record
- `test_synthetic_validation.py` (1 test):
  - `test_covariance_gaussian_mi_matches_closed_form` — closed-form covariance MI on small d/n

Total: 7 tests (matches what `tasks/exp2/todo.md` claims).

## Spec
`docs/exp2/06_atomic_tasks.md`:
- T-011 InfoNCE DoD: "Unit tests: pass synthetic ground-truth recovery (sweep $\rho \in \{0.0, 0.3, 0.5, 0.9\}$, each within 0.1 nats of analytical MI)."
- T-012 KSG DoD: "$k$ sensitivity: sweep $k \in \{3, 5, 10\}$ on synthetic data; instability documented."
- T-013 DoD: "Sweep $\rho \in \{0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99\}$" + InfoNCE within 0.1; KSG within 0.15.
- T-014 DoD: "Unit tests on known distributions (Gaussian mean, sample variance) recover expected CIs."

## Gaps
1. **No InfoNCE test at all.** `test_estimators.py` has zero InfoNCE coverage. T-011's DoD calls for sweep across rho ∈ {0, 0.3, 0.5, 0.9}, each within 0.1 nats. None of this exists.
2. **No k-sensitivity test for KSG.** Need k in {3, 5, 10} comparison.
3. **No T-013 full sweep.** The methodology requires a 7-rho sweep on InfoNCE+KSG comparing to closed-form. Implementation only has rho=0 and rho=0.5 tested with KSG, dim=1.
4. **No bootstrap variance test.** T-014 says "Gaussian mean, sample variance" — only mean is tested.
5. **No InfoNCE-overfit test.** The overfit-raises-Exp2Error path is untested.
6. **No projected.npz schema test.** No test verifies that `fit_projections.apply_pca_models` writes the correct keys.
7. **No FSV replay test.** `fsv.compare_replay` is untested.
8. **No corpus-build test.** Building a stratum, sampling, manifesting — all untested.
9. **No mcp_client test.** Reasonable since it requires the daemon, but a stub-driven contract test would help.
10. **No effective_rank bootstrap test.** Only point estimates are tested; bootstrap path is uncovered.

## Why this matters
- T-013 is the synthetic-validation gate per Constitution C2 (verify on closed-form ground truth before production).
- The coverage gap means the code COULD have a bug in InfoNCE (e.g., the train/val swap, the `loss = -estimate` sign) that the tests don't catch.
- pytest passes 7 tests — that's a green light, but green doesn't mean the methodology was validated.

## Recommended fix
1. Add `tests/test_infonce.py` with rho sweep, dim=8 or 16, n=400 or larger.
2. Add KSG k-sweep test (4 KSGs at k=3,5,10 should agree within 0.15 nats on the same data).
3. Add `test_synthetic_validation.py::test_full_t013_sweep` that runs both estimators across the full rho list and writes a report fixture.
4. Add a CPU-only InfoNCE smoke test that doesn't require CUDA.

## Severity
MEDIUM. Tests exist; they pass; they're just not extensive enough to back the spec claims.

## Cross-check
Agent-02 should run pytest and report exact pass/fail count.
