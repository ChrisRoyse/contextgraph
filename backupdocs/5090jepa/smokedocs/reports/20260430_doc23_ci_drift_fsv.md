# 2026-04-30 Doc 23 CI Drift FSV Report

## Scope

Implemented smokedoc 23 first: CI smoke regression and drift detection for
DynamicJEPA research-smoke bundles.

Primary docs reviewed for implementation choices:

- GitHub Actions workflow syntax: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- GitHub Actions artifacts: https://docs.github.com/en/actions/how-tos/writing-workflows/choosing-what-your-workflow-does/storing-and-sharing-data-from-a-workflow
- Rust Clippy CI guidance: https://doc.rust-lang.org/clippy/continuous_integration/index.html

## Source Of Truth

The CI drift source of truth is the checked-in reference directory:

- `reference/reference_smoke_hashes.json`
- `reference/reference_paper_small_v1.json`
- `reference/reference_release_v1.0.0.json`
- `reference/tolerances.json`

The physical bundle source of truth for paper-small validation is:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/`
- Career RocksDB:
  `tmp/mejepa_research_runs/20260430T_paper_small_1k_v1/dbs/20260430T_paper_small_1k_v1_career_taxonomy_rocksdb`

## Implemented

- Added `dynamicjepa capture-reference`.
- Added `dynamicjepa check-reference`.
- Added checked-in reference/tolerance JSONs.
- Added three workflow files:
  - `.github/workflows/ci_smoke_a.yml`
  - `.github/workflows/ci_smoke_b.yml`
  - `.github/workflows/ci_smoke_c.yml`
- Updated `.gitignore` so the smoke workflows are not silently ignored.
- Updated doc 09 iteration policy to point to doc 23 and `check-reference`.
- Made the clippy gate pass without suppressing warnings by fixing current
  toolchain lints in dependent crates/tests.

Tier B and C include fail-closed steps for not-yet-implemented later docs:
`dynamicjepa hp-sweep`, `dynamicjepa multiseed-aggregate`, `reproduce.sh`, and
artifact-evaluation dry run. These steps intentionally fail until the required
features are implemented; they are not mocked.

## Happy Path FSV

Command:

```bash
./target/debug/context-graph dynamicjepa check-reference \
  --bundle tmp/mejepa_research_runs/20260430T_paper_small_1k_v1 \
  --reference reference/reference_paper_small_v1.json \
  --tolerances reference/tolerances.json \
  --require-artifact-hash-match \
  --json
```

Actual persisted state read after execution:

- status: `bit_equal`
- reference SHA: `c24308e5555742d2cc0cb7bc21ca15210360aeb92b86eb0e36a7bc354b6b5aba`
- tolerances SHA: `6449a1d0545d00536921a024e51b18b38dd985380d075de2fb69964c358c7ab3`
- file hash mismatches: `0`
- numeric diffs: `0`
- artifact hash rows: `45`
- career `val_latent_mse`: `0.14071947055420186`

Reference file readback:

- `reference_id`: `paper_small_1k_v1`
- bundle hash: `91c6f04ea2cf6f18fdda3e0ea80b4faee186d27f43679a36c935870e5e2071a6`
- required files: `10`
- prediction rows: `3`
- ablation rows: `6`
- edge rows: `11`

Career RocksDB physical readback:

- `dj_verification_runs`: `1`
- `dj_model_artifacts`: `1`
- `dj_training_runs`: `1`
- `dj_raw_events`: `1004`
- `dj_transitions`: `1000`

## Edge Case FSV

All edge cases were run manually with before-state inspection and persisted JSON
outputs under `tmp/doc23_fsv/`.

1. Corrupted reference JSON
   - Input: `tmp/doc23_fsv/corrupt/reference_bad.json`
   - Before SHA: `1584bcb7b2aa57a46715e4c75b203fb03774d4b1dc4b6e07b99ef65db7470640`
   - Exit code: `1`
   - Error code: `REFERENCE_CORRUPTED`
   - Proof: parser reported `key must be a string at line 1 column 3`.

2. Incomplete bundle
   - Input bundle: `tmp/doc23_fsv/incomplete_bundle`
   - Deleted file: `paper_tables/table_ablation.csv`
   - Exit code: `1`
   - Error code: `BUNDLE_INCOMPLETE`
   - Proof: error names the exact missing file path.

3. Out-of-tolerance metric drift
   - Input bundle: `tmp/doc23_fsv/out_of_tolerance_bundle`
   - Synthetic change: career `val_latent_mse`
     `0.14071947055420186 -> 0.6507194705542019`
   - Exit code: `1`
   - Error code: `OUT_OF_TOLERANCE`
   - Proof: absolute delta `0.51`, tolerance `0.005`.

4. Artifact hash drift
   - Input bundle: `tmp/doc23_fsv/artifact_drift_bundle`
   - Synthetic change: one artifact `size_bytes` value changed `3672 -> 3673`
   - Exit code: `1`
   - Error code: `ARTIFACT_HASH_DRIFT`
   - Proof: artifact mismatch count `1`.

## Verification Commands

Passed:

```bash
cargo check -p context-graph-cli --all-features
cargo build -p context-graph-cli --bin context-graph --all-features
cargo clippy -p context-graph-cli --all-targets --all-features -- -D warnings
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path); puts "ok #{path}" }' \
  .github/workflows/ci_smoke_a.yml \
  .github/workflows/ci_smoke_b.yml \
  .github/workflows/ci_smoke_c.yml
```

FSV test result:

- `5 passed; 0 failed; finished in 201.10s`

## Findings

- The project’s intended clippy gate was not passable on the current stable
  toolchain before this work. The root cause was new/expanded clippy lints
  across dependent crates and tests, plus a missing CLI test dev-dependency on
  `context-graph-mcp`.
- The new drift checker correctly fails closed for corrupted references,
  missing bundle artifacts, numerical drift, and artifact hash drift.
- `.github/` was globally ignored, which would have prevented new workflow
  files from being checked in. The ignore rule now permits the CI smoke
  workflows explicitly.
