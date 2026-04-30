# 15 — Runbook

End-to-end command sequence a fresh agent can follow to reproduce the 5090jepa demo from a clean tree. Every step is idempotent (or explicit about wiping). All commands assume the working directory is `/home/cabdru/contextgraph`.

## 1. One-Time Setup

```bash
cd /home/cabdru/contextgraph
git status --short
git log --oneline -3
nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv
nvcc --version | grep release
rustc --version
```

Confirm: branch `main`, RTX 5090 / driver 596+ / sm_120 / 32 GB VRAM, CUDA 13.2, rustc 1.95+.

```bash
mkdir -p tmp/5090jepa_evidence
echo "tmp/" >> .gitignore   # if not already present; verify before commit
```

## 2. Build

```bash
export CUDA_COMPUTE_CAP=120
export CUDA_HOME=/usr/local/cuda-13.2
export PATH="$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"
export CONTEXT_GRAPH_ENV=development
export RUST_LOG=info,context_graph_dynamicjepa=debug

cargo build --workspace --release --all-features
```

Cold build target ≤ 15 min on the operator's hardware. If it overruns, check `nvcc` is on PATH and `CUDA_COMPUTE_CAP=120` is exported (verify with `env | grep CUDA`).

## 3. Generate Gridworld Corpus (Once)

```bash
mkdir -p configs/dynamicjepa/verification
python3 scripts/5090jepa/generate_gridworld_corpus.py \
  configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl
sha256sum configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl
```

Record the hash; the test suite asserts it. If the hash drifts, revert the generator change.

## 4. Current Phase 7 Registry/Adapter/Panel/Trajectory/Dataset/Training Run

This is the implemented runbook today. It proves the Phase 3, Phase 4, and
Phase 5/6 surfaces: domain-pack registration, decoded CF inspection, strict
validation, JSONL ingestion, adapter success/failure, instrument readings,
latent panels, deterministic bindings, trajectories, and physical RocksDB
evidence. Phase 6 adds deterministic one-step dataset shards, decoded dataset
row inspection, and independent `ldb` count proof. Phase 7 adds CUDA-only
training, `dj_training_runs`, `dj_model_artifacts`, on-disk artifact hashes,
and corrupted-artifact verification.

```bash
# Phase 3 registry regression run; Phase 4 depends on these rows.
export DJ_DB=/tmp/5090jepa-phase3-fsv
rm -rf "$DJ_DB"
mkdir -p tmp/5090jepa_evidence/phase3/manual

./target/debug/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json \
  > tmp/5090jepa_evidence/phase3/manual/00_initial_counts.json

./target/debug/context-graph dynamicjepa register-domain --db "$DJ_DB" \
  --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json \
  > tmp/5090jepa_evidence/phase3/manual/01_register_counter_world.json

./target/debug/context-graph dynamicjepa register-domain --db "$DJ_DB" \
  --file configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml --json \
  > tmp/5090jepa_evidence/phase3/manual/02_register_gridworld.json

./target/debug/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json \
  > tmp/5090jepa_evidence/phase3/manual/03_counts_after_both.json

for cf in dj_domain_packs dj_domain_pack_by_nv dj_instrument_registry dj_adapter_registry dj_audit_log; do
  ./target/debug/context-graph dynamicjepa inspect-cf --db "$DJ_DB" --cf "$cf" --limit 10 --json \
    > "tmp/5090jepa_evidence/phase3/manual/inspect_${cf}.json"
done
```

Expected counts after both registrations:

```text
dj_domain_packs 2
dj_domain_pack_by_nv 2
dj_instrument_registry 5
dj_adapter_registry 2
dj_audit_log 2
```

Independent physical check:

```bash
for cf in dj_domain_packs dj_domain_pack_by_nv dj_instrument_registry dj_adapter_registry dj_audit_log; do
  /usr/bin/ldb --db="$DJ_DB" --column_family="$cf" dump --count_only | awk -v cf="$cf" '/Keys in range:/ {print cf, $4}'
done
```

Phase 3 edge cases to run before claiming the phase still works:

