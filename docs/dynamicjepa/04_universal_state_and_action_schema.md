# Universal State And Action Schema

DynamicJEPA needs one schema that can represent many domains without erasing domain-specific structure.

## Core Types

```text
World
Domain
Actor
Entity
Observation
Action
Outcome
Goal
Constraint
Trajectory
Artifact
VerifierDecision
```

## Universal Event

```text
DynamicEvent {
  event_id,
  domain_id,
  world_id,
  timestamp,
  time_interval?,
  actor_ids[],
  entity_ids[],
  event_kind,
  modality_bundle_id,
  payload_hash,
  source_metadata_id,
  consent_or_use_policy_id?,
  safety_policy_id?,
  adapter_version,
  validation_status,
  audit_id
}
```

## Universal Action

```text
DynamicAction {
  action_id,
  domain_id,
  actor_id,
  tool_or_policy_id?,
  action_kind,
  parameters_hash,
  precondition_ids[],
  expected_outcome_ids[],
  allowed_effects[],
  forbidden_effects[],
  source_event_id?,
  audit_id
}
```

## Universal State

```text
DynamicState {
  state_id,
  domain_id,
  world_id,
  entity_ids[],
  event_window,
  latent_panel_id,
  graph_snapshot_id?,
  constraint_set_id,
  goal_state_id?,
  uncertainty_summary,
  created_at,
  audit_id
}
```

## Trajectory

```text
DynamicTrajectory {
  trajectory_id,
  domain_id,
  world_id,
  state_ids[],
  action_ids[],
  outcome_ids[],
  maskable_positions[],
  split_assignment?,
  leakage_checks,
  created_at,
  audit_id
}
```

## Domain Extension Rule

Domain packs may add typed payload schemas, but they cannot bypass the universal wrapper.

Example:

- robotics adds joints, gripper state, image frames,
- finance adds instrument, order book, trade, price series,
- code adds commit, file path, symbol, test result,
- health adds observation code, unit, patient consent envelope,
- recruiting adds candidate, role, company, placement.

The wrapper is what makes universal storage, training, and FSV possible.

## Validation Invariants

Hard errors:

- event without domain ID,
- action without actor or policy,
- state without source event window,
- trajectory with missing state IDs,
- future event leakage in a training split,
- payload hash mismatch,
- adapter version not registered,
- unapproved use policy.

