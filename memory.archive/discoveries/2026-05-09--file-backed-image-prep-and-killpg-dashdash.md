---
namespace: contextgraph/mejepa-corpus
agent_id: codex
timestamp: 2026-05-09T20:48:23Z
status: active
tags:
  - mejepa
  - phase0-corpus
  - process-management
  - fsv
---

# File-backed image-prep logs and process-group termination

Root cause found during hardening:

- `prepare-images` spawned `docker pull --quiet` with piped stdout/stderr and only drained those pipes after process exit. `--quiet` reduced output volume but did not remove the structural pipe-buffer deadlock class.
- Timeout handling sent process-group signals as `kill -TERM -<pgid>` / `kill -KILL -<pgid>`. On this host that could return success without terminating the process group promptly. Passing `--` before the negative PGID (`kill -TERM -- -<pgid>`) is required so the negative argument is interpreted as a process group operand, not an option-like token.

Fix shipped:

- `prepare-images` now redirects each pull to content-addressed physical log files under `/tmp/contextgraph-mejepa-image-prep-logs/` and waits on status only. Evidence JSON includes `stdout_log`, `stderr_log`, and bounded tails.
- Shared oracle process-group termination now uses `kill -TERM -- -<pgid>` and `kill -KILL -- -<pgid>`.
- Regression tests print FSV before/after state for large file-backed output, timeout reaping through `/proc/<pid>`, and deterministic pull log paths.

FSV evidence:

- `/tmp/contextgraph-mejepa-hardening-fsv/subprocess-regression-tests.log`
- `/tmp/contextgraph-mejepa-hardening-fsv/prepare-images-happy.json`
- `/tmp/contextgraph-mejepa-hardening-fsv/prepare-images-edge-cases.log`

