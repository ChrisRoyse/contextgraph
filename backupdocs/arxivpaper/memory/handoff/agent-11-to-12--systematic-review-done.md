---
namespace: handoff
agent_id: agent-11-systematic-reviewer
timestamp: 2026-04-16T22:00:00Z
status: active
tags: handoff, agent-12, context-tier-manager, prisma-lite-done, evidence-grading-done
---

# Handoff: Agent-11 (systematic-reviewer) → Agent-12 (context-tier-manager)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-11-systematic-reviewer--prisma-flow.md` —
   Inclusion / exclusion criteria; PRISMA-lite flow diagram; 87 identified → 3
   dedup removed → 3 alignment added → 2 INELIGIBLE → **88 included**.
2. `./memory/patterns/agent-11-systematic-reviewer--evidence-grading.md` —
   32 claims (CL-01 … CL-32) graded STRONG / MODERATE / WEAK / SINGLE-SOURCE /
   AUTHOR-EMPIRICAL, each with primary + supporting citations and §§ placement.
3. `./memory/patterns/agent-11-systematic-reviewer--cluster-synthesis.md` —
   Per-cluster (A–I) 2–3 sentence narrative synthesis, paste-ready for §2.
4. `./memory/discoveries/agent-11-systematic-reviewer--cluster-biases.md` —
   7-axis bias audit per cluster. Top flags: Microsoft-dominated Cluster E,
   Alibaba-dominated Cluster F, US-only 3-paper Cluster I, English-only pool.
5. `./memory/reports/agent-11-systematic-reviewer--final-report.md` — SMAP.

## What agent-11 did

1. **Applied PRISMA-lite inclusion / exclusion criteria** (I1–I5, E1–E6) to
   the master reference pool. Documented in the flow-diagram file.
2. **Produced PRISMA flow diagram** with actual numbers from the 88-citation
   pool: 87 identified → 3 duplicates removed → 3 alignment added → 2
   INELIGIBLE excluded → **88 included**. Core (A–G + I) = 68 sources, 85%
   T1+T2. Sidebar (H) = 20 sources, 15% T1+T2 (quota-limited to ≤ 6 in body).
3. **Evidence-graded 32 claims** (CL-01 … CL-32) STRONG / MODERATE / WEAK /
   SINGLE-SOURCE / AUTHOR-EMPIRICAL against ≥ 3 citations each. Summary below.
4. **Narrative synthesis per cluster A–I** (2–3 sentences each), paste-ready
   for agent-36 Related Work §2.
5. **Bias assessment per cluster** along 7 axes (language, lab concentration,
   geographic, venue, temporal, publication bias, methodology). Top flags
   carried forward to §8 Limitations and §2 balancing rules.

## Headline outcomes (for agent-12)

- **Flow numbers:** 87 → 3 dedup → 85 → +3 alignment → 88 → −2 INELIGIBLE
  marker → 88 included (86 eligible, 2 pool-marked INELIGIBLE).
- **Grade distribution (32 claims):** STRONG 8, MODERATE 9, WEAK 2 (both
  framing-only), SINGLE-SOURCE 6, AUTHOR-EMPIRICAL 7.
- **STRONG claims:** scaling law (CL-01), model collapse (CL-04),
  prediction–compression duality (CL-07), embedders-as-rankers (CL-12),
  TCT vs. scalar-reward contrast (CL-14), zero-shot TTS frontier (CL-15),
  flow-matching + LoRA paradigm (CL-19), RLHF/DPO/CAI (CL-28).
- **SINGLE-SOURCE claims** (foundational — need attribution, no hedge
  beyond "as introduced by"): LoRA (CL-22), Flow Matching (CL-23), HNSW
  (CL-24), Huang compression-capability correlation (CL-08),
  Longpre accessible-fraction (CL-29), Henderson Pile of Law (CL-32).
- **Cluster bias flags** (§8 Limitations must note):
  - Cluster E ≈ 50% Microsoft authorship (wang2023valle, chen2024valle2,
    tan2024naturalspeech3, shen2023naturalspeech2).
  - Cluster F ≈ 85% Chinese labs / ≈ 33% Alibaba (meng2025echomimicv3,
    tian2024emo, tian2025emo2, wan2025video).
  - Cluster I = 3 papers, US-only, alignment-lab monoculture.
  - Full pool English-only.
  - Cluster A T3 corroboration is a press cross-reference loop (dated Nov
    2024), not independent replications.

## Quality-gate status

- **QG2 G8 (≥ 80% T1+T2 in body).** Core set (A–G + I) = 85% T1+T2. **PASS**
  as long as Cluster-H body quota ≤ 6 is respected with T1/T2 preferred.
- **QG2 G10 (systematic review / evidence grading).** 32 claims graded; primary
  citations named per claim; grading rules documented. **PASS.**
- **QG2 G14 (bias disclosure).** 7-axis bias audit per cluster; §8 placeholder
  text provided. **PASS.**

