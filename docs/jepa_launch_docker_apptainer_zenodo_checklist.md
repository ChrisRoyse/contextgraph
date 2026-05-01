# JEPA Launch Checklist: Docker, Apptainer/Singularity, Zenodo

This document lists what is done and what still needs to be done to get the
DynamicJEPA / ME-JEPA artifact ready for public launch.

## Current Launch State

Current local release candidate:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

Current status:

```text
local Docker artifact: complete and verified
local SIF artifact: complete and verified
local release archive: complete and verified
remaining public-launch blocker: Zenodo DOI not published
```

Critical hashes:

```text
Release archive SHA-256:
8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd

Docker tar SHA-256:
099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e

SIF SHA-256:
30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825
```

Main evidence report:

```text
docs/5090jepa/smokedocs/reports/20260501_sif_release_candidate_fsv.md
```

## Official Guidance References

- Docker reproducible builds and `SOURCE_DATE_EPOCH`:
  https://docs.docker.com/build/ci/github-actions/reproducible-builds/
- Docker immutable image digests:
  https://docs.docker.com/dhi/core-concepts/digests/
- Apptainer build documentation:
  https://apptainer.org/docs/user/main/build_a_container.html
- Apptainer appendix for `docker-archive` bootstrap:
  https://apptainer.org/docs/user/main/appendix.html
- Zenodo developer API:
  https://developers.zenodo.org/
- Zenodo DOI reservation:
  https://support.zenodo.org/help/en-gb/1-upload-deposit/35-how-can-i-reserve-a-doi-before-i-publish

## Docker: Done

Dockerfile:

```text
Dockerfile
```

Current Docker state:

- CUDA base image is pinned by digest.
- The runtime image copies the already-built `target/debug/context-graph`,
  `reproduce.sh`, configs, docs2 system specs, and reference files.
- Release-C produced two Docker archive exports.
- The two Docker tar files are byte-identical by hash.
- Docker verification ran inside the container with `--gpus all`.

Current Docker source of truth:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_container_verify_aggregate.json
```

Required launch checks before public release:

```bash
sha256sum \
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar \
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar

cmp -s \
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar \
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar

jq '{status, comparison, readback}' \
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_container_verify_aggregate.json
```

Expected:

```text
both Docker tar hashes equal 099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e
container verifier status is ok
diff_count is 0
manifest_status is passed
fsv_status is passed
```

Docker launch TODOs:

- Decide whether to publish a Docker image to a registry or distribute only
  `container/contextgraph-mejepa.docker.tar` through Zenodo.
- If publishing to a registry, push by immutable tag and record the registry
  digest in a new release manifest.
- Re-run container verification after pulling from the registry by digest.
- Do not use mutable tags as the source of truth.

## Apptainer/Singularity: Done

The host has SingularityCE installed from Ubuntu 24.04 packages:

```text
singularity-container 4.1.1+ds2-1ubuntu0.3
singularity-ce version 4.1.1
```

Current SIF source of truth:

```text
tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/contextgraph-mejepa.sif
```

Build command:

```bash
singularity build \
  tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif \
  docker-archive:tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar
```

Important syntax:

```text
Use docker-archive:relative/path.tar with this installed SingularityCE build.
The earlier docker-archive://relative/path.tar form failed by resolving to
//relative/path.tar.
```

Current SIF validation evidence:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/manifests/sif_inspect.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/manifests/sif_list.txt
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/manifests/sif_verify_aggregate.json
```

Required launch checks:

```bash
singularity inspect --json \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/contextgraph-mejepa.sif

singularity sif list \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/contextgraph-mejepa.sif

singularity exec --nv --cleanenv --no-home \
  -B /home/cabdru/contextgraph/tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate:/work/aggregate:ro \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/contextgraph-mejepa.sif \
  /workspace/reproduce.sh verify-aggregate \
    --aggregate /work/aggregate \
    --reference /workspace/reference/reference_release_v1.0.0.json \
    --json
```

Expected:

