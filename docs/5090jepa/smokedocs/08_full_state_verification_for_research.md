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
| Data source | `data_manifest.json`, local file hash, O*NET ZIP hash |
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

Surprise verification encodes observed panels with the persisted target encoder,
matching the JEPA training objective. The no-action ablation also zeroes the
action vector during training, prediction, and surprise observation encoding.

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
- Every generated career row must map to the O*NET ZIP hash and generated walk
  CSV in `generated_fixtures/onet_30_2/verification/`.
- Every failure claim must map to `edge_case_results.json`.
- Every "single GPU" claim must map to `hardware.json` and runtime metrics.
- Every ablation claim must map to `ablation_results.json`, a model artifact
  config, and a RocksDB count readback.

## Fail-Closed Rules

The smoke harness must stop if:

- a required source has no license/provenance;
- an artifact hash mismatch occurs;
- decoded readback cannot find a written row;
- edge-case count deltas differ;
- training uses CPU when config says CUDA;
- split leakage is non-zero;
- generated paper tables cannot be traced to bundle paths.

## Current Manual Edge Evidence

The current O*NET smoke run records 11 edge-case rows:

- counter_world: empty input, invalid action, max panel;
- gridworld_5x5: empty input, invalid action, out-of-bounds state;
- career_taxonomy: empty input, missing source license, invalid action, wrong
  skill-vector dimension, out-of-bounds job zone.

For career taxonomy, the expected invalid-row outcome is raw/adapter/audit
evidence only, with no normalized state or transition persisted. The empty file
case records only an audit failure. The wrapper also fails closed for missing
O*NET source, custom event counts without O*NET, duplicate run IDs, and bad
O*NET paths.
