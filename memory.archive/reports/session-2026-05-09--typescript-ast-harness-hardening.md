---
namespace: reports
agent_id: codex-typescript
timestamp: 2026-05-09T04:35:00Z
status: complete
tags: mejepa,typescript,tsx,ast,semantic-index,fsv
---

# TypeScript AST Harness Hardening Report

## Scope

Focused on TypeScript/TSX only for the ME-JEPA DynamicJEPA parser/action
harness. Other language changes observed in the workspace were treated as
concurrent work and not reverted.

## Root Cause Summary

- `.tsx` was being discovered as `typescript` in semantic-index, while the
  action path already used the TSX grammar.
- TypeScript assigned functions needed binding-site symbol identity so
  parameters like `x`, `value`, `name`, and `count` never become function
  symbols.
- TypeScript nested `function` grammar nodes needed to be suppressed to avoid
  duplicate/bogus symbols while preserving real anonymous default exports as
  `default`.
- Interfaces and type aliases needed first-class type graph symbols.
- Live action attribution needed to climb from inner expressions to the nearest
  TypeScript semantic owner.
- TypeScript/TSX compiler evidence needed to require project-local
  `node_modules/.bin/tsc` and refuse parser-only/global fallback.

## Research Applied

- tree-sitter TypeScript documents TypeScript and TSX as separate grammars.
- tree-sitter `Node::has_error`, `is_error`, and `is_missing` support
  fail-closed parse diagnostics.
- TypeScript `noEmit` is the intended compiler option for type-checking without
  writing JavaScript/declaration outputs.

## Source Of Truth

Happy-path manual FSV:

- Output JSON:
  `/tmp/contextgraph-ts-fsv-20260509/happy/semantic-index-fixed.json`
- Evidence directory:
  `/tmp/contextgraph-ts-fsv-20260509/happy/semantic-index-fixed.json.evidence/`

Readback facts:

- `languages`: `["tsx", "typescript"]`
- symbol count: `9`
- symbols: `Widget`, `WidgetProps`, `Element`, `IntrinsicElements`,
  `CalculatorOptions`, `UserId`, `default`, `double`, `plusOne`
- bad parameter symbols: `[]`
- duplicate symbols: `[]`
- project-local TypeScript evidence:
  `node_modules/.bin/tsc --noEmit --pretty false`
- `typescript_tsc.stdout` bytes: `0`
- `typescript_tsc.stderr` bytes: `0`

Manual edge source-of-truth logs:

- Missing local tsc:
  `/tmp/contextgraph-ts-fsv-20260509/missing-toolchain/direct.stdout`
  - status `1`
  - semantic output absent
  - evidence directory present with git evidence
  - source unchanged
- Invalid TypeScript syntax:
  `/tmp/contextgraph-ts-fsv-20260509/invalid-syntax/direct.stdout`
  - status `1`
  - semantic output absent
  - `typescript_tsc.stdout` contains
    `src/broken.ts(1,31): error TS1109: Expression expected.`
  - source unchanged
- Empty TSX:
  `/tmp/contextgraph-ts-fsv-20260509/empty-tsx/direct.stdout`
  - status `1`
  - semantic output absent
  - `typescript_tsc.stdout` and `.stderr` empty because the compiler accepted
    whitespace
  - parser refused empty TSX with a fail-closed diagnostic
  - source unchanged

## Verification Commands

- `cargo fmt --all`
- `cargo test -p context-graph-cli --no-default-features --lib typescript_semantic_extraction_uses_tsx_dialect_and_binding_symbols -- --nocapture`
- `cargo test -p context-graph-cli --no-default-features --lib tsx_semantic_extraction -- --nocapture`
- `cargo test -p context-graph-cli --no-default-features --lib typescript_semantic_index_requires_project_local_tsc -- --nocapture`
- `cargo test -p context-graph-cli --no-default-features --bin reality-loop dynamicjepa_typescript -- --nocapture`
- `target/debug/deps/context_graph_cli-425ba30a0168ed8d tsx_semantic_extraction --nocapture`
- `target/debug/deps/context_graph_cli-425ba30a0168ed8d typescript_semantic_index_requires_project_local_tsc --nocapture`
- `target/debug/deps/reality_loop-fd57860eb98cbc76 dynamicjepa_typescript --nocapture`
- `target/debug/deps/context_graph_cli-c0ea3c14d73ef84d dynamicjepa build-semantic-index --repo /tmp/contextgraph-ts-fsv-20260509/missing-toolchain/repo --output /tmp/contextgraph-ts-fsv-20260509/missing-toolchain/semantic-index-direct.json --language typescript --json`
- `target/debug/deps/context_graph_cli-c0ea3c14d73ef84d dynamicjepa build-semantic-index --repo /tmp/contextgraph-ts-fsv-20260509/invalid-syntax/repo --output /tmp/contextgraph-ts-fsv-20260509/invalid-syntax/semantic-index-direct.json --language typescript --json`
- `target/debug/deps/context_graph_cli-c0ea3c14d73ef84d dynamicjepa build-semantic-index --repo /tmp/contextgraph-ts-fsv-20260509/empty-tsx/repo --output /tmp/contextgraph-ts-fsv-20260509/empty-tsx/semantic-index-direct.json --language typescript --json`

## Notes

The user-provided sudo credential was not used, stored, or required. Build
pressure from other language agents caused Cargo lock/native build delays; direct
execution of freshly built test/CLI binaries was used for final manual edge
verification.
