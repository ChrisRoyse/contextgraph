---
namespace: handoff
agent_id: agent-v42-1-ensemble-builder
timestamp: 2026-04-22T06:12:00Z
status: active
tags: v4.2, ensemble, handoff, agent-2, reference-builder
---

# Handoff: Extended Ensemble class is ready for Agent 2

## Where is the module
Absolute path:
    /home/cabdru/contextgraph/scripts/v4_2/extended_ensemble.py
Line count: 499 (under the 500 rule)
Smoke test: PASS (5/5 style-gating checks, all embedders correctly prefer
Shakespeare over modern English; full output in
memory/journal/agent-v42-1-ensemble-builder--success-smoke-test.md)

## Public API

```python
from scripts.v4_2.extended_ensemble import (
    ExtendedEmbedderEnsemble, BGEM3Output,
    BGE_M3_MODEL_ID, JINA_V3_MODEL_ID, QWEN3_MODEL_ID, BGE_RERANKER_MODEL_ID,
    EXPECTED_DIMS,
)
import torch

ens = ExtendedEmbedderEnsemble(device='cuda', dtype=torch.bfloat16)
# Loads all four models; raises RuntimeError with full diagnostics on ANY
# load failure (per user directive: no fallbacks).
```

### `embed_bge_m3(text: str) -> BGEM3Output`
Returns a dataclass with three fields:
- `.dense` : `np.ndarray` shape `(1024,)`, L2-normalized float32
- `.sparse`: `dict[int, float]` — token-id -> SPLADE-style weight.
              Typically 30-50 keys per text.
- `.colbert`: `np.ndarray` shape `(seq_len, 1024)`, per-token multi-vec
              (seq_len varies with the input text; max 8192 for BGE-M3).

### `embed_jina_v3(text: str, task: Literal[...] = 'classification') -> np.ndarray`
Returns an L2-normalized `np.ndarray` shape `(1024,)` dtype float32.
Valid task values: `'classification'`, `'text-matching'`, `'retrieval.query'`,
`'retrieval.passage'`, `'separation'`. For style discrimination use
`'classification'` (verified in smoke test).

### `embed_qwen3(text: str, task_prompt: str) -> np.ndarray`
Returns an L2-normalized `np.ndarray` shape `(1024,)` dtype float32.
`task_prompt` MUST be non-empty (RuntimeError if empty). The prompt is wrapped
as `"Instruct: {task_prompt.strip()}\nQuery: "` before the text.
Recommended task_prompt for v4.2 Shakespeare style:
    "Classify the literary style of the text as Shakespearean early modern
    English verse or as ordinary modern English prose."

### `rerank(query: str, passages: list[str]) -> list[float]`
Returns a list of raw logits (NOT sigmoid). Higher = more relevant. Can be
negative (the model was trained with BCE-with-logits). Smoke test shows
Shakespeare vs modern on "Shakespearean verse about summer" query:
-1.22 vs -7.17 — 5.95-unit gap. Use `torch.sigmoid` or `scipy.special.expit`
if you want [0,1].

### `describe() -> dict[str, dict[str, Any]]`
Returns per-model provenance. Example:
```
{"bge_m3": {"name": "BAAI/bge-m3", "params_million": 568.81, "dim": 1024,
            "context_length": 8192, "framework": "FlagEmbedding.BGEM3FlagModel",
            "weights_hash": "011b6000"}, ...}
```

## Sonnets constellation to use
ID: `18ef2460-9ab6-44fc-8d7e-8d7513174ac8`
Members: 313 (per `memory/context/current-phase-v4_2-extended-ensemble.md:105`)
Query via daemon RPC `get_constellation({"id": "<id>"})` -> members list
with member_ids. Then `get_memory_fingerprint(memoryId=member_id)` for the
text of each member.

## Recommended reference vectors (for Agent 2 to compute + persist)

For each of the 313 Sonnets members, encode through the ensemble and
aggregate as follows:

1. **BGE-M3 dense centroid + diag-cov**:
   - `dense_centroid   = mean(dense_vecs[313])` shape (1024,)
   - `dense_cov_diag   = var(dense_vecs[313], axis=0)` shape (1024,) — diag
     covariance is enough; full 1024x1024 is too large and unstable for 313
     samples.

2. **BGE-M3 ColBERT exemplar set**:
   - Keep ALL 313 per-member per-token matrices as a ragged list
     `colbert_exemplars: list[np.ndarray]`. Do NOT centroid ColBERT vectors;
     the whole point is MaxSim over the per-token structure.
   - A candidate's ColBERT score is `mean_i(max_j(cosine(candidate_tokens[i],
     exemplar_tokens[j])))` then averaged over the 313 exemplars.

3. **BGE-M3 sparse centroid (LEXICAL bucket, not style)**:
   - `sparse_centroid = mean(sparse_dicts[313])` — a defaultdict of token_id
     -> mean weight across all 313 members. Per the discovery note
     `memory/discoveries/agent-v42-1-ensemble-builder--bge-m3-sparse-is-lexical-not-style.md`,
     DO NOT put this in the style-aggregate bucket. It's a lexical-recall
     signal; use it only for keyword-overlap checks.

4. **Jina-v3 classification centroid + diag-cov**:
   - `jina_centroid = mean(jina_vecs[313])` shape (1024,)
   - Use task='classification' uniformly across all 313 members.

5. **Qwen3 centroid + diag-cov**:
   - `qwen_centroid = mean(qwen_vecs[313])` shape (1024,)
   - Use the SAME task_prompt across all 313 members (suggestion above).

## Output file (for Agent 3 to consume)
Save to: `~/.contextgraph/models/extended_ensemble/sonnets_references.npz`

Recommended npz keys:
- `bge_m3_dense_centroid`       : shape (1024,)
- `bge_m3_dense_cov_diag`       : shape (1024,)
- `bge_m3_colbert_exemplars_list`: see numpy.savez with allow_pickle=True
- `bge_m3_sparse_centroid_keys` : shape (K,) int32
- `bge_m3_sparse_centroid_vals` : shape (K,) float32
- `jina_v3_centroid`            : shape (1024,)
- `jina_v3_cov_diag`            : shape (1024,)
- `qwen3_centroid`              : shape (1024,)
- `qwen3_cov_diag`              : shape (1024,)
- `provenance`                  : JSON-stringified dict of ensemble.describe()
                                  + constellation_id + member count + timestamp
                                  + qwen3_task_prompt

## Performance notes
- Encoding 313 members on warm cache: estimate 313 * 4 models * ~0.1 s =
  ~125 s. BGE-M3 ColBERT is the slowest (per-token outputs).
- Batch encoding is supported by sentence-transformers' `encode()` (pass a list
  of strings). BGE-M3's `encode()` also accepts lists. This will speed up
  Agent 2 substantially — don't iterate one-at-a-time.

## Open items Agent 2 should be aware of
- The sparse view is a genuine signal but for LEXICAL discrimination, not
  style. Put it in the keyword bucket when Agent 3 integrates.
- Qwen3 is highly sensitive to the task_prompt; changing the prompt will
  move the centroid materially. Pick one prompt and stick to it.
- All four models produce float32 at the Python boundary (we convert from bf16
  tensor output via convert_to_numpy=True + asarray(..., dtype=np.float32)).
  Keep that when building references.

## Confidence
HIGH. Smoke test PASSES with real Sonnet 18 + real modern prose; VRAM budget
(5.4 GB) leaves room for concurrent daemon queries; API is clean and
documented.
