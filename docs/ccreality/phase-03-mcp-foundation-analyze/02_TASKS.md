# Phase 3 — Tasks

Seventeen ordered tasks. The first three set up the module skeleton; tasks 3.5–3.15 implement one tool each; tasks 3.16–3.17 wire and verify.

## Task 3.1 — Read the existing tool-registration pattern

```bash
cd /home/cabdru/contextgraph
ls crates/context-graph-mcp/src/handlers/tools/
cat crates/context-graph-mcp/src/handlers/tools/mod.rs
cat crates/context-graph-mcp/src/main.rs | head -80
```

Identify:
- How tool-family modules are declared (in `mod.rs`).
- How tool handlers are exposed (function signature, return type).
- Whether there's a registry, a trait, or a macro.
- Where the JSON-RPC server is bound to stdin/stdout.

Document findings in your own working notes. Do not modify code yet.

## Task 3.2 — Create the `reality_loop/` module skeleton

```bash
mkdir -p /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/reality_loop
```

Create files per [03_FILE_SPECS.md](03_FILE_SPECS.md):
- `mod.rs` — module declarations + a public `register(...)` function.
- `errors.rs` — `CCRealityError` type and `Result<T>` alias.
- `helpers.rs` — `read_active_runtime_root()`, `parse_attempt_dir(...)`, etc.
- `path_policy.rs` — `GovernedPathsPolicy::load()` and `is_allowed(...)`.
- `analyze.rs` — empty for now; tools added in 3.5–3.15.

Register in `crates/context-graph-mcp/src/handlers/tools/mod.rs`:
```rust
pub mod reality_loop;
```

Verify:
```bash
cargo check -p context-graph-mcp --quiet
echo "exit=$?"
```
Expected: 0.

## Task 3.3 — Add `--mode` CLI to `main.rs`

In `crates/context-graph-mcp/src/main.rs`, add a clap arg parser if not present, with a `--mode` flag accepting `default | reality-loop`. When `--mode reality-loop`, after registering existing tools, also call `reality_loop::register(...)`.

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"main.rs additions" for the exact code.

Verify:
```bash
cargo build -p context-graph-mcp --quiet
./target/debug/context-graph-mcp --help 2>&1 | grep -q -- "--mode"
echo "exit=$?"
```
Expected: 0.

## Task 3.4 — Implement `path_policy.rs`

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"path_policy.rs". Include:
- `GovernedPathsPolicy { allow: Vec<glob::Pattern>, deny: Vec<glob::Pattern> }`
- `pub fn load() -> Result<Self>` — reads `$CGREALITY_GOVERNED_PATHS` (env var) or falls back to `~/.config/cgreality/governed_paths.json` or `<project_root>/config/ccreality_governed_paths.json` or a hard-coded conservative deny list.
- `pub fn is_allowed(&self, path: &str) -> PolicyVerdict` — returns Allow / DenyExplicit / NoMatch (caller decides default).
- Globs use the `glob` crate (add to Cargo.toml if missing).

Add a test:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deny_dotenv_and_swebench() {
        let p = GovernedPathsPolicy::default_conservative();
        assert!(matches!(p.is_allowed(".env"), PolicyVerdict::DenyExplicit(_)));
        assert!(matches!(p.is_allowed("swebench/eval/runner.py"), PolicyVerdict::DenyExplicit(_)));
        assert!(matches!(p.is_allowed("crates/context-graph-cli/src/bin/reality-loop.rs"), PolicyVerdict::Allow));
    }
}
```

Verify:
```bash
cargo test -p context-graph-mcp --lib reality_loop::path_policy --quiet
```

## Task 3.5 — Implement `reality_latest_root` tool

In `analyze.rs`, add:

```rust
pub async fn reality_latest_root(_args: serde_json::Value) -> Result<serde_json::Value> {
    let runtime_root = read_active_runtime_root().await?;
    let target = read_active_target_instance().await.unwrap_or(None);
    // Walk runtime_root for the most recent run-* dir
    let active_run_id = runtime_root.as_ref().and_then(|r| latest_run_id(r));
    Ok(serde_json::json!({
        "runtime_root": runtime_root.as_ref().map(|p| p.to_string_lossy()),
        "active_run_id": active_run_id,
        "active_task_id": target,
        "active_target_instance": target,
        "success_predicate": "official SWE-bench report places target instance in resolved_ids",
        "last_attempt_number": runtime_root.as_ref().and_then(|r| latest_attempt_number(r, target.as_deref())),
        "source_of_truth": runtime_root.as_ref().map(|p| format!("file:{}", p.to_string_lossy()))
    }))
}
```

Register in `mod.rs::register()`. Test by piping a tools/call request.

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"analyze.rs reality_latest_root" for the full implementation.

Verify:
```bash
cargo build -p context-graph-mcp --quiet
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"reality_latest_root","arguments":{}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop \
  | jq -e '.result.content[0].text | fromjson | .runtime_root' > /dev/null
