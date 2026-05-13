---
namespace: discoveries
agent_id: codex
created: 2026-05-12T02:43:34Z
updated: 2026-05-12T02:43:34Z
status: active
tags: discovery, mejepa, corpus, swebench, wsl, rocksdb, fsv, hardening
---

# ME-JEPA corpus ENOENT root cause was concurrent durable patch writes

## What

The latest corpus failures were not fixed by serially pre-creating task
directories. The failing path was `write_text_checked` during durable patch-file
materialization under `/mnt/d/contextgraph/corpus`.

The root cause was the generator doing high-concurrency rayon patch generation
and durable patch-file writes in the same worker closure. On WSL2, `/mnt/d` is
DrvFs/9P-backed and is much slower and less reliable for concurrent Linux file
creation than the ext4 filesystem. Pre-creating directories only removed one
possible ENOENT source; it did not remove the actual concurrent write pressure.

## Fix Pattern

- Keep rayon workers on CPU/source-prep work only, using ext4 ephemeral roots
  under `/home/cabdru/.cache/mejepa-corpus`.
- Hold generated patch text in memory for the oracle.
- Run the real SWE-bench oracle.
- Only after oracle verdicts exist, serialize durable writes to `/mnt/d`:
  patch file, RocksDB row, readback, index update.
- Resume now fails closed if mutation patch files or RocksDB rows diverge from
  `index.json`. This caught the old partial corpus: 1,500 indexed/store rows but
  1,800 patch files.

## Evidence

Final current-binary FSV corpus:

`/mnt/d/contextgraph/corpus/codex-hardening-fsv-current-20260512T025100Z`

Source-of-truth readbacks:

- `index.json`: `complete=true`, `entries=1`, task `sympy__sympy-20590`,
  category `known_good`.
- Patch tree: exactly `1` `*.patch` file.
- RocksDB `oracle-store`: mutation/verdict/provenance counts `1/1/1`.
- `verify-evidence.json`: `passes=true`, `oracle_sample_count=1`,
  fresh oracle verdict hash matched the persisted verdict hash.

Stale production guard:

`/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511`

Current resume rejects it before Docker execution:

`cannot resume because mutation patch files diverge from index: index_count=1500 patch_file_count=1800`.

## Research Notes

- Microsoft WSL docs: Linux command-line work is fastest in the WSL filesystem;
  `/mnt/<drive>` paths are mounted Windows drives.
- Docker Desktop WSL best practices: Linux container source and bind-mounted
  data should live in the Linux filesystem for performance.
- Rust `std::process` docs warn that waiting while child stdout/stderr pipes are
  not drained can deadlock.
- Atomic file-write best practice is temp file in same directory, fsync file,
  rename, then fsync parent directory.
