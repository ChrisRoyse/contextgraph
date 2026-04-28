---
namespace: discoveries
agent_id: agent-28-analysis-planner
timestamp: 2026-04-16T17:10:00Z
status: active
tags: negative-scope, what-we-dont-run, no-hypothesis-tests, no-power-calcs, defensive-disclosure
depends_on:
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/patterns/agent-28-analysis-planner--reporting-conventions.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/discoveries/agent-27-instrument-developer--instrument-limitations.md
downstream_consumers:
  - agent-29-validity-guardian
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# What We Deliberately Do NOT Analyse (Negative-Scope Register)

## 0. Purpose

This file enumerates, **by explicit name**, every statistical
procedure, metric, claim, or analysis move that we **refuse** to make
in the manuscript. Agent-42 (adversarial reviewer) uses it to
pre-empt reviewer challenges. Agent-40 (limitations writer) converts
these items into §8 disclosures where appropriate.

The reason every item here is excluded is the same underlying fact:
**our three case studies are exploratory demonstrations on
single-subject, single-pipeline, or single-session designs**. Any
inferential statistic on these designs either assumes a population
we do not claim to sample from, or requires an N we do not have.
Running the procedure anyway would be statistical theatre, not
science.

---

## 1. Hypothesis tests — ALL excluded

### 1.1 Tests we explicitly do NOT run

- **Wilcoxon signed-rank** on DNSMOS P.808 clone vs real (N=10 vs
  N=14; difference-of-means on small paired samples with bounded
  outcomes is non-informative).
- **Paired t-test** on UTMOS clone vs real (same small-N concern;
  plus UTMOS variance structure at ~3.0 is heteroscedastic by
  unclear amount).
- **Mann-Whitney U** on per-modality cosine distributions across
  ablations (no population, no independence structure to support).
- **χ² / Fisher's exact** on conjunction accept-rates across
  ablations (discrete N is too small to be informative; Fisher
  exact is defined but uninterpretable at N=10).
- **Kolmogorov-Smirnov** on SECS distribution vs any reference
  (no canonical reference distribution exists for argmax-selected
  candidate SECS).
- **Permutation tests** across any ablation arm (same small-N
  concern; permutation tests exaggerate Type-I when Ns are
  asymmetric and the statistic is a bounded cosine).

### 1.2 Why not (paste-ready rationale)

> *"We do not run hypothesis tests on Cases 1–3. Case 1 is a
> single-pipeline demonstration on one canonical corpus; Case 2
> is a single-subject single-session demonstration; Case 3 has N=10
> held-out sentences from one subject in one session on one
> microphone. Running a test would require a defined null hypothesis
> over a population we do not claim to sample from, and would produce
> a p-value whose interpretation is either misleading (as a
> generalisation claim we are not making) or trivial (as a check on
> a statistic already described descriptively). We report descriptive
> statistics and ablation deltas instead."*

### 1.3 What we do instead

- **Parity claims**: "identical to 3 sig-fig across held-out set"
  (not "failed to reject H₀ of equality").