```text
SIF SHA-256 is 30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825
SIF inspect succeeds
SIF list shows one Squashfs system partition
SIF verify-aggregate status is ok
diff_count is 0
```

Apptainer/Singularity launch TODOs:

- Keep the SIF in the Zenodo artifact archive.
- Keep the SIF inspect/list/verify JSON in the archive.
- Document that launch verification uses `singularity exec --nv --cleanenv
  --no-home`.
- If using Apptainer instead of SingularityCE on another host, rebuild or
  revalidate the SIF and record the exact version and hash.

## Release Archive: Done Locally

Build command for current SIF-inclusive release candidate:

```bash
scripts/5090jepa/prepare_release_candidate_archive.sh \
  --release-id mejepa_5090_artifact_v1.0.0_rc5_sif_verified \
  --aggregate tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate \
  --local-ci-manifest tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json \
  --docker-tar-1 tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar \
  --docker-tar-2 tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar \
  --container-verify-json tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_container_verify_aggregate.json \
  --reference reference/reference_release_v1.0.0.json \
  --output-root tmp/mejepa_release_artifacts \
  --full-bundle-seed 42 \
  --apptainer-sif tmp/mejepa_release_artifacts/container_builds/contextgraph-mejepa_release_c_gate_v2.sif \
  --require-apptainer
```

Release archive readback files:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/release_manifest.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/reference_hashes.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/archive_file_hashes.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/container_manifest.json
```

Required launch checks:

```bash
jq '{status, release_stage, blockers, critical_hashes, container}' \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/release_manifest.json

jq '{docker, apptainer}' \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/container_manifest.json

tar -tzf \
  tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz \
  | rg 'contextgraph-mejepa.(docker.tar|sif)|sif_verify_aggregate|release_manifest|archive_file_hashes'
```

Expected:

```text
release_stage is local_docker_sif_release_candidate
blockers contains only zenodo_doi_not_published
container_manifest docker.verify is ok
container_manifest apptainer.verify_status is ok
archive contains Docker tar, SIF, SIF verification evidence, reference hashes,
and archive file hashes
```

## Zenodo: Not Done

Zenodo is the remaining public-launch blocker.

Needed inputs:

```text
Zenodo account
Zenodo access token with deposit write/publish permissions
final release title
author metadata
license metadata
artifact description
keywords
related identifiers for paper/code, if available
decision whether to use Zenodo sandbox first
```

Current archive to upload:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

Recommended fail-closed Zenodo flow:

1. Create a new deposition.
2. Reserve a DOI before publishing.
3. Upload the release archive.
4. Upload a small checksum file containing SHA-256 values.
5. Set metadata, license, creators, description, and related identifiers.
6. Read the deposition back through the API and verify:
   - archive file exists on Zenodo;
   - archive size matches local size;
   - checksum matches local SHA-256 if Zenodo exposes it;
   - DOI is reserved;
   - record is not accidentally public before final review.
7. Do a final local extraction test from the exact upload file.
8. Publish the deposition.
9. Resolve the DOI from a clean browser/API request.
10. Download the published archive and verify SHA-256 locally.
11. Update:
    - `CITATION.cff`
    - `docs/5090jepa/smokedocs/20_reproducibility_artifact_and_release.md`
    - `docs/5090jepa/smokedocs/23_ci_smoke_regression_and_drift_detection.md`
    - `reference/reference_release_v1.0.0.json` only if the release reference
      itself intentionally changes.

Do not publish if any readback step fails.

## Launch Acceptance Criteria

Launch is ready only when all of these are true:

- Docker tar exists and matches the recorded SHA-256.
- Docker verifier JSON reports `status=ok` and `diff_count=0`.
- SIF exists and matches the recorded SHA-256.
- SIF inspect/list/verify evidence is packaged.
- Release archive exists and matches the recorded SHA-256.
- `release_manifest.json` has no blocker except any intentionally documented
  DOI pre-publication state.
- Zenodo DOI is published and resolves.
- Downloaded Zenodo archive hash matches local archive hash.
- The paper or launch page states the safe claim only: public O*NET taxonomy
  demo, not real recruiting prediction.

