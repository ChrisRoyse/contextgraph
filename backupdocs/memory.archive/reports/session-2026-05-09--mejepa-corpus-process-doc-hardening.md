---
namespace: contextgraph/mejepa-hardening
agent_id: codex
timestamp: 2026-05-09T20:48:23Z
status: changed
tags:
  - mejepa
  - phase0-corpus
  - docs
  - fsv
---

# ME-JEPA corpus process and docs hardening

## Summary

Hardened Phase 0 image preparation and process termination, cleaned clippy/test debt in the ME-JEPA dependency path, and repaired stale ME-JEPA roadmap links after the single-loop pivot.

## Root causes fixed

- Image preparation still relied on piped child stdout/stderr and delayed draining until process exit, so a sufficiently verbose pull could block before exit.
- Timeout process-group cleanup used `kill -TERM -<pgid>` / `kill -KILL -<pgid>` without `--`, which was not reliably terminating the process group on this host.
- Active loop/docs still referenced deleted `sitrep/` paths and renamed ME-JEPA source docs.

## Verification

- `cargo test -p context-graph-mejepa-corpus --all-targets`: 100 lib tests + 3 bin tests passed.
- `cargo clippy -p context-graph-mejepa-corpus --all-targets -- -D warnings`: passed.
- `cargo build --release -p context-graph-mejepa-corpus`: passed.
- `cargo test -p context-graph-mejepa-instruments`: 45 passed.
- `cargo test -p context-graph-core memory::ast -- --nocapture`: 27 passed.

## FSV evidence

- Happy path: `/tmp/contextgraph-mejepa-hardening-fsv/prepare-images-happy.json` shows `passes=true`, `selected_task_count=1`, target image `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`, and `missing_after_count=0`. Docker image source of truth was separately read with `docker images --format '{{.Repository}}:{{.Tag}}'`.
- Edge cases: `/tmp/contextgraph-mejepa-hardening-fsv/prepare-images-edge-cases.log` shows `max_tasks=0`, `parallel=0`, and whitespace namespace all fail before writing evidence.
- Subprocess regression: `/tmp/contextgraph-mejepa-hardening-fsv/subprocess-regression-tests.log` shows 920000 bytes written to both stdout/stderr physical log files, timeout `/proc/<pid>` absent after termination, and deterministic pull log paths.
- Docs source of truth: `/tmp/contextgraph-mejepa-hardening-fsv/docs-reference-audit.log` shows no stale `sitrep`, deleted doc filename, or old theory-path references in active loop/spec docs.

## Residual risk

There were pre-existing uncommitted `swebench.rs` timeout-report changes in the working tree. I left them intact and verified them through the corpus test suite rather than reverting user work.

