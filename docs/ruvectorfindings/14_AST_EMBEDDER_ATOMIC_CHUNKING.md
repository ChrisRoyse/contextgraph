# 14 — AST × Embedder Atomic Chunking: Perfect Atomic Code Parsing for ME-JEPA

> **Operator directive 2026-05-09 (extension):** *"we have ast parsers set up for i
> believe 10 coding languages. we are going to want to set up our embedders to work with
> these ast parsers to perfectly chunk all code for perfect atomic parsing of code. add
> all this into the plans as well into all plannning docs where needed."*

> **Key claim of this doc.** The repo has **11 tree-sitter parsers wired** plus an
> existing AST chunker (`crates/context-graph-core/src/memory/ast_chunker.rs`) with FSV.
> By feeding ATOMIC AST chunks (function / method / class / struct level — not raw
> 1-token-window splits) through the **14 frozen embedders** (doc 12), every code change
> produces **panel triples at the atomic AST-node granularity**. This makes
> `panel[t=1] − panel[t=0]` **literally a delta over the AST itself**, not a delta over
> "the file got bigger by 12 lines". DDA's 105 signals/input now applies *per AST chunk*
> rather than per file — multiplying supervisory yield by ~10-100×.

> **Companion docs:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md)
> §2.2 (instrument set), [`12_EMBEDDER_INTEGRATION.md`](./12_EMBEDDER_INTEGRATION.md)
> (14 embedders), [`13_UNIVERSAL_AGENT_VERIFICATION.md`](./13_UNIVERSAL_AGENT_VERIFICATION.md)
> (universal verifier), [`docs2/UTL_v2_embodied.md`](../../docs2/UTL_v2_embodied.md)
> (UTML v1.0).

---

## Section 1 — The 11 wired tree-sitter parsers (current state)

Per `crates/context-graph-cli/Cargo.toml` and `crates/context-graph-core/Cargo.toml`:

| Slot | Language | Parser crate | Version | Compiler/LSP backed | Test fixtures |
|---|---|---|---|---|---|
| L1 | Rust | `tree-sitter-rust` | 0.23 | rustc / rust-analyzer | `ast_chunker_fsv.rs` covers Rust |
| L2 | Python | `tree-sitter-python` | 0.23.6 | Pyright / mypy / py_compile | `mejepa_corpus_fsv` (8 mutation operators) |
| L3 | JavaScript | `tree-sitter-javascript` | 0.23.1 | node / tsc (for `.ts` cousins) | binding-site symbol identity tests |
| L4 | TypeScript | `tree-sitter-typescript` | 0.23.2 | tsc (`--noEmit`) | `tsx`/`ts`/`mts`/`cts` dialect support |
| L5 | Go | `tree-sitter-go` | 0.23.4 | `go build` (TBD wired) | (TBD) |
| L6 | Java | `tree-sitter-java` | 0.23.5 | `javac` | Maven Surefire/Failsafe defaults; JUnit annotations |
| L7 | C | `tree-sitter-c` | 0.23.4 | gcc/clang via `compile_commands.json` | declarator-aware extraction; `preproc_def`/`preproc_function_def` as macro symbols |
| L8 | C++ | `tree-sitter-cpp` | 0.23.4 | g++/clang++ via `compile_commands.json` | template/namespace/qualified_identifier; `.h/.hh/.hpp/.hxx/.h++/.ipp/.inl/.tpp/.txx/.ixx` C++ headers |
| L9 | C# | `tree-sitter-c-sharp` | 0.23.5 | `dotnet build` | operator overload identity (`operator +`, `operator int`, etc.) |
| L10 | Ruby | `tree-sitter-ruby` | 0.23.1 | `ruby -c` | RSpec `describe/context/it`/Minitest `test_` methods |
| L11 | PHP | `tree-sitter-php` | 0.23.11 | (TBD wired) | (TBD) |

