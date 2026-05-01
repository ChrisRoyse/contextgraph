# 20 - Reproducibility Artifact And Release

## Goal

Package the system so an external reviewer with one RTX 5090 (or comparable
sm_120 / Ada Lovelace fallback) can reproduce the headline numbers from a
single command, with no hidden state. Target ACM Artifact Evaluation badges
**Available + Functional + Reusable** and the equivalent NeurIPS / ICML
reproducibility expectations.

## Reviewer's Path (One Command)

The artifact must support exactly this end-user flow:

```bash
# 1. Pull the container (Docker example; Apptainer mirrored).
docker pull <registry>/contextgraph-mejepa:<tag>

# 2. Mount the local O*NET ZIP and an output directory.
docker run --gpus all \
  -v /path/to/onet:/data/onet:ro \
  -v /path/to/output:/work \
  <registry>/contextgraph-mejepa:<tag> \
  paper_small_1k --seeds 42,43,44,45,46
```

The `paper_small_1k` mode wraps `dynamicjepa research-smoke` + the multi-seed
aggregator (doc 15) and writes the full bundle to `/work`. After completion,
the script prints the SHA-256 of `aggregate_manifest.json` and the reviewer
checks it against the published reference hash.

If the reference hash matches, the reviewer has reproduced the paper-headline
multi-seed bundle. If it does not, the script writes a `diff_report.json`
naming which CSV row(s) differ and by how much; expected differences
(non-deterministic kernel order, etc.) are listed in advance.

## Container Strategy

Two images, same content, two formats:

### Docker (developer-facing)

- Base: `nvidia/cuda:13.2.0-devel-ubuntu24.04`, pinned by digest in the
  checked-in `Dockerfile`.
- Pinned: `rustc 1.95.0`, `cargo 1.95.0`, `candle-core` git hash from
  `Cargo.lock`.
- Current local release gate builds the Rust binary on the verified RTX 5090
  host, then packages only `target/debug/context-graph`, `reproduce.sh`,
  configs, docs, and checked-in references into the CUDA image. This avoids
  unpinned package installation inside the image and keeps the runtime
  dependency surface explicit.
- Determinism is checked with two `docker buildx build` exports using
  `SOURCE_DATE_EPOCH` and `type=docker,rewrite-timestamp=true`; the two tar
  files must be byte-identical.
- Tag scheme: `mejepa-paper-v1.0.0` matching the Zenodo DOI tag.

### Apptainer (HPC-facing, primary for paper artifact)

- Built **from** the Docker image via `apptainer build` so the two images are
  bit-equivalent at the binary level (only the layout differs).
- Run with `--cleanenv --no-home --nv` to maximize isolation.
- Distributed as `.sif` file alongside the bundle DOI archive.

Why Apptainer: HPC reviewers cannot run rootful Docker. Apptainer is the
de-facto HPC standard. Docker is included for laptop reviewers.

### Build Determinism

- Pin every Rust dep via `Cargo.lock` (already standard).
- Pin the CUDA base image by digest, not tag (`nvidia/cuda@sha256:...`).
- No `apt update && apt install` without `--no-install-recommends` and a
  pinned snapshot date (`http://snapshot.debian.org/...` or equivalent).
- Build inside the container, not on the host, so nothing leaks from the
  developer machine.
- Record the resulting Docker image digest + Apptainer SIF SHA-256 in the
  release manifest.

## Artifact Layout (Zenodo Archive)

One Zenodo record, one DOI:

```text
mejepa_5090_artifact_v1.0.0/
  README.md                    # one-page reproduction instructions
  CITATION.cff                 # how to cite
  LICENSE                      # PolyForm Noncommercial 1.0.0 for root code
  THIRD_PARTY_LICENSES/        # bundled license texts (O*NET CC-BY, etc.)
  bin/
    context-graph              # local verifier binary for aggregate checks
  container/
    contextgraph-mejepa.sif    # Apptainer image
    container_manifest.json    # digest, build date, base image digest
  bundles/
    20260430T_paper_small_1k_seed42_v1/   # one passed bundle, full
    aggregate_paper_small_v1/             # multi-seed aggregate
  paper_tables_reference/      # the exact CSVs cited by the paper
  reference_hashes.json        # SHA-256 of every file in the archive
  reproduce.sh                 # the reviewer command
  CHANGELOG.md
```

