# 14 — AST-Aware Atomic Chunking for the Embedder Ensemble

> **Operator directive 2026-05-09:** *"we have ast parsers set up for i believe 10 coding languages. we are going to want to set up our embedders to work with these ast parsers to perfectly chunk all code for perfect atomic parsing of code. add all this into the plans as well into all planning docs where needed inside /home/cabdru/contextgraph/docs/ruvectorfindings directory."*

> **Companion docs:**
> - `12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md` — 21-embedder ensemble inventory
> - `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` — TCT/DDA framework
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` §2.2 — instrument set (E_AST, E_CFG, E_DataFlow, etc.)

---

## 0. Headline

**The ContextGraph workspace already has tree-sitter parsers for 11 languages wired in `crates/context-graph-cli/Cargo.toml`** (Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP). It also has:

- **`crates/context-graph-core/src/memory/ast_chunker.rs`** (1216 LOC) — cAST-methodology AST-aware chunker targeting ~500-char chunks per Qodo recommendation. **Currently Rust-only** (calls `tree_sitter_rust::LANGUAGE` directly).
- **`crates/context-graph-cli/src/commands/dynamicjepa.rs`** (~7000 LOC) — multi-language semantic extractor that DOES handle all 11 tree-sitter languages, with per-language symbol/binding/test classification + parse-diagnostics + tooling-evidence harnesses.

**The gap:** these two AST capabilities are not connected to the **embedder ensemble** (doc 12). The 21 embedders today receive raw text/source bytes; they should receive **AST-perfect atomic chunks** so each chunk fits cleanly inside a syntactic boundary (function, struct, class, impl, module-decl) and never splits mid-statement.

**This doc specifies the integration**: extend `ast_chunker.rs` to all 11 languages, reuse DynamicJEPA's per-language node-classification logic, and route AST-perfect chunks through every relevant embedder (E1, E5, E7, E8, E10, E11, E14 + the new E_AST / E_CFG / E_DataFlow / E_Diff instruments) so the panel materialization pipeline gets perfectly-bounded code units.

---

## 1. Why AST-aware chunking is load-bearing

### 1.1 Naïve text chunking corrupts code semantics

Most embedder pipelines slice source code by character count or token count. For natural language this is fine — sentence boundaries are loose. For code:

- A function split mid-body produces a chunk whose embedding represents a half-function — the syntactic context (parameters, return type, signature) is in chunk A, the body in chunk B, neither chunk represents a meaningful semantic unit.
- A `class Foo:` split from its methods produces a chunk that mentions a class name with no methods, and method-only chunks orphaned from their class definition.
- An `impl Display for Foo` split from `fn fmt` body is the same problem in Rust.
- A `function() => ...` arrow function in JS/TS that spans multiple lines is ambiguous; without AST, the chunker doesn't know where it ends.

The result: the embedder's similarity space becomes noisy. Two chunks that the human would consider semantically equivalent (because they ARE the same function, just split differently) end up far apart in latent space.

### 1.2 The Qodo / cAST recommendation

cAST (EMNLP 2025) and Qodo's blog post (`https://www.qodo.ai/blog/rag-for-large-scale-code-repos/`) both empirically establish:

1. **Target ~500 non-whitespace characters per chunk** for code embeddings (E7 Qodo is trained on this distribution).
2. **Preserve syntactic boundaries** (functions, structs, impls, classes) — never split a function in the middle of its body.
3. **Prepend context** (file path, scope chain, surrounding imports) so the chunk is interpretable in isolation.
4. **Include parent class/struct definition** with each method chunk so the method's `self` context is explicit.

The shipped `ast_chunker.rs` already implements all four points for Rust. The work in this doc is **multi-language extension** — apply the same logic to the 10 other tree-sitter parsers we already have.

### 1.3 Atomic parsing for ME-JEPA-Code's instruments

For ME-JEPA-Code's E_AST / E_CFG / E_DataFlow / E_TypeGraph / E_Diff instruments to work correctly:

- **E_AST** must encode the AST node-token sequence for each chunk. AST-perfect chunking guarantees the parse tree fits inside the chunk; non-AST chunking produces partial parse trees that misrepresent the code.
- **E_CFG** computes basic-block control flow within each chunk. A chunk that splits mid-function has an artificial CFG boundary that doesn't reflect the actual program flow.
- **E_DataFlow** tracks def-use chains. Variables defined in chunk A and used in chunk B produce broken def-use edges in non-AST chunking.
- **E_Diff** computes diff edges across chunk pairs. AST-perfect chunks let diff operate on syntactic units (added function, modified body, deleted method) instead of on text-line ranges.

