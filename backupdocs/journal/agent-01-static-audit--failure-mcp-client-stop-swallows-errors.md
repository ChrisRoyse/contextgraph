---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:13:00Z
status: active
severity: low
tags: exp2, mcp-client, fail-loud, error-swallow
---

# Issue: McpClient.stop() swallows all exceptions

## Where
`scripts/exp2/mcp_client.py:76-87`

```python
def stop(self) -> None:
    if self.proc is None:
        return
    try:
        if self.proc.stdin:
            self.proc.stdin.close()
        self.proc.terminate()
        self.proc.wait(timeout=10)
    except Exception:
        self.proc.kill()
    finally:
        self.proc = None
```

## Constraint
EXP-2 constraints C2 (fail loud): "Any silent error, any `except: pass`... is a finding."

## Observation
The `except Exception` clause swallows everything (including the case where `self.proc` is reaped during `.wait()` and the kill() itself fails). After the kill, `self.proc = None` masks the failure. A subsequent run sees a stale RocksDB lock if the process didn't actually die.

This is a teardown path; full fail-loud is awkward (terminate() failures don't usually have remediation). But silently kill() and continue is not great either.

## Recommended fix
Log the exception via the audit chain or stderr buffer:
```python
except Exception as exc:
    self.stderr_lines.append(f"stop() exception: {exc!r}")
    self.proc.kill()
```
At minimum the stderr_lines are accessible to the next failure path.

## Severity
LOW. The exception is rarely consequential (process exited, just need to release file descriptors). But it's an `except Exception:` against the C2 spirit.

## Cross-check
None. This is a code-review observation.