## What agent-12 (context-tier-manager) should do next

Per agent-05 tasking-table row 12, agent-12 maintains the tier context and
budgets for downstream writing. Specifically:

1. **Carry forward the 88-source pool with stable identifiers.** Keys defined
   in agent-08's master pool; no renaming.
2. **Enforce Cluster-H body quota ≤ 6.** Prefer T1/T2 (H05 Henderson, H13
   Singhal, H15 Tu, H20 BloombergGPT).
3. **Surface SINGLE-SOURCE claims to writers as hedge-required.** CL-08,
   CL-22, CL-23, CL-24, CL-29, CL-32 each need attribution-style citation
   (e.g., "as introduced by Hu et al. (2022)" for LoRA) rather than
   consensus-framing prose.
4. **Surface AUTHOR-EMPIRICAL claims as V12 / V17 / V22 hedges mandatory.**
   CL-13, CL-16, CL-17, CL-18, CL-21, CL-26, CL-27 all require the
   "observed in this study, not generalised" hedge.
5. **Propagate bias flags to §8 Limitations placeholder** for agent-40.
6. **No new citations.** Pool is LOCKED. Blocker required for any addition.

## What writers (agents 36–41) must do with agent-11's outputs

- **agent-36 Related Work.** Use the cluster-synthesis prose as §2 building
  blocks. Balance Cluster-E Microsoft dominance with non-Microsoft citations;
  balance Cluster-F Alibaba dominance with non-Alibaba citations. For every
  STRONG claim, cite ≥ 3 of the named primary sources.
- **agent-37 Methodology.** Use the evidence-grading table to calibrate
  §5 TCT formal spec against CL-14 (STRONG) and CL-13 (AUTHOR-EMPIRICAL).
- **agent-38 Results.** CL-16, CL-17, CL-18 are AUTHOR-EMPIRICAL — report with
  V12 / V17 / V22 hedges per §6.3.
- **agent-39 Discussion.** Use GRADE strata to organise §7: STRONG claims as
  consensus, SINGLE-SOURCE as attribution, AUTHOR-EMPIRICAL as "in our hands".
- **agent-40 Limitations.** Three Limitations paragraphs pre-drafted in the
  bias file: language-only, lab-concentration (Microsoft / Alibaba / US-only
  alignment), Cluster-H press cross-reference inflation.
- **agent-42 Adversarial Reviewer.** Enforce:
  - STRONG claim → ≥ 3 primary citations from pool.
  - SINGLE-SOURCE claim → attribution-style phrasing.
  - AUTHOR-EMPIRICAL claim → V12/V17/V22 hedge within 2 sentences.
  - Cluster-E cite without Microsoft-balance → flag.
  - Cluster-F cite without Alibaba-balance → flag.
  - Cluster-H body-count > 6 → flag.
  - Any citation outside master pool → flag as unauthorised addition.

## Critical constraints carrying forward

- **Master pool LOCKED.** No citation additions without blocker.
- **Cluster labels A–I LOCKED.** Do not rename.
- **Tier assignments LOCKED** per agent-07 + agent-08.
- **Evidence grades LOCKED** per agent-11.
- **Bias flags must appear in §8 Limitations** (agent-40 mandatory).
- **V12 / V17 / V22 hedges mandatory** on AUTHOR-EMPIRICAL claims.
- **Cluster-H body quota ≤ 6**, T1/T2 preferred.

## Open items (for agent-12 to resolve or defer)

1. **Qwen3-TTS disclosure in §6.3 Methods.** Per mission brief, Case 3 base
   model must be named. agent-11 recommends citing as T3 GitHub with proposed
   key `qwen2024qwen3tts`; agent-12 should confirm / update the pool, or
   file a blocker.
2. **Snell 2017 Prototypical Networks** — agent-10 flagged for glossary #3 /
   §5.4 TCT contrast table; not in pool. agent-11 did NOT add (would violate
   locked-pool rule). agent-12 can include in a blocker if §5.4 warrants.
3. **Runtime-verification (formal methods) anchor** — agent-10 flagged for
   §5.3 / §7 VERIF-IMPORT positioning. Not in pool. agent-11 recommends
   keeping as narrative analogy unless §7 needs a formal anchor.
4. **Prototypical Networks / Runtime-verification** — both deferred to
   agent-12 unless agent-36 signals a blocker.

## Confidence

**HIGH.** The 88-source PRISMA-lite pool is consistent with agent-07 / agent-08
/ agent-09 / agent-10 outputs. Grade distribution reflects the actual
citation economics of the field (STRONG claims on well-replicated topics;
SINGLE-SOURCE on foundational methods; AUTHOR-EMPIRICAL on three case studies).
Bias flags are observable from author affiliations + venue counts and are
actionable.

---

## Changelog

- 2026-04-16 — Initial handoff by agent-11-systematic-reviewer.
