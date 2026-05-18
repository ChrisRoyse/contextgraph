---
namespace: decisions
created: 2026-05-09T04:32:00Z
updated: 2026-05-09T04:32:00Z
status: active
tags: mejepa,typescript,tsx,ast,compiler-evidence,fail-closed
---

# TypeScript ME-JEPA Parser Harness Fails Closed

## Decision

TypeScript and TSX ME-JEPA semantic extraction must use the exact tree-sitter
dialect for the file suffix and must require project-local
`node_modules/.bin/tsc` for compiler-backed evidence.

`--language typescript` intentionally expands to both detected `typescript` and
`tsx` files, because a TypeScript project commonly contains both. The harness
runs one real `tsc --noEmit --pretty false` check for the TypeScript family and
stores the physical stdout/stderr evidence once.

## Consequences

- `.tsx` files parse with `tree_sitter_typescript::LANGUAGE_TSX`; `.ts`,
  `.mts`, and `.cts` parse with the TypeScript grammar.
- Missing local `node_modules/.bin/tsc` is a hard `VERIFICATION_FAILED` /
  `RUST_REALITY_DYNAMICJEPA_TYPESCRIPT_TOOLCHAIN_MISSING` condition.
- Syntax or type-check failures stop the semantic index before writing output,
  while preserving evidence under the output `.evidence/` directory.
- Empty TSX is refused even if `tsc` has no diagnostic, because no semantic
  facts can be reality-grounded from whitespace.
- Live action attribution climbs from inner expressions to the nearest
  TypeScript owner (`function`, `method`, `class`, `interface`, or `type`).

## Verification

Manual FSV used real temp git repositories and TypeScript 5.9.3 installed via
`npm install`. Readback verified:

- Happy path output:
  `/tmp/contextgraph-ts-fsv-20260509/happy/semantic-index-fixed.json`
- Missing local `tsc` edge:
  `/tmp/contextgraph-ts-fsv-20260509/missing-toolchain/direct.stdout`
- Invalid TypeScript syntax edge:
  `/tmp/contextgraph-ts-fsv-20260509/invalid-syntax/direct.stdout`
- Empty TSX edge:
  `/tmp/contextgraph-ts-fsv-20260509/empty-tsx/direct.stdout`
