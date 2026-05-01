# 23 - CI Smoke Regression And Drift Detection

## Goal

Defend the DynamicJEPA artifact and paper-demo numbers from silent drift with
a free, local-first gate. The canonical gate is
`scripts/5090jepa/local_ci_gate.sh`; GitHub workflow files are only thin
self-hosted wrappers around that script.

The gate must fail closed. It must not replace CUDA work with CPU-only work,
must not use mock O*NET data, must not overwrite evidence bundles, and must not
hide broken behavior behind tolerant tests.

## Cost Boundary

The CI gate is designed to be free:

- no GitHub-hosted runners;
- no larger hosted runners;
- no `actions/upload-artifact`;
- no `actions/cache`;
- no Codecov or other third-party reporting service;
- no automatic issue creation or external API calls;
- evidence stays on local disk under `tmp/mejepa_local_ci_runs`.

GitHub's billing documentation says Actions usage is free for self-hosted
runners, while hosted minutes, artifact storage, and cache storage can consume
quota or be billed. This project therefore keeps DynamicJEPA evidence local and
uses self-hosted runner labels only when GitHub is used as a trigger.

Reference: https://docs.github.com/en/billing/concepts/product-billing/github-actions

## Source Of Truth

For every local gate run, the source of truth is the generated local evidence:

```text
tmp/mejepa_local_ci_runs/<run_id>_local_ci_manifest.json
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/run_manifest.json
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/fsv_report.json
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/artifact_hashes.json
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/data_manifest.json
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/dbs/*_rocksdb/
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/paper_tables/*.csv
tmp/mejepa_local_ci_runs/<run_id>_smoke_a/plots/*.svg
tmp/mejepa_local_ci_runs/<run_id>_paper_small_seed42_{a,b}/
tmp/mejepa_local_ci_runs/<run_id>_hp_sweep_batch_size/
tmp/mejepa_local_ci_runs/<run_id>_hp_sweep_batch_size/sweep_manifest.json
tmp/mejepa_local_ci_runs/<run_id>_hp_sweep_batch_size/sweep_fsv_report.json
tmp/mejepa_local_ci_runs/<run_id>_paper_small_seed{42,43,44,45,46}/
tmp/mejepa_local_ci_runs/<run_id>_paper_small_multiseed_aggregate/
tmp/mejepa_local_ci_runs/<run_id>_docker_image_{1,2}.tar
tmp/mejepa_local_ci_runs/<run_id>_docker_container_verify_aggregate.json
```

Return values are not sufficient. The gate writes the bundle, then reads the
bundle back with separate checks:

- `run_manifest.json` status must be `passed`;
- `fsv_report.json` status must be `passed`;
- career RocksDB must contain persisted rows for verification runs and model
  artifacts;
- `artifact_hashes.json` must show every registry hash equals the recomputed
  file hash inside the bundle;
- `check-reference` must report no structural drift and no out-of-tolerance
  numeric drift;
- tables and plots must exist on disk.

## Three Local Tiers

### Tier A - `smoke-a`

Command:

```bash
LOCAL_CI_RUN_ID_PREFIX=<unique_id> scripts/5090jepa/local_ci_gate.sh smoke-a
```

Runs:

- CUDA and O*NET preflight;
- free-workflow scan;
- `cargo fmt --all -- --check`;
- `cargo check -p context-graph-cli --all-features`;
- `cargo clippy -p context-graph-cli --all-targets --all-features -- -D warnings`;
- workflow YAML parsing;
- `cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv
  --all-features -- --nocapture`;
- `dynamicjepa research-smoke --purpose smoke --career-event-count 120`
  against the verified O*NET 30.2 text ZIP;
- RocksDB `inspect-counts`;
- artifact registry hash readback;
- reference comparison against `reference/reference_smoke_hashes.json` using
  `reference/tolerances.json`.

Pass requires all static checks, the FSV integration suite, bundle generation,
database readback, artifact hash readback, and reference comparison to pass.

### Tier B - `smoke-b`

Command:

```bash
LOCAL_CI_RUN_ID_PREFIX=<unique_id> scripts/5090jepa/local_ci_gate.sh smoke-b
```

Runs Tier A, then:

