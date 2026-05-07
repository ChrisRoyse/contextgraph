---
namespace: reports
agent_id: agent-07-simplifier
timestamp: 2026-04-14T23:55:00Z
status: complete
tags: final-report, refactor, typed-edges, deduplication, simplifier
---

# Final Report — agent-07-simplifier (typed-edges refactor)

## Task

Review Phases 2–4 of the typed-edges training-data factory for duplication,
dead code, weak error context, missing docstrings, and >500-line files.
Produce a minimal refactor that preserves behavior.

## Outcome

SUCCESS. Three refinements landed, workspace builds clean (zero errors,
zero warnings), all 3376 relevant tests pass unchanged.

## Files changed

Modified (4):

- `crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs` — registered 2 new modules + adjusted re-exports
- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs` — 359 → 310 lines (-49)
- `crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs` — 295 → 267 lines (-28)
- `crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs` — 370 → 394 lines (+24; silent fallback replaced with observable skip counter)
- `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs` — 958 → 960 lines (+2; exposed new counter in the response payload + info! log)

Created (2):

- `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_keys.rs` — 83 lines (shared key helper + parser + 3 unit tests)
- `crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs` — 103 lines (shared `[version: u8][bincode]` encode/decode + 3 unit tests)

## Findings and decisions

### 1. Deduplicated 33-byte composite key helper — DONE

**Finding**: `typed_edge_export.rs` owned the helper; `llm_validation.rs` imported it as a sibling. This reads as if one CF "owned" the key shape, but both are peers.

**Fix**: Extracted `typed_edge_record_key`, `parse_typed_edge_record_key`, and `TYPED_EDGE_KEY_LEN` to a dedicated `typed_edge_keys.rs`. Both CFs now import from a neutral sibling. Bonus: centralized the key-parsing logic (was copy-pasted in the `list_*_keys` fn of both modules) behind a single `parse_typed_edge_record_key` helper.

**Public API**: `typed_edge_record_key` stays re-exported from `rocksdb_store::mod`. No external callers break.

### 2. Deduplicated `[version: u8][bincode]` encode/decode — DONE

**Finding**: `typed_edge_export.rs` and `llm_validation.rs` had near-identical encode/decode pairs — ~62 lines of duplicated logic differing only in type, version constant, and error retry hint.

**Fix**: Extracted two generic helpers (`encode_versioned` / `decode_versioned`) into `versioned_bincode.rs`. Per-CF wrappers are now 3-6 lines each and keep the public API stable.

**Scope**: Deliberately left the pre-existing `export.rs` (training records) alone per the "don't touch pre-existing code" instruction, even though its encode/decode pair is structurally identical. A follow-up simplifier pass could collapse that one too.

### 3. Replaced silent `unwrap_or_default()` with observable skip — DONE

**Finding**: `anomaly_derivation.rs` lines 118-125 used `self.get_content_async(...).await?.unwrap_or_default()` for both anchor and negative text. Missing content → empty string → useless ContrastivePair row written to disk. Spec §Non-Negotiables says "Fail fast with structured errors. No silent `unwrap_or_default()` paths."

**Fix**:
- Missing content now skips the pair with a `warn!` log carrying `src`, `tgt`, and `kind`.
- Added `AnomalyDerivationSummary::skipped_missing_content: usize` so callers can see how many pairs were skipped for this reason.
- Propagated the counter up through the MCP handler's JSON response (`summary.skipped_missing_content`) and the `info!` log line.
- No test changes needed — the integration test `anomaly_derivation_integration.rs` stores content for every fingerprint it creates, so the skip path is never exercised by existing tests and the behavior is unchanged for any well-formed corpus.

### 4. File length — NO-OP (justified)

- `typed_edges_tools.rs`: 960 lines (was 958). Agent-04 documented the decision to keep all four handlers in one module: the project convention already has `training_tools.rs` at 945 and `graph_tools.rs` at 1265. Splitting would fragment `impl Handlers { ... }` inconsistently. Per-handler section dividers already in place.
- `training/tests.rs`: 518 lines. Agent-02 documented at `memory/discoveries/agent-02-coder-core--tests-rs-line-budget.md`. Only 18 over the 500-line soft limit; splitting adds mechanical noise for negligible benefit.

### 5. Dead code — NO-OP (no findings)

Verified every public function has at least one caller:
- `count_typed_edge_records`, `count_llm_edge_validations` — called by integration tests + FSV example.
- `clear_all_*` — called by MCP `clearExisting` flag + integration tests.
- `list_*_keys` — called by MCP `list_typed_edge_records` handler + integration tests.
- `encode_*` / `decode_*` — called by CRUD paths + integration tests.

### 6. Docstrings on public items — VERIFIED complete

Every `pub` / `pub(crate)` item in the 7 new files already has a `///` doc comment. Confirmed with spot-read of each.

### 7. Encoding symmetry — VERIFIED

`encode_versioned` + `decode_versioned` are strictly inverse (encode prepends the version byte + bincode body; decode strips the version byte + bincodes the tail). Version mismatch and empty-payload cases are both rejected. Unit tests (`versioned_bincode::tests::*`) cover all three branches.

### 8. Version-byte handling — VERIFIED

Both new wire types (`TYPED_EDGE_RECORD_VERSION=1`, `LLM_EDGE_VALIDATION_VERSION=1`) use versioned encode/decode. Mismatches return `CoreError::SerializationError` with the expected + actual version + a retry hint. No silent migration.

### 9. Test quality — VERIFIED

- `fsv_typed_edges_physical.rs` is a real end-to-end run against `TempDir` RocksDB.
- No `unimplemented!()`, `todo!()`, or mocks in the 7 files in scope (grep confirmed).
- Integration tests use real `TempDir` + real `EdgeRepository` + real `AnomalyDerivationConfig::default()`.

## Build verification (final)

```
env CUDA_HOME=/usr/local/cuda cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.93s
```

Zero errors, zero warnings across 15 crates.

```
env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-storage -p context-graph-core -p context-graph-mcp --tests --lib
TOTAL: passed=3376 failed=0 ignored=1
```

Same count, same outcome as pre-refactor baseline (which was 3376 passed, 0 failed, 1 ignored).

## Memory files written

| Namespace | File |
|---|---|
| patterns | `agent-07-simplifier--versioned-bincode-shared-helper.md` |
| patterns | `agent-07-simplifier--shared-composite-key-module.md` |
| reports | `agent-07-simplifier--final-report.md` (this file) |

## Notes for Phase 5+ reviewers

- If a third CF ever needs the `[source:16][target:16][edge_type:u8]` shape, import from `typed_edge_keys` not a sibling CF.
- If `export.rs` (training records) is ever touched, consider collapsing its encode/decode into `versioned_bincode::{encode,decode}_versioned` for consistency.
- The `AnomalyDerivationSummary::skipped_missing_content` field is new — downstream dashboards that parse the JSON may want to surface it.
