---
namespace: journal
created: 2026-05-09T03:59:44Z
updated: 2026-05-09T03:59:44Z
status: active
severity: medium
tags: failure, verification, cli, semantic-extraction, rustc, oom
---

# Failure: CLI Semantic-Extraction Test Sweep OOM

## What Happened
During the last-four-hours hardening pass, `cargo test -p context-graph-cli --lib semantic_extraction -- --nocapture`
was attempted to verify the recent tree-sitter semantic extraction changes. The command compiled partway through
`context-graph-cli` and then `context-graph-mcp`; `rustc-LLVM ERROR: out of memory` aborted compilation before any
filtered tests ran.

## Why It Matters
This is not evidence that semantic extraction behavior failed. It means the broad CLI lib test target pulls in
large MCP/CUDA/LLM dependencies and can exceed available compiler memory under concurrent workspace activity.
Future verification should prefer narrower binaries/examples or isolate the target directory and jobs when a full
CLI sweep is needed.

## Related
- Successful narrower checks in this session: `context-graph-mejepa-instruments`, `context-graph-tier-compression`,
  `context-graph-mejepa-corpus`, `context-graph-core --example ast_chunker_fsv`, and `samplecode` tree-sitter parser
  exercise.
- Tags: cli, rustc, oom, semantic-extraction
