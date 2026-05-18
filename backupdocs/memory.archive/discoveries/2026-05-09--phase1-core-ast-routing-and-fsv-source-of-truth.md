---
namespace: discoveries
created: 2026-05-09T10:00:00Z
updated: 2026-05-09T10:00:00Z
status: active
severity: high
tags: mejepa, phase1, ast, chunking, embedders, fsv, swebench
---

# Phase 1 Core AST Routing And FSV Source Of Truth

## What Happened

Phase 1 was not fully integrated because the reusable core AST chunker and the ME-JEPA instruments did not share one source of truth. Multi-language parser/classifier logic existed in CLI-oriented code, while `context-graph-core` and `context-graph-mejepa-instruments` were still effectively Python/Rust-limited in practice. This made it impossible to run one consistent AST chunking plus embedder routing smoke over the full Phase 1 language set.

The implementation now routes Phase 1 through `crates/context-graph-core/src/memory/ast/`:

- `mod.rs` owns the reusable AST chunker API, fail-closed parser validation, structural SHA, chunk metadata, and routing validation.
- `classify.rs` owns language-aware classification across Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP.
- `routing.rs` owns the static E1-E21 embedder id model plus routing for entity types and internal direct instruments.
- `tests.rs` verifies route coverage, test-function detection, parse failures, mixed PHP/HTML rejection, and structural hash behavior.

Two real root causes were found during FSV:

- Ruby tree-sitter emits unnamed keyword tokens such as `class`; those were being classified as entity chunks. The classifier now ignores unnamed nodes.
- Comment chunks were hashed structurally without comment text, so distinct comments in real SWE-bench code could share identical SHA values. Comment node text is now included in the stable structural token set.

An additional correctness issue was hardened: broad path-based test-file detection mislabeled ordinary helpers as test functions in languages such as JavaScript, Java, C#, and Python. The classifier now requires test-like names or explicit test attributes/DSLs instead of treating every function in a test path as a test.

## Source Of Truth

Evidence files were written and then read back independently:

- Multi-language AST chunker FSV: `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`
- Embedder routing coverage FSV: `/tmp/contextgraph-mejepa-embedders-fsv/routing_coverage.json`
- Real SWE-bench AST FSV: `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`
- Phase 1 AST/embedder smoke FSV: `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`
- Phase 1 extended instrument panel FSV: `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`
- Phase 0 corpus FSV: `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
- Phase 0 Docker one-task smoke: `/tmp/contextgraph-phase0-smoke-one-task/verify-evidence.json` and `/tmp/contextgraph-phase0-smoke-one-task/stats.json`

Readback highlights:

- AST chunker FSV covered 11 languages with chunk counts: Rust 6, Python 5, JavaScript 5, TypeScript 6, Go 8, Java 4, C 6, C++ 7, C# 4, Ruby 3, PHP 4.
- Routing coverage produced 143 rows: 11 languages times 13 entity types.
- Real SWE-bench AST FSV used `sympy__sympy-20590`, applied the Phase 0 known-good patch, compiled the real target Python file, and produced 20 chunks. The 9 comment chunks had 9 unique SHAs after the hash fix.
- AST/embedder smoke produced an E-panel with `panel_dim=5120`, `panel_filled_mask=32767`, 11 real language sample reports, and passing edge cases for empty source, invalid syntax, and mixed PHP/HTML.
- Phase 0 one-task Docker smoke wrote 8 mutation entries and 8 oracle verdict entries to RocksDB for `sympy__sympy-20590`.

## Why It Matters

Future Phase 2+ work should consume the core AST API and routing API, not reimplement parser logic in an instrument, CLI, or script. If a parser accepts an erroneous recovered tree, missing token, mixed embedded language, or entity type with no routing, that is a correctness failure and should remain fail-closed with an explicit MEJEPA error code.

## Related

- See also: `memory/reports/session-2026-05-09--phase0-phase1-fsv-smoke.md`
- Tags: mejepa, phase1, ast, chunking, embedders, fsv, swebench
