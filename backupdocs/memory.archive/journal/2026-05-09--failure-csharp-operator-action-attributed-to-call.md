---
namespace: journal
created: 2026-05-09T04:18:10Z
updated: 2026-05-09T04:18:10Z
status: resolved
severity: medium
tags: failure,csharp,ast,action-semantics,operator,mejepa
---

# Failure: C# Operator Action Was Attributed To Nested Call

## What Happened

The live C# action test failed for an edit inside `operator +`. The action AST
walk selected the nested `object_creation_expression` (`new Vector(...)`) before
the enclosing `operator_declaration`, producing:

- `node_kind = call`
- `symbol_name = Vector`
- `visibility = private`

The expected reality was the enclosing public operator method:
`operator +`.

## Why It Matters

ME-JEPA action records would learn the wrong action target for operator overload
body edits. This corrupts symbol identity, reverse-reference features, and
visibility/API-contract features even though the C# parser itself succeeds.

## Resolution

The C# live action semantic walk now checks for an enclosing
`operator_declaration` or `conversion_operator_declaration` before accepting a
nested call/assignment node. Operator body edits are attributed to the operator
method, while ordinary non-operator method body calls keep their existing
nearest-node behavior.

## Related

- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `/tmp/contextgraph-csharp-action-fsv/csharp-action-tests.log`