echo "exit=$?"
```

## Task 3.6 — Implement `reality_attempt_summary`

Schema:
```json
{ "run_id": "string", "attempt": integer }
```

Body: read `<active_runtime_root>/<run_id>/<task_id>/attempt-N/attempt-summary.json`, return verbatim plus `source_of_truth`.

Need to map `run_id` → directory. Convention: `<runtime_root>/<run_id>/` is the run dir; the task_id is a subdirectory underneath. To find the right attempt, walk:
```rust
let attempt_path = runtime_root.join(&run_id).join(&task_id).join(format!("attempt-{}", attempt));
```

If `task_id` is not provided, infer from `active_target_instance`. If none, error `CCREALITY_NO_ACTIVE_TASK`.

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) for the full code.

## Task 3.7 — Implement `reality_official_report`

Same input as 3.6. Reads `<attempt-dir>/official-evidence/<instance>.json`.

If the instance file doesn't exist, return error `CCREALITY_OFFICIAL_REPORT_MISSING` with the searched path.

## Task 3.8 — Implement `reality_problem_packet`

Schema: `{ "run_id": "string", "compact": "boolean" }` (no attempt — problem reality is per-run, not per-attempt).

Reads `<runtime_root>/<run_id>/<task_id>/problem-reality-packet.json`.

If `compact: true`, apply a JSON projection that mirrors what `compact_problem_reality_packet` does in `reality-loop.rs` (you can either duplicate the logic in Rust here, or shell out to `reality-loop replay --compact-problem-reality`; first option is cleaner). Document choice in [03_FILE_SPECS.md](03_FILE_SPECS.md).

## Task 3.9 — Implement `reality_signal`

Schema: `{ "run_id": "string", "attempt": integer, "compact": "boolean" }`.

Reads `<attempt-dir>/reality-signal-packet.json`. If `compact`, apply projection.

## Task 3.10 — Implement `reality_failure`

Schema: `{ "run_id": "string", "attempt": integer }`.

Reads `<attempt-dir>/validation.json` (or `attempt-summary.json::failure_evidence` field), returns the compact form (see `compact_failure_evidence` in `reality-loop.rs`).

## Task 3.11 — Implement `reality_trigger_decision`

Schema: `{ "run_id": "string", "attempt": integer }`.

Reads `<attempt-dir>/trigger-decision.json`. This file is written by `optimizer_record_decision` (Phase 4). If file doesn't exist (no decision recorded yet for this attempt), return `{ "status": "no_decision_recorded", "source_of_truth": null }`.

## Task 3.12 — Implement `reality_harness_transitions`

Schema: `{ "run_id": "string", "attempt": integer, "round": integer | null }`.

Reads `<attempt-dir>/harness-transitions/round-NN.json` (full record) or lists all rounds (`{ round, tool_use_id, file_path, outcome, source_of_truth }` per record).

## Task 3.13 — Implement `reality_compare_attempts`

Schema: `{ "run_id": "string", "attempt_a": integer, "attempt_b": integer }`.

Reads both attempt-summaries, computes:
- patch_diff_summary: line counts via `git diff` between the two patch files.
- validation_status_change: `{ "from": A.status, "to": B.status }`.
- official_resolved_change: same shape from `attempt.resolved`.
- reward_delta: `B.reward.clipped - A.reward.clipped`.

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) for the full body.

## Task 3.14 — Implement `reality_audit_trail`

Schema: `{ "entity_id": "string (uuid)", "limit": integer }`.

This is a thin re-export of the existing memory-tool `get_audit_trail` (or whatever it's called in the existing codebase). Look it up in `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` (or `provenance_tools.rs`). Wrap it so the response includes `source_of_truth` and conforms to the cgreality envelope.

## Task 3.15 — Implement `reality_replay_artifact`

Schema: `{ "path": "string (file:... or absolute)" }`.

Reads the file, computes SHA-256, returns `{ exists, sha256, size_bytes, mtime_unix, content_or_pointer, source_of_truth }`. Inlines content only if `size_bytes < 32 KB`.

## Task 3.16 — Re-export memory/graph/causal tools under `cgreality`

In `mod.rs::register()`, after registering reality_* tools, ALSO register the existing memory tools under the same MCP server. The simplest way:

```rust
pub fn register(server: &mut McpServer) {
    // 1. cgreality-native reality tools
    server.register_tool("reality_latest_root", analyze::reality_latest_root_descriptor());
    // ... 11 more reality_* registrations ...

    // 2. Re-export existing memory/graph/causal tools
    crate::handlers::tools::memory_tools::register(server);
    crate::handlers::tools::graph_tools::register(server);
    crate::handlers::tools::causal_tools::register(server);
    // ... whatever else makes sense ...
}
```

If the existing tool families are already registered by the default `register_default_tools()` path, then `--mode reality-loop` should call BOTH: default tools + reality_loop module. The right pattern depends on the existing structure (verify in Task 3.1).

## Task 3.17 — Final build + verify

```bash
cd /home/cabdru/contextgraph
cargo fmt -p context-graph-mcp
cargo build -p context-graph-mcp --quiet
cargo test -p context-graph-mcp --lib --quiet
```

Then execute [04_VERIFICATION.md](04_VERIFICATION.md) end-to-end.
