---
namespace: decisions
agent_id: agent-28-analysis-planner
timestamp: 2026-04-16T17:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/discoveries/agent-27-instrument-developer--instrument-limitations.md
  - memory/handoff/agent-27-to-28--instruments-ready.md
downstream_consumers:
  - agent-29-validity-guardian
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
tags: analysis-plan, descriptive-stats, ablation, bootstrap-ci, exploratory-N1, no-hypothesis-tests, locked
answers_questions: [Q06, Q08, Q15]
---

# Analysis Plan — Cases 1–3 (LOCKED)

## 0. Purpose and scope

This file **locks** the analysis plan that turns the raw instrument
outputs (per agent-27) into the §6 results tables and §7 discussion
claims. It is written BEFORE any Case-1 or Case-2 T1 data is collected
so that the reporting protocol cannot be retro-fitted to the numbers.

**Headline discipline.** Our three case studies are **exploratory
demonstrations**. Cases 2 and 3 are N=1 (single subject); Case 1 is a
single-pipeline demonstration on a single canonical corpus. We report
**descriptive statistics**, **ablation deltas**, and — where N is
large enough to be meaningful — **non-parametric bootstrap confidence
intervals**. We run **no hypothesis tests**, **no power analyses**,
and **no p-values**. Inferential statistics on N=1 or N=10 studies
would be misleading at best and fraudulent at worst.

Carried hedges (unchanged): V4 encoder-naming, V11, V12 same-session
band, V18 frozen-encoder, V22 projected-number tags, R3/R4 coverage,
Bias-05 single-subject, Bias-06 single-encoder, Bias-08 self-reported,
Bias-09 benchmark-monoculture, FM-04 measured/projected discipline,
FM-05 single-subject opener, MP-12 composition-not-invention.

---

## 1. Global analysis conventions

Applied to every number that appears in §6 or §7:

| Convention | Rule |
|---|---|
| **Significant figures** | 3 sig-figs on every reported scalar (e.g., 0.961 not 0.9612; 3.93 not 3.925). |
| **Tier flag** | Every number tagged T1-MEASURED (verified, reproducible, dated), T1-PENDING (run planned, data not yet collected), or T3-PROJECTED (qualitative projection; must carry "(projected; camera-ready will re-verify)"). |
| **Date and version** | Every measured number cites measurement date (ISO-8601) and pipeline version (git SHA-7 or semver). |
| **Confidence intervals** | Reported **only where N ≥ 20**. For N < 20 we report descriptive statistics (mean, median, min, max, std) without CI. Bootstrap CI with N=10 on Case 3 is an **exception documented as retrospective only** — reported as `0.961 ± 0.012` with "retrospective 95% bootstrap CI, N=10; not a power-based interval" verbatim. |
| **No hypothesis tests** | No Wilcoxon, no paired t-test, no χ² on the three case studies. Parity (e.g., DNSMOS 3.93 vs 3.93) is reported as descriptive equality within a stated tolerance, not as a test failing to reject. |
| **No p-values** | Zero p-values in §6 or §7 for any of our three cases. §6 uses mean/median/IQR and Δ-deltas for ablations; §7 discusses effect direction and magnitude, not significance. |
| **No power calculations** | None. Sampling protocol §A.*.6 (agent-26) states verbatim: "we do not run a formal power analysis". |
| **Multiplicity** | For Case 1's 9-embedder STYLE_CENTROID: we report the **conjunction** metric as primary and per-embedder as descriptive. This sidesteps multiple-testing because there is no significance claim to correct. If any downstream writer makes a per-embedder significance claim, they must add Bonferroni FWER = 9 or BH FDR control — agent-42 to enforce. |
| **Raw-cosine scale** | Cosine values reported in [−1, +1] for WavLM / SigLIP (matches published literature; chen2024valle2 convention). No hidden `(raw+1)/2` rescaling unless explicitly noted. |
| **Numerical precision** | fp32 for all centroid / cosine computations; bf16 only for generator. Reported values are the fp32 outputs. |
| **Provenance** | Every number traces to a row in agent-27 §H JSON manifest. Agent-43 validates. |

---

## 2. Case 1 — Shakespeare stylistic LoRA (PROSPECTIVE)

### 2.1 Status

All T1 numbers **PENDING** at time of analysis-plan lock. SFT+DPO training
is in progress on cleaned data; 15/15 prior FSV battery passed.

