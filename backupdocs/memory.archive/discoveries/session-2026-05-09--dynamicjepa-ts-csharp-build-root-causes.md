---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T04:15:11Z
status: active
tags:
  - dynamicjepa
  - typescript
  - csharp
  - build-system
  - fsv
---

# DynamicJEPA TypeScript/C# And Build-System Root Causes

## Findings

- TypeScript/TSX semantic extraction needed dialect-specific handling for TSX and binding-site identity. The hardened path now treats interfaces/type aliases as first-class symbols, extracts exported function bindings from lexical declarations, and refuses parser-only action semantics when project-local `node_modules/.bin/tsc` is missing.
- C# semantic extraction needed framework-attribute classification, not only path/name heuristics. The hardened path now recognizes `[Fact]`, `[Theory]`, `[TestMethod]`, and `[TestCase]` as tests while leaving ordinary helper methods classified as methods.
- No-default-feature compilation exposed real helper gaps that default-feature runs had not surfaced: PHP attribute-tree walkers returned iterator temporaries tied to a dropped tree-sitter cursor, and `reality-loop` referenced PHP/TypeScript helper names that needed concrete implementations in that binary.
- Full default-feature CLI release verification is blocked by build-system capacity, not semantic test failure. `llama-cpp-sys-2 v0.1.145` sets `CMAKE_BUILD_PARALLEL_LEVEL` from `std::thread::available_parallelism()` inside its published `build.rs`, overriding caller intent. On this host it forced CMake/gmake to `-j30`, which exhausted memory while compiling CUDA/C++ `ggml` units. A durable project fix would require a dependency patch/upstream fix to make that parallelism configurable, or separating the llama-backed binary behind an explicit feature boundary.

## Evidence

- `cargo test -p context-graph-cli --no-default-features typescript_semantic_extraction_uses_tsx_dialect_and_binding_symbols -- --nocapture`: passed; read back `typescript-semantic-fsv.json` with TypeScript and TSX source hashes and exported symbol rows.
- `cargo test -p context-graph-cli --no-default-features dynamicjepa_typescript -- --nocapture`: passed 2 action-side tests, including missing local `tsc` fail-closed behavior.
- `cargo test -p context-graph-cli --no-default-features csharp_semantic_extraction_recognizes_framework_test_attributes -- --nocapture`: passed; printed the actual C# symbol table with four framework test methods and one helper method.
- `cargo build --release -p context-graph-cli`: failed twice under default features. First failure was `rustc-LLVM ERROR: out of memory` in `context-graph-embeddings`; second got past that and failed in `llama-cpp-sys-2` CMake with multiple `out of memory` / `virtual memory exhausted` diagnostics after forcing `-j30`.
