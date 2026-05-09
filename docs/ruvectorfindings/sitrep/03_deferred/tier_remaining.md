# Tier Items Deferred (Doc 05)

> **Status (2026-05-09):** non-shipped Tier-1/2/3 primitives. Tier-4 is
> the explicit defer/skip pile per doc 05's own taxonomy.

## Doc 05 tier overview

Per doc 05 §0:
- **Tier 1**: foundational primitives every system needs.
- **Tier 2**: integration primitives that combine multiple Tier-1 atoms.
- **Tier 3**: advanced primitives — high value but risky / time-consuming.
- **Tier 4**: explicit defer/skip pile — captured for completeness, not
  intended for this generation of the system.

## Tier 1 — deferred items

### §1.2 BMSSP (Bounded Multi-Source Shortest-Path) — V-cycle implementation

> Status: NOT shipped.
> Priority: medium. Used by doc 09 §3.5 dataflow analysis (E_DataFlow).

The Bounded Multi-Source SSSP V-cycle algorithm computes shortest
paths from K sources to V vertices in `O((V+E) log V + K * V)` rather
than naive `O(K * (V+E) log V)`. Used in dataflow analysis to compute
def-use distances across the function-call graph.

**Why deferred**: Phase 1 instrument E_DataFlow uses a placeholder
hashed-feature encoder (per `phase1.md` Turn 5); BMSSP's exact graph
distances become load-bearing only when the placeholder is replaced
with a real graph-attention encoder during Phase 3 training.

### §1.5 LCM (Largest Common Mask)

> Status: NOT shipped.
> Priority: low. Used by causal-graph deduplication.

Identifies the maximal common attention-mask pattern across a batch of
attention layers. Useful for KV-cache reuse at inference time, but
ME-JEPA's predictor is small enough that this isn't load-bearing yet.

**Why deferred**: scope-creep. Doc 09 doesn't depend on §1.5 for any
phase DoD.

## Tier 2 — deferred items

### §2.1 SONA (Self-Optimizing Neural Architecture)

> Status: NOT shipped.
> Priority: low. NOT used by doc 09 phases.

Architecture-search-flavored primitive. Doc 06 explicitly classifies
this as a **non-transferable to ME-JEPA** — see
`06_RISKS_AND_NON_TRANSFERABLE.md ❌-7`. ME-JEPA's predictor architecture
(6-layer pre-LN transformer, hidden 1024, 8 heads) is fixed by doc 09
§2.4; SONA's value would require a different problem formulation.

**Why deferred**: explicit non-transferable. Will not be implemented
for ME-JEPA-Code.

### §2.2 Verified Training (cryptographic training proofs)

> Status: NOT shipped.
> Priority: medium. Adjacent to Phase 3 per-step certs (which ARE planned).

Distinct from per-step certs: §2.2 specifies a zero-knowledge proof
that training was carried out per the published spec. The Phase 3 cert
chain is a **transparency log**, not a ZK proof — anyone can replay
and verify, but no privacy is preserved.

**Why deferred**: doc 09 §3.4 does not require ZK proofs (only the
per-step transparency cert), so §2.2 is over-spec for the build-ready
master plan.

### §2.5 Matryoshka + Graph RAG (Leiden community detection)

> Status: NOT shipped.
> Priority: medium-high. Used by doc 09 §3.5 panel construction.

Matryoshka embeddings (multi-resolution single embedding) + Graph RAG
(retrieval-augmented generation over a property graph) + Leiden
clustering for community structure.

The `context-graph-embeddings` crate ships **legacy** Matryoshka
support (`E1Matryoshka128`); doc 05 §2.5 specifies a richer
re-implementation: per-token Matryoshka heads + Leiden-clustered graph
RAG retrieval at multiple granularities.

**Why deferred**: Phase 1 E_Test/E_Problem/E_CommitMsg instruments
reuse `E1` (the legacy path). Migrating to the richer §2.5 form is a
Phase 3 / Phase 8 optimization if the holdout-agreement gate falls
short.

## Tier 3 — deferred items

### §3.1 Homomorphic-secure attention

> Status: NOT shipped.
> Priority: low. NOT used by doc 09 phases.

Privacy-preserving attention using homomorphic encryption.
Operationally relevant when ME-JEPA verifies patches on private
codebases, not for SWE-bench Lite's public corpus.

**Why deferred**: not in doc 09's critical path. Future feature for
deployment in regulated/private contexts.

### §3.3 Multi-resolution retrieval

> Status: NOT shipped.
> Priority: medium. Adjacent to doc 09 §3.6 verify() inference.

A retrieval primitive that returns results at multiple granularities
(token, sentence, doc, repo). Helps `verify(patch)` retrieve relevant
historical attempts during inference.

**Why deferred**: Phase 4 verify() doesn't need historical retrieval
in the MVP — the trained predictor is self-contained. §3.3 becomes
useful in a Phase 4.5+ "RAG-augmented verify" mode.

### §3.4 Agentic tool routing

> Status: NOT shipped.
> Priority: low. NOT in doc 09 critical path.

Routes tool calls based on predicted utility. Useful for the inner
actor (retired post-2026-05-09 pivot), not the outer optimizer or ME-JEPA. Now obsolete entirely.

### §3.5 Causal counterfactual reasoning

> Status: NOT shipped.
> Priority: medium. Augments doc 09 §3.7 Phase 5 fine-tuning.

Counterfactual: "would the predictor's verdict change if E_AST were
zeroed out?" Useful as an interpretability tool to debug Phase 8
correlation failures.

**Why deferred**: diagnostic primitive, not on the build-ready
critical path.

### §3.6 Active learning corpus expansion

> Status: NOT shipped.
> Priority: medium. Augments Phase 5 + Phase 8.

Selects the next mutation samples to add to the corpus based on which
samples the current predictor is least confident on. Speeds up
holdout-agreement growth.

**Why deferred**: Phase 0 already specifies a fixed 2,400-entry
corpus; active learning is a Phase 5+ corpus-growth strategy.

## Tier 4

Per doc 05 itself: explicit defer pile. Not in scope for this
generation.

## Priority ranking for next-N turns

If returning to Tier work after Phase 1 instruments are further along:

1. **§2.5 Matryoshka + Graph RAG** — load-bearing for E_Test reuse
   path; affects 3 of 14 instruments.
2. **§1.2 BMSSP V-cycle** — load-bearing for E_DataFlow real
   neural-network replacement.
3. **§3.5 Causal counterfactual** — diagnostic primitive for Phase 8
   correlation-failure debugging.
4. Lower priority: §1.5, §2.1 (skip), §2.2, §3.1, §3.3, §3.4, §3.6.

## See also

- Doc 05: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md`
- Non-transferables: `docs/ruvectorfindings/06_RISKS_AND_NON_TRANSFERABLE.md`
- Doc 09 phase dependencies: `phases/phase0.md`..`phases/phase8.md`