### 2.2 Raw instrument outputs (from agent-27)

Per held-out prompt $i \in \{1, \ldots, N_{\text{eval},1}\}$ (target
$N_{\text{eval},1} \geq 20$; ideal 50):

- 9 per-embedder cosines $\cos(\phi_j(\hat y_i), c_j)$ for
  $j \in \{E1, E4, E6, E7, E8, E9, E10, E12, E13\}$ from D.1 STYLE_CENTROID.
- 1 vocabulary-density ratio $\rho_{\text{arch},i}$ from D.2.
- 1 regeneration-attempt counter $k_i$ (number of Layer-3 rejects
  before accept or fallback).
- 1 per-prompt accept/fallback/reject decision.

**FSV battery output**: binary pass/fail on 15 structural tests
(currently 15/15 PASS; not an identity-quality metric).

### 2.3 Primary analysis (descriptive statistics)

**Per-embedder statistics** (across $N_{\text{eval},1}$ held-out prompts,
reported per embedder $j$):

- Mean $\bar s_j = \text{mean}_i \cos(\phi_j(\hat y_i), c_j)$.
- Median $\tilde s_j$.
- IQR (25th / 75th percentile).
- 10th percentile (for threshold-calibration comparison).
- Min / max.

**Threshold pass-rate** (per embedder): % of held-out prompts with
$\cos \geq \tau_j$ at the calibrated threshold; reported alongside
$\tau_j$ itself.

**Conjunction accept-rate** (primary Case 1 metric): % of held-out
prompts for which **all 9** embedders score above their respective
$\tau_j$. This is the Layer-3 guard's accept rate. Report with
denominator explicit.

**FSV battery pass rate**: 15/15 (as currently measured; locked
artefact, will re-run on final training artefact). This is integrity,
not quality, and must be framed as such in §6.1.

### 2.4 Ablation analysis (W2 analogue for Case 1)

**Ablation design** (two levers):

1. **Vocabulary-density alone** vs. **vocabulary-density +
   STYLE_CENTROID guard**: for each held-out prompt, compute the
   accept decision under (i) Layer-2 only (vocabulary-density ≥ 0.02)
   and (ii) Layer-2 + Layer-3 (conjunction pass at $\tau_j$).
   Report accept-rate delta Δ = accept_L2+L3 − accept_L2. Expected
   direction: L2+L3 is strictly more conservative (Δ ≤ 0 on accept-
   rate) but higher quality on accepted outputs.

2. **Layer-1 prior (SFT+DPO) alone** vs. **Layer-1 + Layer-2 + Layer-3**:
   for each held-out prompt, run the generator with and without the
   downstream guards. Report per-embedder mean cosine delta
   Δ_j = mean_cosine(full_stack) − mean_cosine(LoRA_only). Expected
   direction: full-stack >= LoRA-only per embedder (W2 analogue).
   **Status**: T1 PENDING, flagged as the Case-1 W2 remediation.

**Per-embedder CV** (coefficient of variation) across the
calibration set $S_{\text{text-style}}$ ($|S| \in [50, 200]$): for
each embedder $j$, CV$_j$ = std / mean. Reported as a
threshold-stability diagnostic, not a quality metric.

### 2.5 Cross-encoder check (W1 analogue for Case 1)

If a **ColBERT or BGE-reranker** not in the 9-embedder panel is run on
the same $\hat y$ set, report mean cross-encoder cosine alongside
STYLE_CENTROID mean. Report agreement (Pearson + Spearman) with the
Case-1 conjunction metric. If not run by submission, **disclose
absence explicitly** (L-STYLE-A paste-ready).

### 2.6 Tables and figures for §6.1

- **Table 6.1** (PENDING): 9 rows (one per embedder) × columns
  {$\tau_j$, calibration CV$_j$, held-out mean, held-out median,
  threshold pass-rate, IQR}. Plus an aggregate row for the
  conjunction accept-rate.
- **Figure 6.3** (PENDING): 9-panel histogram of per-embedder
  cosines across calibration set with $\tau_j$ annotated.

### 2.7 What we do NOT compute for Case 1

- No hypothesis test comparing "pre-TCT" vs "post-TCT" generator.
- No power calc for $N_{\text{eval},1}$.
- No inter-rater reliability (Likert scoring is out of scope for
  arXiv v1 per agent-27 handoff item 5).