A second Zenodo record archives the **paper repo** (LaTeX source + Makefile +
generator scripts). Cross-cite both DOIs.

## Reference Bundle Selection

The multi-seed aggregator runs five seeds. The **reference bundle** for the
artifact is the one whose `aggregate_manifest.json` SHA-256 matches the
published reference. To make the reference deterministic across rebuilds, the
aggregator must:

- order child bundle paths lexicographically before hashing;
- record `aggregate_random_state_seed` in the manifest;
- exclude any non-deterministic field (timestamps in human-readable form) from
  the hash input.

A wrapper command writes a stable `reference_hashes.json` summarizing the
critical hashes a reviewer should match:

```json
{
  "aggregate_manifest_sha256": "...",
  "table_prediction_metrics_multiseed_sha256": "...",
  "table_ablation_multiseed_sha256": "...",
  "table_paired_ablation_tests_sha256": "...",
  "container_sif_sha256": "...",
  "container_docker_digest": "sha256:..."
}
```

## ACM Artifact Badging Mapping

Three badges; each has explicit criteria. Map our artifact to each:

| Badge | Criterion | How We Meet It |
| --- | --- | --- |
| **Available** | Permanently archived, DOI, not personal page or GitHub-only | Zenodo DOI on both code and bundle archives |
| **Functional** | Documented, consistent, complete, exercisable | One-command reproduce, README, container, bundle layout matches paper |
| **Reusable** | Exceeds minimal functionality, well-documented for repurposing | Domain-pack abstraction, smokedocs 00-25, second taxonomy via `--taxonomy esco`, integration tests |

The artifact-evaluation packet for the venue includes:

- the DOI;
- a single PDF "Artifact Appendix" (≤2 pages) listing what the reviewer should
  see and the exact commands;
- a hardware-requirement statement: "RTX 5090 (sm_120) preferred; runs on Ada
  Lovelace sm_89 with documented metric variance" (if doc 22's HP sweep
  confirms cross-gen behavior).

## NeurIPS Reproducibility Checklist Mapping

Mandatory; included in the LaTeX style file. Pre-fill answers from existing
artifacts:

| Question | Answer Source |
| --- | --- |
| Code released? | Zenodo DOI |
| Hardware listed? | `hardware.json` per bundle, doc 09 §Preflight |
| Software listed? | `environment.json` per bundle |
| Random seeds? | `run_manifest.json` per bundle |
| Hyperparameters? | `training_metrics.json` per bundle, doc 22 sweep |
| Datasets + licenses? | `data_manifest.json` per bundle, doc 18 |
| Computational budget? | `paper_tables/table_runtime.csv`, doc 02, doc 17 |
| Limitations? | Smokedoc 11, paper §8 (doc 19) |
| Broader impact? | Doc 21 |
| Risks of misuse? | Doc 21 §Dual-Use Disclosure |

The checklist is a **filing chore**, not a research task. Block ~2 hours for
final pass.

## NIST AI RMF Mapping

The full doc-21 mapping (GOVERN / MAP / MEASURE / MANAGE) is detailed there.
For the artifact specifically, ensure:

- Provenance recorded for every model and dataset (already done via bundle).
- Pre-deployment testing recorded (FSV is the testing record).
- Incident disclosure path: `failed_bundles/` directory + the seed-43 worked
  example.
- Third-party risk: container base image digest + dependency lockfile +
  recorded SHA of upstream JEPA / VICReg references (text only).

## Release Workflow

```text
git tag v1.0.0
make container       # builds Docker + Apptainer
make reference       # runs the multi-seed program, stores reference hashes
make zenodo          # uploads to Zenodo, returns DOI
make paper-supp      # builds the paper supplementary archive
make arxiv           # builds the arXiv submission tarball
```

`make reference` must run on the same machine the paper claims; running on a
different sm_120 / different driver version produces a slightly different
hash and is rejected.

