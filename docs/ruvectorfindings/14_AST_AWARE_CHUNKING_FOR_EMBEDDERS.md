# 14 — AST-Aware Atomic Chunking for the Embedder Ensemble

> **📊 2026-05-11 HARDENING STATUS — Phase 1a operational path shipped; 4 Phase 1a follow-ups remain.** The older `crates/context-graph-core/src/memory/ast_chunker.rs::AstCodeChunker` remains Rust-only, but the current operational Phase 1a path is `context_graph_core::memory::ast`: 11-language tree-sitter chunking with typed `Language`/`EntityType`, AST parse diagnostics, SHA-256 chunk IDs, per-chunk embedder routing sets, and fail-closed error codes (`MEJEPA_CHUNKER_EMPTY_SOURCE`, `MEJEPA_CHUNKER_PARSE_FAILED`, `MEJEPA_CHUNKER_MIXED_LANGUAGE`). Manual FSV passes through `cargo run -p context-graph-core --example ast_chunker_multilang_fsv`, evidence at `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`. Edge cases prove fail-closed behavior for empty source, invalid syntax, and mixed PHP/HTML. **Remaining Phase 1a work**: (1) converge/deprecate legacy Rust-only `AstCodeChunker`; (2) reformat-invariant AST-node-ID SHA auditing; (3) wire chunk-level TCT centroids over the now-shipped chunk IDs (doc 13 item 11); (4) chunk-level `GuardRejected` localization at inference (doc 13 item 12). Phases 1-5 FSV: [`15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`](./15_PHASE_1_5_HARDENING_FSV_2026_05_10.md). Phases 6/7/8 + UTML carriers FSV: [`16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`](./16_PHASE_6_8_HARDENING_FSV_2026_05_11.md).

> **Operator directive 2026-05-09:** *"we have ast parsers set up for i believe 10 coding languages. we are going to want to set up our embedders to work with these ast parsers to perfectly chunk all code for perfect atomic parsing of code. add all this into the plans as well into all planning docs where needed."*

> **Companion docs:**
> - `09_END_GOAL_REVIEW_REPLACEMENT.md` §2.2 — instrument set
> - `12_EMBEDDER_INTEGRATION.md` — 21-embedder ensemble inventory
> - `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` — TCT/DDA framework
> - `docs2/TheoryOfMachineLearning.md` — UTML v1.0 (the source theory)

---

## 0. Headline

**The repo has 11 wired tree-sitter parsers** (Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP), a legacy Rust-only `AstCodeChunker`, and a new FSV-proven multi-language chunking API in `context_graph_core::memory::ast`.

**The remaining gap:** chunking and routing are now operational, but the embedders are not yet executing full neural forward passes per chunk. Today the Phase 1b FSV proves artifact integrity, VRAM budget, active/retired policy, and routing coverage; the next deep integration is actual model-backed per-chunk embedding and chunk-level TCT.

**This doc specifies the integration**: use the new `memory::ast` API as the canonical chunking path, converge/deprecate the older Rust-only `AstCodeChunker`, and route AST-perfect chunks through active embedders (E1, E7, E8, E10, E12, E13, E14 plus deterministic/custom E2/E3/E4/E6/E9 where relevant; E5/E11 are retired).

**The DDA multiplier (per doc 13 §1).** Naïve whole-file panels yield ~50-200 panel triples per /loop turn. With AST-atomic chunking, the multiplier is the **average chunks-per-file**:

| Average chunks/file | Per-turn panel triples | Per-turn signals at N=21 (×231) | Per day |
|---:|---:|---:|---:|
| 5 (small files) | 250-1,000 | 58K-231K | 5.5M-22M |
| 10 (typical Python/Rust) | 500-2,000 | **115K-462K** | **11M-44M** |
| 20 (large class-heavy) | 1,000-4,000 | 231K-924K | 22M-89M |

At a typical Python/Rust ratio of ~10 chunks/file, the per-/loop-turn signal yield jumps **~10× over panel-only**. This is the operator's "perfect atomic parsing" — quantified in supervisory yield. Every chunk-level signal is **automatically labeled** by the entry's known `(mutation_category, oracle_verdict, entity_type, language)` tuple.

---

## 1. Why AST-aware chunking is load-bearing

### 1.1 Naïve text chunking corrupts code semantics

Most embedder pipelines slice source by character or token count. For natural language this is fine — sentence boundaries are loose. For code:

- A function split mid-body → chunk A has signature/parameters/return type, chunk B has body. Neither represents a meaningful semantic unit.
- A `class Foo:` split from its methods → chunk that mentions a class name with no methods, plus method-only chunks orphaned from class definition.
- An `impl Display for Foo` split from `fn fmt` body — same problem in Rust.
- A `function() => ...` arrow function in JS/TS spanning multiple lines is ambiguous without AST.

**Result:** the embedder's similarity space becomes noisy. Two chunks the human would consider semantically equivalent (because they ARE the same function, just split differently) end up far apart in latent space. ME-JEPA's predictor would have to learn to ignore 99% of its input — bad signal-to-noise.

### 1.2 The atomic approach: chunk by AST node, embed each chunk separately

Tree-sitter gives a *parse tree*. The chunker walks the tree and emits atomic chunks at the **function / method / class / struct / impl-block / module level**:

```rust
pub enum EntityType {
    Function, Method, Class, Struct, Module, Impl, Trait,
    TestFunction, Import, CommentBlock, Docstring,
    // ... etc
}
```

