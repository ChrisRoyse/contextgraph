---
namespace: decisions
created: 2026-05-11T10:19:34Z
updated: 2026-05-11T10:19:34Z
status: active
tags: daemon, shutdown, tokio, pid-lock, source-of-truth
---

# Headless Daemon Explicit Exit After Durable Shutdown

## What Happened
The headless MCP daemon process must not return through ordinary Tokio runtime teardown after it has written `stopped` and removed its daemon PID lock. After `server.shutdown()` and flywheel shutdown complete, the daemon now computes an exit code, explicitly drops its locks, and calls `std::process::exit(exit_code)`.

`main()` also calls `runtime.shutdown_timeout(Duration::from_secs(5))` after `async_main()` for non-headless paths so runtime drop cannot wait indefinitely on blocking tasks.

## Why It Matters
The daemon PID file is a source of truth. If the PID file is absent while the process is still alive, launchers and operators can start duplicate daemons or misdiagnose the state. The accepted invariant is:

- While the daemon process is alive, the daemon PID file exists and is locked.
- After graceful shutdown completes, the daemon process is gone, the PID file is absent, the transport ports have no listeners, and `flywheel_supervisor_status.json` reads `status: "stopped"`.

## Related
- See also: ./memory/discoveries/2026-05-11--flywheel-daemon-hardening-review-root-causes.md
- Tags: daemon, shutdown, source-of-truth
