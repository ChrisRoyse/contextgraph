---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T09:41:23Z
status: active
tags: mejepa, ast, embedders, routing, fsv, root-cause, smoke-test
---

# AST Test Classification Root Cause and FSV

## Discovery

The AST classifier was overusing path-based test detection. Functions and methods in test paths were classified as `TestFunction` even when the language's own test discovery rules would treat them as production helpers. This polluted chunk entity labels and routed helper code toward test embedders.

Observed failure mode:
- JavaScript helper functions inside `tests/*.test.js` were labeled as `TestFunction`.
- Java methods inside `src/test/java` were labeled as `TestFunction` solely from the path.
- C# methods like `TestimonyScore` inside `Tests/*.cs` were vulnerable to false-positive test labeling.

## Fix

`crates/context-graph-core/src/memory/ast/classify.rs` now requires language-specific test signals:
- Rust: `#[test]`/test attribute, not path alone.
- Python/JS/TS/Java/C#: test-like function or method names/annotations, not path alone.
- Go: `_test.go` plus valid `Test`/`Benchmark`/`Fuzz` signature markers, with `Example*` handled.
- PHP/Ruby retain framework-specific signals without broad path-only helper promotion.

`crates/context-graph-core/src/memory/ast/tests.rs` adds `test_path_helpers_keep_production_entity_types`, covering JS, Java, and C# false positives.

## Verification

Commands passed:
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-core memory::ast -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-mejepa-instruments --all-targets -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p context-graph-mejepa-corpus -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-core --example ast_chunker_multilang_fsv`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv`
- `CARGO_BUILD_JOBS=1 cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`

Source-of-truth readback:
- `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`: 11 languages, all chunk counts nonzero; edge cases fail closed with `MEJEPA_CHUNKER_EMPTY_SOURCE`, `MEJEPA_CHUNKER_PARSE_FAILED`, and `MEJEPA_CHUNKER_MIXED_LANGUAGE`.
- `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/panel.json`: 5120 persisted values, `filled_mask=32767`, nonzero_count 964.
- `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`: 11 real sample files plus SWE-bench manifest `sympy__sympy-20639`, all 15 panel slots filled.
- `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json`: 5120 persisted values, `filled_mask=32767`, nonzero_count 828.
- `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`: 11 edge cases all rejected with `MEJEPA_INSTRUMENTS_INVALID_INPUT`.
- `/tmp/contextgraph-phase0-smoke-one-task/index.json`: one real SWE-bench instance `sympy__sympy-20590`, 8 mutation categories, complete index.
- `/tmp/contextgraph-phase0-smoke-one-task/oracle-store`: persisted RocksDB oracle store with `mutation_cf_count=8` and `verdict_cf_count=8`; `mejepa-corpus verify` reported `passes=true`.

Additional hardening:
- `crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs` now writes its FSV directory as `0700`, evidence files as `0600`, syncs writes, and byte-verifies readback.
