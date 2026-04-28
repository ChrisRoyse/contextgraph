---
namespace: blockers
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T02:10:00Z
status: active
severity: medium
tags: blocker, fallback, day14, exp-2, self-run
---

# BLOCKER — Day-14 Silence Fallback Trigger

## Severity: MEDIUM (preventive)
If the Day-0 through Day-14 outreach sequence (Dohmatob, Khattab, Feng, Kempe, Longpre, Delétang — see ranking §4) produces **zero technical endorsement**, proceeding with Track A (MGX) or Track C (Qatalyst) without a researcher co-sign risks burning the sovereign/banker channel with an under-validated paper.

## Trigger condition
At Day 14 past first Dohmatob send, count replies in the set:
`{Dohmatob, Khattab, Feng, Kempe, Longpre, Delétang, Perez, Olah, Villalobos, Gal}`
If count = 0 AND no positive X/Twitter engagement on paper thread:

## Mandatory halt actions
1. **Pause all commercial outreach** (MGX, Qatalyst, LionTree, Centerview, Meta, Anthropic formal).
2. **Do not book FII9 Riyadh Oct 2026 / LEAP 2026 Riyadh Aug-Sep / Milken MEA Dec / LEAP East Jul pitches** until researcher endorsement lands. (FII Miami Mar 2026 already passed.)
3. **Do not submit Qatalyst cold email** — quality of warm-channel is binary at $3B+ level.

## Self-runnable fallback (EXP-2)
Chris owns the Context Graph substrate (~370K LOC Rust, 13-embedder panel) — EXP-2 is self-runnable.

**EXP-2 specification** (paper §9 future work):
- Inputs: 100–1,000 representative documents from Context Graph's memory store.
- Embedders: all 13 panel members (E1–E13).
- Estimator: MINE (Mutual Information Neural Estimation) or partitioned KSG-adjacent.
- Output: pairwise I(ϕ_j(X); ϕ_k(X)) for j<k, binned into redundancy histogram.
- Deliverable: replaces "up to 91 signals" with a measured Neff ∈ [N, N+C(N,2)].

**Wall-clock estimate**: 1-2 weeks of dedicated compute + analysis.

**Outcome**: paper v2 with Neff measured, bootstrap CI, and a figure showing the redundancy structure of the 13-embedder panel. This transforms the claim from "upper bound" to "measured capacity" — materially upgrading the paper's credential stock.

## Cold-restart after EXP-2
With EXP-2 in hand:
- Resend Dohmatob/Khattab/Feng with the strengthened paper.
- Higher reply probability because the falsification instrument has been run and the claim is now quantified.
- Track A (MGX) becomes viable: sovereigns weight measured capacity much higher than "upper bound".

## Resolution
Mark `status: resolved` only when either:
- (a) A researcher has explicitly endorsed the scope argument, OR
- (b) EXP-2 has been run and paper v2 is published.

Do not proceed to commercial channels before one of these two conditions.
