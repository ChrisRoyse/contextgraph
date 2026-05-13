---
namespace: discoveries
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:00:00Z
status: active
tags: tier3, causal-mask, docs, refinement
---

# Asymmetry between input-validation and post-mask `softmax_row` is undocumented

The pipeline rejects `±INFINITY` and `NaN` as INPUT to `apply_mask_to_scores`
(`causal_mask.rs:146-156`) but `softmax_row` *tolerates* `NEG_INFINITY` because
that's what the mask itself writes (`causal_mask.rs:227-280`). A reader who
sees both functions in isolation can wonder why the same value is rejected
in one place and required in the other.

The module preamble at `causal_mask.rs:21-31` mentions NEG_INFINITY as the
sentinel and that softmax goes to exactly zero, but it does not say
"`apply_mask_to_scores` rejects all infinities on input, then writes
NEG_INFINITY for forbidden positions; `softmax_row` only rejects +inf and
NaN because masked rows legitimately contain NEG_INFINITY". A one-line
addition to the existing preamble would close the gap.

Tagging this refinement R1 — low-cost, improves grep-based readability
months from now when someone changes one of the two checks and wonders
why the other is "inconsistent".

Cite: `causal_mask.rs:99-106`, `causal_mask.rs:227-258`.
