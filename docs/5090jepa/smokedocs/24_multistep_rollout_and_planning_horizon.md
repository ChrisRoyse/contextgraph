# 24 - Multi-Step Rollout And Planning Horizon

## Goal

Extend the predictor from one-step ($H=1$) to multi-step rollouts ($H>1$) with
honest error-compounding analysis. Smokedoc 02 §What Not To Spend GPU On Yet
explicitly defers this until "one-step action-conditioned prediction is
stable", which is now true. This is **appendix / extension scope** for the
first paper, **not** a headline claim.

## Why Multi-Step Matters Eventually

Single-step predictors capture local dynamics; planning over career-decision
horizons (years, multiple skill gains, multiple role moves) requires multi-step
rollouts. Recent literature (2024-2026) on world-model planning is converged
on two facts:

1. Autoregressive rollouts of single-step models compound error. The error
   often grows superlinearly with horizon.
2. Two mitigations exist:
   - **Direct multi-step prediction** at training time (predict $z_{t+H}$ from
     $z_t, a_{t:t+H-1}$), trading model complexity for less compounding;
   - **Hierarchical world models** (high-level coarse predictor + low-level
     fine predictor), where the high-level model rolls out coarsely and the
     low-level fills in.

For our first multi-step pass we adopt the simpler **direct multi-step**
training, with autoregressive rollout as a baseline for the compounding
analysis.

## Scope For This Doc

- $H \in \{1, 2, 5, 10\}$.
- Two predictors trained: autoregressive $H=1$ (existing) and direct multi-step
  $H \in \{2, 5, 10\}$ (new).
- Evaluation on val/test of each $H$, with paired error reporting.
- Single-seed for the appendix; multi-seed only if doc-15 budget permits.

Out of scope for this doc:

- Hierarchical world models (future work; one paper away).
- Uncertainty-aware rollouts with epistemic uncertainty (future work; cited).
- Online planning with re-planning (future work; doc 25 territory).

## Architecture Changes

### Direct Multi-Step Predictor

Same JEPA encoder. Predictor takes $(z_t, a_t, a_{t+1}, \ldots, a_{t+H-1})$
and outputs $\hat{z}_{t+H}$. Action sequence concatenated to the latent input
of the predictor.

Parameter cost: predictor input dimension grows from $d_{latent} + d_{action}$
to $d_{latent} + H \cdot d_{action}$. For $H=10$, action dim 5, that is +45
input dims. Negligible parameter count change.

Training data: rebuild trajectory-derived datasets with $H$ different shard
splits:

```text
dj_dataset_shards rows tagged with rollout_h: 1, 2, 5, 10
```

The same trajectories produce all $H$ shards. Splits (train/val/test) carry
across $H$.

### Autoregressive Rollout Baseline

Reuse the trained $H=1$ predictor. At eval time, roll out $H$ steps:
$\hat{z}_{t+1}, \hat{z}_{t+2}, \ldots, \hat{z}_{t+H}$ where each step feeds
the previous prediction as input. No retraining.

This is the comparison baseline. The headline plot is direct vs autoregressive
val MSE / cosine as a function of $H$.

## CLI Additions

```bash
target/debug/context-graph dynamicjepa train \
  --db "$DB" \
  --dataset-id "$DATASET" \
  --config configs/dynamicjepa/career_taxonomy_paper_small_h5.json \
  --rollout-h 5 \
  --artifact-root "$ARTIFACT_ROOT" \
  --json
```

`--rollout-h` is the new flag. The dataset shard for the matching $H$ must
exist; if not, typed `SCHEMA_MISMATCH` error.

A separate CLI rolls out the $H=1$ predictor autoregressively at eval:

```bash
target/debug/context-graph dynamicjepa rollout-eval \
  --db "$DB" \
  --jepa-artifact-id "$H1_ARTIFACT" \
  --rollout-h 5 \
  --split val \
  --json
```

