---
namespace: patterns
agent_id: agent-29-validity-guardian
timestamp: 2026-04-16T18:15:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/patterns/agent-25-method-designer--weakness-remediation.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
downstream_consumers:
  - agent-30-risk-analyst
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
tags: mitigation, disclosure, validity, patterns, writer-cues
answers_questions: [Q06, Q08, Q15]
---

# Mitigation and Disclosure Plan — Per-Threat Action Table

## 0. Purpose

Paste-ready mitigation and disclosure cues mapped to each confirmed
validity threat (from `validity-threats.md`). Writers (agent-37
methodology, agent-38 results, agent-40 limitations) consult this file
to see: **what mitigation is already baked into the design**, **what
mitigation is PENDING (W1/W2/W3 remediations)**, and **what paragraph
each writer must land**.

Every row has:
- **Threat ID** (VT-x from validity-threats).
- **Mitigation**: design- or analysis-level action already specified
  by agents 25–28.
- **Section**: target section in the manuscript.
- **Writer cue**: one-line instruction for the responsible writer.
- **Paste-ready snippet**: the verbatim sentence or paragraph the
  writer uses.

---

## 1. Mitigations already baked into the design (no writer action beyond disclosure)

### VT-I1 — Case 3 sentence selection (LOW, MITIGATED)

- **Design mitigation**: agent-26 §A.3.5 — 10 sentences authored with
  zero lexical overlap to ICL transcripts, soft-balanced by utterance
  type.
- **Section**: §6.3 methodology (agent-37).
- **Writer cue**: state the zero-lexical-overlap construction.
- **Paste-ready snippet**:
  > *"Held-out sentences were authored with zero lexical overlap to
  > the ICL reference transcripts (verified by string-match and
  > lemma-overlap checks), with soft utterance-type balancing
  > (declarative / interrogative / exclamatory)."*

### VT-I5 — Case 3 temperature-sweep carry-over (LOW, MITIGATED)

- **Design mitigation**: each of the 12 candidates per T is an
  independent forward pass with a distinct seed.
- **Section**: Table 6.3 footnote.
- **Writer cue**: footnote confirming per-candidate seed
  independence.
- **Paste-ready snippet**:
  > *"Each of the K=12 candidates per temperature is an independent
  > forward pass with a distinct seed; no autoregressive state leaks
  > across candidates."*

### VT-C5 — Identity-locked cross-modality conflation (MEDIUM, MITIGATED)

- **Design mitigation**: agent-04 identity-locked definition is
  geometric-proximity-to-centroid per modality; §5.4 derivation
  maintains per-modality conjunction framing.
- **Section**: §4 definition; §5.4 derivation; §6 openers.
- **Writer cue**: no collapse of M into a single score; per-modality
  language throughout.

---

## 2. Mitigations via language tightening (writer does the rewrite)

### VT-C7 — Meaning-compression dimensional claim (MEDIUM)

- **Rewrite**:
  - **Before**: "Meaning compression is dimensionally distinct from
    bit-, weight-, activation-compression."
  - **After**: "Meaning compression is **semantically distinct**,
    measured as structured-signal density (signals per raw-data unit)
    rather than Shannon bits."
- **Section**: §3.1 meaning-compression ratio view; §5.2 relation to
  existing frameworks; §9 Conclusion if cited.
- **Writer cue**: replace "dimensionally distinct" with "semantically
  distinct / measured as structured-signal density".

### VT-T4 — TCT attribution vs embedder conditioning (MEDIUM)

- **Rewrite**:
  - **Before**: "TCT causes sample-complexity collapse."
  - **After**: "TCT **operationalises** the sample-complexity
    collapse implicit in well-conditioned frozen embedders."
- **Section**: §3.6; §5.4; §7 Discussion; §9 Conclusion.
- **Writer cue**: replace "TCT causes" / "attributable to TCT" with
  "TCT operationalises" / "inherited from frozen-embedder conditioning
  literature".

### VT-T6 — GPU fp determinism footnote (LOW)

- **Paste-ready footnote** on §5.3 guard definition:
  > *"Deterministic modulo GPU floating-point reproducibility; the
  > guard implementation uses identical parameters, inputs, and kernel
  > order at both training and inference time."*