Each chunk has: source text (the lines it spans), signature, abbreviated definition (when source is too long), relevant imports, AST-node hash (deterministic identifier), byte/line range, file hash (provenance back to source-of-truth SHA chain).

**For ME-JEPA's panel materialization, each AST chunk is a separate panel input.** A 500-line file with 20 functions becomes 20 panel inputs. A 3-line edit to one function produces:

- 1 changed chunk (the edited function) → `panel[t=0]` (pre-edit) and `panel[t=1]` (post-edit) at full 14-embedder × 105-signal resolution
- 19 unchanged chunks → unchanged panels (cached)

**ME-JEPA sees the delta exactly where it happened**, with full embedder resolution on the changed chunk and zero noise from the other 19.

### 1.3 The Qodo / cAST recommendation

cAST (EMNLP 2025) and Qodo's blog (`https://www.qodo.ai/blog/rag-for-large-scale-code-repos/`) empirically establish:

1. **Target ~500 non-whitespace characters per chunk** (E7 Qodo is trained on this distribution).
2. **Preserve syntactic boundaries** — never split a function in the middle of its body.
3. **Prepend context** (file path, scope chain, surrounding imports) so the chunk is interpretable in isolation.
4. **Include parent class/struct definition** with each method chunk so `self` context is explicit.

The shipped `ast_chunker.rs` already implements all four for Rust. This doc's work is **multi-language extension** — apply the same logic to the 10 other tree-sitter parsers we already have.

### 1.4 Atomic parsing for ME-JEPA-Code's instruments

For E_AST / E_CFG / E_DataFlow / E_TypeGraph / E_Diff to work correctly:

- **E_AST** must encode the AST node-token sequence per chunk. AST-perfect chunking guarantees the parse tree fits inside the chunk; non-AST produces partial parse trees that misrepresent code.
- **E_CFG** computes basic-block control flow within each chunk. A chunk that splits mid-function has an artificial CFG boundary not reflecting actual program flow.
- **E_DataFlow** tracks def-use chains. Variables defined in chunk A and used in chunk B produce broken def-use edges in non-AST chunking.
- **E_Diff** computes diff edges across chunk pairs. AST-perfect chunks let diff operate on syntactic units (added function, modified body, deleted method) instead of text-line ranges.

Without AST-perfect chunking, these instruments produce noise; with it, they produce signal.

---

## 2. The 11-language matrix (current state)

Per `crates/context-graph-cli/Cargo.toml` and `crates/context-graph-core/Cargo.toml`:

| Slot | Language | tree-sitter crate | Version | Compiler/LSP backed | DynamicJEPA extractor | E7 chunker (`ast_chunker.rs`) |
|---|---|---|---|---|---|---|
| L1 | Rust | `tree-sitter-rust` | 0.23 | rustc / rust-analyzer | ✅ shipped | ✅ shipped via `memory::ast` |
| L2 | Python | `tree-sitter-python` | 0.23.6 | Pyright / mypy / py_compile | ✅ shipped | ✅ shipped via `memory::ast` |
| L3 | JavaScript | `tree-sitter-javascript` | 0.23.1 | node / tsc | ✅ shipped | ✅ shipped via `memory::ast` |
| L4 | TypeScript | `tree-sitter-typescript` | 0.23.2 | tsc (`--noEmit`) | ✅ shipped | ✅ shipped via `memory::ast` |
| L5 | Go | `tree-sitter-go` | 0.23.4 | `go test` in FSV | ✅ shipped | ✅ shipped via `memory::ast` |
| L6 | Java | `tree-sitter-java` | 0.23.5 | `javac` | ✅ shipped | ✅ shipped via `memory::ast` |
| L7 | C | `tree-sitter-c` | 0.23.4 | gcc/clang via `compile_commands.json` | ✅ shipped | ✅ shipped via `memory::ast` |
| L8 | C++ | `tree-sitter-cpp` | 0.23.4 | g++/clang++ | ✅ shipped | ✅ shipped via `memory::ast` |
| L9 | C# | `tree-sitter-c-sharp` | 0.23.5 | `dotnet build` | ✅ shipped | ✅ shipped via `memory::ast` |
| L10 | Ruby | `tree-sitter-ruby` | 0.23.1 | `ruby -c` | ✅ shipped | ✅ shipped via `memory::ast` |
| L11 | PHP | `tree-sitter-php` | 0.23.11 | `php -l` in FSV | ✅ shipped | ✅ shipped via `memory::ast` |