- Empty TOML registration fails and leaves counts unchanged.
- Duplicate instrument id fails and leaves counts unchanged.
- Unknown TOML key fails and leaves counts unchanged.
- Invalid `inspect-cf --cf` fails and leaves counts unchanged.

The automated equivalent is:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --all-features -- --nocapture
```

Phase 4 counter_world run:

```bash
export DJ_DB=/tmp/5090jepa-phase4-manual-cw
rm -rf "$DJ_DB"
mkdir -p tmp/5090jepa_evidence/phase4/manual_counter

./target/debug/context-graph dynamicjepa register-domain --db "$DJ_DB" \
  --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json

./target/debug/context-graph dynamicjepa ingest-event --db "$DJ_DB" \
  --domain counter_world --adapter json_event \
  --file configs/dynamicjepa/verification/counter_world_happy_path.jsonl --json

./target/debug/context-graph dynamicjepa materialize-panel --db "$DJ_DB" \
  --all-pending --domain counter_world --json

./target/debug/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Expected Phase 4 counter_world happy-path counts:

```text
dj_domain_packs 1
dj_domain_pack_by_nv 1
dj_instrument_registry 2
dj_adapter_registry 1
dj_raw_events 3
dj_adapter_runs 3
dj_normalized_states 3
dj_actions 3
dj_outcomes 3
dj_transitions 3
dj_instrument_readings 6
dj_latent_panels 3
dj_audit_log 10
```

Phase 4 gridworld one-trajectory run:

```bash
export DJ_DB=/tmp/5090jepa-phase4-manual-gw
rm -rf "$DJ_DB"
mkdir -p tmp/5090jepa_evidence/phase4/manual_gridworld
head -n 8 configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl \
  > tmp/5090jepa_evidence/phase4/manual_gridworld/gridworld_first_trajectory.jsonl

./target/debug/context-graph dynamicjepa register-domain --db "$DJ_DB" \
  --file configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml --json

./target/debug/context-graph dynamicjepa ingest-event --db "$DJ_DB" \
  --domain gridworld_5x5 --adapter json_event \
  --file tmp/5090jepa_evidence/phase4/manual_gridworld/gridworld_first_trajectory.jsonl --json

./target/debug/context-graph dynamicjepa materialize-panel --db "$DJ_DB" \
  --all-pending --domain gridworld_5x5 --json

./target/debug/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Expected Phase 4 gridworld happy-path counts:

```text
dj_domain_packs 1
dj_domain_pack_by_nv 1
dj_instrument_registry 3
dj_adapter_registry 1
dj_raw_events 8
dj_adapter_runs 8
dj_normalized_states 8
dj_actions 8
dj_outcomes 8
dj_transitions 8
dj_instrument_readings 24
dj_latent_panels 8
dj_audit_log 25
```

Independent physical check for either Phase 4 DB:

```bash
for cf in dj_raw_events dj_adapter_runs dj_normalized_states dj_actions dj_outcomes dj_transitions dj_instrument_readings dj_latent_panels dj_audit_log; do
  /usr/bin/ldb --db="$DJ_DB" --column_family="$cf" dump --count_only | awk -v cf="$cf" '/Keys in range:/ {print cf, $4}'
done
```

Phase 4 edge cases to run before claiming the phase still works:

- Empty JSONL input fails with `ADAPTER_FAILED`, leaves raw/adapter/normalized/readings/panels unchanged, and adds one failed audit row.
- Invalid action writes one raw event and one failed adapter run, writes no normalized/readings/panels, and adds two audit rows.
- Counter_world max outcome and gridworld out-of-bounds state write one raw event and one failed adapter run, write no normalized/readings/panels, and add two audit rows.

The automated equivalent is:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase4_fsv --all-features -- --nocapture
cargo test -p context-graph-cli --test dynamicjepa_phase5_fsv --all-features -- --nocapture
```

Phase 5 counter_world run:

```bash
export DJ_DB=/tmp/5090jepa-phase5-manual-cw
rm -rf "$DJ_DB"
mkdir -p tmp/5090jepa_evidence/phase5/manual_counter

./target/debug/context-graph dynamicjepa register-domain --db "$DJ_DB" \
  --file configs/dynamicjepa/domain_packs/counter_world.v1.toml --json

./target/debug/context-graph dynamicjepa ingest-event --db "$DJ_DB" \
  --domain counter_world --adapter json_event \
  --file configs/dynamicjepa/verification/counter_world_happy_path.jsonl --json

./target/debug/context-graph dynamicjepa materialize-panel --db "$DJ_DB" \
  --all-pending --domain counter_world --json

./target/debug/context-graph dynamicjepa compile-trajectories --db "$DJ_DB" \
  --domain counter_world --policy by_domain_session --json

./target/debug/context-graph dynamicjepa inspect-cf --db "$DJ_DB" \
  --cf dj_trajectories --limit 10 --json
```

Expected Phase 5 counter_world counts after trajectory compile:

```text
dj_raw_events 3
dj_transitions 3
dj_latent_panels 3
dj_trajectories 1
dj_audit_log 11
```

To verify binding, copy a `key_hex` from `dj_raw_events` and one from
`dj_trajectories`, then run:

```bash
./target/debug/context-graph dynamicjepa bind --db "$DJ_DB" \
  --left-cf dj_raw_events --left-key <raw_event_key_hex> \
  --right-cf dj_trajectories --right-key <trajectory_key_hex> \
  --method explicit_mapping --json

./target/debug/context-graph dynamicjepa inspect-cf --db "$DJ_DB" \
  --cf dj_bindings --limit 10 --json
```

Expected binding counts after the bind command:

```text
dj_bindings 1
dj_bindings_by_entity 1
dj_audit_log 12
```

Also verify `id_equality` by binding one decoded row to itself:

```bash
./target/debug/context-graph dynamicjepa bind --db "$DJ_DB" \
  --left-cf dj_raw_events --left-key <raw_event_key_hex> \
  --right-cf dj_raw_events --right-key <raw_event_key_hex> \
  --method id_equality --json
```

Expected cumulative binding counts after both methods:

```text
dj_bindings 2
dj_bindings_by_entity 2
dj_audit_log 13
```

Phase 5 gridworld one-trajectory run mirrors the counter run with the first
eight corpus rows. Expected counts after trajectory compile:

```text
dj_raw_events 8
dj_transitions 8
dj_latent_panels 8
dj_trajectories 1
dj_audit_log 26
```

Phase 5 edge cases to run before claiming the phase still works:

- Unsupported trajectory policy fails and leaves `dj_trajectories` and `dj_audit_log` unchanged.
- Duplicate trajectory compile fails and leaves `dj_trajectories` and `dj_audit_log` unchanged.
- Missing binding evidence fails with `BINDING_EVIDENCE_MISSING` and leaves `dj_bindings`, `dj_bindings_by_entity`, and `dj_audit_log` unchanged.
- Missing panel source fails with `SOURCE_OF_TRUTH_MISSING` and leaves `dj_trajectories` and `dj_audit_log` unchanged.

Manual Phase 5 evidence lives in `tmp/5090jepa_evidence/phase5/manual/`.
Independent physical check:

```bash
for cf in dj_raw_events dj_transitions dj_latent_panels dj_bindings dj_bindings_by_entity dj_trajectories dj_audit_log; do
  /usr/bin/ldb --db="$DJ_DB" --column_family="$cf" dump --count_only | awk -v cf="$cf" '/Keys in range:/ {print cf, $4}'
done
```

Phase 6 counter_world dataset run continues from a DB that has one trajectory
and three panels:

```bash
./target/debug/context-graph dynamicjepa compile-dataset --db "$DJ_DB" \
  --domain counter_world --policy one_step --split train --json
./target/debug/context-graph dynamicjepa compile-dataset --db "$DJ_DB" \
  --domain counter_world --policy one_step --split val --json
./target/debug/context-graph dynamicjepa inspect-cf --db "$DJ_DB" \
  --cf dj_dataset_shards --limit 10 --json
```

Expected counter_world dataset counts after train+val:

```text
dj_dataset_shards 2
dj_trajectories 1
dj_latent_panels 3
dj_audit_log 13
```

For the full gridworld corpus, run the same register/ingest/materialize/
compile-trajectories flow against `gridworld_5x5_corpus.jsonl`, then compile
all three splits:

