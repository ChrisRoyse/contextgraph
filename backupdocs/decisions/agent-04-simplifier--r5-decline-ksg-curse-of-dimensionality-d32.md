---
namespace: decisions
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:50:00Z
status: declined-operator-decision
severity: high
tags: exp2, ksg, curse-of-dimensionality, b5, methodology, operator-decision
---

# R5 — DECLINED: B-5 KSG curse-of-dimensionality at d=32

## Brief instruction

> R5. (Decline) B-5 KSG curse-of-dimensionality
> This is a METHODOLOGY DECISION. Do NOT change methodology. Document for
> operator: at d=32 KSG underestimates by ~91% on Gaussian truth; either
> reduce target_dim, drop KSG cross-validation gate, or recalibrate threshold.

I declined the change as instructed.

## What the conflict is (recap)

agent-02's blocker B-5: at the production hyperparameter `pca.target_dim=32`,
KSG-1 underestimates Gaussian MI by 90%+ at moderate-to-high rho. The
synthetic_validation_report.json that ships with the artefacts directory
documents:

| rho | truth (nats) | ksg (nats) | abs_err |
|-----|--------------|------------|---------|
| 0.30 | 1.509 | 0.138 | 1.371 |
| 0.50 | 4.603 | 0.427 | 4.176 |
| 0.70 | 10.774 | 1.004 | 9.770 |

Spec `docs/exp2/03_methodology.md` §12 demands |err| < 0.15 nats for rho ≤ 0.7.
That target is unachievable at d=32 with current KSG implementation.

## Why a code fix would be wrong

The cross_validation_report gate (`run_estimators.py:122-123`) requires ≥ 80%
of pairs to agree within 0.15 nats between InfoNCE and KSG. At d=32 KSG will
systematically return near-zero on high-MI pairs, so InfoNCE (correctly high)
and KSG (incorrectly near-zero) will disagree by > 0.15 nats on most
high-redundancy pairs (e.g. E1/E10 from spec §10). The gate fires
spuriously and halts the run.

Three resolution paths exist; ALL are methodology-level decisions, not code:

a. **Reduce production dim to d=8 or d=16**: Literature (Khan et al. 2007)
   shows KSG well-behaved at d≤8. Trade-off: PCA at d=8 may not capture
   enough variance to retain the panel's distinguishing signal. The spec
   already requires `pca.variance_threshold=0.85`; reducing dim would either
   require relaxing that threshold or accepting that some embedders fall
   below it (they fail-loud per `fit_projections.py:46`).

b. **Drop the KSG cross-validation gate at d=32**: Use InfoNCE as the
   primary estimator, KSG as sanity check on low-MI pairs only. Modify
   `cross_validation_report` to apply the agreement check only to pairs
   with predicted-low MI. Methodology shift.

c. **Recalibrate the agreement threshold**: From the synthetic sweep,
   |err|=4.18 at rho=0.5; recalibrate `cross_validation.agreement_threshold_nats`
   from 0.15 to e.g. 5.0. This would document the bias rather than
   pretending it's not there.

## Operator action required (carry to final synthesis)

Operator must pick one of (a), (b), (c) and pre-register the choice in
`docs/exp2/01_constitution.md` (per Constitution C18 immutable spec rule)
before any production run is attempted.

## Status

Code untouched. Methodology untouched. Documented for operator arbitration.

## Cross-link

- Blocker: `memory/blockers/agent-02-tester--ksg-curse-of-dimensionality-at-d-32.md`
- Synthetic numbers: `artefacts/exp2/synthetic_validation_report.json`
- Spec contradiction: `docs/exp2/03_methodology.md` §2 (acknowledges)
  vs §3 (sets dim=32) vs §12 (demands |err|<0.15) vs `docs/exp2/07_validation_harness.md` §3a
