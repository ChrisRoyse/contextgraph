# E14 — BGE-M3 Dense: Semantic Role in the Fusion Stack

## What model is E14?

`BAAI/bge-m3` is a multilingual multi-functionality encoder trained by BAAI
(Beijing Academy of AI). The **dense head** we use for E14 emits a single
1024-dimension L2-normalised vector per document, produced by:

1. Tokenising input with XLM-RoBERTa SentencePiece (250 002-token vocab
   covering 194 languages).
2. Running the XLM-RoBERTa-Large encoder (24 layers, 1024 hidden, 16 heads,
   8192-token context window).
3. Taking the `<s>` (CLS) token representation from the final layer.
4. L2-normalising to unit length.

## What do the E14 vectors mean?

An E14 vector is a point on the unit sphere in a **multilingual semantic
manifold**. Geometric distance between two E14 vectors corresponds to
semantic distance of meaning **regardless of the surface language, register,
or document length**.

Concretely, two documents have a high E14 cosine (≥ 0.7) when any of the
following hold:

| Relationship | Example | Other embedders' likely behaviour |
|--------------|---------|-----------------------------------|
| **Cross-lingual paraphrase** | `"quantum entanglement"` vs `"量子纠缠"` | E1/E10 are English-centric; they'd rate these very differently |
| **Translation pair** | `"The cat sat on the mat."` vs `"Le chat s'est assis sur le tapis."` | E1 ≈ 0.3–0.5; E14 ≈ 0.85+ |
| **Register-shifted paraphrase** | `"The patient exhibited marked improvement."` vs `"Dude got way better."` | E1/E10 penalise the register gap more than E14 |
| **Long-document similarity** | Two 2 000-token essays on the same topic | E1/E5/E7 see only the first 512 tokens; E14 sees all 8 192 |
| **Domain-general relevance** | Informal tweet vs formal paper on same event | E14's wider training catches this better than domain-specific models |
| **Content invariant to lexical noise** | `"dog bites man"` vs `"man bites dog"` | Low E14 cosine (different meanings despite lexical overlap) — E14 is harder to fool with keyword overlap than E6/E13 |

The training recipe (InfoNCE with mined hard negatives across MIRACL,
Mr. TyDi, ELI5, and a massive unsupervised multilingual corpus) teaches the
model to **separate meaning from surface form**. Lexical overlap alone does
not bring two documents close in E14 space; meaning does.

## What E14 does NOT capture

1. **Causal direction** — `"rain caused the flood"` and `"the flood caused
   rain"` can have near-identical E14 vectors. Use E5 for that.
2. **Graph structure** — E14 does not model entity–relation triples the
   way E8 (e5-large-v2 graph) and E11 (KEPLER + TransE) do.
3. **Exact token overlap** — E14 is a dense semantic embedder; lexical
   matches are the domain of E6 and E13 SPLADE.
4. **Precise code tokens** — E14 handles code *text* reasonably but E7
   (Qodo-Embed-1) is purpose-trained and more discriminating on API
   signatures and identifier names.
5. **Fine-grained temporal ordering** — E2/E3/E4 are the temporal stack.
   E14 is temporally agnostic.
6. **Token-level late interaction** — that's E12 (ColBERT MaxSim).
7. **Recency bias / freshness** — E14 has no notion of time.

## Where E14 wins that no other embedder does

1. **Cross-lingual retrieval.** E14 is the only embedder in the stack with
   genuine multilingual training. Query in English, retrieve Mandarin docs
   about the same concept — E14 ranks them highly; nothing else does.
2. **Long-document matching.** At 8 192-token context, E14 sees the full
   document where every other dense embedder truncates around 512 tokens.
   A query buried in the second half of a long document only surfaces when
   E14 participates.
3. **Style/register neutrality.** Informal vs formal rewordings are
   penalised less by E14 than by e5-family embedders.
4. **Robust-to-lexical-noise meaning.** E14 is the most reliable signal
   when surface form is unrelated but meaning is the same.

## How context-graph exploits E14

### Default `semantic_search` profile (E14 contribution)

`E14 = 0.10` — a modest boost over E1 that kicks in when a document's
meaning outruns its English surface form. For pure English corpora this
still helps on long docs and paraphrase-heavy content.

### New `multilingual_search` profile

Dominant E14 (0.45) paired with E1 (0.20), E10 (0.15) — for queries where
the user expects hits across languages (mixed corpus, translated sources,
cross-language search).

### New `long_context` profile

E14 is the only embedder that sees past 512 tokens. For explicit long-doc
search, E14 carries 0.45 of the weight; E1 (0.15) and E10 (0.15) hold the
rest of the semantic load; E7 (0.10) and E11 (0.10) contribute
domain-specific structure.

### New `translation_finder` profile

Purpose-built to find translation pairs and cross-lingual near-duplicates.
E14 = 0.70 primary; E10 = 0.20 paraphrase backup; others 0.10.

### Cross-embedder anomaly detection

The `search_cross_embedder_anomalies` MCP tool can now use
`--primary E14 --contrast E1` to surface documents that E14 finds but E1
misses — i.e. content whose meaning is well-matched but whose English
surface form is different or absent. This is how a user discovers
non-English or stylistically distant hits that pure E1-based search would
never surface.

### Auto-routing by query characteristics

At query time, the MCP layer can inspect the incoming query and auto-pick
a profile: non-Latin-script characters → `multilingual_search`; query
length > 500 tokens → `long_context`. These heuristics are not yet wired
but the profiles are in place so the runtime can switch in O(1).

## Invariants kept

1. **14-element fingerprint array everywhere.** Adding E14 did not change
   array arity; Phase A guaranteed this.
2. **Legacy records stay searchable.** `#[serde(default)]` on the field
   means pre-Phase-A fingerprints deserialize with an empty E14 Vec, and
   fusion paths treat empty as 0.0 contribution.
3. **Every profile still sums to 1.0.** When we boost E14 we rebalance by
   shaving from E1/E10 (the nearest-neighbour semantic slots); no profile
   leaves its partition.
4. **HNSW invariant.** E14 vectors are pushed to `EmbedderIndex::E14BgeM3Dense`
   (1024D Cosine) at `store_memory` time. Search paths query that index
   when the active profile has `weights[13] > 0.0`.

## Expected empirical signatures post-activation

Synthetic FSV (see `tests/e14_optimization_fsv.rs`):

- **Identical content, different languages**: E14 cosine ≥ 0.8; E1 cosine
  likely < 0.5.
- **Paraphrase in same language**: E14 and E10 both ≥ 0.7; E1 may be
  lower for heavy register shift.
- **Unrelated documents**: all embedders < 0.3 cosine; E14 is not looser
  than its peers on unrelated content.
- **Dog-bites-man / man-bites-dog**: E6/E13 cosine ≥ 0.5 (lexical
  overlap), E14 < 0.4 (meaning difference).

## References

- Model card: https://huggingface.co/BAAI/bge-m3
- Paper: *"BGE M3-Embedding: Multi-Lingual, Multi-Functionality,
  Multi-Granularity Text Embeddings Through Self-Knowledge Distillation"*
  (Chen et al., 2024).
- Training data breakdown — MIRACL (18 langs), Mr. TyDi (11 langs),
  MS MARCO, NQ, plus a large unsupervised multilingual corpus.
