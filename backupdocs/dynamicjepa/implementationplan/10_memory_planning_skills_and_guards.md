# Memory, Planning, Skills, And Guards

DynamicJEPA becomes useful when predictions drive action selection and when prediction errors update memory. Planning must be recorded as physical evidence, not just returned as a selected action.

## Planning Loop

```text
current panel
  -> candidate actions
  -> model predictions
  -> guard checks
  -> utility/risk scoring
  -> selected action
  -> plan trace record
  -> observed outcome
  -> surprise event if prediction miss crosses threshold
```

Every selected action must be traceable to candidate actions, model artifact id, predictions, guard decisions, and scoring policy.

## Candidate Actions

Candidate actions may come from:

- domain pack action generator;
- skill policy;
- existing memory retrieval;
- graph neighbor traversal;
- operator-provided action file;
- persisted hypothetical action record.

Rules:

- generated candidates must be persisted before prediction;
- candidates must validate against domain pack action schema;
- invalid candidates fail the planning run;
- candidate count limits are explicit config, not hidden truncation.

## Skill Policy Records

`SkillPolicyRecord` fields:

- skill id;
- domain pack id;
- skill name;
- precondition schema;
- candidate action generator config;
- guard policy refs;
- utility policy;
- source artifact refs;
- version;
- status.

Skills are records, not code-only behavior. A planning run must reference the skill policy id it used.

## Guard Decisions

`GuardDecisionRecord` fields:

- guard decision id;
- plan trace id;
- guard id;
- candidate action id;
- decision: allow, reject, require_review;
- reason code;
- evidence refs;
- threshold values;
- created timestamp.

Rules:

- rejected action cannot be selected;
- missing guard policy is fatal when the domain pack requires guards;
- guard decisions are persisted even when every candidate is rejected;
- each guard decision must be independently readable.

## Plan Trace

`PlanTraceRecord` fields:

- plan trace id;
- domain pack id;
- current panel id;
- model artifact id;
- skill policy id;
- candidate action ids;
- prediction ids;
- guard decision ids;
- utility scores;
- selected action id;
- status;
- error details if failed.

Rules:

- selected action id must be one of candidate action ids;
- selected action must have an allow guard decision for all required guards;
- every prediction id must read back from storage;
- utility scores must be finite;
- no plan trace may reference an unregistered model artifact.

## Surprise Events

`SurpriseEventRecord` links predictions to observed reality:

- surprise id;
- prediction id;
- observed outcome id;
- observed next panel id;
- predicted next latent summary;
- error metrics;
- threshold policy;
- classification;
- created timestamp.

Uses:

- trigger dataset recompile;
- trigger model retraining;
- update memory priority;
- mark domain-pack objective drift;
- produce human-readable diagnostic evidence.

Rules:

- surprise event requires a persisted prediction and observed panel;
- threshold policy must be declared in domain pack or skill policy;
- no surprise event is written for a missing prediction; that is a separate validation error.

## Memory Integration

DynamicJEPA should read existing Context Graph records as context:

- memory content and source metadata;
- 14-embedder fingerprints;
- constellations;
- graph links and typed edges;
- learning events and learner-state records.

DynamicJEPA should write back:

- surprise events;
- plan traces;
- DynamicJEPA bindings to memory ids;
- optional memory records summarizing high-value plan outcomes only through existing memory APIs.

No direct mutation of existing memory fingerprints is required for v1.

## FSV Expectations

Happy path:

- current panel exists;
- candidate action record exists;
- model artifact row exists and file hash matches;
- prediction row is written;
- guard decision row is written;
- plan trace row is written;
- read plan trace and verify it references actual prediction and guard rows.

Edge cases:

- no valid candidate actions: plan trace status `failed`, no selected action;
- required guard rejects action: guard row exists, plan trace status `rejected`;
- prediction error exceeds threshold after observed outcome: surprise row exists and links prediction/outcome/panel.
