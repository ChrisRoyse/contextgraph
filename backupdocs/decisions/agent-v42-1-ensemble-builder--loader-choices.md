---
namespace: decisions
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T05:47:54Z
status: active
tags: v4.2, ensemble, loader, bge-m3, jina-v3, qwen3, reranker, flagembedding
---

# Loader Choices for the Extended 4-Embedder + 1-Reranker Ensemble

## Context
Coordinator mission: wrap 4 new embedders (BGE-M3, Jina-v3, Qwen3-Embedding-0.6B,
plus a reranker BGE-reranker-v2-m3) for the v4.2 CAMD-DPO stack. Hard rule: NO
fallbacks, ERROR-OUT on failure, real data in tests, file size < 500 lines.

Environment verified:
- Python 3.13, torch 2.11+cu130 on RTX 5090 (32 GB, 14.6 GB free)
- transformers 5.5.4, sentence-transformers 5.2.0, FlagEmbedding installed
- HF_TOKEN (37 chars) set, BGE-M3 already cached locally (4.3 GB)
- All 4 models exist on HF Hub (verified via HfApi)

## Decisions

### D1: BGE-M3 via `FlagEmbedding.BGEM3FlagModel` (NOT sentence-transformers)
Rationale: sentence-transformers `encode` does not expose `return_dense`,
`return_sparse`, `return_colbert_vecs` as kwargs. The unified multi-vector API
is ONLY available through `FlagEmbedding.BGEM3FlagModel.encode`. Since we need
all three views (dense + sparse + ColBERT) for style discrimination, the
FlagEmbedding path is mandatory.

### D2: Shim the missing `transformers.utils.import_utils.is_torch_fx_available`
FlagEmbedding 1.3 was pinned to transformers 4.x. transformers 5.x removed
`is_torch_fx_available` from the public import path. FlagEmbedding's
`reranker/decoder_only/models/modeling_minicpm_reranker.py:53` hard-imports it,
and the package top-level `__init__.py` unconditionally pulls that chain in.

Minimal shim at module load:
```python
import transformers.utils.import_utils as _iu
if not hasattr(_iu, "is_torch_fx_available"):
    _iu.is_torch_fx_available = lambda: False
```

Verified: with this shim, `from FlagEmbedding import BGEM3FlagModel` succeeds.
This is NOT a fallback — it's a compatibility shim for a pinned dependency. A
failed M3 load still raises RuntimeError with diagnostics. No silent
degradation.

### D3: BGE-reranker-v2-m3 via `transformers.AutoModelForSequenceClassification`
FlagEmbedding's `FlagReranker` class is in the same broken import chain and
relies on additional reranker utilities. Since v2-m3 is a plain cross-encoder
(XLM-RoBERTa sequence classifier outputting a single relevance logit), we can
load it directly with AutoTokenizer + AutoModelForSequenceClassification. The
forward returns one logit per (query, passage) pair.

### D4: Jina-v3 via `sentence_transformers.SentenceTransformer` with `task` kwarg
Jina-v3 publishes task LoRAs through the sentence-transformers `prompt_name`/
`task` kwargs: `classification`, `text-matching`, `retrieval.query`,
`retrieval.passage`. For style discrimination we use `classification` (the
task designed to separate texts by categorical feature).

### D5: Qwen3-Embedding via `sentence_transformers.SentenceTransformer` + instruction prompt
Qwen3-Embedding-0.6B is instruction-tuned; official guidance wraps input in
`"Instruct: {task}\nQuery: {text}"`. The instruction steers the embedding
toward a user-specified aspect. For our use, `task_prompt` is supplied by
caller and MUST be non-empty (empty → RuntimeError, per D1 of hard rules).

### D6: Dims asserted at load
A hard `EXPECTED_DIMS` dict is checked against each model's output on
initialization. Mismatch → `RuntimeError(model, expected, got)`. This guards
against HF silently shipping a re-trained variant or a quantized mismatch.

### D7: bfloat16 preferred for transformer backbones, fp16 for reranker
RTX 5090 (Blackwell) has native bf16 tensor cores; bf16 is numerically safer
than fp16 for long-context attention. The reranker uses fp16 to match
reference eval setup (BGE team publishes fp16 numbers). Explicit
`torch_dtype=...` passed on load.

## Open non-issues
- `cobert_dim` default of -1 returns the model's full hidden size (1024 for
  BGE-M3). No override needed.
- Reranker outputs raw logit; we expose it as-is (callers may sigmoid if they
  want [0,1]).

## Consequences
- Module imports two heavy packages (FlagEmbedding + sentence-transformers) but
  only one path per model.
- Smoke test asserts discrimination gap ≥ 0.05 in normalized cosine per
  embedder against real Sonnet 18 vs modern text. Any embedder missing the
  gap → sys.exit(1).
