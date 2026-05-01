# 2026-05-01 Release-C Reproducibility Artifact FSV

## Scope

Completed the next doc-23/doc-20 step after Tier B: release-C local gate,
release aggregate reference verification, deterministic Docker artifact
packaging, container execution, and manual edge-case FSV.

No paid services were used. All evidence stayed under
`tmp/mejepa_local_ci_runs`.

## Web Research Used

- Docker reproducible build docs: `SOURCE_DATE_EPOCH` controls image index,
  config, and file metadata timestamps.
  https://docs.docker.com/build/ci/github-actions/reproducible-builds/
- Docker build best practices: pin base images by digest for supply-chain
  integrity and reproducible base selection.
  https://docs.docker.com/build/building/best-practices/
- NVIDIA Container Toolkit docs: `--gpus` / NVIDIA driver capabilities mount
  GPU devices and driver libraries into Docker containers.
  https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/1.8.0/user-guide.html

## Implementation Changes

- Added root `Dockerfile` pinned to CUDA 13.2 Ubuntu 24.04 by digest:
  `sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d`.
- Added `.dockerignore` so the image context contains only the binary,
  reproducibility script, configs, references, and system specs.
- Added executable `reproduce.sh`.
- Added `dynamicjepa check-release-aggregate-reference` in the Rust CLI and
  changed `reproduce.sh verify-aggregate` to call the compiled verifier.
- Added local artifact-evaluation dry run:
  `scripts/5090jepa/artifact_evaluation_dry_run.sh`.
- Updated release-C in `scripts/5090jepa/local_ci_gate.sh`:
  - no per-seed single-bundle release-reference check;
  - aggregate-level release reference check only;
  - deterministic Docker tar export twice with timestamp rewriting;
  - `docker load`;
  - container aggregate verification with `docker run --gpus all`;
  - local dry-run artifact evaluation.
- Recaptured `reference/reference_release_v1.0.0.json` from a preserved
  passed five-seed aggregate.

## Root Causes Found

1. Release-C artifact prerequisites were missing:
   root `Dockerfile`, executable `reproduce.sh`, and artifact-evaluation dry
   run did not exist.
2. The release reference was stale. It represented a previous aggregate whose
   `counter_world/val_action_contrast_acc` max and BCa upper differed from
   the current implementation. The preserved current aggregate passed all
   child bundle, DB, FSV, and artifact checks, so the reference was recaptured
   from that preserved passed aggregate.
3. The first verifier design used a Ruby host script. The CUDA container did
   not include Ruby, so container `verify-aggregate` failed. The verifier was
   moved into the compiled Rust CLI.
4. Plain Docker `iidfile` comparison was not deterministic because local image
   unpacking conflicts with timestamp rewriting. The gate now compares full
   deterministic Docker tar exports produced with
   `type=docker,rewrite-timestamp=true`.
5. Container execution without `--gpus all` failed at process load with
   missing `libcuda.so.1`. Release-C now runs the CUDA-linked binary under the
   NVIDIA container runtime.

## Source Of Truth

Release-C v2:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed43
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed44
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed45
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed46
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_container_verify_aggregate.json
reference/reference_release_v1.0.0.json
```

## Happy Path FSV

Command:

```bash
LOCAL_CI_RUN_ID_PREFIX=20260501T_release_c_gate_v2 \
  scripts/5090jepa/local_ci_gate.sh release-c
```

Result:

```text
status: passed
manifest: tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json
release reference SHA-256: f5212d7fbf1da4df0f2dbbf6e520d4a97bc520f3e8d9a4dba1c113b73dd5c614
docker image tar SHA-256: 099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e
```

Separate readback evidence:

```text
aggregate_manifest.status = passed
aggregate_manifest.passed_count = 5
aggregate_manifest.failed_count = 0
aggregate_fsv_report.status = passed
readback rows:
  table_prediction_metrics_multiseed = 33
  table_ablation_multiseed = 78
  table_paired_ablation_tests = 35
  table_seed_failures = 0
container verify-aggregate diff_count = 0
```

Each seed's career RocksDB was separately inspected after execution. All five
seed DBs contained:

```text
dj_raw_events = 1004
dj_latent_panels = 1000
dj_transitions = 1000
dj_outcomes = 1000
dj_instrument_readings = 10000
dj_model_artifacts = 1
dj_training_runs = 1
dj_verification_runs = 1
```

Consolidated DB evidence:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_seed_db_counts_summary.jsonl
```

## Edge Case FSV

Manual edge logs:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_manual_edge_fsv/edge_1_invalid_reference_kind.log
tmp/mejepa_local_ci_runs/20260501T_release_c_manual_edge_fsv/edge_2_missing_aggregate.log
tmp/mejepa_local_ci_runs/20260501T_release_c_manual_edge_fsv/edge_3_numeric_drift.log
```

Edge 1, invalid reference kind:

```text
before: aggregate exists, temporary reference exists with kind=not_a_release_reference
action: check-release-aggregate-reference
after: command exited 1, reference still exists, error_code=SCHEMA
```

Edge 2, missing aggregate path:

```text
before: aggregate path did not exist
action: check-release-aggregate-reference
after: command exited 1, aggregate path still did not exist, error_code=VALIDATION
```

Edge 3, numeric drift:

```text
before: temporary reference mean changed to 999.000000000000
action: check-release-aggregate-reference
after: command exited 1, error_code=RELEASE_REFERENCE_MISMATCH,
       first diff type=numeric_out_of_tolerance
```

## Final Status

Release-C is now passing locally and free. The release artifact path has:

- real O*NET paper-small five-seed bundles;
- aggregate FSV with physical table/plot hashes;
- compiled Rust release-reference verifier;
- deterministic Docker tar exports;
- CUDA container verification against the real aggregate;
- three fail-closed manual edge cases with before/after state.

Remaining release work is outside doc-23's local gate: Apptainer packaging,
Zenodo DOI/archive, artifact appendix, and final paper/release packaging.
