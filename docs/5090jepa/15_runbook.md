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

Confirm: branch `main`, RTX 5090 / driver 596+ / sm_120 / 32 GB VRAM, CUDA 13.x, rustc 1.75+.

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

## 4. Counter_world Run

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

If `inspect-counts` does not match Section 3 of `14_full_state_verification_protocol.md`, the run is not valid — read the verification_run_record.json `failure_details` and fix.

## 5. Gridworld Run

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

## 6. Universality Audit

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

## 7. Capability Matrix Check

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

## 8. Test Suite

```bash
cargo test -p context-graph-core dynamicjepa --release --all-features
cargo test -p context-graph-storage dynamicjepa --release --all-features
cargo test -p context-graph-dynamicjepa --release --all-features
cargo test -p context-graph-cli dynamicjepa --release --all-features
cargo test -p context-graph-mcp dynamicjepa --release --all-features

# Full workspace if you have time:
cargo test --workspace --release --all-features
```

All targets must pass with **zero** ignored DynamicJEPA tests. If any are ignored, that is a bug — `#[ignore]` is reserved for cases the user has explicitly approved.

## 9. Evidence Bundle

```bash
ls tmp/5090jepa_evidence/
# Bundle for review:
tar czf /tmp/5090jepa_evidence.tar.gz tmp/5090jepa_evidence/
sha256sum /tmp/5090jepa_evidence.tar.gz
```

Hand the tarball to the operator along with the `verification_run_record.json` of each run. That is the artifact reviewer expects.

## 10. Failure Recovery

If a run fails:

1. Open `tmp/5090jepa_evidence/<domain>/<run_uuid>/verification_run_record.json`. Read `status`, `failure_details`, `evidence_path`.
2. Open the file at `evidence_path` to see the exact mismatch (count diff, decoded record, hash mismatch).
3. Cross-reference against `14_full_state_verification_protocol.md`.
4. Fix the lowest-level invariant and rebuild only the affected crate (`cargo build --release -p <crate>`).
5. Re-run from Section 4 (or 5) — the harness wipes its `/tmp/` paths automatically.

Never re-run with stale `/tmp/` state. Never delete the evidence directory until you have the working run for comparison.
