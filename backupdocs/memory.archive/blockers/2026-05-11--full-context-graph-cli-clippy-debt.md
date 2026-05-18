---
namespace: blockers
created: 2026-05-11T06:11:33Z
updated: 2026-05-11T10:19:34Z
status: active
severity: medium
tags: clippy, context-graph-cli, blackbox, governed-edit, dynamicjepa
---

# Full Context Graph CLI Clippy Debt

## What Happened
`cargo clippy -p context-graph-mcp --no-deps -- -D warnings` passes. The broader command `cargo clippy -p context-graph-mcp -- -D warnings` still fails in dependency crates, primarily `context-graph-cli`, with 528 clippy errors.

Representative failures:
- `crates/context-graph-cli/src/commands/blackbox.rs:540` `large_enum_variant` for `PolicyCommands::Evaluate(PolicyEvaluateArgs)`.
- Many `result_large_err` findings for `BlackboxError` and `GovernedEditError`.
- `too_many_arguments` in blackbox helper functions.
- Several `dynamicjepa.rs` style/perf lints such as identical branches, `last()` on a double-ended iterator, needless clones, and type complexity.
- `export_dynamicjepa.rs` needless borrows and `learning.rs` `&PathBuf` instead of `&Path`.

## Why It Matters
This is outside the daemon implementation path, but it blocks using full dependency clippy as a release gate for `context-graph-mcp`. The likely root fix is to shrink/box the CLI error types and large enum variants, then make the remaining mechanical clippy fixes in `context-graph-cli`.

## Latest Verification
Re-run on 2026-05-11T10:19:34Z after the flywheel daemon hardening patches:

- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings` passed.
- `cargo clippy -p context-graph-mcp -- -D warnings` still failed in `context-graph-cli` with 528 prior errors; first failure remains `crates/context-graph-cli/src/commands/blackbox.rs:540` `large_enum_variant`.

## Related
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-mode.md
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-hardening-review.md
- Tags: clippy, blocker, cli
