---
namespace: journal
created: 2026-05-12T16:20:00Z
updated: 2026-05-12T16:20:00Z
status: active
tags: phase-d, fsv, embedder-forward, failure
---

# Failure: Forward Wrapper FSV Stale Edge

## What Happened
Running `cargo run -p context-graph-mejepa-embedders --example forward_wrapper_fsv`
failed before writing final evidence. The edge case expected `embedder_model_id(E2)`
to fail, but E2 is now an active algorithmic forward slot and returns `Ok`.

## Why It Matters
This was a stale verification harness, not a production forward failure. The
Phase D implementation promoted E2/E3/E4/E9 into real algorithmic content
forwards, so FSV must use a true unsupported/non-content slot when proving
fail-closed behavior.

## Fix
Updated the edge case to use E17, which remains learner-state/affect and is
not a content forward slot.

## Related
- `crates/context-graph-mejepa-embedders/examples/forward_wrapper_fsv.rs`
- Tags: phase-d, fsv, embedder-forward