```bash
./target/debug/context-graph dynamicjepa compile-dataset --db "$DJ_DB" \
  --domain gridworld_5x5 --policy one_step --split train --json
./target/debug/context-graph dynamicjepa compile-dataset --db "$DJ_DB" \
  --domain gridworld_5x5 --policy one_step --split val --json
./target/debug/context-graph dynamicjepa compile-dataset --db "$DJ_DB" \
  --domain gridworld_5x5 --policy one_step --split test --json
```

Expected full-gridworld Phase 6 counts for the checked-in corpus:

```text
dj_raw_events 8000
dj_latent_panels 8000
dj_trajectories 1000
dj_dataset_shards 3
dataset rows train=4977 val=1330 test=693 total=7000
```

Phase 6 manual evidence lives in `tmp/5090jepa_evidence/phase6/manual_counter/`
and `tmp/5090jepa_evidence/phase6/manual_gridworld/`.

Phase 7 counter_world training/artifact FSV is currently driven by the CLI test
because it generates a deterministic 60-event counter_world fixture before
running the real CLI pipeline:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase7_fsv --all-features -- --nocapture
```

Expected Phase 7 evidence:

```text
tmp/5090jepa_evidence/phase7/cli_phase7_fsv.json
tmp/5090jepa_evidence/phase7/cli_phase7_fsv_runtime/
```

Expected happy-path source-of-truth counts after training:

```text
dj_raw_events 60
dj_latent_panels 60
dj_trajectories 1
dj_dataset_shards 3
dj_training_runs 1
dj_model_artifacts 1
dj_predictions 0
dj_audit_log 187
```

Expected current model evidence in `happy_train.summary.metrics`:

```text
parameter_count_trainable 480
val_latent_mse <= 0.005
val_latent_cosine >= 0.99
vicreg_variance_per_dim_min >= 0.5
```

Known caveats: the strict shuffled-target and covariance targets are recorded but
not yet passing acceptance gates, gridworld model training FSV has not been
completed, and bundled `verify-*` commands are still future work.

Phase 8 counter_world prediction/planning/surprise FSV is:

```bash
cargo test -p context-graph-cli --test dynamicjepa_phase8_fsv --all-features -- --nocapture
```

Expected Phase 8 evidence:

```text
tmp/5090jepa_evidence/phase8/cli_phase8_fsv.json
tmp/5090jepa_evidence/phase8/cli_phase8_fsv_runtime/
```

Expected Phase 8 final source-of-truth counts include:

```text
dj_actions 71
dj_predictions 11
dj_guard_decisions 10
dj_plan_traces 2
dj_skill_policies 1
dj_surprise_events 1
dj_training_runs 1
dj_model_artifacts 1
dj_audit_log 194
```

## 5. Future Phase 10 Bundled Counter_world Run

Current Phase 8 evidence is produced by `dynamicjepa_phase8_fsv.rs` in Section 4. The bundled `verify-counter-world` command below is still future Phase 10 work.

```bash
export DJ_DB=/tmp/5090jepa-cw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts/cw
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"

# Single bundled FSV harness
./target/release/context-graph dynamicjepa verify-counter-world \
  --db "$DJ_DB" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "$DJ_ARTIFACTS" \
  --evidence-root tmp/5090jepa_evidence/counter_world \
  --json | tee tmp/5090jepa_evidence/counter_world/verify-counter-world.json
```

Expected exit code: `0`. Expected duration: ≤ 60 s. Inspect:

```bash
./target/release/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
ls tmp/5090jepa_evidence/counter_world/<verification_run_uuid>/decoded/
```

If `inspect-counts` does not match the counter_world table in Section 4 of `14_full_state_verification_protocol.md`, the run is not valid — read the verification_run_record.json `failure_details` and fix.

## 6. Future Phase 10 Gridworld Run

```bash
export DJ_DB=/tmp/5090jepa-gw-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts/gw
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"

./target/release/context-graph dynamicjepa verify-gridworld \
  --db "$DJ_DB" \
  --fixtures-root configs/dynamicjepa \
  --artifact-root "$DJ_ARTIFACTS" \
  --evidence-root tmp/5090jepa_evidence/gridworld \
  --json | tee tmp/5090jepa_evidence/gridworld/verify-gridworld.json