- two real O*NET `paper_small` seed-42 runs with `--career-event-count 1000`;
- per-bundle manifest, FSV, RocksDB, artifact-hash, and paper-small reference
  checks;
- same-seed cross-run reference comparison by capturing run A as a normalized
  reference and checking run B against it with the same tolerance schema;
- reduced real O*NET HP sweep for `batch_size=16,32` and seeds `42,43`
  at `--career-event-count 1000`.

Same-seed comparison is table/structure/tolerance based. Exact CUDA model file
bytes are not a pass/fail criterion because GPU training can produce small
numeric differences even when seeds are fixed. PyTorch's reproducibility notes
make the same general point: completely reproducible results are not guaranteed
across releases, platforms, or CPU/GPU execution.

Reference: https://docs.pytorch.org/docs/2.11/notes/randomness.html

The batch-size sweep preserves the base optimizer-step budget. With the current
base config (`batch_size=16`, `epochs=1000`) and 1K O*NET career events,
`batch_size=32` cells persist `epochs=2000`; the sweep manifest and generated
tables include the requested and persisted epoch count.

### Tier C - `release-c`

Command:

```bash
LOCAL_CI_RUN_ID_PREFIX=<unique_id> scripts/5090jepa/local_ci_gate.sh release-c
```

Runs the release matrix:

- five real O*NET paper-small seeds: `42,43,44,45,46`;
- preserved bundle list even if a seed fails;
- `dynamicjepa multiseed-aggregate` with `--min-passed 5`;
- release aggregate reference comparison with
  `dynamicjepa check-release-aggregate-reference`;
- deterministic Docker artifact export twice with `SOURCE_DATE_EPOCH` and
  `rewrite-timestamp=true`;
- container load + `docker run --gpus all ... verify-aggregate` against the
  real aggregate source of truth;
- local artifact-evaluation dry run.

Tier C is expected to be longer than Tier A and Tier B. It still keeps evidence
local and free.

## Reference Files

Checked-in reference files:

```text
reference/reference_smoke_hashes.json
reference/reference_paper_small_v1.json
reference/reference_release_v1.0.0.json
reference/tolerances.json
```

Reference updates require an explicit bundle-bump review. The new reference
must be generated from a preserved bundle and committed with the code change
that explains why drift is expected.

## Drift Detection

`dynamicjepa check-reference` compares bundle tables and required files against
a reference:

- `bit_equal`: table content is byte-identical where exact equality is expected;
- `within_tolerance`: all recognized numeric metric deltas are within the
  checked-in tolerance schema;
- `out_of_tolerance`: at least one recognized metric exceeds tolerance and the
  gate fails;
- `REFERENCE_TOLERANCE_MISSING`: a known numeric metric column has no tolerance
  entry and the gate fails;
- structural drift: missing rows, missing columns, or unexpected non-numeric
  changes fail.

The tolerance file now covers all known numeric metrics emitted by
`table_prediction_metrics.csv` and `table_ablation.csv`, including
`random_init_cosine`, `shuffled_target_cosine`, and
`val_action_contrast_acc`.

## Artifact Integrity

Artifact integrity means the bundle's artifact registry matches the physical
files in that same bundle. The gate enforces:

```bash
jq -e '[.[].artifact_hashes[] | select(.equal != true)] | length == 0' \
  <bundle>/artifact_hashes.json
```

Cross-run exact hashes for generated reports, manifests, and CUDA model files
are diagnostic evidence, not the primary pass/fail rule. The primary regression
rule is: no structural drift, no missing tolerance for numeric metrics, no
out-of-tolerance numeric drift, and no in-bundle artifact hash mismatch.

## Root Causes Fixed

The current local gate fixes the issues found during integration:

- stale smoke reference did not match the expanded smoke bundle rows;
- `random_init_cosine` and `shuffled_target_cosine` were numeric metrics but
  had no tolerance entries;
- missing numeric tolerances now fail with `REFERENCE_TOLERANCE_MISSING`;
- career negative sampling depended on run-local UUIDs, so same real data and
  same seed could select different negative panels;
- negative panel ordering and selection now use stable panel feature hashes and
  outcome-signature hashes instead of run-local UUIDs;
