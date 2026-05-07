---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:00:00Z
status: active
tags: constellation, centroid, zero-imputation, documentation-gap
concern: C-1
---

# Concern-1: Constellation centroid zero-imputation

## Classification
**DOCUMENTATION-GAP with DEFECT-potential** (silent semantic collision)

## Severity
**Medium** — does not corrupt data, but silently conflates two distinct states:
"no observations" vs "observations whose mean is zero".

## Location
- `crates/context-graph-core/src/constellation/compiler.rs:200-249` — `observe`
  only updates `topic_profile`/`group_alignments`/`cross_correlations` Welford
  accumulators when `stored_topic_profile` is `Some(_)`.
- `crates/context-graph-core/src/constellation/compiler.rs:297-306` — `finalize`
  unconditionally copies `topic_profile.mean()` / `group_alignments.mean()` /
  `cross_correlations.mean()` into the output `Constellation`.
- `crates/context-graph-core/src/constellation/welford.rs:131-135` —
  `WelfordVector::mean()` returns a vector of zeros when `count == 0` (the
  initial state), with no distinguishing signal for the caller.
- `crates/context-graph-core/src/constellation/types.rs:53-59` — struct doc
  claims simply "Mean 13D topic profile across all members" with no mention
  of the empty-observation case.

## Root cause

The compiler's `observe` docstring at `compiler.rs:202-206` explicitly states
that when `stored_topic_profile = None`, the contribution is skipped and "not
zero-imputed, since zero-imputing would bias the centroid toward origin".

However, `WelfordVector::mean()` returns all-zeros for `count == 0`, so the
*result* the caller sees is **identical to zero-imputation**. Worse, the
`Constellation` struct surfaces these three centroids as plain `[f32; N]` with
no companion field telling downstream consumers how many observations actually
contributed. Live-testing on a fresh DB (before clustering has written any
rows to `CF_TOPIC_PROFILES`) yields exactly this pathological case — 100% of
members produce `None`, all three centroids are all-zero, and the consumer
cannot distinguish "no signal" from "all-signal happens to be zero".

Note also `store.rs:1144-1145`: `CF_TOPIC_PROFILES is written by the
clustering pipeline, not here`. So the "empty CF_TOPIC_PROFILES on fresh DB"
state is not an edge case — it's the default starting state until the user
runs clustering.

## Why the docstring is not sufficient

The claim "not zero-imputed" is technically true in the accumulator but
indistinguishable from zero-imputation in the output. A caller reading
`Constellation.topic_profile_centroid = [0, 0, ..., 0]` has no way to tell
whether:

1. No members had `stored_topic_profile` populated (the fresh-DB case), or
2. All members had `stored_topic_profile = [0, 0, ..., 0]` (mathematically
   impossible in practice because topic profiles are SRC-3-clamped alignment
   scores in `[0, 1]` — all-zero would mean every embedder produced an
   exactly-zero-norm vector, a ~nil probability on real data), or
3. The mean of a few non-zero profiles that happens to average to zero
   (low probability on a bounded non-negative signal in `[0, 1]`).

In practice only case (1) occurs — and that's the silent failure.

## Minimum elegant fix

Add an observation-count tail-field to `Constellation`. Bincode is
positional; appending one field at the end is backwards-compatible after a
version bump. Because this codebase has a strict "no backwards compatibility"
policy (`constraints.md`), bump `CONSTELLATION_VERSION` and rebuild.

```rust
// crates/context-graph-core/src/constellation/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constellation {
    // ... existing fields ...

    /// Number of members whose `stored_topic_profile` was Some(_) — the only
    /// members that contributed to `topic_profile_centroid`,
    /// `group_alignment_centroid`, and `cross_correlation_centroid`. When
    /// zero, all three centroids are meaningless zeros (fresh DB, clustering
    /// has not yet populated CF_TOPIC_PROFILES).
    pub topic_profile_observation_count: u64,
}
```

```rust
// crates/context-graph-core/src/constellation/compiler.rs:319-333
Ok(Constellation {
    // ... existing fields ...
    topic_profile_observation_count: topic_profile.count(),
})
```

Then update `render_constellation` (constellation_tools.rs:602-656) to always
surface this field, and teach `score_memory_against_constellation` (or its
callers) to refuse to score when
`topic_profile_observation_count == 0 && selector_kind != ExplicitIds`.

**Cheaper alternative (no version bump)**: Update the struct doc at
`types.rs:53-59` to explicitly warn about the zero case, and add a helper
`Constellation::has_topic_profile_observations(&self) -> bool` that returns
`self.topic_profile_centroid.iter().any(|&x| x != 0.0)` as a best-effort
probe. This is weaker but ships without an on-disk layout change.

## Recommendation

Pick the stronger fix (tail-field + version bump). The codebase explicitly
forbids backwards compatibility (`constraints.md`: "NO backwards
compatibility — if it breaks, fail fast with logs"), so the version bump is
free. Downstream consumers of the training corpus will absolutely use this
signal to decide whether to trust the topic/group/cross-correlation
centroids, so surfacing it is the elegant choice.

## Rationale for why this is better

1. Eliminates the silent-failure mode on fresh DBs.
2. A single `u64` at the end of the struct costs 8 bytes per constellation
   (negligible vs. the ~800KB typical payload from `per_embedder`).
3. Tail-append matches the existing bincode + version-byte contract.
4. Surfaces information the caller already needs: "is this constellation's
   topic data trustworthy?"
5. Lower cost than changing `[f32; N]` to `Option<[f32; N]>` throughout,
   which would ripple through every consumer including the Parquet export.
