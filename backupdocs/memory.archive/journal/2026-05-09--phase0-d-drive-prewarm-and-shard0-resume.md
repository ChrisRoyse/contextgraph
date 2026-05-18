---
namespace: journal
created: 2026-05-09T22:42:48Z
updated: 2026-05-09T22:42:48Z
status: active
tags: phase0, corpus, docker, d-drive, prewarm, production
---

# Phase 0 D Drive Prewarm and Shard 0 Resume

## What Happened

Docker Desktop was updated/restarted by the operator and verified from WSL:

```text
Docker client/server 29.4.2
context desktop-linux
```

All 300 SWE-bench Lite evaluator images were prewarmed at parallelism 6 after
moving Docker Desktop storage to D:

```text
/mnt/d/contextgraph/logs/mejepa-prewarm-all300-20260509.json
passes=true
target_image_count=300
missing_after_count=0
duration_secs=3238.990173074
```

Shard 0 production resumed under:

```text
/tmp/contextgraph-mejepa-corpus-prod-shard0-20260509
```

The resumed command must run under `sg docker -c` in this current shell because
Python Docker SDK access to `/var/run/docker.sock` depends on live process group
membership, not just `/etc/group`.

## Current Production State at This Checkpoint

Before resuming, shard 0 had 750 committed entries:

```text
known_good=150
subtle_flip=150
off_by_one=150
swap_variable=150
delete_test_call=150
```

The resumed run is processing `wrong_file`. At checkpoint, the first worker
containers were active and per-instance reports had started appearing.

## Code Hardening Added

`crates/context-graph-mejepa-corpus/src/swebench.rs` now preflights Python
Docker SDK access before launching SWE-bench. This catches the real harness
failure mode where `docker version` succeeds through Docker Desktop's Windows
CLI path but `docker.from_env().ping()` cannot access `/var/run/docker.sock`.

## Resume Here

Watch active exec session `89790`. After `wrong_file` completes, read
`index.json` and the RocksDB column families directly. Expected shard 0 final
state is 1200 entries: 150 rows for each of the 8 Phase 0 mutation categories.