### VT-T7 — H4 collapse-silent scope (MEDIUM)

- **Rewrite**:
  - **Before**: "DDA is collapse-silent."
  - **After**: "DDA is **collapse-silent under Shumailov-2024-form
    generator-in-loop recursion**. Other forms of embedder-mediated
    data selection feedback are out of scope of those theorems."
- **Section**: §3.3 DDA scope argument.

### VT-T1 / VT-C6 — DDA independence language (HIGH)

- **Rewrite**:
  - **Before**: "13 embedders yield 91 orthogonal labeled signals per
    input."
  - **After**: "13 embedders yield **up to** 91 structured signals per
    input (**N + C(N,2)** under pairwise-independence assumption
    Axiom 2; empirical redundancy measurement is RO-8 future work)."
- **Section**: §3.4 DDA counting; §1 opener; §4.1 Context Graph;
  any abstract / headline.
- **Writer cue**: every occurrence of the "91 signals" figure carries
  the "up to" hedge and cites Axiom 2 independence assumption.

---

## 3. Mitigations requiring analysis-run execution (W1/W2/W3 remediation; PENDING)

These are the three remediation runs agent-25 § (weakness remediation)
specifies. All are instrument-ready per agent-27; runs PENDING. The
writer posture is **two-track**: paste the measurement if the run
lands; paste the L-01 / L-02 / L-03 disclosure paragraph if the run
does not land.

### VT-I3 / VT-C1 / VT-S4 — WavLM encoder-matched evaluation (CRITICAL)

**Remediation target**: **W1 cross-encoder** on Case 3 — re-score 10
argmax candidates with ECAPA-TDNN and Resemblyzer.

**Analysis plan (agent-28 §4.6)**:
- Pearson $r$, Spearman $\rho$ between WavLM 10-vector and
  ECAPA 10-vector (and Resemblyzer).
- Bland-Altman plot of $\Delta = s_{\text{WavLM}} - s_{\text{ECAPA}}$.
- Mean / max / 9-of-10-above-threshold under each cross-encoder.

**Section**: §6.3 Case 3 results + §8 L-01.

**Writer cue (track A — W1 lands)**: add §6.3 sub-table:
> *"Cross-encoder agreement (10 argmax candidates): WavLM mean
> 0.961; ECAPA-TDNN mean [X]; Resemblyzer mean [Y]. Pearson
> $r$(WavLM, ECAPA) = [Z]; Spearman $\rho$(WavLM, ECAPA) = [W].
> Bland-Altman Δ mean = [V], ±1.96·std = [U]. Agreement within
> ±0.10 supports within-family identity-ranking consistency; residual
> absolute offset is expected per L-ECAPA-A (VoxCeleb-trained
> encoder calibration)."*

**Writer cue (track B — W1 does not land)**: paste §8 L-01 paragraph
verbatim (see §4 below).

**Mandatory regardless of track**: §6.3 **must** carry agent-24 W1
paste-ready single sentence:
> *"The 0.961 mean SECS is an encoder-matched measurement: the guard
> and evaluator are both WavLM-base-plus-sv. This demonstrates the
> guard behaves as specified; it does not constitute cross-encoder
> identity verification."*

### VT-I2 — LoRA / Qwen3-TTS confound via pre-selection distribution (CRITICAL)

**Remediation target**: **W2 pre-selection histogram** on Case 3 —
120 WavLM SECS values (12 candidates × 10 sentences) before argmax
selection.

**Analysis plan (agent-28 §4.7)**:
- Freedman-Diaconis-binned histogram.
- Pre-selection mean $\bar s_{\text{pre}}$, post-selection mean
  0.961, TCT-delta $\Delta_{\text{TCT}}$.
- Best-of-K with $\tau=0$ vs with $\tau=0.95$ (should match within
  numerical precision).

**Section**: §6.3 Case 3 results + §8 L-02.

