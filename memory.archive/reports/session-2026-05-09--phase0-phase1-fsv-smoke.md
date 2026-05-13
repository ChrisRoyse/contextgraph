---
namespace: reports
created: 2026-05-09T10:00:00Z
updated: 2026-05-09T10:00:00Z
status: active
tags: report, session, mejepa, phase0, phase1, ast, embedders, corpus, fsv
---

# Session Report: 2026-05-09 — Phase 0/1 FSV Smoke

## Task: Complete Phase 0 scope and Phase 1 AST/embedder integration with full state verification.
## Outcome: complete for implementation and smoke/FSV; the full 2,400-entry Docker batch remains an intentionally separate long operational run.

## Summary

Phase 1 now has a reusable core AST chunker/routing source of truth used by the ME-JEPA code instruments. The core AST module supports the 11-language Phase 1 set, fail-closed parsing, mixed-language rejection for PHP/HTML, deterministic structural SHAs, entity classification, and a static embedder routing table covering all 13 entity types across all 11 languages.

The root causes fixed were split AST ownership between CLI/core/instruments, unnamed tree-sitter tokens being emitted as Ruby chunks, broad test-path classification mislabeling helper functions as test functions, and comment chunks with identical structural SHAs. Instruments now call the core chunker and propagate explicit MEJEPA error codes rather than accepting recovered parse trees or missing routes.

Full state verification evidence was generated and read back from files/RocksDB. The one-task Docker smoke for Phase 0 generated 8 mutation entries and 8 oracle verdicts for `sympy__sympy-20590`, then verified the RocksDB column families and evidence JSONs. The expected full Phase 0 count remains 2,400, and `full_phase0_complete=false` for that smoke by design.

## Files Created or Modified in the Repo

- `Cargo.lock` — dependency lock refresh.
- `crates/context-graph-core/src/memory/mod.rs` — exported the core AST chunking and routing APIs.
- `crates/context-graph-core/src/memory/ast/mod.rs` — added fail-closed routing validation and structural SHA hardening.
- `crates/context-graph-core/src/memory/ast/classify.rs` — hardened named-node filtering and test function classification.
- `crates/context-graph-core/src/memory/ast/routing.rs` — added E1-E21 embedder ids, projected dimensions, routing keys/results, and direct instrument routing.
- `crates/context-graph-core/src/memory/ast/tests.rs` — added route coverage, language/entity, hash, parse, and classification tests.
- `crates/context-graph-core/examples/ast_chunker_multilang_fsv.rs` — new 11-language AST FSV harness.
- `crates/context-graph-core/examples/embedder_routing_fsv.rs` — new routing coverage FSV harness.
- `crates/context-graph-core/examples/swebench_ast_fsv.rs` — new real SWE-bench AST FSV harness.
- `crates/context-graph-mejepa-instruments/src/e_code.rs` — routed code instruments through the core chunker and mapped routing failures.
- `crates/context-graph-mejepa-instruments/src/tests.rs` — updated unsupported-language and multi-language instrument coverage.
- `crates/context-graph-mejepa-instruments/examples/ast_embedder_smoke_fsv.rs` — new Phase 1 AST/embedder panel smoke.
- `crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs` — hardened evidence permissions.
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs` — hardened evidence permissions.

## Files Written or Updated in ./memory/

- `discoveries/2026-05-09--phase1-core-ast-routing-and-fsv-source-of-truth.md` — root causes, source-of-truth files, and Phase 1 ownership guidance.
- `journal/2026-05-09--success-phase0-phase1-fsv-smoke.md` — verification commands and success note.
- `reports/session-2026-05-09--phase0-phase1-fsv-smoke.md` — this report.

## Decisions Made

- Use `context-graph-core::memory::ast` as the canonical AST chunker/routing source for ME-JEPA code instruments. Rationale: prevents divergent parser/classifier behavior between CLI, instruments, and future training/evaluation code.
- Fail closed on parser errors, missing tokens, mixed embedded language, and missing embedder routes. Rationale: recovered ASTs and silent route gaps would poison corpus/training data.

## Discoveries

- Ruby parser keyword tokens can look like class nodes unless unnamed nodes are filtered.
- Comment chunks need comment text in their stable structural hash, otherwise distinct real SWE-bench comments can collide.
- Test-file path alone is too broad for function classification; helpers in tests must stay helpers unless names/attributes/DSL indicate test semantics.

## Verification

- Core AST unit tests: passed 10/10.
- Core multi-language AST FSV: 11 languages passed; edge cases passed for empty input, invalid Python, and mixed PHP/HTML.
- Embedder routing FSV: 143 rows read back, covering 11 languages times 13 entity types.
- Real SWE-bench AST FSV: `sympy__sympy-20590` produced 20 chunks; 9 comment chunks had 9 unique SHAs.
- Instruments test suite: passed 45/45.
- Phase 1 AST/embedder panel FSV: passed with 11 sample reports, `panel_dim=5120`, and `panel_filled_mask=32767`.
- Phase 0 corpus tests: passed 87/87.
- Phase 0 corpus FSV: passed with 300 real manifests and split counts instance `[240,30,30]`, repo-atomic `[195,77,28]`.
- Phase 0 Docker smoke: passed with 8 entries, 8 mutation CF rows, 8 oracle verdict CF rows, and no verification failures.

## Residual Risk

The full 2,400-entry Docker batch was not run in this session because it is a long sequential operational job. The smoke proves the generation, oracle, RocksDB store, verification, stats, and SWE-bench AST path on one real task, but the full run can still expose task-specific Docker/oracle variance. Also, mutation category names should not be treated as guaranteed pass/fail labels; the oracle verdict is the source of truth.

## Related

- See also: `memory/discoveries/2026-05-09--phase1-core-ast-routing-and-fsv-source-of-truth.md`
- Tags: mejepa, phase0, phase1, ast, embedders, corpus, fsv
