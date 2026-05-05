# Phase 4 — Specification

## Objectives

1. Implement every write-side MCP tool: 6 INTERACT + 13 ALTER + 2 SHIFT = 21 new tools.
2. Implement the shift-log discipline: every alter tool appends a JSONL record after fsync; the record is queryable via `reality_shift_log`.
3. Implement single-writer enforcement: an OS-level lock at `~/.cache/cgreality/edit_lock` prevents concurrent harness edits.
4. Implement the recommendation-schema validator using `jsonschema` against `config/ccreality_recommendation_schema.json` (Phase 5 produces this file).
5. Implement the path-policy enforcement at every alter call site (re-uses Phase 3's `path_policy.rs`).

## Scope

In scope, new files under `crates/context-graph-mcp/src/handlers/tools/reality_loop/`:
- `alter.rs` — every harness_* and memory-mutation tool.
- `interact.rs` — every reality_run_* tool (shells out to engine binary).
- `shift_log.rs` — append-only JSONL writer + `reality_shift_log` + `reality_shift_compare_to_my_view`.
- `optimizer.rs` — `optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`.
- `lock.rs` — single-writer file lock helpers.
- `schema_validator.rs` — JSON-Schema validator for recommendations.

Updated:
- `mod.rs` — register the four new modules.
- `Cargo.toml` — add `jsonschema`, `fs2` (for file locking), `uuid`, and `regex` (for the strict CCReality path-pattern compiler used by Phase 5's governed-paths policy).

Out of scope:
- Hooks (Phase 6).
- Subagents/skills/.claude scaffolding (Phase 7).

## Deliverables

| Tool | Family | Behavior |
|------|--------|----------|
| `reality_run_attempt` | INTERACT | Spawns `reality-loop attempt`; returns exit_code + stdout/stderr tails + artifact path |
| `reality_run_smoke` | INTERACT | Spawns `reality-loop smoke` |
| `reality_run_official_eval` | INTERACT | Re-runs official Docker eval; needs existing patch |
| `reality_run_targeted_test` | INTERACT | One fail_to_pass test |
| `reality_recompile_reward` | INTERACT | Re-emits reality-signal-packet (calls `reality-loop replay`) |
| `reality_query_ledger` | INTERACT | Read-only SQL against ledger.sqlite |
| `harness_open_window` | ALTER | Opens project file window, returns SHA |
| `harness_apply_line_window_edit` | ALTER | Edits with SHA verification, mints transition + shift |
| `harness_run_command` | ALTER | Allowlisted command runner (cargo, git, rustfmt, etc.) |
| `harness_git_diff` | ALTER (read-tier) | Convenience wrapper for `git diff` |
| `harness_git_status` | ALTER (read-tier) | `git status --porcelain` parsed |
| `harness_verify_state` | ALTER | rustfmt --check + cargo check + cargo test + git status |
| `store_memory` | ALTER | Re-exported with namespace=ccreality |
| `boost_importance` | ALTER | Re-exported with namespace=ccreality |
| `forget_concept` | ALTER | Re-exported with namespace=ccreality |
| `merge_concepts` | ALTER | Re-exported with namespace=ccreality |
| `optimizer_record_decision` | ALTER | Mints trigger-decision artifact |
| `optimizer_record_recommendation` | ALTER | Validates + mints recommendation artifact |
| `optimizer_record_harness_transition` | ALTER | Mints harness-transition record (called by PostToolUse hook) |
| `reality_shift_log` | SHIFT | Reads cgreality-shift-log/<session>.jsonl |
| `reality_shift_compare_to_my_view` | SHIFT | Drift detection |

## Shift-log discipline

Every successful alter call:
1. Performs the mutation.
2. Calls `fsync` on the parent directory of the mutated artifact.
3. Re-reads the mutated artifact, computes new SHA.
4. Computes delta (line counts, before/after SHA, etc.).
5. Appends a JSON line to `<runtime_root>/cgreality-shift-log/<session_id>.jsonl`.
6. fsyncs the JSONL.
7. Returns the response, including `shift_id`.

If steps 1–6 do not all succeed, the tool returns a structured error that names the failed operation, the artifact path, and the remediation. Do not return success for a mutation whose shift-log append or fsync failed; that would hide a broken reality-drip record.

If step 1 fails, no shift is logged. Caller retries against fresh state.

## Single-writer enforcement

The `lock.rs` module provides:
```rust
pub struct EditLock {
    path: PathBuf,
    holder: String,  // session_id
    acquired_at: u64,
}
impl EditLock {
    pub fn try_acquire(session_id: &str) -> Result<Self> { /* O_EXCL on ~/.cache/cgreality/edit_lock */ }
    pub fn release(self) -> Result<()> { /* unlink */ }
}
```

`harness_apply_line_window_edit` acquires the lock at start, releases at end (or on drop via RAII).

If acquisition fails, returns `CCREALITY_HARNESS_EDIT_OWNER_BUSY` with the current holder's session_id and acquired_at.

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). 14 verification clauses covering build, file existence, tool count, edit round-trip with shift logging, drift detection, schema validation, recommendation recording, ledger query, single-writer lock, INTERACT shell-out.

## Time budget

3–5 hours typical:
- Tasks 4.1–4.4 (foundation: lock, shift-log, schema validator): 60 min.
- Tasks 4.5–4.10 (ALTER family): 90 min.
- Tasks 4.11–4.16 (INTERACT family): 60 min.
- Tasks 4.17–4.18 (optimizer family): 30 min.
- Tasks 4.19–4.20 (verify, iterate): 30–60 min.