**Cross-language mandate alignment** (per `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`): the 9 explicit "supported languages" in the operator directive (Python, Rust, C, C++, Java, C#, JS, TS, Ruby) are all wired. Go and PHP are bonus coverage.

**The remaining work**: make `memory::ast` the single canonical chunker path, update any stale `AstCodeChunker::new_rust` consumers, add reformat-invariance audits, and wire the produced chunk IDs into per-chunk TCT and `GuardRejected` diagnostics.

---

## 3. The atomic-chunk specification

### 3.1 Per-language entity types

Every language's AST has language-specific node types corresponding to "atomic" chunkable units. DynamicJEPA already classifies these per language. Mapping to the generic `EntityType` enum:

| Generic EntityType | Rust | Python | JS/TS | Go | Java | C/C++ | C# | Ruby | PHP |
|---|---|---|---|---|---|---|---|---|---|
| Function | `function_item` | `function_definition` | `function_declaration`, `arrow_function`, `method_definition` | `function_declaration`, `method_declaration` | `method_declaration` | `function_definition` | `method_declaration` | `method`, `singleton_method` | `function_definition` |
| Class | `struct_item`, `enum_item` | `class_definition` | `class_declaration` | `type_declaration` (struct) | `class_declaration` | `class_specifier`, `struct_specifier` | `class_declaration`, `record_declaration` | `class`, `module` | `class_declaration` |
| Trait/Interface | `trait_item` | `protocol_definition` (3.12+) | `interface_declaration` (TS) | `interface_type` | `interface_declaration` | (none) | `interface_declaration` | (mixin) | `interface_declaration` |
| Impl/Method-block | `impl_item` | (inside class) | (inside class) | (receiver method) | (inside class) | (inside class) | (inside class) | (inside class) | (inside class) |
| Module | `mod_item` | `module` | `program`, `module` | `package_clause` | `package_declaration` | `namespace_definition` | `namespace_declaration` | `module` | `namespace_definition` |
| Test function | `function_item` w/ `#[test]` | `def test_*`, `class TestX` | `it()`, `test()`, `describe()` | `func TestX` | `@Test` annotation | (separate file) | `[Test]`, `[Fact]` attribute | `it` (RSpec), `def test_*` (Minitest) | `function test*`, `@test` PHPDoc |
| Import | `use_declaration` | `import_statement`, `import_from_statement` | `import_statement` | `import_declaration` | `import_declaration` | `preproc_include` | `using_directive` | `require`, `require_relative` | `namespace_use_declaration` |

### 3.2 The chunk boundary algorithm

For each language, the chunker walks the AST and produces chunks that:

1. **Start at an AST entity boundary** (function, class, struct — never mid-statement).
2. **End at an AST entity boundary** of the same nesting level.
3. **Target ~500 non-whitespace characters**. If a single function exceeds, emit it as one chunk (don't split mid-function). If multiple small functions can be combined to approach the target, group adjacent siblings under the same parent.
4. **Prepend context**: file path + scope chain (e.g. `crates/foo/src/lib.rs::mod bar::impl Baz::`) + relevant imports.
5. **Include parent definition**: a method chunk includes its class/struct signature line at the top (Qodo recommendation).
6. **Tag with metadata**: `{language, entity_type, parent_chain, line_start, line_end, sha256, parse_diagnostics}`.

The shipped Rust chunker (`AstCodeChunker::new_rust(...)` at `ast_chunker.rs:262`) implements steps 1-6. Per-language extension:

- Add per-language `set_language()` calls (one match arm per language).
- Add per-language node-type matchers to existing `EntityType` enum machinery.
- Reuse DynamicJEPA's `semantic_extract_tree_sitter_source_facts` (`dynamicjepa.rs:6400`+) classification logic.

### 3.3 Failure modes the chunker MUST handle

Per CLAUDE.md "no mocks, fail-closed errors":

- **Parse error in source**: emit `AstChunkerError::ParseFailed` with first 32 ERROR/MISSING node diagnostics (already implemented). Never silently fall back to text chunking.
- **Unsupported language**: emit `AstChunkerError::UnsupportedExtension`. The 11-language matrix is exhaustive; anything else rejected.
- **Empty source**: emit `AstChunkerError::EmptySource`. No empty chunks.
- **Mixed-language file** (`.html` with embedded JS, PHP with embedded HTML): emit `AstChunkerError::MixedLanguageNotSupported` for now; future work could route per-block.
- **Chunk exceeds 4× target**: emit warning diagnostic but produce chunk (large auto-generated files happen). Downstream embedders may truncate.

---

## 4. How AST chunks feed each ME-JEPA instrument

Doc 09 §2.2 lists 14 ME-JEPA instruments. Mapping AST-chunk granularity onto each:

| Instrument | AST role | Chunk-level computation |
|---|---|---|
| `E_AST` | Direct: encode AST node sequence of the chunk | Tree-sitter walk → token sequence → small transformer |
| `E_CFG` | Build CFG over chunk's basic blocks (within function/method scope) | Within-chunk control flow (branches, loops, returns) |
| `E_DataFlow` | Def-use chains within chunk's variable scope | Variable lifetimes within function/method |
| `E_TypeGraph` | Type-graph node induced by the chunk (signatures, return types, struct fields) | Pyright/mypy/rustc/javac per-chunk type info |
| `E_Test` | Tests covering this chunk specifically | Pytest collection / cargo test / JUnit @Test discovery → reverse map to chunk |
| `E_Trace` | Per-chunk execution trace (function entry/exit, line hits, value snapshots) | When `pytest --trace` / `cargo test -- --nocapture` runs, line hits map to chunk |
| `E_Diff` | AST-diff between pre-edit and post-edit chunk | Tree-sitter walk over `(pre, post)` → diff edges (added/removed/modified nodes) |
| `E_Witness` | Witness-chain entries pertaining to chunk's edits | Per-chunk SHA in chain (already byte-aligned 73-byte units; no chunker needed) |
| `E_Oracle` | Per-chunk pass/fail attribution (per §3.6 #4 hunk-attribution) | Sequential application: which chunks' edits flipped which test outcomes |
| `E_Problem` | Whole-task issue text (shared across all chunks for this attempt) | Reuse E1 on issue body; same panel slot for every chunk |
| `E_CommitMsg` | Whole-commit message (shared) | Reuse E1; same panel slot across chunks |
| `E_StaticAnalysis` | Per-chunk linter/typecheck warnings | mypy/Pyright/ruff/clippy outputs reverse-mapped to chunk via line-range |
| `E_Runtime` | Per-chunk runtime stats (walltime, RSS, allocations) | `pytest --durations` / `cargo bench` / `valgrind` per-chunk |
| `E_Reasoning` | Per-chunk agent reasoning trace (why did the agent edit THIS chunk) | Claude Code's tool-use sequence reverse-mapped to chunk SHA |

**Key insight:** every instrument either operates *natively* at chunk granularity (E_AST, E_CFG, E_DataFlow, E_Diff, E_Trace), or has its global signal *reverse-mapped* back to the affected chunk (E_Test, E_Oracle, E_StaticAnalysis, E_Runtime, E_Reasoning), or is *shared* across chunks for a given attempt (E_Problem, E_CommitMsg).

### 4.1 Embedder routing per chunk type

The 21-embedder ensemble doesn't all want the same chunks. AST-aware routing matches each chunk to embedders for which it's most informative:

| Chunk type | Routed to | Why |
|---|---|---|
| **Function/method body** | E7 Code (Qodo, 1536D) + E14 BGE-M3 (multilingual, 8192-token) | Code embedders trained on function-bodies; multilingual coverage |
| **Function signature alone** | E1 Semantic + E10 Contextual | Signatures are short natural-language-like strings |
| **Test function** | E1 Semantic + E7 Code | Tests interpreted both as code (E7) and as natural-language descriptions (E1, on docstring + name) |
| **Class/struct definition** | E8 Graph + E7 Code + E14 BGE-M3 | Active replacement for retired E11: graph context + structural code + multilingual semantics |
| **Module/namespace declaration** | E1 Semantic + E8 Graph (sentence-level e5-large-v2) | High-level semantic + graph context |
| **Import/use statement** | E8 Graph + E13 SPLADE v3 (sparse keyword) + E14 BGE-M3 | Active replacement for retired E11: graph context, sparse retrieval, multilingual semantics |
| **Comment block** | E1 Semantic + E10 Contextual | Pure natural language |
| **Docstring** | E1 Semantic + E10 Contextual | Pure natural language |
| **Test assertion line** | E7 Code + E1 Semantic + E10 Contextual | Active replacement for retired E5: code structure plus semantic/contextual assertion meaning |
| **Diff hunk** | E_Diff (new, doc 09) + E7 Code | Diff is structural code change |
| **AST node-token sequence** | E_AST (new, doc 09) | The AST IS the input |
| **CFG basic blocks** | E_CFG (new, doc 09) | The CFG IS the input |
| **Def-use edges** | E_DataFlow (new, doc 09) | The dataflow graph IS the input |

**Implementation**: each chunk produced by `ast_chunker` carries an `EntityType` tag; the panel materializer routes chunks to embedders based on this tag using a lookup table.

---

## 5. The constellation per chunk-type (extends doc 13 §2.1)

TCT Phase 1 (constellation construction) becomes per-chunk-type when AST-aware routing is wired:

```rust
TctConstellation_AST_aware {
    centroids: BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Vec<f32>>,
    thresholds: BTreeMap<(EntityType, EmbedderId), f32>,
    ...
}
```

**Cell count math:**

- Panel level: 14 embedders × 8 mutation categories × 9 languages = **1,008 cells** at file granularity.
- Chunk level: 8 categories × 13 entity types × 11 languages × 21 embedders = **24,024 cells**. Many cells will be sparse (no `interface_declaration` in C — it doesn't exist). The Bayesian shrinkage from doc 13 §10 handles sparsity.
- After Bayesian shrinkage: ~30 MB persisted.

The result: a patch in Python that modifies a `class_definition` is verified against the centroid for `(KnownGood, Class, Python, E11_Kepler)` etc., not against a generic per-mutation-category centroid. **This dramatically tightens the Gτ verification** because each chunk-type has its own learned region in each embedder's space.

### 5.1 Per-chunk runtime guard

`Gτ` at AST-chunk granularity:

```
Gτ(chunk; CP) = 𝟙[
  ∀m: cos(φ_m(chunk_post), centroid_m^{KnownGood, language, node_type})
       ≥ τ_m^{node_type}
]
```

The cosine threshold `τ_m` can also be **per-node-type**: a tighter threshold for critical structures (Class, Module — fewer should pass) and a looser threshold for common structures (Function — more variety).

When `Gτ` rejects a chunk, the typed error includes the chunk's AST-node-type and the failed modalities — a dense, structural debugging signal:

```
MEJEPA_INFER_CONSTELLATION_VIOLATION {
  chunk_id: "fn_id_abc123",
  node_type: Function,
  language: Python,
  failed_modalities: [E5, E7],          // causal direction wrong AND code structure wrong
  cosine_distances: { E5: 0.71, E7: 0.68, ... },
  taus_required: { E5: 0.85, E7: 0.85, ... }
}
```

This is **per-output verifiability** at AST-node precision.

---

## 6. Cross-chunk pair signals + module-level guard

DDA's 91/210 cross-embedder pair signals (doc 12 §6) become *much more diagnostic* at AST granularity:

| Pair | Per-chunk meaning | High-leverage example |
|---|---|---|
| (E1 semantic, E7 code) | "Does the function's docstring match the function's code?" | Catches AI patches where the docstring lies about the function |
| (E7 code, E10 contextual) | "Does the predicted effect description match the AST diff *of this function*?" | Active replacement for retired E5 causal checks; catches confused diff explanations at function granularity |
| (E7 code, E8 graph) | "Does this function call the entities the dependency graph says are reachable *from this scope*?" | Active replacement for retired E11; catches ImportError / NameError before runtime, per-function |
| (E1 semantic, E14 multilingual) | "Does the function survive translation through the multilingual head?" | Catches language-specific overfitting at function granularity |
| (E6 sparse, E13 splade) | "Do two independent sparse encoders agree on keyword salience *in this function*?" | Catches AI patches that hallucinate function names *here specifically* |
| (E9 HDC, E12 late_interaction) | "Robust signature × token-precision agreement *for this function*" | Catches typo-induced regressions per function |
| (E10 contextual, E8 graph) | "Static-analysis warnings × type-graph structure *for this function*" | Catches type errors before compiler runs, per-function |

**Cross-chunk pair signals** (new at AST granularity): for two chunks `c_i, c_j`, compute `cos(φ_m(c_i), φ_m(c_j))` for each embedder. This captures:

- **"Does the new function fit with its sibling functions?"** — high cosine across E1, E7, E11 means new chunk integrates with its module.
- **"Is the new chunk an outlier?"** — low cosine in some embedders relative to siblings → potential regression.

### 6.1 Module-level cross-chunk guard

A single function passing `Gτ` is necessary but not sufficient. The **module-level constellation** checks all chunks together:

```
Gτ_module(chunks; CP_module) = 𝟙[
  ∀m: cos(φ_m(centroid_of_chunks), centroid_m^{KnownGood, module}) ≥ τ_m^{module}
]
```

This catches "individually-OK functions that don't make sense together" — the integration-level bugs that per-function checks miss.

### 6.2 Cross-embedder DDA per chunk

Per doc 13 §1, every chunk routed to N=K embedders produces `K + (K choose 2)` structured signals. A typical function chunk routed to E7 + E14 + E1 (3 embedders) → 3 + 3 = **6 signals per chunk**. A class definition routed to E11 + E7 + E14 + E1 (4) → 4 + 6 = **10 signals per chunk**.

A typical SWE-bench Lite Phase 0 mutation produces ~30-50 chunks across patched files. At ~7 signals per chunk × 40 chunks = **~280 chunk-level signals per mutation entry**, multiplied by panel-level signals (doc 13 §1: 231 at N=21) to give the full DDA budget of ~510 signals per entry.

---

## 7. Why this is structurally critical for ME-JEPA

### 7.1 ΔS sharpening (UTML v1.0 §III)

ΔS = `1 − cosine(predictor_output, target)` is a single number per panel. At whole-file granularity, that number is dominated by unchanged code. At AST-chunk granularity, ΔS *directly measures* the semantic distance between predicted and actual chunk panels — the loss signal goes from "noisy" to "sharp."

### 7.2 ΔK improvement (UTML v1.0 §IV)

The `cos_align(g_t, ḡ_prior)` term is more meaningful when gradients are computed per chunk: a gradient that helps Function-A representations should align with prior gradients on similar Function-A patterns. Cross-chunk-type gradient alignment is the deeper cousin of NTK alignment (Atanasov 2021).

### 7.3 ΔΞ verification (UTML v1.0 §VI)

The `predictor_redundancy` term in ΔΞ is computed via pairwise MI across panel slots. At AST granularity, redundancy detection is much sharper: if E5 and E7 always agree on "same-shape function with same-shape control flow," then their pair signal is redundant and one of them can be down-weighted.

### 7.4 Consolidation M(t) at chunk granularity (UTML v1.0 §VII)

Each chunk is a separate consolidation event. EWC++ Fisher importance is per-parameter × per-chunk-type, not per-file. This protects parameters important for Class chunks without preventing learning on Function chunks.

### 7.5 The "perfect atomic parsing" claim — formalized

The operator's "perfect atomic parsing of code" reduces to:

```
For any code change C in language L:
  C decomposes into AST chunks {c_1, c_2, ..., c_k}
    where each c_i has {EntityType, signature, source, AST_hash, file_provenance}.
  Each c_i produces a 14-embedder panel of fixed dimension.
  Each panel produces a 105-signal supervisory vector.
  ME-JEPA's predictor + Gτ guard + verify() API operate at chunk granularity.
  The verdict for C is a structured {chunks: [(chunk_id, verdict)]} list,
    not a single bool.
```

**This is what makes ME-JEPA's verdict diagnosable rather than opaque.** Operators inspecting a refused patch see *exactly which functions/classes failed which embedder constellations* — not "the patch is bad."

---

## 8. Doc 13 §3.3 Verdict updated for AST granularity

`Verdict::GuardRejected` now includes `violating_chunks: Vec<ChunkId>` so rejection is localized to specific functions/classes/impl-blocks/module-decls, not just the whole patch. This is the chunk-level form spec'd in doc 13 §2.3:

### 8.1 Per-chunk localization in practice

With chunk-level Gτ wired, a `GuardRejected` no longer says "this 200-line patch is bad" — it says: "the `Function:apply_filter` chunk in `tests/test_units.py` lines 124-167 fails the `(SubtleFlip, Function, Python, E11)` centroid by 0.18 below threshold."

The agent (or the operator reviewing the agent's output) can:

1. **Know precisely what failed** — which function, which class, which import block.
2. **Re-verify independently** — re-run the deterministic O(M·d_max) Gτ test on the persisted chunk panel, no model state needed.
3. **Repair surgically** — the next outer-loop turn can target only the violating chunks, not re-attempt the whole patch.

This is the operational form of the operator's "verify if the agent has done what it says it's done" framing: **every chunk the agent emitted has its own deterministic geometric verdict, computable by anyone with the frozen embedders + the frozen constellation**, no trust in the agent's self-report required.

---

## 9. Implementation roadmap

### 9.1 What's already shipped

- ✅ 11 tree-sitter parsers wired in `context-graph-cli` and `context-graph-core`.
- ✅ Rust AST chunker (`AstCodeChunker::new_rust`) with FSV at `target/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`.
- ✅ Python AST mutation operators (`crates/context-graph-mejepa-corpus/src/`).
- ✅ Per-language symbol-identity rules per doc 09 §3.1: Rust `function_item` inside `impl_item` → Method; Python `function_definition` inside `class_definition` → method; JS/TS binding-site naming for arrow functions; C declarator-aware extraction; C++ template/namespace; Java JUnit annotations; C# operator-overload identity; Ruby DSL methods.

### 9.2 What ships next

| # | Item | Effort | Dependency | Status |
|---|---|---|---|---|
| 1 | Refactor `ast_chunker.rs::EntityType` to be language-tagged enum | ~1 day | none | new |
| 2 | Per-language node-type classifier (Python) reusing DynamicJEPA logic | ~1 day | (1) | new |
| 3 | Per-language node-type classifier (JS/TS, Go, Java) | ~3 days | (1) | new |
| 4 | Per-language node-type classifier (C/C++, C#, Ruby, PHP) | ~4 days | (1) | new |
| 5 | Cross-language test harness — chunk every real-Python / real-Rust / real-Go fixture and assert chunk boundaries align with AST entity boundaries | ~2 days | (2)(3)(4) | new |
| 6 | Embedder router: chunk → embedder set based on EntityType | ~1 day | (1) | new |
| 7 | `Instrument` trait wrapper that consumes chunks (per doc 12 §11 item #1) | ~1 day | (6) | new |
| 8 | Per-language test extraction (pytest collection / cargo test / JUnit / RSpec → chunk reverse-map) | ~2 days/language | (1) | partial |
| 9 | Per-language compiler/LSP backed type evidence (Pyright/mypy, rustc, javac, dotnet, tsc, ruby -c, gcc/clang) | ~2-3 days/language; some shipped | (1) | partial |
| 10 | `mejepa_compute_panel(file_path, chunk_filter)` MCP tool: per-chunk panel materialization | ~2 days | (1) | new |
| 11 | Per-AST-node-type constellation centroids (24,024 cells with Bayesian shrinkage) at Phase 0 finalization | ~3 days | corpus across all languages, (10) | new |
| 12 | Per-chunk `Gτ` guard with chunk-id-typed error variants | ~3 days | (11) | new |
| 13 | Module-level cross-chunk constellation (the integration-test analog) | ~3 days | (11) | new |
| 14 | E_AST instrument as AST-token-sequence transformer over chunks | ~3 days | (1) | per existing plan |
| 15 | E_CFG + E_DataFlow as graph-attention over per-chunk control/data graphs | ~5 days | (14) | per existing plan |
| 16 | E_Diff as AST-diff-edge encoder over chunk-pairs | ~5 days | (14) | per existing plan |
| 17 | E_Trace per-chunk: pytest `--trace` / cargo `--nocapture` parsing → reverse-map to chunk SHA | ~5 days | (1) + per-language test extraction | deferred |
| 18 | E_Reasoning per-chunk: Claude Code tool-use sequence → chunk SHA reverse-map | ~3 days | (1) + harness shift-log | deferred |
| 19 | Real-data FSV harness — SHA-pinned chunk boundaries persisted per fixture | ~1 day | (5) | new |

**Total AST-extension work: ~50-70 days for full per-AST-chunk integration across all 11 languages.** Sequence by language priority: Python first (already deepest), Rust second, then C/C++/Java/C#/JS/TS/Ruby/Go/PHP per operator preference. The bulk is per-language extension (items 2-4 = ~8 days).

### 9.3 Phase-plan integration with doc 09

Doc 09 Phase 1 (instruments) should be re-scoped to include AST-atomic granularity from the start:

- **Phase 1 deliverable update:** instruments operate at AST-chunk granularity, not whole-file. Each instrument has a `compute_for_chunk(chunk: &CodeChunk) -> Latent` method.
- **Phase 0 corpus update:** mutation operators run at chunk granularity (mutate one function within a file, not the whole file). Current Python mutation operators already do this; extending to all languages follows the same pattern.
- **Phase 4 verify() update:** `mejepa_verify(patch)` decomposes the patch into AST chunks, runs `Gτ` per chunk + module-level cross-chunk guard, returns a typed verdict with chunk-id provenance for every gate failure.

---

## 10. What this changes in earlier docs

| Earlier doc | Change |
|---|---|
| **Doc 09 §2.2 instrument set** | E_AST / E_CFG / E_DataFlow / E_Diff / E_Test explicitly consume AST-perfect chunks from multi-language `ast_chunker`. |
| **Doc 09 §3.1 mutation corpus** | Mutation operators now produce **chunked** mutated source — mutation applied to original source, then re-chunked. The (panel_t1, panel_t2) pair is a chunk-level diff. |
| **Doc 09 §7 Phase 1 deliverables** | Add: "extend `ast_chunker.rs` from Rust-only to all 11 supported languages; produce AST-perfect atomic chunks for every panel materialization." |
| **Doc 10 §C cross-language mandate** | Note that 11 parsers are wired and the chunker dispatches per-language. |
| **Doc 11 §2 ΔP / L_step** | Per-chunk ΔP becomes load-bearing: instead of one ΔP per panel, there are N ΔP values per panel (one per chunk), aggregated by the loss. L_step computed per chunk, not per file. |
| **Doc 12 §3 instrument-mapping** | All "thin head on existing embedder" entries now route through the multi-language chunker first. E_AST as `Qodo + AST-token-transformer` becomes `Qodo + AST-token-transformer over multi-language chunks`. |
| **Doc 12 §2 signal yield** | Update from "50-200 panel triples per /loop turn" to "500-2,000 chunk-level triples per /loop turn at typical 10 chunks/file ratio." |
| **Doc 13 §2.1 constellation** | Per-class centroid construction is now per `(mutation_category × entity_type × language × embedder)` — 24,024 cells with Bayesian shrinkage on sparse cells. |
| **Doc 13 §2.3 Verdict** | `Verdict::GuardRejected` includes `violating_chunks: Vec<ChunkId>` so rejection is localized to specific functions/classes. |
| **Doc 13 §1.3 signal budget** | Adjusted: ~40 chunks per mutation × ~7 signals per chunk = ~280 chunk-level signals + panel-level 231 = **~510 signals per mutation entry** (loose upper bound ~840). |
| **Sitrep `04_implementation_plans/plan_e_witness.md`** | E_Witness chunks the witness chain entries (already byte-aligned 73-byte units) — no AST chunker needed for E_Witness specifically; it's already atomic. |
| **Sitrep `04_implementation_plans/plan_e_static_analysis.md`** | The static-analysis aggregator routes per-language tool output and aggregates per chunk via line-range. |
| **CLAUDE.md §1** | The 11-language matrix becomes part of the project description. |

### 10.1 Memory decisions to update

- `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md` — append "AST-atomic chunking via 11 wired tree-sitter parsers is part of the cross-language mandate; see doc 14."
- `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md` — append "Embedders operate at AST-chunk granularity (per-function/method/class/struct/impl-block/module), multiplying signal yield by ~10× over whole-file embedding."

---

## 11. Why this unlocks new critical capabilities

### 11.1 Per-function (not per-file) verification

Today, `MeJepaCompiler::verify(patch)` operates at the patch level — pass or fail. With AST-aware chunking, verification operates at the **function/method/class level** — a patch that modifies 5 functions yields 5 separate Pass/Fail/GuardRejected verdicts. The reviewer (or automation) can localize disagreement to a specific syntactic unit.

### 11.2 Cross-language transfer becomes structurally tractable

Per-language constellation centroids let ME-JEPA learn that "a Python `class_definition` for a test class" and "a Java `class_declaration` annotated `@Test`" are the SAME entity-type at the cross-language level. This is the structural foundation for the doc 10 Section C cross-language mandate — Python-trained patterns transfer to Rust, Java, etc. through the shared EntityType taxonomy.

### 11.3 Real-Python / real-Rust FSV becomes deterministic

Per CLAUDE.md "no mocks, real data only", FSV harnesses can use **real source files** from SWE-bench Lite repos as fixtures. Chunk boundaries are AST-aligned, so they're stable across reruns — chunk SHAs are deterministic per `(file content, language, ast_chunker version)`.

### 11.4 ME-JEPA training amplifies via DDA at the chunk level

Per doc 13 §1.3, naïve panel-level DDA at N=21 is 231 signals per input. **Chunk-level DDA at ~40 chunks × 7 signals = 280 chunk-level signals** in addition. Combined: ~510 supervisory signals per mutation entry — over 2× the panel-level number. This is the operational meaning of "perfect atomic parsing" for the operator's "insane amount of reward signals" framing.

### 11.5 The reality-loop's edit/test cycle produces denser supervision

Every Edit tool call by Claude Code in the outer loop touches one or more chunks. The shift-log subscriber (doc 09 §3.8) routes the edit through the multi-language chunker, identifies which chunks changed, and produces a per-chunk delta panel. ME-JEPA's online observe receives one supervision sample per **changed chunk**, not per file. A typical edit-2-functions-in-1-file produces 2 chunk-level training examples instead of 1 file-level.

---

## 12. Risks (AST-chunking specific)

1. **tree-sitter grammar version skew** — each language's grammar evolves independently. Pin all 11 grammars in `Cargo.toml` (already done). Document skew strategy: bump one language at a time; re-fit per-language chunk fixtures after each bump.
2. **Per-language node-type vocabulary drift** — tree-sitter sometimes renames node types between major versions (`class_body` → `class_block`). Per-language classifier must be defensive — match new and old names during transition. DynamicJEPA's existing extractor has examples of this defensive pattern.
3. **Mixed-language files** (templating engines, embedded SQL/HTML, JSX-with-embedded-CSS) — out of scope for first-pass; reject with explicit error. Future work could route per-block.
4. **Chunk-boundary instability under re-formatting** — `cargo fmt` or `prettier` reformats source without changing semantics; chunk SHAs would differ. Mitigation: chunk SHAs computed on **AST node IDs**, not raw text, so reformatting is invisible.
5. **Per-language testing burden** — 11 languages × ~10 entity types × multiple edge cases = large test surface. Plan ~30-40 fixture files per language (real source extracted from SWE-bench Lite + DynamicJEPA test fixtures + Phase 0 mutation outputs).
6. **Constellation cell sparsity** — 24,024 cells. Many under-supported. Mitigation: Bayesian shrinkage to language-aggregate or entity-aggregate when per-cell N < 50.

---

## 13. Falsification

The framework is **falsifiable**. Specific scenarios:

1. **AST-perfect chunks are not better than fixed-size text chunks** for E7 code embeddings — ablation comparing both on a mutation-corpus retrieval task shows no top-k MRR improvement. **Falsifies if** linear-probe accuracy on AST-chunk panels is ≥10pp below whole-file panels on the same task. → Default back to text chunking with smaller chunk size; consider merging tiny chunks via existing `merge_small_chunks` function.
2. **Per-AST-node-type constellations don't outperform per-mutation-category constellations** — ablation with vs without entity-type stratification shows no Gτ false-positive-rate improvement. → Drop entity-type stratification; aggregate to mutation-category × language.
3. **Cross-language transfer doesn't materialize from shared EntityType taxonomy** — a model trained only on Python doesn't predict Rust patches better than baseline. → Shared taxonomy is noise predictor; treat languages as independent.
4. **DynamicJEPA's per-language classification doesn't generalize to ME-JEPA's mutation-applied source** — chunks from mutated source have different node-type distributions than clean source. → Re-fit DynamicJEPA's classifier on mutated-source fixtures.
5. **Module-level cross-chunk guard adds no value over per-chunk Gτ** — single-chunk passes already correlate with whole-module passes. → Skip module-level guard.
6. **Cross-language chunker generalization is too costly** — extending to 9 languages takes >50 days. → Sequence by priority (Python ✓, Rust ✓, C/C++ next, then Java/C#, then JS/TS/Ruby/Go/PHP).

If any falsification holds, the relevant section gets `status: superseded`.

---

## 14. The integration with TCT/DDA/ME-JEPA in one diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Source code (any of 11 languages)                                          │
│                                                                              │
│  ↓ tree-sitter parse + ast_chunker (this doc)                                │
│                                                                              │
│  N chunks, each tagged with (EntityType, Language, parent_chain, sha256)     │
│                                                                              │
│  ↓ per-chunk routing (doc 14 §4.1)                                           │
│                                                                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────┐ │
│  │ E7 Qodo    │  │ E14 BGE-M3 │  │ E11 Kepler │  │ E1 Semantic│  │ ... 17 │ │
│  │ (1536D)    │  │ (1024D ML) │  │ (768D KG)  │  │ (1024D)    │  │ more   │ │
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
│      gtau_per_chunk: BTreeMap<ChunkId, GtauResult>,    ← per doc 14          │
│      conformal_interval, confidence,                                          │
│  }                                                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 15. One-paragraph summary

The repo has **11 tree-sitter parsers wired** plus an **existing AST chunker with FSV** plus a **multi-language semantic extractor** already handling all 11 languages. By feeding ATOMIC AST chunks (function / method / class / struct / impl-block / module — not raw line splits) through the **21 frozen embedders** from doc 12, every code change produces panel triples *at the AST-node granularity*. This makes `panel[t=1] − panel[t=0]` a delta over the AST itself, not a delta over "the file got bigger by 12 lines." DDA's 105/231 signals/input applies per AST chunk rather than per file — multiplying supervisory yield by ~10× to ~50K-200K signals per /loop turn, ~5M-20M per day per Claude Code instance. Per-AST-node-type constellation centroids (24,024 cells = 8 mutation categories × 13 entity types × 11 languages × 21 embedders, with Bayesian shrinkage on sparse cells) sharpen the runtime guard `Gτ` to *per-function precision*. The verify() API decomposes any patch into AST chunks, runs Gτ per chunk + module-level cross-chunk guard, and returns a structured verdict listing exactly which chunk(s) failed which embedder constellation(s) — turning ME-JEPA's verdict from "the patch is bad" into a structural diagnostic at AST-node precision. **This is the "perfect atomic parsing" the operator demanded — quantified and grounded in the existing tree-sitter infrastructure.**

---

## See also

- AST chunker (Rust-only today, extend to all 11): `crates/context-graph-core/src/memory/ast_chunker.rs`
- AST chunker FSV: `crates/context-graph-core/examples/ast_chunker_fsv.rs`
- Multi-language semantic extractor (already 11-language): `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- Python lexer (Phase 0 mutation operator support): `crates/context-graph-mejepa-corpus/src/python_lex.rs`
- 11 tree-sitter parser dependencies: `crates/context-graph-cli/Cargo.toml`
- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- 21-embedder integration: `12_EMBEDDER_INTEGRATION.md`
- TCT/DDA framework: `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- cAST methodology: https://arxiv.org/html/2506.15655v1
- Qodo code-embeddings best practices: https://www.qodo.ai/blog/rag-for-large-scale-code-repos/
- Source theory: `docs2/TheoryOfMachineLearning.md` (UTML v1.0 — JEPA Edition)
- Locked decisions:
  - `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md`
  - `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`