**Writer cue (track A — W2 lands)**:
> *"Pre-selection WavLM SECS over 120 candidates: mean
> $\bar s_{\text{pre}}$ = [X], std [Y]. Post-selection argmax mean
> 0.961. TCT-delta $\Delta_{\text{TCT}}$ = 0.961 − [X] = [Z]. This
> isolates the contribution of best-of-K selection under the guard
> from the underlying Qwen3-TTS + ICL base-pipeline quality. Figure
> 6.1 shows the pre-selection histogram with pre/post means annotated."*

**Writer cue (track B — W2 does not land)**: paste §8 L-02 paragraph
verbatim.

**Mandatory regardless of track**: §5.4 attribution language softens
from "TCT causes" to "TCT verifies and selectively accepts"
(agent-24 W6).

### VT-T3 — Sample-complexity single-operating-point (HIGH)

**Remediation target**: **W3 four-point $n_m$ curve** on Case 3 —
$n_m \in \{10, 25, 50, 100\}$ on the same 10 held-out sentences.

**Analysis plan (agent-28 §4.8)**:
- Top-$n$ WavLM-similar clips to form each centroid.
- Best-of-12 at T=0.3, same 10 sentences.
- Report mean WavLM SECS at each $n$.
- 4-point curve (straight-line connections acceptable).

**Section**: §6.3 Case 3 results (Figure 6.2) + §8 L-03.

**Writer cue (track A — W3 lands)**:
> *"Sample-complexity curve: mean WavLM SECS at $n_m \in \{10, 25,
> 50, 100\}$ = [A, B, 0.961, C] (3 sig-figs). Figure 6.2 shows the
> monotone/non-monotone curve shape."*

**Writer cue (track B — W3 does not land)**: paste §8 L-03 paragraph
verbatim.

**Mandatory regardless of track**: §5.4 Theorem 1 statement carries:
*"Case 3 supplies the single T1 operating point at $n_m=50$ on one
modality; the sample-complexity curve is RO-10 future work."*

### VT-T5 — Multi-modal M>1 evidence (HIGH)

**Remediation target**: **Case 2 end-to-end T1 run** — four-modality
conjunction accept-rate on held-out scenes.

**Analysis plan (agent-28 §3.3)**:
- Per-modality mean / median / IQR on $N_{\text{eval,2}} \in [20,100]$
  chunks.
- Conjunction accept-rate with Wilson-score 95% CI.
- Ablation: guard ON/OFF, mouth-weighted ON/OFF, flow-matching sign
  (historical).

**Section**: §6.2 Case 2 results + §8 L-05.

**Writer cue (track A — Case 2 lands)**: use agent-28 §3.6 table 6.2
template with measured values at 3 sig-figs.

**Writer cue (track B — Case 2 does not land)**: §6.2 remains T3
PROJECTED placeholder per agent-28 §3.2; §8 L-05 paste-ready.

---

## 4. Disclosure-only (UNCLOSEABLE) threats — §8 paste-ready

These threats cannot be remediated under current scope; they are
**disclosure-only**.

### VT-E1 — Population validity / single subject (CRITICAL)

- **Paste-ready** (agent-26 §F, Bias-05):
  > *"Cases 2 and 3 are N=1 subject (the author). Case 1 is a
  > single-pipeline study on a single canonical corpus (Shakespeare).
  > No case supports multi-subject, multi-speaker, or multi-author
  > generalisation."*

### VT-E2 — Single language (HIGH)

- **Paste-ready** (Bias-09):
  > *"All three case studies are conducted in English only. No
  > cross-language evidence exists; transfer to other languages is
  > out of scope."*

### VT-E3 — Single microphone / session nuance (HIGH)

- **Paste-ready L-04** (agent-26 §A.3.5 + §D.3):
  > *"Case 3 session structure: the 50-clip reference centroid was
  > derived from a 16-minute video source (one session, one
  > microphone). The 10 held-out sentences were recorded in a
  > separate session on a different microphone in a different room,
  > which **also served as** the real-microphone parity-comparator
  > session for DNSMOS / UTMOS / WavLM parity scoring. Consequently
  > the generator-input condition is cross-session (reference ≠
  > evaluation), while the parity-scorer condition is same-session
  > (held-out = real-mic). Reproducibility of the parity claim is
  > contingent on this session structure being reconstructed."*

