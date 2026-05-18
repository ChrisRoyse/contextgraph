---
namespace: discoveries
created: 2026-05-09T04:13:20Z
updated: 2026-05-09T05:35:00Z
status: active
severity: medium
tags: mejepa,csharp,ast,tree-sitter,dotnet,fsv,fail-closed
---

# C# AST Operator And Dotnet Build Hardening

## What Happened

The C# AST/parser harness had three C#-specific gaps after the earlier tree-sitter ABI fix:

- `operator_declaration` symbols were persisted as the generic name `operator`, so all overloaded C# operators in a type collapsed to the same semantic symbol.
- Semantic-index C# build-target discovery only accepted `.sln` and `.csproj`, while the live action path already accepted `.slnx`.
- The C# compiler evidence command used `dotnet build --no-restore`, which makes a clean semantic snapshot fail before a normal .NET build can create the required `project.assets.json`.

## Why It Matters

ME-JEPA prediction features depend on stable symbol identity and compiler-backed state. Collapsing `operator +`, `operator ==`, and other overloads into one `operator` symbol corrupts reverse-reference and action-target features. Semantic-index/live-action disagreement on `.slnx` makes modern .NET solution repos behave differently depending on which harness path runs. `--no-restore` can turn an otherwise valid clean project into a false compiler failure.

## Related

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`

## Verification

The hardening adds C# tests that write real C# source to disk and re-read durable evidence under `/tmp/contextgraph-csharp-semantic-fsv/` and `/tmp/contextgraph-csharp-action-fsv/`.

After installing the user-local .NET SDK, the live action compiler branch now verifies the checked path with real `dotnet build` evidence:

- `dotnet --version`: `8.0.420`
- semantic source of truth: `/tmp/contextgraph-csharp-semantic-fsv/csharp-semantic-fsv.json`
- action source of truth: `/tmp/contextgraph-csharp-action-fsv/csharp-action-fsv.json`
- action checked state: `compiler_semantic_status="checked"`, `compiler_diagnostic_count=0`, `semantic_provider="compiler_lsp"`

The missing-toolchain branch remains fail-closed in code and was previously verified by hiding `dotnet` from `PATH`; the current host state exercises the real compiler happy path.
