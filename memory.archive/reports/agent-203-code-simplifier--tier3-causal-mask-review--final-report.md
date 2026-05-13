---
namespace: reports
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:05:00Z
status: active
tags: tier3, causal-mask, mcp-resolver, code-review, final
---

# Tier-3 review — causal mask + MCP resolver — final report

Read-only review per handoff at
`memory/handoff/agent-202-coordinator-to-code-simplifier--causal-mask-and-mcp-resolver-review.md`.
No source mutated.

## Verdict per file

| File | Verdict |
|------|---------|
| `crates/context-graph-solver/src/causal_mask.rs` | REFINEMENT-RECOMMENDED (R1, R2 — both doc-only) |
| `crates/context-graph-solver/src/causal_mask_tests.rs` | PASS |
| `crates/context-graph-solver/examples/causal_mask_fsv.rs` | PASS |
| `crates/context-graph-cli/src/commands/ccreality/mod.rs` (`mcp_binary`) | PASS (handoff doc is wrong; code is correct + more conservative than described) |
| `scripts/cc/_common.sh` (`cgreality_context_graph_cli_bin`) | PASS (no inconsistency — different binary, different env-var) |
| `crates/context-graph-witness/src/cert.rs` (path-attribute fix) | PASS |
| `crates/context-graph-solver/src/granger.rs` (path-attribute fix) | PASS |

## Refinements

### R1 — Document the input/output infinity asymmetry (LOW)

`causal_mask.rs:21-31` (module preamble). Add one line clarifying that
`apply_mask_to_scores` rejects all infinities on input, then writes
NEG_INFINITY to forbidden positions, and that `softmax_row` consequently
*tolerates* NEG_INFINITY but rejects +inf and NaN. Concrete:

```
// Note on infinities: `apply_mask_to_scores` rejects ALL non-finite
// scores on input (NaN or ±INF) and writes NEG_INFINITY to forbidden
// positions. Downstream `softmax_row` therefore tolerates NEG_INFINITY
// (the mask sentinel) but rejects +INF and NaN. The asymmetry is
// deliberate, not a bug.
```

Rationale: future readers seeing both validators will otherwise wonder
why one accepts a value the other rejects. Doc-only; no semantic change.

### R2 — Doc steer for retrocausal_safety_check bulk path (LOW)

`causal_mask.rs:165-178` (function doc). Add one sentence:

```
/// For per-cell auditing or sparse spot checks. For bulk masking,
/// prefer `apply_mask_to_scores` — it is O(N²) once instead of O(N²)
/// function calls and produces the masked matrix directly.
```

Rationale: the function is the right shape for spot checks; without
this hint a naive caller will reach for it in a loop.

### R3 — DEFER (naming consistency)

`CGREALITY_MCP_BIN` (new) vs `CGREALITY_CONTEXT_GRAPH_CLI` (existing in
bash). Different binaries, different envs — fine today. Pick a
convention before a third overridable bin lands. Not actionable in this
turn.

## What I confirmed (no change needed)

- **Unguarded n*n allocation in `build_causal_mask`**: the `checked_mul`
  guard at `causal_mask.rs:69-75` catches the only silent-corruption
  case. A soft cap on `n` would be a magic number that varies by
  hardware. Trust the caller, fail fast on OOM. Coordinator's stance is
  correct.
- **Two-pass validate-then-mutate in `apply_mask_to_scores`**: the FSV
  at `causal_mask_fsv.rs:73-78` and tests at
  `causal_mask_tests.rs:62-72` actively assert "no mutation on
  rejection" — keeping the two-pass shape preserves that contract
  visibly. Don't fuse.
- **MCP binary resolver**: code is `env > .mcp.json > hard error`. The
  handoff says `env > release > debug`, but the actual code is more
  conservative — there is no profile-inference layer that could pick
  stale debug bytes. The function comment at lines 552-562 is
  self-consistent. Coordinator should update the handoff doc, not the
  code.
- **Path-attribute fixes** (`cert.rs:28`, `granger.rs:484`): standard
  Rust pattern for non-mod.rs sibling-test files; both correct.
- **lib.rs re-exports**: `apply_mask_to_scores`, `build_causal_mask`,
  `retrocausal_safety_check`, `softmax_row`, `MaskStrategy` — all
  surfaced cleanly at `lib.rs:15-17`. No dead code.

## Counts

- PASS: 5 files
- REFINEMENT-RECOMMENDED: 1 file (causal_mask.rs, two doc-only Rs)
- BLOCKER: 0
- Total Rs: 2 actionable (R1, R2), 1 deferred (R3)

## Top-3 refinements by priority

1. **R1** — `causal_mask.rs:21-31` — preamble note on infinity asymmetry
   between `apply_mask_to_scores` input and `softmax_row` input.
2. **R2** — `causal_mask.rs:165-178` — `retrocausal_safety_check`
   doc-string adds bulk-path steer to `apply_mask_to_scores`.
3. **R3 (DEFER)** — naming convention for binary-override env vars
   (`CGREALITY_MCP_BIN` vs `CGREALITY_CONTEXT_GRAPH_CLI`); not worth a
   change today.

End of review.
