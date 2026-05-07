---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:14:00Z
status: active
severity: medium
tags: exp2, mcp-client, fail-loud, json-decode, silent-skip
---

# Defect: McpClient._read_stdout silently skips lines that don't parse as JSON

## Where
`scripts/exp2/mcp_client.py:146-155`

```python
def _read_stdout(self) -> None:
    assert self.proc is not None and self.proc.stdout is not None
    for raw in self.proc.stdout:
        line = raw.decode("utf-8", errors="replace").strip()
        if not line:
            continue
        try:
            self.responses.put(json.loads(line))
        except json.JSONDecodeError:
            continue   # silently dropped
```

## Constraint
EXP-2 C2 (fail loud): "Any silent error... that hides a fault is a finding."

## Observation
The MCP server *should* only emit JSON-RPC lines on stdout (per JSON-RPC spec, every line is a complete JSON value). If a line isn't JSON-decodable it's either:
1. Daemon emitted a debug print (RUST_LOG > error somehow leaked).
2. Protocol corruption (truncated read, encoding mismatch).
3. Mixed binary content (shouldn't happen with this server).

Silently dropping it loses the diagnostic. The `responses` queue gets the next valid JSON line; the call_method timeout check at line 122-138 will eventually catch a stuck call. But by then the actual line content is lost.

## Why it matters
- Diagnostic loss: the `stderr_tail` is preserved, but stdout corruption is invisible.
- Some MCP servers print warnings to stdout when shutting down (e.g., daemon crash banner). Those would be lost.

## Recommended fix
Push malformed lines into stderr_lines (or a separate `stdout_garbage` list) so they surface in timeout error messages:
```python
except json.JSONDecodeError:
    self.stderr_lines.append(f"non-json stdout: {line[:500]}")
    continue
```

## Cross-check
Agent-03 (sherlock, with running daemon) can stress this by passing a malformed JSON via stdin and confirming behavior.

## Severity
MEDIUM. C2 says fail-loud; here we silently continue. Not a runtime bug because no MCP code path produces non-JSON stdout, but the safety net is missing.