```

Expected duration: ≤ 5 min. Same inspection step.

## 7. Future Phase 10 Universality Audit

Run both verifications against the **same** fresh DB to prove no hard-coded `if domain == ...` branches:

```bash
export DJ_DB=/tmp/5090jepa-shared-fsv
export DJ_ARTIFACTS=/tmp/5090jepa_artifacts/shared
rm -rf "$DJ_DB" "$DJ_ARTIFACTS"
mkdir -p "$DJ_ARTIFACTS"

./target/release/context-graph dynamicjepa verify-counter-world \
  --db "$DJ_DB" --fixtures-root configs/dynamicjepa \
  --artifact-root "$DJ_ARTIFACTS" \
  --evidence-root tmp/5090jepa_evidence/shared/cw --json

./target/release/context-graph dynamicjepa verify-gridworld \
  --db "$DJ_DB" --fixtures-root configs/dynamicjepa \
  --artifact-root "$DJ_ARTIFACTS" \
  --evidence-root tmp/5090jepa_evidence/shared/gw --json

./target/release/context-graph dynamicjepa inspect-counts --db "$DJ_DB" --json
```

Expected: domain pack count = 2, training runs = 2, artifacts = 2, verification runs = 2 — all in one DB.

## 8. Future Phase 9 Capability Matrix Check

```bash
# Spawn a temp MCP server pointing at the shared demo DB:
CONTEXT_GRAPH_STORAGE_PATH="$DJ_DB" \
  ./target/release/context-graph-mcp \
  --transport tcp --port 3199 --bind 127.0.0.1 \
  --no-warm &
MCP_PID=$!
sleep 2

# Call tools/list and get_capability_matrix via curl JSON-RPC:
curl -sS -X POST http://127.0.0.1:3199 -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq '.result | length'
curl -sS -X POST http://127.0.0.1:3199 -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_capability_matrix"}}' | jq '.result.dynamicjepa'

kill $MCP_PID
```

Expected: `tools/list` length = 136 (110 base + 26 dynamicjepa) with `llm` feature, or 132 without. `dynamicjepa.enabled = true` and CF counts populated.

## 9. Test Suite

```bash
cargo test -p context-graph-core dynamicjepa --release --all-features
cargo test -p context-graph-storage dynamicjepa --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase3_fsv --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase4_fsv --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase5_fsv --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase6_fsv --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase7_fsv --release --all-features
cargo test -p context-graph-cli --test dynamicjepa_phase8_fsv --release --all-features
cargo test -p context-graph-dynamicjepa --release --all-features

# Future phases, once MCP DynamicJEPA surface exists:
# cargo test -p context-graph-mcp dynamicjepa --release --all-features

# Full workspace if you have time:
cargo test --workspace --release --all-features
```

All targets must pass with **zero** ignored DynamicJEPA tests. If any are ignored, that is a bug — `#[ignore]` is reserved for cases the user has explicitly approved.

## 10. Evidence Bundle

```bash
ls tmp/5090jepa_evidence/
# Bundle for review:
tar czf /tmp/5090jepa_evidence.tar.gz tmp/5090jepa_evidence/
sha256sum /tmp/5090jepa_evidence.tar.gz
```

Hand the tarball to the operator along with the `verification_run_record.json` of each run. That is the artifact reviewer expects.

## 11. Failure Recovery

If a run fails:

1. Open `tmp/5090jepa_evidence/<domain>/<run_uuid>/verification_run_record.json`. Read `status`, `failure_details`, `evidence_path`.
2. Open the file at `evidence_path` to see the exact mismatch (count diff, decoded record, hash mismatch).
3. Cross-reference against `14_full_state_verification_protocol.md`.
4. Fix the lowest-level invariant and rebuild only the affected crate (`cargo build --release -p <crate>`).
5. Re-run the relevant runbook section from a clean DB — the harness wipes its `/tmp/` paths automatically.

Never re-run with stale `/tmp/` state. Never delete the evidence directory until you have the working run for comparison.
