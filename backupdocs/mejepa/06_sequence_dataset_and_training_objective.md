# Sequence Dataset And Training Objective

ME-JEPA learns from longitudinal trajectories. The training unit is an ordered sequence of typed events with materialized latent panels.

## Event Types

Initial event taxonomy:

- candidate profile update,
- resume version,
- job description,
- recruiter note,
- recruiter-candidate email,
- recruiter-client email,
- call transcript segment,
- interview event,
- offer event,
- compensation event,
- placement event,
- retention outcome,
- promotion outcome,
- failure outcome,
- client payment/refund outcome.

Each event must have:

- event ID,
- subject IDs,
- actor roles,
- timestamp or time interval,
- source metadata,
- consent envelope,
- jurisdiction envelope,
- typed payload hash,
- panel record ID after embedding.

## Trajectory Types

| Trajectory | Sequence key | Prediction target |
| --- | --- | --- |
| Candidate trajectory | candidate pseudonymous ID | next career/search state |
| Placement episode | candidate + role + company | offer, acceptance, retention |
| Hiring manager trajectory | hiring manager ID | candidate acceptance/rejection pattern |
| Recruiter trajectory | recruiter ID | action style and success pattern |
| Company trajectory | company ID | hiring pattern, comp pattern, retention pattern |
| Role-market trajectory | role + region + industry | comp and demand movement |

## Split Rules

No leakage:

- subject-level leakage is forbidden unless a specific experiment is designed to test within-subject forecasting,
- future events cannot appear in context,
- target event content cannot leak through source metadata,
- duplicate resumes/emails must be deduplicated before split,
- train/validation/test manifests must be persisted.

Split assignment is source-of-truth state. It cannot be recomputed silently during training.

## Masking

The predictor sees context panels and must predict target panels.

Masking modes:

- future-step prediction,
- span masking inside a trajectory,
- actor-conditional masking,
- outcome-masked prediction,
- counterfactual conditioning experiments.

Every training row must store the mask spec. If the mask spec is missing, the row is invalid.

## Objective

Initial objective:

```text
L_total =
  alpha * L_panel_prediction
  + beta * L_pairwise_prediction
  + gamma * L_constellation_anchor
  + delta * L_anti_pole
  + epsilon * L_variance_diagnostic
```

Where:

- `L_panel_prediction`: per-slot cosine or task-appropriate distance to frozen target panel.
- `L_pairwise_prediction`: distance to materialized 171 pairwise interaction features.
- `L_constellation_anchor`: distance to approved success/role/company constellations for target tasks.
- `L_anti_pole`: hinge penalty for proximity to forbidden regions.
- `L_variance_diagnostic`: not a fallback; a diagnostic/regularizer to catch predictor degeneracy.

## Training Stages

1. **Panel dataset export**: build and verify training rows.
2. **Tiny controlled run**: train on a small real persisted subset to prove plumbing.
3. **Baseline predictor**: train without pairwise and constellation losses.
4. **Full predictor**: add pairwise and constellation/anti-pole losses.
5. **Ablations**: N=14, N=19 without pairwise, N=19 full.
6. **Read-out heads**: train product heads only after predictor artifact passes evaluation.
7. **Guard calibration**: calibrate G_tau thresholds on validation rows.

## Artifact Requirements

Every model artifact stores:

- source dataset manifest hash,
- split manifest hash,
- code commit,
- config hash,
- embedder version set,
- model weight hash,
- training log hash,
- evaluation report hash,
- consent manifest hash,
- allowed-use policy.

Missing artifact metadata is a hard error.