### VT-E4 — Clean-audio ecological validity (HIGH)

- **Paste-ready** (Bias-09 expansion):
  > *"Case 3 evaluates clean-audio TTS on author-read English
  > sentences in a single acoustic environment. Performance under
  > noisy microphones, background interference, codec compression,
  > and variable SNR is not characterised by this study."*

### VT-E6 — Case-1 single-corpus (MEDIUM)

- **Paste-ready** (agent-26 §A.1.6):
  > *"Case 1 evaluates one pipeline on one canonical corpus
  > (Shakespeare) with one guard-encoder panel. Held-out N is in the
  > tens; the measurement reports mean and distribution of per-prompt
  > style cosine, not a statistically-powered comparison against a
  > published benchmark."*

### VT-E5 — Vendor-versioned scorers (MEDIUM)

- **Paste-ready**:
  > *"All measurements are tagged to encoder weight SHA-256 and
  > pipeline version. Future vendor releases of Qwen3-TTS, WavLM,
  > ECAPA-TDNN, SigLIP, DNSMOS, or UTMOS may produce different
  > values. The manifest (agent-27 §H schema) enables byte-identical
  > re-measurement against pinned weights."*

### VT-C2 — DNSMOS/UTMOS neural proxy (HIGH)

- **Paste-ready L-DNSMOS-A + L-UTMOS-A** (agent-27 verbatim):
  > *"DNSMOS and UTMOS are neural proxies for human MOS, trained on
  > human-labelled MOS datasets. The parity claim (DNSMOS 3.93
  > clone vs 3.93 real; UTMOS 3.012 clone vs 2.997 real) is parity
  > on the predictor's output, not validated against direct human
  > naturalness judgement. DNSMOS was trained on noise-suppression
  > output, not on TTS; UTMOS was trained on BVCC + VCC (TTS/VC
  > data) and is better-suited but still a predictor. Two
  > independent scorers agreeing at parity provides modest
  > triangulation; blinded human Likert evaluation is named future
  > work."*

### VT-C3 — SigLIP "same face" construct (HIGH)

- **Paste-ready L-SigLIP-A** (agent-27 verbatim):
  > *"Case 2's visual-identity modality uses SigLIP-SO400M
  > cosine similarity on face-region crops. SigLIP is a
  > general-purpose visual-text encoder; its similarity metric
  > measures visual-embedding proximity on the chosen crop, not
  > face identity in the face-recognition sense. Cross-encoder
  > validation against a dedicated face-recognition model
  > (ArcFace / MagFace / FaceNet) is named future work."*

### VT-C4 — 9-embedder STYLE_CENTROID construct (MEDIUM)

- **Paste-ready** (agent-27 L-STYLE-A + L-VOCAB-A):
  > *"Case 1's Shakespearean-style guard is a conjunction of 9
  > per-embedder cosines plus a vocabulary-density prefilter. The
  > 9-embedder panel (dense retrieval, late-interaction, sparse) is
  > architecturally diverse but shares a single Shakespeare
  > reference corpus; the construct coverage is strong on lexical
  > and semantic signals but does not directly measure
  > meter / rhythm / rhetorical devices. Cross-encoder evaluation
  > (ColBERT or BGE-reranker) is named future work."*

### VT-T2 — Threshold calibration portability (HIGH)

- **Paste-ready** (V12 hedge):
  > *"Per-output guarantees are contingent on calibrated thresholds
  > (τ_m). Thresholds reported in this paper (τ = 0.95 for Case 3
  > speaker; τ = (0.70, 0.60, 0.80, 0.85) for Case 2) are empirical
  > values on author-specific data at author-specific sessions.
  > Cross-subject / cross-session / cross-encoder threshold
  > portability is not characterised and is named future work."*

### VT-I6 — Forensic-fix confound (MEDIUM)

- **Paste-ready**:
  > *"Headline numbers are conditional on the forensic fixes
  > documented in each case's audit trail (Case 1: training-data
  > echo-bug fix and 15/15 FSV battery; Case 2: flow-matching
  > sign-bug fix and q_audio exclusion from LoRA target modules;
  > Case 3: SITREP-documented protocol fixes). Reproducibility is
  > by re-execution against pinned weight hashes and pipeline
  > versions in the manifest schema (agent-27 §H)."*