- Tier B no longer uses exact model artifact byte equality as the same-seed
  CUDA pass/fail gate.
- read-only inspection commands no longer create missing RocksDB paths;
  `inspect-counts` and `inspect-cf` open existing stores only, while the
  internal smoke verifier uses explicit `--allow-missing` before-state reads;
- the Tier B HP sweep no longer uses the 120-event proxy that failed the
  `batch_size=32` transition ablation; it uses the verified 1K real O*NET
  scale and records the scaled epoch budget in child bundles, sweep manifests,
  and sweep tables;
- the top-level CLI enum boxes the DynamicJEPA subcommand so `cargo clippy
  --all-targets -D warnings` remains part of the free local gate.
- release-C now has checked-in `Dockerfile` and executable `reproduce.sh`
  artifacts and fails closed if either is missing;
- `reproduce.sh verify-aggregate` now calls the compiled Rust verifier instead
  of requiring Ruby inside the CUDA container;
- Docker reproducibility is checked on deterministic tar exports because
  local Docker daemon unpacking conflicts with BuildKit timestamp rewriting;
- CUDA container verification uses `--gpus all`, which is required for the
  host driver library (`libcuda.so.1`) to be mounted into the container;
- the release reference was recaptured from a preserved passed five-seed
  aggregate after the old reference was proven stale for the current
  implementation's counter-world auxiliary action-contrast row.

## Edge Cases

The local gate must fail closed for:

- unsupported tier;
- missing O*NET ZIP;
- existing bundle path;
- CPU-only runner;
- corrupted reference JSON;
- missing table or plot;
- missing numeric tolerance;
- artifact registry hash mismatch;
- out-of-tolerance metric drift.

Failure evidence must remain on disk when a bundle has already been created.
The gate must never delete and recreate an evidence bundle for the same run id.

## Current Verified Run

Latest Tier B proof from this integration pass:

```text
run id: 20260501T_local_smoke_b_free_gate_v5
manifest: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_local_ci_manifest.json
smoke bundle: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_smoke_a
paper-small bundles:
  tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_paper_small_seed42_a
  tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_paper_small_seed42_b
hp sweep bundle: tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v5_hp_sweep_batch_size
status: passed
system specs SHA-256: acdc549a5913c59f46376414c08373a8363d646fd3c36d1fc6289a193c0e1205
```

The full findings and FSV evidence are recorded in:

```text
docs/5090jepa/smokedocs/reports/20260501_local_smoke_b_tier_fsv.md
```

Latest Tier C proof from this integration pass:

```text
run id: 20260501T_release_c_gate_v2
manifest: tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json
paper-small bundles:
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed43
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed44
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed45
  tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed46
aggregate: tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate
status: passed
docker artifact SHA-256: 099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e
release reference SHA-256: f5212d7fbf1da4df0f2dbbf6e520d4a97bc520f3e8d9a4dba1c113b73dd5c614
```

The full findings and FSV evidence are recorded in:

```text
docs/5090jepa/smokedocs/reports/20260501_release_c_repro_artifact_fsv.md
```

Latest local release-candidate archive proof:

```text
release id: mejepa_5090_artifact_v1.0.0_rc2
artifact dir: tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2
archive: tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc2.tar.gz
archive SHA-256: eaa3c8cf72524a1e3746bb17d6cf4deb453f2e632a6404f4281efd03b854e85f
embedded verifier SHA-256: 41560d160acdaade55f16153678db373c33127ce7a12b4eb5a418d30e86ec0eb
docker tar SHA-256: 099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e
packaged verify-aggregate: passed with diff_count=0
blocked final-release items: Apptainer SIF not included, Zenodo DOI not published
```

## Acceptance

This work is accepted when:

- `.github/workflows/{ci,ci_smoke_a,ci_smoke_b,ci_smoke_c}.yml` use only
  self-hosted/local execution for DynamicJEPA;
- the free-service workflow scan returns no matches;
- `scripts/5090jepa/local_ci_gate.sh smoke-b` passes on the local RTX 5090;
- the source-of-truth bundle is physically inspected after execution;
- at least three fail-closed edge cases are manually verified with before and
  after state;
- reports are written under `docs/5090jepa/smokedocs/reports/`.
