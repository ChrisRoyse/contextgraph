# Phase 1a — AST Chunking Task Index

> **2026-05-10 repo audit update:** Phase 1 is no longer greenfield. The current multi-language AST chunker exists in `crates/context-graph-core/src/memory/ast/` and is wired into ME-JEPA code instruments. Treat the 22 task files as audit/hardening tasks against that implementation.

## Occam Answer

Do not create a second `ast_chunker/` directory module. The simple correct path is to harden the existing canonical module, align all callers and docs to it, delete or isolate stale legacy surfaces when they block correctness, and verify behavior through persisted evidence. The old Rust-only `crates/context-graph-core/src/memory/ast_chunker.rs` still exists, but it is legacy and must not be extended as the Phase 1 multi-language source of truth.

## Canonical State

- **Canonical AST module:** `crates/context-graph-core/src/memory/ast/{mod.rs,classify.rs,routing.rs,tests.rs}`
- **Public API:** `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options, AstChunkOptions, Language, EntityType, AstChunk, ParseDiagnostic, AstChunkerError}`
- **Languages:** `rust`, `python`, `javascript`, `typescript`, `go`, `java`, `c`, `cpp`, `csharp`, `ruby`, `php`
- **Entity types:** `Function`, `Method`, `Class`, `Struct`, `Enum`, `TraitOrInterface`, `Impl`, `Module`, `Namespace`, `TestFunction`, `Import`, `CommentBlock`, `Docstring`
- **Routing:** `crates/context-graph-core/src/memory/ast/routing.rs` maps `EntityType` and special routing keys to `EmbedderId::E1..E21` or direct instruments. Missing routes are typed failures, not defaults.
- **ME-JEPA consumer:** `crates/context-graph-mejepa-instruments/src/e_code.rs` already imports the canonical AST API and must keep failing closed on parse/chunk/routing errors.
- **Phase 0 real corpus source:** `/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509`
- **Phase 0 verification evidence:** `verify-evidence.json` in that corpus passed with 2400 entries and RocksDB CF counts `2400/2400/1`.
- **Phase 0 final corpus SHA:** `f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33`

## Best-Practice Notes

