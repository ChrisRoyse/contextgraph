# 28 — Full JEPA Architectural Alignment per Yann LeCun (2026-05-17)

> **Authority.** This document captures the operator-directed alignment between ME-JEPA-Code's architecture and Yann LeCun's full JEPA blueprint as articulated in his 2026-05-17 transcript on AMI (Advanced Machine Intelligence). Where LeCun's blueprint and ME-JEPA-Code's current implementation diverge, this doc enumerates the gap and the sub-task that closes it.
>
> **Companion EPIC:** [GitHub issue #314](https://github.com/ChrisRoyse/contextgraph/issues/314).
> **Source theory:** [`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`](../ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md), [`docs/whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md`](../whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md).
> **Reward-signal companion:** [`29_REWARD_SIGNAL_COMPLETENESS.md`](29_REWARD_SIGNAL_COMPLETENESS.md).

---

## §1 — Why this doc exists

ME-JEPA-Code is already a JEPA-class system. It already has frozen instruments, multi-embedder substrate, conformal calibration, OOD detection, action-conditioned `mejepa_predict_what_if`, and a continuous self-optimization daemon. **But LeCun's full blueprint is broader than what we've shipped.** The 2026-05-17 transcript articulates ten architectural pillars that together define what a JEPA-class agentic system must be. This doc maps each pillar to its current ME-JEPA-Code state and to the sub-task (if any) that closes the gap.

---

## §2 — The ten JEPA pillars (LeCun 2026-05-17)

### Pillar 1 — Frozen target panel (collapse-immune by construction)

**LeCun:** *"All the generative ones — VAE / masked autoencoder — basically have been failures. JEPA works much better for representing images and video."* — *"You want to train a system to make prediction in representation space."*

**ME-JEPA state:** ✅ **SHIPPED.** 15-slot frozen instrument panel; `ΔΞ.target_collapse = 0` by construction per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`. Per-instrument deterministic functions of typed events, zero trainable parameters. This is the central architectural innovation that distinguishes ME-JEPA from learnable-target JEPA/BYOL/I-JEPA.

**No new work needed.**

---

### Pillar 2 — Anti-collapse regularizer on the predictor side

**LeCun:** *"The latest [is] called SIGreg — sketched isotropic gaussian regularization. ... If you want to read one paper, read the LE World Model paper [Bistriero et al.]."*

**ME-JEPA state:** ✅ **SHIPPED.** VICReg variance+invariance+covariance remains present, and TASK-FP-101 adds a differentiable SIGReg component to `compose_l_full` through `crates/context-graph-mejepa-train/src/loss/sigreg.rs`. The implementation uses deterministic sketched Gaussian projections, Candle tensor `matmul` + `sin`/`cos` characteristic-function matching, and a configurable `lambda_sigreg` coefficient in `LossCoefficients`. REQ-UTML-06 pairwise MI audit is shipped + exposed via `mejepa_audit_pairwise_mi` MCP tool. Predictor redundancy is gated per-head via `ΔΞ_h(t) = (1 − predictor_redundancy_h)^0.3 · (1 − constellation_violation_rate_h)^0.2`.

**FSV evidence:** `/mnt/d/contextgraph/fsv/task-fp-101-sigreg-fsv/run-1779036305137/sigreg_fsv.json`, sha256 `b2d4995d4de9539916558f51cdd1423081af94962868c18077653c0d075e80c7`, `allPassed=true`, 5 cases. The FSV proves collapsed and anisotropic batches score worse than Gaussian-like latents, SIGReg loss descends across an information-content trajectory, invalid inputs fail closed, and the training loss backpropagates a finite non-zero gradient to predicted latents.

**Sub-task:** **TASK-FP-101 — COMPLETE.** SIGReg is shipped as an additive regularizer beside VICReg rather than as a destructive replacement, preserving the old anti-collapse path while adding the LeCun/Bistriero-style sketched isotropic Gaussian objective.

---

### Pillar 3 — Hierarchical JEPA architecture

**LeCun:** *"Within a year or so we'll have a general methodology to train hierarchical models on a very wide variety of modalities. ... You don't do this at a pixel level. Our mental model of the world predicts at an abstract level of representation."*

**ME-JEPA state:** ✅ **SHIPPED.** TASK-FP-102 (commit 62501579) lands a durable per-prediction hierarchy row written beside each live `RealityPrediction`. Each row carries File-, Function-, AstNode-, and Chunk-level entries with parent-link validation, persisted in `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` via `read_hierarchical_predictions` / `write_hierarchical_prediction` (sync_all + reopen readback). `compiler.verify()` produces the hierarchy alongside the live prediction.

**Sub-task:** **TASK-FP-102 — COMPLETE.** Hierarchical predictor emits 4 abstraction levels (File / Function / AstNode / Chunk) per query, persisted to `CF_MEJEPA_HIERARCHICAL_PREDICTIONS`, FSV `/mnt/d/contextgraph/fsv/task-fp-102-hierarchical-jepa-fsv/run-1779037368822/hierarchical_jepa_fsv.json` sha256 `5dfce2ebe37d2b52a6ad036ec89e22f659c014fa3ff9c543b123000f2d7f9bf1` (`all_passed=true`; 8 levels across 2 changed files; parent links validated; live + hierarchy readback equal).

---

### Pillar 4 — Action-conditioned world model

**LeCun:** *"A world model at a very general level is something that allows an agentic system to anticipate the consequences of its own actions. ... If it's action condition then you get a world model that allows you to control it optimally for whatever purpose you have."*

**ME-JEPA state:** ✅ **SHIPPED.** `mejepa_predict_what_if(diff)` MCP tool is action-conditioned: it takes a candidate diff and returns predicted post-edit panel. This is exactly the LeCun action-condition requirement.

**No new work needed.**

---

### Pillar 5 — Planning by search in latent space (NOT token space)

**LeCun:** *"Inference is by predicting the next token right — it's not by search. ... You do this by searching for a sequence of actions that will accomplish the task you set for yourself. ... You don't do this in token space. You do this in abstract thoughts space."*

**ME-JEPA state:** ✅ **SHIPPED.** TASK-FP-103 adds `search_latent_actions` in `context-graph-mejepa` plus MCP `mcp__cgreality__mejepa_search_latent_actions`. The search evaluates best-of-K candidate patches through the existing ME-JEPA compiler, derives a 16-d abstract action latent from panel deltas, builds the per-candidate hierarchy, and ranks candidates by predicted oracle pass, calibrated confidence, in-distribution mass, hierarchy signal, and optional goal-latent alignment. It remains non-mutating and returns file-SHA before/after readback through the MCP tool.

**FSV evidence:** `/mnt/d/contextgraph/fsv/task-fp-103-latent-search-fsv/run-1779038591152/latent_search_fsv.json`, sha256 `f31de8c1d4a0035731170e4a24640dc04d16095c2525ceacde84e7b0c8167e4e`, `all_passed=true`. The FSV ranks 3 candidates, recovers the expected goal-matching candidate (`candidate-goal`), proves rank contiguity, finite 16-d action latents, selected score monotonicity, no referenced-file mutation, and fail-closed rejection for single-candidate and duplicate-candidate-id inputs.

**Sub-task:** **TASK-FP-103 — COMPLETE.** The shipped primitive searches over pre-enumerated candidate patches in representation space. It does not yet synthesize new diffs from a free-form latent action vector; TASK-FP-105 consumes this search backend for counterfactual repair ranking, and future generator-side work can materialize latent actions into edits.

---

### Pillar 6 — Objective-driven AI with hardwired safety constraints

**LeCun:** *"You give an objective to an AI system... The system has a world model and it predicts the outcome of a sequence of actions it imagines taking. If this outcome satisfies a cost function... [the system] minimizes this cost according to its model. It can do nothing else. You can add ... safety constraints that say 'don't hurt anybody on the way.' The system cannot violate those things by construction."*

**ME-JEPA state:** ✅ **SHIPPED.** TASK-FP-104 adds `MejepaObjective`, deterministic objective-cost scoring, and a hardwired safety-constraint vector in `context-graph-mejepa/src/objective_safety.rs`. `compiler.compile_with_panel()` evaluates the objective before building the verdict, and `compiler.verify()` escalates `FailedGate::SafetyConstraintViolation` before approval when the objective report blocks pass. This makes test deletion, secret exposure, auth bypass, and unsafe crypto constraints hard blockers by construction.

**FSV evidence:** `/mnt/d/contextgraph/fsv/task-fp-104-objective-safety-fsv/run-1779040194889/objective_safety_fsv.json`, sha256 `31cda8fa8c7bb0ce980947786b42d104f6ea1d2cde42a6bef88d6cc1aff288a1`, `all_passed=true`. The FSV proves a clean high-oracle patch receives `Verdict::Pass`, while a synthetic no-test-deletion violation receives `Verdict::GuardRejected` plus `FailedGate::SafetyConstraintViolation`, with live prediction and hierarchy CF readback for the safety-rejected prediction.

**Sub-task:** **TASK-FP-104 — COMPLETE.** The shipped implementation uses the existing Q4 cost-like heads as inputs to the objective cost and hard-blocks safety constraint violations before `Pass`.

---

### Pillar 7 — Counterfactual best-of-K candidate ranking

**LeCun (via operator §12 future capabilities #7):** *"ME-JEPA can score candidate fixes before execution: 'this edit is more likely to satisfy the oracle and preserve invariants than that edit.' This becomes feedback to coding agents without relying on the LLM's self-assessment."*

**ME-JEPA state:** ⚠️ **INFRASTRUCTURE EXISTS, FORMAL TOOL MISSING.** Agent can manually call `mejepa_predict_what_if(diff)` K times and compare. No `mejepa_rank_candidates(diffs[])` MCP tool that does this atomically with consistent calibration.

**Gap:** The agent shouldn't have to re-implement the ranking logic. Need an MCP tool that takes N candidate diffs, calls the predictor N times, and returns the ranked list with per-candidate predicted-Pass probability, predicted failure modes, and OOD score.

**Sub-task:** **TASK-FP-105 — `mejepa_rank_candidates` MCP tool for counterfactual repair ranking.** Acceptance: MCP tool implemented, tested with 5+ candidates per query, FSV proves ranking is monotonic with predicted oracle-pass × (1 − OOD_score).

---

### Pillar 8 — Calibrated uncertainty ("knows when it doesn't know")

**LeCun (implicit):** *"It should emit lower confidence and point to the missing evidence, instead of overclaiming."*

**ME-JEPA state:** ✅ **DESIGNED.** `Verdict::OutOfDistribution` + `Unknown` fingerprint kind + active-learning queue per [#309 fingerprint methodology EPIC] and [`docs/whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md`](../whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md). Conformal calibration per-fingerprint.

**No new work needed beyond #309 sub-tasks.**

---

### Pillar 9 — System 1 vs System 2 framing

**LeCun:** *"The way you behave in deliberate reflective behavior is that you imagine, predict the consequences of your actions, and you plan accordingly — contrary to system 1 where you just act reactively and instinctively."*

**ME-JEPA framing:**
- **System 1 = the AI coding agent (LLM)** — generates patches reactively from prompts.
- **System 2 = ME-JEPA-Code** — deliberatively predicts whether each candidate patch will satisfy reality, ranks alternatives, surfaces failure modes, and routes Unknown cases to active learning.

**Sub-task:** Documentation update — make this separation explicit in `docs/whitepapermejepasystem/04_FIVE_OPERATOR_QUESTIONS.md` and MCP tool descriptions. (Doc edit, no code.)

---

### Pillar 10 — Reward-signal completeness from durable reality

**LeCun:** *"This system can derive reward signals from durable reality. ... You need to capture all the reward signals possible."*

**ME-JEPA state:** ⚠️ **NEEDS COMPLETENESS AUDIT.** Many reward signals captured (oracle, AST diff, witness, DDA, pairwise MI, blind-spot z-scores). Some tiers partial or missing.

**Sub-task tree:** See companion doc [`29_REWARD_SIGNAL_COMPLETENESS.md`](29_REWARD_SIGNAL_COMPLETENESS.md) for the 8-tier audit + TASK-RWD-201..212 sub-tasks.

---

## §3 — Pillar status summary

| # | Pillar | Status | Sub-task |
|---|---|---|---|
| 1 | Frozen target panel | ✅ shipped | — |
| 2 | Anti-collapse regularizer | ✅ VICReg + SIGReg shipped | TASK-FP-101 complete |
| 3 | Hierarchical JEPA | ✅ **file→function→AST→chunk shipped** | TASK-FP-102 complete |
| 4 | Action-conditioned world model | ✅ shipped | — |
| 5 | Latent-space planning | ✅ **best-of-K latent action search shipped** | TASK-FP-103 complete |
| 6 | Objective + safety constraints | ✅ **hardwired safety constraints shipped** | TASK-FP-104 complete |
| 7 | Counterfactual best-of-K ranking | ⚠️ infra exists, MCP tool missing | TASK-FP-105 |
| 8 | Calibrated uncertainty | ✅ designed (#309) | — |
| 9 | System 1 vs System 2 framing | doc update needed | (whitepaper edit) |
| 10 | Reward-signal completeness | spawns full audit | doc [`29_*`](29_REWARD_SIGNAL_COMPLETENESS.md) + EPIC #315 |

---

## §4 — Acceptance gate (composite)

ME-JEPA-Code is fully JEPA-aligned per LeCun's transcript when ALL of the following are true:

1. **SIGreg** (or comparable explicit info-max regularizer) is shipped with FSV proving info-content monotonicity (TASK-FP-101). ✅ Complete: `/mnt/d/contextgraph/fsv/task-fp-101-sigreg-fsv/run-1779036305137/sigreg_fsv.json`.
2. **Hierarchical** predictor emits multi-scale predictions across ≥ 3 abstraction levels with per-level calibration (TASK-FP-102). ✅ Complete: `/mnt/d/contextgraph/fsv/task-fp-102-hierarchical-jepa-fsv/run-1779037368822/hierarchical_jepa_fsv.json`.
3. **Latent-space planning**: `mejepa_search_latent_actions` returns ranked latent vectors, NOT token-by-token regenerated patches (TASK-FP-103). ✅ Complete: `/mnt/d/contextgraph/fsv/task-fp-103-latent-search-fsv/run-1779038591152/latent_search_fsv.json`.
4. **Objective-driven**: explicit cost function + hardwired safety constraint vector; constraint violation hard-blocks `Verdict::Pass` (TASK-FP-104). ✅ Complete: `/mnt/d/contextgraph/fsv/task-fp-104-objective-safety-fsv/run-1779040194889/objective_safety_fsv.json`.
5. **Counterfactual ranking**: `mejepa_rank_candidates` MCP tool ships and is exercised by the live integration FSV (TASK-FP-105).
6. **System 2 framing**: whitepaper + MCP tool descriptions make the System 1 LLM vs System 2 ME-JEPA separation explicit.
7. **Reward-signal completeness**: per `29_*` audit, ≥ 95% of LeCun-enumerated reward signals are captured to durable storage (EPIC #315).

---

## §5 — Atomic task references

| TASK | Title | Pillar | DoD anchor |
|---|---|---|---|
| TASK-FP-101 | SIGReg regularizer for predictor | 2 | ✅ Complete: differentiable characteristic-function sketch + FSV `b2d4995d4de9539916558f51cdd1423081af94962868c18077653c0d075e80c7` |
| TASK-FP-102 | Hierarchical JEPA predictor (file → function → AST → chunk) | 3 | ✅ Complete: `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` readback + FSV `5dfce2ebe37d2b52a6ad036ec89e22f659c014fa3ff9c543b123000f2d7f9bf1` |
| TASK-FP-103 | Latent-space candidate-patch search | 5 | ✅ Complete: `mcp__cgreality__mejepa_search_latent_actions` + FSV `f31de8c1d4a0035731170e4a24640dc04d16095c2525ceacde84e7b0c8167e4e` |
| TASK-FP-104 | Objective-driven cost + safety constraints | 6 | ✅ Complete: `MejepaObjective` + `FailedGate::SafetyConstraintViolation` + FSV `31cda8fa8c7bb0ce980947786b42d104f6ea1d2cde42a6bef88d6cc1aff288a1` |
| TASK-FP-105 | `mejepa_rank_candidates` MCP tool | 7 | Monotonic ranking by predicted-Pass × (1 − OOD_score) |

Each TASK file follows `prdtospec.md` Level-4 atomic-task format and the FSV protocol per [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md).

---

## §6 — Implementation order

1. **TASK-FP-101 (SIGReg)** — complete. Pure substrate-side improvement, doesn't depend on other tasks, immediately benefits all downstream predictor heads.
2. **TASK-FP-102 (hierarchical)** — complete. It provides the multi-scale substrate that TASK-FP-103 latent search will operate over.
3. **TASK-FP-103 (latent search)** — complete. Depends on hierarchical predictor for richer search space and now provides the candidate-search backend for TASK-FP-105.
4. **TASK-FP-104 (cost + safety)** — complete. Depends on Q4 head emitting cost-like outputs (existing) + adds hard constraint layer.
5. **TASK-FP-105 (counterfactual rank)** next — depends on TASK-FP-103 and should consume TASK-FP-104 objective/safety scores in ranking.

This ordering is captured in `13_TASK_INDEX.md` task DAG.

---

## §7 — Cross-references

- LeCun transcript (2026-05-17): operator-supplied verbatim, archived in conversation log.
- [GitHub issue #314](https://github.com/ChrisRoyse/contextgraph/issues/314) — companion EPIC.
- [`docs/futurebuild/27_PHASE_G_ROADMAP.md`](27_PHASE_G_ROADMAP.md) — overall Phase G roadmap (TASK-FP-101..105 are part of Phase G).
- [`docs/futurebuild/29_REWARD_SIGNAL_COMPLETENESS.md`](29_REWARD_SIGNAL_COMPLETENESS.md) — Pillar 10 companion.
- [`docs/whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md`](../whitepapermejepasystem/14_FAILURE_SHAPE_FINGERPRINTS.md) — fingerprint-first methodology (#309) that this aligns to.
- [`docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`](10_LEARNING_FORMULA_MEJEPA_TRUTH.md) — current VICReg implementation that SIGreg upgrades.
- [`docs/futurebuild/15_PREDICTION_TAXONOMY.md`](15_PREDICTION_TAXONOMY.md) — `RealityPrediction` schema that TASK-FP-104 extends with `Constraints`.
- [`docs/futurebuild/13_TASK_INDEX.md`](13_TASK_INDEX.md) — atomic task DAG.
- LE World Model paper (Bistriero et al, NYU/MILA): cited in LeCun transcript as the canonical SIGreg reference.
- VICReg (Bardes et al 2022): current anti-collapse regularizer.
- Becker & Hinton 1989: information-maximization foundations.
- Continual backprop (Dohare et al 2024): SHIPPED in ME-JEPA per REQ-UTML-07.

---

**TL;DR.** ME-JEPA-Code has the code-bearing pillars 1, 2, 3, 4, 5, 6, and 8 shipped or designed with FSV-backed evidence where code changed. The remaining #314 code-bearing pillar gap is TASK-FP-105 counterfactual best-of-K ranking. Pillar 9 is a framing/documentation update, and pillar 10 is tracked separately by the #315 reward-signal-completeness audit.
