---
namespace: reports
agent_id: codex
created: 2026-05-12T02:43:34Z
updated: 2026-05-12T02:43:34Z
status: active
tags: report, mejepa, corpus, hardening, fsv, swebench, wsl
---

# Session Report: ME-JEPA Corpus Durable Write Hardening

## Outcome

Fixed the durable-write root cause and verified with real SWE-bench data. No
GitHub issue was filed because the observed issue was fixed in this session.

## Root Cause

The ENOENT failures happened inside `write_text_checked`, but the deeper cause
was durable patch-file writes to `/mnt/d/contextgraph/corpus` occurring inside
the rayon patch-generation closure. On WSL2, `/mnt/d` is a mounted Windows drive
and is the wrong place for high-concurrency Linux file creation. Serial
directory pre-creation did not address concurrent atomic file creation.

The previous partial production output also exposed a state-integrity gap:
`index.json` and RocksDB had 1,500 rows, while `mutations/` contained 1,800
patch files from interrupted attempts.

## Code Changes

- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`
  - Rayon workers now generate patch bytes and predictions only.
  - Durable patch files are written serially after oracle verdicts return.
  - Resume validates patch-file set and RocksDB row set against `index.json`.
  - Docker query calls and official patch loading use timed subprocess capture.
  - `write_text_checked` uses unique `create_new` temp files, fsync, rename,
    parent fsync, and immediate readback with detailed failure context.
- `crates/context-graph-mejepa-corpus/src/timed_subprocess.rs`
  - New timed subprocess helper with stdout/stderr drain threads and structured
    timeout errors.
- `crates/context-graph-mejepa-corpus/src/generate.rs`
  - Library official-patch loader uses timed subprocess capture.
- `crates/context-graph-mejepa-corpus/src/source_patch_support.rs`
  - Timeout kill escalation suppresses kill command noise.

## Verification

Automated:

- `cargo test -p context-graph-mejepa-corpus` passed.
- `cargo clippy -p context-graph-mejepa-corpus --no-deps --lib --bins --tests -- -D warnings` passed.
- `cargo build --release -p context-graph-mejepa-corpus` passed.
- `git diff --check` passed.

Manual FSV source of truth:

- Corpus: `/mnt/d/contextgraph/corpus/codex-hardening-fsv-current-20260512T025100Z`
- Trigger: `target/release/mejepa-corpus generate` for real
  `sympy__sympy-20590`, category `known_good`, oracle worker `1`.
- Source of truth:
  - `index.json`
  - `mutations/sympy__sympy-20590/known_good.patch`
  - RocksDB `oracle-store` column families
  - `verify-evidence.json`
- Observed state:
  - `index.complete=true`, `entries=1`
  - patch count `1`
  - patch SHA `2726d808ea27b330fcec46cf49e60de768796feb42a36d62b0f164444c785876`
  - RocksDB mutation/verdict/provenance counts `1/1/1`
  - fresh oracle verdict matched persisted hash
    `sha256:3b32e0acbaff83a8f49d8303622407db29ee9d11eab45f66d0c595e73c539516`
  - `verify-evidence.json` `passes=true`, `failures=[]`

Boundary checks:

- Output under `/tmp`: rejected before output creation.
- `--oracle-workers 0`: rejected before output creation.
- `--max-tasks 0`: rejected before output creation.
- Stale production resume:
  `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511`
  rejected before Docker because patch files diverged from index:
  `index_count=1500`, `patch_file_count=1800`.

## External Research Used

- Microsoft WSL filesystem guidance:
  https://learn.microsoft.com/en-us/windows/wsl/filesystems
- Docker Desktop WSL best practices:
  https://docs.docker.com/desktop/features/wsl/best-practices/
- Rust `std::process` docs on pipe deadlocks:
  https://doc.rust-lang.org/stable/std/process/index.html
- Reth atomic-write sequence:
  https://reth.rs/docs/reth_fs_util/fn.atomic_write_file.html

## Current State

There are no matching `mejepa-corpus generate`, SWE-bench harness, or stale
run-id Docker containers left running. The old production partial corpus is now
honestly blocked from resume until its stale patch-file state is corrected or a
fresh output directory is used.
