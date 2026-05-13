---
namespace: discoveries
created: 2026-05-09T03:20:50Z
updated: 2026-05-09T03:20:50Z
status: active
severity: high
tags: cpp, mejepa, ast, tree-sitter, action-harness, fsv
---

# C++ Action Range Whitespace Can Misattribute Methods As Classes

## What Happened
The reality-loop ME-JEPA action harness used the full governed line-window byte range for `tree_sitter::Node::descendant_for_byte_range`. For an indented C++ class-member method signature, the full range included leading spaces and the trailing newline. Tree-sitter C++ method nodes do not own the leading indentation bytes, so the smallest enclosing node became the surrounding `class_specifier`.

The failing FSV readback showed an edit to `int add(int value) {` classified as:

- `ast_node_kind = class`
- `ast_edit_kind = class_body`
- `symbol_bucket = class`

The correct reality state is:

- `ast_node_kind = method`
- `ast_edit_kind = signature`
- `symbol_bucket = function`

## Why It Matters
ME-JEPA action prediction trains on these action records. If C++ method-signature edits are labeled as class-body edits, the model learns the wrong causal target and cannot reliably predict whether an AI patch changes an API contract. The fix is to use a significant token span for AST node lookup while preserving the raw edited line text for patch-delta and changed-text analysis.

## Resolution
`crates/context-graph-cli/src/bin/reality-loop.rs` now trims leading and trailing whitespace only for AST node selection via `ast_significant_byte_range`. The raw changed text is still used for patch-delta features.

Manual FSV evidence was persisted at:

- `/tmp/contextgraph-cpp-ast-harness-fsv/cpp-action-tests.log`

That log contains the before/after C++ header state and the re-read action evidence JSON showing `ast_node_kind = method`, `ast_edit_kind = signature`, and `semantic_provider = compiler_lsp_compile_commands`.

## Related
- See also: ./memory/decisions/2026-05-09--cpp-compile-commands-fail-closed.md
- Tags: cpp, ast, tree-sitter, action-attribution, mejepa
