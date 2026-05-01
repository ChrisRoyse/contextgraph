# Memory Planning And Skills

DynamicJEPA needs memory because a universal dynamic system cannot rely only on fixed weights.

## Memory Classes

| Memory | Stores |
| --- | --- |
| Episodic | observed events, actions, outcomes |
| Semantic | generalized facts, entities, relations, constellations |
| Procedural | verified skills and tool workflows |
| Artifact | datasets, model artifacts, configs, reports |
| Failure | typed errors, failed plans, guard rejections |

## Planner

The planner proposes actions by simulating latent futures:

```text
current_state
  -> candidate_actions
  -> predicted_future_states
  -> verifier scoring
  -> selected action or no-action
```

Planner strategies:

- model-predictive control,
- tree search,
- beam search over skill sequences,
- counterfactual search,
- retrieve-and-adapt prior workflow,
- human approval route for high-risk actions.

## Skill Record

```text
DynamicSkill {
  skill_id,
  domain_id,
  name,
  preconditions[],
  action_sequence[],
  expected_outcomes[],
  forbidden_outcomes[],
  verification_harness_id,
  success_evidence_refs[],
  failure_evidence_refs[],
  version,
  status
}
```

## Skill Lifecycle

1. Proposed from successful trajectory or human-authored workflow.
2. Tested on real or approved simulator source of truth.
3. FSV proves state before/after.
4. Skill is registered as draft.
5. Repeated evidence promotes it to active.
6. Failures demote or retire it.

No skill can execute in active mode without FSV evidence.

## Surprise Loop

When observed state differs from predicted state:

1. write surprise event,
2. store predicted and observed panel refs,
3. compute disagreement by instrument,
4. search prior failures,
5. route to retraining, new instrument, new invariant, or domain-adapter fix.

Surprise is not noise to hide. It is the main signal for making the system more universal.

