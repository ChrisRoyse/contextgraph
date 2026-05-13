---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T01:46:43Z
status: active
tags: tier2,mincut,granger,certificates,fail-closed,fsv
---

# Tier-2 Recent Hardening Findings

## What Happened

Review of the latest Tier-2 commit found several follow-up hardening issues that were not fixed by the prior session report:

- `context-graph-mincut::weights_from_edges` silently ignored out-of-range endpoints and self-loops. That hides bad caller input at the graph-construction boundary.
- `granger_test` documented the unrestricted model degrees-of-freedom requirement but checked `2*lag+2` raw observations instead of `3*lag+2`, letting invalid inputs reach a later, less-specific df check.
- `stoer_wagner` had misleading dead code around `prev_last_set` and a comment claiming the phase cut had to be recomputed even though the selected vertex weight already carries that value.
- `MerkleProof.leaf_count` needed stronger API documentation: it is used for structural proof verification but must be matched against the certificate/tree source of truth by callers.
- A concurrent recent `causal_mask.rs` addition was referenced from `solver::lib` but declared a missing `causal_mask_tests.rs`, causing `cargo fmt --all` to fail. The module also needed checked `n*n` allocation and non-finite score rejection before masking/softmax.

## Why It Matters

The project's fail-closed rule means invalid input should be rejected where it crosses a public boundary, not ignored. Numerical/statistical APIs need their preconditions to match the actual model degrees of freedom. Certificate APIs need public fields to make their trust boundaries explicit.

## Related

- `crates/context-graph-mincut/src/lib.rs`
- `crates/context-graph-solver/src/granger.rs`
- `crates/context-graph-witness/src/cert.rs`