## CI Integration (Coordinates With Doc 23)

- On every tagged release, CI rebuilds the container from scratch and asserts
  the Docker artifact tar SHA-256 is reproducible across two consecutive
  builds (binary nondeterminism is a release-blocker).
- On every PR that touches `Cargo.lock` or `Dockerfile`, CI rebuilds the
  container and runs the smoke ladder gate 0-2 inside it.
- A scheduled weekly job runs `reproduce.sh paper_small_1k --seeds 42` and
  asserts the `aggregate_manifest_sha256` matches the published reference.
  Drift fails the build and pages the maintainer.

## License + Attribution

- Code: PolyForm Noncommercial License 1.0.0 at the repository root unless a
  subtree has a more specific license boundary in `LICENSING.md`.
- Bundle data (generated walks, paper tables): CC-BY-4.0.
- Third-party data (O*NET, ESCO, BLS crosswalk) retain their original
  licenses; full text bundled in `THIRD_PARTY_LICENSES/`.
- O*NET attribution language must be visible in:
  - the container build log;
  - the README;
  - every paper figure that uses O*NET data;
  - `data_manifest.json` (already required by smokedoc 03).

The O*NET non-endorsement clause is reproduced verbatim in the README.

## Edge Cases (Fail-Closed)

- Container build with a Docker image tag (not digest): release script aborts.
- Zenodo upload returns a DOI but the deposit is private: release script
  blocks until the deposit is published and the DOI resolves.
- Reference hash differs across two consecutive `make reference` runs on the
  same machine: release script aborts; root-cause investigation required.
- A reviewer's reproduce hash differs from the reference: the diff_report
  must name CSV cells, not just say "mismatch".

## Current Local Implementation Status

Verified on 2026-05-01:

- `Dockerfile` exists and pins CUDA 13.2 Ubuntu 24.04 by digest:
  `sha256:d266e59b88c295bc5fa0e4cef9064eaff84939381b09e2c3d76a5532a303e42d`.
- `reproduce.sh verify-aggregate` uses the compiled `context-graph`
  `dynamicjepa check-release-aggregate-reference` command, not a host-only
  Ruby verifier.
- The CUDA-linked binary requires the NVIDIA container runtime; release-C
  runs container verification with `docker run --gpus all`.
- Latest deterministic Docker artifact proof:
  `tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_docker_image_1.tar`
  and `_image_2.tar` both hash to
  `099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e`.
- Latest release reference SHA-256:
  `f5212d7fbf1da4df0f2dbbf6e520d4a97bc520f3e8d9a4dba1c113b73dd5c614`.
- `scripts/5090jepa/prepare_release_candidate_archive.sh` builds a local,
  no-upload release-candidate archive from preserved release-C evidence and
  fails closed if the aggregate, FSV report, Docker tars, container verifier,
  selected full seed bundle, reference, citation, or license files are invalid.
- Latest local release-candidate archive:
  `tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2.tar.gz`.
  Archive SHA-256:
  `eaa3c8cf72524a1e3746bb17d6cf4deb453f2e632a6404f4281efd03b854e85f`.
- The rc2 archive embeds `bin/context-graph`, records its dynamic-link
  readback in `manifests/context_graph_ldd.txt`, and passes packaged
  `CONTEXT_GRAPH_BIN=bin/context-graph ./reproduce.sh verify-aggregate`.
- Apptainer/Singularity is not installed on the current host. The script can
  include a supplied `.sif` and can be forced to fail with
  `--require-apptainer`, but it does not synthesize or fake an HPC image.
- Zenodo upload/DOI publication has not been performed from this environment.
  The release-candidate archive is local evidence, not the final DOI artifact.

## Acceptance

This work is done when:

- the container builds reproducibly (twice = same digest) on the developer
  machine and in CI;
- a Zenodo DOI exists for the artifact archive and the paper repo;
- ACM Artifact Appendix PDF is drafted and ≤2 pages;
- `reproduce.sh paper_small_1k --seeds 42` matches the reference hash on a
  clean machine;
- the NeurIPS checklist is complete with bundle paths in every Yes answer.
