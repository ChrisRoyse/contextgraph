---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:18:00Z
status: active
severity: medium
tags: phase-b, file-descriptor, error-log, daemon-socket-lifecycle
---

# Phase B analyzer: error_log fd only in one try/finally; daemon socket never closed on success

## Evidence

### FD leak on error in resume path
`scripts/shakespeare_stylistic_analyzer.py:1246-1319`
```python
    error_log = ERROR_LOG_PATH.open("a", encoding="utf-8")
    error_log.write(f"\n=== analyzer run @ {datetime.now(timezone.utc).isoformat()} ===\n")

    try:
        with input_path.open("r", encoding="utf-8") as fin, \
                partial_path.open(write_mode) as fout:
            for idx, line in enumerate(fin):
                ...
    finally:
        error_log.close()
```

If `_maybe_resume(input_path, output_path)` or `daemon.health_check()`
(lines 1193, 1201) raises AFTER the `error_log.open(...)` on line 1246,
it DOESN'T because those calls are BEFORE line 1246 — OK.

BUT: `partial_path.open(write_mode)` itself might raise (permission,
disk full). In that case, the `try:` body raises before the inner `with`
succeeds, so inner `with` context manager doesn't clean up (correct —
it hasn't acquired the resource). `error_log` DOES close via the outer
finally. OK here.

### Daemon socket (self._sock in DaemonClient) never explicitly closed
`scripts/shakespeare_stylistic_analyzer.py:283-294`
```python
def __init__(self, host, port, constellation_id):
    ...
    self._sock: socket.socket | None = None
    ...

def _reset(self) -> None:
    if self._sock is not None:
        try:
            self._sock.close()
        except Exception:
            pass
    self._sock = None
```

`_reset()` is called on every error path. But on the HAPPY path at end of
`main()`, the daemon socket stays open until GC or process exit. No
explicit `daemon.close()` / `daemon._reset()` is called after the
`for idx, line in enumerate(fin):` loop. Process exit reclaims the FD
but the daemon logs a "client closed abruptly" — harmless but noisy.

Also: if Python exits abnormally (OOM, SIGKILL), the Linux kernel
sends RST on the open TCP connection — daemon sees "client closed
socket mid-" error at the recv boundary. The daemon then (correctly)
drops the client session but logs an ERROR-level message. Over 7,925
records with restarts, daemon logs fill up.

### Error log grows unboundedly across runs
`ERROR_LOG_PATH = Path("/tmp/shake_analyzer_errors.log")` opened with
`"a"` (append). Every run appends. The file never rotates. Over many
resume cycles, /tmp fills up. /tmp on WSL2 is usually tmpfs which
counts against RAM.

## Fix Recommendation

1. **Add explicit daemon.close() at end of analyzer main()**:
```python
try:
    ...main loop...
finally:
    error_log.close()
    try:
        daemon._reset()   # close socket gracefully
    except Exception:
        pass
```

2. **Rotate error log on start**:
```python
if ERROR_LOG_PATH.exists() and ERROR_LOG_PATH.stat().st_size > 10_000_000:  # 10MB
    ERROR_LOG_PATH.rename(ERROR_LOG_PATH.with_suffix(".log.old"))
```

3. **Or use logging.handlers.RotatingFileHandler** for bounded error log.

## Related
- Tags: fd-leak, error-log, phase-b
