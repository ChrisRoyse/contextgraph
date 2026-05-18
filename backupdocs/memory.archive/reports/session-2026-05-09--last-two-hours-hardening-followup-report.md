---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T05:23:05Z
status: completed_with_existing_warnings
tags:
  - last-two-hours
  - hardening
  - dynamicjepa
  - mejepa-corpus
  - tier-compression
  - fsv
---

# Last-Two-Hours Hardening Follow-Up Report

## Scope

Reviewed recent history through `0f9b3c2e` on `ccreality-cutover` plus the dirty worktree that appeared during the session. Read the solo-agent memory protocol and the requested RuVector findings docs before acting. Researched parser/validation/durable-write practices from official Python and Rust documentation and used local crate/source inspection for Tree-sitter ABI, Cargo patching, CMake CUDA compiler selection, and llama build-system root causes.

## Hardened Areas

- ME-JEPA Python mutation corpus: verified parser-backed lexical masking, py_compile readbacks, real SWE-bench Lite split assignments, duplicate/whitespace/fraction edge cases, and generated-source SHA-256 evidence.
- Rust AST chunker: verified fail-closed parse diagnostics for ERROR/MISSING nodes, method classification under impl blocks, rustc metadata source-of-truth readbacks, and release FSV output.
- Tier compression: verified canonical encode/decode/serialize/deserialize behavior, exact payload length rejection, fsync/readback evidence, SHA-256s for binary blobs, unit tests, clippy, and release build.
- DynamicJEPA language semantics: verified C#/Java/C++ happy and fail-closed paths after the Tree-sitter 0.26 ABI fix; then hardened TypeScript/TSX owner attribution, JavaScript binding-symbol dedupe, C# `.slnx` target selection/framework-test classification, and compiler/LSP failure diagnostics in the remaining dirty diff.
- No-default-feature build gaps: verified the no-default CLI/reality-loop semantic tests so parser-only hardening remains testable without the llama default-feature build.
- Default-feature CLI release build: vendored the locked `llama-cpp-sys-2 0.1.145` crate through `[patch.crates-io]`, made CMake parallelism caller-controlled and fail-fast, pinned CUDA 13.2 compiler/toolkit/Blackwell architecture, and forced clean CMake reconfiguration after failed compiler probes.

## Verification

- `cargo test -p context-graph-mejepa-corpus -- --nocapture`: 50 passed.
- `cargo clippy -p context-graph-mejepa-corpus --all-targets -- -D warnings`: passed.
- `cargo build --release -p context-graph-mejepa-corpus`: passed.
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`: passed; evidence `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`, SHA-256 `df411b067e403744b781a5211fcc8573341c9ca70d82f5fef9ce599ebaf0871e`.
- `cargo run --release -p context-graph-core --example ast_chunker_fsv`: passed; evidence `/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json`, SHA-256 `9c03fbe01d433b3f0c8b5ce3587b0c725747daa722a33f0fdf4a28824ab7ee61`.
- `cargo test -p context-graph-core --lib ast_chunker -- --nocapture`: 17 passed, with existing unused-import warnings outside the AST chunker change.
- `cargo test -p context-graph-tier-compression -- --nocapture`: 24 passed.
- `cargo clippy -p context-graph-tier-compression --all-targets -- -D warnings`: passed.
- `cargo build --release -p context-graph-tier-compression`: passed.
- `cargo run -p context-graph-tier-compression --example tier_compression_fsv`: passed; evidence `/tmp/contextgraph-tier-compression-fsv/tier-compression-evidence.json`, SHA-256 `5d0b1941edee599db01905f0d669af672996098824723bac22f1440b36cfc4b0`.
- CLI targeted default-feature tests passed earlier for `csharp_`, `java_semantic_extraction`, `dynamicjepa_java`, `dynamicjepa_ast_action_semantics_cpp`, and `parse_real_cpp_header`, with existing dead-code warnings in path-policy modules.
- CLI no-default-feature tests passed for `typescript_semantic_extraction_uses_tsx_dialect_and_binding_symbols`, `dynamicjepa_typescript`, and `csharp_semantic_extraction_recognizes_framework_test_attributes`.
- Final no-default CLI readback tests passed for `javascript_semantic_extraction_uses_binding_symbols_and_export_visibility`, `dynamicjepa_typescript_ast_uses_tsx_dialect_and_binding_symbols`, `csharp_semantic_extraction_recognizes_framework_test_attributes`, and `dynamicjepa_csharp_ast_preserves_operator_symbols_and_slnx_targets`.
- `cargo build --release -p context-graph-cli`: passed after vendored llama/CUDA build hardening; release binary SHA-256 `f1130bb309cb74ee3e981423509a8b14843a89f5a106ad8c2b7d86ac2cea0355`.
- Release source-of-truth readbacks: `target/release/context-graph-cli` size `78850208`, timestamp `2026-05-09 00:15:50.109107370 -0500`; `target/release/context-graph` SHA-256 `1de6860d5dc53425adc33c878c2b1ac6999c09399c18142e27b4f2af174c1b96`; `target/release/blackbox` SHA-256 `57a1e116b0c72499c207b8b35db5975fa8d5d0544a3f0ff251c12b5cf0c9ca01`.
- Llama native source-of-truth readbacks: `libggml-cuda.a` size `57739774`, SHA-256 `8137d2f1588195feb9f60bed9a140bf8c3d59aefc43a6163b7aa09d9d2f41eb9`; `CMakeCache.txt` contains `CMAKE_CUDA_COMPILER=/usr/local/cuda-13.2/bin/nvcc`, `CMAKE_CUDA_ARCHITECTURES=120a-real`, and `CUDAToolkit_ROOT=/usr/local/cuda-13.2`.
- Runtime smoke readback: `target/release/context-graph-cli --help` printed the expected command surface including `dynamicjepa`, `blackbox`, and `ccreality`.
- `cargo fmt --all --check`: passed after final formatting.
- `rustfmt --check vendor/llama-cpp-sys-2-0.1.145/build.rs`: passed.
- `git diff --check`: passed.

## Resolved Blocker

`cargo build --release -p context-graph-cli` was blocked under default features by three separate build-system root causes:

- Attempt 1 failed in `context-graph-embeddings` with `rustc-LLVM ERROR: out of memory`.
- Attempt 2 got past embeddings and failed in `llama-cpp-sys-2` CMake after the dependency's `build.rs` forced host-parallel CMake/gmake (`-j30`), producing repeated CUDA/C++ `out of memory` / `virtual memory exhausted` diagnostics.
- After parallelism was fixed, CMake selected the distro CUDA 12 `/usr/bin/nvcc` while finding CUDA 13.2 headers, causing compiler identification to fail on `sm_52`.
- After CUDA selection was fixed, a half-configured CMake directory was incorrectly reused and failed with `No rule to make target 'Makefile'`.

The durable project fix is now in the tree via the vendored dependency patch and project-local Cargo config. The patched build prints the selected CMake parallelism, CUDA root, CUDA compiler, and CUDA architecture, and fails fast on invalid parallelism or CUDA versions older than 12.8.

## Worktree Note

At report time HEAD is `0f9b3c2e`. The remaining tracked dirty diff is from concurrent docs/backup work plus a rustfmt normalization in `vendor/llama-cpp-sys-2-0.1.145/build.rs`; the larger docs and backup deletions were left untouched.