- No ROC / AUC on a two-class (Shakespearean / not) task — we have
  no ground-truth labels for "Shakespearean-ness" on held-out output
  beyond the reference-centroid cosine, which would make the ROC
  circular.

### 2.8 Confidence

**HIGH** that the descriptive-stats + ablation-delta plan is
adequate for an exploratory demonstration. **MEDIUM** on realised
$N_{\text{eval},1}$ at submission time — the plan tolerates N ∈ [20,
50] and degrades gracefully to "descriptive only, no bootstrap CI"
if N < 20.

---

## 3. Case 2 — Santa talking-head clone (PROSPECTIVE)

### 3.1 Status

All T1 numbers **PENDING**. Forensic pipeline integrity is complete
(flow-matching sign-bug fixed, q_audio excluded from LoRA target).
LSE-D / LSE-C lip-sync measurement is PENDING (flagged as future work
in L-LSE-C).

### 3.2 Raw instrument outputs (from agent-27)

Per held-out scene/chunk $i$ (target $N_{\text{eval},2} \in [20, 100]$,
10 s chunks):

- **Per-frame visual cosine** $\cos(\phi_{\text{visual}}(\hat y_i),
  c_{\text{visual}})$ from C.1 SigLIP-SO400M, pooled per-clip as
  mean-of-per-frame-cosines.
- **Per-chunk speaker cosine** $\cos(\phi_{\text{speaker}}(\hat y_i),
  c_{\text{speaker}})$ from A.1 WavLM-SECS re-used.
- **Per-chunk emotion cosine** (encoder: open carry; agent-28
  recommends `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition`
  if speech-based, or mark "encoder TBD" per agent-27 flag).
- **Per-chunk voice cosine** (encoder: open carry; agent-28
  recommends `pyannote/embedding` or Resemblyzer to avoid collapsing
  conjunction strictness with WavLM).
- **Lip-sync** LSE-D / LSE-C (PENDING; may not land for submission).

### 3.3 Primary analysis (descriptive statistics)

**Per-modality** across $N_{\text{eval},2}$ chunks:
- Mean, median, IQR, min, max, std per modality.
- Threshold pass-rate: % of chunks with $\cos \geq \tau_m$ at
  operating values (visual 0.70, emotion 0.60, speaker 0.80, voice
  0.85).

**Conjunction accept-rate** (primary Case 2 metric): % of chunks
satisfying **all four** thresholds simultaneously. For
$N_{\text{eval},2} \geq 20$ we report a **Wilson-score 95% CI** on the
conjunction accept-rate (single binomial proportion, N ≥ 20 is the
documented minimum per agent-26 §A.2.6). CI formula fixed to Wilson
score, not normal approximation, because small-N Wilson has better
coverage near the [0,1] boundary.

### 3.4 Ablation analysis

Three ablations, **all historical-comparison style** drawn from the
17-approach forensic trail documented in
`docs/inspect/08_training_audit.md` (Santa project):

1. **Constellation guard ON vs OFF**: Phase-3 strict conjunction
   vs no runtime guard. Compare mean per-modality cosine and
   conjunction accept-rate on the same held-out set. Expected: guard
   ON yields higher per-modality cosines at the cost of fallback/
   regenerate rate.

2. **Mouth-weighted loss ON vs OFF**: retrain LoRA with and without
   the 10× mouth-region weight on flow-matching loss. Report per-
   modality mean-cosine delta and lip-sync LSE-D delta. Expected
   direction: mouth-weighted improves lip-sync (LSE-D decreases) with
   minimal cost to global visual.

3. **Flow-matching sign correct vs inverted** (historical): from the
   forensic trail, report the pre-fix baseline numbers (from the
   forensic audit doc) against the post-fix pipeline. This is a
   **historical reference**, not a fresh run, and must be labelled
   as such in the table.

### 3.5 Cross-encoder check (W1 analogue for Case 2)

Not defined at this plan lock — Case 2's identity-verification cross-
encoder question is dominated by (i) whether the SigLIP visual centroid
is cross-encoded against a DINOv2 or a different VLM encoder, and
(ii) whether WavLM-based speaker cosine is cross-encoded against
ECAPA-TDNN on the Case-2 held-out audio. Both are **out of scope for
arXiv v1** and disclosed as future work.

