---
namespace: discoveries
agent_id: codex-tsx
timestamp: 2026-05-09T03:51:34Z
status: active
tags: mejepa,tsx,ast,tree-sitter,dynamicjepa,root-cause
---

# TSX Semantic Index Used The TypeScript Grammar

## What Happened

The DynamicJEPA semantic-index source discovery path classified `.tsx` files as
`typescript`. A real temp git repository containing a valid `src/Button.tsx`
with JSX then failed closed before writing the semantic index:

`tree-sitter typescript parse reported syntax errors in src/Button.tsx`

The source of truth for the reproduction is
`/tmp/contextgraph-tsx-repro-before-3015429/repo/src/Button.tsx`; the command
left `/tmp/contextgraph-tsx-repro-before-3015429/semantic-index.json` absent and
created command evidence under the sibling evidence directory.

## Why It Matters

TSX is the React/JSX-bearing dialect of TypeScript. Treating `.tsx` as plain
TypeScript means valid UI source becomes unusable to ME-JEPA `E_AST`, while
wrong parser errors can be mistaken for broken code reality. It also prevents
requesting `--language tsx` because detected languages never contain `tsx`.

## Root Cause

`semantic_language_for_path` returned `Some("typescript")` for both `.ts` and
`.tsx`, even though `semantic_tree_sitter_language` already has a separate
`tree_sitter_typescript::LANGUAGE_TSX` branch. The live action path already
distinguishes `.tsx`; the semantic-index path did not.

## Related

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `crates/context-graph-cli/src/bin/reality-loop.rs`
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
