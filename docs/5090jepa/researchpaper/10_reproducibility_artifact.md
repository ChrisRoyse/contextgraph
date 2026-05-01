# 8. Reproducibility Artifact

The reproducibility artifact is the canonical mechanism by which a
third party regenerates §7's numbers from a clean machine. As of
2026-05-01 the artifact is at the **local Docker + Singularity (SIF)
release-candidate** stage.

## 8.1 The Reviewer Path

```bash
# 1. Pull (or import) the container from the artifact archive.
tar -xf mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
cd mejepa_5090_artifact_v1.0.0_rc5_sif_verified

# 2. Use the SIF (HPC-friendly) ...
singularity exec --nv --cleanenv --no-home \
  -B "$(pwd)/bundles/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate:/work/aggregate:ro" \
  container/contextgraph-mejepa.sif \
  /workspace/reproduce.sh verify-aggregate \
    --aggregate /work/aggregate \
    --reference /workspace/reference/reference_release_v1.0.0.json \
    --json

# 2'. ... or use the Docker tar (laptop-friendly).
docker load -i container/contextgraph-mejepa.docker.tar
docker run --gpus all \
  -v "$(pwd)/bundles/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate:/work/aggregate:ro" \
  contextgraph-mejepa:latest \
  verify-aggregate \
    --aggregate /work/aggregate \
    --reference /workspace/reference/reference_release_v1.0.0.json \
    --json
```

Either invocation should print, byte-for-byte:

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

(Exact JSON read back from
`tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/manifests/sif_verify_aggregate.json`.
Source:
[reports/20260501_sif_release_candidate_fsv.md](../smokedocs/reports/20260501_sif_release_candidate_fsv.md).)

## 8.2 Container Strategy

Two images, same content, two formats.

### Docker (laptop-facing)

- Base: `nvidia/cuda:13.2.0-devel-ubuntu24.04`, pinned by digest
  `sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d`
  in the checked-in `Dockerfile`.
- Pinned: `rustc 1.95.0`, `cargo 1.95.0`, `candle-core` git hash from
  `Cargo.lock`.
- Build is done on the verified RTX 5090 host; the image then packages
  the resulting `target/debug/context-graph` plus `reproduce.sh`,
  `configs/dynamicjepa`, `reference/`, `docs2/systemspecs.md`, and
  the explicit `.dockerignore`. Nothing else is included.
- Determinism is checked with two consecutive `docker buildx build`
  exports using `SOURCE_DATE_EPOCH` and `type=docker,rewrite-timestamp=true`.
  Both exports must be byte-identical:

| Export | SHA-256 |
| --- | --- |
| `..._docker_image_1.tar` | `099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e` |
| `..._docker_image_2.tar` | `099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e` |

Source: release-C v2 manifest. Rationale: Docker reproducible-build
guidance [Docker, 2025] and base-image-by-digest pinning best practice
[Docker, 2024].

### Apptainer (HPC-facing, primary for paper artifact)

- Built from the Docker tar via the documented `docker-archive`
  bootstrap agent [Apptainer, 2025]:

```bash
singularity build \
  container_builds/contextgraph-mejepa.sif \
  docker-archive:20260501T_release_c_gate_v2_docker_image_1.tar
```

- Run with `--cleanenv --no-home --nv` for maximum isolation.
- Resulting SIF: 3{,}616{,}337{,}920 bytes; SHA-256
  `30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825`.
- `singularity inspect`, `singularity sif list`, and the aggregate
  `verify-aggregate` are all required by the packager *before* it
  will include a supplied SIF; the packager fails closed if any of
  these does not pass. Logs are at
  `tmp/mejepa_release_artifacts/manual_edge_fsv_final/edge_4_invalid_sif_preoutput_validation.log`.

## 8.3 Build Determinism

- Every Rust dep is pinned by `Cargo.lock` (workspace-level).
- The CUDA base image is pinned by digest, not tag.
- No `apt update && apt install` happens inside the Dockerfile (the
  binary is built on the host on the verified RTX 5090 toolchain and
  copied in).
- The Docker tar is exported with `SOURCE_DATE_EPOCH` and
  `type=docker,rewrite-timestamp=true`; two consecutive exports are
  required to be byte-identical, and the gate fails if not.
- The release manifest records the resulting Docker image digest +
  Apptainer SIF SHA-256.

