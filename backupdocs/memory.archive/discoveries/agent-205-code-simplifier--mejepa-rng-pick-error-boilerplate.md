---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:05:00Z
status: active
tags: mejepa, phase-0, dry, rng, refinement
---

# `rng.pick_index(...).ok_or_else(... op_failed("rng", ...))` boilerplate repeated 5x

## Where

- `subtle_flip.rs:38-44`
- `off_by_one.rs:34-40`
- `swap_variable.rs:55-61` AND `:63-69` (twice)
- `delete_test_call.rs:50-56`

(over_engineer / compile_error use `rng.next_u64()` directly, not
`pick_index`.)

## What

Every operator does the same dance:

```rust
let pick = rng.pick_index(candidates.len()).ok_or_else(|| {
    MutationError::op_failed(
        "rng",
        "SplitMix64::pick_index returned None despite non-empty candidates",
        "internal error; report",
    )
})?;
```

Five copies. The `pick_index` API in `prng.rs` already returns `None`
only when `len == 0`; every caller has just verified `len >= 1` (the
empty case maps to `NoMutationSite` earlier). So the `ok_or_else` arm
is unreachable in practice, but written defensively.

## Why it matters

The error-message string "SplitMix64::pick_index returned None despite
non-empty candidates" is repeated verbatim 5 times. If we ever rename
the PRNG or change the precondition, all 5 sites need updating.

## Recommended refinement

Two options:

1. **Add `SplitMix64::pick_index_unchecked(len: usize) -> usize`** that
   panics on `len == 0` (with a clear panic message). Callers do
   `let pick = rng.pick_index_unchecked(candidates.len());`. Loses the
   typed error on the impossible-in-practice path, but matches what
   the code already asserts.

2. **Add a crate-internal helper** in `prng.rs` or `lib.rs`:

   ```rust
   pub(crate) fn pick_or_op_fail(
       rng: &mut SplitMix64, len: usize,
   ) -> MutationResult<usize> {
       rng.pick_index(len).ok_or_else(|| MutationError::op_failed(
           "rng",
           "SplitMix64::pick_index returned None despite non-empty candidates",
           "internal error; report",
       ))
   }
   ```

   Each call site shrinks to `let pick = pick_or_op_fail(&mut rng, candidates.len())?;`.

I'd take (2). Same defensive shape, single source of truth, and it
doesn't break the pure-error contract that the rest of the crate uses.

## Severity

REFINEMENT-RECOMMENDED. Pure DRY, no behavior change.
