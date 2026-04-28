---
namespace: reports
agent_id: agent-12-context-tier-manager
timestamp: 2026-04-16T23:00:00Z
status: active
tags: smap, final-report, context-tiers, hot-warm-cold, writer-cues
---

# Agent-12 Context Tier Manager — SMAP Final Report

## S — Summary

Agent-12 organised the LOCKED 88-citation master reference pool (produced by
agents 07 + 08, graded by agent-11) into **hot / warm / cold traffic tiers**
for downstream writing agents 35–41. Outputs:

1. **Master tier table** — 18 HOT / 26 WARM / 44 COLD citations with per-entry
   justification, Q-coverage, cluster, source-tier (T1/T2/T3), GRADE, and
   "why this tier" reason. Distribution 20.5% / 29.5% / 50%, inside the
   20 / 30 / 50 ±5% target. Hot tier is 100% peer-reviewed (12 T1 + 6 T2 +
   0 T3).
2. **Writer cue sheet** — per-section (Abstract, §1 through §9) tagged
   citation cues [HOT-M] / [HOT-U] / [WARM-U] / [WARM-R] / [COLD-R], with
   mandatory / useful / relevant distinction and per-section word + citation
   targets. Total body-unique citations estimated 40–55, inside the mission-
   brief 30–60 reference band.
3. **Handoff to agent-13 quality-assessor** carrying forward all LOCKED rules
   (master pool, cluster labels, T1/T2/T3 tiers, evidence grades, Cluster-H
   quota ≤ 6, SINGLE-SOURCE attribution, AUTHOR-EMPIRICAL hedges).

## M — Method

1. **Read full context** in the order specified by task brief: mission brief,
   topic topology (agent-06), master reference pool + claim-to-citation index
   (agent-08), evidence grading (agent-11), systematic-review handoff
   (agent-11→12). Also read section-ownership (agent-05) for word budgets.
2. **Defined tier criteria** orthogonal to T1/T2/T3 source-quality tier:
   - HOT = Q-coverage ≥ 2 **OR** anchors a STRONG/AUTHOR-EMPIRICAL claim in
     the paper's load-bearing spine (three paths + DDA + TCT + Case 3).
   - WARM = Corroborates a hot-tier primary, fills a methodological slot, or
     provides a direct comparator that one STRONG claim needs.
   - COLD = Background / framing / industry-signal / sidebar citation; cap
     on traffic.
3. **Scored each of the 88 citations** on (Q-coverage count) × (GRADE weight:
   STRONG=2, AUTHOR-EMPIRICAL anchor=2, MODERATE=1.5, SINGLE-SOURCE=1.5,
   WEAK=0.5) and sorted into tiers with the 20/30/50 target as soft
   constraint.
4. **Enforced structural constraints** from upstream agents: Cluster-H body
   quota ≤ 6 (so H gets zero hot, one warm [Pile of Law], the rest cold);
   the Q06 alignment triad (ouyang / rafailov / bai) must be hot because
   agent-07 identified it as a gap; foundational SINGLE-SOURCE papers (LoRA,
   Flow Matching, HNSW) hot because the paper is unwritable without them.
5. **Built per-section cue sheet** mapping the 18 hot citations + 26 warm
   citations into the 10 section slots (Abstract + §1–§9), flagging
   mandatory / useful / relevant, and counting per-section totals against
   agent-05's word budgets.
6. **Wrote the handoff** to agent-13 carrying forward LOCKED constraints +
   open items (Qwen3-TTS, Snell 2017, runtime-verification anchor, bias
   balance rules for §2.5).

## A — Artefacts

Files written by agent-12:

