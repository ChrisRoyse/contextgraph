---
namespace: handoff
agent_id: agent-28-analysis-planner
timestamp: 2026-04-16T17:15:00Z
status: active
tags: handoff, agent-29, validity-guardian, analysis-ready
---

# Handoff: Agent-28 (analysis-planner) → Agent-29 (validity-guardian)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-28-analysis-planner--analysis-plan.md` —
   LOCKED per-case analysis plan. Contains the analysis design for
   Case 1 (Shakespeare; PROSPECTIVE), Case 2 (Santa; PROSPECTIVE),
   Case 3 (voice clone; partially MEASURED), plus cross-case §7
   synthesis rules and the reproducibility check-plan for agent-43.
2. `./memory/patterns/agent-28-analysis-planner--reporting-conventions.md` —
   paste-ready number-reporting conventions (3 sig-figs, tier tags,
   measurement dates, CI rules, encoder-naming discipline, anti-
   patterns for agent-42 to catch).
3. `./memory/discoveries/agent-28-analysis-planner--what-we-do-not-analyse.md` —
   negative-scope register: every hypothesis test / power calc /
   meta-analysis we refuse to run, with paste-ready rationale.
4. `./memory/decisions/agent-27-instrument-developer--instrument-specifications.md` —
   upstream instrument specs.
5. `./memory/decisions/agent-26-sampling-strategist--sampling-protocols.md` —
   upstream sampling (exploratory N=1 framing).
6. `./memory/decisions/agent-25-method-designer--case-study-protocols.md` —
   upstream measurement protocols (PENDING flag discipline).

## What agent-28 did (summary)

- **Locked the analysis plan** for all three case studies, prospectively
  (before Cases 1 and 2 T1 data is collected) so nothing can be
  retro-fitted to the numbers.
- **Locked global conventions**: 3 sig-figs on every scalar; tier
  tags T1-MEASURED / T1-PENDING / T3-PROJECTED on every number;
  measurement date + pipeline version mandatory on T1-MEASURED;
  CI only for N ≥ 20 with Case-3 N=10 bootstrap as a
  documented-verbatim exception; encoder naming on every SECS/MOS.
- **Prohibited** hypothesis tests, p-values, power analyses,
  multiplicity correction, ROC/AUC, Bayesian posteriors, cross-case
  meta-analysis, subjective Likert, and OOD/adversarial tests.
  Paste-ready rationale provided for every exclusion.
- **Designed the Case-3 ablation table**: full ICL vs x-vector
  (+0.094), target-vs-mismatched encoder (−0.013), temperature
  sweep (T=0.3 optimum), 1-clip vs 50-clip centroid (+0.015) —
  already MEASURED, directly paste-into-§6.3-ready.
- **Designed the Case-1 ablation**: vocabulary-density alone vs
  vocabulary-density + STYLE_CENTROID guard (Layer-2 vs Layer-2+3);
  plus LoRA-only vs full-stack (Layer-1 only vs Layers-1+2+3).
- **Designed the Case-2 ablation**: constellation guard ON vs OFF;
  mouth-weighted loss ON vs OFF; flow-matching sign correct vs
  inverted (historical from forensic trail).
- **Designed the W1/W2/W3 remediation analysis** for Case 3:
  cross-encoder agreement (Pearson + Spearman + Bland-Altman);
  pre-selection distribution histogram with Freedman-Diaconis bin
  rule; 4-point $n_m$ curve.
- **Wrote the retrospective bootstrap CI specification** for Case 3:
  N=10, 10,000 resamples with replacement, percentile method, seed
  20260416, verbatim "retrospective 95% bootstrap CI … not a
  power-based interval" phrasing.
- **Specified the reproducibility check-plan** for agent-43:
  manifest completeness, hash validation, threshold lock,
  per-output trace length, frozen-encoder audit, tier-flag
  completeness, bootstrap reproducibility.
- **Resolved agent-27 open carries**: Case-2 emotion encoder
  recommended `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition`
  (or "encoder TBD" given end-to-end PENDING); Case-2 voice encoder
  recommended `pyannote/embedding` or Resemblyzer (avoid WavLM
  collapse); Case-3 W2/W3 runs specified as post-submission-runs or
  v2; Human-Likert confirmed out of scope for v1.

## What agent-29 (validity-guardian) must check

Agent-28 has designed **what analyses run and how numbers are
reported**. Agent-29's role is to **audit validity threats** against
the analysis plan:

### 1. Internal-validity audit

- **Selection threat on Case 3 10-sentence held-out**: 10 sentences
  authored with zero lexical overlap to ICL transcript (agent-26 §A.3.5);
  agent-29 confirms no prompt-selection-for-quality bias.
- **Instrumentation threat on Case 3**: WavLM is both guard and
  evaluator (L-SECS-A); this is **measurement-measurement dependence**,
  not instrumentation drift. W1 cross-encoder remediation targeted
  at this — agent-29 confirms the mitigation path is documented.
- **Historical-comparison threat on Case 2 flow-matching sign
  ablation**: pre-fix baseline is cited from forensic trail, not
  re-run — agent-29 confirms the table labels this as "historical
  reference" not "fresh run".
