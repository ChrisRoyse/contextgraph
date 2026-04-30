# 12 — Prediction, Planning, And Surprise

After a model is trained and an artifact is registered, the demo proves that predictions, plans, and surprise events all persist independently readable rows. Planning is a recorded process, not a function call.

Current implementation status as of the Phase 5 double-check on 2026-04-30: prediction, planning, guard, and surprise commands are not implemented yet. The storage CFs exist, but these rows stay at zero during Phase 5 FSV.

## 1. Prediction

`predict(artifact_id, input_panel_id, candidate_action_id) -> PredictionId`:

1. Load the artifact via `load_artifact_for_inference` (Section 5 of `11_training_runs_and_artifacts.md`). Hash mismatch → no row.
2. `get_latent_panel(db, input_panel_id)`. Missing → `PredictionInputMissing`.
3. `get_action(db, candidate_action_id)`. Missing → `PredictionInputMissing`.
4. Encode input panel through online encoder. Encode action via the action panel slot in the input panel (the panel already contains the action's onehot — for a hypothetical action, the planner-created panel includes the hypothetical action onehot in the action slot).
5. Run predictor → `z_hat`.
6. Compute `uncertainty = ‖z_hat‖_2 - target_std` if available; or set to a placeholder finite scalar from `model.predictor.uncertainty_head` (demo: simple scalar = `1.0 - cos(z_hat, z_avg_train)`).
7. Build `PredictionRecord` and persist with one audit row in one batch.

`predict` returns the `PredictionId`, but the FSV harness reads the row back from `dj_predictions` and decodes it as the source of truth.

## 2. Candidate Action Generation

The demo's only skill is `enumerate_declared_actions`. Implementation in `crates/context-graph-dynamicjepa/src/planner/skill_policy.rs`:

```rust
pub fn enumerate_candidates(
    db: &Database,
    domain_pack: &DomainPack,
    skill_id: SkillId,
    plan_run_id: Uuid,
) -> Result<Vec<ActionId>, DynamicJepaError> {
    let candidates = match &domain_pack.planner_policy.candidate_actions {
        CandidateActionConfig::Enumerated { deltas } => {
            // counter_world: deltas = [-2, -1, 0, 1, 2]
            deltas.iter().map(|d| build_hypothetical_action(domain_pack, "delta", FieldValue::I64(*d))).collect()
        }
        CandidateActionConfig::EnumeratedMoves { moves } => {
            // gridworld: moves = ["up", "down", "left", "right", "noop"]
            moves.iter().map(|m| build_hypothetical_action(domain_pack, "move", FieldValue::Categorical { variant: m.clone() })).collect()
        }
    };
    let action_ids = candidates.into_iter().map(|action| {
        let action_id = ActionId(Uuid::new_v4());
        // ActionOrigin::Hypothetical, source_event_id = synthetic plan run uuid
        put_action(db, &action, action_id)?;
        Ok(action_id)
    }).collect::<Result<Vec<_>>>()?;
    Ok(action_ids)
}
```

Hypothetical actions are written to `dj_actions` with `action_origin = Hypothetical`. They share the CF with observed actions, distinguished only by the `action_origin` enum.

## 3. Guards

`crates/context-graph-dynamicjepa/src/planner/guard.rs`:

```rust
pub fn evaluate_guards(
    domain_pack: &DomainPack,
    candidate_action: &NormalizedAction,
    predicted_next_panel: &Vec<f32>,
) -> Vec<GuardDecisionRecord>;
```

Demo guards:

- **`bounds_check`**: decode the predicted next panel’s state slot back to nominal field values; reject if any value is outside the declared `[min, max]`. The decoder for the state slot is the inverse of the instrument (`Onehot` → `argmax`; `Scalar` → standard‑score inverse).

If any guard rejects an action, the action cannot be selected. A `GuardDecisionRecord` is persisted regardless of accept/reject so an operator can see the full set of guard evaluations.

```rust
pub struct GuardDecisionRecord {
    pub header: DjRecordHeader,
    pub guard_decision_id: GuardDecisionId,
    pub plan_trace_id: Uuid,
    pub guard_id: String,                  // "bounds_check"
    pub candidate_action_id: ActionId,
    pub decision: GuardDecision,           // Allow | Reject { reason_code: String, reason_message: String }
    pub evidence_refs: Vec<String>,
    pub threshold_values: BTreeMap<String, f64>,
    pub created_at_unix_ms: i64,
}
```

## 4. Plan Trace

`plan(artifact_id, input_panel_id, skill_id) -> PlanTraceId`:

1. Read domain pack, panel, artifact (with hash check). Any miss → typed error, no row.
2. Generate candidate actions (Section 2). One batch writes K action rows.
3. For each candidate, run `predict(...)` (Section 1). One batch writes K prediction rows.
4. For each candidate+prediction, run `evaluate_guards(...)` (Section 3). Persist all guard decision rows in one batch.
5. Score each candidate. Demo scoring: `score = utility(predicted_next_panel)` where `utility` is a plug-in function declared by the domain pack. Counter_world utility: `score = next_counter_value` (higher is better). Gridworld utility: `score = -manhattan_distance(predicted_xy, goal_xy)` if a `goal` is supplied, else `score = 0` and the planner picks the lowest-uncertainty candidate.
6. Select the highest-scoring candidate that has Allow on every required guard. If no candidate passes guards, status = `Rejected` with `selected_action_id = None`. **The plan trace row is still written.**
7. One batch writes the `PlanTraceRecord` + audit row.

```rust
pub struct PlanTraceRecord {
    pub header: DjRecordHeader,
    pub plan_trace_id: PlanTraceId,
    pub domain_pack_id: DomainPackId,
    pub current_panel_id: PanelId,
    pub model_artifact_id: ModelArtifactId,
    pub model_artifact_hash_at_plan: [u8; 32],
    pub skill_policy_id: SkillId,
    pub candidate_action_ids: Vec<ActionId>,
    pub prediction_ids: Vec<PredictionId>,
    pub guard_decision_ids: Vec<GuardDecisionId>,
    pub utility_scores: Vec<f32>,
    pub selected_action_id: Option<ActionId>,
    pub status: PlanTraceStatus,           // Selected | Rejected | Failed { error: String }
    pub created_at_unix_ms: i64,
}
```

Validation invariants (asserted on read-back):

- `candidate_action_ids.len() == prediction_ids.len() == utility_scores.len()`.
- `guard_decision_ids.len() == candidate_action_ids.len() * num_guards`.
- If `selected_action_id.is_some()`, it is in `candidate_action_ids` AND every required guard decision for that action is `Allow`.
- Every `prediction_id` reads back from `dj_predictions`.
- `model_artifact_id` reads back; the registry hash equals `model_artifact_hash_at_plan`.

## 5. Surprise Events

A surprise is the comparison of a prediction against the *observed* outcome. The demo's flow:

1. Operator runs `plan` for `panel_t0` → selected action `move=down`.
2. Operator (or the harness) **executes** the action by ingesting the next observed event (the gridworld corpus already contains the next event; the harness simulates execution by picking up event `t1`).
3. Materialize panel `panel_t1` (already done during ingestion).
4. `record_surprise(prediction_id, observed_outcome_id, observed_panel_id)`:

```rust
pub fn record_surprise(...) -> Result<Option<SurpriseEventId>, DynamicJepaError> {
    let pred = get_prediction(...)?.ok_or(SoT_missing)?;
    let observed_panel = get_latent_panel(...)?.ok_or(SoT_missing)?;
    // Re-encode observed panel via the artifact's online encoder (registry hash check still applies).
    let z_observed = encode_panel(...);
    let cosine = cosine(&pred.predicted_next_panel_vec, &z_observed);
    let domain = get_domain_pack(...)?;
    if cosine < domain.planner_policy.surprise_threshold {
        let id = SurpriseEventId(Uuid::new_v4());
        let record = SurpriseEventRecord { ... };
        put_surprise_event(db, &record)?;
        Ok(Some(id))
    } else {
        Ok(None)
    }
}
```

`SurpriseEventRecord` includes the prediction id, observed outcome id, observed panel id, predicted vector summary, error metrics, threshold value used, and classification (`UnexpectedOutcome` for the demo).

Surprise events are inputs to future training runs — when surprises accumulate above a threshold, the operator triggers `compile-dataset` again and a new training run incorporates them. That loop is *out of scope* for the demo's first verification but the schema is complete so it can be wired in later without migrations.

## 6. APIs

```rust
// service
pub fn predict(...) -> Result<PredictionId, DynamicJepaError>;
pub fn plan(...)    -> Result<PlanTraceId, DynamicJepaError>;
pub fn record_surprise(...) -> Result<Option<SurpriseEventId>, DynamicJepaError>;

// storage
pub fn get_prediction(db, id)   -> Result<Option<PredictionRecord>>;
pub fn get_plan_trace(db, id)   -> Result<Option<PlanTraceRecord>>;
pub fn get_guard_decision(db, id) -> Result<Option<GuardDecisionRecord>>;
pub fn get_surprise_event(db, id) -> Result<Option<SurpriseEventRecord>>;
pub fn list_predictions_for_artifact(...);
pub fn list_plan_traces_for_panel(...);
pub fn list_surprise_events_for_prediction(...);
```

CLI:

```
dynamicjepa predict --db ... --artifact-id UUID --panel-id UUID --action-id UUID --json
dynamicjepa plan    --db ... --artifact-id UUID --panel-id UUID --skill-id UUID --json
dynamicjepa record-surprise --db ... --prediction-id UUID --observed-outcome-id UUID --observed-panel-id UUID --json

dynamicjepa get-prediction --db ... --id UUID --json
dynamicjepa get-plan-trace --db ... --id UUID --include-predictions --include-guards --json
dynamicjepa get-surprise   --db ... --id UUID --json
```

## 7. FSV Expectations

Counter_world planning:

- `panel_t0` → 5 candidates (deltas -2..2) → 5 predictions → 5 guard decisions → 1 plan trace.
- Best score: `delta = 2` (utility = predicted next_counter, highest = 2). `selected_action_id` references the hypothetical action with `delta=2`.
- Surprise event for the selected action vs `panel_t1`: cosine should be high (the model is correctly trained), so `record_surprise` returns `None` (no surprise row written) for the happy path. Edge case: shuffle the prediction vec to force a surprise — verify that the surprise row is written and links prediction → outcome → panel.

Gridworld planning:

- `panel_t0_for_traj_X` → 5 candidates (moves) → 5 predictions → 5 guard decisions → 1 plan trace.
- Selected move depends on the (no-goal) demo utility = `-mean(uncertainty)`; should pick the most-confident move.
- Surprise: same shape as counter_world.

## 8. Failure Modes (And What They Should Look Like)

- Missing artifact: `PredictionInputMissing { target_id: artifact_id }`. No prediction or plan trace row.
- Stale artifact (registry says hash X, disk says hash Y): `ArtifactHashMismatch`. No prediction row. **No plan trace row** — plan does not start without a verified artifact.
- Missing input panel: `PredictionInputMissing { target_id: panel_id }`. No row.
- Domain pack missing for action validation: `DomainPackNotFound`. No hypothetical actions written.
- All candidates rejected by guards: plan trace row is written with `status = Rejected`, `selected_action_id = None`. Guard decision rows exist for every candidate.
- Encoder/predictor numerical NaN at inference (extremely rare for tiny models, but possible): `TrainingFailed { remediation: "model produced NaN at inference; retrain with seed change" }`. No prediction row.
