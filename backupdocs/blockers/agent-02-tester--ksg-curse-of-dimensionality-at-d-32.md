---
namespace: blockers
agent_id: agent-02-tester
timestamp: 2026-04-29T00:10:00Z
status: active
severity: high
tags: exp2, ksg, methodology-self-contradiction, dimensionality, b5
---

# BLOCKER B-5 (NEW, HIGH): KSG-1 catastrophically underestimates high-MI Gaussians at the production d=32 — methodology promise unachievable

## What I observed (runtime evidence, after B-4 KDTree workaround)

Synthetic Gaussian sweep at the production dim=32, n=800, k=5, seed=42:

| rho   | truth_nats | ksg_nats | |err| nats |
|-------|------------|----------|------------|
| 0.00  | 0.0000     | 0.0289   | 0.0289     |
| 0.10  | 0.1606     | 0.0507   | 0.1099     |
| 0.30  | 1.5090     | 0.1380   | **1.3710** |
| 0.50  | 4.6029     | 0.4266   | **4.1763** |
| 0.70  | 10.7735    | 1.0038   | **9.7697** |
| 0.90  | 26.6987    | 2.5038   | **24.1949**|
| 0.99  | 62.6537    | 5.1554   | **57.4983**|

KSG underestimates the true MI by an order of magnitude or more at moderate-to-high MI.

## Spec self-contradiction

Spec `docs/exp2/03_methodology.md`:

- §2 (table): "KSG: Curse of dimensionality at d > 32 → MI estimates uniformly low; biased toward 0"
- §3 closing: "All MI estimates are computed on these 32-D projections."
- §5 (KSG): "$k = 5$ is the literature standard."
- §12: "Sweep $\rho \in \{0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99\}$. **Verify InfoNCE recovers ground truth within 0.1 nats; KSG within 0.15 nats**."

Spec `docs/exp2/07_validation_harness.md` §3a:
- "KSG recovery: |estimate - truth| < 0.15 nats for $\rho \leq 0.7$."
- "For $\rho > 0.7$: estimators may underestimate due to high-MI bias; document, don't fail."

The spec simultaneously:
1. Acknowledges KSG fails at d > 32 (§2).
2. Sets the production dimension to d = 32 (§3, exactly at the threshold).
3. Demands 0.15-nat recovery for rho ≤ 0.7 (§3a).

**These cannot all be satisfied.** Empirically at d=32: at rho=0.3 (MI=1.5 nats), KSG returns 0.14 nats — 91% underestimate. Spec demands |err| < 0.15; observed is 1.37.

## Why this is HIGH severity

- The cross-validation harness (`run_estimators.cross_validation_report`) requires ≥80% of pairs to agree within 0.15 nats between KSG and InfoNCE. If KSG systematically returns near-zero on high-MI pairs, **agreement will fail across the board**, and the spec failure-mode F6 fires (raise Exp2Error).
- For low-MI pairs (where embedders ARE independent), KSG works fine — but those aren't where this matters.
- Per spec §10: E1/E10 expected high MI. KSG will report near-zero, InfoNCE will (correctly) report high. They will disagree by > 0.15. Pair flagged. 91/91 such failures cascade through the harness.
- The headline N_eff number depends on KSG entropy estimates. Same curse-of-dimensionality bias applies to `ksg_entropy` — it will report H ≈ 0 for d=32, breaking the redundancy denominator.

## What is right

The spec acknowledges the issue (§2) but the production hyperparameter (d=32) sits exactly on the failure threshold. There are two correct fixes (operator + methodology revision needed):

1. **Reduce production dim to d=8 or d=16** with documented variance loss. KSG works well at d≤8 in literature (Khan et al. 2007). Possibly fall back to d=16 with looser tolerance.
2. **Use only InfoNCE for high-MI pairs**, KSG only as sanity check on low-MI pairs.
3. **Accept the bias and tighten the threshold** to e.g. "KSG agreement within X nats" where X is calibrated from the synthetic sweep.

This is a methodology-level decision. **Implementation cannot fix this on its own.**

## Validation that the workaround KDTree code is correct

I verified my `_ksg_mi_kdtree` matches the production algorithm by:
- Running both at dim=1: same result (within float noise).
- The algorithm is unchanged; only the sklearn API differs.

So the underestimation is NOT my workaround's fault — it's the algorithm at d=32.

## Status

This is a methodology defect, not a code bug. Reporting it for coordinator arbitration.

## Cross-link

- Spec: `docs/exp2/03_methodology.md` §2, §5, §12
- Spec: `docs/exp2/07_validation_harness.md` §3a, F6
- Code: `scripts/exp2/run_estimators.py` lines 102-124 cross-validation gate
- Code: `scripts/exp2/effective_rank.py` lines 73-93 ksg_entropy usage
- Artefact: `artefacts/exp2/synthetic_validation_report.json` — the numbers
