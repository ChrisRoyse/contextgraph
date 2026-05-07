---
namespace: patterns
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:35:00Z
status: active
tags: exp2, fail-loud, pattern, exp2error
---

# Pattern: every EXP-2 module raises Exp2Error on any precondition violation

## Observation
Every script in `scripts/exp2/*.py` uses the same fail-loud structure:
```python
def main():
    ...

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        raise SystemExit(f"EXP-2 <stage> failed: {exc}") from exc
```

And inside `main()`:
- Inputs validated up front; raise `Exp2Error` with rich detail on miss.
- Numpy arrays checked for finiteness, correct shape, non-zero norm.
- Subprocess calls (cargo, nvidia-smi, git) raise on non-zero return.
- Optional dependencies (datasets, npeet) raise `Exp2Error` if missing.

## Why this matters
Compliant with Constitution C2 (fail loud, no silent errors).

## Single exception
`McpClient.stop()` (mcp_client.py:84) silently swallows exception (`except Exception: self.proc.kill()`). See `agent-01-static-audit--failure-mcp-client-stop-swallows-errors.md`.

## Pattern recommendation for agent-04 simplifier
The fail-loud-via-Exp2Error pattern is already consistent. Don't refactor it. Don't add abstraction layers. The minor lapses (mcp_client.stop, mcp_client._read_stdout) are localized; fix them in place.
