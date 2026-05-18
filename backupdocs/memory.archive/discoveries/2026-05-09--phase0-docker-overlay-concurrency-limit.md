---
namespace: discoveries
created: 2026-05-09T23:12:00Z
updated: 2026-05-09T23:12:00Z
status: active
severity: high
tags: phase0, docker, swebench, performance, overlayfs, concurrency
---

# Phase 0 Docker Overlay Concurrency Limit

## What Happened

Shard 0 `wrong_file` resumed at `--oracle-workers 12` after all 300 images were
prewarmed. The batch progressed to 54 reports, then repeated waves of 12 Django
containers stalled in:

```text
git -c core.fileMode=false diff
State: D (disk sleep)
wchan: folio_wait_bit_common
```

Docker stats showed almost no CPU usage inside the containers, while Docker
Desktop reported `overlayfs` over its WSL2 VHDX:

```text
Driver: overlayfs
NCPU: 30
MemTotal: 118156230656
DockerRootDir: /var/lib/docker
OperatingSystem: Docker Desktop
```

The harness had already proven it can skip completed reports:

```text
<n> instances already run, skipping...
```

So the run was interrupted after verifying 54 completed `report.json` files,
stale containers were removed, and the same shard/category was resumed with
`--oracle-workers 6`.

## Result

With 6 workers, the harness resumed at the first missing IDs instead of
re-running the 54 completed reports, and quickly advanced beyond the stalled
point. The first post-resume checkpoint jumped from 54 to 65 reports in about
30 seconds.

## Why It Matters

For this Docker Desktop + WSL2 + D-drive VHDX setup, the limiting resource is
not CPU threads or RAM. It is Docker overlay filesystem/page-cache pressure from
parallel SWE-bench repo operations (`git diff`, checkout, cleanup). Increasing
workers above 6 can reduce throughput by forcing many containers into kernel
I/O wait.

## Operational Rule

Use 6 oracle workers for Phase 0 production on this host unless a fresh timing
run proves a higher value is faster:

```text
--oracle-workers 6
```

Keep image prewarm at parallelism 6 as well. This preserves real Docker
execution while avoiding the observed overlayfs contention.
