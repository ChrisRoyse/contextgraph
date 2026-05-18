---
kind: report
session: 2026-05-12-python-ast-atomic-chunking-fsv
created_at: 2026-05-12
agent: codex
repo: /home/cabdru/contextgraph
---

# Session Report: Python AST Atomic Chunking FSV

## Task
Prioritize Python AST chunking before the other AST systems, investigate the remaining GitHub issue surface, use Exa research, fix root causes without fallbacks, and prove the result with full state verification including the 300 Docker SWE-bench Lite Python corpus on the mounted D drive.

## Outcome
Complete for the Python AST chunking scope. Python chunking now uses Ruff's Python parser as the canonical Python AST source instead of tree-sitter-python. Tree-sitter remains in place for non-Python languages.

## Root Cause
The previous Python AST chunker relied on tree-sitter-python. The SWE-bench FSV exposed a valid Matplotlib Python file that compiled inside Docker but was rejected by tree-sitter-python because of an f-string format-spec construct. The mismatch meant valid Python source could fail chunking before any downstream embedding/retrieval stage.

Two additional atomicity defects were found during real corpus verification:
- Nested method-only classes inside a function needed a synthetic `pass` shell indented to the class scope, not a fixed four spaces.
- One-line classes such as `class _EmptyClass: pass` needed normalized class shells instead of duplicating the header.

## Implementation
- Added Ruff parser dependencies to `crates/context-graph-core/Cargo.toml`.
- Routed Python chunking through `ruff_python_parser::parse_module`.
- Preserved imports, module globals, class shells, functions, methods, decorators, and parent chains.
- Built class shells that exclude nested method/function/class bodies while preserving direct class docstrings/attrs.
- Added token-based Python structural hashes that ignore whitespace-only formatting changes.
- Kept fail-closed parse behavior with `MEJEPA_CHUNKER_PARSE_FAILED` diagnostics.
- Removed Python tree-sitter docstring/module whole-file classification from the generic classifier path.
- Added synthetic and SWE-bench FSV examples:
  - `crates/context-graph-core/examples/python_ast_atomic_fsv.rs`
  - `crates/context-graph-core/examples/swebench_python_ast_corpus_fsv.rs`
- Fixed stale clustering doctests found during full package verification: examples now derive all 14 embedder dimensions from `Embedder`/`get_dimension`; topic examples use 14-value arrays.

## Exa Research Used
- Ruff Python parser README: parser builds a Python AST and tracks current Python syntax support.
- Ruff PEP 701 f-string parser work: validates the choice for modern f-string grammar coverage.
- tree-sitter-python f-string issue #164: confirms the parser-family risk around f-string interpolation/format-spec parsing.
- cAST paper / ACL Anthology: supports structure-preserving AST chunks and sibling-aware semantic chunking for code RAG.
- NVIDIA CUDA 13.2 and Blackwell tuning docs: AST parsing stays CPU-bound; GPU work should be reserved for batched embedding/model inference with Blackwell-aware builds and CUDA/TensorRT graph/batching where applicable.

## Source Of Truth
- Synthetic Python FSV source/evidence:
  - `/mnt/d/contextgraph/fsv/python-ast-atomic-fsv/sources`
  - `/mnt/d/contextgraph/fsv/python-ast-atomic-fsv/python-ast-atomic-fsv.json`
- SWE-bench corpus FSV source/evidence:
  - `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json`
  - `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json`
  - `/mnt/d/contextgraph/docker-desktop-data`
  - `/mnt/d/contextgraph/cache/mejepa-corpus/repos`
  - `/mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/sources`
  - `/mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/swebench-python-ast-corpus-fsv.json`

## FSV Evidence
- Synthetic happy path: passed; 9 chunks, 2 methods, 1 class, 1 module, `.pyc` read back from `__pycache__`.
- Synthetic edge cases: empty input, invalid syntax, maximum 40-method class all passed/fail-closed as expected.
- SWE-bench 300-container corpus: passed.
  - `task_count=300`
  - `repo_count=12`
  - `docker_image_count=300`
  - `docker_py_compile_pass_count=300`
  - `docker_git_sha_match_count=300`
  - `source_files_after=300`
  - `total_chunks=27064`
  - `total_classes=1878`
  - `total_methods=16537`
  - `total_functions=2721`
  - `total_modules=234`
- SWE-bench edge cases: missing Docker image, missing source in image, invalid Python source all fail closed with expected codes.
- Evidence file permissions were read back as `0600`.

## Verification Commands
- `cargo test -p context-graph-core memory::ast::tests:: -- --nocapture`
- `cargo check -p context-graph-core --examples`
- `cargo run -p context-graph-core --example python_ast_atomic_fsv`
- `CONTEXTGRAPH_AST_FSV_DOCKER_CONCURRENCY=8 cargo run -p context-graph-core --example swebench_python_ast_corpus_fsv`
- `stat -c '%a %s %n' /mnt/d/contextgraph/fsv/python-ast-atomic-fsv/python-ast-atomic-fsv.json /mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/swebench-python-ast-corpus-fsv.json`
- `jq ... /mnt/d/contextgraph/fsv/python-ast-atomic-fsv/python-ast-atomic-fsv.json`
- `jq ... /mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/swebench-python-ast-corpus-fsv.json`
- `find /mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/sources -type f -name '*.py' | wc -l`
- `cargo test -p context-graph-core --lib`
- `cargo test -p context-graph-core --doc`
- `cargo test -p context-graph-core`
- `cargo fmt --check`
- `git diff --check`

## Resume Here Next Session
Continue with the non-Python AST issue surface only after this Python baseline is preserved. The legacy `AstCodeChunker` convergence/deprecation and other language AST systems are still separate work; do not replace this Python Ruff parser path with a tree-sitter fallback.
