# 2026-04-30 Career Seed-Control Patch Report

## Problem

The smoke docs required at least three paper-small runs for paper-quality
multi-seed evidence. The existing `research-smoke` CLI generated strict career
training configs, and those configs already had a `seed` field, but the CLI did
not expose a seed override.

Running multiple "seed" bundles without an actual seed parameter would have
been misleading. It would compare repeated runs with the same training seed
instead of true seed variation.

## Root Cause

- `TrainConfig` has a strict `seed: u64` field.
- `research-smoke` generated O*NET career configs through
  `write_onet_career_train_config`.
- No CLI option passed a seed into that generated config.
- Ablations reused the same generated career config, so varying the main career
  seed also varied every ablation unless separated.

## Changes

Files changed:

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-cli/tests/dynamicjepa_research_smoke_fsv.rs`

Implemented:

- Added `--career-training-seed`.
- Added `--career-ablation-training-seed`.
- Both seed overrides fail closed unless `--onet-text-zip` is supplied.
- `run_manifest.json` records both seed fields.
- Generated O*NET career train config records `--career-training-seed`.
- Ablation train configs default to the checked-in career config seed when
  `--career-training-seed` is supplied and `--career-ablation-training-seed` is
  omitted.
- Ablations can still be explicitly varied with
  `--career-ablation-training-seed`.

## Validation

Build checks:

```bash
cargo fmt
cargo check -p context-graph-cli --all-features
cargo build -p context-graph-cli --bin context-graph --all-features
cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
```

All completed successfully. The integration test result was 5 passed, 0 failed,
finished in 302.98 seconds.

Manual fail-closed seed check:

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_seed_without_onet_manual \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --career-training-seed 43 \
  --json
```

Observed:

- Error code: `VALIDATION`
- Field: `research_smoke.career_training_seed`
- No bundle directory was created.

Final bundle validation:

| Run ID | Main Seed | A0 Ablation Seed | Status |
| --- | ---: | ---: | --- |
| `20260430T_paper_small_1k_seed42_v2` | 42 | 42 | passed |
| `20260430T_paper_small_1k_seed43_v2` | 43 | 42 | passed |
| `20260430T_paper_small_1k_seed44_v1` | 44 | 42 | passed |

The seed 43 rerun is the key proof. The earlier
`20260430T_paper_small_1k_seed43_v1` failed when the main seed override also
changed A0. After separating main career seed and ablation control seed,
`20260430T_paper_small_1k_seed43_v2` passed while recording `main_seed=43` and
`a0_seed=42` in the generated train configs.

## Learning

Multi-seed evidence needs a real source-of-truth seed field in the generated
train config and run manifest. A command-line label alone is not evidence.