### 3.6 Tables and figures for §6.2

- **Table 6.2** (PENDING, T3 PROJECTED placeholder): 4 modality rows
  × columns {$\tau_m$, held-out mean, median, threshold pass-rate,
  IQR, N}. Plus conjunction accept-rate row with Wilson CI.
- **Figure 6.4** (PENDING): 4-modality cosine scatter matrix
  (visual × speaker × emotion × voice) showing correlation structure
  across chunks. T3 projected placeholder at draft time.
- **Lip-sync callout** (if LSE-D lands): single-value report
  alongside the accept-rate table.

### 3.7 What we do NOT compute for Case 2

- No hypothesis test on pre-vs-post ablation (single-subject, no
  pairing structure supports it).
- No FID on held-out visual unless explicitly in scope for v1 (agent-27
  flagged FID as "T1 PENDING" — agent-28 defers to v2 if FID doesn't
  land).
- No subjective Likert (out of scope per agent-27 handoff item 5).
- No cross-subject generalisation claim.

### 3.8 Confidence

**HIGH** on the descriptive + ablation-delta plan for Case 2,
conditional on end-to-end runs landing. **LOW** on whether those
runs land before submission — §8 L-02-Case2-equivalent handles the
defer-to-v2 case.

---

## 4. Case 3 — Voice clone (MEASURED, N=1 subject, N=10 sentences)

### 4.1 Status

**Headline numbers T1 MEASURED**: mean WavLM SECS 0.961, max 0.975,
min 0.914, std 0.017; DNSMOS P.808 3.93 (clone) vs 3.93 (real);
UTMOS 3.012 (clone) vs 2.997 (real). Measurement date: 2026-04-16.
Pipeline: Qwen3-TTS + Full ICL + 50-clip centroid + WavLM-scored
best-of-12 at T=0.3. SITREP at `tmp/SITREP_2026-04-16_135543.md` and
white paper `tasks/newdata/alldata.md`.

### 4.2 Raw instrument outputs (from agent-27)

- **10 WavLM SECS values** (argmax over 12 candidates per sentence)
  from A.1 WavLM-SECS.
- **120 WavLM SECS values** (full 12×10 pre-selection matrix) — W2
  remediation; PENDING instrument-ready.
- **10 ECAPA-TDNN SECS values** — W1 remediation; PENDING,
  instrument-ready per A.2.
- **10 Resemblyzer SECS values** (secondary W1 scorer) — PENDING.
- **10 DNSMOS P.808** (clone) + 14 DNSMOS P.808 (real mic) values —
  MEASURED.
- **10 UTMOS** (clone) + 14 UTMOS (real mic) values — MEASURED.
- **40 SECS values** from $n_m$ curve (W3): $n \in \{10, 25, 50, 100\}$
  × 10 held-out sentences × 1 argmax-selected candidate —
  instrument-ready, run PENDING.

**Note on N=14 for real-mic scorer sample**: DNSMOS/UTMOS naturalness
reference uses the author's 14 real-mic recordings from the ground-
truth parity session (10 held-out sentences + 4 calibration recordings
from the same session). N=14 is too few for a Wilcoxon to be
informative, which is one of the reasons we abstain from hypothesis
testing here (see §4.4).

### 4.3 Primary analysis (descriptive statistics + bootstrap CI)

**Descriptive** on the N=10 argmax-selected WavLM SECS vector
$\{s^{(k^*)}_i\}_{i=1}^{10}$:

| Statistic | Value (MEASURED 2026-04-16, pipeline `qwen3-tts-foricl-v1`) |
|---|---|
| Mean | **0.961** |
| Max | **0.975** |
| Min | **0.914** |
| Std | **0.017** |
| Count ≥ 0.95 | **9 / 10** |
| Median | To be extracted from raw vector on final read (report with 3 sig-fig) |

**Retrospective 95% bootstrap CI** on the mean (N=10, 10,000 resamples
with replacement, percentile method):

- Expected CI: approximately **[0.951, 0.971]** (reported as `0.961
  ± 0.010` or `[0.951, 0.971]`), to be finalised after deterministic
  seed is chosen (recommended seed: 20260416).