We make *no* claim of byte-equality across hosts or sm versions.
Per [PyTorch, 2026] and observed RTX 5090 behavior, GPU training can
produce small numeric differences across releases, platforms, or even
re-runs on the same host. The release reference therefore uses a
*structural + numeric-tolerance* schema (`tolerances.json`); cross-run
exact model-weight bytes are diagnostic evidence, not the pass/fail
criterion (§6,
[smokedoc 23](../smokedocs/23_ci_smoke_regression_and_drift_detection.md)).

## 8.4 Artifact Layout

```text
mejepa_5090_artifact_v1.0.0_rc5_sif_verified/
  README.md                             one-page reproduce instructions
  CITATION.cff                          how to cite (PolyForm-Noncommercial-1.0.0)
  LICENSE                               PolyForm Noncommercial 1.0.0 for code
  THIRD_PARTY_LICENSES/ONET.md          O*NET CC BY 4.0 attribution
  bin/
    context-graph                       static-mostly Rust binary, sha256=41560d160…
  container/
    contextgraph-mejepa.docker.tar      099f053…7d4cd80e
    contextgraph-mejepa.sif             30b531c…f2825
    container_manifest.json             both digests + verify=ok + diff_count=0
  bundles/
    20260501T_release_c_gate_v2_paper_small_seed42/
      run_manifest.json                  status=passed
      fsv_report.json                    status=passed
      data_manifest.json                 O*NET source SHA-256 + license + walk hash
      paper_tables/*.csv                 5×CSV
      plots/*.svg                        2×SVG
      dbs/*_rocksdb/                     three per-domain RocksDB instances
      ... (all of: domain_pack_manifest, environment, hardware,
           git_state, reproducibility, evaluation_report,
           training_metrics, command_log, command_outputs,
           edge_case_results, ablation_results, db_counts_{before,after},
           generated_fixtures, decoded_records, fsv_evidence,
           local_ci_inspect_counts_career_taxonomy.json,
           onet_source_preflight.json, artifact_hashes.json)
    20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/
      aggregate_manifest.json
      aggregate_fsv_report.json
      paper_tables/table_prediction_metrics_multiseed.csv
      paper_tables/table_ablation_multiseed.csv
      paper_tables/table_paired_ablation_tests.csv
      paper_tables/table_seed_failures.csv
      plots/*.svg
  paper_tables_reference/                exact CSVs cited by §7
  reference_hashes.json                  SHA-256 of every file in archive
  reference_release_v1.0.0.json          checked-in numeric+structural reference
  reproduce.sh                           reviewer command
  manifests/
    container_graph_ldd.txt              dynamic-link readback for bin/context-graph
    sif_verify_aggregate.json            SIF aggregate verify result
    sif_inspect.json                     singularity inspect output
    sif_list.txt                         singularity sif list output
  release_manifest.json                  release_stage="local_docker_sif_release_candidate";
                                         blockers=["zenodo_doi_not_published"]
  archive_file_hashes.json               file_count + SHA-256 of every file
  build_report.json                      packager output
  CHANGELOG.md
```

Top-level archive hash:

```text
8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd
mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

(Source: `release_manifest.json` and `archive_file_hashes.json` of the
rc5 release candidate; cross-checked with the SIF FSV report and the
build report
[reports/20260501_sif_release_candidate_fsv.md](../smokedocs/reports/20260501_sif_release_candidate_fsv.md).)

## 8.5 Reviewer Reproducibility Mapping

The artifact pre-fills the standard reproducibility-checklist questions
from bundle paths:

| Question | Bundle path / Source |
| --- | --- |
| Code released? | release archive `bin/context-graph` and `container/contextgraph-mejepa.{sif,docker.tar}` |
| Hardware listed? | `bundles/<seed>/hardware.json` per bundle |
| Software listed? | `bundles/<seed>/environment.json` per bundle |
| Random seeds? | `bundles/<seed>/run_manifest.json` (`career_training_seed`, `career_ablation_training_seed_effective`, `career_ablation_training_seed_origin`) |
| Hyperparameters? | `bundles/<seed>/training_metrics.json` and persisted in `dj_model_artifacts` config |
| Datasets + licenses? | `bundles/<seed>/data_manifest.json` |
| Computational budget? | `bundles/<seed>/run_manifest.json` (`updated_at_unix_ms - created_at_unix_ms`) and §5.6 |
| Limitations? | §10 |
