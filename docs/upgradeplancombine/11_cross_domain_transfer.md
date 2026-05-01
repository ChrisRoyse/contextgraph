# 11 — Cross-Domain Transfer Pilot Experiment

This document specifies a single deferred-axis experiment that tests
whether DDA-enriched supervision (pairwise features carried into the
predictor's input set) produces emergent cross-domain transfer
analogous to the Case-1 cross-lingual observation in PAPER.md §6.1.4.

This experiment is **deferred to v2.1**, after the v2.0 storage and
runtime upgrade lands. v2.0 ships *all the infrastructure to run it*;
v2.1 actually runs it.

## 1. Hypothesis

> A predictor trained on counter\_world events with DDA-enriched
> input (per-instrument readings + pairwise interaction features)
> produces a non-trivially better-than-random next-state prediction
> when given gridworld\_5x5 events as input, *without* further
> training. The pairwise features carry domain-agnostic structural
> patterns that single-instrument predictors miss.

This is a generalisation pilot: positive evidence would support the
claim that pairwise features encode domain-portable structure;
negative evidence would constrain the v1 paper's training-data class
argument to within-domain use only.

## 2. Setup

**Source domain:** counter\_world.v2 ($N_d = 2$). The predictor
trained here has 480 trainable parameters (existing config).

**Target domain (transfer):** gridworld\_5x5.v2 ($N_d = 3$).

**Predictor architecture:** the existing counter\_world MLP, modified
to accept the DDA-enriched input layout (per-instrument readings +
$\binom{N_d}{2}$ pairwise features). For counter\_world,
$\binom{2}{2} = 1$ pairwise feature. For gridworld $N_d = 3$, but the
predictor must consume only the 2 instruments shared between domains
(or apply a fixed projection from gridworld's 3-instrument panel down
to counter\_world's 2-instrument input space).

The cleanest formulation: ship a *bridge* domain pack
`counter_to_grid.v1` that declares the *intersection* of
counter\_world's and gridworld's instruments. Both domain packs use
the bridge instruments as a subset of their full panel; the
predictor trains on the intersection only.

## 3. Bridge domain pack design

`counter_to_grid.v1.toml`:

```toml
[domain]
id = "counter_to_grid"
version = "1.0.0"
title = "Counter-World ↔ Gridworld Cross-Domain Bridge"
schema_version = 2

[time]
field = { name = "step_index", kind = "i64", required = true, min = 0, max = 1000000 }

[[state.fields]]
name = "abstract_position"        # one-dim integer position
kind = { type = "i64" }
required = true
min = 0
max = 24                          # accommodates gridworld_5x5 max linear index

[[state.fields]]
name = "abstract_action_kind"     # categorical action label, mapped from each domain
kind = { type = "categorical", variants = ["increment", "decrement", "noop", "lateral", "perpendicular"] }
required = true

[[constellation.global]]
modalities = "all"
require_loo_min = 0.85
calibration_percentile = 10
calibration_set_size = 50
```

The two source domains' action labels map deterministically to this
bridge:

- counter\_world: `step_up → increment`, `step_down → decrement`,
  `noop → noop`.
- gridworld\_5x5: `up → lateral`, `down → lateral`, `left → perpendicular`,
  `right → perpendicular`, `noop → noop`.

The mapping is one-way from each domain to the bridge; counter→bridge
loses no information; grid→bridge collapses 4 directions to 2
(lateral / perpendicular), so cross-domain transfer is meaningfully
testable: the bridge predictor learns a coarser action vocabulary.

## 4. Procedure

```text
PHASE 1: Source-domain training (counter_world)
  - Generate 1,000 counter_world events.
  - For each event, compute the bridge instruments (mapping from
    counter_world panel to bridge panel).
  - Materialize bridge panels + pairwise features.
  - Train counter_to_grid predictor (P_psi^{c2g}) on bridge transitions.
  - Persist trained model artifact.

PHASE 2: Target-domain evaluation (gridworld_5x5, no training)
  - Generate 200 gridworld_5x5 events (held-out).
  - For each event, compute the bridge instruments (mapping from
    gridworld panel to bridge panel; lossy collapse of 4 directions).
  - Materialize bridge panels + pairwise features.
  - Run P_psi^{c2g} (trained on counter_world only) on bridge inputs.
  - Compare predicted next-bridge-panel against observed
    next-bridge-panel.

PHASE 3: Baselines (also no training)
  - random-init: re-initialise P_psi^{c2g}; predict; report cosine.
  - shuffled-target: shuffle the observed next-bridge-panels; predict
    against shuffled; report cosine.
  - within-domain-only: run a P_psi^{grid} trained on gridworld
    counter_world-bridge-input data; serves as the "ceiling" of
    transfer.

PHASE 4: Reporting
  - Mean / std / BCa CI of test cosine, compared across:
    * counter→grid transfer (predictor trained on counter, evaluated on grid)
    * random-init baseline
    * shuffled-target baseline
    * within-domain ceiling (predictor trained on grid bridge-input)
  - Per-pairwise-feature ablation: re-train counter_to_grid predictor
    *without* pairwise features, repeat transfer evaluation. Compare
    transfer gap.
```

## 5. Acceptance criteria

The experiment supports the hypothesis when:

1. counter→grid transfer cosine `> shuffled_target_cosine`'s BCa upper
   bound by at least 0.05.
2. counter→grid transfer cosine `> random_init_cosine`'s BCa upper
   bound by at least 0.10.
3. The pairwise-features-removed ablation reduces transfer cosine by
   at least 0.05 (effect size attributable to pairwise features).

If criteria 1 and 2 hold but 3 fails, the supported claim narrows to
"the bridge domain admits transfer; pairwise features are not the
cause." If criterion 1 fails outright, the v1 paper's training-data
class argument is *unaffected* (the within-domain MC ratio claim does
not depend on cross-domain transfer); the experiment simply yields a
negative result that's reported honestly.

## 6. Bundle output

The experiment generates a new bundle kind:
`tmp/mejepa_research_runs/<run-id>_cross_domain_transfer/` with:

- `counter_world/` — Phase 1 training bundle (1{,}000 events).
- `gridworld_5x5/` — Phase 2 evaluation bundle (200 events,
  no training).
- `bridge/` — bridge predictor artifacts.
- `transfer_results.json` — summary of cosines.
- `paper_tables/table_cross_domain_transfer.csv`.
- `plots/cross_domain_transfer_box.svg`.
- Standard FSV evidence (FSV-1 through FSV-12 on each sub-bundle).

## 7. Statistical reporting

Following PAPER.md §6.3.5 and the v1 paper's protocol:

- $n = 5$ seeds for each phase (counter\_world training seeds 42-46;
  gridworld evaluation also 5 seeds for the held-out generation).
- BCa bootstrap 10{,}000 iterations, percentile method,
  fixed seed 20260601 for the bootstrap.
- Sign-flip permutation test for the
  pairwise-features-on vs.\ pairwise-features-off contrast.
- Honest disclosure: $n = 5$ seed floor at $p = 0.0625$ — same as
  v1's protocol.

## 8. Resource budget

Estimated wall time on the documented host:

| Phase | Wall time |
| --- | ---: |
| Phase 1 (counter\_world training, 5 seeds) | ~20 minutes |
| Phase 2 (gridworld eval, no training, 5 seeds × 200 events) | ~3 minutes |
| Phase 3 (baselines, ablations) | ~15 minutes |
| Phase 4 (aggregation, plots, FSV) | ~5 minutes |
| **Total** | ~45 minutes |

This fits comfortably in the release-D ~3 h budget when added; or it
can run as a standalone v2.1 experiment.

## 9. Edge cases (typed-error catalog)

The cross-domain experiment adds:

| Edge | Trigger | Typed error |
| --- | --- | --- |
| Bridge action mapping incomplete | counter or grid action label not in the mapping | `BRIDGE_ACTION_MAPPING_INCOMPLETE` |
| Bridge instrument count drift | bridge declared $\ne$ actual | `BRIDGE_INSTRUMENT_COUNT_DRIFT` |
| Bridge predictor architectures mismatch | source-trained input dim $\ne$ target eval input dim | `BRIDGE_PREDICTOR_INPUT_DIM_MISMATCH` |
| Transfer cosine below shuffled-target | criterion 1 fails | report as negative result, NOT a typed error |
| Pairwise-features ablation criterion fails | criterion 3 fails | report as negative result, NOT a typed error |

Three new typed errors at infrastructure level; the criterion failures
are *measured outcomes*, not errors.

## 10. Why this experiment is the right next test

- It exercises the same multi-embedder decomposition machinery the
  v1 paper §7 names as a contribution.
- It tests an *out-of-distribution* property of the runtime, which the
  v1 paper §10 disclaimed but PAPER.md §6.1.4 hinted at empirically
  for text style.
- It uses two existing domain packs (counter\_world, gridworld) so no
  new dataset preparation is needed beyond the bridge pack.
- It produces both *a positive and a negative* possible outcome, both
  of which are paper-worthy.
- It runs on a single GPU in well under an hour.

## 11. What this experiment does *not* claim

- Does not claim general cross-domain transfer for arbitrary domain
  pairs.
- Does not claim that ME-JEPA-DDA is a general-purpose foundation
  model.
- Does not claim that the bridge mapping is the optimal projection.
- Does not claim the pairwise features alone are the *only* mechanism
  driving transfer; other contributors (predictor architecture
  inductive bias, hyperparameters) are not isolated by this single
  ablation.

The deferred-to-v2.1 status is part of the discipline: v2.0 ships
the infrastructure; v2.1 runs the experiment with results disclosed
honestly.
