---
namespace: journal
agent_id: codex
timestamp: 2026-05-09T03:55:05Z
created: 2026-05-09T03:55:05Z
updated: 2026-05-09T03:56:43Z
status: active
tags: failure,javascript,ast,verification,oom
---

# Failure: JavaScript AST Verification Fork OOM

## What Happened

After patching the JavaScript/TypeScript AST binding logic, the first verification batch tried to run `cargo fmt --all` and a `git diff` read. Both failed before process startup with `Out of memory (os error 12)`.

Immediate readback showed high process pressure from multiple `rust-analyzer` processes and an active release `rustc` under `/tmp/contextgraph-target-cli-review`, while `free -h` reported only 717 MiB actually free despite 70 GiB available via cache.

## Update 2026-05-09T03:56:43Z

`cargo fmt --all` succeeded after the initial fork failure. The next targeted test run got past the earlier compile errors, then `rustc` aborted with `memory allocation of 3178512 bytes failed`; the parallel reality-loop test command could not fork with the same OS error 12. A concurrent release build under `/tmp/contextgraph-target-cli-review` was still compiling `context_graph_embeddings` and another `context_graph_cli` rustc was left active.

## Why It Matters

The code patch is not verified until formatting and targeted tests can run. Future sessions should not treat the edit as complete merely because the patch applied.

## Related

- Files under test: `crates/context-graph-cli/src/commands/dynamicjepa.rs`, `crates/context-graph-cli/src/bin/reality-loop.rs`
- Next action: retry formatting/tests with constrained commands once the host can fork reliably.
