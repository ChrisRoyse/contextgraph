---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T03:53:56Z
status: active
tags: c, ast, mejepa, semantic-index, compile-commands, fsv
---

# C AST extraction must be declarator-aware

The C ME-JEPA parser harness cannot reuse the generic C++ `function_declarator`
and compound-specifier mapping.

Root cause found in `context-graph-cli`:

- `function_definition` and its nested `function_declarator` were both indexed
  as symbols, so real C functions could be double-counted.
- The nested `function_declarator` did not include the outer `static`, so a
  private C function could also appear as a public duplicate.
- `struct_specifier` was treated as a declaration even when it had no `body`;
  `struct Point p` in a function body therefore minted a false second `Point`
  type declaration.
- C function prototypes were classified as generic `declaration` dataflow.
- Macro definitions were not first-class semantic symbols even though C
  reality often depends on preprocessor definitions.

Correct invariant:

- `function_definition` owns nested `function_declarator` symbols.
- A `function_declarator` outside `function_definition` is a prototype/function
  declaration.
- A `declaration` containing a `function_declarator` is not dataflow.
- `struct_specifier`, `enum_specifier`, and `union_specifier` are type
  declarations only when the node has a `body`.
- `preproc_def` and `preproc_function_def` are symbols.
- C compiler-backed evidence must come from structured `compile_commands.json`
  `arguments`, never a guessed `gcc file.c` command or shell-string `command`.

Manual source-of-truth evidence for this discovery was written under
`/tmp/contextgraph-c-ast-fsv.eTmgwU/semantic-index.json` and
`/tmp/contextgraph-c-ast-fsv.eTmgwU/semantic-index.json.evidence/`.
