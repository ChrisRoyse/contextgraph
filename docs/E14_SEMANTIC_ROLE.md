# E14 BGE-M3 Dense Semantic Role

E14 is the BGE-M3 dense semantic slot in the 14-embedder fusion stack. It is
used when retrieval needs multilingual meaning, Cross-lingual matching,
long-document context, or translation-pair discovery that English-centric
semantic embedders can miss.

## Model Shape

The deployed E14 role is the dense head of `BAAI/bge-m3`. The input text is
tokenized with the XLM-RoBERTa SentencePiece vocabulary, encoded by the
XLM-RoBERTa backbone, represented with the CLS token from the final layer, and
normalized as one dense vector. The model supports an 8192 token context window,
which is why E14 is the preferred dense signal for long documents.

## Retrieval Meaning

E14 should be interpreted as a multilingual semantic manifold:

- documents with the same meaning in different languages should be close;
- translation pairs should rank highly even when lexical overlap is low;
- long documents can match on content that appears beyond short-context
  embedders' effective windows;
- style and register changes should matter less than meaning changes.

E14 is not a replacement for every other embedder. Causal direction still
belongs primarily to E5, code-specific identifier matching belongs primarily to
E7, graph/entity structure belongs to E8 and E11, exact sparse recall belongs to
E6 and E13, and late-interaction reranking belongs to E12.

## Weight Profiles

The E14-aware profiles are explicit and opt in:

- `multilingual_search`: use for multilingual corpora and cross-language
  retrieval.
- `long_context`: use when long source text or long queries need the 8192 token
  context window.
- `translation_finder`: use to find translation pairs and Cross-lingual
  near-duplicates.

Learner-state-aware routing may also select E14-positive affect profiles:
`affect_repair`, `affect_priming`, and `affect_neutral`. Those profiles keep
E14 active while changing the retrieval balance according to persisted learner
state; they do not create a separate learner embedder.

## Operational Invariants

- `NUM_EMBEDDERS` remains 14.
- Every static weight profile must contain 14 weights and sum to approximately
  1.0.
- Legacy semantic profiles remain available by name.
- E14-optimized profiles must remain available by name:
  `multilingual_search`, `long_context`, and `translation_finder`.
- `search.rs` must compute the E14 cosine path instead of hardcoding slot 13 to
  zero.
- Warmup and provider registries must include BGE-M3 dense model metadata.

## Expected FSV Signatures

The `e14_optimization_fsv` test suite verifies this file as a source of truth
and checks the runtime surface with synthetic evidence:

- multilingual examples should show strong E14 similarity;
- translation_finder should select E14 as the dominant signal;
- long_context should select E14 as the primary long-document dense signal;
- unrelated text should not become similar merely because E14 is active.

## References

- BAAI BGE-M3 model card: https://huggingface.co/BAAI/bge-m3
- BGE M3-Embedding paper: Chen et al., 2024,
  "Multi-Lingual, Multi-Functionality, Multi-Granularity Text Embeddings
  Through Self-Knowledge Distillation"
