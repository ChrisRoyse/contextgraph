---
namespace: patterns
agent_id: agent-29-validity-guardian
timestamp: 2026-04-16T18:30:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/patterns/agent-29-validity-guardian--mitigation-plan.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
downstream_consumers:
  - agent-40-limitations-writer
  - agent-37-methodology-writer
  - agent-42-adversarial-reviewer
tags: section-8, limitations, paste-ready, validity, disclosures
answers_questions: [Q06, Q08, Q15]
---

# §8 Limitations — Paste-Ready Draft

## 0. Usage

This is the **paste-ready** §8 Limitations section for the arXiv
manuscript. Agent-40 (limitations-writer) uses it as the anchor draft
and may reorder / condense to fit §8's allotted length, but MUST
retain the content of every paragraph below.

Paragraphs marked **[MANDATORY]** cannot be cut; they close a
CRITICAL or HIGH threat. Paragraphs marked **[ENHANCE IF ROOM]** may
be condensed to a single sentence if space is tight.

Limitation IDs L-01 through L-12 are assigned here and referenced
from §6 case-study sections.

---

## §8 — Limitations

We structure this section by the four validity frames (internal,
external, construct, statistical-conclusion) plus a theoretical-claim
frame and a reproducibility note. The three threats that drive the
tightest disclosures are **L-01** (encoder-matched evaluation on
Case 3), **L-02** (missing pre-selection distribution on Case 3), and
**L-05** (multi-modal conjunction has no M>1 empirical evidence).

### §8.1 Internal-validity limitations

**[MANDATORY] L-01 — Encoder-matched evaluation on Case 3.**
The Case 3 pipeline uses **WavLM-base-plus-sv** as both the Phase-3
guard (best-of-K selects candidates by WavLM cosine) and the
evaluator (the headline mean SECS of 0.961 is computed on the same
encoder). Samples are **selected** for WavLM cosine and then
**scored** on WavLM cosine; the number we report is the signature of
the guard behaving as specified, not cross-encoder identity
verification. W1 remediation (re-scoring with ECAPA-TDNN and
Resemblyzer on the same 10 argmax candidates, reporting Pearson $r$,
Spearman $\rho$, and Bland-Altman agreement) is instrument-ready
(agent-27 A.2; agent-28 §4.6); at the time of submission the run
is [LANDED / PENDING — strike one]. If PENDING, the 0.961 headline
reads as "**within-WavLM-family** identity agreement, 9 of 10 $\geq
0.95$", not as a general identity-quality claim.

**[MANDATORY] L-02 — Missing pre-selection distribution on Case 3.**
The 0.961 mean SECS is the post-argmax selection over 12 candidates
per sentence. We do **not** report the pre-selection distribution
(all 120 = 12 × 10 candidates) at this revision. Without it we
cannot quantitatively decompose the 0.961 into (a) base Qwen3-TTS +
50-clip ICL conditioning and (b) best-of-K selection under the
WavLM guard (TCT Phase 2 inference-only mode). Consequently §5.4
attributes per-output identity-lock to TCT as a **selection and
verification operator** over the vendor generator's sample space,
not as a causal source of the identity-lock property. W2
remediation (pre-selection histogram with Freedman–Diaconis binning,
pre/post mean, TCT-delta) is specified at agent-28 §4.7; at the
time of submission the run is [LANDED / PENDING — strike one]. If
PENDING, the ablation table in §6.3 stands on the four
forensic-trail deltas (full-ICL vs x-vector: +0.094; target-vs-
mismatched encoder: −0.013; 1-clip vs 50-clip centroid: +0.015;
temperature sweep at T=0.3 optimum) as the evidence for
best-of-K-under-guard contributing non-trivially.

**[ENHANCE IF ROOM] L-Forensic — Forensic-fix dependency.**
All three cases embed forensic pipeline fixes: Case 1 (training-data
echo-bug fix + 15/15 FSV battery), Case 2 (flow-matching sign-bug
fix + q_audio exclusion from LoRA target), Case 3 (SITREP-documented
protocol fixes). Headline numbers are conditional on these fixes.
Reproducibility is by re-execution against pinned weight hashes and
pipeline versions (agent-27 §H manifest schema) with the documented
fixes in place.

### §8.2 External-validity limitations

