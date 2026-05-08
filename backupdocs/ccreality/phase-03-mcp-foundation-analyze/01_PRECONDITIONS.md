# Phase 3 — Preconditions

## Required state

1. **Phase 1 complete.** `cargo test -p context-graph-cli --bin reality-loop` passes; `governed_edit.rs` exists.
2. **Phase 2 complete.** `./target/debug/reality-loop smoke` produces artifacts; `~/.cache/cgreality/active_runtime_root` is written.
3. **Phase 5 complete.** `config/ccreality_governed_paths.json` exists. The Phase 1 `governed_edit` module fails closed when `CGREALITY_GOVERNED_PATHS` is missing, so Phase 3 must not introduce hard-coded path-policy fallbacks. Do Phase 5 before Phase 3.
4. **The existing `context-graph-mcp` binary builds and runs.** Verify:
   ```bash
   cargo build -p context-graph-mcp --quiet
   echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/context-graph-mcp | head -c 1000
   ```
   Should return a JSON-RPC reply listing the existing tools.
5. **Smoke runtime root exists** (from Phase 2):
   ```bash
   ls $(cat ~/.cache/cgreality/active_runtime_root)
   ```
   Should list the `run-*/cgreality-smoke-*` tree from Phase 2's verification.

## Required reading

Before starting:
- [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) — full ANALYZE spec.
- [00_SPEC.md](00_SPEC.md) — this phase's scope.
- A read-through of `crates/context-graph-mcp/src/handlers/tools/mod.rs` to understand the existing tool-registration pattern.
- A read-through of one existing tool family (suggest `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`) to understand the rmcp `#[tool]` macro pattern (or whatever mechanism the existing crate uses).

## Required tools

- `cargo` (Rust 1.80+).
- `jq` (for verification).
- A way to send JSON-RPC over stdio for verification. The simplest is:
  ```bash
  echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/context-graph-mcp --mode reality-loop
  ```
  Multi-request or multi-line responses may need a small helper script; one is provided in [04_VERIFICATION.md](04_VERIFICATION.md).

## What to learn from existing code first

Before writing any new code, an executing agent must understand:

1. **How tools are registered.** Read `crates/context-graph-mcp/src/handlers/tools/mod.rs` and identify the registration pattern. Three possibilities:
   - rmcp `#[tool]` proc-macro on a struct method.
   - A manual registry: `pub fn tools() -> Vec<Tool>` style.
   - A trait-based dispatch: `impl ToolHandler for ...`.

2. **How tool input/output is shaped.** Look at one existing handler. Note: input is typically `serde_json::Value` parsed by `schemars`; output is `Result<Value, ToolError>` or similar.

3. **How the MCP server is invoked.** Read `main.rs`. Look for `tokio::main`, `Server::stdio()`, `tracing` setup. Confirm logging goes to stderr.

4. **Where `--mode` (or any CLI subcommand) hooks in.** If `clap` is already used, `--mode` is a flag; if not, the entry point is bare and we add clap.

If any of these is unclear, STOP and read the existing `context-graph-mcp` source thoroughly before writing new tools. The new module must follow the existing patterns.

## What to NOT do

- Do not modify any existing tool family (memory_tools, graph_tools, causal_tools, etc.).
- Do not change the existing JSON-RPC framing or transport.
- Do not introduce a new MCP SDK; use whatever the existing crate uses (likely `rmcp`).
- Do not write to stdout from any code path (corrupts protocol).
- Do not add any tool that mutates state in this phase. Mutation is Phase 4. ANALYZE = read-only.