| Path | Purpose |
|---|---|
| `memory/patterns/agent-12-context-tier-manager--hot-warm-cold.md` | Master tier table (18H/26W/44C). |
| `memory/patterns/agent-12-context-tier-manager--writer-cue-sheet.md` | Per-section citation cues (Abstract, §1–§9). |
| `memory/handoff/agent-12-to-13--tiers-ready.md` | Handoff to agent-13 quality-assessor. |
| `memory/reports/agent-12-context-tier-manager--final-report.md` | This SMAP report. |

## P — Problems / Open items

1. **Two Q-IDs not served by hot-tier primaries by design.** Q17 (forensic
   failures) leans on the author's own forensic files (Shakespeare echo-bug,
   Santa flow-matching sign-bug) rather than published literature. Q19
   (ethics) is scoped to Cluster-H sidebar and `henderson2022pileoflaw` (warm
   tier). This is structurally consistent with agent-08's claim-to-citation
   index and agent-07's Cluster-H body quota; flagged for agent-13
   quality-assessor review.
2. **Three open items inherited from agent-11**: Qwen3-TTS T3 disclosure,
   Snell 2017 Prototypical Networks (not in pool), runtime-verification
   formal-methods anchor (not in pool). None require action from agent-12;
   all three are deferred to writer-agent signals.
3. **Tier assignments are soft-locked**, not hard-locked. If agent-36
   Related Work finds that one currently-cold citation gets cited in ≥ 2
   sections, it should be promoted to warm via a blocker + changelog entry
   in the hot/warm/cold file. Similarly, any currently-hot citation not
   referenced in any final draft demotes to warm.
4. **Bias balance rules** inherited from agent-11's bias file require
   agent-36 to actively balance Cluster-E Microsoft dominance (3/4 warm
   citations in §2.5 are Microsoft) and Cluster-F Alibaba dominance
   (1 Alibaba hot + 1 Alibaba cold + 2 other-lab cold). This is a §2.5
   drafting constraint, not a tier-file constraint.

## Headline numbers

- **Total citations in pool**: 88 (LOCKED per agent-08).
- **HOT tier**: 18 citations (20.5%). 12 T1 + 6 T2 + 0 T3 = 100% peer-reviewed.
- **WARM tier**: 26 citations (29.5%). 9 T1 + 16 T2 + 1 T3 = 96% peer-reviewed.
- **COLD tier**: 44 citations (50.0%). 4 T1 + 15 T2 + 25 T3 (T3-heavy by
  design — press, gov, software refs).
- **Most-loaded citation**: `chen2022wavlm` at 9 Q-IDs (Q04, Q08, Q09, Q10,
  Q12, Q14, Q16, Q18, Q20). SECS scorer + embedder substrate + Case 3 anchor.
- **Cluster-H in body**: 0 hot + 1 warm (Pile of Law) + up to 5 more from
  cold (Singhal, Tu, BloombergGPT + 2 optional industry signals) = max 6.
  Quota satisfied.
- **Microsoft-in-hot**: 1 (`chen2024valle2`). Microsoft-in-warm: 3
  (`wang2023valle`, `tan2024naturalspeech3` co-author, `shen2023naturalspeech2`).
  §2.5 balance: achievable via `wang2024maskgct` (non-Microsoft ICLR 2025) +
  `chen2024f5tts` (non-Microsoft).
- **Alibaba-in-hot**: 1 (`meng2025echomimicv3`). Alibaba-in-cold: 3
  (`tian2024emo`, `tian2025emo2`, `wan2025video`). §2.5 balance: achievable
  via `yang2024cogvideox` (Tsinghua) + `cui2024hallo3` (Fudan/Baidu).

## Confidence

**HIGH.** Every tier assignment traces to a repeatable rule (Q-coverage ×
GRADE); the distribution hits the 20/30/50 ±5% target; hot tier is
100% peer-reviewed; every Q-ID is served by either a hot-tier primary or a
by-design exception (forensics, Cluster-H sidebar). Downstream writers now
have a single file telling them which citations to open first per section.

## Changelog

- 2026-04-16 — Initial issue by agent-12-context-tier-manager.
