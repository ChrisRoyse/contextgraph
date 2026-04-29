# Full State Verification And Evaluation

DynamicJEPA must prove physical outcomes at every layer.

## FSV Rule

For every operation:

1. Define the source of truth.
2. Print before counts/state.
3. Execute the trigger.
4. Read the source of truth independently.
5. Decode and verify expected values.
6. Run at least three edge cases.
7. Print after counts/state.
8. Write an evidence report.

## Core Harnesses

### FSV-1: Domain Pack Registration

Happy path:

- register a domain pack,
- read `dynamic_domains`,
- verify hash, event kinds, instruments, FSV harness IDs.

Edge cases:

- duplicate domain ID,
- missing required instrument declaration,
- invalid event kind.

### FSV-2: Instrument Registration

Happy path:

- register a real instrument artifact,
- read `dynamic_instruments`,
- hash artifact,
- run one real sample.

Edge cases:

- missing artifact file,
- wrong declared dimension,
- unregistered preprocessing hash.

### FSV-3: Event To Panel

Happy path:

- ingest event,
- materialize panel,
- read `dynamic_events` and `dynamic_panels`.

Edge cases:

- empty payload,
- unsupported modality,
- required instrument unavailable.

### FSV-4: Binding And Trajectory

Happy path:

- bind events across modalities/entities,
- build trajectory,
- read `dynamic_bindings` and `dynamic_trajectories`.

Edge cases:

- duplicate binding,
- time-order violation,
- missing state ID.

### FSV-5: Predictor Dataset

Happy path:

- export trajectory dataset,
- read `dynamic_training_datasets` and `dynamic_splits`,
- verify no leakage.

Edge cases:

- zero eligible rows,
- train/test leakage,
- revoked policy row.

### FSV-6: Predictor Training

Happy path:

- train tiny predictor,
- read run row,
- hash model artifact,
- read eval report.

Edge cases:

- missing source dataset,
- artifact hash mismatch,
- eval report missing.

### FSV-7: Planning And Skill Execution

Happy path:

- plan from current state,
- execute a low-risk skill in a real or approved simulator source of truth,
- read before/action/outcome/after.

Edge cases:

- violated precondition,
- forbidden outcome predicted,
- missing outcome readback.

### FSV-8: Guard And Surprise

Happy path:

- run prediction,
- guard it,
- compare observed outcome,
- write guard decision and surprise event if needed.

Edge cases:

- missing constellation,
- anti-pole violation,
- audit write failure.

## Evaluation Axes

DynamicJEPA needs multiple evaluation axes:

- prediction error by instrument,
- action-effect prediction,
- planning success,
- uncertainty calibration,
- surprise detection,
- cross-domain transfer,
- no-leakage verification,
- no-regression after dynamic updates,
- guard precision/recall,
- storage/readback integrity.

## Passing Standard

A benchmark does not pass unless:

- the dataset rows physically exist,
- artifact hashes match,
- every output has guard/audit evidence,
- edge cases fail correctly,
- no mock storage was used,
- source records can be traced from output back to raw event hashes.