---

## 5. Discipline-enforcement by adversarial reviewer (agent-42)

Agent-29 passes the following **must-catch** list to agent-42:

1. **No §6 text** may claim "perceptual identity" or "psychological
   identity". Identity-locked is geometric-proximity-to-centroid
   per agent-04.
2. **No §6 text** may cite the Qwen3-TTS internal 2048-D speaker
   latent as a cross-encoder verification (agent-27 L-Qwen3-B;
   FM-04 category error).
3. **No §7 text** may claim "TCT causes sample-complexity collapse"
   without the "operationalises" softening.
4. **No §3 / §5 / abstract text** may say "91 orthogonal signals"
   without the "up to" hedge and Axiom 2 citation.
5. **No §6.3 text** may cite the 0.961 headline without the
   encoder-matched disclosure sentence (VT-I3/C1/S4 paste-ready).
6. **No statement** of cross-subject, cross-language, cross-dialect,
   or cross-corpus generalisation may appear in §6 / §7 / §9.
7. **No hypothesis-test / p-value / power-calculation** may appear in
   §6 / §7 (agent-28 discipline).
8. **No Likert / human-perception** claim in v1 (out of scope per
   agent-28 §7 item 5).
9. **Every T1 MEASURED number** carries measurement date + pipeline
   version (agent-28 §1 "Date and version" convention).
10. **Every cosine / MOS value** names the encoder (V4 discipline).

---

## 6. Mitigation priority order (cost-benefit)

Ordered by the biggest risk-reduction for the smallest remediation cost.

**Rank 1 (zero-cost language rewrites; must land)**:
- VT-C7 "dimensionally distinct" → "semantically distinct"
- VT-T4 "TCT causes" → "TCT operationalises"
- VT-T1 / VT-C6 "91 signals" → "up to 91 signals under Axiom 2"
- VT-T6 GPU fp determinism footnote
- VT-T7 H4 "collapse-silent under Shumailov-form" scope

**Rank 2 (mandatory disclosures; paste-ready paragraphs)**:
- VT-I3 / VT-C1 / VT-S4 encoder-matched single-sentence disclosure
  + §8 L-01 paragraph
- VT-E1 / VT-E2 / VT-E4 / VT-E6 disclosure-only §8 paragraphs
- VT-E3 §8 L-04 session-structure paragraph
- VT-C2 / VT-C3 / VT-C4 construct-limitation paragraphs
- VT-I6 forensic-fix disclosure

**Rank 3 (PENDING runs; high-ROI if they land)**:
- **W1** ECAPA + Resemblyzer on 10 argmax candidates (closes VT-I3 /
  VT-C1 / VT-S4 partially)
- **W2** 120-candidate pre-selection histogram (closes VT-I2)
- **W3** 4-point $n_m$ curve (closes VT-T3)

**Rank 4 (stretch runs; v2)**:
- Case 2 end-to-end T1 (closes VT-T5 multi-modal evidence)
- Case 1 end-to-end T1 (closes H2c projected-value pendings)
- RO-8 pairwise-MI measurement (closes VT-T1 / VT-C6)
- Cross-subject replication (closes VT-E1)
- Cross-language replication (closes VT-E2)
- Face-recognition cross-encoder on Case 2 (closes VT-C3)

---

## 7. Confidence

**HIGH** that the mitigation table above exhaustively covers the
confirmed threats with a clear decision tree (design-mitigated vs
language-rewrite vs pending-run vs disclosure-only).

**HIGH** that the Rank-1 language rewrites have zero dependency on
any PENDING run and can land in v1 with no further input.

**HIGH** that the Rank-2 disclosure paragraphs are paste-ready and
require no new measurement.

**MEDIUM** on Rank-3 run timing (W1/W2/W3); the plan tolerates v2
deferral gracefully.

## Changelog

- 2026-04-16 — Initial issue by agent-29-validity-guardian.