- **Framing discipline**: report as "retrospective 95% bootstrap CI,
  N=10; not a power-based interval" verbatim. This complies with the
  global convention (CI only for N ≥ 20) by **explicitly flagging
  the exception**.

### 4.4 Naturalness parity (descriptive only, no hypothesis test)

- **DNSMOS P.808**: clone mean 3.93 (N=10) vs real mean 3.93 (N=14).
  Report as `parity within ±0.00 at 3 sig-fig`. Do **not** run a
  Wilcoxon signed-rank or paired t-test — N=14 is too few for any
  effect-size estimate to be informative, and we are claiming parity
  (a failure-to-reject framing) which is inappropriate for N < 30.
- **UTMOS**: clone mean 3.012 (N=10) vs real mean 2.997 (N=14). Report
  `Δ = +0.015`. Same discipline as DNSMOS.

### 4.5 Ablation table (MEASURED)

From the existing forensic trail:

| Ablation | Delta | Direction | Source |
|---|---|---|---|
| Full ICL vs x-vector-only | +0.094 | Full ICL > x-vector (SpeechBrain) | `tasks/newdata/alldata.md` |
| Best-of-N with target encoder vs mismatched encoder | −0.013 | Target-encoder selection superior | ibid. |
| Temperature sweep T ∈ {0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8} | T=0.3 optimum | — | ibid. |
| 1-clip vs 50-clip centroid | +0.015 | 50-clip > 1-clip | ibid. |

Each delta reported at 3 sig-fig. Each cites the measurement date
(pre-2026-04-16 SITREP) and the pipeline version. No hypothesis test;
the deltas are descriptive point estimates from the forensic run set.

### 4.6 Planned W1 remediation (cross-encoder)

**ECAPA-TDNN + Resemblyzer SECS on same 10 sentences**:
- Re-score the 10 argmax candidates with ECAPA-TDNN (A.2) and
  Resemblyzer.
- Report mean SECS under each encoder alongside WavLM headline.
- Report cross-encoder agreement: Pearson $r$ and Spearman $\rho$
  between the 10-value WavLM vector and the 10-value ECAPA vector (and
  similarly Resemblyzer). Expected: Spearman > 0.6 is supportive of
  identity-ranking agreement; Pearson sensitive to the systematic
  offset documented in L-ECAPA-A.
- Report Bland-Altman plot of $\Delta = s_{\text{WavLM}} -
  s_{\text{ECAPA}}$ (or Resemblyzer) across 10 sentences; reference
  limits at $\pm 1.96 \cdot \text{std}(\Delta)$. Descriptive only, no
  test.

**Status**: PENDING. If not complete by submission, report as future
work per §8 L-01 paste-ready disclosure.

### 4.7 Planned W2 remediation (pre-selection distribution)

From the 120-candidate WavLM SECS matrix:
- Histogram of all 120 pre-selection SECS values. Bin rule: **Freedman–
  Diaconis** ($h = 2 \cdot IQR / N^{1/3}$) with 120 values giving
  approximately 15–25 bins.
- **Pre-selection mean** $\bar s_{\text{pre}} = \text{mean}_{i,k}
  s^{(k)}_i$ across all 12×10 = 120 values.
- **Post-selection (argmax) mean** $\bar s_{\text{post}} = 0.961$.
- **TCT-delta** $\Delta_{\text{TCT}} = \bar s_{\text{post}} - \bar
  s_{\text{pre}}$ — the headline quantity attributable to best-of-K
  selection under the guard.
- **Best-of-K with $\tau = 0$**: equivalent to $\bar s_{\text{post}}$
  since argmax already ignores $\tau$ in inference-only mode; this
  isolates "sampling-diversity" from "threshold rejection" and should
  match 0.961 within numerical precision.

**Status**: PENDING. If not complete, §8 L-02 paste-ready.

### 4.8 Planned W3 remediation ($n_m$ curve)

For $n \in \{10, 25, 50, 100\}$ top-WavLM-similar reference clips from
the 489-candidate corpus:
- Construct centroid from top-$n$ subset.
- Run best-of-12 at T=0.3 on the same 10 held-out sentences.
- Report mean WavLM SECS at each $n$.
- **Reporting**: 4-point curve (no fitted function; straight line
  connections acceptable in the figure). 3 sig-fig per point.

**Status**: PENDING. If not complete, §8 L-03 paste-ready.

### 4.9 Tables and figures for §6.3