Without AST-perfect chunking, these instruments produce noise; with it, they produce signal.

---

## 2. The 11 language matrix (current state)

| Language | tree-sitter crate | DynamicJEPA semantic extractor | E7 code-embedder chunker (`ast_chunker.rs`) | E_AST instrument (Phase 1) |
|---|---|---|---|---|
| Rust | `tree-sitter-rust` 0.23 | ✅ shipped | ✅ shipped | ⏳ deferred |
| Python | `tree-sitter-python` 0.23.6 | ✅ shipped | ❌ not extended | ⏳ deferred |
| JavaScript | `tree-sitter-javascript` 0.23.1 | ✅ shipped | ❌ not extended | ⏳ deferred |
| TypeScript / TSX | `tree-sitter-typescript` 0.23.2 | ✅ shipped | ❌ not extended | ⏳ deferred |
| Go | `tree-sitter-go` 0.23.4 | ✅ shipped | ❌ not extended | ⏳ deferred |
| Java | `tree-sitter-java` 0.23.5 | ✅ shipped | ❌ not extended | ⏳ deferred |
| C | `tree-sitter-c` 0.23.4 | ✅ shipped | ❌ not extended | ⏳ deferred |
| C++ | `tree-sitter-cpp` 0.23.4 | ✅ shipped | ❌ not extended | ⏳ deferred |
| C# | `tree-sitter-c-sharp` 0.23.5 | ✅ shipped | ❌ not extended | ⏳ deferred |
| Ruby | `tree-sitter-ruby` 0.23.1 | ✅ shipped | ❌ not extended | ⏳ deferred |
| PHP | `tree-sitter-php` 0.23.11 | ✅ shipped | ❌ not extended | ⏳ deferred |

**The work**: extend `ast_chunker.rs` from 1 language to 11; reuse DynamicJEPA's per-language node classification (already shipped) so we don't reinvent function/struct/class detection per language.

---

## 3. The atomic-chunk specification

### 3.1 Per-language entity types

Every language's AST has language-specific node types that correspond to "atomic" chunkable units. The DynamicJEPA extractor in `dynamicjepa.rs` already classifies these per language. Mapping to the doc 09 `EntityType` enum (currently in `ast_chunker.rs::EntityType`):

| Generic EntityType | Rust | Python | JS/TS | Go | Java | C/C++ | C# | Ruby | PHP |
|---|---|---|---|---|---|---|---|---|---|
| Function | `function_item` | `function_definition` | `function_declaration`, `arrow_function`, `method_definition` | `function_declaration`, `method_declaration` | `method_declaration` | `function_definition` | `method_declaration` | `method`, `singleton_method` | `function_definition` |
| Class | `struct_item`, `enum_item` | `class_definition` | `class_declaration` | `type_declaration` (struct) | `class_declaration` | `class_specifier`, `struct_specifier` | `class_declaration`, `record_declaration` | `class`, `module` | `class_declaration` |
| Trait/Interface | `trait_item` | `protocol_definition` (3.12+) | `interface_declaration` (TS) | `interface_type` | `interface_declaration` | (none — virtual class) | `interface_declaration` | (mixin) | `interface_declaration` |
| Impl/Method-block | `impl_item` | (inside class) | (inside class) | (receiver method) | (inside class) | (inside class) | (inside class) | (inside class) | (inside class) |
| Module | `mod_item` | `module` | `program`, `module` | `package_clause` | `package_declaration` | `namespace_definition` | `namespace_declaration` | `module` | `namespace_definition` |
| Test function | `function_item` w/ `#[test]` | `def test_*`, `class TestX` | `it()`, `test()`, `describe()` | `func TestX` | `@Test` annotation | (none — separate file) | `[Test]`, `[Fact]` attribute | `it` (RSpec), `def test_*` (Minitest) | `function test*`, `@test` PHPDoc |
| Import | `use_declaration` | `import_statement`, `import_from_statement` | `import_statement` | `import_declaration` | `import_declaration` | `preproc_include` | `using_directive` | `require`, `require_relative` | `namespace_use_declaration` |

