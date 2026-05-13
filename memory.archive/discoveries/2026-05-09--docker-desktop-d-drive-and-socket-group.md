---
namespace: discoveries
created: 2026-05-09T22:42:48Z
updated: 2026-05-09T22:42:48Z
status: active
severity: high
tags: phase0, docker, wsl2, d-drive, swebench, source-of-truth
---

# Docker Desktop D Drive Storage and Python SDK Socket Group

## What Happened

Docker Desktop for Windows was moved off `C:` for Phase 0 SWE-bench image
storage. The active Docker Desktop settings source of truth is:

```text
/mnt/c/Users/hotra/AppData/Roaming/Docker/settings-store.json
```

It now contains:

```json
{
  "CustomWslDistroDir": "D:\\contextgraph\\docker-desktop-data",
  "DiskSizeMiB": 2097152
}
```

The active Docker VHDX source of truth is:

```text
/mnt/d/contextgraph/docker-desktop-data/disk/docker_data.vhdx
```

The stale copied `C:` VHDX at
`/mnt/c/Users/hotra/AppData/Local/Docker/wsl/disk/docker_data.vhdx` was deleted
only after Docker Desktop restarted, `docker images` still read back all 300
SWE-bench evaluator images, and the D-side VHDX was observed growing while the
C-side copy stayed unchanged.

## Python Docker SDK Root Cause

The Docker CLI in this WSL session works through the Windows Docker Desktop CLI
context:

```text
/home/cabdru/.local/bin/docker -> /mnt/c/Program Files/Docker/Docker/resources/bin/docker.exe
context desktop-linux -> npipe:////./pipe/dockerDesktopLinuxEngine
```

The official SWE-bench harness does not use that path. It imports Python's
Docker SDK and connects to `/var/run/docker.sock`. After Docker Desktop restart,
the user account belonged to the `docker` group in `/etc/group`, but this
long-lived shell process did not have that supplementary group in its live
process credentials. Result:

```text
curl --unix-socket /var/run/docker.sock http://localhost/_ping -> Permission denied
python -c 'import docker; docker.from_env().version()' -> PermissionError(13)
sg docker -c 'python -c ... docker.from_env().version()' -> success
```

## Why It Matters

For Phase 0 production, `docker version` is not enough evidence. It can pass
through `docker.exe` while the actual SWE-bench evaluator source of truth,
Python Docker SDK access to `/var/run/docker.sock`, fails.

Any corpus generation command in this current shell should run under:

```text
sg docker -c 'target/release/mejepa-corpus generate ...'
```

New shells should inherit the group normally, but future automation should
still preflight Python Docker SDK access before launching the expensive harness.

## Evidence

- `/mnt/d/contextgraph/logs/docker-desktop-d-migration-and-prewarm-evidence-20260509.json`
- `/mnt/d/contextgraph/logs/mejepa-prewarm-all300-20260509.json`
- Docker readback after prewarm: 300 `swebench/sweb.eval.*` images.
- D free space after all image prewarm: about 4.0 TB free.
- C free space after stale VHDX deletion: about 424 GB free.

## Best-Practice Basis

Docker documents the WSL2 engine default storage path under
`C:\Users\[USERNAME]\AppData\Local\Docker\wsl` and the supported Settings >
Resources > Advanced disk image location. Docker also recommends keeping
bind-mounted source in the Linux filesystem for performance and using explicit
prune commands for unused Docker objects.
