# Discovery: ME-JEPA Embedder/AST Smoke Root Causes

Date: 2026-05-09

## Root Causes

1. AST test-path classification contaminated production chunks.
   - Rust/Python/JS/Java/C# helper functions in test paths were being classified as `TestFunction` by path alone.
   - That routed normal helper/method code to the test-function route (`E1`,`E7`) instead of production function/method route (`E7`,`E14`).
   - Fixed by requiring semantic test markers: language attributes/annotations or test-like names with a real test prefix boundary.

2. E14 BGE-M3 had a zero-vector fallback.
   - `ProductionMultiArrayProvider` used `E14ZeroAdapter` when `models/bge-m3-dense` was missing.
   - This made the provider appear ready while multilingual/long-context dense retrieval was off.
   - Fixed by deleting the adapter and adding fail-closed asset validation for `config.json`, `model.safetensors`, and `tokenizer.json`.

3. Full production embedder readiness is blocked by missing local model assets.
   - Source of truth: `/tmp/contextgraph-mejepa-embedders-fsv/embedder_smoke_preflight.json`.
   - GPU ready: RTX 5090, CC 12.0, 32607 MiB total VRAM.
   - SWE-bench Lite manifests ready: 300/300 checked-in manifests.
   - Model assets not ready: 32 missing required files under `/home/cabdru/contextgraph/models`.

## Verified Evidence

- AST routing FSV: `/tmp/contextgraph-mejepa-embedders-fsv/routing_coverage.json`, 143 rows.
- Multi-language AST chunker FSV: `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`, 11 languages and 3 fail-closed edge cases.
- AST-to-panel smoke FSV: `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`, 5120-d panel, filled mask `32767`, 15 filled slots.
- Embedder asset smoke FSV: `/tmp/contextgraph-mejepa-embedders-fsv/embedder_smoke_preflight.json`, intentionally fails because production assets are absent.

## Notes

- `cargo test -p context-graph-core memory::ast::tests -- --nocapture` passed: 10 tests.
- `cargo test -p context-graph-mejepa-instruments code_instruments_accept_real_multilanguage_ast_sources -- --nocapture` passed.
- `cargo run -p context-graph-core --example embedder_routing_fsv` passed.
- `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` passed.
- `cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv` passed.
- `cargo check -p context-graph-embeddings --lib` could not be captured to completion; repeated attempts were interrupted during `librocksdb-sys` compile/check. No OOM or disk pressure was observed.
