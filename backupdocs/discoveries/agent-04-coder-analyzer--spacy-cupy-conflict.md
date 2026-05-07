---
namespace: discoveries
agent_id: agent-04-coder-analyzer
timestamp: 2026-04-17T16:25:00Z
status: active
severity: medium
tags: spacy, cupy, environment, workaround
---

# spacy import broken by cupy-cuda12x 14.0.1 AttributeError

## What Happened
In this environment, `import spacy` fails because spacy's `compat.py` does `import cupy` inside a `try/except ImportError` block — but the installed `cupy-cuda12x==14.0.1` raises `AttributeError: 'LoadedDL' object has no attribute 'found_via'` during init, which is NOT an ImportError and therefore propagates past spacy's guard.

Trace:
```
cupy/_core/_cub_reduction.pyx
  → cupy/_environment.py:198 _get_nvcc_path
    → cupy/_environment.py:165 _get_cuda_path
      → AttributeError: 'LoadedDL' object has no attribute 'found_via'
```

## Fix (in analyzer script)
Before `import spacy`, install a `sys.meta_path` finder that raises `ImportError` for any `cupy*` module. Spacy's try/except then handles it cleanly and falls back to CPU-only operation. The analyzer is CPU-bound anyway (daemon does the GPU work).

```python
class _BlockCupy:
    def find_spec(self, name, path=None, target=None):
        if name == "cupy" or name.startswith("cupy."):
            raise ImportError("cupy disabled for CPU-only spacy")
        return None
sys.meta_path.insert(0, _BlockCupy())
import spacy
```

## Why It Matters
Without this workaround, spacy fails to import and the analyzer either crashes or falls back to punctuation-splitting (degrading sentence_shape / subordination analysis). Better: block cupy cleanly, keep spacy fully functional.

## Library installs needed
- `pip install pronouncing` (CMU dict)
- `pip install https://github.com/explosion/spacy-models/releases/download/en_core_web_sm-3.8.0/en_core_web_sm-3.8.0-py3-none-any.whl`
- NOT needed: `python -m spacy download` (blocked by same cupy issue)

## Related
- Tags: spacy, cupy, environment
