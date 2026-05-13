---
namespace: decisions
agent_id: agent-document-architect
created: 2026-05-09T03:00:00Z
updated: 2026-05-09T03:00:00Z
status: active
severity: high
tags: directive, mejepa, ast-chunker, tree-sitter, atomic-chunking, embedders, 11-languages, perfect-atomic-parsing
---

# AST × Embedder Atomic Chunking: perfect atomic parsing across 11 languages

## Operator directive (2026-05-09)

> "we have ast parsers set up for i believe 10 coding languages. we are going to want to
> set up our embedders to work with these ast parsers to perfectly chunk all code for
> perfect atomic parsing of code. add all this into the plans as well into all
> plannning docs where needed inside /home/cabdru/contextgraph/docs/ruvectorfindings
> directory."

## What this decision locks

1. **The 11 wired tree-sitter parsers** (Rust, Python, JS, TS, Go, Java, C, C++, C#,
   Ruby, PHP) become the canonical *atomic-chunk source* for ME-JEPA's panel
   materialization. Every code change is decomposed into AST chunks at the
   function/method/class/struct/impl-block/module level **before** being fed to the 14
   frozen embedders.
2. **Each AST chunk produces an independent 14-embedder panel** with full DDA 105-signal
   resolution. Whole-file embedding is replaced by per-chunk embedding.
3. **Signal yield multiplies by ~10×** (typical chunks-per-file ratio). Per-day per
   Claude Code instance: ~5M-20M signals (vs ~1M at file granularity).
4. **Constellation granularity sharpens to (mutation_category × language × node_type)**:
   6,048 centroids = 14 embedders × 8 categories × 9 languages × 6 node types. Computed
   once at Phase 0 corpus finalization; persisted to `CF_MEJEPA_CONSTELLATIONS`; never
   updated.
5. **The runtime guard `Gτ` operates at per-chunk precision.** Refusals include
   chunk-id, AST-node-type, language, failed modalities — a structural diagnostic, not
   an opaque scalar.
6. **Module-level cross-chunk constellation** catches integration-level bugs that
   per-chunk checks miss.
7. **All instruments (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff,
   E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning)
   operate at AST-chunk granularity** — natively, by reverse-mapping global signal to
   chunk, or by sharing across chunks per attempt.

## Existing infrastructure (current state)

- `crates/context-graph-core/src/memory/ast_chunker.rs` — shipped Rust chunker with
  `EntityType {Function, Method, Class, Struct, Module, Impl, ...}`; FSV at
  `/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`.
- `crates/context-graph-cli/Cargo.toml` — 11 `tree-sitter-*` parser dependencies.
- `crates/context-graph-mejepa-corpus/` — Python AST mutation operators
  (`subtle_flip`, `off_by_one`, etc.) already operate at chunk granularity.
