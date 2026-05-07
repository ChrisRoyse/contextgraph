---
namespace: discoveries
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:33:00Z
status: active
tags: exp2, failure-modes, validation-harness, f1-f7, coverage
---

# Discovery: F1-F7 failure-mode coverage map

`docs/exp2/07_validation_harness.md` §4 names 7 failure modes. Audited each against implementation:

## F1: Pair MI Estimate Implausibly Low
- **Spec**: Diagnose via PCA variance (≥0.85), TSNE/UMAP plot, k=10 KSG sweep, longer InfoNCE training.
- **Implementation**: `fit_projections.py` raises if explained < 0.85 (already a defect — should fall back to d=64 first per methodology). No automatic 2D plot generation. No k-sweep on suspect pairs. No "longer training" path.
- **Coverage**: PARTIAL. The PCA variance check is the only automated guard. Other diagnostics are manual.

## F2: Pair MI Estimate Implausibly High
- **Spec**: Check data leakage; check overfit gap; KSG k-sensitivity.
- **Implementation**: `infonce.py:120-121` raises Exp2Error on overfit gap > 0.1. **Should flag, not raise** (already a HIGH defect). Cross-validation runs only at end if all 91 pairs complete.
- **Coverage**: WEAK. Halts production if any single pair overfits.

## F3: InfoNCE Doesn't Converge
- **Spec**: Reduce LR, increase critic depth, fall back to KSG-only.
- **Implementation**: No "non-convergence" detection. Early stopping is on val improvement (`patience=500`). If loss is flat the patience triggers and the run exits with whatever val_median was computed. No remediation path. No fallback to KSG-only.
- **Coverage**: NONE. Silently produces a low-quality estimate.

## F4: KSG Numerical Issues
- **Spec**: Verify finite, check npeet/sklearn version, k-sweep.
- **Implementation**: `ksg.py:_validate_matrix` rejects NaN/Inf. Raises on negative MI < -1e-6. Raises on duplicates without jitter_seed. **Good fail-loud coverage of F4 mechanics.** k-sweep is in pilot phase (T-015, but T-015 is not invoked by the orchestrator).
- **Coverage**: GOOD on numerics, MISSING on k-sweep.

## F5: Bootstrap CIs Wider Than Expected
- **Spec**: Threshold = 0.30 nats median CI width. Fail → diagnose corpus/sample-size.
- **Implementation**: `bootstrap_ci` returns CIs but the median width is NEVER computed. `cross_validation_report` only computes agreement, not CI width.
- **Coverage**: NONE.

## F6: Cross-Validation Agreement < 80%
- **Spec**: Threshold 80%; failures listed; methodology revisited.
- **Implementation**: `run_estimators.cross_validation_report` (line 102-124) computes the rate. If `rate < min_agreement_rate`, raises Exp2Error with a one-line diagnosis. No specific list of "potentially adjusted hyperparameters". The yaml has `cross_validation.on_failure: fail_with_diagnostics` (line 53) — but the code raises generic Exp2Error, not "with diagnostics".
- **Coverage**: ADEQUATE on detection, WEAK on diagnostic content.

## F7: N_eff Scalars Diverge Across Methods
- **Spec**: Threshold = 2× factor across N_eff^(1,2,3).
- **Implementation**: `effective_rank.py` computes all three but doesn't compare them. The summary writes them all; no automated divergence detection.
- **Coverage**: NONE.

## Summary
| Mode | Detection | Action | Coverage |
|------|-----------|--------|----------|
| F1 | partial | none | partial |
| F2 | overfit-only | raise (wrong) | weak |
| F3 | none | none | none |
| F4 | full | raise | good |
| F5 | none | none | none |
| F6 | full | raise | adequate |
| F7 | none | none | none |

**3 of 7 failure modes have effectively no detection.** Constitution C10 ("defense in depth against known failure modes") not fully satisfied.

## Recommended path
Add a `tests/test_failure_modes.py` that:
- Constructs synthetic data triggering each F1-F7 mode.
- Runs the relevant harness.
- Asserts the expected diagnostic / failure / flag.

Document each mode's coverage in the audit chain.

## Severity
This is a meta-discovery, not a single defect. It contributes to the overall evaluation:
- F2 + F3 collectively make the production InfoNCE run fragile.
- F5 + F7 mean the operator can't tell if the headline number is wide-CI'd or method-divergent at finalization.