## Storage Schema

`dj_dataset_shards` gains a `rollout_h: u8` field. Default 1 (matches existing
data).

`dj_predictions` rows gain a `rollout_h: u8` and a `rollout_kind: enum {
direct, autoregressive }`.

`dj_model_artifacts` rows for multi-step predictors set `artifact_kind =
predictor_multistep` and record `rollout_h`.

No new CFs.

## Metrics

For each $H$ and each prediction kind:

- val/test latent MSE;
- val/test latent cosine;
- decoder-head top-k occupation (doc 16) at horizon $H$;
- skill-vector reconstruction cosine at horizon $H$.

Plus the **error compounding metric**:

$$
C(H) = \frac{\text{MSE}(\text{autoregressive}, H)}{\text{MSE}(\text{direct}, H)}
$$

$C(H) > 1$ means direct multi-step beats autoregressive. The plot of $C(H)$
vs $H$ is the appendix headline figure.

## New Tables And Figures

### `table_horizon.csv`

| domain | rollout_h | rollout_kind | val_mse | val_cosine | test_mse | test_cosine | top1_occ_acc | skill_cosine |

### Figure F7 (appendix)

Two panels:

- (left) val cosine vs $H$ for direct and autoregressive on the same axes;
- (right) compounding ratio $C(H)$ vs $H$.

Generated from `table_horizon.csv`.

## Planning Horizon For The Existing `plan` CLI

Smokedoc 04 §Gate 7 (Prediction, Planning, Surprise) currently plans over
candidate actions at $H=1$ only. After this work:

- `dynamicjepa plan --horizon 5` is supported;
- the plan trace records $H$ and the rollout kind used;
- guard decisions are produced per step ($H$ of them) instead of per plan;
- utility scoring sums or discounts across the horizon (configurable, default
  sum with $\gamma=0.95$ discount).

This makes the existing plan-trace evidence richer without changing its
schema: the plan trace already carries multiple guard decisions, just from
candidate actions; now they also span horizon steps.

## Acceptance Gate (Smokedoc 04 §Gate 13)

> **Gate 13 — Multi-Step Rollout (Optional Appendix)**
>
> Pass criteria (only if this doc's work is included in the paper):
> - direct $H \in \{2, 5, 10\}$ predictors trained;
> - autoregressive rollout from $H=1$ predictor evaluated at the same horizons;
> - `table_horizon.csv` exists with all rows;
> - F7 figure exists;
> - the paper appendix discusses the compounding ratio honestly.

Not blocking for paper submission.

## Discussion Points For The Paper

When the paper reports multi-step results, it must say:

- Direct multi-step trades parameter count for less compounding error.
- Autoregressive rollouts of small models compound error sharply on this
  benchmark; the compounding ratio at $H=10$ is reported, not hidden.
- Hierarchical models (Hafner et al., recent 2025-2026 work) are an
  unimplemented alternative.
- Uncertainty-aware rollouts (RWM-U style) are an unimplemented alternative.

## Edge Cases (Fail-Closed)

- `--rollout-h` larger than max trajectory length in the dataset: typed
  `SCHEMA_MISMATCH` with the offending value.
- Mixing dataset shards of different $H$ in one training run: typed
  `SCHEMA_MISMATCH`.
- Autoregressive rollout requested but the source predictor's `rollout_h !=
  1`: typed `SCHEMA_MISMATCH`.
- Decoder head trained on $H=1$ latents reused for $H \neq 1$ without
  retraining: typed `ARTIFACT_KIND_MISMATCH`.

## Acceptance

This work is done when:

- the CLI flags exist and route through `train` and `rollout-eval`;
- direct multi-step predictors exist as dedicated artifacts;
- `table_horizon.csv` and F7 figure are generated;
- the paper appendix has a one-paragraph discussion of compounding;
- the work is **explicitly opt-in** in the runbook (doc 09) — not in the
  default headline path.
