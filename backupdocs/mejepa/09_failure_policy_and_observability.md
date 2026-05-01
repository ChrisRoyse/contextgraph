# Failure Policy And Observability

This system must fail loudly, specifically, and early.

## No Fallback Policy

Forbidden patterns:

- silently using a smaller embedder panel because a model failed to load,
- replacing missing real records with mock data,
- returning partial predictions when a required source record is missing,
- training on rows with unknown consent state,
- skipping audit writes and returning output anyway,
- swallowing dimension mismatches,
- passing tests that only assert a function returned success.

Allowed behavior:

- return a typed error,
- write a failure audit record when the operation is expected to log rejected attempts,
- stop the pipeline,
- provide exact remediation data in logs.

## Typed Error Categories

Minimum categories:

- `LegalGateMissing`
- `ConsentStateMissing`
- `ConsentDenied`
- `JurisdictionMissing`
- `SourceRecordMissing`
- `SourceHashMismatch`
- `PiiLeakDetected`
- `EmbedderUnavailable`
- `EmbedderVersionUnregistered`
- `VectorDimensionMismatch`
- `InvalidVectorValue`
- `PairwiseFeatureMissing`
- `TrajectoryLeakageDetected`
- `SplitManifestMissing`
- `TrainingDatasetInvalid`
- `ModelArtifactMissing`
- `ModelArtifactHashMismatch`
- `GuardRejected`
- `AuditWriteFailed`
- `StorageReadbackFailed`

## Required Log Fields

Every state-changing operation logs:

- request ID,
- operator ID,
- command/tool name,
- source-of-truth CF/table names,
- target keys,
- before counts,
- after counts,
- consent envelope,
- jurisdiction envelope,
- source hashes,
- artifact hashes if relevant,
- error category if failed,
- remediation hint.

Logs must include enough information to answer:

```text
What triggered the operation?
What exact state changed?
Where is the proof?
What failed?
What should be inspected next?
```

## Audit Write Semantics

If an operation is state-changing:

1. prepare validation,
2. write primary state,
3. write audit state in the same batch/transaction when possible,
4. read back primary state,
5. read back audit state,
6. only then report success.

If audit write fails after primary write, the operation is failed and must be marked for repair. Silent success is forbidden.

## Metrics

Minimum metrics:

- records by consent state,
- records by jurisdiction,
- PII detector rejection count,
- panel materialization failures by embedder,
- vector validation failures,
- pairwise materialization failures,
- trajectory leakage failures,
- training rows exported/excluded,
- guard accept/reject rates,
- audit write/readback failures,
- model artifact hash verification failures.

