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
```

Build:

```bash
cargo build -p context-graph-cli --bin context-graph --all-features
```

## Smoke Paths

```bash
export RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)_mejepa_5090_smoke"
export RUN_ROOT="tmp/mejepa_research_runs/$RUN_ID"
export DB="/tmp/mejepa-5090-smoke-$RUN_ID"
export ARTIFACT_ROOT="/tmp/mejepa-5090-artifacts-$RUN_ID"
rm -rf "$DB" "$ARTIFACT_ROOT" "$RUN_ROOT"
mkdir -p "$RUN_ROOT" "$ARTIFACT_ROOT"
```

## Gate 0/1 - Environment And Storage

```bash
nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader
nvcc --version
rustc --version
cargo --version
./target/debug/context-graph dynamicjepa inspect-counts --db "$DB" --json
```

Write outputs into `environment.json`, `hardware.json`, and
`db_counts_before.json`.

## Gate 2 - Existing Runtime Controls

```bash
./target/debug/context-graph dynamicjepa verify-counter-world \
  --db "$DB-counter" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "$ARTIFACT_ROOT/counter" \
  --evidence-root "$RUN_ROOT/control_counter" \
  --json

./target/debug/context-graph dynamicjepa verify-gridworld \
  --db "$DB-grid" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "$ARTIFACT_ROOT/gridworld" \
  --evidence-root "$RUN_ROOT/control_gridworld" \
  --json
```

For a later universality run, use one shared DB. For smoke iteration, separate
DBs reduce lock and cleanup risk.

## Gate 3+ - Career Taxonomy

Planned commands after the career domain pack and fixture generator are added:

```bash
./target/debug/context-graph dynamicjepa register-domain \
  --db "$DB-career" \
  --file configs/dynamicjepa/domain_packs/career_taxonomy.v1.toml \
  --json

./target/debug/context-graph dynamicjepa ingest-event \
  --db "$DB-career" \
  --domain career_taxonomy \
  --adapter json_event \
  --file "$RUN_ROOT/career_taxonomy_smoke.jsonl" \
  --json

./target/debug/context-graph dynamicjepa materialize-panel \
  --db "$DB-career" \
  --all-pending \
  --domain career_taxonomy \
  --json

./target/debug/context-graph dynamicjepa compile-trajectories \
  --db "$DB-career" \
  --domain career_taxonomy \
  --policy by_domain_session \
  --json
```

Then compile train/val/test, train, verify artifact, predict, plan, and record
surprise using the same pattern as Phase 10.

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

