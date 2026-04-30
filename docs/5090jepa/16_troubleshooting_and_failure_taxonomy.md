# 16 — Troubleshooting & Failure Taxonomy

This file is a debugging reference. The first job when something fails is to identify the *category* of failure — that determines which evidence to gather and which fix to apply.

Current implementation status as of the Phase 5 double-check on 2026-04-30: the failure modes through `ingest-event`, `run-adapter`, `materialize-panel`, `get-panel`, `list-instrument-readings`, `bind`, and `compile-trajectories` are operational and covered by FSV. Later dataset/model/planning failures are specified here for future phases.

## 1. Categories

| Code | Cause | Where it surfaces | Where to look |
| --- | --- | --- | --- |
| `VALIDATION` | Input violated a declared invariant before any storage write | CLI/MCP error body | The error message points to the field; check the fixture or domain pack TOML |
| `SOURCE_OF_TRUTH_MISSING` | A read tried to fetch a record that does not exist | Anywhere | Run `dynamicjepa inspect-counts` to check whether the previous step wrote the row |
| `STORAGE` | RocksDB open/read/write/deserialize problem | DB open or read/write paths | Check `LOG.old.*` files in the DB dir; check `mcp.pid` lock state |
| `CODEC` | Version mismatch, empty bytes, bincode failure, hash mismatch | Any decode | DB needs to be wiped (no migrations) — the demo's pre-flight already handles this |
| `INSTRUMENT_FAILED` | A required instrument could not produce a valid reading | `materialize-panel` | Check the instrument validation block in the domain pack and the input field's value |
| `ADAPTER_FAILED` | JSON parse, mapping, or coercion failure | `ingest-event` | Check the failing line of the JSONL fixture; field path is in the error body |
| `DATASET` | Leakage detected, missing rows, insufficient negatives | `compile-dataset` | LeakageReport is in the error body |
| `MODEL` | Training/inference/artifact failure | `train`/`predict`/`plan` | TrainingRunRecord status is `Failed`; metrics hint at NaN/non-convergence; ArtifactHashMismatch |
| `GUARD` | Action rejected by declared guard | `plan` | GuardDecisionRecord is persisted with reason |
| `VERIFICATION` | Actual state differs from expected during FSV | `verify-*` | VerificationRunRecord points to the specific count or hash mismatch |

Every error variant in `DynamicJepaError` (see `04_data_contracts_v1.md` §11) maps to one of these categories. Generic `String` errors are forbidden.

## 2. Common Failure Modes And Fixes

### 2.1 `cargo build` — `compute capability 120 not supported`

Cause: `CUDA_COMPUTE_CAP` is unset or set to a different value, and candle compiled cubin for the wrong sm.

Fix:
```bash
cargo clean -p candle-core -p candle-nn
export CUDA_COMPUTE_CAP=120
cargo build --workspace --release
```

If the error persists, confirm `nvcc --version` reports 13.x and `nvidia-smi` reports compute_cap 12.0.

### 2.2 `register-domain` — `unknown TOML key: <foo>`

Cause: TOML is in strict mode; an unknown key was added (typo or unsupported feature).

Fix: Either remove the unknown key from the TOML, or extend the schema parser to accept it (and bump the domain pack `schema_version`). Strict mode is intentional.

### 2.3 `register-domain` — duplicate instrument id

Cause: two `[[instruments]]` entries in a domain pack use the same `id`. Phase 3 validation catches this before writing any domain, instrument, adapter, or audit rows.

Fix: inspect the TOML, deduplicate the instrument ids, and rerun against a fresh demo DB. The expected source-of-truth proof is identical `inspect-counts` output before and after the failed command.

### 2.4 `ingest-event` — `field_path = action.delta, expected = i64, actual = string`

Cause: a JSONL line has the wrong type for a declared field. This is the **expected** output for the EC2 edge case fixtures; if it happens during a happy-path run, the fixture has been corrupted.

Fix: Validate the fixture file with `jq -c .` to see if any line has type drift. Re-generate from the spec.

### 2.4a `ingest-event` — empty input writes one failed audit row

Cause: the JSONL file is zero bytes. Phase 4 treats this as an operation-level adapter failure, not a pre-operation schema validation failure.

Fix: If this was not an edge-case test, point the command at a non-empty fixture. The expected source-of-truth delta is `dj_audit_log +1` and no change to `dj_raw_events`, `dj_adapter_runs`, normalized CFs, readings, or panels.

### 2.4b `ingest-event` — gridworld OOB reports `state.x`

Cause: the adapter validates fields in registered schema order, not arbitrary TOML mapping-table order. For `gridworld_5x5_edge_oob_state.jsonl`, `state.x=7` must be reported before the invalid mirrored `outcome.next_x=7`.

Fix: If a future refactor reports `outcome.next_x` first for this fixture, restore schema-order validation. The FSV test intentionally checks `state.x` to prevent masking the root cause.

### 2.5 `materialize-panel` — `RequiredInstrumentMissing`

Cause: a required instrument's input field is missing from the normalized state/action/outcome rows.

Fix: Re-read `08_adapter_and_panel_pipeline.md` Section 4. The most likely cause is an adapter mapping that points to a JSON path that does not exist in the fixture. In Phase 4, confirm by `dynamicjepa inspect-cf --cf dj_normalized_states` / `dj_actions` / `dj_outcomes` and inspect the field maps; dedicated `get-state` style commands are later-phase read conveniences.

### 2.6 `compile-dataset` — `DatasetLeakageDetected: insufficient negatives`

Cause: not enough panels with different outcomes to satisfy `min_negatives_per_row`.

