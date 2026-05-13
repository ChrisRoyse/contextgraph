---
namespace: discoveries
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:00:00Z
status: active
tags: tier3, causal-mask, ergonomics, refinement
---

# `retrocausal_safety_check` doc does not steer callers toward the bulk path

`causal_mask.rs:165-178` describes `retrocausal_safety_check` as the
"explicit fail-closed check callers run BEFORE reading off an attention
weight at a specific position when they did not pre-mask the score
matrix." That is correct, but a naive caller iterating the full N×N
score matrix would call this O(N²) times — at which point
`apply_mask_to_scores` is strictly cheaper and produces a less-leak-prone
artifact (the matrix itself, not a yes/no per cell).

Recommended one-line refinement (R2): in the doc comment add a
"Performance" or "When to use" note steering bulk callers to
`apply_mask_to_scores`, e.g. "For per-cell auditing or sparse spot
checks. For bulk masking, prefer `apply_mask_to_scores`, which is O(N²)
once instead of O(N²) function calls and produces the masked matrix
directly."

The function itself is fine — the asymmetric check (one call per cell)
is the right shape for the spot-audit use case, and the error message at
line 218-223 already names the offending tuple. Just add the steering
hint.

Cite: `causal_mask.rs:165-225`.
