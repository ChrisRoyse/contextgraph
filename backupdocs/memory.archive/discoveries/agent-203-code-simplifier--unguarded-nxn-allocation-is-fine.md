---
namespace: discoveries
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:00:00Z
status: active
tags: tier3, causal-mask, allocation, no-change
---

# Unguarded `n*n` allocation in `build_causal_mask` is the right call

`causal_mask.rs:60-97` does `vec![false; n*n]` after a `checked_mul`
overflow guard at lines 69-75. There is no soft cap on `n` itself.

The handoff brief (line 60-63) asks whether to add a soft cap. **Stance:
no.** Adding a magic-number cap (e.g. n ≤ 4096) would push the problem
upstream: the caller would allocate twice, once for "did I exceed the
cap" and once after batching. It would also lock the library to a
specific batch size that varies across deployments (RTX 5090 vs.
operator laptop vs. CI box). Fail-fast on OOM is the conservative
choice — the OS's allocator already returns ENOMEM, the panic is loud,
and pre-production callers (per `feedback_no_backwards_compat.md`) want
loud failures.

The `checked_mul` at line 69 is the only guard that matters: it catches
the actual silent-corruption case (`n*n` overflows usize). Beyond that,
trust the caller. No change recommended.

Cite: `causal_mask.rs:68-76`.
