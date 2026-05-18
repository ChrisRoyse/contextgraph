---
namespace: context
agent_id: agent-201-coordinator
timestamp: 2026-05-08T23:55:00Z
status: active
tags: tier2, ruvector-findings, clean-room, mejepa, mincut, granger, certificates
---

# Tier-2 Foundation Gaps Initiative (2026-05-08)

## What this initiative is

Per `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier-2 has six
items totaling ~24-31 days. This initiative ships the three most contained,
publication-quality items in the current turn and queues the three larger
items as separate next-turn work.

Tier-1 is shipped (see `tier1-foundation-gaps-initiative.md`) — generic MMR,
split conformal predictor, and ed25519 segment signing.

## Scope split for Tier-2

### THIS TURN (3 items, fully implemented + FSV)

1. **§2.3 Stage 0 + Stage F + Merkle certificates** in `context-graph-witness`.
   Builds on the already-shipped witness-chain core. Adds:
   - `MerkleTree` over `(id, content_sha)` pairs with proof-of-membership.
   - `Stage0Certificate` for ME-JEPA corpus legal-gate per `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §5`.
   - `ReadoutCertificate` for read-out head emissions per `04 §5a`.

2. **§2.4 Stoer-Wagner mincut** in new crate `crates/context-graph-mincut/`.
   Per `docs/ruvectorfindings/03 §6` and `04 §6`: global minimum cut for
   ccreality A/B splits + ME-JEPA train/test leakage check.

3. **§2.6 Granger causality test** in `context-graph-solver` as a new module.
   Per `docs/ruvectorfindings/03 §9`, `04 §8b`: bivariate Granger F-test
   for "did skill X Granger-cause subsequent uplift Y?" attribution.

### DEFERRED TO NEXT TURNS (3 items)

4. **§2.1 SONA-style adaptive learner** (`crates/context-graph-sona/`).
   μLoRA + EWC++ + REINFORCE trajectory + ReasoningBank + 3 learning loops
   + safetensors export. Multi-thousand LOC across at least 6 files.
   Estimated 5-7 days clean-room. **NOT in this turn.**

5. **§2.2 Verified-training per-step certificates** (`crates/context-graph-verified-training/`).
   `TrainingCertificate` per gradient step with formal/statistical invariants,
   delta-apply pattern, fail-closed-by-default. Estimated 5-6 days.
   **NOT in this turn.**

6. **§2.5 Matryoshka funnel + Graph RAG (Leiden)**. Coarse-to-fine cascade
   search + Leiden community detection on the cross-task graph. Estimated
   5-6 days. **NOT in this turn.**

## Why this split

The deferred items each require either (a) ML training pipeline integration
(SONA needs autograd-style gradient flow over real model weights;
verified-training needs gradient-step interception) or (b) algorithmic depth
that resists scope-cutting (Leiden community detection is a real iterative
optimizer).

The three items shipped this turn are mathematically self-contained (Stoer-
Wagner is a single textbook algorithm; Granger is OLS + F-test; certificates
are SHAKE-256 + tree hashing). They can be implemented to publication quality
with full synthetic-data FSV in one focused turn without violating the
"no workarounds, no fallbacks" rule.

## Subagent coordination

Implementation work is coordinator-direct (per
`/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/feedback_no_delegate_research_audit.md`).
The `code-simplifier` subagent reviews all three modules at end-of-turn.

## Cross-references

- Tier-1: `memory/context/tier1-foundation-gaps-initiative.md`
- Tier-1 final report: `memory/reports/agent-200-coordinator--tier1-foundation-gaps-final-report.md`
- ME-JEPA-Code master plan: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
- Tier-2 detail: `docs/ruvectorfindings/03_REALITY_LOOP_APPLICATIONS.md §6, §9`,
  `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §5, §6, §8b, §9`,
  `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier-2 table.
