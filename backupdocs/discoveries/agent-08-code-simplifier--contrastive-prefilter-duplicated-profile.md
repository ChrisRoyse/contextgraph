---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:15:00Z
status: active
tags: contrastive, mining, performance, duplication, wasted-computation
---

# contrastive_tools.rs: similarity_profile computed three times per pair in the worst case

## Classification
**NIT / SIMPLIFICATION** (adds to the prior review's M-1 finding with new
evidence of a third redundant computation not captured before)

## Severity
**Low-Medium** — correctness is fine; compute is wasted on larger pools.
Prior review's M-1 called out the 2x duplication; my re-reading shows a
third path that recomputes when the pair reaches `mine_pair_from_candidate`.

## Location
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:281-284`
  — first profile computation (top-K pre-filter).
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:320-328`
  (implicit inside `mine_pair_from_candidate`) — second computation:
  `mine_pair_from_candidate` calls `similarity_profile(anchor_fp, neg_fp)`
  at `mining.rs:158`.
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:329-346`
  — third computation on the skip path: `let profile =
  similarity_profile(&anchor_fp.semantic, &neg_fp.semantic);` (line 330)
  to classify the skip reason.

## Cost analysis

For a `candidatePoolSize=500` default, per the plan:
- First profile: 500 × 499 = 249,500 computations (pre-filter scan).
- Second profile (inside `mine_pair_from_candidate`): runs only on the
  top-K candidates that survive sort — up to 500 × 10 = 5,000 computations.
- Third profile (skip-reason classification): runs only when
  `mine_pair_from_candidate` returns `None` — most skipped survivors, so
  up to 5,000 additional computations.

Each `similarity_profile` call does 13 × (1024D dot + norm) = ~26K FMAs.
At defaults that's 259,500 × 26K = ~6.7B FMA per anchor worst-case before
any pair persists. The CPU is not the bottleneck on real-world pools (I/O
dominates), but the logic is still needlessly tangled.

## Root cause

Three separate code paths all "figure out the profile" without sharing it:
1. The pre-filter (line 281) discards the profile after storing it in the
   `scored` tuple — but downstream code doesn't read the stored profile,
   only the `(neg_idx, disagreement, _profile)` is destructured with an
   underscore at line 306.
2. `mine_pair_from_candidate` recomputes from scratch because its signature
   takes the two fingerprints, not a pre-computed profile.
3. The skip-classification block doesn't read the `_profile` from step 1;
   it calls `similarity_profile` a third time at line 330.

## Minimum elegant fix

Two independent simplifications, each valuable on its own:

### Fix A: Reuse the `_profile` already captured by the pre-filter.

Stop discarding it at line 306; pass it to both the mine call (via a new
function variant that takes the pre-computed profile) and the
skip-classification block:

```rust
for (neg_idx, _disagreement, profile) in scored {  // drop the leading underscore
    if pairs_stored >= cfg.max_pairs {
        break 'anchor_loop;
    }
    let neg_fp = &pool[neg_idx];
    let anchor_text = content_by_id.get(&anchor_fp.id).cloned().unwrap_or_default();
    let negative_text = content_by_id.get(&neg_fp.id).cloned().unwrap_or_default();

    // New mining entry-point that takes a pre-computed profile.
    let Some(pair) = mine_pair_with_profile(
        anchor_fp.id, &anchor_text, negative_id, &negative_text, profile, &cfg,
    ) else {
        // Use the captured profile; no recomputation.
        let (max_high, min_low) = high_low_bounds(&profile, cfg.high_threshold, cfg.low_threshold);
        if !(max_high - min_low).is_finite() || (max_high - min_low) < cfg.min_disagreement {
            pairs_skipped_below_threshold += 1;
        } else {
            pairs_skipped_kind_filter += 1;
        }
        continue;
    };
    // ...
}
```

Add `mine_pair_with_profile` to `core/src/contrastive/mining.rs` as a peer
of `mine_pair_from_candidate` that takes the profile by-value and skips the
recomputation:

```rust
pub fn mine_pair_with_profile(
    anchor_id: Uuid, anchor_text: &str,
    negative_id: Uuid, negative_text: &str,
    profile: [f32; NUM_EMBEDDERS],
    cfg: &MiningConfig,
) -> Option<ContrastivePair> {
    if anchor_id == negative_id { return None; }
    let (high_embedders, low_embedders) = high_low_indices(&profile, cfg);
    if high_embedders.is_empty() || low_embedders.is_empty() { return None; }
    let max_high = high_embedders.iter().map(|&i| profile[i as usize]).fold(f32::NEG_INFINITY, f32::max);
    let min_low = low_embedders.iter().map(|&i| profile[i as usize]).fold(f32::INFINITY, f32::min);
    let disagreement = max_high - min_low;
    if !disagreement.is_finite() || disagreement < cfg.min_disagreement { return None; }
    let kind = classify_anomaly(&profile, cfg.high_threshold, cfg.low_threshold);
    if let Some(kinds) = &cfg.kinds { if !kinds.contains(&kind) { return None; } }
    Some(ContrastivePair { /* fields same as before */ })
}
```

Keep the original `mine_pair_from_candidate` wrapper for single-profile
callers.

### Fix B (bigger): Drop the skip-reason distinction and emit one counter.

Per the prior review's M-1 alternative: if the counter accuracy is not
worth the handler complexity, merge `pairs_skipped_below_threshold` +
`pairs_skipped_kind_filter` into a single `pairs_skipped` counter. Removes
the third `similarity_profile` call entirely.

This is the simplest fix and I'd recommend it. The downstream consumer
(the FSV report) can distinguish the two through per-pair inspection;
aggregate counters only need "how many survived".

## Recommendation

Apply **Fix B**. It deletes the most code, matches the spirit of the
plan's "signal clarity over instrumentation", and removes the third
redundant computation. If the team wants the counter granularity back
later, add a debug-only `kind_filter_skips_estimate` field computed on the
cheap from the `scored` tuple.

## Rationale for why simpler is better

1. Three computations of the same deterministic function on the same
   inputs is a code-smell regardless of micro-benchmark cost.
2. The skip-reason distinction is an instrumentation leak into the miner's
   core logic — debug-stats should not drive control flow.
3. Merging the counters lets `MiningSummary` lose its `Default`-only
   pairs_skipped_{below_threshold, kind_filter} split (addresses the prior
   L-1 refinement too: `MiningSummary` would have all its fields populated
   uniformly, so the `#[derive(Default)]` sheds its ambiguity).
