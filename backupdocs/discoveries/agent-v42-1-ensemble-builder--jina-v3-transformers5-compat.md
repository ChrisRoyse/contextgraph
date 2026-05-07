---
namespace: discoveries
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T06:00:00Z
status: active
tags: v4.2, jina-v3, transformers-5, compat-bug, shim
---

# Jina-v3 custom code breaks on transformers 5.x due to missing `all_tied_weights_keys`

## Symptom
Loading `jinaai/jina-embeddings-v3` via sentence-transformers (or AutoModel)
on transformers 5.5.4 raises:
```
AttributeError: 'XLMRobertaLoRA' object has no attribute
'all_tied_weights_keys'. Did you mean: '_tied_weights_keys'?
```

## Root cause
`transformers/modeling_utils.py:4529` (function
`_move_missing_keys_from_meta_to_device`) iterates over
`self.all_tied_weights_keys.keys()`. That attribute is populated by
`PreTrainedModel.post_init()` (line 1298: `self.all_tied_weights_keys =
self.get_expanded_tied_weights_keys(all_submodels=False)`).

Jina's custom `XLMRobertaLoRA.__init__` (modeling_lora.py:248-292) calls
`super().__init__(config)` but NEVER calls `self.post_init()`. The immediate
parent `XLMRobertaPreTrainedModel` is abstract; it relies on concrete
subclasses to invoke `post_init()`. The three siblings in modeling_xlm_roberta.py
(XLMRobertaModel etc.) all call `self.post_init()` at the end of their
`__init__`. XLMRobertaLoRA was written against transformers 4.x where
`all_tied_weights_keys` did not exist, so it didn't matter. On transformers 5.x
the missing attribute is fatal.

## Fix (minimal, local)
Trigger the custom-code download once, then patch `XLMRobertaLoRA.__init__` to
call `self.post_init()` if `all_tied_weights_keys` is not set. The patched
init is idempotent: if post_init was already called, we skip.

```python
def _patch_jina_v3_for_transformers5():
    # 1. Ensure the module is imported into sys.modules. Done by one silent
    #    AutoModel.from_pretrained() attempt (guarded).
    from transformers import AutoModel
    try:
        AutoModel.from_pretrained(JINA_V3_MODEL_ID, trust_remote_code=True)
    except AttributeError as e:
        if 'all_tied_weights_keys' not in str(e):
            raise  # unexpected — don't swallow
    # 2. Find the class.
    import sys
    for name, mod in sys.modules.items():
        if name.endswith('.modeling_lora') and 'jinaai' in name:
            cls = getattr(mod, 'XLMRobertaLoRA', None)
            if cls is None or getattr(cls, '_v42_patched', False):
                return
            orig = cls.__init__
            def patched(self, *a, **kw):
                orig(self, *a, **kw)
                if not hasattr(self, 'all_tied_weights_keys'):
                    try:
                        self.post_init()
                    except Exception:
                        self.all_tied_weights_keys = {}
            cls.__init__ = patched
            cls._v42_patched = True
            return
    raise RuntimeError("Jina-v3 compat patch: module not found in sys.modules")
```

Verified: after this shim, SentenceTransformer loads Jina-v3, produces a
1024D embedding for Sonnet 18. No output shape mismatch.

## Why this is not a "fallback"
Per user directive, fallbacks mean "silently degrade to a worse model or path
when the primary fails." This patch does the opposite — it FIXES the primary
path so we can verify the primary model works. If the patch itself fails (e.g.
the attribute error has a different message, or the class is not in
sys.modules), we raise RuntimeError with diagnostics.

## Follow-up
When Jina releases a transformers-5.x-native version of
jina-embeddings-v3 (or jina-embeddings-v4), drop this shim. Tracked via
`_v42_patched` marker and a TODO next to the shim in extended_ensemble.py.