**[MANDATORY] L-Population — N=1 subject across all cases.**
Cases 2 and 3 are N=1 subject (the author). Case 1 is a
single-pipeline study on a single canonical corpus (Shakespeare).
**No case supports multi-subject, multi-speaker, multi-author,
cross-language, cross-dialect, or cross-corpus generalisation.** Our
three case studies are exploratory demonstrations; power
calculations are not run and are not appropriate for this design
(agent-26 §A.*.6; agent-28 §1). Multi-subject replication is the
highest-priority future work.

**[MANDATORY] L-03 — Case 3 single operating point on n_m.**
Axiom 3 ($n_m \in [10, 10^2]$ suffices) is supported empirically by
a **single operating point**: $n_m = 50$ producing mean WavLM SECS
0.961 on Case 3's one subject, one modality. The sample-complexity
curve shape is unknown. W3 remediation (4-point $n_m$ curve at
$n \in \{10, 25, 50, 100\}$) is specified at agent-28 §4.8; at the
time of submission the run is [LANDED / PENDING — strike one]. If
PENDING, §5.4 Theorem 1's operating-point language remains carried
through: *"Case 3 supplies the single T1 operating point at
$n_m=50$ on one modality; the sample-complexity curve is RO-10
future work."*

**[MANDATORY] L-04 — Case 3 session structure.**
The 50-clip reference centroid was derived from a 16-minute video
source (one session, one microphone). The 10 held-out sentences
were recorded in a **separate** session on a different microphone
in a different room, which **also served as** the
real-microphone parity-comparator session for DNSMOS / UTMOS /
WavLM parity scoring. Consequently the generator-input condition is
cross-session (reference ≠ evaluation — a strengthener), while the
parity-scorer condition is same-session (held-out = real-mic — an
accepted design decision). Reproducibility of the naturalness-
parity claim is contingent on this session structure being
reconstructed.

**[ENHANCE IF ROOM] L-Language — Single language across all cases.**
All three cases are in English. No cross-language evidence exists.
Transfer to other languages is out of scope of this manuscript.

**[ENHANCE IF ROOM] L-Ecological — Clean-audio / author-read.**
Case 3 evaluates clean-audio TTS on author-read English sentences
in a single acoustic environment. Performance under noisy
microphones, background interference, codec compression, and
variable SNR is not characterised by this study.

**[ENHANCE IF ROOM] L-Corpus — Case 1 single canonical corpus.**
Case 1 evaluates one pipeline on one corpus (Shakespeare) with one
9-embedder guard panel. The construct "Shakespearean" is operational
as similarity to canonical Folger / Gutenberg passages; transfer to
Milton, King James Bible, Chaucer, or other early-modern-English
corpora is out of scope.

### §8.3 Construct-validity limitations

**[MANDATORY] L-Construct-A — "Identity-locked" is geometric, not
perceptual.** Identity-locked generation is operationalised as
geometric proximity to a frozen-encoder centroid. This is a
measurement construct, not a claim about perceptual or psychological
identity. Human naïve-listener evaluation and cross-encoder
face-recognition / speaker-recognition verification against
task-specific models are named future work.

**[MANDATORY] L-DNSMOS — DNSMOS / UTMOS are neural proxies.**
DNSMOS P.808 and UTMOS are **neural predictors** of human MOS
labels, trained on MOS-labelled datasets (DNS-Challenge for DNSMOS;
BVCC + VCC for UTMOS). The Case 3 parity claim (DNSMOS 3.93 clone
vs 3.93 real; UTMOS 3.012 clone vs 2.997 real) is parity on the
predictor's output, not validated against direct human naturalness
judgement. DNSMOS was trained on noise-suppression output rather
than TTS; UTMOS is better-matched to TTS / VC but still a
predictor. Two independent scorers agreeing at parity provides
modest triangulation; a blinded human Likert evaluation is named
future work (out of scope for v1).

**[ENHANCE IF ROOM] L-SigLIP — SigLIP visual-centroid cosine.**
Case 2's visual-identity modality uses SigLIP-SO400M cosine on
face-region crops (YOLOv5-Face detection). SigLIP is a
general-purpose visual-text encoder; the cosine measures
visual-embedding proximity on the crop, not face identity in the
face-recognition sense. Cross-encoder validation against a
dedicated face-recognition model (ArcFace / MagFace / FaceNet) is
future work.