- Tree-sitter represents parse recovery through `ERROR` and `MISSING` nodes; those must be surfaced as diagnostics and fail closed, not chunked as partially valid source. See [Tree-sitter query syntax: special nodes](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html) and [tree-sitter parse](https://tree-sitter.github.io/tree-sitter/cli/parse.html).
- Classifiers should be grounded in grammar node types/fields, not fragile text scans. Tree-sitter publishes structured node type metadata via `node-types.json`; use it when adding or auditing language coverage. See [Tree-sitter static node types](https://tree-sitter.github.io/tree-sitter/using-parsers/6-static-node-types).
- Structure-aware chunking is the right strategy for downstream RAG/ME-JEPA use: preserve syntactic units, split large AST units recursively when needed, and merge related siblings only when size limits require it. The cAST paper reports gains from AST-respecting chunks over line-based chunks for retrieval and SWE-bench generation. See [cAST on arXiv](https://arxiv.org/abs/2506.15655).

## Non-Negotiables

- **No backwards compatibility work.** Do not preserve stale APIs just to keep old imports alive. Update callers to the canonical API or fail fast with actionable errors.
- **No fallbacks.** Unsupported language, empty input, mixed language, invalid UTF-8, Tree-sitter `ERROR`/`MISSING`, and routing gaps must return typed errors with enough context to debug.
- **No mock data in verification.** Use real `samplecode/` files, real Phase 0 corpus artifacts, real parser/toolchain checks, and persisted JSON/RocksDB evidence.
- **Determinism required.** Output ordering is source order; route sets and serialized maps must be deterministic (`BTreeSet`/ordered data structures).
- **Fail loudly.** Every failure path should include language, path, reason, and diagnostics or command stdout/stderr where applicable.

## Task Status

| Task | Current focus | Canonical file(s) | Status |
|---|---|---|---|
| TASK-AST-001 | `Language` enum and slug/path parsing | `ast/mod.rs` | Implemented; audit/harden |
| TASK-AST-002 | `EntityType` and classification context | `ast/mod.rs`, `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-003 | `AstChunk`, `ParseDiagnostic`, serde shape | `ast/mod.rs` | Implemented; audit/harden |
| TASK-AST-004 | `AstChunkerError` typed failures/codes | `ast/mod.rs`, `ast/routing.rs`, `e_code.rs` | Implemented; audit/harden |
| TASK-AST-005 | Embedder routing table | `ast/routing.rs` | Implemented; audit/harden |
| TASK-AST-006 | Reformat-invariant chunk hash | `ast/mod.rs::structural_sha256` | Implemented; audit/harden |
| TASK-AST-007 | Boundary walk/chunk emission | `ast/mod.rs::walk_and_emit`, `build_chunk` | Implemented; needs size/merge audit |
| TASK-AST-008 | Parse diagnostics | `ast/mod.rs::parse_diagnostics` | Implemented; audit/harden |
| TASK-AST-009 | Mixed-language rejection | `ast/mod.rs::detect_mixed_language` | Implemented; audit/harden |
| TASK-AST-010 | Language -> parser dispatch | `ast/mod.rs::tree_sitter_language` | Implemented; audit/harden |
| TASK-AST-011 | Python classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-012 | JavaScript classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-013 | TypeScript/TSX classifier | `ast/classify.rs`, `tree_sitter_language` | Implemented; audit/harden |
| TASK-AST-014 | Go classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-015 | Java classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-016 | C classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-017 | C++ classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-018 | C# classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-019 | Ruby classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-020 | PHP classifier | `ast/classify.rs` | Implemented; audit/harden |
| TASK-AST-021 | Canonical facade + integration | `ast/mod.rs`, `e_code.rs`, `ast/tests.rs` | Implemented; audit/harden |
| TASK-AST-022 | FSV harnesses and persisted evidence | `examples/ast_chunker_multilang_fsv.rs`, `examples/swebench_ast_fsv.rs`, `mejepa-instruments/examples/ast_embedder_smoke_fsv.rs` | Implemented; audit/harden |

## Current Evidence Commands

Run these from `/home/cabdru/contextgraph` after touching Phase 1:

```bash
cargo test -p context-graph-core memory::ast::tests
cargo test -p context-graph-mejepa-instruments
cargo run -p context-graph-core --example ast_chunker_multilang_fsv
cargo run -p context-graph-core --example swebench_ast_fsv
cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv
```

Required persisted evidence paths:

- `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`
- `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`
- `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`

## Full State Verification Contract

Every task must end with FSV:

1. **Define source of truth.** Name the actual file/database/table/JSON/RocksDB CF/panel where the result is stored.
2. **Execute and inspect.** Run the logic, then perform an independent readback from the source of truth. Do not rely only on function return values.
3. **Happy path.** Use real source files or real Phase 0 corpus data. Verify expected chunks, routes, diagnostics, and persisted bytes.
4. **Three edge cases.** Print before and after system state for `empty_source`, `invalid_syntax`, and `mixed_language_or_unsupported_language`.
5. **Evidence log.** Persist and print actual stored data: chunk rows, `sha256`, line spans, `entity_type`, route sets, diagnostics, and command stdout/stderr on failures.

## Parallel Batches

- **Audit batch A:** TASK-AST-001..010 can be checked together because they share `ast/mod.rs` and `ast/routing.rs`.
- **Audit batch B:** TASK-AST-011..020 can be checked as one classifier audit because the current implementation consolidates classifiers in `ast/classify.rs`.
- **Audit batch C:** TASK-AST-021..022 should run after A/B because they validate the public facade, ME-JEPA integration, real corpus path, and evidence readback.

## Cross-References

- Functional spec: `../../functional/SPEC-AST-CHUNKING.md`
- Technical spec: `../../technical/TECH-AST-CHUNKING.md`
- Constitution: `../../constitution.md`
- Roadmap source: `../../14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- Traceability: `../_traceability.md`
- Master DAG: `../_index.md`
- Recent implementation commits: `63acaa1a phase1(mejepa): 11-language AST chunker + rewire code instruments`, `acc6f78c phase1(mejepa): embedder routing API + 21-embedder enum + spec/task tree + E14 fail-closed`