**Cross-language mandate alignment** (per
`memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`):
the 9 explicit "supported languages" in the operator directive (Python, Rust, C, C++,
Java, C#, JS, TS, Ruby) are all wired. Go and PHP are bonus coverage.

---

## Section 2 — Why AST-atomic chunking matters for ME-JEPA

### 2.1 The naive approach fails

If you feed *raw file deltas* through the 14 embedders, the 105-signal vector is
contaminated by *unchanged code*. A 500-line file with a 3-line bug fix will produce
a panel that's 99% "the rest of the file" — the embedders can't see the actual change.

ME-JEPA's predictor would have to learn to ignore 99% of its input. Bad signal-to-noise.

### 2.2 The atomic approach: chunk by AST node, embed each chunk separately

Tree-sitter gives a *parse tree* of the code. The chunker walks the tree and emits
**atomic chunks at the function / method / class / struct / impl-block / module level**:

```rust
pub enum EntityType {
    Function,    // top-level fn (Rust), def (Python), etc.
    Method,      // fn within impl/class
    Class,       // class declaration
    Struct,      // struct declaration
    Module,      // pub mod / namespace
    Impl,        // impl block (Rust); also captures C++ class implementation
    // ... etc
}
```

Each chunk has:
- Source text (the lines it spans)
- Signature (`pub async fn embed(&self, input: &ModelInput) -> Result<ModelEmbedding>`)
- Abbreviated definition (when source is too long)
- Imports relevant to the chunk
- AST-node hash (deterministic identifier)
- Byte range + line range
- File hash (provenance back to source-of-truth SHA chain)

**For ME-JEPA's panel materialization, each AST chunk is a separate panel input.** A
500-line file with 20 functions becomes 20 panel inputs. A 3-line edit to one function
produces:
- 1 changed chunk (the edited function) → `panel[t=0]` (pre-edit) and `panel[t=1]`
  (post-edit) at full 14-embedder × 105-signal resolution
- 19 unchanged chunks → unchanged panels (cached)

ME-JEPA sees the delta *exactly where it happened*, with full embedder resolution on
the changed chunk and zero noise from the other 19.

### 2.3 The DDA multiplier

Per doc 12 §1.2, naive whole-file panels yield ~50-200 panel triples per /loop turn.
With AST-atomic chunking, the multiplier is the **average chunks-per-file**:

| Average chunks/file | Per-turn panel triples (was 50-200) | Per-turn signals (×105) | Per day |
|---:|---:|---:|---:|
| 5 (small files) | 250-1,000 | 26K-105K | 2.5M-10M |
| 10 (typical Python/Rust) | 500-2,000 | 53K-210K | 5M-20M |
| 20 (large class-heavy file) | 1,000-4,000 | 105K-420K | 10M-40M |
| 50 (very large module) | 2,500-10,000 | 263K-1.05M | 25M-100M |

**At a typical Python/Rust codebase ratio of ~10 chunks/file, the per-/loop-turn signal
yield jumps from 5K-20K to 53K-210K — a 10× increase** purely from chunking
granularity. Per day per Claude Code instance: **5M-20M signals**, vs the 504K-2M
estimate in doc 12 §1.2.

This is the operator's "perfect atomic parsing" — quantified in supervisory yield.

---

## Section 3 — How AST chunks feed each ME-JEPA instrument

Doc 09 §2.2 lists 14 ME-JEPA instruments. Mapping AST-chunk granularity onto each:

| Instrument | AST role | Chunk-level computation |
|---|---|---|
| `E_AST` | Direct: encode the AST node sequence of the chunk | Tree-sitter walk → token sequence → small transformer |
| `E_CFG` | Build CFG over the chunk's basic blocks (within function/method scope) | Within-chunk control flow (branches, loops, returns) |
| `E_DataFlow` | Def-use chains within the chunk's variable scope | Variable lifetimes within function/method |
| `E_TypeGraph` | Type-graph node induced by the chunk (signatures, return types, struct fields) | Pyright/mypy/rustc/javac per-chunk type info |
| `E_Test` | Tests covering this chunk specifically | Pytest collection / cargo test / JUnit @Test discovery → reverse map to chunk |
| `E_Trace` | Per-chunk execution trace (function entry/exit, line hits, value snapshots) | When `pytest --trace` / `cargo test -- --nocapture` runs, each line hit maps to its chunk |
| `E_Diff` | AST-diff between pre-edit chunk and post-edit chunk | Tree-sitter walk over `(pre, post)` → diff edges (added/removed/modified nodes) |
| `E_Witness` | Witness-chain entries pertaining to this chunk's edits | Per-chunk SHA in the chain |
| `E_Oracle` | Per-chunk pass/fail attribution (per §3.6 #4 hunk-attribution) | Sequential application: which chunks' edits flipped which test outcomes |
| `E_Problem` | Whole-task issue text (shared across all chunks for this attempt) | Reuse E1 on the issue body; same panel slot for every chunk |
| `E_CommitMsg` | Whole-commit message (shared) | Reuse E1; same panel slot across chunks |
| `E_StaticAnalysis` | Per-chunk linter/typecheck warnings | mypy/Pyright/ruff/clippy outputs reverse-mapped to chunk |
| `E_Runtime` | Per-chunk runtime stats (walltime, RSS, allocations) | `pytest --durations` / `cargo bench` / `valgrind` per-chunk |
| `E_Reasoning` | Per-chunk agent reasoning trace (why did the agent edit THIS chunk) | Claude Code's tool-use sequence reverse-mapped to chunk SHA |

**Key insight:** every instrument either operates *natively* at chunk granularity
(E_AST, E_CFG, E_DataFlow, E_Diff, E_Trace), or has its global signal *reverse-mapped*
back to the affected chunk (E_Test, E_Oracle, E_StaticAnalysis, E_Runtime, E_Reasoning),
or is *shared* across chunks for a given attempt (E_Problem, E_CommitMsg).

### 3.1 Cross-language node-kind mappings (from existing harness)

Per the live harness state in
`docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §3.1 (current AST harness
state for each language):

| Language | Function symbol kinds | Method symbol kinds | Class/struct symbol kinds | Test detection |
|---|---|---|---|---|
| Python | `function_definition` | `function_definition` inside `class_definition` | `class_definition` | (TBD wired; pytest collection) |
| Rust | `function_item` | `function_item` inside `impl_item` | `struct_item`, `enum_item`, `union_item` | `#[test]`, `#[cfg(test)]` |
| JavaScript | `function_declaration`, `function_expression`, `arrow_function` (binding-site naming) | `method_definition` | `class_declaration` | (none native; relies on test runner) |
| TypeScript | same as JS + interface/type aliases | same as JS | `class_declaration`, `interface_declaration`, `type_alias_declaration` | same |
| Go | `function_declaration` | `method_declaration` | `type_declaration` | `Test*` function names |
| Java | `method_declaration` | `method_declaration` inside class | `class_declaration` | `@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`, `@TestTemplate` |
| C | `function_definition` (declarator-aware) | n/a | `struct_specifier`, `enum_specifier`, `union_specifier` (with `body`) | (TBD; relies on test framework) |
| C++ | `function_definition` | `function_definition` inside class | `class_specifier`, `struct_specifier`, `template_declaration` | (TBD) |
| C# | `method_declaration` (operator overloads `operator +`, `operator int`) | `method_declaration` inside class | `class_declaration`, `struct_declaration`, `interface_declaration` | (TBD; xUnit/NUnit attributes) |
| Ruby | `method`, `singleton_method` | `method` inside `class` | `class`, `module` | RSpec `describe/it/specify`, Minitest `test_` |
| PHP | `function_definition` | `method_declaration` | `class_declaration`, `interface_declaration`, `trait_declaration` | PHPUnit `@Test` annotation |

The chunker must dispatch to per-language node-kind tables. The shipped Rust chunker
(`AstCodeChunker::new_rust(...)`) provides the pattern; per-language extensions follow
the same shape.

---

## Section 4 — Atomic chunking + frozen instruments = constellation precision

Doc 12 §3 describes constellation centroids at the **(mutation_category, language)**
granularity (1,008 = 14 × 8 × 9 centroids). With AST-atomic chunking, the constellation
geometry sharpens:

### 4.1 Per-AST-node-type constellations

For each language, build constellations at **finer granularity**:

```
Constellation(category, language, node_type) for each:
  category ∈ {KnownGood, SubtleFlip, OffByOne, ..., CompileError}     # 8
  language ∈ {Python, Rust, C, C++, Java, C#, JS, TS, Ruby}            # 9
  node_type ∈ {Function, Method, Class, Struct, Impl, Module}          # 6
```

Total: 14 embedders × 8 × 9 × 6 = **6,048 constellation centroids** at the AST-node-type
granularity (vs 1,008 at file-level).

This is bounded — each centroid is ~1-5 KB; total storage ~30 MB. Computed once at Phase
0 corpus finalization.

**Why this matters:** a "KnownGood Python function" is geometrically distinct from a
"KnownGood Python class". Forcing the panel near a single (category, language) centroid
ignores the structural diversity of the language. Per-node-type centroids sharpen the
guard `Gτ`.

### 4.2 Per-chunk runtime guard

`Gτ` at AST-chunk granularity:

```
Gτ(chunk; CP) = 𝟙[
  ∀m: cos(φ_m(chunk_post), centroid_m^{KnownGood, language, node_type})
       ≥ τ_m^{node_type}
]
```

The cosine threshold `τ_m` can also be **per-node-type**: a tighter threshold for
critical structures (Class, Module — fewer should pass) and a looser threshold for
common structures (Function — more variety).

When `Gτ` rejects a chunk, the typed error includes the chunk's AST-node-type and the
failed modalities — a dense, structural debugging signal:

```
MEJEPA_INFER_CONSTELLATION_VIOLATION {
  chunk_id: "fn_id_abc123",
  node_type: Function,
  language: Python,
  failed_modalities: [E5, E7],  // causal direction wrong AND code structure wrong
  cosine_distances: { E5: 0.71, E7: 0.68, ... },
  taus_required: { E5: 0.85, E7: 0.85, ... }
}
```

This is the **per-output verifiability** structural advantage — at AST-node precision.

---

## Section 5 — Per-chunk pair signals (the 91 cross-embedder signals × atomic chunks)

DDA's 91 cross-embedder pair signals (doc 12 §4) become *much more diagnostic* at AST
granularity:

| Pair | Per-chunk meaning | High-leverage example |
|---|---|---|
| (E1 semantic, E7 code) | "Does the function's docstring match the function's code?" | Catches AI patches where the docstring lies about what the function does |
| (E5 causal, E7 code) | "Does the predicted cause→effect direction match the AST diff *of this function*?" | Catches confused causal claims at function granularity |
| (E7 code, E11 kepler) | "Does this function call the entities the dependency graph says are reachable *from this scope*?" | Catches ImportError / NameError before runtime, per-function |
| (E1 semantic, E14 multilingual) | "Does the function survive translation through the multilingual head?" | Catches language-specific overfitting at function granularity |
| (E6 sparse, E13 splade) | "Do two independent sparse encoders agree on keyword salience *in this function*?" | Catches AI patches that hallucinate function names *here specifically* |
| (E9 HDC, E12 late_interaction) | "Robust signature × token-precision agreement *for this function*" | Catches typo-induced regressions per function |
| (E10 contextual, E8 graph) | "Static-analysis warnings × type-graph structure *for this function*" | Catches type errors before compiler runs, per-function |

**Cross-chunk pair signals** (new at AST granularity): for two chunks `c_i, c_j`,
compute `cos(φ_m(c_i), φ_m(c_j))` for each embedder. This captures:

- **"Does the new function fit with its sibling functions?"** — high cosine across
  E1, E7, E11 means the new chunk integrates with its module.
- **"Is the new chunk an outlier?"** — low cosine in some embedders relative to
  sibling chunks → potential regression.

### 5.1 Cross-chunk constellation: the module-level guard

A single function passing `Gτ` is necessary but not sufficient. The **module-level
constellation** checks all chunks together:

```
Gτ_module(chunks; CP_module) = 𝟙[
  ∀m: cos(φ_m(centroid_of_chunks), centroid_m^{KnownGood, module}) ≥ τ_m^{module}
]
```

This catches "individually-OK functions that don't make sense together" — the
integration-level bugs that per-function checks miss.

---

## Section 6 — Implementation roadmap

### 6.1 What's already shipped

- ✅ 11 tree-sitter parsers wired in `context-graph-cli` and `context-graph-core`.
- ✅ Rust AST chunker (`AstCodeChunker::new_rust`) with FSV at
  `target/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`.
- ✅ Python AST chunker (mutation-corpus-driven; `tree-sitter-python` parsed input gate).
- ✅ Per-language symbol-identity rules (per doc 09 §3.1 AST harness state):
  Rust `function_item` inside `impl_item` → Method; Python `function_definition` inside
  `class_definition` → method; JS/TS binding-site naming for arrow functions; C
  declarator-aware extraction; C++ template/namespace; Java JUnit annotations; C#
  operator-overload identity; Ruby DSL methods.

### 6.2 What ships next (per Phase 1 deliverables)

| Step | Description | Effort | Dependencies | Status |
|---|---|---|---|---|
| 1 | Generalize `AstCodeChunker` to all 11 languages: per-language `new_*` constructors with node-kind tables | ~2 days/language × 10 = ~20 days; or ~5 days for shared pattern + per-language tables | tree-sitter parsers wired ✅ | deferred |
| 2 | Per-language test extraction (pytest collection / cargo test / JUnit / RSpec / etc. → chunk reverse-map) | ~2 days/language | step 1 | deferred |
| 3 | Per-language compiler/LSP backed type evidence (Pyright/mypy, rustc, javac, dotnet, tsc, ruby -c, gcc/clang via `compile_commands.json`, etc.) | ~2-3 days/language; some shipped (C, C++, Java, C#, Ruby per doc 09 §3.1) | step 1 | partial |
| 4 | `mejepa_compute_panel(file_path, chunk_filter)` MCP tool: per-chunk panel materialization | ~2 days | step 1 | deferred |
| 5 | Per-AST-node-type constellation centroids (6,048 = 14 × 8 × 9 × 6) at Phase 0 finalization | ~3 days | corpus across all languages, step 4 | deferred |
| 6 | Per-chunk `Gτ` guard with chunk-id-typed error variants | ~3 days | step 5 | deferred |
| 7 | Module-level cross-chunk constellation (the integration-test analog) | ~3 days | step 5 | deferred |
| 8 | E_Diff at AST granularity: tree-sitter walk over `(pre_chunk_ast, post_chunk_ast)` → diff edges | ~5 days | step 1 | deferred |
| 9 | E_Trace per-chunk: pytest `--trace` / cargo `--nocapture` parsing → reverse-map to chunk SHA | ~5 days | step 1 + per-language test extraction | deferred |
| 10 | E_Reasoning per-chunk: Claude Code tool-use sequence → chunk SHA reverse-map | ~3 days | step 1 + harness shift-log | deferred |

**Total roughly 50-70 days for full per-AST-chunk integration across all 11 languages.**
Sequence by language priority: Python first (already deepest), Rust second, then
C/C++/Java/C#/JS/TS/Ruby/Go/PHP per operator preference.

### 6.3 Phase-plan integration with doc 09

Doc 09 Phase 1 (instruments) should be re-scoped to include AST-atomic granularity from
the start:

- **Phase 1 deliverable update:** instruments operate at AST-chunk granularity, not
  whole-file. Each instrument has a `compute_for_chunk(chunk: &CodeChunk) -> Latent`
  method.
- **Phase 0 corpus update:** mutation operators run at chunk granularity (mutate one
  function within a file, not the whole file). The current Python mutation operators
  already do this for `subtle_flip` / `off_by_one` / etc.; extending to all languages
  follows the same pattern.
- **Phase 4 verify() update:** `mejepa_verify(patch)` decomposes the patch into AST
  chunks, runs `Gτ` per chunk + module-level cross-chunk guard, returns a typed verdict
  with chunk-id provenance for every gate failure.

---

## Section 7 — Why this is structurally critical for ME-JEPA

### 7.1 ΔS sharpening (UTML v1.0 §III)

ΔS = `1 − cosine(predictor_output, target)` is a single number per panel. At whole-file
granularity, that number is dominated by unchanged code. At AST-chunk granularity, ΔS
*directly measures* the semantic distance between predicted and actual chunk panels —
the loss signal goes from "noisy" to "sharp."

### 7.2 ΔK improvement (UTML v1.0 §IV)

The `cos_align(g_t, ḡ_prior)` term is more meaningful when gradients are computed per
chunk: a gradient that helps Function-A representations should align with prior gradients
on similar Function-A patterns. Cross-chunk-type gradient alignment is the deeper cousin
of NTK alignment (Atanasov 2021).

### 7.3 ΔΞ verification (UTML v1.0 §VI)

The `predictor_redundancy` term in ΔΞ is computed via pairwise MI across panel slots.
At AST granularity, redundancy detection is much sharper: if E5 and E7 always agree on
"same-shape function with same-shape control flow," then their pair signal is redundant
and one of them can be down-weighted.

### 7.4 Consolidation M(t) at chunk granularity (UTML v1.0 §VII)

Each chunk is a separate consolidation event. EWC++ Fisher importance is per-parameter
× per-chunk-type, not per-file. This protects parameters important for Class chunks
without preventing learning on Function chunks.

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

**This is what makes ME-JEPA's verdict diagnosable rather than opaque.** Operators
inspecting a refused patch see *exactly which functions/classes failed which embedder
constellations* — not "the patch is bad."

---

## Section 8 — Cross-references to update

### 8.1 Docs that need a paragraph added about AST-atomic granularity

- `09_END_GOAL_REVIEW_REPLACEMENT.md` §2.2 (instrument set) — note that each instrument
  operates at AST-chunk granularity per doc 14.
- `09_END_GOAL_REVIEW_REPLACEMENT.md` §3.1 (training corpus) — note that mutation
  operators already work at chunk granularity for Python; cross-language extension
  follows.
- `10_MEJEPA_PIVOT_2026_05_09.md` §C (cross-language mandate) — note that the 11
  parsers are wired and the chunker dispatches per-language.
- `11_MEJEPA_LEARNING_FORMULA.md` — note that L_step is computed per chunk, not per file.
- `12_EMBEDDER_INTEGRATION.md` §1.2 (signal yield) — update from "50-200 panel triples
  per /loop turn" to "500-2,000 chunk-level triples per /loop turn at typical 10
  chunks/file ratio."
- `13_UNIVERSAL_AGENT_VERIFICATION.md` §2 — update verifier to operate at chunk
  granularity; verdict structure becomes `{chunks: [(chunk_id, verdict)]}`.
- `specs/implementation-plan.md` — add Phase 1 line items for per-language chunker
  generalization (currently shows only Python and Rust as having AST infrastructure
  shipped).

### 8.2 Memory decisions that need a follow-up

- `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`
  — append "AST-atomic chunking via 11 wired tree-sitter parsers is part of the
  cross-language mandate; see doc 14."
- `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md` —
  append "Embedders operate at AST-chunk granularity (per-function/method/class/struct/
  impl-block/module), multiplying signal yield by ~10× over whole-file embedding."

---

## Section 9 — Falsification (per UTML v1.0 §XIII discipline)

This integration is **falsifiable**. Falsification scenarios:

1. **AST-atomic chunking degrades signal quality.** If per-chunk panels are too small
   (some chunks are 5 lines), the embedders may not produce stable representations.
   **Falsifies if** the linear-probe accuracy on AST-chunk panels is ≥10pp below
   whole-file panels on the same task. Mitigation: merge tiny chunks via the existing
   `merge_small_chunks` function in `ast_chunker.rs`.
2. **Per-AST-node-type constellations don't separate categories.** If the
   (KnownGood, Python, Function) centroid is closer to (KnownGood, Python, Method)
   than to (SubtleFlip, Python, Function), the node-type axis is not contributing
   useful signal — collapse to (category, language) granularity.
3. **Cross-language chunker generalization is too costly.** If extending the chunker
   to 9 languages takes >50 days, sequence by language priority (Python ✓, Rust ✓,
   C/C++ next, then Java/C#, then JS/TS/Ruby/Go/PHP).
4. **Module-level cross-chunk guard adds no value over per-chunk Gτ.** Single-chunk
   passes might already correlate with whole-module passes — skip the module-level
   guard.

If any falsification holds, the relevant section gets `status: superseded`.

---

## Section 10 — One-paragraph summary

The repo has **11 tree-sitter parsers wired** (Rust, Python, JS, TS, Go, Java, C, C++,
C#, Ruby, PHP) plus an **existing AST chunker** with FSV. By feeding ATOMIC AST chunks
(function / method / class / struct / impl-block / module — not raw line splits) through
the **14 frozen embedders** from doc 12, every code change produces panel triples *at
the AST-node granularity*. This makes `panel[t=1] − panel[t=0]` a delta over the AST
itself, not a delta over "the file got bigger by 12 lines." DDA's 105 signals/input
applies per AST chunk rather than per file — multiplying supervisory yield by ~10× to
~50K-200K signals per /loop turn, ~5M-20M per day per Claude Code instance. Per-AST-
node-type constellation centroids (6,048 = 14 embedders × 8 mutation categories × 9
languages × 6 node types) sharpen the runtime guard `Gτ` to *per-function precision*.
The verify() API decomposes any patch into AST chunks, runs Gτ per chunk + module-level
cross-chunk guard, and returns a structured verdict listing exactly which
chunk(s) failed which embedder constellation(s) — turning ME-JEPA's verdict from "the
patch is bad" into a structural diagnostic at AST-node precision. **This is the
"perfect atomic parsing" the operator demanded — quantified and grounded in the
existing tree-sitter infrastructure.**

---

## See also

- `09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan (instruments operate at AST chunk granularity).
- `10_MEJEPA_PIVOT_2026_05_09.md` — single-loop architecture + cross-language mandate.
- `11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 (L_step computed per chunk).
- `12_EMBEDDER_INTEGRATION.md` — 14 embedders × 105 signals.
- `13_UNIVERSAL_AGENT_VERIFICATION.md` — universal verifier (now at chunk granularity).
- `crates/context-graph-core/src/memory/ast_chunker.rs` — the shipped Rust chunker.
- `crates/context-graph-core/examples/ast_chunker_fsv.rs` — FSV evidence.
- `crates/context-graph-mejepa-corpus/src/` — Python AST mutation operators.
- `crates/context-graph-cli/Cargo.toml` — 11 tree-sitter parser dependencies.
- `memory/decisions/2026-05-09--multi-embedder-utml-integration-post-qwen.md` —
  embedder integration decision.
