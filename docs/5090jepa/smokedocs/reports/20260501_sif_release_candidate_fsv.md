# SIF Release Candidate FSV - 2026-05-01

## Scope

Completed the HPC container portion of the release candidate by installing a
real SingularityCE toolchain, building a SIF from the deterministic Docker
archive, validating the SIF with aggregate verification, and rebuilding the
release archive with `--require-apptainer`.

## Sources Checked

- Apptainer/Singularity build guidance supports building SIF images from
  Docker/OCI sources and local archives:
  https://apptainer.org/docs/user/main/build_a_container.html
- The Apptainer appendix documents the `docker-archive` bootstrap agent for
  creating an image from a Docker-save tar file:
  https://apptainer.org/docs/user/main/appendix.html
- Zenodo REST API supports depositions, DOI reservation, uploads, and publish
  actions, but this run had no Zenodo token:
  https://developers.zenodo.org/

## System Tooling

Installed from Ubuntu 24.04 packages:

```text
singularity-container 4.1.1+ds2-1ubuntu0.3
squashfuse 0.5.0-2build1
containernetworking-plugins 1.1.1+ds1-3ubuntu0.24.04.3
conmon 2.1.10+ds1-1build2
```

Installed command:

```text
/usr/bin/singularity
singularity-ce version 4.1.1
```

## SIF Build

Source of truth:

```text
Docker tar: tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar
SIF: tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif
```

Command:

```bash
singularity build \
  tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif \
  docker-archive:tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar
```

Readback:

```text
SIF SHA-256: 30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825
SIF size: 3616337920 bytes
```

SIF metadata:

```json
{
  "version": "4.1.1",
  "bootstrap": "docker-archive",
  "from": "tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar",
  "title": "contextgraph-mejepa"
}
```

SIF layout:

```text
ID 4 | FS (Squashfs/*System/amd64) | SIF POSITION 45056-3616337920
```

## SIF Aggregate Verification

Command shape:

```bash
singularity exec --nv --cleanenv --no-home \
  -B /home/cabdru/contextgraph/tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate:/work/aggregate:ro \
  tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif \
  /workspace/reproduce.sh verify-aggregate \
    --aggregate /work/aggregate \
    --reference /workspace/reference/reference_release_v1.0.0.json \
    --json
```

Readback:

```json
{
  "status": "ok",
  "diff_count": 0,
  "manifest": "passed",
  "fsv": "passed",
  "passed_count": 5,
  "rows": {
    "ablation_multiseed_rows": 78,
    "paired_ablation_test_rows": 35,
    "prediction_multiseed_rows": 33,
    "seed_failure_rows": 0
  }
}
```

## Final Local Archive

Current source of truth:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

Build report:

```json
{
  "status": "ok",
  "release_id": "mejepa_5090_artifact_v1.0.0_rc5_sif_verified",
  "archive_sha256": "8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd",
  "artifact_dir_size_bytes": 8376098295,
  "artifact_file_count": 1481,
  "archive_size_bytes": 7360205717,
  "release_stage": "local_docker_sif_release_candidate",
  "blockers": ["zenodo_doi_not_published"]
}
```

Physical hash readback:

```text
30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825  container/contextgraph-mejepa.sif
099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e  container/contextgraph-mejepa.docker.tar
8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd  mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

`container/container_manifest.json` readback:

```json
{
  "docker": {
    "sha": "099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e",
    "verify": "ok"
  },
  "apptainer": {
    "status": "included",
    "sha": "30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825",
    "verify_status": "ok",
    "diff_count": 0
  }
}
```

Archive hash registry readback:

```json
{
  "file_count": 1479,
  "sif": {
    "sha256": "30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825",
    "size_bytes": 3616337920,
    "path": "container/contextgraph-mejepa.sif"
  },
  "docker": {
    "sha256": "099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e",
    "size_bytes": 3644745728,
    "path": "container/contextgraph-mejepa.docker.tar"
  },
  "sif_verify": {
    "sha256": "8b9fd3f0c81fe8dd5183af0d6d7cb12783cf9f69b4ee843f57e14f80183d308d",
    "size_bytes": 2176,
    "path": "manifests/sif_verify_aggregate.json"
  }
}
```

Compressed archive listing was inspected and includes:

```text
container/contextgraph-mejepa.docker.tar
container/contextgraph-mejepa.sif
manifests/sif_inspect.json
manifests/sif_list.txt
manifests/sif_verify_aggregate.json
release_manifest.json
reference_hashes.json
archive_file_hashes.json
```

## Root Causes Fixed

1. The first Singularity probe used `docker-archive://relative/path.tar`.
   SingularityCE interpreted this as `//relative/path.tar` and failed. The
   correct local archive syntax is `docker-archive:relative/path.tar`.
2. `prepare_release_candidate_archive.sh` accepted an existing SIF path without
   proving it was a valid and runnable SIF. The script now requires:
   `singularity inspect --json`, `singularity sif list`, and
   `singularity exec --nv --cleanenv --no-home ... verify-aggregate`.
3. The first invalid-SIF edge failed after creating a partial output directory.
   SIF validation now runs before `artifact_dir` is created.

## Edge Cases

Final edge logs:

```text
tmp/mejepa_release_artifacts/manual_edge_fsv_final/edge_1_existing_rc4_output.log
tmp/mejepa_release_artifacts/manual_edge_fsv_final/edge_2_missing_required_sif.log
tmp/mejepa_release_artifacts/manual_edge_fsv_final/edge_4_invalid_sif_preoutput_validation.log
```

Existing output:

```text
BEFORE exists=true kind=directory size_bytes=8376099726
RELEASE_ARCHIVE_OUTPUT_DIR_EXISTS
EXIT_CODE=1
AFTER exists=true kind=directory size_bytes=8376099726
```

Missing required SIF:

```text
BEFORE exists=false
RELEASE_ARCHIVE_APPTAINER_SIF_MISSING
EXIT_CODE=1
AFTER exists=false
```

Invalid SIF after pre-output validation fix:

```text
BEFORE exists=false
FATAL: Failed to open image reference/reference_release_v1.0.0.json: image format not recognized
RELEASE_ARCHIVE_APPTAINER_INSPECT_FAILED
EXIT_CODE=1
AFTER exists=false
```

## Remaining Blocker

The only remaining final-release blocker in `release_manifest.json` is:

```text
zenodo_doi_not_published
```

Publishing requires a Zenodo access token and a deliberate publish action.
No Zenodo token was available during this run, so no DOI reservation or upload
was attempted.
