---
namespace: discoveries
agent_id: agent-02-tester
timestamp: 2026-04-29T00:25:00Z
status: active
tags: exp2, failure-modes, f1-f7, runtime-evidence, validation-harness
---

# Failure-mode F1-F7 coverage: runtime evidence (M8)

Spec: `docs/exp2/07_validation_harness.md` §4. Each failure mode either has an automatic detector in code OR is silent. I tested each via runtime probe where possible.

| ID | Mode | Detection in code? | Runtime evidence |
|----|------|---------------------|------------------|
| F1 | Pair MI implausibly low | NO automatic detector | Spec says "expected high MI for E1/E10". Runtime would surface only if cross_validation gate fires AND a human reads the pair file. With B-5 KSG-d32 underestimate, F1 will fire across all high-MI pairs — but the harness reports it as F6 (cross-val mismatch), not F1 (implausibly low). **Silent on the F1 axis.** |
| F2 | Pair MI implausibly high (overfit) | YES — `infonce.py:120-121` raises when train-val gap > overfit threshold | Verified by reading code. Existing test_estimators.py does NOT exercise this; my synthetic sweep doesn't either (would need a high-MI overfit setup with tiny n + huge critic). |
| F3 | InfoNCE doesn't converge | NO automatic detector | Code has only the F2 gap-based check. Flat-loss / oscillation NOT detected. **Silent.** |
| F4 | KSG NaN/Inf | YES — `ksg.py:_validate_matrix` raises on `np.isfinite().all() == False` | Verified at runtime: `test_ksg_raises_on_non_finite_input_dim_1` passes. |
| F5 | Bootstrap CI wider than 0.30 nats | NO automatic detector | Code computes CI but never checks width. **Silent.** |
| F6 | Cross-val agreement < 80% | YES — `run_estimators.cross_validation_report:122-123` raises | Verified by reading code. Will fire spuriously due to B-5 (KSG bias at d=32 will push agreement below 80% for most high-MI pairs). |
| F7 | N_eff methods diverge by > 2x | NO automatic detector | `effective_rank.main` writes all three but never compares. **Silent.** |

## Coverage summary
- **Detected (3 of 7)**: F2 (overfit gap), F4 (KSG NaN), F6 (cross-val rate).
- **Silent (4 of 7)**: F1, F3, F5, F7.

This is consistent with agent-01's static finding (failure-mode-coverage-vs-spec-f1-f7.md), though agent-01 said 3/7 silent; my count is 4/7. The discrepancy is on F3 — agent-01 may have considered the F2 gap check as also covering F3, but technically F3 is a different symptom (flat loss, not big gap), and the code path is identical (only checks gap). Either way, ≥4 silent failure modes is concerning.

## Constitution C2 (fail loud) status
- 3 modes fail loud: ✓ partial compliance
- 4 modes silent: ✗ violation

## Recommended action
- F5 wide-CI check is trivial: add a `if (high - low) > 0.30: log.warning(...)` after each bootstrap.
- F1 and F3 detectors require domain heuristics (e.g., "expected MI > X for E1/E10"); deferred to coordinator.
- F7 N_eff cross-method check: trivial — assert no method differs from primary by > 2x; otherwise warn.

## Did not test
- F2 overfit at runtime: would require constructed inputs that overfit a 64-hidden critic. Not necessary; static review confirms the gate exists.

## Related
- `memory/discoveries/agent-01-static-audit--failure-mode-coverage-vs-spec-f1-f7.md` — agent-01's coverage map (3/7 silent).
- `memory/blockers/agent-02-tester--ksg-curse-of-dimensionality-at-d-32.md` — B-5: F6 will fire spuriously due to KSG bias.
