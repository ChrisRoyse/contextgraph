---
namespace: discoveries
agent_id: agent-09-simplifier-v2
timestamp: 2026-04-14T17:30:00Z
status: low
tags: constellation, docs, helper, silent-zero
severity: low
topic: has_topic_profile_signal-docstring-states-too-narrow-a-cause
---

# `has_topic_profile_signal` docstring asserts a narrower cause than the predicate actually detects

## Scope
`crates/context-graph-core/src/constellation/types.rs:83-100`

## Observation
The helper is:

```rust
pub fn has_topic_profile_signal(&self) -> bool {
    self.topic_profile_centroid.iter().any(|&x| x != 0.0)
}
```

Docstring (paraphrased): *"an all-zero centroid reliably indicates that `stored_topic_profile` was `None` for every member"*.

That's one cause, not the only one. Looking at `compiler.rs::observe` (lines 241-247), three Welford accumulators are all gated on `stored_topic_profile = Some(_)`:

```rust
if let Some(prof) = stored_topic_profile {
    self.topic_profile.observe(&prof);
    let groups = crate::training::compute_group_alignments(&prof);
    self.group_alignments.observe(&groups);
    let cc = crate::training::compute_cross_correlations(&prof, synergy);
    self.cross_correlations.observe(&cc);
}
```

So the helper returning `false` actually means *either*:
1. every member had `stored_topic_profile == None`, **or**
2. every member had `stored_topic_profile == Some([0.0; 13])` — legitimately all-zero topic alignments.

Case 2 is rare but not impossible (a member with no topic memberships and no group alignments). The helper's semantic ("don't trust this centroid as a topic signal") is still correct in case 2, but the docstring causally narrows to case 1 only.

## Cross-check: is `cross_correlation_centroid` a more sensitive probe?
Agent asked this directly. **No** — and that's a genuinely interesting subtlety worth recording in the lessons file:

- Both `topic_profile_centroid` and `cross_correlation_centroid` are fed from the **same** `stored_topic_profile` gate in `observe`.
- If `topic_profile` Welford count == 0, then `cross_correlations` Welford count == 0, because both `observe` calls live inside the same `if let Some(prof)` branch.
- Welford `mean()` returns a zero-filled vector when count == 0.
- Therefore `topic_profile_centroid.any_nonzero() == cross_correlation_centroid.any_nonzero()` in all cases where the compiler was the writer.

The only way those could diverge is a hand-constructed `Constellation` (tests, external tooling) — which is out of scope for a runtime signal-trust helper. Checking the 13D slot is strictly cheaper than scanning 78 slots and returns the same answer.

**Bottom line:** the helper picks the right field; only the docstring's stated cause is too narrow.

## Severity
**low** — docstring accuracy issue. The helper's return value and all documented caller obligations remain correct.

## Minimal diff
Replace lines 87-94 with:

```rust
/// Because topic profiles are SRC-3-normalized alignments in `[0, 1]` and
/// a real member essentially always has a non-empty vector for some
/// embedder, an all-zero centroid means the Welford accumulator observed
/// no usable topic profile — either `stored_topic_profile` was `None` for
/// every member (fresh DB, clustering pipeline hasn't populated
/// `CF_TOPIC_PROFILES` yet), OR every member's stored profile was itself
/// all-zero. Both cases mean "do not trust this field as a topic signal".
```

(Also consider removing the final `.any(|&x| x != 0.0)` → `.any(|&x| x > 0.0)` for defensiveness against hypothetical negative noise — purely cosmetic, skip unless we've seen negatives in practice.)

## Verdict
Worth a one-paragraph docstring tweak. Not a correctness bug.
