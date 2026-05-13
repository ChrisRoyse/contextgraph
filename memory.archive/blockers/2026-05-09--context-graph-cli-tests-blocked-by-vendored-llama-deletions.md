---
namespace: blockers
created: 2026-05-09T05:48:32Z
updated: 2026-05-09T07:03:36Z
resolved_at: 2026-05-09T06:35:00Z
status: resolved
severity: high
tags: ast, context-graph-cli, tests, llama-cpp-sys, vendor, build-blocker
---

# Context Graph CLI Tests Blocked By Vendored Llama Deletions

## What Happened

During the AST hardening pass, focused `context-graph-cli` Cargo tests could not reach Rust compilation. The build failed inside the unconditional `llama-cpp-sys-2` native build because vendored llama/ggml source files are missing from the working tree.

Observed failure shape:

```text
gmake[2]: *** No rule to make target
/home/cabdru/contextgraph/vendor/llama-cpp-sys-2-0.1.145/llama.cpp/ggml/src/ggml-cuda/template-instances/mmq-instance-q2_k.cu
```

`git status` also reports staged deletion of `crates/context-graph-cli/src/bin/reality-loop.rs` and broad staged deletions under `vendor/llama-cpp-sys-2-0.1.145/`. I did not restore those files because they appear to be pre-existing or concurrent workspace changes, and the active workspace rule is to avoid reverting user/concurrent changes without explicit permission.

## Why It Matters

Resolved 2026-05-09: the vendored llama dependency surface was retired instead of restored. `llama-cpp-2`, the vendored native patch directory, stale model pins, and the legacy `reality-loop` binary target are no longer part of the active build path.

Verification evidence:

```text
/tmp/contextgraph-mejepa-retirement-fsv/run.log
PASS: reality-loop source, vendored llama.cpp, model pins, model directory, and stale target binaries are absent
PASS: active CLI/MCP/benchmark/tooling surface does not reference retired local LLM or runner symbols
cargo check -p context-graph-cli --no-default-features --lib --quiet exit_code=0
```

Additional readback on 2026-05-09T07:03:36Z:

```text
/tmp/contextgraph-mejepa-retirement-fsv/run.log
FSV PASS after CLI and MCP no-default library checks.
cargo test -p context-graph-mcp --no-default-features path_policy --lib -- --nocapture: 2 passed
cargo test -p context-graph-cli --no-default-features governed_path_validation --lib -- --nocapture: 2 passed
cargo check -p context-graph-benchmark --no-default-features --quiet: passed
/tmp/contextgraph-local-model-retirement-fsv/run.log
ASSERTION=PASS temporary isolated Cargo target removed
```

## Related

- See also: ./memory/discoveries/2026-05-09--ruby-require-is-call-node.md
- See also: ./memory/blockers/2026-05-09--go-harness-rerun-blocked-by-typescript-compile-error.md
- FSV artifacts: `/tmp/contextgraph-ast-system-fsv/manual-fsv.log`, `/tmp/contextgraph-ast-system-fsv/ast-sample-evidence.json`
- Tags: ast, cargo, context-graph-cli, llama-cpp-sys, vendored-source, blocker
