# Dynamic Predictor And Objectives

The DynamicJEPA predictor learns latent dynamics across typed states and actions.

## Predictor Signature

```text
P_psi(
  context_panels,
  action_panels?,
  goal_panel?,
  time_delta,
  domain_id,
  constraints
) -> predicted_target_panel_distribution
```

The output is a distribution or uncertainty-aware prediction, not just one point vector.

## Objective

```text
L_total =
  alpha * L_panel_prediction
  + beta * L_pairwise_prediction
  + gamma * L_action_effect
  + delta * L_constellation
  + epsilon * L_anti_pole
  + zeta * L_uncertainty_calibration
  + eta * L_invariant_violation
```

Where:

- `L_panel_prediction`: target latent panel distance.
- `L_pairwise_prediction`: predicted cross-instrument relation distance.
- `L_action_effect`: predicted transition under action.
- `L_constellation`: attraction to approved target manifold when task-specific.
- `L_anti_pole`: repulsion from forbidden regions.
- `L_uncertainty_calibration`: surprise estimates match observed errors.
- `L_invariant_violation`: domain invariants are not broken.

## Prediction Modes

- next-state prediction,
- masked-event prediction,
- action-conditioned rollout,
- counterfactual branch,
- anomaly/surprise scoring,
- goal-conditioned planning,
- missing-modality prediction,
- domain-transfer prediction.

## Dynamic Adaptation

Dynamic adaptation can happen through:

- new memories,
- new trajectories,
- new domain pack,
- new instrument version,
- new read-out head,
- new skill,
- scheduled predictor retraining.

Live weight mutation is not allowed unless it writes:

- training dataset manifest,
- pre-update model artifact hash,
- post-update model artifact hash,
- evaluation report,
- rollback artifact,
- approval record.

## Collapse And Degeneracy Checks

The system must report:

- target-panel variance,
- predicted-panel variance,
- per-instrument loss,
- pairwise loss,
- context ablation loss,
- action ablation loss,
- shuffled-target loss,
- future leakage check,
- memorization check.

If the predictor performs well with shuffled targets or hidden target leakage, the run fails.

