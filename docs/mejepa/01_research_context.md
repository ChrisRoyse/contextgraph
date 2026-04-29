# Research Context

This document captures the external research used to ground the ME-JEPA plan. The goal is not to restate every paper, but to identify which parts are load-bearing for this build.

## JEPA Lineage

JEPA systems learn by predicting representations in latent space instead of reconstructing raw pixels, frames, or tokens.

Relevant external anchors:

- I-JEPA introduced an image-based JEPA that predicts target-block representations from a context block, with masking strategy as a key design choice. See arXiv:2301.08243.
- V-JEPA applies feature prediction to video and reports strong downstream performance from learned video features without reconstruction, negatives, or text supervision. See arXiv:2404.08471.
- Meta's public V-JEPA code and V-JEPA 2 codebases show that JEPA has become an active engineering program, not only a position-paper idea.
- LeCun's "A Path Towards Autonomous Machine Intelligence" frames JEPA-like world models as a path toward predictive systems that reason in abstract representation space.

ME-JEPA keeps the core JEPA idea, but changes the target representation source:

```text
Standard JEPA:
  trainable/EMA target encoder produces target latent

ME-JEPA:
  frozen external embedder panel produces target latent
```

That difference is the architectural claim.

## Collapse Context

Representation collapse is a central problem in self-supervised latent prediction. VICReg explicitly addresses collapse with variance, invariance, and covariance regularization. BYOL is also part of the relevant collapse discussion because it uses online/target networks and stop-gradient/asymmetry.

ME-JEPA should not claim collapse is impossible in every sense. It should claim a narrower, testable property:

- The target panel cannot co-collapse with the predictor because target representations are produced by frozen external embedders.
- The predictor can still degenerate by ignoring context, leaking targets, or learning dataset shortcuts.
- Therefore ME-JEPA still needs masking audits, target leakage checks, variance diagnostics, ablation tests, and held-out trajectory prediction.

The docs and implementation must use the narrower claim: **external frozen targets remove target-encoder collapse, but they do not remove all training failure modes.**

## Compliance And Employment-AI Context

Recruiting use cases are high-stakes. The planning assumptions are:

- EU AI Act Annex III includes recruitment and candidate evaluation systems in the employment category.
- NYC Local Law 144 requires a bias audit, public audit information, and candidate/employee notices before employers or employment agencies use an AEDT.
- California CCPA/CPRA rights include know, delete, opt out of sale/sharing, correct, limit sensitive personal information, and non-discrimination.
- NIST AI RMF is useful as a risk-management framework even when not legally mandatory.

This means ME-JEPA cannot be designed as a research-only model and then patched for compliance. The audit log, guard decisions, consent filters, deletion propagation, and model/dataset cards are system primitives.

## Dataset And Model Documentation

Datasheets for Datasets and Model Cards are relevant because this system will create:

- a de-identified trajectory dataset,
- a latent panel dataset,
- a predictor artifact,
- product read-out heads,
- guard and anti-pole artifacts.

Each of those needs explicit documentation of provenance, composition, allowed uses, excluded uses, evaluation conditions, and known limitations.

## Research Contribution To Test

The publishable research hypothesis is:

```text
Frozen, independently trained, multi-embedder target panels can support JEPA-style
latent prediction over non-physical human trajectories while reducing target-encoder
collapse risk and preserving per-output verification through constellation guards.
```

That hypothesis must be tested by:

- prediction quality against held-out future trajectory states,
- collapse diagnostics,
- ablations removing pairwise features,
- ablations reducing N from 19 to smaller panels,
- guard acceptance/rejection calibration,
- source-of-truth FSV on every materialized dataset and artifact.

