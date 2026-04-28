---
namespace: reports
agent_id: agent-11-systematic-reviewer
timestamp: 2026-04-16T22:15:00Z
status: active
tags: smap, final-report, prisma-lite, evidence-grading, bias-assessment
---

# Agent-11 Systematic Reviewer — SMAP Final Report

## S — Summary

Agent-11 conducted a **PRISMA-lite** systematic synthesis of the LOCKED
88-citation master reference pool produced by agents 07 + 08 (with
methodological scaffolding from agent-09 and theoretical framework from
agent-10). Outputs:

1. **PRISMA-lite flow diagram** with I1–I5 inclusion / E1–E6 exclusion
   criteria explicit: 87 identified → 3 duplicates removed → 85 eligible + 3
   alignment additions − 2 INELIGIBLE markers = **88 included**. Tier
   distribution: T1 = 25, T2 = 36, T3 = 27 (full pool 69% T1+T2; core A–G + I
   set 85% T1+T2 — passes QG1 G8 by 5 points).

2. **Evidence-grading table** of 32 major manuscript claims (CL-01 …
   CL-32) graded STRONG / MODERATE / WEAK / SINGLE-SOURCE / AUTHOR-EMPIRICAL
   with primary + supporting citations and §§ placement. Distribution:
   - **STRONG** (8): CL-01, CL-04, CL-07, CL-12, CL-14, CL-15, CL-19, CL-28.
   - **MODERATE** (9): CL-02, CL-05, CL-06, CL-09, CL-10, CL-11, CL-20, CL-25,
     CL-31.
   - **WEAK framing-only** (2): CL-03, CL-30 (T3 industry signals).
   - **SINGLE-SOURCE** (6): CL-08, CL-22, CL-23, CL-24, CL-29, CL-32.
   - **AUTHOR-EMPIRICAL** (7): CL-13, CL-16, CL-17, CL-18, CL-21, CL-26, CL-27.

3. **Per-cluster narrative synthesis** (Clusters A–I; 2–3 sentences each),
   paste-ready for §2.

4. **Per-cluster bias assessment** along 7 axes (language, lab concentration,
   geographic, venue, temporal, publication bias, methodology). Top flags:
   - Cluster E is Microsoft-dominant (~50%).
   - Cluster F is Alibaba / Chinese-labs dominant (~85%) and arXiv-heavy.
   - Cluster I is a 3-paper, US-only, alignment-lab monoculture.
   - Full pool is English-only.
   - Cluster A T3 signals are a press cross-reference loop, not independent
     replications.

No new citations introduced; master pool remains LOCKED.

## M — Methodology

### Sources consulted

- `memory/context/mission-brief.md`
- `memory/decisions/agent-07-source-tier-classifier--tier-assignments.md`
- `memory/decisions/agent-08-citation-extractor--master-reference-pool.md`
- `memory/discoveries/agent-09-methodology-scanner--methodology-scan.md`
- `memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md`
- `memory/handoff/agent-10-to-11--frameworks-ready.md`

### Process

1. **PRISMA-lite criteria** designed to match the scope of an arXiv empirical
   paper (§2 Related Work section of ~3,000 words, not a standalone systematic
   review). Inclusion criteria privileged T1/T2 peer-reviewed work and T3
   entries with no Tier-1 equivalent (industry signals, specs, reference works,
   tool releases). Exclusion criteria flagged pure marketing blogs and
   unverified preprints.
2. **Flow diagram** reconstructed from agent-06 (634-identified → 87-after-
   dedup-review → 85-tier-assigned → 88-after-alignment-addition) with actual
   numbers traceable to the master pool.
3. **Evidence grading** walked every agent-03 research question (Q01 … Q20),
   extracted the claims carried in §§ 1–9 from the ReWOO plan + agent-08
   Q-coverage index, and rated each claim against the count + tier + inter-
   source agreement of its citations. Grading followed the rule:
   - STRONG = ≥ 3 independent T1 + methodological agreement
   - MODERATE = 2 T1 or 1 T1 + ≥ 2 T2 with agreement
   - WEAK = T3-only or T2-only with disagreement
   - SINGLE-SOURCE = 1 load-bearing source (corroborative sources do not
     establish claim)
   - AUTHOR-EMPIRICAL = measured by Chris Royse in Cases 1 / 2 / 3