**[ENHANCE IF ROOM] L-STYLE — Shakespearean-style guard coverage.**
Case 1's Shakespearean-style guard is a conjunction of 9 per-embedder
cosines (E1 / E4 / E6 / E7 / E8 / E9 / E10 / E12 / E13) plus a
vocabulary-density prefilter. Construct coverage is strong on
lexical and semantic signals but does not directly measure
iambic meter, rhythmic patterns, or rhetorical devices (anaphora,
chiasmus, caesura). Cross-encoder evaluation with a ColBERT or
BGE-reranker from outside the 9-embedder panel is future work.

### §8.4 Statistical-conclusion limitations

**[MANDATORY] L-StatDiscipline — No hypothesis tests, no
p-values, no power analyses.** Our three case studies are
exploratory N = 1 (or single-pipeline) demonstrations. We report
**descriptive statistics**, **ablation point-estimate deltas**, and
— for Case 3's N = 10 held-out set — a **retrospective 95%
bootstrap CI on the mean** (10,000 resamples with replacement,
percentile method, deterministic seed 20260416), explicitly flagged
as retrospective, not power-based. We run **no hypothesis tests,
no p-values, no power calculations, no multiplicity corrections,
no ROC/AUC on circular reference-centroid targets, no Bayesian
posteriors, no cross-case meta-analysis** on our three cases.
Inferential statistics on N = 1 or N = 10 exploratory
demonstrations would be misleading at best; the descriptive /
ablation-delta discipline is appropriate for the design
(agent-28 analysis plan locks this).

**[ENHANCE IF ROOM] L-Parity — Naturalness parity is descriptive.**
The DNSMOS P.808 3.93 / 3.93 and UTMOS 3.012 / 2.997 parity numbers
are reported as descriptive equality within a stated tolerance
(±0.00 at 3 sig-figs), not as a failure-to-reject inferential claim.
N = 14 real-mic reference recordings + N = 10 clone generations
are insufficient for informative hypothesis tests; parity as
descriptive equality is the defensible framing.

### §8.5 Theoretical-claim limitations

**[MANDATORY] L-DDA-Independence — DDA orthogonal-signal count
assumes embedder independence.** The derived-data-abundance headline
("13 embedders yield up to 91 structured signals per input") counts
$N + \binom{N}{2} = 91$ under Axiom 2 pairwise-independence
assumption. Empirical pairwise-mutual-information measurement across
the 13 embedders on Context Graph's corpus (RO-8) is future work.
The count is an upper bound; the empirical redundancy distribution
is unknown.

**[MANDATORY] L-05 — Multi-modal M > 1 has no empirical evidence.**
H2b's multi-modal conjunction-strictness claim is derivationally
true under Axiom 2 at M > 1, but we have **zero T1 measured data at
M > 1**. Case 3 is M = 1; Case 1 has a single text-style modality;
Case 2's four-modality (visual, emotion, speaker, voice) conjunction
accept-rate is T1 PENDING end-to-end. The multi-modal paradigm
rests on a derivational argument, not on measurement at this
revision. Case 2 end-to-end measurement is the highest-priority
future work for empirical validation of the multi-modal framing.

**[MANDATORY] L-ThresholdPortability — Threshold calibration
portability.** Theorem 2's per-output guarantee is contingent on
calibrated thresholds $\tau_m$. Values reported (τ = 0.95 for Case 3
speaker; τ = (0.70, 0.60, 0.80, 0.85) for Case 2 operating)
are empirical observations on author-specific data at
author-specific sessions, calibrated per agent-25 §3.3
(10th percentile of a calibration set). Cross-subject,
cross-session, cross-encoder threshold portability is not
characterised; reimplementers must re-calibrate for their own
data per the canonical procedure.

**[ENHANCE IF ROOM] L-Attribution — TCT operationalises, does not
cause.** The sample-complexity collapse (Theorem 1) is attributable
in non-trivial part to modern frozen embedders' good conditioning
(inherited from retrieval / contrastive-geometry literature). TCT
is the vehicle that exploits this conditioning via
geometric-proximity alignment and per-output verification; the
paper does not claim TCT originates the conditioning property.

