# 09 - Runbook And Iteration Loop

This runbook is intentionally staged. Do not start the full paper run until the
smoke bundle passes and the generated tables look coherent.

## Preflight

```bash
cd /home/cabdru/contextgraph
export CUDA_COMPUTE_CAP=120
export CUDA_HOME=/usr/local/cuda-13.2
export PATH="$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"
export CONTEXT_GRAPH_ENV=development
export RUST_LOG=info,context_graph_dynamicjepa=debug
export ONET_TEXT_ZIP="tmp/onet_probe/db_30_2_text.zip"
```

Build:

```bash
cargo build -p context-graph-cli --bin context-graph --all-features
```

## Smoke Paths

```bash
export RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)_mejepa_5090_smoke"
export RUN_ROOT="tmp/mejepa_research_runs"
```

Use a fresh run ID. The research-smoke bundle is immutable and will fail if the
run directory already exists.

## One-Command Research Smoke

```bash
./target/debug/context-graph dynamicjepa research-smoke \
  --run-id "$RUN_ID" \
  --run-root "$RUN_ROOT" \
  --fixtures-root configs/dynamicjepa \
  --onet-text-zip "$ONET_TEXT_ZIP" \
  --system-specs docs2/systemspecs.md \
  --json
```

This command runs environment capture, git state capture, counter_world,
gridworld_5x5, O*NET-derived career_taxonomy, career same-seed
reproducibility, and the full career A0-A5 ablation matrix. It writes all
source-of-truth evidence into `tmp/mejepa_research_runs/<run_id>/`.

Expected top-level files:

- `run_manifest.json`;
- `environment.json`;
- `hardware.json`;
- `git_state.json`;
- `data_manifest.json`;
- `domain_pack_manifest.json`;
- `command_log.jsonl`;
- `fsv_report.json`;
- `db_counts_before.json`;
- `db_counts_after.json`;
- `training_metrics.json`;
- `evaluation_report.json`;
- `ablation_results.json`;
- `edge_case_results.json`;
- `reproducibility.json`;
- `paper_tables/*.csv`.
- `plots/*.svg`.
- `generated_fixtures/onet_30_2/*`.

## Manual Spot Checks

After the command passes:

```bash
jq '{status, run_id, db_path, artifact_root}' \
  "tmp/mejepa_research_runs/$RUN_ID/run_manifest.json"

jq '{status, readback_counts:(.readbacks|to_entries|map({domain:.key, verification_runs:.value.after_counts.dj_verification_runs, model_artifacts:.value.after_counts.dj_model_artifacts, predictions:.value.after_counts.dj_predictions}))}' \
  "tmp/mejepa_research_runs/$RUN_ID/fsv_report.json"

./target/debug/context-graph dynamicjepa inspect-counts \
  --db "tmp/mejepa_research_runs/$RUN_ID/dbs/${RUN_ID}_career_taxonomy_rocksdb" \
  --json

jq '{source:.sources[0], local_fixture_sha256, source_manifest_sha256}' \
  "tmp/mejepa_research_runs/$RUN_ID/data_manifest.json"
```

Do not run multiple `inspect-cf` commands against the same RocksDB path in
parallel. RocksDB lock failures are expected and useful diagnostics, but manual
source-of-truth inspection should be sequential for one DB.

## Gate 2 - Existing Runtime Controls

The one-command harness runs these controls. Use the explicit commands below
only when isolating a failing gate:

```bash
./target/debug/context-graph dynamicjepa verify-counter-world \
  --db "/tmp/mejepa-counter-debug-$RUN_ID" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "/tmp/mejepa-counter-artifacts-$RUN_ID" \
  --evidence-root "tmp/mejepa_research_runs/${RUN_ID}_counter_debug" \
  --json

./target/debug/context-graph dynamicjepa verify-gridworld \
  --db "/tmp/mejepa-grid-debug-$RUN_ID" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "/tmp/mejepa-grid-artifacts-$RUN_ID" \
  --evidence-root "tmp/mejepa_research_runs/${RUN_ID}_grid_debug" \
  --json
```

For a later universality run, use one shared DB. For smoke iteration, separate
DBs reduce lock and cleanup risk.

## Gate 3+ - Career Taxonomy Debug Path

The career taxonomy verifier is implemented. Use it directly only when
debugging the career slice outside the consolidated bundle:

```bash
./target/debug/context-graph dynamicjepa verify-career-taxonomy \
  --db "/tmp/mejepa-career-debug-$RUN_ID" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "/tmp/mejepa-career-artifacts-$RUN_ID" \
  --evidence-root "tmp/mejepa_research_runs/${RUN_ID}_career_debug" \
  --json
```

## Iteration Policy

When a gate fails:

1. Stop the run.
2. Read the failing command log.
3. Inspect source-of-truth counts.
4. Decode the relevant CF rows.
5. Fix the lowest-level root cause.
6. Rerun from a fresh DB.
7. Keep the failed bundle; do not overwrite it.

## Full Paper Run Promotion

Promote only when:

- smoke gates 0-9 pass;
- data manifest is complete;
- all generated tables trace to bundle paths;
- edge cases pass;
- no paper claim depends on unavailable real recruiting data.
