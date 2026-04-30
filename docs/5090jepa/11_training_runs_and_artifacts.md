# 11 — Training Runs & Artifacts (Persistence Contract)

The model file on disk is *not* proof that a training run succeeded. The proof is the `dj_training_runs` row plus the `dj_model_artifacts` row plus a recomputed file hash that matches the registry value. This file specifies the lifecycle and the FSV requirements.

Current implementation status as of the Phase 7/8 review on 2026-04-30: `dynamicjepa train`, `get-training-run`, and `get-artifact --verify-files` are implemented and FSV-accepted. The Phase 7 FSV writes and reads real `dj_training_runs` and `dj_model_artifacts` rows, recomputes artifact file hashes from disk, and proves corrupted artifact verification fails without writing predictions. The Phase 8 FSV verifies `predict` writes `model_artifact_hash_at_inference` from the verified artifact and refuses to write a prediction after artifact corruption.

## 1. Lifecycle Steps

```
Phase 7 training command:

(1) read DomainPack                  ─ source-of-truth: dj_domain_packs
(2) read DatasetShardRecord          ─ source-of-truth: dj_dataset_shards
(3) write TrainingRunRecord(started) ─ batch: dj_training_runs + dj_audit_log
(4) train (see 10_tiny_jepa_model.md)
(5) evaluate (val + test) and produce EvaluationReport
(6) write artifact files to disk     ─ artifacts/dynamicjepa/<domain>/<run_id>/
(7) compute SHA-256 of every file
(8) write ModelArtifactRecord        ─ batch: dj_model_artifacts + dj_training_runs(completed) + dj_audit_log
(9) FSV: read back training run, artifact row, recompute file hashes, compare
```

Each numbered step writes its persisted records before the next step begins. Failures at step N do **not** orphan rows from step <N: the only row that may remain after a failed training is the `dj_training_runs` row updated to `status = Failed { error_message: ... }`. No `dj_model_artifacts` row is written when training fails.

## 2. On‑Disk Layout

```
artifacts/dynamicjepa/<domain_pack_id>/<training_run_uuid>/
  model.safetensors          # online encoder + predictor + target encoder weights
  config.json                # exact training config used (canonical JSON)
  metrics.json               # final + per-epoch metrics
  evaluation_report.json     # val/test metrics structured per ObjectiveSpec
  manifest.json              # {file -> sha256, size_bytes}; produced last
```

Rules:

- The directory must not exist before step (6). If it does, fail with `Validation { field: "artifact_root", message: "artifact root directory already exists" }`.
- `model.safetensors` is candle’s safetensors format — single file with named tensors such as `online.encoder.layer0.weight`, `online.encoder.layer0.bias`, `target.encoder.layer0.weight`, and `predictor.layer0.weight`. No pickle.
- `config.json` is the *canonical* serialization of the config struct (sorted keys, no whitespace) so the hash is reproducible.
- `manifest.json` is written **last** after every other file's hash is computed, so a partial directory has no manifest and is therefore not "complete".

`artifact_root` is the CLI flag `--artifact-root`. The demo defaults to `/tmp/5090jepa_artifacts`. Operator can override; any path must be canonicalized (`std::fs::canonicalize`) before being written into the registry.

## 3. ModelArtifactRecord Fields (Repeated From `04_data_contracts_v1.md` For Convenience)

```rust
pub struct ModelArtifactRecord {
    pub header: DjRecordHeader,
    pub artifact_id: ModelArtifactId,
    pub training_run_id: TrainingRunId,
    pub domain_pack_id: DomainPackId,
    pub domain_pack_version: String,
    pub dataset_id: DatasetId,
    pub artifact_root: PathBuf,
    pub files: Vec<ArtifactFile>,
    pub model_config_hash: [u8; 32],
    pub evaluation_report_hash: [u8; 32],
    pub created_at_unix_ms: i64,
    pub status: ArtifactStatus,
}

pub struct ArtifactFile {
    pub relative_path: String,        // "model.safetensors", "config.json", ...
    pub sha256: [u8; 32],
    pub size_bytes: u64,
}

pub enum ArtifactStatus { Active, Quarantined { reason: String } }
```

`Active` is the only status written during a successful run. `Quarantined` is reserved for "we discovered a problem with this artifact after the fact" — out of scope for the demo.

## 4. Hashing Rules

- File hash: streaming SHA-256 of the file bytes. Implementation: `sha2::Sha256` chunked at 1 MiB.
- `model_config_hash`: SHA-256 over canonical `config.json` bytes (post-write).
- `evaluation_report_hash`: SHA-256 over canonical `evaluation_report.json` bytes (post-write).

`compute_artifact_hashes(artifact_root) -> Vec<ArtifactFile>` walks the directory in sorted order and produces the list. The same function is called at registration time and at FSV time; they must produce byte-identical output.

## 5. Artifact Load / Verification

Implementation in `crates/context-graph-dynamicjepa/src/artifact.rs`:

```rust
pub fn load_artifact_for_inference(
    db: &Database,
    artifact_id: ModelArtifactId,
) -> Result<LoadedArtifact, DynamicJepaError> {
    let registry = get_model_artifact(db, artifact_id)?
        .ok_or(DynamicJepaError::SourceOfTruthMissing { cf: "dj_model_artifacts", key: artifact_id.into_bytes() })?;
    let recomputed = compute_artifact_hashes(&registry.artifact_root)?;
    for (reg, disk) in registry.files.iter().zip(recomputed.iter()) {
        if reg.relative_path != disk.relative_path
            || reg.sha256 != disk.sha256
            || reg.size_bytes != disk.size_bytes
        {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: artifact_id.0,
                file: reg.relative_path.clone(),
                expected: hex::encode(reg.sha256),
                actual: hex::encode(disk.sha256),
            });
        }
    }
    let weights = load_safetensors(&registry.artifact_root.join("model.safetensors"))?;
    Ok(LoadedArtifact { registry, weights, ... })
}
```

The artifact load path **never** loads weights without first verifying every file hash. Mismatch → typed error. In Phase 7 the CLI exercises this through `get-artifact --verify-files`; Phase 8 exercises the same check through `predict` and verifies no `PredictionRecord` is written on mismatch.

The current corrupted-artifact edge cases explicitly rewrite one byte of `model.safetensors` after registration. Phase 7 asserts that `get-artifact --verify-files` returns `ARTIFACT_HASH_MISMATCH`; Phase 8 asserts that `predict` returns `ARTIFACT_HASH_MISMATCH`, reports `wrote_prediction=false`, and leaves `dj_predictions` unchanged.

## 6. Loading "Latest" Is Forbidden

There is no "latest" model resolution by name. Every inference call must reference a concrete `artifact_id` (a `Uuid`). Any helper that picks "latest by created_at" must be a separate explicit operation that resolves to a `Uuid` *and prints which uuid it picked* before passing it to the inference path. This is documented in the full plan's open questions doc and is a hard rule.

## 7. Evaluation Report

```rust
pub struct EvaluationReport {
    pub objective_id: String,
    pub split_metrics: BTreeMap<String, BTreeMap<String, f64>>,
    pub skipped_row_count: u32,
    pub skipped_reasons: BTreeMap<String, u32>,
    pub random_init_baseline: BTreeMap<String, f64>,
    pub shuffled_target_baseline: BTreeMap<String, f64>,
    pub vicreg_variance_per_dim_min: f64,
    pub vicreg_variance_per_dim_mean: f64,
    pub vicreg_covariance_off_diag_frobenius: f64,
    pub epoch_metrics: Vec<EpochMetric>,
    pub parameter_count_trainable: usize,
}
```

`split_metrics["val"]["latent_mse"]`, etc. The baselines are computed by the same `evaluate(...)` path that scored the trained model and are recorded in the same report. Current evidence shows good trained validation MSE/cosine and variance floor, but the shuffled-target metric remains high; do not describe the current model as fully passing the strict leakage audit yet.

If `skipped_row_count > 0`, every reason must have a count and an example row id. Silent skips are forbidden; a row is either evaluated or accounted for.

## 8. CLI

```
dynamicjepa train \
  --db /tmp/5090jepa-cw-fsv \
  --dataset-id <DATASET_UUID> \
  --config configs/dynamicjepa/verification/tiny_train_config.json \
  --artifact-root /tmp/5090jepa_artifacts \
  --json

dynamicjepa get-training-run --db ... --id <UUID> --json
dynamicjepa get-artifact      --db ... --id <UUID> --verify-files --json
```

`--verify-files` is the FSV-critical flag: with it, the CLI recomputes every file hash and includes a `verified: true|false` field per file in the JSON output. Without it, the CLI returns the registry values as-is and prints a warning "files not verified — pass --verify-files for FSV evidence".

## 9. FSV Evidence (Per Run)

Current Phase 7 evidence is written by `crates/context-graph-cli/tests/dynamicjepa_phase7_fsv.rs` to `tmp/5090jepa_evidence/phase7/cli_phase7_fsv.json`, with runtime DB/artifact files under `tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/`. After training, the evidence contains decoded output equivalent to:

```
training_run.json                  # decoded TrainingRunRecord
artifact.json                      # decoded ModelArtifactRecord
artifact_files_recomputed_hashes.json
  → [{relative_path, registry_sha256, recomputed_sha256, equal: true}]
metrics.json                       # final epoch metrics
evaluation_report.json             # full report incl. random/shuffled baselines
```

The harness asserts `equal == true` for every file. Any `equal: false` fails the verification run with status `Failed { failure_details: "...", evidence_path: "<path to mismatched file json>" }`.

## 10. Quarantining (Forward-Looking)

Out of scope for the demo, but defined so the schema is complete: `ArtifactStatus::Quarantined` would be set by an operator command (e.g. `dynamicjepa quarantine-artifact --id ... --reason "..."`) and the inference path would refuse to load any artifact whose status is not `Active`. Demo never sets quarantine; demo asserts `status == Active` at every read site.
