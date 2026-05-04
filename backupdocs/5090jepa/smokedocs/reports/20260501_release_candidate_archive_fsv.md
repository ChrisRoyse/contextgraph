# Release Candidate Archive FSV - 2026-05-01

## Scope

Continued release work after Tier C passed by adding a local, no-upload
release-candidate archive builder. The goal was to package preserved release-C
evidence without hiding missing final-release requirements.

## Best-Practice References Checked

- Docker reproducible-build guidance records `SOURCE_DATE_EPOCH` as the
  timestamp-control mechanism for reproducible image output:
  https://docs.docker.com/build/ci/github-actions/reproducible-builds/
- Apptainer documents direct SIF creation from a Docker archive via
  `apptainer build <image>.sif docker-archive:<image>.tar`, but no Apptainer
  or Singularity binary is installed on this host:
  https://apptainer.org/docs/user/latest/docker_and_oci.html
- Zenodo's API supports depositions, file uploads, DOI reservation, and publish
  actions, but publication was not performed from this local run:
  https://developers.zenodo.org/
- ACM artifact badges require artifacts to be documented, complete, exercisable,
  and permanently available for the relevant badge. Local rc2 is exercisable
  evidence, not a DOI-published final artifact:
  https://www.acm.org/publications/policies/artifact-review-badging

## Implementation

Added:

- `scripts/5090jepa/prepare_release_candidate_archive.sh`
- `make release-candidate-archive`
- `CITATION.cff`
- `THIRD_PARTY_LICENSES/ONET.md`

Updated:

- `docs/5090jepa/smokedocs/20_reproducibility_artifact_and_release.md`
- `docs/5090jepa/smokedocs/23_ci_smoke_regression_and_drift_detection.md`

The packager validates the release-C local CI manifest, aggregate manifest,
aggregate FSV report, deterministic Docker tar pair, CUDA base digest pinning,
container verification JSON, checked-in release reference, selected full seed
bundle, root license, licensing boundary, and citation metadata. It refuses to
overwrite existing archive paths.

## Source Of Truth

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2/
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2.tar.gz
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2/release_manifest.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2/archive_file_hashes.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2/build_report.json
```

## Happy Path Command

```bash
scripts/5090jepa/prepare_release_candidate_archive.sh \
  --release-id mejepa_5090_artifact_v1.0.0_rc2 \
  --aggregate tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate \
  --local-ci-manifest tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json \
  --docker-tar-1 tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar \
  --docker-tar-2 tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_2.tar \
  --container-verify-json tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_container_verify_aggregate.json \
  --reference reference/reference_release_v1.0.0.json \
  --output-root tmp/mejepa_release_artifacts \
  --full-bundle-seed 42
```

Result:

```json
{
  "status": "ok",
  "release_id": "mejepa_5090_artifact_v1.0.0_rc2",
  "archive_sha256": "eaa3c8cf72524a1e3746bb17d6cf4deb453f2e632a6404f4281efd03b854e85f",
  "artifact_dir_size_bytes": 4759753295,
  "artifact_file_count": 1477,
  "archive_size_bytes": 3761154563,
  "release_stage": "local_docker_release_candidate",
  "blockers": [
    "apptainer_sif_not_included_on_this_host",
    "zenodo_doi_not_published"
  ]
}
```

## Readback Evidence

`release_manifest.json` readback:

```json
{
  "status": "packaged",
  "release_stage": "local_docker_release_candidate",
  "context_graph_bin_sha256": "41560d160acdaade55f16153678db373c33127ce7a12b4eb5a418d30e86ec0eb",
  "docker_tar_sha256": "099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e",
  "archive_file_hashes_sha256": "f01c89eab689e7b0ee57d6ab514533f51c81f192e5128db564e4de3f2d094e4f"
}
```

Physical hash readback:

```text
41560d160acdaade55f16153678db373c33127ce7a12b4eb5a418d30e86ec0eb  bin/context-graph
099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e  container/contextgraph-mejepa.docker.tar
eaa3c8cf72524a1e3746bb17d6cf4deb453f2e632a6404f4281efd03b854e85f  mejepa_5090_artifact_v1.0.0_rc2.tar.gz
```

`archive_file_hashes.json` readback:

```json
{
  "file_count": 1475,
  "bin_context_graph": {
    "sha256": "41560d160acdaade55f16153678db373c33127ce7a12b4eb5a418d30e86ec0eb",
    "size_bytes": 156293880,
    "path": "bin/context-graph"
  },
  "docker": {
    "sha256": "099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e",
    "size_bytes": 3644745728,
    "path": "container/contextgraph-mejepa.docker.tar"
  },
  "aggregate_manifest_entries": 1,
  "ldd": {
    "sha256": "32eb9b4f8ced3337ab5db956af8178a47e78c9e95fbc2e40e1f50e7392848ab8",
    "size_bytes": 1072,
    "path": "manifests/context_graph_ldd.txt"
  }
}
```

Packaged verifier command run from inside the archive directory:

```bash
CONTEXT_GRAPH_BIN=bin/context-graph ./reproduce.sh verify-aggregate \
  --aggregate bundles/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate \
  --reference reference_release_v1.0.0.json \
  --json