4. **Per-cluster synthesis** produced by compressing agent-09 methodology and
   agent-10 framework analyses into 2–3 sentence paragraphs usable as §2
   paragraph seeds.
5. **Bias audit** along 7 axes observable from citation metadata:
   affiliation, geography, venue, temporal distribution, positive-result
   dominance, methodological monoculture.

### Scope constraints respected

- No new citations added. Agent-10 flagged two possible expansions (Snell 2017
  Prototypical Networks; formal runtime-verification reference); both deferred
  to agent-12 per locked-pool rule.
- Qwen3-TTS model card NOT added to pool; left as INELIGIBLE with
  recommendation for T3 disclosure in §6.3 Methods only.
- Demucs v1/v2/v3 remained INELIGIBLE; HTDemucs (G05) is sufficient coverage.

## A — Artefacts

### Files written

1. `./memory/discoveries/agent-11-systematic-reviewer--prisma-flow.md`
   (PRISMA-lite inclusion/exclusion + flow diagram + screening decisions).
2. `./memory/patterns/agent-11-systematic-reviewer--evidence-grading.md`
   (32-claim GRADE table with STRONG/MODERATE/WEAK/SINGLE-SOURCE/
   AUTHOR-EMPIRICAL).
3. `./memory/patterns/agent-11-systematic-reviewer--cluster-synthesis.md`
   (A–I narrative synthesis, paste-ready for §2).
4. `./memory/discoveries/agent-11-systematic-reviewer--cluster-biases.md`
   (7-axis bias assessment per cluster; §8 Limitations pre-draft).
5. `./memory/handoff/agent-11-to-12--systematic-review-done.md` (handoff to
   context-tier-manager).
6. `./memory/reports/agent-11-systematic-reviewer--final-report.md` (this
   file).

### Key numeric outputs

- **88 citations** in LOCKED master pool (as received from agent-08; not
  changed by agent-11).
- **32 manuscript claims** mapped to citations and graded.
- **68 core citations** (Clusters A–G + I); 85% T1+T2 — passes QG1 G8.
- **20 sidebar citations** (Cluster H); body quota ≤ 6, T1/T2 preferred.

## P — Pointers / pending work

### Downstream agents

- **Agent-12 context-tier-manager**: carry forward the pool and the bias /
  quota flags; next in pipeline.
- **Agent-36 related-work-writer**: narrative synthesis file is paste-ready;
  balance Microsoft / Alibaba / US-only concentrations when composing §2.5
  and §2.6.
- **Agent-40 limitations-writer**: bias-file Recommendations section is a
  pre-draft of three §8 Limitations paragraphs.
- **Agent-42 adversarial-reviewer**: enforce (a) STRONG claims cite ≥ 3
  primary sources; (b) SINGLE-SOURCE claims use attribution phrasing;
  (c) AUTHOR-EMPIRICAL claims carry V12/V17/V22 hedges within 2 sentences;
  (d) Cluster-H body-citations ≤ 6 with T1/T2 preferred; (e) no citation
  outside master pool.

### Open items

1. **Qwen3-TTS disclosure** in §6.3 Methods — T3 GitHub model card; proposed
   key `qwen2024qwen3tts` if needed.
2. **Prototypical Networks (Snell 2017)** — possible §5.4 TCT contrast table
   addition; flagged by agent-10, deferred to agent-12.
3. **Runtime-verification formal-methods anchor** — possible §5.3 / §7 VERIF-
   IMPORT positioning; flagged by agent-10, agent-11 recommends narrative
   analogy without addition.
4. **Non-English source scope** — full pool English-only (L-Bias-1). §8
   must disclose.
5. **Cluster-H press cross-reference inflation** — treat as one signal per
   deal, not N-source consensus. §8 must disclose.

## Confidence

**HIGH.** The PRISMA-lite outputs are fully traceable to prior agents'
LOCKED files; the grade table's 32 claims cover all §§ 1–9 load-bearing
content per agent-08's Q-to-citation index; the bias audit is observable from
citation metadata without domain-specific judgement.

## Changelog

- 2026-04-16 — Initial issue by agent-11-systematic-reviewer. SMAP final.
