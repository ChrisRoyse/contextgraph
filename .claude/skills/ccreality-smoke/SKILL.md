---
name: ccreality-smoke
description: Run the post-pivot ME-JEPA retirement smoke that verifies the local inner loop is absent and the remaining CLI/MCP build surface compiles.
---

# ccreality-smoke

You are running the post-pivot ME-JEPA smoke flow. Goal: confirm the retired local reality-loop cannot run, while the remaining ME-JEPA/CLI/MCP surface still compiles.

For deterministic full-state verification from a shell, run `bash scripts/cc/tests/mejepa_retirement_fsv.sh`. That harness is the source of truth for the post-pivot cleanup because it checks file absence, active manifest references, fail-closed binary behavior, and focused cargo checks.

When invoked interactively, run the same fail-closed checks. Do not use mock data, inferred state, or a fallback success message. If any required readback is missing, stop and report the exact command and source-of-truth path that failed.

Step 1 - Verify retired files are absent. Confirm `crates/context-graph-cli/src/bin/reality-loop.rs` and `vendor/llama-cpp-sys-2-0.1.145/` do not exist.

Step 2 - Verify active manifests do not require llama. Run `rg "llama-cpp-2|llama_cpp_2|vendor/llama-cpp-sys-2" Cargo.toml crates/context-graph-cli crates/context-graph-mcp`; it must return no active references.

Step 3 - Verify build surface. Run `cargo check -p context-graph-cli --no-default-features --lib` and `cargo check -p context-graph-mcp --no-default-features --lib`.

Step 4 - Verify fail-closed binary behavior. Run `cargo check -p context-graph-cli --bin reality-loop`; it must fail because there is no `reality-loop` target.

Step 5 - Record evidence paths. The shell FSV writes logs under `/tmp/contextgraph-mejepa-retirement-fsv/`; use those files as source-of-truth readbacks.

Step 6 - Summarize the FSV result. Report the command exits plus the concrete files under `/tmp/contextgraph-mejepa-retirement-fsv/` that prove the outcome. If any step fails, stop and report the failing command and source-of-truth file.
