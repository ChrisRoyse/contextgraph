---
namespace: journal
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T00:00:00Z
status: active
tags: phase0, corpus, swebench, docker, failure, root-cause
---

# Failure: SWE-bench Parallel Docker Reporting Race

## What Happened
While running four shard generators concurrently, shard session `10174` failed in the SWE-bench harness after 32 persisted rows. The harness completed the instance run, then failed during reporting/cleanup:

```text
docker.errors.NotFound: 404 Client Error ... inspect_container ... No such container
```

The stack was inside `swebench/harness/reporting.py` at `client.containers.list(all=True)`, which internally lists containers and then inspects each container. A concurrently cleaned-up container disappeared between list and inspect.

Live shard state at failure:
- out-00: 37 mutation/verdict rows
- out-01: 32 mutation/verdict rows, failed
- out-02: 40 mutation/verdict rows
- out-03: 34 mutation/verdict rows

I stopped the remaining shard generators to avoid compounding the shared Docker race.

## Why It Matters
The mutation and RocksDB paths were not the root cause. The failure came from running multiple independent SWE-bench harness subprocesses against one shared Docker daemon. Docker SDK documentation exposes `ignore_removed` on `containers.list` for this exact race shape, but we are not patching upstream SWE-bench. The local robust fix is an interprocess file lock around the SWE-bench harness subprocess in `run_swebench_lite_oracle`, so corpus jobs cannot make the harness list/inspect/cleanup containers concurrently.

## Related
- Docker SDK docs: `containers.list(..., ignore_removed=True)` exists for removed-container races.
- SWE-bench harness docs: evaluation uses Docker containers.
- Code fix: `crates/context-graph-mejepa-corpus/src/swebench.rs` now acquires `/tmp/contextgraph-mejepa-corpus-swebench-harness.lock` while the harness subprocess runs.
