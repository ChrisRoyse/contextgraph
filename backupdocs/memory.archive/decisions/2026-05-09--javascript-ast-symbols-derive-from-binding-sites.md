---
namespace: decisions
created: 2026-05-09T04:23:22Z
updated: 2026-05-09T04:23:22Z
status: active
tags: mejepa,javascript,ast,tree-sitter,semantic-index,source-of-truth
---

# JavaScript AST Symbols Derive From Binding Sites

## Decision

JavaScript and TypeScript ME-JEPA AST extraction must derive function symbol
identity from the binding site that introduces the function, not from the first
identifier found inside the function node.

For JavaScript this means:

- `export const double = (x) => ...` persists `double`, not `x`.
- `const plusOne = function(value) { ... }` persists `plusOne`, not `value`.
- `module.exports.load = function(config) { ... }` persists `load`, not
  `config`.
- Object member functions derive from the owning `pair` key.
- Anonymous unbound callbacks do not mint top-level function symbols.
- Nested keyword nodes under `function_expression` do not create duplicate
  symbols.

## Rationale

Tree-sitter's own code-navigation convention pairs a definition capture with an
inner `@name` capture. For JavaScript assigned functions, the `@name` capture
lives on the enclosing declaration, assignment, or object pair, not on the
function's parameter list. ME-JEPA predictions need the symbol that exists in
repository reality, because reverse references, blast radius, action
attribution, and training examples are all keyed by that identity.

## Verification

The persisted semantic-index source of truth was read from
`/tmp/contextgraph-js-fsv.DqnqeB/happy/semantic-index.json`; it contained exactly
one function symbol each for `double`, `plusOne`, `run`, `stop`, and `load`.
The sibling evidence directory
`/tmp/contextgraph-js-fsv.DqnqeB/happy/semantic-index.json.evidence/` contains
the real `git rev-parse HEAD` and `node_modules/.bin/tsc --noEmit --pretty
false` command evidence.

## Related

- Discovery: `memory/discoveries/2026-05-09--javascript-ast-binding-symbol-mismatch.md`
- Project doc: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
