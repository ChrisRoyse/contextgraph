---
namespace: journal
agent_id: agent-02-tester
timestamp: 2026-04-28T23:50:00Z
status: active
tags: exp2, pytest, baseline, runtime
---

# SUCCESS: pytest baseline — 7 passed, 0 failed

## Command
```bash
cd /home/cabdru/contextgraph
python3 -m pytest -v scripts/exp2/tests
```

## Result
```
============================= test session starts ==============================
platform linux -- Python 3.13.9, pytest-9.0.2, pluggy-1.5.0
collecting ... collected 7 items

scripts/exp2/tests/test_effective_rank.py::test_redundancy_adjusted_count_known_three_embedder_cases PASSED [ 14%]
scripts/exp2/tests/test_effective_rank.py::test_spectrum_and_schur_are_finite_on_positive_matrix PASSED [ 28%]
scripts/exp2/tests/test_estimators.py::test_bootstrap_ci_is_deterministic_on_known_mean PASSED [ 42%]
scripts/exp2/tests/test_estimators.py::test_ksg_recovers_low_dimensional_gaussian_mi PASSED [ 57%]
scripts/exp2/tests/test_estimators.py::test_ksg_independence_near_zero PASSED [ 71%]
scripts/exp2/tests/test_extraction_contract.py::test_training_record_vector_contract PASSED [ 85%]
scripts/exp2/tests/test_synthetic_validation.py::test_covariance_gaussian_mi_matches_closed_form PASSED [100%]

============================== 7 passed in 2.96s ===============================
EXIT_CODE=0
```

## Log file
`/tmp/agent-02-pytest-baseline.log`

## Notes
- Exact match to agent-01's claim of "7 passed" baseline.
- Zero skipped, zero errors.
- Total wall time 2.96s — confirms CPU-only baseline runs fast.
- Confirms the unit-test claims in `tasks/exp2/todo.md` are accurate.
- torch.cuda.is_available() = True, torch 2.11.0+cu130 — but no CUDA-only test exists in baseline (InfoNCE constructor would refuse if requested and CUDA missing; that path is not exercised in current tests).
