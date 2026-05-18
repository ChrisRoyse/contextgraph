---
namespace: journal
created: 2026-05-09T10:00:00Z
updated: 2026-05-09T10:00:00Z
status: active
tags: mejepa, phase0, phase1, fsv, ast, corpus, smoke
---

# Success: Phase 0 And Phase 1 FSV Smoke

## What Happened

Implemented and verified the Phase 1 reusable AST chunker/routing surface and reran the Phase 0 corpus FSV plus one-task Docker-backed operational smoke. Evidence was written under `/tmp/contextgraph-*` and then read back with separate commands.

## Verification

- `cargo test -p context-graph-core ast::tests --no-default-features --jobs 1`
- `cargo run -p context-graph-core --example ast_chunker_multilang_fsv --jobs 1`
- `cargo run -p context-graph-core --example embedder_routing_fsv --jobs 1`
- `cargo run -p context-graph-core --example swebench_ast_fsv --jobs 1`
- `cargo test -p context-graph-mejepa-instruments --all-targets --jobs 1`
- `cargo run -p context-graph-mejepa-instruments --example ast_embedder_smoke_fsv --jobs 1`
- `cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv --jobs 1`
- `cargo test -p context-graph-mejepa-corpus --jobs 1`
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv --jobs 1`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- generate --output /tmp/contextgraph-phase0-smoke-one-task --instance-id sympy__sympy-20590 --run-id-prefix phase0smoke --instance-timeout-secs 1800 --overall-timeout-secs 3600`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- verify --corpus /tmp/contextgraph-phase0-smoke-one-task`
- `cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- stats --corpus /tmp/contextgraph-phase0-smoke-one-task`

## Source Of Truth

- Phase 1 AST evidence: `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json`
- Phase 1 routing evidence: `/tmp/contextgraph-mejepa-embedders-fsv/routing_coverage.json`
- Phase 1 SWE-bench AST evidence: `/tmp/contextgraph-swebench-ast-fsv/swebench-ast-evidence.json`
- Phase 1 panel evidence: `/tmp/contextgraph-mejepa-ast-embedder-smoke-fsv/ast-embedder-smoke-evidence.json`
- Phase 0 smoke corpus: `/tmp/contextgraph-phase0-smoke-one-task/index.json` and `/tmp/contextgraph-phase0-smoke-one-task/oracle-store`

## Related

- See also: `memory/discoveries/2026-05-09--phase1-core-ast-routing-and-fsv-source-of-truth.md`
- Tags: mejepa, phase0, phase1, fsv, ast, corpus