**[ENHANCE IF ROOM] L-Compression — Meaning compression is
semantically distinct, not dimensionally.** The "meaning
compression as fourth category" claim is semantic (what is
counted — structured signals rather than Shannon bits), not
dimensional. Both bit-compression (bits/bits) and meaning-compression
(signals/raw-data unit) have the same count/count form.

**[ENHANCE IF ROOM] L-Scope-H4 — Collapse-silent scope.**
DDA's collapse-silence claim applies to Shumailov-2024-form
generator-in-loop recursion. Other forms of embedder-mediated
data-selection feedback (e.g., using embedder scores to filter
ingestion inputs) are out of scope of those specific collapse
theorems and are not analysed here.

### §8.6 Reproducibility / determinism

**[ENHANCE IF ROOM] L-Determinism — GPU floating-point
determinism.** Theorem 2's per-output-verifiability claim is
deterministic by construction (frozen $\phi$, frozen $c_m$, fixed
$\tau$, cosine comparison) **up to GPU floating-point
reproducibility**. Reimplementers should use identical parameters,
inputs, and kernel order at training and inference time to
eliminate the residual fp-noise risk near the threshold boundary.

**[ENHANCE IF ROOM] L-VendorVersioning — Vendor-versioned scorers.**
All T1 MEASURED numbers are tagged to encoder weight SHA-256 and
pipeline version (agent-27 §H manifest). Future vendor releases of
Qwen3-TTS, WavLM-base-plus-sv, ECAPA-TDNN, SigLIP-SO400M, DNSMOS,
or UTMOS may produce different values. Byte-identical re-measurement
against pinned weights is supported by the manifest schema.

---

## Summary line for §8 opener (paste-ready)

> *"We structure our limitations by the four validity frames
> (internal, external, construct, statistical-conclusion) plus a
> theoretical-claim frame. The three threats that drive the tightest
> disclosures are encoder-matched evaluation on Case 3 (the guard
> and the evaluator are the same WavLM model — L-01), missing
> pre-selection distribution on Case 3 (we cannot yet isolate
> best-of-K selection from base Qwen3-TTS + ICL quality — L-02), and
> the absence of M > 1 empirical evidence for multi-modal conjunction
> strictness (Case 2 end-to-end remains PENDING — L-05). Our three
> case studies are exploratory N = 1 demonstrations; we run no
> hypothesis tests, no power calculations, and no cross-case
> meta-analysis, in accordance with the analysis discipline locked
> in our methodology."*

---

## §10 Future Work pointer (agent-40 hands to §10 writer)

Highest-priority next measurements, ordered by threat-closure
value:

1. **W1**: Cross-encoder ECAPA-TDNN + Resemblyzer on Case 3 (closes
   L-01 directly; partially closes VT-C1 construct).
2. **W2**: Pre-selection distribution on Case 3 (closes L-02
   directly; closes VT-I2 LoRA/base-model confound).
3. **W3**: 4-point $n_m$ curve on Case 3 (closes L-03; strengthens
   Axiom 3 evidence).
4. **Case 2 end-to-end T1**: closes L-05 multi-modal evidence gap;
   highest-impact external measurement.
5. **Multi-subject replication**: closes L-Population; cross-subject
   generalisation claim.
6. **Cross-language replication**: closes L-Language.
7. **RO-8 pairwise-MI measurement**: closes L-DDA-Independence
   upper-bound-only qualifier.
8. **Face-recognition cross-encoder on Case 2**: closes L-SigLIP.
9. **ColBERT / BGE-reranker on Case 1**: closes L-STYLE construct
   coverage.
10. **Blinded human Likert**: closes L-Construct-A + L-DNSMOS
    perceptual validation.

---

## Confidence

**HIGH** that the §8 Limitations draft above closes every CRITICAL
and HIGH validity threat identified in
`validity-threats.md`.

**HIGH** that the three-threat headline (L-01, L-02, L-05) correctly
summarises the tightest disclosures the arXiv reader will expect.

**HIGH** that the paste-ready paragraphs are stylistically consistent
with the manuscript's register and can drop into §8 with minimal
editing.

**MEDIUM** on exact space allocation; agent-40 may condense the
ENHANCE-IF-ROOM paragraphs to single sentences if §8 length is tight.

## Changelog

- 2026-04-16 — Initial issue by agent-29-validity-guardian.
