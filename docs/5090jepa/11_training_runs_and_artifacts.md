# 11 — Training Runs & Artifacts (Persistence Contract)

The model file on disk is *not* proof that a training run succeeded. The proof is the `dj_training_runs` row plus the `dj_model_artifacts` row plus a recomputed file hash that matches the registry value. This file specifies the lifecycle and the FSV requirements.

## 1. Lifecycle Steps

```
Phase 7 / Phase 8 training command:

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
- `model.safetensors` is candle’s safetensors format — single file with named tensors `online.layer_0.weight`, …, `target.layer_out.bias`, etc. No pickle.
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

## 5. Inference-Time Hash Verification

Implementation in `crates/context-graph-dynamicjepa/src/model/inference.rs`:

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

The inference path **never** loads weights without first verifying every file hash. Mismatch → typed error → no `PredictionRecord` is written.

The corrupted-artifact edge case (`tests/artifact_hash_fsv.rs`) explicitly rewrites one byte of `model.safetensors` after registration and asserts that the next `predict` call returns `ArtifactHashMismatch` and writes no prediction row.

## 6. Loading "Latest" Is Forbidden

There is no "latest" model resolution by name. Every inference call must reference a concrete `artifact_id` (a `Uuid`). Any helper that picks "latest by created_at" must be a separate explicit operation that resolves to a `Uuid` *and prints which uuid it picked* before passing it to the inference path. This is documented in the full plan's open questions doc and is a hard rule.

## 7. Evaluation Report

```rust
pub struct EvaluationReport {
    pub training_run_id: TrainingRunId,
    pub artifact_id: ModelArtifactId,
    pub objective_id: String,
    pub split_metrics: BTreeMap<String, BTreeMap<String, f64>>,
    pub skipped_row_count: u32,
    pub skipped_reasons: BTreeMap<String, u32>,
    pub random_init_baseline: BTreeMap<String, f64>,
    pub shuffled_target_baseline: BTreeMap<String, f64>,
    pub vicreg_variance_per_dim_min: f64,
    pub vicreg_variance_per_dim_mean: f64,
    pub vicreg_covariance_off_diag_frobenius: f64,
}
```

`split_metrics["val"]["latent_mse"]`, etc. The skipped baselines are computed by the same `evaluate(...)` function that scored the trained model — they are recorded in the same report so a reviewer can see in one place "trained: cosine 0.99, shuffled: 0.07, random-init: 0.05". This is the model's anti-leakage proof.

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
dynamicjepa register-artifact --db ... --training-run-id <UUID> --path PATH --json   # only if a manual artifact write is needed; demo never uses this
```

`--verify-files` is the FSV-critical flag: with it, the CLI recomputes every file hash and includes a `verified: true|false` field per file in the JSON output. Without it, the CLI returns the registry values as-is and prints a warning "files not verified — pass --verify-files for FSV evidence".

## 9. FSV Evidence (Per Run)

After training, `tmp/5090jepa_evidence/<domain>/<run_id>/decoded/` contains:

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