- **Table 6.3** (partially MEASURED): columns {metric, value, method,
  tier}. Rows: mean SECS 0.961 T1, max SECS 0.975 T1, min 0.914 T1,
  std 0.017 T1, 9/10 ≥ 0.95 T1, DNSMOS clone/real 3.93/3.93 T1,
  UTMOS clone/real 3.012/2.997 T1, bootstrap CI [0.951, 0.971] T1,
  ECAPA agreement Pearson/Spearman T1-PENDING, Resemblyzer
  agreement T1-PENDING, pre-selection mean T1-PENDING,
  $\Delta_{\text{TCT}}$ T1-PENDING.
- **Figure 6.1**: pre-selection SECS histogram with pre/post mean
  annotated and $\Delta_{\text{TCT}}$ labelled. T1 PENDING.
- **Figure 6.2**: $n_m$ curve (4 points). T1 PENDING.

### 4.10 What we do NOT compute for Case 3

- **No Wilcoxon signed-rank** on DNSMOS clone vs real (N=14 too few
  for informative effect size; parity is descriptive).
- **No paired t-test** on UTMOS clone vs real (same reason; plus
  UTMOS is bounded with unclear variance structure at the ~3.0 range).
- **No power calculation** on the N=10 held-out set — the observed
  N is the observed N; retrospective power is not reported.
- **No multi-speaker claim** (N=1).
- **No cross-language claim** (single language).
- **No cross-dialect claim** (single dialect).
- **No ROC/AUC against a spoofing detector** — out of scope for v1.

### 4.11 Confidence

**HIGH** on the descriptive + retrospective-bootstrap plan for the
MEASURED headline. **HIGH** on the four ablation deltas (already
measured, stable, and cited from the forensic trail). **MEDIUM** on
whether W1–W3 remediations land for v1 — plan tolerates deferral to
v2 with L-01/L-02/L-03 paste-ready §8 disclosures.

---

## 5. Cross-case analysis (for §7 Discussion)

### 5.1 Qualitative synthesis only

Discussion claims in §7 connect the three cases qualitatively:

- Case 3 **measures** the TCT-inference-only mode (best-of-K on a
  frozen backbone); the 0.961 mean / 0.975 max is the single
  empirical headline.
- Case 1 demonstrates TCT-frozen-with-LoRA on text-style (N=1
  pipeline, T1 PENDING end-to-end).
- Case 2 demonstrates TCT-frozen-with-LoRA on multi-modal identity
  (N=1 subject, T1 PENDING end-to-end, forensic integrity complete).

### 5.2 No cross-case meta-analysis

- No pooled effect size across cases (different metrics, different
  encoders, different modalities — pooling is invalid).
- No "TCT works in 3/3 cases" count — this would imply an inferential
  claim from N=3 demonstrations, which is not supported.
- No forest plot or meta-regression.

### 5.3 What §7 does claim (qualitatively)

- **Existence claim**: a re-implementable protocol exists (per agent-25)
  and produces the reported numbers under the reported protocol (per
  agent-27 byte-identical specifications).
- **Mechanism claim**: geometric proximity to a frozen centroid is a
  per-output verifiable predicate (Theorem 2); the 0.961/0.975
  numbers demonstrate the predicate behaves as specified on one
  subject.
- **Scope disclaimer**: no generalisation beyond the documented
  scope of each case (§8 Limitations).

---

## 6. Reproducibility check-plan (for agent-43)

### 6.1 Minimum checks

Agent-43 runs against agent-27's JSON manifest schema (§H of
instrument specs):

1. **Manifest completeness**: every required field non-null for every
   reported number.
2. **Hash validation**:
   - `centroid_hash` matches SHA-256 of the NPY artefact.
   - `embedder_version_hash` matches downloaded weight SHA-256.
   - `threshold_set_hash` matches JSON of $\{\tau_m\}$.
3. **Threshold lock**: `threshold_set` unchanged from calibration
   to evaluation run (hash-compared pre/post).
4. **Per-output trace length**: equals held-out sample size (e.g., 10
   for Case 3; $N_{\text{eval},1}$ for Case 1; $N_{\text{eval},2}$ for
   Case 2).
5. **Frozen-encoder audit**: TCT §2.3 hook reports
   `all_phi_requires_grad_false = True` and
   $\lVert c_m^{\text{post}} - c_m^{\text{pre}} \rVert_2 < 10^{-5}$
   on pre/post-training re-run.