Fix: For counter_world this should never happen (3 distinct outcomes). For gridworld this should also not happen (8000 panels). If it does, the corpus is degenerate — re-generate.

### 2.7 `train` — `TrainingFailed: max_seconds budget exceeded before convergence`

Cause: training did not hit `stopping.target` within `stopping.max_seconds`.

Fixes (in order):

1. Check `nvidia-smi -l 1` during training; if GPU utilization is 0%, the training is on CPU. Confirm `cfg.schedule.device == "cuda"` and the device is visible from the build.
2. Check VICReg variance per dim; if it is collapsing toward zero, increase `vicreg_variance_weight`.
3. Check loss curve in metrics; if it is oscillating, lower `lr` by 2x.
4. As a last resort, raise `max_seconds` and `epochs` and document why in the run's evidence log.

Do **not** bypass the convergence target by lowering it — the convergence asserts in `tests/*_fsv.rs` will still fail.

### 2.8 `predict` — `ArtifactHashMismatch`

Cause: the artifact files on disk do not match the registry. Either someone edited the file (intentional EC test) or the run was interrupted between file write and registry write.

Fix:

- If intentional (EC test): the demo expects this and the harness records it.
- Otherwise: drop the `ModelArtifactRecord` row, re-run training (the demo wipes its DB anyway).

### 2.9 `verify-counter-world` — `VerificationFailed: count mismatch dj_panels expected=3 actual=2`

Cause: one panel did not materialize. Almost always a missing required instrument (see 2.4).

Fix: re-read the AdapterRunRecord rows; one will be `Completed` while another is `Failed`. The harness's evidence dir contains the failing record. Fix the input or the instrument and re-run.

## 3. Anti-Patterns To Reject In PR Review

The user has been explicit: **fail fast, no fallbacks, no covering up broken state with passing tests**. Any of these patterns is a P0 review block:

- `unwrap_or_default()` on a vector or record read.
- `Option::unwrap_or(some_default)` where the missing value is genuinely an error.
- A test that asserts only the operation's return value and skips the source-of-truth read.
- A `#[cfg(test)]` branch that swaps a real RocksDB for an in-memory mock.
- A function that catches and downgrades a typed error to a generic string.
- A "best effort" decode path that returns a default record on bincode failure.
- An `if cfg!(feature = "...")` branch that silently disables a verification step.
- Ignoring `#[allow(dead_code)]` on a verification-related function (verification helpers must always be live).
- A test that mutates the fixture file at runtime to make itself pass.
- A handler that writes one row then ignores the failure of writing the related audit row (the batch must be atomic).

## 4. Logging Conventions

Every operation emits a single `tracing` event at `info` (success) or `error` (failure) with these structured fields:

```text
operation         = "dynamicjepa_<...>"
run_id            = <uuid> if applicable
domain_pack_id    = <id> if applicable
target_id         = <uuid> if applicable
source_of_truth_cf = comma-separated list
status            = "ok" | "error"
elapsed_ms        = i64
record_count_before = <map serialized as JSON>
record_count_after  = <map>
error_code        = <enum string> if error
error_message     = <string> if error
remediation_hint  = <string> if error
```

Never emit `format!("{:?}", err)` — always use the typed fields. Tests in `tests/logging_test.rs` assert that the structured fields are present for every public CLI command.

## 5. Operator Debugging Flow

When a fresh report says "the demo failed":

1. Read the operation name and `verification_run_id`.
2. `cat tmp/5090jepa_evidence/<domain>/<run_id>/verification_run_record.json | jq '.failure_details'`.
3. Open the file at `evidence_path`.
4. `dynamicjepa inspect-counts --db <path> --json` to see the actual current state.
5. `dynamicjepa get-* --id <id>` for any decoded record referenced.
6. If a hash check failed: `sha256sum <file>` and compare to the registry value; if they differ but the demo never mutated the file, suspect filesystem corruption (rare on tmpfs).
7. Fix the lowest-level invariant and re-run the failing harness command.

The user's rule is to identify root causes, not bypass them. Apply ruthlessly.

## 6. Known Hardware Quirks (5090 + WSL2)

- **WSL2 first-call latency**: the very first CUDA kernel launch on a fresh WSL session takes 1–3 s while the driver maps the device. Tests that have tight `max_seconds` budgets (counter_world: 30 s) build in ≥ 5 s headroom for this. If you see "max_seconds exceeded by 2 s on first run only", warm up CUDA before the timer starts.
- **Driver/Toolkit mismatch**: if `nvcc --version` reports CUDA 13.2 but `nvidia-smi` reports a driver compatible with only CUDA 12.x, the WSL passthrough is misconfigured. Re-install the NVIDIA WSL driver from the operator's Windows host.
- **Power state**: laptops or balanced power plans throttle the 5090 down to 30% within a few seconds of GPU idleness. The first training step pays a wake-up cost. Demo tolerates it; if you see consistent 1.5x slowdowns, switch to "Performance" power plan in Windows.

## 7. When To Escalate

Escalate to the user (do not silently work around) when:

- A pinned dependency (candle, llama-cpp-2, rust toolchain) needs to change.
- A hardware constraint cannot be satisfied (e.g. VRAM is genuinely insufficient — should not happen with the tiny model).
- A demo target cannot be met (e.g. counter_world cannot reach 0.99 cosine; means there is a leakage or capacity issue, not a tuning issue).
- A change is requested that would touch the existing 73 CFs.
- A structured error category not in Section 1 appears.

Escalation includes pasting the relevant evidence files. Never claim "fixed" without re-running the FSV harness from a clean DB.