### 3.2 The chunk boundary algorithm

For each language, the chunker walks the AST and produces chunks that:

1. **Start at an AST entity boundary** (function, class, struct, etc. — never mid-statement).
2. **End at an AST entity boundary** of the same nesting level.
3. **Target ~500 non-whitespace characters** per chunk. If a single function exceeds the target, emit it as one chunk anyway (don't split mid-function). If multiple small functions can be combined to approach the target, group adjacent siblings under the same parent.
4. **Prepend context**: file path + scope chain (e.g. `crates/foo/src/lib.rs::mod bar::impl Baz::`) + relevant imports.
5. **Include parent definition**: a method chunk includes its class/struct signature line at the top (Qodo recommendation).
6. **Tag with metadata**: `{language, entity_type, parent_chain, line_start, line_end, sha256, parse_diagnostics}`.

The shipped `ast_chunker.rs` already implements steps 1-6 for Rust. The `chunk()` function at `ast_chunker.rs:262` is the entry point. The work is to:
- Add per-language `set_language()` calls (one match arm per language).
- Add per-language node-type matchers to the existing `EntityType` enum machinery.
- Reuse DynamicJEPA's `semantic_extract_tree_sitter_source_facts` (`dynamicjepa.rs:6400`+) classification logic so each language's "function-like" node-type strings are unified.

### 3.3 Failure modes the chunker MUST handle

Per CLAUDE.md "no mocks, fail-closed errors":

- **Parse error in source**: emit `AstChunkerError::ParseFailed` with first 32 ERROR/MISSING node diagnostics (already implemented). Never silently fall back to text chunking.
- **Unsupported language**: emit `AstChunkerError::UnsupportedExtension`. The 11-language matrix above is exhaustive; anything else is rejected.
- **Empty source**: emit `AstChunkerError::EmptySource`. No empty chunks.
- **Mixed-language file** (e.g. `.html` with embedded JS, or PHP with embedded HTML): emit `AstChunkerError::MixedLanguageNotSupported` for now; future work could route per-block to per-language chunkers.
- **Chunk exceeds 4× target**: emit a warning diagnostic but produce the chunk (large auto-generated files happen). Downstream embedders may truncate.

---

## 4. Embedder routing — which embedder gets which AST chunk

The 21-embedder ensemble (doc 12) doesn't all want the same chunks. AST-aware routing matches each chunk to the embedders for which it's most informative:

| Chunk type | Routed to | Why |
|---|---|---|
| **Function/method body** | E7 Code (Qodo, 1536D) + E14 BGE-M3 (multilingual, 8192-token context) | Code embedders trained on function-bodies; multilingual coverage |
| **Function signature alone** | E1 Semantic (e5-large-v2) + E10 Contextual (e5-base-v2) | Signatures are short natural-language-like strings |
| **Test function** | E1 Semantic + E7 Code | Test functions are interpreted both as code (E7) and as natural-language descriptions of intent (E1, on docstring + name) |
| **Class/struct definition** | E11 Kepler (RoBERTa+TransE knowledge graph) + E7 Code | Type-graph reasoning (E11) + structural code (E7) |
| **Module/namespace declaration** | E1 Semantic + E8 Graph (sentence-level e5-large-v2) | High-level semantic + graph context |
| **Import/use statement** | E11 Kepler (entity graph) + E13 SPLADE v3 (sparse keyword) | Imports are entity references; sparse retrieval finds related modules |
| **Comment block** | E1 Semantic + E10 Contextual | Pure natural language |
| **Docstring** | E1 Semantic + E10 Contextual | Pure natural language |
| **Test assertion line** | E5 Causal (cause-effect) + E1 Semantic | Assertions encode causal expectations |
| **Diff hunk** | E_Diff (new, doc 09) + E7 Code | Diff is structural code change |
| **AST node-token sequence** | E_AST (new, doc 09) | The AST IS the input |
| **CFG basic blocks** | E_CFG (new, doc 09) | The CFG IS the input |
| **Def-use edges** | E_DataFlow (new, doc 09) | The dataflow graph IS the input |

**Implementation**: each chunk produced by `ast_chunker` carries an `EntityType` tag; the panel materializer routes chunks to embedders based on this tag using a lookup table.

### 4.1 Cross-embedder DDA per chunk

Per doc 13 §1, every chunk routed to N=K embedders produces `K + (K choose 2)` structured signals. For a typical function chunk routed to E7 + E14 + E1 (3 embedders), that's 3 + 3 = **6 signals per chunk**. For a class definition routed to E11 + E7 + E14 + E1 (4), that's 4 + 6 = **10 signals per chunk**.

A typical SWE-bench Lite Phase 0 mutation produces ~30-50 chunks across the patched files. At ~7 signals per chunk × 40 chunks = **~280 chunk-level signals per mutation entry**, multiplied by the panel-level signals (doc 13 §1.1: 231 at N=21) to give the full DDA budget.

---

## 5. The constellation per chunk-type (extends doc 13 §3.1)

TCT Phase 1 (constellation construction) becomes per-chunk-type when AST-aware routing is wired:

```
TctConstellation_AST_aware {
    centroids: BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Vec<f32>>,
    thresholds: BTreeMap<(EntityType, EmbedderId), f32>,
    ...
}
```

That's `8 mutation categories × 13 entity types × 11 languages × 21 embedders = 24,024 cells`. Many cells will be sparse (no `interface_declaration` in C — it doesn't exist). The Bayesian shrinkage from doc 13 §10 risk #5 handles sparsity.

