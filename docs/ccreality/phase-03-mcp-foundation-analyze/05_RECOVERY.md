# Phase 3 — Recovery

Common failures during MCP module setup and how to fix them.

## Failure: stdout is corrupted (3.V10)

**Symptom:** `jq` fails to parse the MCP server's stdout. Verbose JSON-RPC errors appear interleaved with what looks like log lines.

**Cause:** Some code path uses `println!`, `eprintln!` (oops, eprintln goes to stderr correctly — but check anyway), or a third-party crate logs to stdout (some HTTP clients, some debug crates).

**Fix:**
1. Confirm `tracing_subscriber` is initialized with `.with_writer(std::io::stderr)`.
2. Search for any `println!`, `dbg!`, `print!` in the `reality_loop` module: `rg "println!|dbg!|^[[:space:]]*print!" crates/context-graph-mcp/src/handlers/tools/reality_loop/`
3. If a third-party crate is the culprit, set `RUST_LOG=warn` or use `.with_target(true)` to identify the source.

The rmcp crate documentation explicitly warns: "Any println!, dbg!, or third-party crate that logs to stdout will corrupt the stdio transport and Claude Code will silently disconnect."

## Failure: tools/list returns 0 reality_* tools

**Symptom:** The clause 3.V5 grep finds no matches.

**Cause:** Either:
1. The `register()` function in `reality_loop/mod.rs` is not called.
2. The tool name strings in `register()` don't match the verification grep regex.

**Fix:**
1. Trace through `main.rs`: confirm `--mode reality-loop` actually calls `reality_loop::register(...)`.
2. Print the registered tool list at startup (to stderr): add `tracing::info!("registered reality_* tools: {}", count);` in `register()`.

## Failure: tool returns "method not found"

**Symptom:** `tools/call` for `reality_latest_root` returns JSON-RPC error -32601.

**Cause:** Tool name mismatch between registration and invocation, OR the existing crate's tool-routing doesn't dispatch by name (uses something else like type IDs).

**Fix:**
1. Look at the existing `memory_tools.rs` registration to confirm naming convention. Is the registered name `mcp__cgreality__reality_latest_root` or just `reality_latest_root`? Some MCP setups prefix in registration; some let the client prefix.
2. Match the existing convention exactly.

## Failure: cargo error "no field `governed_paths` in struct ..."

**Symptom:** `path_policy.rs` fails to compile because `glob::Pattern` doesn't expose what we need.

**Fix:** `glob::Pattern::new(s).unwrap()` panics on malformed patterns. Replace `.unwrap()` with proper error handling that returns `CCRealityError`. Test patterns in unit tests to catch typos.

## Failure: `dirs::home_dir()` returns None on CI

**Symptom:** Verification on CI fails because `$HOME` is unset.

**Fix:** In `helpers.rs`, instead of failing when `home_dir()` returns None, fall back to a `CGREALITY_CACHE_DIR` env var, then a tempdir.

```rust
fn cgreality_cache_dir() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CGREALITY_CACHE_DIR") {
        return Some(std::path::PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".cache/cgreality"))
}
```

## Failure: artifact path traversal

**Symptom:** `reality_attempt_summary` for run_id `../../etc/passwd` returns sensitive file contents.

**Cause:** We didn't validate run_id and task_id strings.

**Fix:** In `helpers.rs`, add:
```rust
fn safe_id(value: &str) -> Result<String> {
    if !value.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(CCRealityError::new(
            "CCREALITY_ARG_UNSAFE_ID",
            "id contains characters outside [a-zA-Z0-9_-]",
            "arguments.id",
            "use the run_id/task_id from reality_latest_root",
            json!({"value": value}),
            None,
        ));
    }
    Ok(value.to_string())
}
```

Apply to every run_id, task_id, instance_id, attempt round-tripped from the caller.

## Failure: reality_attempt_summary returns "no active task"

**Symptom:** Tool returns `CCREALITY_NO_ACTIVE_TARGET_INSTANCE`.

**Cause:** Phase 2's `update_active_runtime_root` wasn't called, or the smoke runtime root was deleted.

**Fix:** Re-run smoke:
```bash
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-phase03-smoke --label fix
```

Then re-verify clause 3.V7.

## Failure: tools/call returns "internal error" with empty content

**Symptom:** Tool dispatches but the response has empty `content[0].text`.

**Cause:** The tool function returned an error but the server framework swallowed it.

**Fix:** Confirm error handling pattern matches existing tools. Most rmcp setups expect `Result<Value, ToolError>`; if you return `Result<Value, CCRealityError>`, you may need an `impl From<CCRealityError> for ToolError` to bridge the types.

## Failure: protocol-version mismatch

**Symptom:** Server replies with "unsupported protocol version".

**Cause:** Initialize handshake wasn't sent or used the wrong version.

**Fix:** Send the initialize request first:
```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cgreality-verify","version":"0.1"}}}
```

Then send tools/list as the next request.

## Diagnosing "I don't know what's wrong"

```bash
cd /home/cabdru/contextgraph
echo "=== Build ==="
cargo build -p context-graph-mcp 2>&1 | tail -20
echo "=== Module files ==="
ls -la crates/context-graph-mcp/src/handlers/tools/reality_loop/
echo "=== Help ==="
./target/debug/context-graph-mcp --help 2>&1 | tail -30
echo "=== Initialize + tools/list ==="
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}'
  sleep 0.5
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
} | ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/mcp-stderr.log | head -3
echo "=== stderr ==="
tail -20 /tmp/mcp-stderr.log
```

The output points at exactly which layer is broken.
