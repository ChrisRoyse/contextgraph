# Session Report: Legacy AstCodeChunker Convergence

Date: 2026-05-12T21:41:35Z

## Scope

Resolved GitHub issue #6: `[DEBT] Converge or deprecate legacy AstCodeChunker`.

## Root Cause

The old `memory::ast_chunker` compatibility surface remained attractive to callers while the canonical implementation had moved to `memory::ast`. That created two hazards:

- internal callers could keep initializing a Rust-only chunker;
- deprecated types emitted warnings inside bridge modules and FSV examples, obscuring the intended migration path.

## Changes

- Kept `AstCodeChunker` as a deprecated source-compatible bridge backed by `context_graph_core::memory::ast`.
- Switched `CodeFileWatcher` from `default_rust` / `new_rust` to canonical multi-language initialization.
- Added the canonical supported extension set for the watcher: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP.
- Added local deprecated-boundary allowances where the bridge is intentionally used.
- Fixed canonical Python text chunk construction by consolidating the optional-hash path into `TextChunkInput`.
- Added FSV examples:
  - `crates/context-graph-core/examples/ast_codechunker_convergence_fsv.rs`
  - `crates/context-graph-core/examples/legacy_ast_chunker_parity_fsv.rs`

## Full State Verification

Primary source of truth:

- `/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv/ast-codechunker-convergence-fsv.json`

Physical readback summary:

- evidence JSON: 24,350 bytes, sha256 `d6f81e8c449de56536b0d3807c62e369313c94446319f6ae2ca90308bdb66f80`
- Rust happy source: `/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv/sources/happy_solver.rs`, sha256 `9f5fdcb49c9acd7ade59522a30ffee3429ece4700439170bec09d8190253baef`
- Python happy source: `/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv/sources/test_solver.py`, sha256 `201496d587f36220f494ff015cc02a915f94122f2c9614252e989179d71a2828`
- Rust toolchain output exists: `/mnt/d/contextgraph/fsv/ast-codechunker-convergence-fsv/toolchain/happy_solver.rmeta`
- Python bytecode output exists under `sources/__pycache__/`

FSV results:

- Rust happy path: pass, 8 canonical chunks match 8 legacy chunks.
- Python happy path: pass, 6 canonical chunks match 6 legacy chunks.
- Empty input edge: pass, canonical and legacy both produce `MEJEPA_CHUNKER_EMPTY_SOURCE`.
- Invalid Rust edge: pass, `rustc` fails and both chunkers produce `MEJEPA_CHUNKER_PARSE_FAILED` with diagnostics.
- Unsupported extension edge: pass, canonical and legacy both produce `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.

Secondary FSV:

- `/mnt/d/contextgraph/fsv/legacy-ast-chunker-parity-fsv/legacy-ast-chunker-parity-fsv.json`
- evidence JSON: 8,979 bytes, sha256 `bbbbafcf750c0208b047ec5b114e090e5c8e1ceca45311c25ec197884d5aa731`
- all happy cases and 3 boundary cases passed.

## Commands

- `cargo run -p context-graph-core --example ast_codechunker_convergence_fsv`
- `cargo run -p context-graph-core --example legacy_ast_chunker_parity_fsv`
- `cargo test -p context-graph-core memory:: -- --nocapture`
- `cargo test -p context-graph-core --examples --no-run`
- `git diff --check`

## Web Research

Used Exa MCP results for:

- Rust deprecation/source compatibility: Rust Reference `deprecated` attribute and Cargo SemVer guidance.
- Tree-sitter parser error behavior: parser docs and ERROR/MISSING node handling.
- AST-aware chunking: cAST paper on syntax-aligned chunks and split/merge strategies.