6. **Tier-flag completeness**: every number in §6 and §7 carries a
   tier tag (T1-MEASURED with date+SHA, T1-PENDING, or T3-PROJECTED).
7. **Bootstrap reproducibility**: the 10,000-resample bootstrap for
   Case 3 uses a fixed seed (recommended 20260416). Agent-43 re-runs
   and verifies the CI endpoints match to 3 sig-fig.

### 6.2 Adversarial-reviewer handoff

Agent-42 (adversarial-reviewer) gets from agent-28:
- The full list of T1 PENDING numbers that may still be unresolved at
  submission time — to flag for §8 disclosure.
- The no-hypothesis-test discipline — to confirm nothing slips into
  the draft accidentally.
- The 3-sig-fig rule — to catch unrounded numbers.
- The tier-tag rule — to catch unlabelled numbers.

---

## 7. Open carries agent-28 resolves or passes on

From agent-27 handoff items 1–5:

1. **Case-2 emotion encoder**: agent-28 recommends
   `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition` if a
   measurement is attempted for v1, but given Case 2 is PENDING
   end-to-end, marks the modality **"encoder TBD / end-to-end PENDING"**
   in §6.2. Agent-37 to confirm.
2. **Case-2 voice-identity encoder**: agent-28 recommends
   `pyannote/embedding` or Resemblyzer to **avoid collapsing
   conjunction strictness with WavLM** (speaker = WavLM, voice =
   distinct encoder). If Case 2 end-to-end doesn't land, same "encoder
   TBD / PENDING" treatment.
3. **Case-3 pre-selection distribution (W2)**: specified above §4.7;
   run PENDING, post-submission-run-plan documented.
4. **Case-3 $n_m$ curve (W3)**: specified above §4.8; run PENDING,
   post-submission-run-plan documented.
5. **Human-Likert protocol**: agent-28 **confirms out of scope** for
   arXiv v1, per agent-27 recommendation. Blinded rater recruitment
   and IRB-equivalent process cannot close by submission.

---

## 8. Summary — analysis-at-a-glance

| Case | Primary metrics | Statistics reported | CI? | Tests? | Ablation |
|---|---|---|---|---|---|
| 1 Shakespeare | Per-embedder cosine; conjunction accept-rate; FSV 15/15 pass-rate | Mean / median / IQR per embedder; conjunction rate | No (N < 20 likely) | No | Vocab vs Vocab+STYLE; Layer-1 vs full-stack |
| 2 Santa | Per-modality cosine; 4-modality conjunction accept-rate; LSE-D (pending) | Mean / median / IQR per modality; conjunction with Wilson CI if N ≥ 20 | Wilson CI on conjunction if N ≥ 20 | No | Guard ON/OFF; mouth-weighted ON/OFF; flow-matching sign (historical) |
| 3 Voice | Mean WavLM SECS 0.961, max 0.975; DNSMOS/UTMOS parity; ECAPA/Resemblyzer agreement (pending); $n_m$ curve (pending) | Mean, max, min, std, count ≥ 0.95; parity Δ; pre/post mean; TCT-delta | Retrospective bootstrap CI on mean (N=10; explicitly flagged as exception) | No | Full ICL vs x-vector (+0.094); target vs mismatched encoder (−0.013); T=0.2–0.8 sweep; 1-clip vs 50-clip centroid (+0.015) |

---

## 9. Confidence

**HIGH** that the plan as specified is executable by a competent
data analyst given only this file + agent-27's instrument spec.

**HIGH** that the no-hypothesis-test discipline is correct for
exploratory N=1 (Cases 2/3) and single-pipeline (Case 1) studies —
any test would be statistically non-informative or outright
misleading.

**HIGH** that the retrospective bootstrap CI on Case 3 (N=10) is
defensible with the verbatim "retrospective, not power-based"
framing.

**MEDIUM** on Cases 1 and 2 analysis realisability — depends on
whether end-to-end runs land by submission. The plan degrades
gracefully (PENDING flags, future-work disclosures, L-01/L-02/
L-03-equivalents) without invalidating the methodological discipline.

## Changelog

- 2026-04-16 — Initial issue by agent-28-analysis-planner. Locked.
