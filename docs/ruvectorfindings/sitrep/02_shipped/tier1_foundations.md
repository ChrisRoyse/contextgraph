# Tier-1 Foundation Primitives

> **Shipped:** 2026-05-08
> **Crates:** `crates/context-graph-solver/`, `crates/context-graph-witness/`
> **Tests:** 49 passing across both crates
> **FSV evidence:** `/tmp/contextgraph-tier1-fsv/`

## What this primitive does

Three independent foundation primitives that doc 05 §1.1, §1.4, and §1.6
all require before any downstream Tier-2/3 or Phase 0..8 work can land:

1. **Generic Maximal Marginal Relevance (MMR)** in
   `context-graph-solver/src/mmr.rs` — diversity-vs-relevance reranking
   over arbitrary `(score, embedding)` items via `select_mmr(items,
   lambda, k)`. Generic over Item/Score/Embedding traits so it can serve
   both retrieval and Phase 4 verdict-set diversification.

2. **Split conformal predictor** in
   `context-graph-solver/src/conformal.rs` — calibrated coverage
   guarantee for arbitrary scalar-prediction tasks. `fit_quantile(scores,
   alpha)` returns the (1-α)-quantile of nonconformity scores; the
   `interval_for(prediction, quantile)` API wraps any underlying
   model into a calibrated [lo, hi] band. Phase 4 reuses this for the
   ME-JEPA verifier.

3. **ed25519 segment signing** in `context-graph-witness/` (gated under
   `ed25519` cargo feature) — sign and verify witness-chain segments
   with detached ed25519 signatures. Provides the cryptographic root
   that the Stage 0 + Stage F + Merkle cert work (Tier-2) chains on top
   of.

## Why it matters

Doc 05 §1.1 (MMR) and §1.4 (conformal) call out these as table stakes
for any retrieval/predict-with-uncertainty system. Doc 05 §1.6 makes
ed25519 segment signing the cryptographic foundation for the
witness-chain authenticity property.

Without these three primitives, downstream work has to inline its own
ad-hoc versions, duplicating math and creating divergence between
Tier-1 and Tier-2 + Phase 4 reuse paths.

## Implementation notes

### MMR
- `select_mmr<I, S, E>(items: &[(S, E)], lambda: f32, k: usize) -> Vec<usize>`
- Score type S: `Into<f32> + Copy`. Embedding type E: a slice-shaped
  `AsRef<[f32]>` so it works with `Vec<f32>`, `&[f32]`, and `[f32; N]`
  alike.
- Cosine similarity used for the diversity term; documented as the
  intended distance even though the trait surface allows substitution.
- `lambda` ∈ [0, 1]; `k` ≤ items.len() asserted.

### Split conformal
- Generic over Score: `f32 + Copy + PartialOrd`.
- `fit_quantile` is `O(n log n)` (sort once); `interval_for` is `O(1)`
  per query.
- Coverage guarantee holds when nonconformity scores are exchangeable;
  the doc explicitly notes this caveat.

### ed25519 segment signing
- `sign_segment(secret_key: &SigningKey, segment: &[u8]) -> Signature`
  + `verify_segment(public_key: &VerifyingKey, segment: &[u8], sig:
  &Signature) -> bool`.
- Segments are SHAKE-256 hash-prefixed before signing so signature
  forgery requires breaking either ed25519 OR SHAKE-256.
- Cargo feature gate `ed25519` keeps the dependency optional for crates
  that consume `context-graph-witness` for the chain only.

### FSV
Physical FSV at `/tmp/contextgraph-tier1-fsv/`:
- `mmr-evidence.json` — 100-item synthetic dataset with 3 reference
  outputs at λ=0.0, 0.5, 1.0; structural-equality round-trip verified.
- `conformal-evidence.json` — 1,000-sample synthetic calibration set;
  empirical coverage at α=0.10 measured at 0.901 (within
  doc-09-Phase-4 spec [0.88, 0.92]).
- `ed25519-evidence.json` — 50-segment chain signed with a fixed key;
  3 tampered-segment cases verified to fail verification.

## What this enables downstream

- **Tier-2 Stage 0/F/Merkle certs** — sign segments with ed25519, hash
  with SHAKE-256.
- **Phase 4 conformal verdict calibration** — the conformal primitive
  is dropped in directly with a JEPA-specific nonconformity function.
- **Phase 7 trigger-decision diversity** — MMR could rerank candidate
  patches by predicted-pass-probability vs. embedding diversity.
- **Phase 6 storage** — ed25519 signatures bind checkpoint Merkle roots.

## See also

- Spec: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md §1.1, §1.4, §1.6`
- Doc 09 reuse points: §3.6 (Phase 4 conformal), §3.4 (Phase 3 cert
  signatures)
- Memory entry: `project_tier1_foundation_primitives_shipped.md`
