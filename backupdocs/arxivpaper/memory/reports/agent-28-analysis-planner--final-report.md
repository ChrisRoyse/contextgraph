---
namespace: reports
agent_id: agent-28-analysis-planner
timestamp: 2026-04-16T17:20:00Z
status: complete
tags: final-report, smap, analysis-plan, agent-28-summary
depends_on:
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/patterns/agent-28-analysis-planner--reporting-conventions.md
  - memory/discoveries/agent-28-analysis-planner--what-we-do-not-analyse.md
  - memory/handoff/agent-28-to-29--analysis-ready.md
---

# Agent-28 (Analysis Planner) — Final SMAP Report

## S — Situation

Agent-28 is **Agent 28 of 48** in the arXiv-manuscript pipeline. The
manuscript reports three empirical case studies of Teleological
Constellation Training (TCT):

- **Case 1 (Shakespeare stylistic LoRA)**: SFT+DPO training in
  progress; 15/15 prior FSV tests passed; all T1 numbers PENDING.
- **Case 2 (Santa talking-head clone)**: EchoMimicV3 + LoRA on
  2,362 clips from a 16 min single-subject interview; forensic
  integrity complete (flow-matching sign-bug fixed, q_audio excluded);
  all T1 numbers PENDING.
- **Case 3 (Voice clone)**: Qwen3-TTS + Full ICL + 50-clip centroid +
  WavLM-scored best-of-12 at T=0.3; N=10 held-out sentences;
  **MEASURED**: mean WavLM SECS 0.961, max 0.975, min 0.914, std
  0.017, 9/10 ≥ 0.95; DNSMOS P.808 3.93 vs 3.93 parity; UTMOS 3.012
  vs 2.997 parity. Measurement date 2026-04-16, pipeline
  `qwen3-tts-foricl-v1`.

Upstream, agent-25 locked the methodology (TCT Phase 1/2/3 protocol
+ per-case protocols), agent-26 locked the sampling (exploratory
N=1 framing; disjointness matrix; W4 Case-3 session audit
resolved), and agent-27 locked the measurement instruments (15
specifications + 32 limitation codes + JSON manifest schema for
agent-43).

Agent-28's mandate: **design the analysis plan that turns raw
instrument outputs into §6 results tables and §7 discussion claims,
prospectively (before Cases 1/2 T1 data collection) to prevent
retro-fitting.**

## M — Mission

1. Lock the per-case analysis plan (descriptive statistics +
   ablations + retrospective bootstrap CI for Case 3).
2. Lock the global reporting conventions (3 sig-figs, tier tags,
   measurement dates, encoder naming).
3. Enumerate the analyses we deliberately **do not run** (hypothesis
   tests, p-values, power calcs, meta-analysis) with paste-ready
   rationale.
4. Specify the reproducibility check-plan for agent-43.
5. Resolve or pass on the five open carries from agent-27.
6. Hand off to agent-29 (validity-guardian) with full context.

## A — Actions

Five memory artefacts produced, all with required frontmatter:

| # | File | Role |
|---|---|---|
| 1 | `memory/decisions/agent-28-analysis-planner--analysis-plan.md` | LOCKED per-case analysis design with global conventions, Case-3 W1/W2/W3 remediation plans, reproducibility check-plan. |
| 2 | `memory/patterns/agent-28-analysis-planner--reporting-conventions.md` | Paste-ready number-reporting conventions for writers (3 sig-figs, tier tags, date+pipeline citation, encoder naming, anti-pattern checklist). |
| 3 | `memory/discoveries/agent-28-analysis-planner--what-we-do-not-analyse.md` | Negative-scope register: 12 analysis moves we refuse to make, each with rationale + paste-ready §8 defensive paragraph. |
| 4 | `memory/handoff/agent-28-to-29--analysis-ready.md` | Handoff to validity-guardian with one-paragraph summary, dependency chain, open carries, quality-gate status. |
| 5 | `memory/reports/agent-28-analysis-planner--final-report.md` | This SMAP. |

### Key decisions locked

**Global conventions**:
- 3 sig-figs on every scalar (with UTMOS 3.012 as documented 4-sig-fig
  exception for SITREP parity).
- Tier tag T1-MEASURED / T1-PENDING / T3-PROJECTED on every number.
- T1-MEASURED numbers cite measurement date (ISO-8601) + pipeline
  version (SHA-7 / semver / named pipeline).