- **Carry-over threat on Case 3 temperature sweep**: single-subject,
  same-ICL, different T — agent-29 confirms this is
  independence-of-candidates claim at inference time; no training
  carry-over.

### 2. External-validity audit

- **Population validity (Bias-05)**: N=1 subject for Cases 2/3;
  agent-29 confirms the scope-of-validity statements in §6 openers
  and §8 paragraphs prohibit generalisation.
- **Ecological validity**: single-session, single-microphone,
  single-language; agent-29 confirms §8 L-04 (Case 3 session
  structure) and Bias-09 (benchmark-monoculture) carry.
- **Temporal validity**: measured 2026-04-16 pipeline version
  `qwen3-tts-foricl-v1`; vendor-versioned scorers (Qwen3-TTS,
  WavLM, ECAPA) may change; agent-29 confirms weight-hash recording
  (agent-27 §H JSON manifest).

### 3. Construct-validity audit

- **Same-session-band (V12)**: "0.95–0.99 is the identity-locked
  regime" is an author observation, not a benchmark — agent-29
  confirms V12 phrasing in §6.3 and §8 L-SECS-C.
- **Encoder-matched self-consistency (L-SECS-A)**: top-priority
  disclosure per agent-27; agent-29 confirms L-SECS-A appears
  verbatim in §6.3 **before** Bias-05/Bias-06.
- **Identity-locked construct**: agent-04's definition is
  geometric-proximity-to-centroid; agent-29 confirms no slippage
  into "perceptual identity" / "psychological identity" claims.
- **Category error prevention (FM-04)**: Qwen3-TTS internal speaker
  encoder is NOT cited as cross-encoder verification (L-Qwen3-B) —
  agent-29 confirms.

### 4. Statistical-conclusion validity audit

This is **agent-28's primary responsibility** and the section where
agent-29's review should be lightest — agent-28 has already:

- Excluded hypothesis tests (no Type-I error to control).
- Excluded power analyses (no Type-II error to control).
- Excluded multiplicity correction (no per-embedder significance
  claims).
- Limited CIs to N ≥ 20 with a single documented N=10 exception.
- Specified the bootstrap procedure (percentile, 10,000 resamples,
  fixed seed).

Agent-29 confirms:
- No statistical-conclusion-validity threats from Type-I/II error
  inflation (none of the designs are inferential).
- The retrospective bootstrap CI on Case 3 is correctly flagged as
  retrospective (not power-based).
- The descriptive-statistics framing is appropriate for the design.

### 5. Reproducibility validity audit

- **Threshold lock**: $\tau_m$ hashed at calibration, verified
  unchanged at evaluation — agent-28 §6.1 / §6.2 / §6.3 check-plan
  covers this.
- **Frozen-encoder audit**: TCT §2.3 hook must return
  `all_phi_requires_grad_false = True` — agent-28 §6.1 covers this.
- **Weight-hash recording**: agent-27 §H schema requires
  `embedder_version_hash` per encoder — agent-29 confirms
  completeness.

---

## Files agent-29 should produce

Per the validity-guardian role:
- `./memory/decisions/agent-29-validity-guardian--validity-register.md` —
  the validity threat register (internal / external / construct /
  statistical / reproducibility) with agent-28's analysis plan as
  the reference design.
- `./memory/patterns/agent-29-validity-guardian--mitigation-checklist.md` —
  per-threat mitigation checklist that writers (agent-37, 38, 40)
  can paste into methodology / results / limitations sections.
- `./memory/discoveries/agent-29-validity-guardian--uncloseable-threats.md` —
  threats that remain uncloseable under our design (e.g., N=1 subject
  population generalisation) and must be acknowledged rather than
  mitigated.
- `./memory/handoff/agent-29-to-30--validity-audited.md` — handoff
  to the next agent (ethics-reviewer per the 28→29→30 chain if that
  exists, or to the writer chain otherwise).

---

## Open carries agent-28 passes to agent-29

1. **Case-3 W1 run timing**: ECAPA + Resemblyzer instrument-ready;
   run PENDING. If lands pre-submission, §6.3 gains the cross-
   encoder agreement rows; if not, §8 L-01 paste-ready. Agent-29
   should confirm §8 carries both contingency paragraphs.
2. **Case-3 W2 run timing**: 120-candidate pre-selection distribution
   PENDING; same contingency as W1. Agent-29 confirms §8 L-02 paste-
   ready if deferred.
3. **Case-3 W3 run timing**: 4-point $n_m$ curve PENDING; same
   contingency. §8 L-03 paste-ready.
4. **Case-2 end-to-end T1 landing**: entire §6.2 table is T3
   PROJECTED if the end-to-end runs do not land. Agent-29 confirms
   the T3 PROJECTED placeholder framing (agent-28 §3.2) is valid.
5. **Case-1 end-to-end T1 landing**: same contingency as Case-2.
6. **Case-2 emotion / voice encoder IDs**: open carry; agent-28
   recommended placeholders but did not lock model IDs given
   end-to-end PENDING. Agent-37 to confirm final §6.2 language.
