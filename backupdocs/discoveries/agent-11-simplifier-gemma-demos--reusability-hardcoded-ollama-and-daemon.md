---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:18:00Z
status: active
severity: low
tags: reusability, configuration, env-vars, hardcoded, portability
---

# Hardcoded host/port/model defaults harm reusability on a different daemon

## What Happened

Reviewer's prompt asked explicitly: "will these work tomorrow on a different
daemon configuration / a different LLM in Ollama?"

Answer: only if the user edits source or passes every flag every invocation.

### Hardcoded defaults across the 7 files

| Default                  | Location                         | Should be overridable via env? |
|--------------------------|----------------------------------|--------------------------------|
| `127.0.0.1:3100`         | `Rpc.__init__` default args      | YES (CG_DAEMON_HOST, _PORT)    |
| `http://127.0.0.1:11434` | `GemmaClient.__init__`           | YES (OLLAMA_HOST)              |
| `gemma4:e4b`              | `GemmaClient.__init__`           | YES (DEMO_MODEL)               |
| `keep_alive="5m"`        | `GemmaClient.__init__`           | YES                            |
| `"Shakespeare"` prefix   | `SHAKESPEARE_LABEL_PREFIX`        | YES (DEMO_CONSTELLATION_PREFIX)|

Demos 1/3/4/5 construct `GemmaClient()` with no args and never expose any
way to change the model. If the user runs a different Ollama model
(e.g. `llama3.1:8b`) or a different daemon (e.g. port 4100 on a test
environment), they must open the source.

Demo 6 (rlhf) already parameterizes `--out`, `--max-pairs`, `--page-size`.
The other 5 could do the same with ~5 lines each.

## Recommendation

Add env var fallbacks in `_gemma_shakespeare_lib.py`:

```python
import os
DEFAULT_DAEMON_HOST = os.environ.get("CG_DAEMON_HOST", "127.0.0.1")
DEFAULT_DAEMON_PORT = int(os.environ.get("CG_DAEMON_PORT", "3100"))
DEFAULT_OLLAMA_HOST = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434")
DEFAULT_MODEL = os.environ.get("DEMO_MODEL", "gemma4:e4b")
```

Then `Rpc.__init__` and `GemmaClient.__init__` use these as defaults. No
caller changes. Portability unlocked. This is a 5-line change.

## Why It Matters

Future coordinator could spin up a second daemon on port 4100 for a
dev-vs-prod A/B test. Today that requires monkey-patching the scripts. Tom's
script hardening pass could just set `CG_DAEMON_PORT=4100` and move on.

## Related

- Tags: reusability, env-config, portability, ollama, daemon