The result: a patch in Python that modifies a `class_definition` is verified against the centroid for `(KnownGood, Class, Python, E11_Kepler)` etc., not against a generic per-mutation-category centroid. **This dramatically tightens the Gτ verification** because each chunk-type has its own learned region in each embedder's space.

---

## 6. Implementation roadmap (AST chunker extension + embedder routing)

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | Refactor `ast_chunker.rs::EntityType` to be language-tagged enum | ~1 day | none | new |
| 2 | Per-language node-type classifier (Python) reusing DynamicJEPA logic | ~1 day | (1) | new |
| 3 | Per-language node-type classifier (JS/TS, Go, Java) | ~3 days | (1) | new |
| 4 | Per-language node-type classifier (C/C++, C#, Ruby, PHP) | ~4 days | (1) | new |
| 5 | Cross-language test harness — chunk every real-Python / real-Rust / real-Go fixture and assert chunk boundaries align with AST entity boundaries | ~2 days | (2)(3)(4) | new |
| 6 | Embedder router: chunk → embedder set based on EntityType | ~1 day | (1) | new |
| 7 | `Instrument` trait wrapper that consumes chunks (per doc 12 §9 item #1) | ~1 day | (6) | new |
| 8 | Per-chunk-type constellation centroid construction | ~2 days | (5) + (6) + Phase 0 corpus | new |
| 9 | Per-chunk-type Gτ verification | ~1 day | (8) | new |
| 10 | E_AST instrument as AST-token-sequence transformer over chunks | ~3 days | (1) | per existing plan |
| 11 | E_CFG + E_DataFlow as graph-attention over per-chunk control/data graphs | ~5 days | (10) | per existing plan |
| 12 | E_Diff as AST-diff-edge encoder over chunk-pairs | ~2 days | (10) | per existing plan |
| 13 | Real-data FSV harness — SHA-pinned chunk boundaries persisted per fixture | ~1 day | (5) | new |

**Total AST-extension work: ~27 days**, slotting into Phase 1 (instruments) and Phase 0 (corpus). The bulk is per-language extension (items 2-4 = ~8 days).

---

## 7. What this changes in earlier docs

| Earlier doc | Change |
|---|---|
| **Doc 09 §2.2 instrument set** | E_AST / E_CFG / E_DataFlow / E_Diff explicitly consume AST-perfect chunks from the multi-language `ast_chunker`. E_Test consumes test-function chunks specifically. |
| **Doc 09 §3.1 mutation corpus** | Mutation operators (KnownGood, SubtleFlip, OffByOne, ...) now produce **chunked** mutated source — the mutation is applied to the original source, then re-chunked through `ast_chunker`. The (panel_t1, panel_t2) pair is a chunk-level diff. |
| **Doc 09 §7 Phase 1 deliverables** | Add: "extend `ast_chunker.rs` from Rust-only to all 11 supported languages; produce AST-perfect atomic chunks for every panel materialization." |
| **Doc 11 §2 ΔP** | Per-chunk ΔP becomes the load-bearing measurement: instead of one ΔP per panel, there are N ΔP values per panel (one per chunk), aggregated by the loss. |
| **Doc 12 §3 instrument-mapping** | All "thin head on existing embedder" entries now route through the multi-language chunker first. E_AST as `Qodo + AST-token-transformer` becomes `Qodo + AST-token-transformer over multi-language chunks`. |
| **Doc 13 §3.1 constellation** | Per-class centroid construction is now per (mutation_category × entity_type × language × embedder) — 24,024 cells (with Bayesian shrinkage on sparse cells). |
| **Doc 13 §3.3 Verdict** | `Verdict::GuardRejected` includes `violating_chunk: Option<ChunkId>` so rejection is localized to a specific function/class, not just the whole patch. |
| **Doc 13 §1.3 signal budget** | Adjusted: at AST-aware chunking with ~40 chunks per mutation × ~7 signals per chunk = ~280 chunk-level signals + the panel-level 231 = **~510 signals per mutation entry** (loose upper bound ~840). |
| **Sitrep `04_implementation_plans/plan_e_witness.md`** | E_Witness chunks the witness chain entries (already byte-aligned 73-byte units) — no AST chunker needed for E_Witness specifically; it's already atomic. |
| **Sitrep `04_implementation_plans/plan_e_static_analysis.md`** | The static-analysis aggregator routes per-language tool output (mypy/Pyright for Python, ruff for Python, clippy for Rust, javac for Java, etc.) and aggregates per chunk via the chunk's line-range. |
| **CLAUDE.md §1** | The 11-language matrix becomes part of the project description: "ContextGraph supports atomic AST chunking and per-language semantic extraction for Python, Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP." |

---

## 8. Why this unlocks new critical capabilities

### 8.1 Per-function (not per-file) verification

Today, `MeJepaCompiler::verify(patch)` operates at the patch level — pass or fail. With AST-aware chunking, verification operates at the **function/method/class level** — a patch that modifies 5 functions yields 5 separate Pass/Fail/GuardRejected verdicts, one per chunk. The reviewer (or automation) can localize disagreement to a specific syntactic unit.

### 8.2 Cross-language transfer becomes structurally tractable

Per-language constellation centroids let ME-JEPA learn that "a Python `class_definition` for a test class" and "a Java `class_declaration` annotated `@Test`" are the SAME entity-type at the cross-language level. This is the structural foundation for the doc 10 Section C cross-language mandate — Python-trained patterns transfer to Rust, Java, etc. through the shared EntityType taxonomy.

### 8.3 Real-Python / real-Rust FSV becomes deterministic

Per CLAUDE.md "no mocks, real data only", FSV harnesses that test the chunker can use **real source files** from the SWE-bench Lite repos as fixtures. The chunk boundaries are AST-aligned, so they're stable across reruns — chunk SHAs are deterministic per (file content, language, ast_chunker version).

### 8.4 ME-JEPA training amplifies via DDA at the chunk level

Per doc 13 §1.3, naïve panel-level DDA at N=21 is 231 signals per input. **Chunk-level DDA at ~40 chunks × 7 signals = 280 chunk-level signals** in addition. Combined: ~510 supervisory signals per mutation entry — over 2× the panel-level number. This is the operational meaning of "perfect atomic parsing" for the operator's "insane amount of reward signals" framing (per doc 13 §7.5.2).

### 8.5 The reality-loop's edit/test cycle produces denser supervision

Every Edit tool call by Claude Code in the outer loop touches one or more chunks. The shift-log subscriber (doc 09 §3.8) routes the edit through the multi-language chunker, identifies which chunks changed, and produces a per-chunk delta panel. ME-JEPA's online observe receives one supervision sample per **changed chunk**, not per file. A typical edit-2-functions-in-1-file produces 2 chunk-level training examples instead of 1 file-level one.

---

## 9. Risks (AST-chunking specific)

1. **tree-sitter grammar version skew across languages**: each language's grammar version evolves independently. Pin all 11 grammars to current versions in `Cargo.toml` (already done). Document the skew strategy: bump one language at a time; re-fit per-language chunk fixtures after each bump.
2. **Per-language node-type vocabulary drift**: tree-sitter sometimes renames node types between major versions (e.g. `class_body` → `class_block`). The per-language classifier must be defensive — match on the new and old names during the transition. DynamicJEPA's existing extractor has examples of this defensive pattern.
3. **Mixed-language files** (templating engines, embedded SQL/HTML, JSX-with-embedded-CSS): out of scope for first-pass; reject with explicit error. Future work could route per-block.
4. **Chunk-boundary instability under re-formatting**: a `cargo fmt` or `prettier` run reformats source without changing semantics; chunk SHAs will differ. Mitigation: chunk SHAs are computed on **AST node IDs**, not raw text, so reformatting is invisible to the chunker.
5. **Per-language testing burden**: 11 languages × ~10 entity types × multiple edge cases = a large test surface. Plan for ~30-40 fixture files per language (real source extracted from SWE-bench Lite + DynamicJEPA test fixtures + Phase 0 mutation outputs).
6. **Constellation cell sparsity**: 24,024 cells in the per-(mutation × entity × language × embedder) constellation. Many cells under-supported. Mitigation: Bayesian shrinkage to language-aggregate or entity-aggregate when per-cell N < 50.

---

## 10. Falsification

The framework is falsifiable. Specific falsifications:

1. **AST-perfect chunks are not better than fixed-size text chunks for E7 code embeddings**: ablation comparing both on a mutation-corpus retrieval task shows no top-k MRR improvement. → The Qodo / cAST recommendation may not transfer to ME-JEPA's specific use case; default back to text chunking with a smaller chunk size.
2. **Per-chunk-type constellations don't outperform per-mutation-category constellations**: ablation with vs without entity-type stratification shows no Gτ false-positive-rate improvement. → Drop entity-type stratification; aggregate to mutation-category × language.
3. **Cross-language transfer doesn't materialize from shared EntityType taxonomy**: a model trained only on Python doesn't predict Rust patches better than baseline. → The shared taxonomy is a noise predictor, not a signal predictor; treat languages as independent.
4. **DynamicJEPA's per-language classification doesn't generalize to ME-JEPA's mutation-applied source**: chunks produced from mutated source have different node-type distributions than chunks from clean source. → Re-fit DynamicJEPA's classifier on mutated-source fixtures.

---

## 11. The integration with TCT/DDA/ME-JEPA in one diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Source code (any of 11 languages)                                          │
│                                                                              │
│  ↓ tree-sitter parse + ast_chunker (this doc)                                │
│                                                                              │
│  N chunks, each tagged with (EntityType, Language, parent_chain, sha256)     │
│                                                                              │
│  ↓ per-chunk routing (doc 14 §4)                                             │
│                                                                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────┐ │
│  │ E7 Qodo    │  │ E14 BGE-M3 │  │ E11 Kepler │  │ E1 Semantic│  │ ... 17 │ │
│  │ (1536D)    │  │ (1024D ML) │  │ (768D KG)  │  │ (1024D)    │  │ more    │ │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘  └────────┘ │
│                                                                              │
│  ↓ panel materialization (doc 12)                                            │
│                                                                              │
│  Panel: 5,120-d concatenation of per-embedder slot outputs                   │
│                                                                              │
│  ↓ TCT Phase 2 training (doc 13)                                             │
│                                                                              │
│  L_step = ΔP × ΔK × ΔΩ × ΔΞ        (UTML v1.0, doc 11)                     │
│  with L_constellation_centroid loss against per-chunk-type centroids        │
│                                                                              │
│  ↓ TCT Phase 3 inference (doc 13)                                            │
│                                                                              │
│  MeJepaCompiler::verify(patch) -> Verdict {                                  │
│      Pass / Fail / OOD / Abstain / GuardRejected,                            │
│      gtau_per_chunk: BTreeMap<ChunkId, GtauResult>,    ← NEW per doc 14      │
│      conformal_interval, confidence,                                          │
│  }                                                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## See also

- AST chunker (Rust-only today, extend to all 11): `crates/context-graph-core/src/memory/ast_chunker.rs`
- Multi-language semantic extractor (already 11-language): `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- Python lexer (Phase 0 mutation operator support): `crates/context-graph-mejepa-corpus/src/python_lex.rs`
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- 21-embedder integration: `12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md`
- TCT/DDA framework: `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- cAST methodology: https://arxiv.org/html/2506.15655v1
- Qodo code-embeddings best practices: https://www.qodo.ai/blog/rag-for-large-scale-code-repos/
- Source theory: `docs2/UTL_v2_embodied.md` (UTML v1.0 — JEPA Edition)
