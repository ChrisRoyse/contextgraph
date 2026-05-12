# 05 — AST Chunker Subsystem

Source files covered: `crates/context-graph-core/src/memory/ast/lib.rs`, `crates/context-graph-core/src/memory/ast/chunker.rs`, `crates/context-graph-core/src/memory/ast/classify.rs`, `crates/context-graph-core/src/memory/ast/entity_types.rs`, `crates/context-graph-core/src/memory/ast_chunker.rs` (legacy), `crates/context-graph-cli/src/commands/dynamicjepa.rs` (multi-language semantic extractor), `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`, `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md §2`.

> **Status**: New 11-language API **shipped** at `context_graph_core::memory::ast`. Legacy `AstCodeChunker` (Rust-only) is being converged ([#6](https://github.com/ChrisRoyse/contextgraph/issues/6)). Reformat-invariant chunk-SHA audit pending ([#12](https://github.com/ChrisRoyse/contextgraph/issues/12)).

---

## 1. Purpose

Produce **AST-perfect atomic chunks** bounded by syntactic entities (function / class / impl / module-decl / trait / macro / interface / type-alias / ...). Every code-domain ME-JEPA instrument (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test) consumes these chunks — **never raw text, never fixed-size windows**.

Target chunk size: **~500 non-whitespace characters**, bounded by AST node, per the Qodo / cAST methodology in `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §3`.

---

## 2. Supported languages (11)

Each backed by a `tree-sitter` grammar at version 0.23.x.

| # | Language | tree-sitter crate | EntityTypes recognized |
|---|---|---|---|
| 1 | Rust | `tree-sitter-rust` | Function, Method, Impl, Trait, Struct, Enum, Module, Macro, TypeAlias, Const, Static |
| 2 | Python | `tree-sitter-python` | Function, Method, Class, Module, AsyncFunction, Decorator |
| 3 | JavaScript | `tree-sitter-javascript` | Function, ArrowFunction, Class, Method, Module |
| 4 | TypeScript | `tree-sitter-typescript` | Function, ArrowFunction, Class, Method, Interface, TypeAlias, Enum, Module |
| 5 | Go | `tree-sitter-go` | Function, Method, Type, Package, Interface, Struct |
| 6 | Java | `tree-sitter-java` | Class, Method, Interface, Enum, Package |
| 7 | C | `tree-sitter-c` | Function, Struct, Typedef, Enum, Macro |
| 8 | C++ | `tree-sitter-cpp` | Function, Class, Struct, Namespace, Method, Template, Enum |
| 9 | C# | `tree-sitter-c-sharp` | Class, Method, Interface, Struct, Namespace, Property, Enum |
| 10 | Ruby | `tree-sitter-ruby` | Method, Class, Module, Block |
| 11 | PHP | `tree-sitter-php` | Function, Class, Method, Trait, Interface, Namespace |

---

## 3. Public API

### 3.1 Types

```rust
pub enum Language {
    Rust, Python, JavaScript, TypeScript, Go, Java,
    C, Cpp, CSharp, Ruby, Php,
}

pub enum EntityType {
    Function, Method, Class, Impl, Trait, Module,
    Struct, Enum, Interface, TypeAlias, Macro,
    Namespace, Package, Property, Const, Static,
    AsyncFunction, Block, Template, Decorator, Other(String),
}

pub struct Chunk {
    pub chunk_id: Sha256,            // SHA-256 of (language, kind, text)
    pub language: Language,
    pub kind: EntityType,
    pub byte_range: (usize, usize),
    pub line_range: (u32, u32),
    pub text: String,                // chunk content
    pub sha: Sha256,                 // same as chunk_id for now
}

pub struct ChunkOptions {
    pub target_chars: usize,         // default 500 non-whitespace
    pub max_chars: usize,            // default 800 hard cap
    pub include_doc_comments: bool,  // default true
    pub include_imports_as_module: bool, // default true
    pub reject_on_parse_error: bool, // default true
}
```

### 3.2 Functions

```rust
// Primary entry point.
pub fn chunk_with_options(
    source: &str,
    language: Language,
    opts: ChunkOptions,
) -> Result<Vec<Chunk>, ChunkError>;

// Quick classifier without chunking.
pub fn classify(source: &str, language: Language)
    -> Result<Vec<(EntityType, u32, u32)>, ChunkError>;

// Language detection by file extension.
pub fn detect_language(path: &Path) -> Option<Language>;

// Detect mixed-language files (HTML+JS, PHP+HTML); rejects.
pub fn detect_mixed_language(source: &str, language: Language) -> bool;
```

### 3.3 Errors

| Error | Trigger |
|---|---|
| `MEJEPA_CHUNKER_PARSE_FAILED` | tree-sitter ERROR or MISSING node; emits `(byte_offset, line, node_kind)` diagnostics. Never falls back to text chunking. |
| `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` | language not in the 11-language matrix |
| `MEJEPA_CHUNKER_EMPTY_SOURCE` | empty source; no empty chunks emitted |
| `MEJEPA_CHUNKER_MIXED_LANGUAGE` | embedded language detected (HTML+JS, PHP+HTML); reject for now; future work routes per-block |

---

## 4. Algorithm

For each call to `chunk_with_options(source, language, opts)`:

1. **Parse**: `Parser::new().set_language(grammar)?; parser.parse(source, None).ok_or(...)`. Fail closed on `ERROR` / `MISSING` nodes.
2. **Mixed-language detection**: per-language pattern check (e.g., `<script>` blocks in PHP; `<?php` in HTML). If detected, return `MEJEPA_CHUNKER_MIXED_LANGUAGE`.
3. **Walk the AST**: depth-first traversal using `tree-sitter::TreeCursor`. For each node, ask the per-language classifier (`classify.rs`) whether it's a chunkable entity (Function, Class, Impl, etc.).
4. **Entity boundary selection**: at each chunkable node, emit a chunk if its size ∈ [target_chars × 0.3, max_chars]. If too large, recursively walk into sub-entities (e.g., methods inside a class). If too small, merge with adjacent siblings at the parent level (configurable; default merges trivial siblings).
5. **Capture doc comments**: leading doc comments are absorbed into the chunk (per language: `///` for Rust, `"""..."""` for Python, `/** */` for JSDoc/Javadoc).
6. **Module preamble**: leading imports / `use` / `package` statements form a single `Module` chunk per file (if `include_imports_as_module=true`).
7. **Compute SHA-256**: `chunk_id = sha256(format!("{lang}|{kind:?}|{text}"))`. **Reformat-invariant audit pending** ([#12](https://github.com/ChrisRoyse/contextgraph/issues/12)).
8. **Return** `Vec<Chunk>` in source-byte order.

---

## 5. Python-specific notes

- `python_lex.rs` (in `context-graph-mejepa-corpus`, shared with the chunker by intent) provides a **triple-quote / f-string-aware masker** so docstrings and f-strings don't get mistakenly mutated by mutation operators or chunked as ordinary code.
- Decorators are absorbed into the following function chunk.
- `async def` becomes `EntityType::AsyncFunction` to distinguish concurrency-related chunks for `E_CFG`.

## 6. Rust-specific notes

- `impl <Type>` blocks become `EntityType::Impl` chunks. Methods inside are emitted as sub-chunks if the impl block is large.
- `mod foo { ... }` becomes one chunk for the mod-decl line + sub-chunks for inner items if large.
- `macro_rules!` becomes `EntityType::Macro`.

## 7. JS/TS-specific notes

- Both `function foo() {}` and `const foo = () => {}` become chunks; the arrow form maps to `EntityType::ArrowFunction`.
- TypeScript-only: `interface`, `type alias`, `enum` are chunked separately.
- Class methods are sub-chunks.

## 8. Cross-language entity-type matrix (per `docs/ruvectorfindings/14`)

Routing in `crates/context-graph-mejepa-embedders/src/routing.rs` uses this `(Language, EntityType)` 2-tuple for the **143-row** embedder-set lookup. Languages × EntityTypes that are physically impossible (e.g., `Impl` for Python) are absent from the routing table.

## 9. FSV evidence

| FSV | Source | Output |
|---|---|---|
| Multi-language chunking | `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` | `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json` + per-language dirs |
| Per-language toolchain check | `cargo run -p context-graph-{rust,python,c,cpp,java,csharp,go,js,ts,ruby,php}-ast-chunker-fsv` | per-language `ast-chunker-evidence.json` |
| Mixed-language rejection | included in multilang FSV | PHP/HTML mixed-language edge case readback passes |

---

## 10. What this subsystem commits to

1. Atomic chunks bounded by AST entities; never text-window or fixed-size.
2. SHA-256 chunk IDs computed from `(language, kind, text)`; reformat-invariance is **partial** ([#12](https://github.com/ChrisRoyse/contextgraph/issues/12)).
3. Fail-closed on parse errors with byte-offset diagnostics. Never fall back to text chunking.
4. Reject mixed-language sources rather than partial-handle.
5. 11 languages × ~13 entity types per language ≈ 143 routing rows.
6. Cross-references: [`06_embedder_substrate.md`](06_embedder_substrate.md) (routing), [`07_instruments_panel.md`](07_instruments_panel.md) (chunk consumers), [`14_corpus_phase0.md`](14_corpus_phase0.md) (mutation operators on chunks).

## 11. Convergence work

- **Legacy `AstCodeChunker` (Rust-only)** in `crates/context-graph-core/src/memory/ast_chunker.rs` needs to converge to the new API or be deleted. Tracked: [#6](https://github.com/ChrisRoyse/contextgraph/issues/6).
- **Reformat-invariant chunk-ID audit**: equivalent AST node IDs across reformatting must produce equivalent chunk SHAs. Tracked: [#12](https://github.com/ChrisRoyse/contextgraph/issues/12).
