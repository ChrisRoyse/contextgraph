---
namespace: reports
agent_id: agent-26-sampling-strategist
timestamp: 2026-04-16T09:30:00Z
status: active
tags: final-report, SMAP, sampling-strategist, agent-26
---

# Final Report — Agent-26 Sampling Strategist

## S — Summary

Locked the sampling strategy the manuscript describes across §5
(TCT reference-set construction) and §6 (Case studies). Deliverables:

- **Locked decisions**: `decisions/agent-26-sampling-strategist--sampling-protocols.md` — Parts A (per-case) and B (TCT R_m), disjointness matrix (§D), eligibility gates (§E), bias-disclosure lines (§F).
- **Scope statements**: `discoveries/agent-26-sampling-strategist--scope-of-validity.md` — paste-ready what-each-case-supports and what-it-does-not paragraphs for Cases 1 / 2 / 3.
- **Handoff**: `handoff/agent-26-to-27--sampling-ready.md` — instrument-developer dependencies.
- **This report**: `reports/agent-26-sampling-strategist--final-report.md`.

**Headline sampling table**:

| Case | Training / corpus N | Reference $R_m$ | Held-out | Power framing |
|---|---|---|---|---|
| 1 Shakespeare | SFT 7,908 + DPO 12,567 + 230 style exemplars | 50–100 passages (E1) | 20–50 prompts (T1 PENDING) | Exploratory; CI $\pm 0.05$ at N = 20 |
| 2 Santa | 2,362 curated clips from 16 min 15 s single-subject interview | 3,146-sample × 4,044-dim NPZ (visual 1,725 / emotion 343 / speaker 300 / prosody 188 / sentiment 202 / semantic 200 / voice 188) | 20–100 scenes / prompts (T1 PENDING) | Exploratory; CI $\pm 0.14$ on conjunction at N = 50 |
| 3 Voice | 489-clip corpus (Demucs + silence split); 2 real-mic ICL refs (8–9 s each) | 50 top-WavLM-similar clips (centroid) | 10 English sentences, zero lexical overlap | Exploratory; bootstrap CI $0.961 \pm 0.012$ on N = 10 |

**Exploratory-N = 1 framing**: stated in §A.*.6 of the sampling
protocols, carried into §8 Limitations verbatim, blocking any
§7 sentence that would widen scope.

## M — Method

1. Read the five mandatory context files (mission brief,
   constraints, agent-25 TCT protocol, case-study protocols,
   weakness-remediation plan, agent-25 handoff, agent-01 FM-05,
   agent-14 Bias-05/06).
2. Pulled exact sample-count anchors from the prior manuscript:
   NPZ counts (`researchpaper/14_appendix_schemas.md §A.6, §A.8`);
   Case-1 cleaned-set N = 7,908 and DPO pair N = 12,567
   (`researchpaper/15_appendix_repro.md §65, §151`,
   `docs/FULL_SCALE_LORA_PLAN.md §82`); Case-3 489 corpus +
   10 held-out (`researchpaper/15_appendix_repro.md §153`);
   Case-2 2,362 clips and viseme oversampling 2–4×
   (`researchpaper/08_case_santa_lora.md`,
   `researchpaper/15_appendix_repro.md §33, §53–57`).
3. Resolved W4 session audit: cross-session for Case 3 generator
   input, same-session for Case 3 parity comparator; same-session
   structural feature for Case 2.
4. Wrote three paste-ready scope-of-validity paragraphs that embed
   Bias-05 / Bias-06 / Bias-08 / Bias-09 / FM-04 / FM-05 /
   V11 / V18 / V22 / R3 / R4 hedges without adding new vocabulary.
5. Wrote per-case eligibility gates and the sample-level / scene-
   level / session-level disjointness matrix so agent-27 can
   operationalise screening instruments and agent-43
   (reproducibility checker) can verify.

## A — Action

- Wrote `decisions/agent-26-sampling-strategist--sampling-protocols.md`
  (locked, ~900 lines): Parts A (three per-case sections), B (TCT R_m
  construction with Santa NPZ counts and Case-3 50-clip procedure),
  §D (disjointness matrix), §E (eligibility gates), §F (paste-ready
  bias-disclosure line), §G (confidence).
- Wrote `discoveries/agent-26-sampling-strategist--scope-of-validity.md`
  (locked, ~130 lines): three paste-ready paragraphs for §6.1 / §6.2
  / §6.3 openers and §8 Limitations anchors.
- Wrote `handoff/agent-26-to-27--sampling-ready.md`:
  instrument-developer dependencies (measurement instruments,
  screening-gate runners, provenance-manifest writers, bootstrap-CI
  computer, pre-selection distribution recorder, ECAPA-TDNN /
  Resemblyzer wrappers for W1).
- Closed W4 session audit for Case 3; documented Case 2 same-session
  structural feature.
- Carried every hedge from agents 01 / 02 / 14 / 23 / 24 / 25 without
  introducing new primitives.

## P — Problems / Pending

- **Realised held-out N for Cases 1 and 2 is engineering-capacity
  dependent** (MEDIUM): recommended floors are 20–50 (Case 1) and
  20–100 (Case 2); the realised N must be reported in §6, not
  asserted.
- **Cross-encoder run for W1 is instrument-ready but run-pending**
  (MEDIUM): ECAPA-TDNN + Resemblyzer instrumentation specified for
  agent-27; if the run does not land in time the §6.3 + §8 L-01
  disclosure language (per agent-25 `weakness-remediation.md §1.3`)
  stands.
- **Pre-selection distribution for W2 remediation is
  instrument-ready but run-pending** (MEDIUM): 12 × 10 = 120 SECS
  values per Case 3; if the run does not land, §8 L-02 language
  stands.
- **$n_m$ curve for W3 is instrument-ready but run-pending**
  (LOW–MEDIUM): nested top-$n$ WavLM-similar subsets at
  $n \in \{10, 25, 50, 100\}$; if the run does not land, §8 L-03
  language stands.
- **Blocker files**: none. No sampling failure to record.

## Confidence

**HIGH** that the sampling plan is defensible against an adversarial
review focused on population specification, disjointness,
session audit, and scope walls.

**HIGH** that Case 3's sampling pipeline (489 → top-50 WavLM-ranked
centroid + 2 real-mic ICL + 10 held-out sentences with zero lexical
overlap + same-session parity) is reproducible from this file alone.

**HIGH** that the exploratory-N = 1 framing in §A.*.6 prevents FM-05
(hidden single-subject) and FM-04 (measured / projected conflation)
at the §6 and §7 boundary.

**MEDIUM** on the realised held-out N for Cases 1 and 2 — engineering
capacity decides; the manuscript reports the realised number, not
the target floor.

## Files written

- `memory/decisions/agent-26-sampling-strategist--sampling-protocols.md`
- `memory/discoveries/agent-26-sampling-strategist--scope-of-validity.md`
- `memory/handoff/agent-26-to-27--sampling-ready.md`
- `memory/reports/agent-26-sampling-strategist--final-report.md`

## Changelog

- 2026-04-16 — Initial final report by agent-26-sampling-strategist.
