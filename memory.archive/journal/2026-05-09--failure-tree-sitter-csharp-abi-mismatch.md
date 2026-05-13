---
namespace: journal
created: 2026-05-09T03:29:42Z
updated: 2026-05-09T03:29:42Z
status: resolved
tags: failure, tree-sitter, csharp, dynamicjepa, abi
---

# Failure: C# Tree-sitter Parser ABI Mismatch

## What Failed

The targeted C# DynamicJEPA semantic extraction test failed at parser initialization, before any semantic
assertions ran:

`Incompatible language version 15. Expected minimum 13, maximum 14`

## Investigation

The failure was not a C# query bug or missing source fixture. It came from dependency skew: the C# grammar
crate produced ABI 15, while the workspace `tree-sitter` runtime was too old to accept that ABI.

## Resolution

Updated the workspace runtime dependency to `tree-sitter = "0.26"` in the direct crates that initialize parsers,
then ran `cargo update -p tree-sitter`. The lockfile now resolves `tree-sitter v0.26.8`.

## Readback Evidence

`cargo tree -p context-graph-cli` now shows:

- `tree-sitter v0.26.8`
- `tree-sitter-c-sharp v0.23.5`

The previously failing C# semantic extraction test passed after this update.
