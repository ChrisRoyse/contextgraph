# 03 - Research Bundle Schema

Every smoke and full paper run writes one immutable research bundle:

```text
tmp/mejepa_research_runs/<run_id>/
  run_manifest.json
  environment.json
  git_state.json
  hardware.json
  data_manifest.json
  domain_pack_manifest.json
  command_log.jsonl
  fsv_report.json
  db_counts_before.json
  db_counts_after.json
  decoded_records/
  artifact_hashes.json
  training_metrics.json
  evaluation_report.json
  ablation_results.json
  edge_case_results.json
  reproducibility.json
  plots/
  paper_tables/
```

The bundle is the paper source of truth. Terminal output is not evidence.

Implemented smoke entry point:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
  --run-id 20260430T_onet30_demo_fsv_v11 \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

The command fails if `<run_id>` already exists, if the fixture/provenance files
are incomplete, if CUDA/5090 environment checks fail, if any verifier fails, if
independent `inspect-counts` readback differs from verifier output, or if paper
tables cannot be written.

## `run_manifest.json`

Required fields:

```json
{
  "run_id": "...",
  "created_at_unix_ms": 0,
  "purpose": "smoke|paper_small|paper_full",
  "operator": "local",
  "repo": "/home/cabdru/contextgraph",
  "db_path": "/tmp/mejepa-5090-smoke-...",
  "artifact_root": "/tmp/mejepa-5090-artifacts-...",
  "evidence_root": "tmp/mejepa_research_runs/<run_id>",
  "fixtures_root": "configs/dynamicjepa",
  "career_fixtures_root": "tmp/mejepa_research_runs/<run_id>/generated_fixtures/onet_30_2",
  "onet_text_zip": "tmp/onet_probe/db_30_2_text.zip",
  "career_event_count": 120,
  "system_specs": "docs2/systemspecs.md",
  "status": "running|passed|failed",
  "failure_details": null
}
```

## `environment.json`

Capture:

- `nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total`;
- `nvcc --version`;
- `CUDA_COMPUTE_CAP`;
- `CUDA_HOME`;
- `LD_LIBRARY_PATH` summary;
- `rustc --version`;
- `cargo --version`;
- `cargo tree -p candle-core`;
- OS/kernel;
- CPU/RAM.
- `docs2/systemspecs.md` path and SHA-256.

## `git_state.json`

Capture:

- `git rev-parse HEAD`;
- `git status --short`;
- diff hash of tracked changes;
- list of untracked files relevant to the run;
- whether the run used a dirty tree.

Dirty tree runs are allowed for iteration but paper tables must label them.

## `data_manifest.json`

Capture one object per data source:

```json
{
  "name": "onet_30_2",
  "source_url": "https://www.onetcenter.org/database.html",
  "license": "Creative Commons, exact text/hash recorded separately",
  "local_path": "...",
  "sha256": "...",
  "row_count": 0,
  "allowed_use": "research-demo",
  "provenance_notes": "..."
}
```

If a source license or provenance is missing, the smoke harness fails closed.
The current O*NET-backed smoke manifest records `row_count: 120`, the source
ZIP SHA-256, generated JSONL and walk CSV hashes, selected occupation/edge
counts, allowed use, and the O*NET attribution/non-endorsement language.

The bundle also writes generated real-source fixtures:

```text
generated_fixtures/onet_30_2/
  verification/career_taxonomy_smoke.jsonl
  verification/career_taxonomy_onet_walk.csv
  verification/career_taxonomy_train_config.json
  verification/career_taxonomy_data_manifest.json
  domain_packs/career_taxonomy.v1.toml
```

## `domain_pack_manifest.json`

Capture:

- domain pack file paths;
- SHA-256 hashes;
- registered domain pack IDs and versions;
- instrument count;
- adapter count;
- objective count;
- verification policy;
- decoded persisted domain rows.

## `command_log.jsonl`

One JSON object per command:

```json
{
  "index": 0,
  "name": "verify_gridworld",
  "argv": ["./target/debug/context-graph", "dynamicjepa", "..."],
  "started_at_unix_ms": 0,
  "finished_at_unix_ms": 0,
  "elapsed_ms": 0,
  "exit_code": 0,
  "stdout_sha256": "...",
  "stderr_sha256": "...",
  "stdout_tail": [],
  "stderr_tail": []
}
```

Do not store huge stdout blobs inline. Store hashes and tails; write full command
stdout/stderr files if needed.

## `fsv_report.json`

Top-level shape:

```json
{
  "source_of_truth": {
    "rocksdb": "...",
    "column_families": [],
    "artifact_root": "...",
    "evidence_root": "..."
  },
  "before": {},
  "after": {},
  "readbacks": {},
  "edge_cases": [],
  "artifact_hash_checks": [],
  "verification_run_ids": [],
  "status": "passed|failed"
}
```

Every mutating gate must name its source-of-truth CFs and include independent
readback evidence.

## `training_metrics.json`

Capture:

- config hash;
- seed;
- dataset IDs;
- split row counts;
- per-epoch train/val loss;
- latent MSE/cosine by split;
- action-contrast accuracy;
- random-init baseline;
- shuffled-target baseline;
- VICReg variance/covariance diagnostics;
- parameter count;
- wall time;
- peak VRAM.

## `evaluation_report.json`

Capture paper-facing metrics:

- next-state prediction metrics;
- top-k occupation/skill metrics where applicable;
- planning/surprise outcomes;
- held-out split metrics;
- calibration summaries;
- confidence intervals if repeated runs exist.

## `ablation_results.json`

Each row includes:

- ablation ID;
- panel configuration;
- panel slots and input/target/action dimensions;
- objective configuration;
- parameter count;
- latent metrics and baselines;
- artifact ID/hash;
- source verification run ID.
- DB count readback and source-of-truth paths.

## `paper_tables/`

Generated CSV/Markdown:

- `table_runtime.csv`;
- `table_prediction_metrics.csv`;
- `table_ablation.csv`;
- `table_auditability.csv`;
- `table_edge_cases.csv`;
- `table_data_sources.csv`;
- `table_domain_runtime_coverage.csv`;
- `table_single_gpu_feasibility.csv`.

The paper should cite tables from this directory, not hand-copied numbers.

## `plots/`

The smoke harness renders deterministic SVG plots from generated table data:

- `prediction_val_mse.svg`;
- `career_ablation_val_mse.svg`;
- `plot_manifest.json`.

Publication styling can be revised later, but the smoke bundle must contain
renderable plot artifacts traced to table sources.
