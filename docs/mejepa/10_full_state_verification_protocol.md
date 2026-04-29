# Full State Verification Protocol

Return values are not proof. Every ME-JEPA stage must define the source of truth, execute the trigger, read the source of truth independently, run edge cases, and save evidence.

## Evidence Report Schema

Every manual FSV run writes JSON:

```json
{
  "status": "ok",
  "repo": "/home/cabdru/contextgraph",
  "gitCommit": "...",
  "storagePath": "...",
  "sourceOfTruth": {
    "columnFamilies": []
  },
  "before": {},
  "actions": [],
  "readback": {},
  "edgeCases": [],
  "after": {},
  "artifactHashes": {},
  "logs": []
}
```

## FSV-0: Legal Gate FSV

Purpose: prove real corpus processing cannot start without approved gates.

Happy path:

1. write legal gate row with document hashes,
2. read `mejepa_legal_gates`,
3. verify gate status, document hashes, approved uses, and expiry.

Edge cases:

- missing title document hash,
- expired privacy memo,
- approved flag false.

Before/after proof:

- print `mejepa_legal_gates` count before and after,
- print decoded gate row,
- prove invalid gates do not enable source ingest.

## FSV-1: Privacy Triage FSV

Purpose: prove records are classified before embedding.

Controlled scenario records are allowed here only as real persisted records through the actual adapter path. They are not mocks.

Happy path:

1. ingest a real or controlled record with known email/phone/name fields,
2. run privacy triage,
3. read `mejepa_source_events`, `mejepa_consent_state`, `mejepa_jurisdiction_tags`, and vault state,
4. prove raw identifiers are absent from ME-JEPA event payloads.

Edge cases:

- empty payload,
- invalid email/phone format plus valid name,
- strict-jurisdiction subject without explicit consent.

Expected:

- empty payload errors or is excluded with exact audit reason,
- invalid format does not crash detector and still logs classification,
- strict jurisdiction without consent is `quarantined` or denied, not embedded.

## FSV-2: Panel Materialization FSV

Purpose: prove E1-E14 + Erec1-Erec5 panels physically exist.

Happy path:

1. select allowed event,
2. run panel materialization,
3. read `mejepa_panel_records`,
4. read `CF_EMBEDDING_REGISTRY`,
5. inspect physical counts.

Edge cases:

- missing Erec model,
- wrong vector dimension,
- NaN in vector.

Expected:

- each edge case fails with typed error,
- no panel row is written,
- failure audit row exists if failure audits are enabled.

## FSV-3: Pairwise Feature FSV

Purpose: prove pairwise features are materialized and complete.

Happy path:

1. use a verified panel row,
2. compute pairwise features,
3. read `mejepa_pairwise_records`,
4. verify 171 pair slots for N=19.

Edge cases:

- panel with one missing slot,
- duplicate pair key,
- unsupported feature version.

Expected:

- no partial pairwise row,
- typed error identifies missing slot or duplicate key.

## FSV-4: Trajectory Builder FSV

Purpose: prove events become ordered trajectories without leakage.

Happy path:

1. create or select a subject with at least five real persisted events,
2. build trajectory,
3. read `mejepa_trajectories`,
4. verify event order, timestamps, actor roles, and panel IDs.

Edge cases:

- out-of-order timestamp,
- target event included in context mask,
- trajectory containing a deleted event.

Expected:

- leakage is detected,
- deleted event blocks trajectory export,
- before/after counts prove no bad trajectory was written.

## FSV-5: Training Dataset Export FSV

Purpose: prove predictor training rows are exported from physical rows.

Happy path:

1. select verified trajectories,
2. assign split manifest,
3. export training dataset,
4. read `mejepa_training_datasets` and `mejepa_split_manifests`,
5. verify row counts and source event IDs.

Edge cases:

- no eligible rows,
- revoked consent row,
- train/test subject leakage.

Expected:

- no eligible rows errors unless explicitly running a dry count command,
- revoked rows are excluded with exact count,
- leakage fails the export.

## FSV-6: Tiny Predictor Training FSV

Purpose: prove a model artifact can be trained and traced to real rows.

Happy path:

1. use verified training dataset,
2. run tiny training job,
3. write model artifact,
4. read `mejepa_training_runs`, `mejepa_model_artifacts`, and artifact files,
5. verify hashes.

Edge cases:

- missing dataset row,
- model artifact file missing after registry write,
- training log hash mismatch.

Expected:

- registry and files agree,
- mismatches fail loudly,
- no artifact is approved without eval.

## FSV-7: Read-Out Head And Guard FSV

Purpose: prove product output is guarded and audited.

Happy path:

1. run a read-out head on verified input,
2. run G_tau,
3. write guard decision,
4. read `mejepa_guard_decisions` and audit log,
5. return output only after readback.

Edge cases:

- missing guard constellation,
- anti-pole threshold violation,
- audit write failure simulation.

Expected:

- missing constellation errors,
- threshold violation returns `GuardRejected`,
- audit failure blocks output.

## FSV-8: Deletion Propagation FSV

Purpose: prove deletion/opt-out affects future use.

Happy path:

1. submit deletion request for a subject,
2. update vault and consent state,
3. read deletion row, consent row, affected trajectories/datasets,
4. prove future training export excludes affected rows.

Edge cases:

- nonexistent subject,
- duplicate deletion request,
- deleted subject still present in training dataset candidate set.

Expected:

- duplicate is idempotent only if it logs exact existing request state,
- nonexistent subject errors or records unverifiable request by policy,
- deleted subject blocks training export.

## Passing Standard

An FSV passes only if:

- all expected physical counts match,
- every written row is independently read,
- decoded values match expected values,
- edge cases fail with typed errors,
- invalid edge cases do not mutate source-of-truth state except intentional failure audit rows,
- evidence JSON is written,
- storage-level proof is included,
- no mocks or fake storage are used.