- **Ablation claims**: "Δ = +0.094 (Full ICL > x-vector)" (not "t(18)
  = 3.4, p < 0.01").
- **Direction claims**: "direction: mouth-weighted > uniform on
  per-modality cosine" (not "one-sided p < 0.05").

---

## 2. Power analysis — NOT run

### 2.1 What we do NOT compute

- Prospective power curves for any case.
- Minimum-detectable-effect at target power 0.80.
- Retrospective power on the N=10 Case-3 measurement.
- Sample-size justification via Cohen's d or benchmark effect.

### 2.2 Why not (paste-ready from agent-26 §A.*.6)

> *"We do not run a formal power analysis for any of Cases 1–3. The
> headline measurements are bounded cosines ([−1, +1]) and MOS
> scalars ([1, 5]) without canonical reference effect sizes in the
> literature. Our case studies are exploratory demonstrations
> (N = 1 subject for Cases 2 and 3; single-pipeline for Case 1);
> a power analysis would require a target effect size and a population
> neither of which is defined in our design. We report observed
> descriptive statistics and note the sample sizes explicitly
> (N = 10 Case 3 sentences; N ≥ 20 planned for Cases 1 and 2
> held-out sets)."*

### 2.3 What we do instead

- **Informal power floor** (non-inferential): agent-26 §A.*.6 cites
  that N ≥ 20 gives approximate $\pm 0.05$ descriptive precision on
  a mean-cosine around 0.9 with $\sigma \approx 0.1$. This is a
  rule-of-thumb for planning, **not** reported as a power number.
- **Explicit N reporting**: every reported mean / median / CI
  cites its N.

---

## 3. Multiple-comparison correction — NOT run

### 3.1 What we do NOT apply

- Bonferroni FWER over 9 per-embedder claims in Case 1.
- Benjamini-Hochberg FDR over the same.
- Holm-Šidák step-down over ablation deltas.

### 3.2 Why not

Because we **do not make per-embedder significance claims** in the
first place. Case 1's primary metric is the **conjunction accept-
rate** (single binary per-prompt), and per-embedder means are
**descriptive** not **inferential**. A descriptive mean is not a
hypothesis, so no correction is needed.

### 3.3 What we do instead

Report the conjunction metric as primary. Report per-embedder means
as descriptive rows in Table 6.1. If any writer slips a per-embedder
significance claim into the draft, agent-42 catches and rejects.

---

## 4. Inferential population claims — excluded

### 4.1 Claims we do NOT make

- "TCT works for voice cloning in general" (single subject; no
  cross-subject evidence).
- "TCT works for English" (single subject, single dialect; no
  cross-subject or cross-dialect evidence).
- "WavLM SECS of 0.961 is typical for TTS+ICL in production"
  (single-session, author-observed; not a population estimate).
- "Mean SECS in the 0.95–0.99 range is the 'identity-locked' regime"
  (same-session-band is the author's observation per V12, not a
  benchmark).
- "TCT reduces sample complexity from N to 1" (framing, not
  measurement; supported by case demonstrations, not inferential
  statistics).

### 4.2 What we say instead (paste-ready)

> *"On this subject, under this protocol, with this pipeline version,
> mean WavLM SECS on N = 10 held-out sentences is 0.961."*

---

## 5. ROC / AUC / precision-recall — NOT run

### 5.1 What we do NOT compute

- ROC curve for Case 1's STYLE_CENTROID as a two-class
  "Shakespearean / not" classifier.
- AUC for any of the guards.
- Precision-recall curve for Case 3's WavLM guard at varying $\tau$.

### 5.2 Why not

- Case 1: no ground-truth "Shakespearean / not" labels on held-out
  output beyond the reference-centroid cosine itself — the ROC would
  be circular.
- Case 3: the guard threshold $\tau = 0.95$ is calibrated from the
  same-session band (agent-25 TCT §3.3 10th percentile); reporting
  a ROC would require a held-out adversarial pool (non-author voices)
  which is not part of our evaluation protocol.

### 5.3 Future work

Cross-subject ROC with adversarial speaker pool is named as
**future work** in §8 (post-L-01 in priority ordering).

---

## 6. Bayesian / credibility-interval analysis — NOT run

### 6.1 What we do NOT compute

- Bayesian posterior on mean SECS given flat prior.
- 95% highest-posterior-density credible interval.
- Bayes factors for ablation comparisons.

### 6.2 Why not

Bayesian methods are defensible in this exploratory setting but
would require specifying a prior, which would be either
(i) uninformative (in which case the posterior is close to the
frequentist bootstrap CI we already report for Case 3) or
(ii) informative (which on N=10 would dominate the posterior and
essentially report the prior). Neither adds information.

### 6.3 What we do instead

Retrospective bootstrap CI on Case 3 (explicitly flagged as
exception to N ≥ 20 rule).

---

## 7. Meta-analysis across the three cases — excluded

### 7.1 What we do NOT compute

- Pooled effect size across Cases 1/2/3.
- Forest plot of "TCT effect" across cases.
- Meta-regression on modality count $M$ against accept-rate.
- Counting "3/3 cases successful" as if it supported a
  generalisation claim.

### 7.2 Why not

- Different metrics per case (per-embedder cosine vs per-modality
  conjunction vs WavLM SECS); pooling is invalid.
- Different encoders, different populations, different guards.
- N=3 demonstrations is not a meta-analysable sample.

### 7.3 What we do instead

Qualitative cross-case synthesis in §7: each case demonstrates a
different TCT mode (inference-only, frozen-with-LoRA, multi-modal
frozen-with-LoRA). The claim is **existence + mechanism + scope
disclaimer**, not effect-size generalisation.

---

## 8. Subjective (human-rater) evaluation — NOT run for v1

### 8.1 What we do NOT run

- 3-rater Likert on Shakespearean-ness (Case 1).
- Blinded preference rating on Case-2 visual naturalness.
- MOS crowdsource on Case-3 naturalness.
- Turing-style "is this the author?" pairing.

### 8.2 Why not

Blinded rater recruitment + IRB-equivalent process cannot close by
submission (agent-27 handoff item 5; agent-28 confirms). Naturalness
parity is covered by DNSMOS + UTMOS (Case 3 MEASURED); identity-lock
is covered by WavLM + planned ECAPA (Case 3).

### 8.3 Future work

Named as future work in §8; not a limitation that invalidates the
case studies.

---

## 9. Out-of-distribution / adversarial robustness — NOT tested

### 9.1 What we do NOT test

- Adversarial voice-clone detection (Case 3 audio vs spoofing
  detectors).
- Out-of-distribution prompts for Case 1 (e.g., mathematical
  equations in Shakespearean style).
- Lighting / pose OOD for Case 2.
- Deepfake-detector robustness on Case 2 output.

### 9.2 Why not

Out of scope for the methodology contribution. This manuscript
contributes TCT as a methodology; robustness against deliberate
adversaries is a distinct contribution that requires a distinct
evaluation infrastructure.

### 9.3 Future work

Named in §8 as a cluster of future-work directions (§7.5
"adversarial robustness" subsection if space permits).

---

## 10. Summary table — what we report vs what we refuse

| Analysis move | Status | Cases applicable | Why |
|---|---|---|---|
| Descriptive stats (mean, median, IQR, min, max, std) | **REPORTED** | 1, 2, 3 | Appropriate for exploratory demos |
| Ablation deltas (signed, direction-labelled) | **REPORTED** | 1, 2, 3 | Appropriate for comparing pipeline arms |
| Retrospective bootstrap CI | **REPORTED (Case 3 exception, N=10)** | 3 | Explicitly flagged as not a power-based interval |
| Wilson-score CI on conjunction accept-rate | **REPORTED (Case 2 if N ≥ 20)** | 2 | Single-proportion CI, small-N robust |
| Pearson / Spearman correlation | **REPORTED (cross-encoder agreement)** | 3 | Descriptive; no significance claim |
| Bland-Altman plot | **REPORTED (cross-encoder difference)** | 3 | Descriptive visualisation, not a test |
| Hypothesis tests (t-test, Wilcoxon, Mann-Whitney, χ², KS, permutation) | **NOT RUN** | — | Population undefined; N too small |
| p-values | **ZERO** | — | Follows from no-hypothesis-tests |
| Power analysis | **NOT RUN** | — | Exploratory N=1 design |
| Multiple-comparison correction | **NOT APPLIED** | — | No per-embedder significance claims |
| ROC / AUC / PR | **NOT RUN** | — | Circular (Case 1) or requires adversarial pool (Case 3) |
| Bayesian posterior / credible interval | **NOT RUN** | — | Adds no information at N=10 |
| Cross-case meta-analysis | **NOT RUN** | — | Invalid pooling across different metrics |
| Subjective human ratings | **NOT RUN for v1** | — | IRB + recruitment infeasible by submission |
| OOD / adversarial robustness | **NOT TESTED** | — | Out of scope; future work |

---

## 11. The single defensive paragraph for §8

Paste-ready for agent-40 limitations writer:

> *"This manuscript reports descriptive statistics and ablation
> deltas only. We run no hypothesis tests, no p-values, and no
> power analyses on any of our three case studies. Cases 1 and 2
> are single-pipeline and single-subject demonstrations; Case 3
> measures N = 10 held-out sentences from one subject in one session
> on one microphone. Inferential statistics on these designs would
> require a population and null hypothesis we do not claim to
> sample from. A retrospective 95% bootstrap confidence interval is
> reported for Case 3's mean WavLM SECS (N = 10, 10,000 resamples);
> this is not a power-based interval, and we flag it as such. All
> other confidence intervals are reported only for subsamples with
> N ≥ 20 (Wilson-score on the Case 2 conjunction accept-rate, if
> realised). Cross-case pooling, meta-analysis, subjective rating,
> and out-of-distribution / adversarial robustness tests are named
> as future work."*

---

## 12. Confidence

**HIGH** that every entry above is correctly excluded given our
design. Any reviewer asking for any of these procedures can be
redirected to this register and to the §8 paragraph.

**HIGH** that the "what we don't do" is a feature, not a bug — it
prevents statistical theatre on N=1 demonstrations, which is a
more common failure mode in adjacent literature than the converse.

**HIGH** on the single defensive paragraph (§11) as the last line
of defence if agent-42 or an external reviewer tries to push a
hypothesis test into §6.

## Changelog

- 2026-04-16 — Initial issue by agent-28-analysis-planner.
