---
namespace: context
agent_id: agent-202-coordinator
timestamp: 2026-05-09T01:00:00Z
status: active
tags: tier3, ruvector-findings, causal-masking, root-cause-fix
---

# Tier-3 Foundation Gaps Initiative (2026-05-09)

## What this initiative is

Per `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier-3 has six items
totaling ~40+ days. Tier-1 and Tier-2 are shipped (see their respective
initiative docs in this directory).

This turn ships **one Tier-3 primitive** plus a **root-cause fix** for the
recurring `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` hook errors that have been
appearing on every PostToolUse:Edit/Write throughout this session. The
remaining five Tier-3 items are deferred with explicit next-turn handoff.

## Scope split

### THIS TURN

1. **Root-cause fix: stale debug MCP binary**. The hook errors
   `CCREALITY_HARNESS_TRANSITION_RECORD_FAILED` ↪ `CCREALITY_WITNESS_OP_TYPE_UNKNOWN`
   are caused by `crates/context-graph-cli/src/commands/ccreality/mod.rs::mcp_binary`
   defaulting to `target/debug/context-graph-mcp` regardless of staleness. The
   shell wrapper `scripts/cc/_common.sh::cgreality_context_graph_cli_bin`
   already prefers release; the CLI's internal MCP-binary resolver does not.
   Apply `.mcp.json` as the default command source of truth with no implicit
   debug fallback; debug/local runs must set `CGREALITY_MCP_BIN` explicitly or
   intentionally edit `.mcp.json`. Rebuild both and verify chain readback
   against the selected binary.

2. **§3.2 Causal masking + retrocausal safety** (clean-room). Per
   `docs/ruvectorfindings/04 §8a, §8d`. New module
   `crates/context-graph-solver/src/causal_mask.rs`. `MaskStrategy` enum
   (Strict, TimeWindow, Causal-with-self), retrocausal-safety guards that
   reject scores attempting to attend to future events, application to
   attention score matrices. Synthetic-data FSV demonstrating zeroing of
   forbidden positions and rejection of retrocausal attention.

### DEFERRED (next turns; documented for handoff)

3. **§3.1 Cross-pack skill transfer** — depends on SONA crate (Tier-2 deferred).
   Build SONA first; transfer engine after.

4. **§3.3 Coherence-driven SONA tuning loop** — depends on SONA + sheaf
   coherence (also deferred). Lowest-priority Tier-3.

5. **§3.4 FastGRNN-style intent router** — bounded ~600 LOC + calibration
   dataset; quality-of-life rather than load-bearing per `03 §7`.

6. **§3.5 prime-radiant sheaf-Laplacian coherence** — XL (10-12 days);
   sheaf cohomology math is non-trivial. Defer until simpler anti-pole-set
   baseline is exhausted.

7. **§3.6 Manifold geometry / Riemannian** — XL (10+ days); Riemannian ops
   on product manifolds. Defer until cross-instrument transfer is needed.

## Why this split

§3.2 is mathematically self-contained (linear-algebra mask matrices over
score arrays) and directly composes with the existing solver primitives.
The remaining items either depend on Tier-2 deferred work (§3.1, §3.3) or
cross the "research-grade math" threshold that resists scope-cutting
(§3.5, §3.6) per the same rationale that deferred SONA + verified-training
in Tier-2.

## Subagent coordination

Implementation is coordinator-direct (per
`feedback_no_delegate_research_audit`). The `code-simplifier` subagent
reviews the new module + the root-cause fix at end-of-turn.

## Cross-references

- Tier-1: `memory/context/tier1-foundation-gaps-initiative.md` + final report.
- Tier-2: `memory/context/tier2-foundation-gaps-initiative.md` + final report.
- ME-JEPA-Code master plan: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`.
- Tier-3 detail: `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §8` (causal masking).