```

Readback result:

```json
{
  "status": "ok",
  "comparison": {
    "diff_count": 0,
    "table_results": [
      {"table": "table_prediction_metrics_multiseed", "actual_row_count": 33, "reference_row_count": 33},
      {"table": "table_ablation_multiseed", "actual_row_count": 78, "reference_row_count": 78},
      {"table": "table_paired_ablation_tests", "actual_row_count": 35, "reference_row_count": 35},
      {"table": "table_seed_failures", "actual_row_count": 0, "reference_row_count": 0}
    ]
  },
  "readback": {
    "manifest_status": "passed",
    "fsv_status": "passed",
    "passed_count": 5,
    "failed_count": 0
  }
}
```

Compressed archive listing was separately inspected and contains:

```text
mejepa_5090_artifact_v1.0.0_rc2/archive_file_hashes.json
mejepa_5090_artifact_v1.0.0_rc2/bin/context-graph
mejepa_5090_artifact_v1.0.0_rc2/bundles/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/aggregate_manifest.json
mejepa_5090_artifact_v1.0.0_rc2/bundles/20260501T_release_c_gate_v2_paper_small_seed42/run_manifest.json
mejepa_5090_artifact_v1.0.0_rc2/container/contextgraph-mejepa.docker.tar
mejepa_5090_artifact_v1.0.0_rc2/release_manifest.json
```

## Root Cause Found And Fixed

The first generated archive candidate (`rc1`) included `reproduce.sh` but did
not include a local verifier binary. That meant the archive-local README
verification command depended on an external build tree. The packager now
requires `CONTEXT_GRAPH_BIN` or `target/debug/context-graph`, copies it into
`bin/context-graph`, records its SHA-256, and writes
`manifests/context_graph_ldd.txt`.

`rc1` remains preserved as failed/obsolete evidence. `rc2` is the current
candidate.

## Edge Cases

Edge logs:

```text
tmp/mejepa_release_artifacts/manual_edge_fsv/edge_1_existing_output_dir.log
tmp/mejepa_release_artifacts/manual_edge_fsv/edge_2_missing_docker_tar.log
tmp/mejepa_release_artifacts/manual_edge_fsv/edge_3_require_missing_apptainer.log
tmp/mejepa_release_artifacts/manual_edge_fsv/edge_4_invalid_release_id.log
```

### Edge 1 - Existing Output Directory

Expected: fail without modifying the existing rc2 archive directory.

Observed:

```text
BEFORE exists=true kind=directory size_bytes=4759754680
RELEASE_ARCHIVE_OUTPUT_DIR_EXISTS: refusing to overwrite existing path 'tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2'
EXIT_CODE=1
AFTER exists=true kind=directory size_bytes=4759754680
```

### Edge 2 - Missing Docker Tar

Expected: fail before creating the output artifact directory.

Observed:

```text
BEFORE exists=false
RELEASE_ARCHIVE_DOCKER_TAR_1_MISSING: required file 'tmp/mejepa_local_ci_runs/does_not_exist_docker_image_1.tar' does not exist
EXIT_CODE=1
AFTER exists=false
```

### Edge 3 - Required Apptainer Missing

Expected: fail before creating the output artifact directory.

Observed:

```text
BEFORE exists=false
RELEASE_ARCHIVE_APPTAINER_REQUIRED: --require-apptainer requires --apptainer-sif PATH
EXIT_CODE=1
AFTER exists=false
```

### Additional Guard - Invalid Release ID

Expected: reject path traversal-like release IDs before any output path is
derived.

Observed:

```text
RELEASE_ARCHIVE_RELEASE_ID_INVALID: --release-id must start and end with a letter or number and may contain only letters, numbers, dot, underscore, and dash
EXIT_CODE=1
```

## Remaining Release Blockers

- No `apptainer` or `singularity` binary is installed on this host, so the
  `.sif` artifact cannot be produced locally without installing that tooling.
- No Zenodo upload, DOI reservation, DOI publication, or DOI-resolution check
  has been performed.
- `rc2` is a local Docker release candidate and FSV proof, not the final ACM
  Available artifact.