7. **Human-Likert**: confirmed out-of-scope for v1. Agent-29 verifies
   no §6 text sneaks in a Likert claim.

---

## Carried hedges (unchanged)

- V4 encoder-naming (every SECS / MOS mention names the encoder).
- V11, V12 same-session-band framing on Case 3.
- V18 frozen-encoder discipline.
- V22 projected-number tags.
- R3 / R4 data-coverage hedges.
- Bias-05 single-subject (Cases 2 / 3).
- Bias-06 single-encoder (Case 3).
- Bias-08 self-reporting.
- Bias-09 benchmark-monoculture.
- FM-04 measured / projected discipline.
- FM-05 single-subject opener on §6.3.
- MP-12 composition-not-invention.

---

## Dependencies agent-29 pulls forward

- Agent-28 analysis plan + reporting conventions + negative-scope
  register (this handoff).
- Agent-27 instrument specifications + limitations register + case
  map.
- Agent-26 sampling protocols + scope-of-validity + disjointness
  matrix.
- Agent-25 TCT protocol + case-study protocols + weakness
  remediation.
- Agent-24 weakness register.
- Agent-23 hypothesis ledger.
- Agent-14 bias audit.
- Agent-09 methodology scan + standard-metrics-per-cluster pattern.

---

## One-paragraph summary for agent-29

> Agent-28 has locked the analysis plan for all three case studies
> prospectively: Case 3 reports mean WavLM SECS 0.961 (retrospective
> 95% bootstrap CI [0.951, 0.971] on N=10, 10,000 resamples, seed
> 20260416, verbatim-flagged "not a power-based interval"), max
> 0.975, min 0.914, std 0.017, 9/10 ≥ 0.95; DNSMOS P.808 parity
> 3.93/3.93; UTMOS parity 3.012/2.997; with W1 (cross-encoder
> ECAPA/Resemblyzer agreement via Pearson/Spearman/Bland-Altman),
> W2 (120-candidate pre-selection histogram with Freedman-Diaconis
> bins + TCT-delta), and W3 (4-point $n_m$ curve at n ∈ {10, 25,
> 50, 100}) as instrument-ready runs PENDING with §8 L-01/L-02/L-03
> paste-ready contingencies. Cases 1 and 2 are PROSPECTIVE
> (T1 PENDING end-to-end); the plan specifies conjunction accept-rate
> as primary, per-embedder / per-modality means as descriptive, and
> ablation deltas (vocabulary vs. STYLE_CENTROID for Case 1;
> constellation guard ON/OFF, mouth-weighted ON/OFF, flow-matching
> sign-correct/inverted historical for Case 2). Global conventions
> are 3 sig-fig on every scalar, tier tags on every number
> (T1-MEASURED / T1-PENDING / T3-PROJECTED), measurement date +
> pipeline version mandatory on T1-MEASURED, CIs only for N ≥ 20
> with the single documented Case-3 N=10 exception, encoder naming
> on every SECS/MOS. We run NO hypothesis tests, NO p-values, NO
> power calculations, NO multiplicity corrections, NO ROC/AUC,
> NO Bayesian posteriors, NO cross-case meta-analysis, NO subjective
> Likert (out of scope for v1), NO OOD/adversarial robustness tests
> — with paste-ready rationale for every exclusion. Agent-29 now
> audits validity threats against this analysis plan.

---

## Quality-gate status delivered by agent-28

| Gate | Status |
|---|---|
| Per-case analysis plan | **PASS** |
| Global reporting conventions | **PASS** |
| Negative-scope register (what we don't run) | **PASS** |
| Tier-tag discipline (T1 MEASURED / T1 PENDING / T3 PROJECTED) | **PASS** |
| No-hypothesis-test discipline | **PASS** |
| No-power-calculation discipline | **PASS** |
| Case-3 bootstrap CI procedure | **PASS** |
| Case-3 W1/W2/W3 remediation analysis plan | **PASS** |
| Reproducibility check-plan for agent-43 | **PASS** |
| Writer contracts for agent-37 / 38 / 40 | **PASS** |
| Anti-pattern checklist for agent-42 | **PASS** |
| Agent-27 open carries resolved or passed-on | **PASS (item 5 confirmed out-of-scope; items 1–4 passed on with recommendation)** |

---

## Confidence

**HIGH** that agent-29 can audit validity from this handoff + the
three memory files without re-reading the upstream chain.

**HIGH** that the descriptive-stats + ablation-delta + retrospective-
bootstrap plan is the correct analysis discipline for our design
and will not need revision under adversarial review.

**HIGH** that the no-hypothesis-test, no-p-value, no-power-calc
discipline is defensible and paste-ready for §8 rebuttal.

**MEDIUM** on whether W1/W2/W3 (Case 3) and end-to-end (Cases 1/2)
runs land by submission. Plan degrades gracefully to v2 deferral
with paste-ready §8 disclosures.

## Changelog

- 2026-04-16 — Initial handoff by agent-28-analysis-planner.
