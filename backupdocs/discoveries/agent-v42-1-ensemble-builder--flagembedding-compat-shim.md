---
namespace: discoveries
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T05:48:30Z
status: active
tags: v4.2, flagembedding, transformers-5, compat-bug, shim
---

# FlagEmbedding top-level import is broken on transformers 5.x

## Symptom
`from FlagEmbedding import BGEM3FlagModel` raises:
```
ImportError: cannot import name 'is_torch_fx_available' from
'transformers.utils.import_utils'
```

## Root cause
- FlagEmbedding's `__init__.py` executes `from .inference import *`.
- `inference/__init__.py` imports `FlagAutoReranker`.
- `FlagAutoReranker` → reranker/decoder_only → MiniCPMForCausalLM backend.
- `modeling_minicpm_reranker.py` line 53: `from transformers.utils.import_utils
  import is_torch_fx_available`.
- transformers 5.x removed that import path. FlagEmbedding 1.3.x was pinned to
  transformers 4.x.

## Fix (explicit shim in our module)
Two lines before any FlagEmbedding import:
```python
import transformers.utils.import_utils as _iu
if not hasattr(_iu, "is_torch_fx_available"):
    _iu.is_torch_fx_available = lambda: False
```
Verified working — `BGEM3FlagModel` imports and encodes successfully.

## Why this is not a "fallback"
The hard-rule "no fallbacks" forbids silent degradation on failed model loads.
This shim is different: it's a compat layer for a transient API removal in
transformers 5.x that was out of FlagEmbedding maintainer control at the time
this codebase was set up. If BGE-M3 itself fails to download or load, we still
raise RuntimeError with full diagnostics (model id, path, CUDA mem, chained
exception). The shim only restores one missing function symbol.

## Follow-up
When FlagEmbedding releases a transformers-5.x-compatible version, remove the
shim. Tracked via a TODO comment in `scripts/v4_2/extended_ensemble.py` near
the shim.
