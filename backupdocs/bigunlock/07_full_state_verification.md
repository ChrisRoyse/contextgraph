# 07 Full State Verification

## FSV rule

Return values are not proof. Every successful path must:

1. Define the source of truth.
2. Execute the trigger.
3. Read the source of truth through an independent read path.
4. Verify expected data values and counts.
5. Run edge cases and prove no mutation.
6. Save evidence logs.

## Required harnesses

### Harness 1: learner baseline FSV

Purpose:

Prove E15-E21 learner-state CFs work.

Happy path:

1. start clean DB
2. register learner
3. record 5 real baseline observations
4. compile learner constellation
5. count learner state
6. read each state row
7. inspect `ldb` counts

Source-of-truth CFs:

- `learner_profile`
- `fingerprints_learner`
- `learner_state_history`
- `learner_constellations`
- `learner_audit`

Edge cases:

- invalid learner UUID
- missing required components
- out-of-range `stressFloor`
- unsupported modality
- empty session list for constellation

Expected no-mutation checks:

- profile count unchanged
- fingerprint count unchanged
- state history count unchanged
- audit count unchanged unless the rejected operation intentionally logs failure; if so, assert exact +1 failure audit

### Harness 2: state-conditioned search FSV

Purpose:

Prove persisted learner state changes Shakespeare retrieval policy.

Happy path:

1. use verified Shakespeare DB or a copy
2. create learner states: optimal, stressed, negative-valence
3. run same Shakespeare query with `stateConditionedProfile=true`
4. inspect selected profile and top results
5. read returned memory IDs and content

Source-of-truth CFs:

- `learner_state_history`
- `fingerprints`
- `content`
- `source_metadata`

Edge cases:

- missing learner ID
- missing session timestamp
- nonexistent learner state
- custom weights plus state conditioning

Expected:

- errors on invalid state
- no state mutation during search
- profile differs for at least stressed vs optimal

### Harness 3: learning event FSV

Purpose:

Prove retrieval/outcome sessions are persisted and exportable.

Happy path:

1. perform state-conditioned search
2. record before and after learning snapshots
3. record learning event
4. read event
5. count event rows

Source-of-truth CF:

- `learning_events`

Edge cases:

- malformed 14D topic profile
- malformed 91D cross correlations
- outcome label outside enum
- too many memory IDs

Expected:

- invalid event does not mutate `learning_events`
- valid event can be fetched with text and signals

### Harness 4: L/M trace FSV

Purpose:

Prove UTL computations persist state.

Happy path:

1. record learner state
2. call `compute_L` with `persist=true`
3. read delta log
4. read M trace
5. read next review
6. inspect CF counts

Source-of-truth CFs:

- `learner_delta_log`
- `learner_m_per_trace`

Edge cases:

- components missing
- NaN/infinite values
- `kSleep` out of range
- missing trace ID when persistence requires trace

Expected:

- L equals DeltaS * DeltaC * DeltaE within tolerance
- next review deterministic for stored M/decay

### Harness 5: learner training export FSV

Purpose:

Prove real learner/event rows compile into a training dataset.

Happy path:

1. create or reuse real learning events
2. create learner state rows and delta logs
3. call `export_learner_training_dataset`
4. read dataset with preview rows
5. inspect physical count

Source-of-truth CF:

- `learner_training_datasets`

Edge cases:

- no eligible rows
- revoked consent row
- maxRows=0
- invalid task enum

Expected:

- dataset row count equals eligible row count
- excluded rows are counted and explained
- matrix shape matches task

### Harness 6: closed-loop benchmark FSV

Purpose:

Prove a training or reranking change improves measurable behavior.

Happy path:

1. freeze baseline retrieval/model
2. collect real outcome events
3. export training dataset
4. train candidate
5. run held-out eval
6. compare metrics

Source of truth:

- training dataset CF
- model artifact directory
- evaluation report file
- input event IDs

Edge cases:

- missing model artifact
- dataset references missing event ID
- held-out prompt leak
- eval produces empty result

Expected:

- candidate beats baseline on predefined metric
- no increase in memorization risk
- all artifacts trace to source rows

## Evidence log format

Each FSV report should include:

```json
{
  "status": "ok",
  "repo": "...",
  "gitCommit": "...",
  "storage": "...",
  "sourceOfTruth": {
    "columnFamilies": []
  },
  "before": {},
  "actions": [],
  "readback": {},
  "edgeCases": [],
  "after": {},
  "benchmarks": {}
}
```

## Physical checks

Use both MCP and direct RocksDB checks.

MCP checks:

- `count_learner_state`
- `get_learner_state`
- `get_learning_event`
- `get_learner_training_dataset`
- `get_memory_fingerprint`
- `get_provenance_chain`

Physical checks:

```bash
ldb --db=<db> --column_family=<cf> dump --count_only
```

Optional deeper checks:

- storage-level Rust tests for exact row decode
- manifest JSON with row IDs and hashes
- `sha256` of raw signal files and exported report

## Passing standard

An FSV passes only if:

- all expected counts match
- every written row is independently read
- edge cases fail with typed errors
- invalid edge cases do not mutate source-of-truth CFs
- no daemon remains running
- report path is written
- no mock data is used for happy path

