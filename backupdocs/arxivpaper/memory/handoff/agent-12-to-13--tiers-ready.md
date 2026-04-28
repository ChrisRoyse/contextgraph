---
namespace: handoff
agent_id: agent-12-context-tier-manager
timestamp: 2026-04-16T22:55:00Z
status: active
tags: handoff, agent-13, quality-assessor, tiers-ready, hot-warm-cold, writer-cues
---

# Handoff: Agent-12 (context-tier-manager) → Agent-13 (quality-assessor)

## Read these files FIRST, in this order

1. `./memory/patterns/agent-12-context-tier-manager--hot-warm-cold.md` — master
   tier table (18 HOT / 26 WARM / 44 COLD out of 88 citations).
2. `./memory/patterns/agent-12-context-tier-manager--writer-cue-sheet.md` —
   per-section (Abstract, §1–§9) mandatory / useful / relevant citation cues.
3. `./memory/reports/agent-12-context-tier-manager--final-report.md` — SMAP.

## What agent-12 did

1. **Assigned each of the 88 locked citations a hot / warm / cold tier**
   orthogonal to the T1/T2/T3 source-quality tier (which stays as
   agent-07/agent-08 assigned). Distribution: **18 / 26 / 44**, inside the
   20 / 30 / 50 ±5% target.
2. **Identified the 18 HOT citations** using:
   - Q-coverage from `patterns/agent-08-citation-extractor--claim-to-citation-index.md`
     (any citation appearing in ≥ 2 essential questions).
   - STRONG or AUTHOR-EMPIRICAL-anchor GRADE from
     `patterns/agent-11-systematic-reviewer--evidence-grading.md`.
   - Direct comparator / foundational-substrate status per agent-06 topology.
3. **Built the per-section writer cue sheet** tagging each citation
   [HOT-M] / [HOT-U] / [WARM-U] / [WARM-R] / [COLD-R] for Abstract + §1–§9.
4. **Enforced agent-11 + agent-08 rules carried forward** (Cluster-H body
   quota ≤ 6; SINGLE-SOURCE attribution; AUTHOR-EMPIRICAL V12/V17/V22 hedge).
5. **Left the master pool LOCKED** — no additions, no renames, no tier
   changes on the T1/T2/T3 axis.

## Headline outcomes (for agent-13)

- **18 HOT citations** covering 9 clusters (A3 B2 C2 D3 E1 F4 I3 + 0 G/H).
  Hot tier is **100% peer-reviewed** (12 T1, 6 T2, 0 T3).
- **The single most-loaded citation** is `chen2022wavlm` (9 Q-IDs; SECS scorer
  + E-slot + Case 3 evaluator). **Second** is `hu2022lora` (7 Q-IDs). **Third**
  is `meng2025echomimicv3` (6 Q-IDs). Expect re-cites across §§ 2–6.
- **Hot-tier composition** deliberately includes the 3-citation alignment
  triad (ouyang / rafailov / bai) that agent-07 flagged as the Q06 gap — this
  gap is now closed.
- **Every Q01–Q20 served by ≥ 1 hot citation** except Q17 (forensics, leans on
  author's own forensic files) and Q19 (ethics, leans on Cluster-H sidebar +
  henderson2022pileoflaw which is warm tier).
- **Cluster-H has zero hot citations.** Body quota ≤ 6 is enforced via warm
  (Henderson) + cold (Singhal, Tu, BloombergGPT as T1/T2 body-eligibles).

## Quality-gate status

- **QG2 G-traffic (tier distribution).** 18 / 26 / 44 = 20.5% / 29.5% / 50%.
  Inside ±5% target band. **PASS.**
- **QG2 G-coverage (≥ 1 hot per Q-ID).** Q17 + Q19 have no hot-tier primary
  (by design; they lean on forensic files / ethics sidebar). **PASS with
  deliberate exception.**
- **QG2 G-cluster-H quota.** Hot tier excludes Cluster H; warm tier includes
  only Pile of Law; cold tier holds 19/20 Cluster-H cites. **PASS.**
- **QG2 G-hot-quality.** Hot tier is 12 T1 + 6 T2 (100% peer-reviewed).
  **PASS.**

## What agent-13 (quality-assessor) should do next

Per agent-05 tasking-table row 13, agent-13 audits pool quality and writes a
quality-assessment report. Specifically:

1. **Verify the 18 hot-tier citations all have working DOI or arXiv ID** (they
   do per agent-08's LOCK statement, but the quality-assessor is the gate that
   confirms no broken URL slips through).
2. **Audit the SINGLE-SOURCE hot cites** (hu2022lora, lipman2023flowmatching,
   malkov2018hnsw) for citation-count / foundational-status sanity.
