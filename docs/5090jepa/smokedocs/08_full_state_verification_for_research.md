# 08 - Full State Verification For Research

Research FSV extends Phase 10 FSV with paper-grade metadata and reproducibility.

## FSV Contract

For every stage:

1. Define trigger command.
2. Define source-of-truth CFs/files.
3. Record before state.
4. Execute trigger.
5. Independently read source of truth.
6. Compare expected and actual state.
7. Record decoded evidence.
8. Run edge cases.
9. Persist a report.

## Stage Sources Of Truth

| Stage | Source Of Truth |
| --- | --- |
| Environment | `environment.json`, command log |
| Data source | `data_manifest.json`, local file hash |
| Domain registration | `dj_domain_packs`, registry CFs |
| Ingest | raw, adapter, normalized, transition CFs |
| Panel | `dj_instrument_readings`, `dj_latent_panels` |
| Trajectory | `dj_trajectories` |
| Dataset | `dj_dataset_shards` |
| Training | `dj_training_runs`, `dj_model_artifacts`, files |
| Prediction | `dj_predictions` |
| Planning | `dj_actions`, `dj_guard_decisions`, `dj_plan_traces` |
| Surprise | `dj_surprise_events` |
| Verification | `dj_verification_runs`, evidence bundle |

## Mandatory Edge Cases

Environment:

- CUDA missing or wrong cap;
- missing CLI binary;
- DB path equals protected production path.

Data:

- missing license;
- source hash mismatch;
- empty source file.

Domain:

- duplicate instrument;
- unsupported field type;
- missing objective target.

Ingest:

- empty input;
- invalid action category;
- vector wrong dimension;
- out-of-bounds numeric field.

Artifact:

- corrupted `model.safetensors`;
- missing metrics file;
- registry hash mismatch.

Planning/surprise:

- missing artifact;
- observed outcome/panel mismatch;
- impossible guard threshold.

## Research-Specific Checks

- Every table cell used in the paper must map to a bundle JSON path.
- Every model metric must map to an artifact hash and training run ID.
- Every data claim must map to `data_manifest.json`.
- Every failure claim must map to `edge_case_results.json`.
- Every "single GPU" claim must map to `hardware.json` and runtime metrics.

## Fail-Closed Rules

The smoke harness must stop if:

- a required source has no license/provenance;
- an artifact hash mismatch occurs;
- decoded readback cannot find a written row;
- edge-case count deltas differ;
- training uses CPU when config says CUDA;
- split leakage is non-zero;
- generated paper tables cannot be traced to bundle paths.

