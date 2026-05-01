# DynamicJEPA Model Training And Artifacts

The model layer learns action-conditioned latent dynamics from persisted panels and trajectories. Training must be reproducible from source-of-truth dataset shard records and artifact registration must prove the physical model files exist and match recorded hashes.

## Crate Ownership

The new `context-graph-dynamicjepa` crate owns model execution:

- encoders;
- predictor networks;
- loss functions;
- training loop;
- evaluation;
- artifact save/load;
- inference service.

`context-graph-core` owns only model-related record types and validation. `context-graph-storage` owns persisted records. `context-graph-mcp` and `context-graph-cli` expose orchestration and readbacks.

## V1 Model Interfaces

Core records:

- `TrainingRunRecord`
- `TrainingConfig`
- `ObjectiveConfig`
- `ModelArchitectureConfig`
- `EvaluationReport`
- `ModelArtifactRecord`
- `PredictionRequest`
- `PredictionRecord`

V1 service traits:

```text
DynamicJepaTrainer
  train(dataset_id, config) -> TrainingRunId

DynamicJepaInference
  predict(model_artifact_id, panel_id, candidate_action_id) -> PredictionId

ArtifactRegistry
  register_artifact(training_run_id, artifact_path) -> ModelArtifactId
```

Return values are not proof. Every method must persist a source-of-truth record and expose a readback.

## Objectives

Start with these objectives:

- `latent_delta`: predict target latent panel from input panel plus action panel.
- `masked_slot_prediction`: hide declared slots and predict their latent values.
- `action_contrast`: distinguish real next panels from real negative panels.
- `temporal_order`: score observed order higher than reversed or shuffled order.
- `surprise_calibration`: predict error/uncertainty so observed surprise can be thresholded.

Every objective must declare:

- input slots;
- target slots;
- mask policy;
- negative source;
- loss weight;
- metric names;
- required dataset fields.

If required dataset fields are missing, training fails before a training run starts.

## Training Run Lifecycle

1. Read domain pack, dataset shard, and model config from source-of-truth storage.
2. Validate dataset shape and objective requirements.
3. Create `TrainingRunRecord` with status `started`.
4. Read dataset rows and linked panels.
5. Train model.
6. Evaluate on declared splits.
7. Write artifact files to a configured artifact root.
8. Compute SHA-256 for every artifact file.
9. Register `ModelArtifactRecord` with hashes, sizes, config hash, dataset id, and training run id.
10. Update training run status to `completed` with artifact ids and metrics.
11. Independently read training run and artifact registry rows.
12. Recompute file hashes from disk and compare to registry rows.

If training fails after the run starts, persist training run status `failed` with error details. Do not register artifact rows for incomplete artifacts.

## Artifact Layout

Recommended physical path:

```text
artifacts/dynamicjepa/{domain_pack_id}/{training_run_id}/
  model.safetensors
  config.json
  metrics.json
  tokenizer_or_adapter_manifest.json
```

`ModelArtifactRecord` stores:

- artifact id;
- training run id;
- domain pack id/version;
- dataset id and shard ids;
- artifact path;
- file hashes;
- file sizes;
- model config hash;
- evaluation report hash;
- created timestamp;
- status.

Rules:

- artifact path must be canonicalized;
- files must exist before registration;
- file hashes must be recomputed during registration;
- loading a model must recompute hashes and reject mismatches;
- no loading "latest" unless latest is resolved to a concrete artifact id and printed.

## Inference And Prediction Records

`PredictionRecord` stores:

- prediction id;
- model artifact id;
- input panel id;
- candidate action id;
- predicted next latent panel;
- predicted outcome summary;
- uncertainty estimate;
- objective-specific scores;
- created timestamp;
- model hash seen at inference time.

Rules:

- prediction input panel must read from RocksDB;
- candidate action must be a persisted action record or a persisted hypothetical action record;
- model artifact hash must match the registered hash before inference;
- prediction row is written only after inference succeeds;
- observed outcome later links to prediction through a surprise event.

## Evaluation Metrics

Required v1 metrics:

- latent mean squared error by slot;
- cosine similarity by slot;
- action contrast accuracy;
- temporal order accuracy;
- calibration error for surprise threshold;
- dataset row count and skipped row count.

Skipped rows should normally be zero. Any skipped row count must include exact reason counts and source ids. Silent skip is forbidden.

## Training FSV

Source of truth:

- `CF_DJ_DATASET_SHARDS`;
- `CF_DJ_TRAINING_RUNS`;
- `CF_DJ_MODEL_ARTIFACTS`;
- artifact files on disk verified by hash.

Happy path proof:

- compile deterministic dataset;
- train a tiny model config that completes quickly;
- read training run row;
- read artifact row;
- hash files from disk;
- compare hashes;
- run prediction against a known panel/action;
- read prediction row and print predicted output shape and artifact id.

Edge cases:

- missing dataset shard: no training run row written;
- corrupt artifact hash: model load fails and no prediction row is written;
- objective requires negatives but dataset has none: training fails before started run or writes failed run with exact reason, depending on lifecycle point.