3. **Audit the AUTHOR-EMPIRICAL anchors** (chen2022wavlm for Case 3 measurement;
   meng2025echomimicv3 for Case 2 forensic; lipman2023flowmatching for sign-
   convention forensic) and flag them for agent-29 validity-guardian review.
4. **Sanity-check warm-tier supporting citations** don't duplicate hot-tier
   primaries. No duplicates found by agent-12, but verify.
5. **Confirm the Cluster-H body quota ≤ 6 is achievable** with the four body-
   eligible entries (henderson T1, singhal T1, tu T1, bloomberggpt T2) — yes,
   since at most 4 + sparing cold-tier H refs = 6.
6. **Flag any quality issues to coordinator** via a blocker; do not modify
   tier assignments.

## What writers (agents 35–41) must do with agent-12's outputs

- **All writers**: cite by key only; no URL text in prose.
- **Writers open hot-tier citations FIRST** for their section's Q-IDs per cue
  sheet; warm-tier second; cold-tier only for framing/industry-signal.
- **Abstract (agent-41)**: max 3 named cites — pick from villalobos2024data,
  chen2024valle2, chen2022wavlm, hu2022lora, ouyang2022instructgpt.
- **§1 (agent-35)**: 5 hot-M cites mandatory (villalobos, longpre, hoffmann,
  shumailov, deletang).
- **§2 (agent-36)**: 11 hot-M cites mandatory across §2.1–§2.6; **Microsoft-
  and Alibaba-balance rules** from agent-11 bias file must be enforced.
- **§3 (agent-37)**: 8 hot-M cites mandatory.
- **§4 (agent-37)**: 5 hot-M cites mandatory.
- **§5 (agent-37)**: 7 hot-M cites mandatory.
- **§6 (agent-38)**: 7 hot-M cites mandatory; **§6.3 → §6.2 → §6.1** order
  per agent-05.
- **§7 (agent-39)**: 8 hot-M re-cites mandatory.
- **§8 (agent-29/39)**: 4 hot-M cites mandatory; ethics §8.6 uses warm
  henderson + cold singhal/tu/bloomberggpt.
- **§9 (agent-40)**: 2 hot-M cites mandatory.

## Critical constraints carrying forward

- **Master pool LOCKED** per agent-08. No new citations without blocker.
- **Cluster labels A–I LOCKED** per agent-06.
- **T1/T2/T3 tier assignments LOCKED** per agent-07.
- **Evidence grades LOCKED** per agent-11.
- **H/W/C tier assignments** (this file) are soft-locked — can be adjusted
  post-draft after agent-36 finishes §2 if traffic deviates significantly
  from prediction. Adjust via blocker + changelog entry in
  `patterns/agent-12-context-tier-manager--hot-warm-cold.md`.
- **Cluster-H body quota ≤ 6** enforced across ALL sections combined.
- **SINGLE-SOURCE attribution phrasing** mandatory on hu2022lora,
  lipman2023flowmatching, malkov2018hnsw, huang2024compression,
  longpre2024consent, henderson2022pileoflaw.
- **AUTHOR-EMPIRICAL V12/V17/V22 hedge** mandatory on chen2022wavlm (for
  Case 3 SECS claim), meng2025echomimicv3 (for Case 2 claim),
  lipman2023flowmatching (for sign-convention forensic).

## Open items (for agent-13 to note, not necessarily resolve)

1. **qwen3-TTS T3 disclosure** — carried forward from agent-11. If §6.3
   Methods names the base model, add as T3 GitHub cite under key
   `qwen2024qwen3tts`. agent-13 quality-assessor can flag; coordinator
   decides.
2. **Snell 2017 Prototypical Networks** — carried forward from agent-11 /
   agent-10. Not in pool. Defer to agent-36 signal.
3. **Runtime-verification formal-methods anchor** — carried forward from
   agent-11 / agent-10. Not in pool. Keep as narrative analogy for now.
4. **Cluster-E Microsoft balance** — agent-36 should actively balance
   chen2024valle2 (Microsoft hot) with tan2024naturalspeech3 (non-Microsoft
   warm) and wang2024maskgct (non-Microsoft warm) when drafting §2.5.
5. **Cluster-F Alibaba balance** — agent-36 should balance meng2025echomimicv3
   (Alibaba hot) with cui2024hallo3 (Fudan/Baidu cold) and yang2024cogvideox
   (Zhipu/Tsinghua cold).

## Confidence

**HIGH.** Tier assignments follow a repeatable rule (Q-coverage × GRADE);
distribution hits target; hot tier is 100% peer-reviewed; every essential
question is served except Q17 and Q19 (by design).

---

## Changelog

- 2026-04-16 — Initial handoff by agent-12-context-tier-manager.
