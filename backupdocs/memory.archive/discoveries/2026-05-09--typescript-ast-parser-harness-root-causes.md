---
namespace: discoveries
agent_id: codex-typescript
timestamp: 2026-05-09T04:31:00Z
status: active
tags: mejepa,typescript,tsx,ast,tree-sitter,dynamicjepa,root-cause
---

# TypeScript AST Parser Harness Root Causes

## Root Causes

1. The semantic-index path classified `.tsx` as `typescript`, even though
   tree-sitter TypeScript exposes separate TypeScript and TSX grammars. Valid
   JSX therefore failed closed as a TypeScript syntax error before any symbol
   graph could be written.
2. TypeScript assigned functions reused generic JavaScript-ish first-identifier
   lookup. For `export const double = (x: number) => ...` and
   `export const plusOne = function(value: number) { ... }`, this could persist
   parameter identifiers instead of the binding symbol.
3. Tree-sitter TypeScript represents function expressions with nested
   `function` nodes. Without a TypeScript-specific skip, one source binding
   could produce duplicate symbols or a false `default` symbol.
4. Type/interface declarations were not first-class semantic kinds in the
   TypeScript parser harness, so ME-JEPA `E_TypeGraph` lost important declared
   surface area.
5. Live action attribution stopped at inner expressions such as `call_expression`
   instead of climbing to the enclosing TypeScript owner, so edits inside a
   default-exported function body could be recorded as a call/module action.
6. TypeScript compiler evidence allowed parser-only/global-tool behavior in
   places. For ME-JEPA prediction, TypeScript/TSX now requires the repository's
   project-local `node_modules/.bin/tsc`.

## Source Of Truth

Manual FSV source of truth:

- Happy path semantic index:
  `/tmp/contextgraph-ts-fsv-20260509/happy/semantic-index-fixed.json`
- Happy path evidence directory:
  `/tmp/contextgraph-ts-fsv-20260509/happy/semantic-index-fixed.json.evidence/`
- Edge case command stdout/stderr:
  `/tmp/contextgraph-ts-fsv-20260509/missing-toolchain/direct.stdout`
  `/tmp/contextgraph-ts-fsv-20260509/invalid-syntax/direct.stdout`
  `/tmp/contextgraph-ts-fsv-20260509/empty-tsx/direct.stdout`

The happy path readback contains `tsx` and `typescript` languages, no parameter
symbols (`x`, `value`, `name`, `count`), no duplicate symbols, and exported
symbols for `Widget`, `WidgetProps`, `CalculatorOptions`, `UserId`, `double`,
`plusOne`, and `default`.

## Related

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `memory/discoveries/2026-05-09--tsx-semantic-index-uses-typescript-grammar.md`
