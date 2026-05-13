---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:03:00Z
status: active
tags: mejepa, phase-0, over_engineer, collision, refinement
---

# over_engineer 24-bit suffix has measurable birthday-collision risk at 2,400-entry corpus scale

## Where

`crates/context-graph-mejepa-corpus/src/over_engineer.rs:20`
(`let suffix_id = rng.next_u64() & 0xFFFFFF;`).

## What

The unused-helper function name takes the form
`_unused_helper_{suffix_id:06x}`, where `suffix_id` is the low 24 bits
of a SplitMix64 output. 24 bits = 16,777,216 distinct slots.

Birthday-bound math: 50% collision probability at √(2 · 16.78M · ln 2) ≈
4,820 helpers. Doc 09 §3.1 plans a 2,400-entry corpus across 8
categories ⇒ ~300 over_engineer entries baseline; not at risk.

But: each operator iteration may be re-rolled with different seeds for
robustness studies, multiple helpers per file may be permitted in
later phases, and two corpus regenerations across the same task pool
double the population. By Phase 1 it's plausible to see thousands of
helper names in flight, at which point random pairwise collisions
become expected.

## Why it matters

If two corpus entries land on the same helper id, the mutation outputs
become indistinguishable on `mutated_source` content (modulo other
mutations that happen to differ). The corpus's deduplication key
should never depend on the helper-id alone, but a generator that DOES
hash on `mutated_source` would silently merge collided rows.

The fix is essentially free: widen to 48 bits.

## Recommended refinement

```rust
let suffix_id = rng.next_u64() & 0xFFFF_FFFF_FFFF; // 48-bit hex tag
let helper = format!(
    "\n\ndef _unused_helper_{suffix_id:012x}() -> None:\n    \
     \"\"\"Auto-generated dead helper for OverEngineer mutation.\"\"\"\n    \
     return None\n"
);
```

48 bits ⇒ √(2 · 281T · ln 2) ≈ 19.7M helpers before 50% collision.
Cost: 6 extra hex chars in the helper name. No test needs updating
(the existing `over_engineer_helper_id_is_seed_dependent` test only
asserts inequality).

## Severity

REFINEMENT-RECOMMENDED. Trivial fix; future-proofs against Phase 1
corpus expansion. Not blocking.
