# Decision: Phase 3 Owns UTML Primitives Until A Dedicated UTML Crate Exists

Date: 2026-05-10

The Phase 3 task specs depend on `../utml-learning-formula/TASK-UTML-001..010`, but no implemented UTML crate/module existed in the workspace. To unblock Phase 3 without a fallback path, `context-graph-mejepa-train` now owns the concrete UTML learning-signal API:

- `LearningSignal`
- `DeltaPComponents`
- `DeltaKComponents`
- `DeltaOmegaComponents`
- `DeltaXiComponents`
- `SkipReason`
- `compute_l_step`
- `sampling_weight`
- `pairwise_mi_audit`

The implementation validates finite `[0,1]` values and feeds the same persisted `TrainingCertificate` schema used by the RocksDB certificate chain.

Future extraction into a dedicated UTML crate is allowed only as a mechanical move that preserves public behavior and FSV evidence.