- Per-language symbol-identity rules per `09_END_GOAL_REVIEW_REPLACEMENT.md` §3.1
  (Python parser-backed input gate; Rust function/method classification; JS/TS
  binding-site naming; C declarator-aware extraction; C++ template/namespace; Java
  JUnit annotations; C# operator-overload identity; Ruby DSL methods).

## What remains (per doc 14 §6.2 implementation roadmap)

| Step | Description | Effort | Status |
|---|---|---|---|
| 1 | Generalize `AstCodeChunker` to all 11 languages (per-language `new_*` constructors with node-kind tables) | ~5 days for shared pattern + per-language tables | deferred |
| 2 | Per-language test extraction → chunk reverse-map | ~2 days/language | deferred |
| 3 | Per-language compiler/LSP backed type evidence | partial (C, C++, Java, C#, Ruby shipped per doc 09 §3.1) | partial |
| 4 | `mejepa_compute_panel(file_path, chunk_filter)` MCP tool | ~2 days | deferred |
| 5 | Per-AST-node-type constellation centroids (6,048) | ~3 days | deferred |
| 6 | Per-chunk `Gτ` with chunk-id-typed errors | ~3 days | deferred |
| 7 | Module-level cross-chunk constellation | ~3 days | deferred |
| 8 | E_Diff at AST granularity (tree-sitter pre/post diff) | ~5 days | deferred |
| 9 | E_Trace per-chunk: pytest --trace / cargo --nocapture parsing | ~5 days | deferred |
| 10 | E_Reasoning per-chunk: tool-use → chunk SHA reverse-map | ~3 days | deferred |

**Total ~50-70 days for full per-AST-chunk integration across all 11 languages.**
Sequence by language priority: Python ✓, Rust ✓, then C/C++/Java/C# (compile_commands
infra exists), then JS/TS/Ruby/Go/PHP.

## Why this is structurally critical

Without AST-atomic chunking:
- Whole-file panels are 99% noise (unchanged code) for typical 3-line bug fixes.
- ΔP (UTML v1.0 §III) is dominated by noise — predictor learns to ignore most of input.
- Constellations at file-level conflate Function and Class structures.
- Verdict is opaque ("the file is wrong") rather than diagnostic ("function Foo failed
  E5+E7 constellation").

With AST-atomic chunking:
- Each chunk is its own panel with full embedder resolution on the changed structure.
- ΔP is sharp — the predictor sees the actual change.
- Constellations at (category × language × node_type) granularity sharpen Gτ.
- Verdict is structural — operators see exactly which functions/classes failed which
  embedder constellations.

This is what makes ME-JEPA's verdict **diagnosable** rather than opaque.

## Cross-references that need updating

- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §2.2 (instruments operate
  at chunk granularity) and §3.1 (cross-language mutation operators).
- `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` §C (cross-language mandate
  has 11 wired parsers).
- `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` (L_step computed per chunk).
- `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` §1.2 (signal yield 10× higher at
  chunk granularity).
- `docs/ruvectorfindings/13_UNIVERSAL_AGENT_VERIFICATION.md` §2 (verifier operates at
  chunk granularity; verdict structure is `{chunks: [(chunk_id, verdict)]}`).
- `docs/ruvectorfindings/sitrep/01_status_overview.md` — add Phase 1 line items for
  per-language chunker generalization.
- `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md` —
  cross-language mandate is satisfied by the 11 wired parsers.
- `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md` —
  embedders operate at AST-chunk granularity.

## Falsification (per UTML v1.0 §XIII)

1. **AST-atomic chunking degrades signal quality** if per-chunk panels are too small.
   Mitigation: existing `merge_small_chunks` in `ast_chunker.rs`. Falsifies if
   linear-probe accuracy on chunk panels is ≥10pp below file panels.
2. **Per-AST-node-type constellations don't separate categories.** Falsifies if
   (KnownGood, Python, Function) centroid is closer to (KnownGood, Python, Method)
   than to (SubtleFlip, Python, Function). Mitigation: collapse to (category, language)
   granularity.
3. **Cross-language chunker generalization too costly (>50 days).** Mitigation:
   sequence by language priority.
4. **Module-level cross-chunk guard adds no value.** Falsifies if single-chunk Gτ
   correlates ≥0.95 with whole-module Gτ. Mitigation: skip module-level guard.

## Related

- `docs/ruvectorfindings/14_AST_EMBEDDER_ATOMIC_CHUNKING.md` — public-facing doc.
- `crates/context-graph-core/src/memory/ast_chunker.rs` — existing chunker code.
- `crates/context-graph-cli/Cargo.toml` — 11 tree-sitter parser deps.
- `crates/context-graph-mejepa-corpus/` — Python mutation operators (chunk-granular).

## Tags

`directive` `mejepa` `ast-chunker` `tree-sitter` `atomic-chunking` `embedders`
`11-languages` `perfect-atomic-parsing` `per-chunk-panel` `chunk-granular-constellations`
`module-level-cross-chunk-guard` `structural-diagnostic-verdict`
