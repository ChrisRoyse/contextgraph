---
namespace: discoveries
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:00:00Z
status: active
tags: tier3, causal-mask, micro-optimization, low-priority
---

# apply_mask_to_scores does two passes over `scores` — could fuse

`crates/context-graph-solver/src/causal_mask.rs:146-161` walks `scores`
twice: first to validate finiteness (`for (idx, value) in scores.iter().enumerate()`
at line 146), then to apply the mask (`for (idx, allowed) in mask.iter().enumerate()`
at line 157). The two-pass shape is intentional — it guarantees that on a
non-finite-score rejection no positions have been written to NEG_INFINITY
(the FSV asserts this at `causal_mask_fsv.rs:73-78` and the test at
`causal_mask_tests.rs:62-72` checks `scores == before` on rejection).

A fused single pass that buffered "would-write" decisions and only
committed once both invariants held would lose nothing semantically but
would obscure the "fail-closed before any mutation" contract. **Verdict:
keep the two passes. The clarity wins over the micro-optimization** for
matrices small enough that this matters, and for the giant matrices
where it would matter, the validation pass is dwarfed by the upstream
attention computation that produced `scores`. No change recommended.

Cite: `causal_mask.rs:146-161`.