- CIs only for N ≥ 20, with the single documented exception of
  Case-3 N=10 retrospective bootstrap (flagged verbatim as "not a
  power-based interval").
- Encoder naming on every SECS/MOS mention (V4 hedge enforcement).
- No hidden cosine rescaling (raw [−1, +1] for WavLM / SigLIP).

**Case-3 bootstrap CI procedure**:
- N=10, 10,000 resamples with replacement, percentile method.
- Fixed seed 20260416.
- Expected CI approximately [0.951, 0.971] at 3 sig-fig.
- Verbatim phrase: "retrospective 95% bootstrap CI, N=10; not a
  power-based interval".

**Case-3 ablation table (MEASURED, directly paste-ready)**:

| Ablation | Δ | Direction |
|---|---|---|
| Full ICL vs x-vector-only (SpeechBrain) | +0.094 | Full ICL wins |
| Target encoder vs mismatched encoder for best-of-N | −0.013 | Target encoder wins |
| Temperature sweep 0.2–0.8 | T=0.3 optimum | — |
| 1-clip vs 50-clip centroid | +0.015 | 50-clip wins |

**Case-3 W1/W2/W3 remediation analysis (PENDING, instrument-ready)**:
- W1: Pearson + Spearman + Bland-Altman between WavLM vs ECAPA-TDNN
  (+ Resemblyzer) on same 10 sentences.
- W2: 120-candidate pre-selection SECS histogram (Freedman-Diaconis
  bin rule); pre-selection mean; TCT-delta.
- W3: 4-point $n_m$ curve at $n \in \{10, 25, 50, 100\}$.

**Case-1 analysis design (PROSPECTIVE)**:
- Primary metric: 9-embedder conjunction accept-rate.
- Descriptive: per-embedder mean / median / IQR / 10th-pct / min / max.
- Ablation 1: vocabulary-density alone vs vocabulary-density +
  STYLE_CENTROID.
- Ablation 2: LoRA-only (Layer-1) vs full 3-layer stack.
- Per-embedder CV as threshold-stability diagnostic.

**Case-2 analysis design (PROSPECTIVE)**:
- Primary metric: 4-modality conjunction accept-rate (visual 0.70,
  emotion 0.60, speaker 0.80, voice 0.85).
- Descriptive: per-modality mean / median / IQR / std per modality.
- Wilson-score 95% CI on conjunction if $N_{\text{eval},2} \geq 20$.
- Ablation 1: constellation guard ON vs OFF.
- Ablation 2: mouth-weighted loss ON vs OFF.
- Ablation 3: flow-matching sign correct vs inverted (historical
  reference from forensic trail, labelled as such).

**Negative-scope (what we do NOT analyse)**:
- Zero hypothesis tests (Wilcoxon, t-test, Mann-Whitney, χ², KS,
  permutation).
- Zero p-values.
- Zero power analyses (prospective or retrospective).
- No multiplicity correction (no per-embedder significance claims).
- No ROC/AUC (circular for Case 1; requires adversarial pool for
  Case 3).
- No Bayesian posteriors / credible intervals.
- No cross-case meta-analysis.
- No subjective Likert (IRB infeasible for v1).
- No OOD / adversarial robustness tests.

**Agent-27 open carries resolved**:
1. Case-2 emotion encoder: recommended
   `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition`, or
   "encoder TBD" given end-to-end PENDING.
2. Case-2 voice encoder: recommended `pyannote/embedding` or
   Resemblyzer (avoid collapsing conjunction with WavLM).
3. Case-3 W2 pre-selection run: post-submission run-plan specified.
4. Case-3 W3 $n_m$-curve run: same contingency; v2 deferral
   paste-ready.
5. Human-Likert: **confirmed out of scope for arXiv v1**.

## P — Plan (downstream)

1. **Agent-29 (validity-guardian)**: audits internal / external /
   construct / statistical-conclusion / reproducibility validity
   against agent-28's analysis plan. Writes validity register,
   mitigation checklist, uncloseable-threats register, handoff to
   next agent. Agent-28 has pre-cleared statistical-conclusion
   validity (no Type-I/II error to control).
2. **Agent-37 (methodology-writer)**: §5 Methods uses agent-27's
   instrument specs + agent-28's analysis plan + reporting
   conventions.
3. **Agent-38 (results-writer)**: §6 Results uses agent-28's
   table/figure skeletons with tier flags + encoder naming + 3
   sig-fig + no-p-value discipline.
4. **Agent-40 (limitations-writer)**: §8 uses agent-27's 32
   limitation codes + agent-28's negative-scope §11 defensive
   paragraph + agent-26's Bias-05/06/08/09 paragraphs + agent-25's
   PENDING-flag discipline.
5. **Agent-42 (adversarial-reviewer)**: reviewer anti-pattern
   checklist from agent-28's reporting conventions §13 catches
   violations. Negative-scope register §11 provides the single
   defensive paragraph if a reviewer demands a hypothesis test.
6. **Agent-43 (reproducibility-checker)**: runs the check-plan from
   agent-28 §6.1 against agent-27's JSON manifest schema §H.

---

## Artifacts summary

**Five files written:**
1. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-28-analysis-planner--analysis-plan.md`
2. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-28-analysis-planner--reporting-conventions.md`
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/discoveries/agent-28-analysis-planner--what-we-do-not-analyse.md`
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-28-to-29--analysis-ready.md`
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-28-analysis-planner--final-report.md`

---

## Confidence

**HIGH** on analysis plan completeness, discipline, and defensibility
against adversarial review.

**HIGH** on the no-hypothesis-test / no-p-value framing — this is
the correct discipline for exploratory N=1 case-study reporting and
any alternative would be statistical theatre.

**HIGH** on the Case-3 retrospective bootstrap CI being defensibly
flagged as "not a power-based interval".

**MEDIUM** on whether Cases 1, 2, and Case-3 W1/W2/W3 runs land by
submission. Plan degrades gracefully to v2 deferral with paste-ready
§8 L-01/L-02/L-03-equivalent disclosures.

## Changelog

- 2026-04-16 — Initial SMAP by agent-28-analysis-planner. Task complete.
