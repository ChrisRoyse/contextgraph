---
namespace: discoveries
agent_id: agent-29-validity-guardian
timestamp: 2026-04-16T18:00:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
  - memory/discoveries/agent-24-causal-simulator--weakness-register.md
  - memory/handoff/agent-28-to-29--analysis-ready.md
downstream_consumers:
  - agent-30-risk-analyst
  - agent-37-methodology-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
tags: validity, internal, external, construct, statistical-conclusion, threats, tct, case-studies
answers_questions: [Q06, Q08, Q15]
---

# Validity Threat Audit — Threat-by-Threat Register

## 0. Purpose and scope

Audit of threats to **internal**, **external**, **construct**, and
**statistical-conclusion** validity across Cases 1–3 and the TCT theoretical
claims (DDA N(N+1)/2 count, per-output guarantee, sample-complexity claim).

Every threat below is either **CONFIRMED** (present under current design,
disclosure required), **MITIGATED** (design or procedure closes the threat),
or **UNCLOSEABLE** (inherent to exploratory N=1 demonstration; must be
acknowledged rather than mitigated).

Threat IDs prefixed **VT-**: VT-I (internal), VT-E (external), VT-C
(construct), VT-S (statistical-conclusion), VT-T (theoretical).

Each entry carries:
- **Severity**: CRITICAL / HIGH / MEDIUM / LOW.
- **Status**: CONFIRMED / MITIGATED / UNCLOSEABLE.
- **Scope**: which case(s) / claim(s) it touches.
- **Disclosure venue**: §6.x (methods/results), §7 (discussion), §8
  (limitations), or footnote.
- **Mitigation**: what the design, instrument spec, or analysis plan
  does to close the threat; "disclosure only" when uncloseable.

---

## 1. Internal validity (alternative explanations)

Internal validity asks: **does the study rule out alternative explanations
for the observed result?** For the causal-inference frame: "does
measurement X support the claim that Y caused Z, or could some other factor
(selection, history, maturation, instrumentation, regression) be
responsible?"

### VT-I1 — Selection bias on Case 3 10-sentence held-out set (LOW, MITIGATED)

**Threat**: 10 held-out sentences could have been selected for easier
pronunciation, higher phonetic regularity, or higher expected WavLM
similarity — inflating the 0.961 headline.

**Status**: **MITIGATED**. Per agent-26 §A.3.5: the 10 sentences were
**authored with zero lexical overlap to the ICL transcripts** and verified
by string and lemma overlap checks. Utterance-type stratification
(declarative / interrogative / exclamatory) was soft-balanced at authoring
time, not resampled to maximise cosine. The reference-centroid session
is **different** from the held-out recording session (W4 audit resolved:
cross-session for generator input, same-session only for DNSMOS/UTMOS
parity comparator).

**Disclosure venue**: §6.3 methodology sentence ("sentences authored
with zero lexical overlap; utterance-type soft-balanced; session
structure disclosed in §8 L-04").

**Residual risk**: author-chosen sentence content is still
author-chosen; some unconscious selection for author-preferred phrasing
cannot be ruled out. Acknowledged in Bias-08 (self-reported).

---

### VT-I2 — LoRA / Qwen3-TTS base-model quality confound on Case 3 (CRITICAL, CONFIRMED)

**Threat (agent-24 W2, upgraded)**: The 0.961 mean SECS on novel sentences
is reported end-to-end from the Qwen3-TTS + Full ICL + 50-clip centroid +
best-of-12 pipeline. Case 3 has **no LoRA step**; the base generator is
inference-only. Under the current design we cannot decompose the 0.961 into:
- (a) contribution of the **base Qwen3-TTS model** (a large proprietary
  TTS that already produces high-similarity speaker transfers from ICL
  alone — the headline may be mostly an artefact of vendor model quality);
- (b) contribution of the **50-clip centroid + ICL conditioning**;
- (c) contribution of the **best-of-12 selection under WavLM scoring** (TCT
  Phase 2 inference-only mode).

The ablation table in agent-28 §4.5 records a +0.094 delta "Full ICL vs
x-vector" and a +0.015 delta "50-clip vs 1-clip centroid" from the
forensic trail, which partly isolates (b) and (c). But the **baseline**
— "Qwen3-TTS with a single random ICL clip, no centroid, no best-of-K"
— is not in the table.

**Status**: **CONFIRMED**. The pre-selection cosine distribution over all
120 (12 × 10) candidates is W2-PENDING (agent-28 §4.7); without it we
cannot report:
- Pre-selection mean $\bar s_{\text{pre}}$.
- TCT-delta $\Delta_{\text{TCT}} = \bar s_{\text{post}} - \bar
  s_{\text{pre}}$ (the headline quantity attributable specifically to
  best-of-K under the guard).

**Scope**: Case 3 headline (0.961 mean, 0.975 max); H2a (guard
efficacy); H3a (sample-complexity collapse); §5.4 TCT attribution;
§1 opener if it cites the number.

**Severity**: **CRITICAL**. This is the single biggest internal-validity
threat in the manuscript. If a reviewer demands "what does TCT add over
the base Qwen3-TTS + ICL pipeline?" we currently cannot answer
quantitatively.

**Disclosure venue**: §6.3 paste-ready sentence near the 0.961 headline;
§8 L-02 paste-ready paragraph (LoRA-only analogue for Case 3 is
"pre-selection distribution"); §5.4 attribution language must soften
from "TCT causes" to "TCT verifies and selectively accepts" (agent-24 W6
instructs).

**Mitigation**:
- **Pre-selection histogram over 120 candidates** (agent-28 §4.7) is
  the instrument-ready W2 remediation. Run status: PENDING. If lands
  by submission, §6.3 gains the TCT-delta row; if not, §8 L-02
  paste-ready disclosure carries.
- **Framing discipline** (mandatory regardless of whether W2 run
  lands): the Case-3 attribution language in §5.4, §6.3, §7, §9 is
  rewritten per agent-24 W6. TCT is the **selection operator** over
  the vendor-model's sample space, not a causal source of the
  identity-lock property.

---

### VT-I3 — Instrumentation circularity on Case 3 (CRITICAL, CONFIRMED)

**Threat (agent-24 W1)**: WavLM-base-plus-sv is **both** the Phase-3
guard (best-of-K selects by WavLM cosine $\geq 0.95$) **and** the
evaluator (the headline 0.961 is the mean of the argmax candidates'
WavLM cosine). This is not instrumentation drift; it is
**measurement-measurement dependence**: the guard selects survivors,
and the survivors by construction score above the selector. The number
we report is the signature of the guard working, not of cross-encoder
identity verification.

**Status**: **CONFIRMED**. No counterfactual reverses this under
current pipeline design.

**Scope**: Case 3 headline; H2a, H3a; §5.4 per-output predicate; §1
opener insofar as it cites the number.

**Severity**: **CRITICAL**. A reviewer will catch this within the
first read.

**Disclosure venue**: §6.3 must carry agent-24's paste-ready single
sentence verbatim: *"The 0.961 mean SECS is an encoder-matched
measurement: the guard and evaluator are both WavLM-base-plus-sv. This
demonstrates the guard behaves as specified; it does not constitute
cross-encoder identity verification."* §8 L-01 paste-ready carries
the cross-encoder remediation plan and the contingency paragraph.

**Mitigation**:
- **W1 remediation** (agent-25 §3.5 + agent-27 A.2 + agent-28 §4.6):
  re-score the 10 argmax candidates with **ECAPA-TDNN** (and
  optionally Resemblyzer) on the same audio. Report mean / max,
  Pearson + Spearman agreement, Bland-Altman plot. Status:
  instrument-ready, run PENDING.
- **Acceptable fallback if W1 does not land by submission**: §6.3
  disclosure plus §8 L-01 paragraph reframing H2a as
  "**within-WavLM-family** identity agreement, 0.961 mean, 9/10 $\geq
  0.95$", not as a general identity-quality claim (agent-24 W1
  remediation path).

---

### VT-I4 — Historical comparison confound on Case 2 flow-matching sign ablation (MEDIUM, MITIGATED with caveat)

**Threat**: The Case-2 ablation table (agent-28 §3.4 item 3) cites the
pre-fix flow-matching-sign-inverted baseline from the forensic audit
doc (`docs/inspect/08_training_audit.md` in the Santa project) against
the post-fix pipeline numbers. This is a **historical reference**, not a
side-by-side re-run; any intervening change (LoRA rank, batch size,
dataset version) between the pre-fix forensic run and the post-fix
run could be a confound for the ablation delta.

**Status**: **MITIGATED with caveat**. Agent-28 §3.4 item 3 explicitly
labels this row as "historical reference, not a fresh run". The table
presentation discipline prevents misreading as a controlled ablation.

**Scope**: Case 2 §6.2 ablation table; §7 Discussion if it cites the
flow-matching delta.

**Severity**: **MEDIUM**. The methodological discipline closes the
threat operationally (reader knows it's historical), but the
ablation's causal attribution remains weaker than a controlled
re-run.

**Disclosure venue**: §6.2 table row label; §8 L-02-Case2-equivalent
paste-ready paragraph.

**Mitigation**:
- **Table labelling**: "historical reference (pre-fix audit trail)"
  row descriptor, not "controlled ablation".
- **Stretch**: if a post-submission controlled re-run is feasible,
  upgrade the row to T1 MEASURED in v2.

---

### VT-I5 — Carry-over effects across temperature sweep (Case 3) (LOW, MITIGATED)

**Threat**: Case 3 temperature sweep (T ∈ {0.2, 0.3, 0.4, 0.5, 0.6,
0.7, 0.8}) is a single-subject, same-ICL-context, different-T
ablation. If the 12 candidates per T are drawn from a shared sampling
state (seed chain), candidate independence across T is not guaranteed.

**Status**: **MITIGATED**. Each candidate generation is an
independent forward pass with a distinct seed (varying-seed under
fixed T per agent-25 §2.6.1a). No autoregressive state leaks across
candidates or temperatures. Agent-28 handoff §1 confirms:
"independence-of-candidates claim at inference time; no training
carry-over".

**Scope**: Case 3 temperature-sweep ablation row.

**Severity**: **LOW**.

**Disclosure venue**: Footnote on Table 6.3 if cited.

**Mitigation**: Seed log in provenance manifest (agent-27 §H) records
per-candidate seed; agent-43 reproducibility check verifies seed
uniqueness.

---

### VT-I6 — Forensic fix confounds in all three cases (MEDIUM, CONFIRMED with disclosure)

**Threat**: All three cases embed forensic bug fixes in the pipeline
(Case 1 training-data echo-bug; Case 2 flow-matching sign-bug +
q_audio exclusion; Case 3 protocol fixes embedded in SITREP trail).
Without the forensic fixes, the numbers would be different. The
manuscript's headline numbers are **conditional on correct pipeline
implementation**, not "the TCT method in the abstract". A reviewer
reading the appendix will see the forensic trail and may ask: how much
of the result is the forensic fix, how much is TCT?

**Status**: **CONFIRMED with disclosure**. Agent-25 §1.5 (FSV 15/15
for Case 1), §2.4 (Case 2 forensic fixes), §3.1 (Case 3 SITREP
trail) document the fixes. Agent-24 W8–W10 are not about this threat,
but agent-14 Bias-08 (self-reported) and FM-04 (measured/projected)
tangentially cover pipeline-implementation-contingent claims.

**Scope**: All three cases; §6 opener for each case; §7 Discussion
reproducibility claim.

**Severity**: **MEDIUM**. The fixes are documented; the reader is not
being misled. But the "a TCT pipeline built from scratch will
reproduce these numbers" claim is stronger than what the evidence
supports.

**Disclosure venue**: §6.1 / §6.2 / §6.3 case openers must state:
"headline numbers are conditional on the forensic fixes documented in
[audit doc]". §8 L-Forensic paragraph.

**Mitigation**:
- **Documentation**: agent-27 §H JSON manifest includes
  `embedder_version_hash`, `centroid_hash`, `threshold_set_hash` —
  byte-identical reproducibility is available.
- **Framing**: paper claims "re-implementable protocol producing these
  numbers **under the documented fixes**"; does not claim "TCT from
  scratch reproduces these numbers naïvely".

---

## 2. External validity (generalisability)

External validity asks: **to whom / where / when do these results
generalise?** For N=1 exploratory demonstrations, the honest answer is
"nowhere; the scope is this subject, this session, this language, this
encoder pair, this session." The threat register below lists the
specific failure modes a reviewer will probe.

### VT-E1 — Population validity: single subject across all three cases (CRITICAL, UNCLOSEABLE)

**Threat (Bias-05, FM-05)**: Cases 2 and 3 are N=1 subject (the
author). Case 1 is a single-pipeline study on a single corpus
(Shakespeare). No case has multi-subject, multi-speaker, or
multi-author generalisation. Claims about TCT's identity-lock
efficacy are conditional on the one-subject evidence.

**Status**: **UNCLOSEABLE** under the current manuscript scope. Adding
subjects requires engineering work outside the arXiv v1 timeline.

**Scope**: All three case studies; §6 openers; §7 Discussion §8
Limitations; §9 Conclusion; §10 Future Work.

**Severity**: **CRITICAL** (for generalisation claims); **zero** (if
no generalisation is claimed, which is our discipline).

**Disclosure venue**: §6.3 opener, §6.2 opener (Bias-05
paste-ready); §8 L-Population paragraph; §9 Conclusion absent any
cross-subject claim.

**Mitigation**: **Disclosure only**. Paste-ready line (agent-26 §F):
*"Our three case studies are exploratory demonstrations. No case
supports generalisation beyond its documented scope."* §10 Future
Work: multi-subject replication is the highest-priority next
measurement.

---

### VT-E2 — Single-language (English) across all three cases (HIGH, UNCLOSEABLE)

**Threat (Bias-09)**: All three cases are English. WavLM is
multilingual-pretrained; the 0.961 SECS is measured on English. No
cross-language evidence exists.

**Status**: **UNCLOSEABLE** for v1.

**Scope**: All cases; §8 Limitations.

**Severity**: **HIGH** for any generalisation claim; **zero** if we
restrict to English.

**Disclosure venue**: §6.3 methodology ("N=1 language"); §8 Bias-09
paste-ready paragraph.

**Mitigation**: Disclosure only. §10 Future Work: cross-language
replication.

---

### VT-E3 — Single-microphone / single-recording-condition (Case 3) (HIGH, CONFIRMED + PARTIALLY MITIGATED)

**Threat (Bias-09)**: The 50-clip reference centroid is from the
16 min video source (one microphone, one room, one day). The 10 held-
out sentences are recorded with a real microphone in a different
session (different room, different day, different mic). The DNSMOS /
UTMOS / WavLM parity scorers compare against real-mic recordings from
the held-out session. Therefore:
- Reference-centroid session ≠ evaluation session (cross-session for
  generator input — this is a **strengthener**, not a weakness).
- Evaluation session = parity-comparator session (same-session for
  naturalness parity — this is a **necessary design** but means
  parity is measured against a condition the evaluator has already
  seen).

**Status**: **CONFIRMED with nuanced disclosure** (W4 resolved at
agent-26 §A.3.5 level). The cross-session structure for the
generator input is explicitly a strengthener; the same-session
structure for the parity scorer is an accepted design.

**Scope**: Case 3 §6.3; §8 L-04.

**Severity**: **HIGH** (if collapsed into "single session"); **MEDIUM**
given the nuanced cross-session structure.

**Disclosure venue**: §6.3 methodology paragraph must state the session
structure verbatim: *"Reference-centroid session differs from
held-out-sentence session (cross-session for generator input).
Held-out-sentence session coincides with real-mic parity-comparator
session (same-session for DNSMOS / UTMOS / WavLM parity scoring)."* §8
L-04 paste-ready.

**Mitigation**:
- **Documented cross-session structure** (agent-26 §A.3.5 + §D.3) —
  partially mitigates the single-session concern by having the
  reference and held-out sessions differ.
- **Stretch**: include one cross-session held-out clip for parity if
  available.

---

### VT-E4 — Ecological validity: laboratory / author-read clean audio (Case 3) (HIGH, UNCLOSEABLE)

**Threat (Bias-09, "benchmark monoculture")**: Case 3 evaluates
clean-audio TTS on author-read English sentences in a single
acoustic environment. Real-world TTS deployment involves noisy
microphones, background interference, codec compression, variable
SNR. The 0.961 SECS under clean author-read conditions does not
generalise to noisy or codec-compressed conditions.

**Status**: **UNCLOSEABLE** for v1.

**Scope**: Case 3; §8 Limitations.

**Severity**: **HIGH** for deployment claims; **zero** if we restrict
to clean-audio author-read.

**Disclosure venue**: §8 Bias-09 paste-ready; §10 Future Work.

**Mitigation**: Disclosure only.

---

### VT-E5 — Temporal validity: vendor-versioned scorers (MEDIUM, PARTIALLY MITIGATED)

**Threat**: Qwen3-TTS, WavLM-base-plus-sv, ECAPA-TDNN, SigLIP,
DNSMOS, UTMOS are all vendor-distributed models whose weights may
change across releases. The 0.961 is anchored to measurement date
2026-04-16 and pipeline version `qwen3-tts-foricl-v1`; future
vendor updates may move the number.

**Status**: **PARTIALLY MITIGATED**. Agent-27 §H JSON manifest
requires `embedder_version_hash` (weight SHA-256) recorded at run
time per encoder. Byte-identical re-measurement is possible given the
recorded hashes.

**Scope**: All cases; §6 methodology; §8 Limitations.

**Severity**: **MEDIUM** (reproducibility-bounded by vendor-version
pinning).

**Disclosure venue**: §6.3 methodology paragraph ("All measurements
tagged to embedder weight SHA-256 and pipeline version; future
vendor releases may produce different values."); §8 Reproducibility
paragraph.

**Mitigation**: Weight-hash recording (agent-27 §H); date + pipeline
version stamped on every T1 MEASURED number (agent-28 §1
"Date and version" convention).

---

### VT-E6 — Population validity on Case 1: single corpus (Shakespeare) (MEDIUM, UNCLOSEABLE)

**Threat**: Case 1 evaluates one pipeline on one canonical corpus
with one guard-encoder panel. "Shakespearean" style is treated as a
single target; no claim extends to, e.g., Milton, King James Bible,
Chaucer, or other early-modern-English corpora.

**Status**: **UNCLOSEABLE** for v1.

**Scope**: Case 1; §8 Limitations.

**Severity**: **MEDIUM**.

**Disclosure venue**: §8 paste-ready (agent-26 §A.1.6 verbatim).

**Mitigation**: Disclosure only.

---

## 3. Construct validity (do our measures measure what we claim?)

Construct validity asks: **does the operationalisation actually capture
the intended construct?** For identity-locked generation, "identity" is
an abstract construct that we operationalise as geometric proximity to a
frozen-encoder centroid. The threats below concern the gap between the
construct and the operationalisation.

### VT-C1 — Circular construct on Case 3 WavLM SECS (CRITICAL, CONFIRMED)

**Threat (agent-27 L-SECS-A, agent-24 W1, upgraded)**: Our operational
definition of "identity-locked" in Case 3 is "WavLM-base-plus-sv
cosine $\geq 0.95$ to a centroid of 50 clips". We then measure identity
lock by computing the same WavLM cosine on the survivors of the guard.
**The measurement is the acceptance criterion**; the two cannot be
independently validated within this case.

Construct-validity question: does "WavLM cosine $\geq 0.95$ to centroid"
measure (a) **speaker identity** (the intended construct), or (b)
"the property of having a WavLM embedding close to this centroid"
(a tautological property of the measurement)?

A healthy construct validation would require: an **independent indicator**
of the construct that is not the measurement itself (e.g., naïve human
listener judgement; a second, architecturally-independent encoder; a
behavioural measure like "can the output fool an unfamiliar speaker-
verification system").

**Status**: **CONFIRMED**. No independent construct indicator has
been measured within Case 3 under current design. The W1 remediation
(cross-encoder ECAPA/Resemblyzer) is the construct-triangulation
path; status PENDING.

**Scope**: Case 3 headline; H2a; the meaning of "identity-locked" in
§4 definition and §5.4 derivation.

**Severity**: **CRITICAL**. This is the dominant construct-validity
threat; it is also the dominant internal-validity threat (VT-I3);
the two threats share the same root cause (single-encoder
self-consistency).

**Disclosure venue**: §6.3 paste-ready L-SECS-A sentence (verbatim
agent-27 L-SECS-A); §8 L-01 paste-ready; §4 construct definition
must carry the explicit qualifier "operationalised as geometric
proximity to a frozen-encoder centroid, not as perceptual / psychological
identity".

**Mitigation**:
- **W1 cross-encoder**: ECAPA-TDNN + Resemblyzer on the same 10
  argmax candidates (agent-28 §4.6). Agreement (Pearson + Spearman
  + Bland-Altman) is the construct-triangulation signal. Status:
  PENDING.
- **Framing discipline** (mandatory regardless): agent-04's
  identity-locked definition is geometric-proximity-to-centroid, not
  perceptual identity. Agent-29 (this report) confirms no §6 text
  slips into "perceptual identity" / "psychological identity" claims.
- **Human naïve-listener protocol** (optional, future work, out of
  scope for v1 per agent-28 §7 item 5).

---

### VT-C2 — DNSMOS P.808 and UTMOS proxy-for-human-naturalness (HIGH, CONFIRMED)

**Threat (agent-27 L-DNSMOS-A, L-UTMOS-A)**: DNSMOS and UTMOS are
**neural proxies** trained on human-labelled MOS data (DNS-Challenge
for DNSMOS; BVCC + VCC for UTMOS). They are not direct measurements
of human naturalness perception; they are **learned predictors** of
human MOS labels. The "parity with real recordings" claim (DNSMOS
3.93 / 3.93; UTMOS 3.012 / 2.997) is parity on the **predictor's
output**, not parity in **human-perceived naturalness**.

Additionally, DNSMOS was trained on noise-suppression output, not on
TTS. Extrapolation to TTS-generated audio is not validated in the
DNSMOS source paper (Reddy et al. 2022). UTMOS was trained on TTS/VC
data and is better-suited to TTS — but still a predictor, not a
direct measurement.

**Status**: **CONFIRMED**.

**Scope**: Case 3 naturalness parity claim; §6.3; §8 Limitations.

**Severity**: **HIGH**. A human-perception reviewer will push on this.

**Disclosure venue**: §6.3 methodology paragraph: *"DNSMOS and UTMOS
are neural proxies for human MOS; parity values are predictor-output
parity, not validated against direct human naturalness judgement."*
§8 L-DNSMOS-A + L-UTMOS-A paste-ready verbatim from agent-27.

**Mitigation**:
- **Two independent scorers**: DNSMOS (P.808) + UTMOS, both
  trained on different data distributions, provide modest
  triangulation. Both agreeing at parity is weak evidence the parity
  claim is not purely an artefact of one scorer's training
  distribution.
- **WAV-only pipeline** (no codec compression) avoids one known
  failure mode of both scorers (codec sensitivity).
- **Disclosure**: framing explicitly names the predictor
  limitations.
- **Human blinded-Likert** (optional, out of scope for v1 per
  agent-28 §7 item 5).

---

### VT-C3 — SigLIP visual centroid cosine as "same face" measure (Case 2) (HIGH, CONFIRMED)

**Threat (agent-27 L-SigLIP-A, L-SigLIP-B)**: Case 2's visual-identity
operationalisation is SigLIP-SO400M cosine $\geq 0.70$ to a
per-frame-pooled visual centroid. SigLIP was trained on web-scale
image-text pairs (WebLI); its similarity metric reflects
**visual-text retrieval utility**, not "same-face identity". A pair
of images that have high SigLIP cosine may share visual gestalt
(lighting, composition, background, pose) without being the same
face; conversely, same-face images under dramatically different
lighting / angle may have low SigLIP cosine.

A dedicated face-recognition encoder (ArcFace, MagFace, FaceNet)
would measure face identity more directly. SigLIP is a compromise
(general visual encoder, easy to run, interpretable) but the
construct it measures is "visual-embedding proximity", not "same
face".

**Status**: **CONFIRMED**. T1 numbers for Case 2 are T1-PENDING, but
the construct-validity concern is design-level, not measurement-level.

**Scope**: Case 2 visual modality; §6.2 table; §8 Limitations.

**Severity**: **HIGH** (for face-identity claims); **MEDIUM** (for
visual-coherence claims, which are the honest framing).

**Disclosure venue**: §6.2 methodology paragraph and §8 L-SigLIP-A
paste-ready verbatim from agent-27.

**Mitigation**:
- **Face-crop preprocessing**: YOLOv5-Face detection plus mouth-landmark
  requirement in the ClipCannon pipeline (agent-26 §A.2.2) restricts
  SigLIP input to face-region crops, tightening the construct from
  "visual scene similarity" to "face-region embedding proximity".
- **Framing discipline**: §6.2 names the measure "visual-identity
  cosine (SigLIP)" not "face recognition"; §4 construct definition
  qualifies that identity is operationalised per-modality as
  centroid-cosine in the respective encoder's embedding space.
- **Future work**: ArcFace / FaceNet cross-encoder as W1-equivalent
  for Case 2.

---

### VT-C4 — 9-embedder STYLE_CENTROID as "Shakespearean style" measure (Case 1) (MEDIUM, PARTIALLY MITIGATED)

**Threat (agent-27 L-STYLE-A, L-VOCAB-A)**: Case 1 operationalises
"Shakespearean style" as conjunction of 9 per-embedder cosines against
centroids of 50–100 Shakespeare passages + a vocabulary-density
prefilter. The 9 embedders are architecturally diverse (dense
retrieval, late-interaction, sparse) — a strength for construct
triangulation. But:
- **Mono-corpus centroid**: all embedders share the same reference
  corpus (Shakespeare canonical passages). Concerns about "what is
  Shakespearean" are encoded once in the corpus, re-measured by 9
  embedders — this is "same-construct 9 times", not 9 independent
  constructs.
- **No rhythm / meter / rhetorical-device measurement**: iambic
  pentameter, caesura, anaphora, chiasmus — these are canonical
  markers of Shakespearean verse that no dense embedder is trained to
  capture directly. The vocabulary-density prefilter catches archaic
  words but not syntax or rhythm.
- **Cross-encoder W1-equivalent** (ColBERT or BGE-reranker): not in
  the 9-embedder panel; status PENDING.

**Status**: **PARTIALLY MITIGATED** by the 9-embedder panel's
architectural diversity. But the "Shakespearean-ness" construct
retains known gaps (syntax, rhythm, rhetorical devices).

**Scope**: Case 1; §6.1 table; §8 Limitations.

**Severity**: **MEDIUM**.

**Disclosure venue**: §6.1 paragraph naming the guard's strengths
(lexical, semantic, sparse retrieval) and gaps (syntax, meter,
rhetoric); §8 L-STYLE-A + L-VOCAB-A paste-ready from agent-27.

**Mitigation**:
- **9-embedder architectural diversity** (partial construct
  triangulation).
- **Vocabulary-density prefilter** (catches lexical archaic markers).
- **Future work**: ColBERT / BGE-reranker cross-encoder W1-equivalent;
  meter and rhetorical-device detection.

---

### VT-C5 — "Identity-locked" construct conflation across modalities (MEDIUM, MITIGATED by definition)

**Threat**: The theoretical construct "identity-locked" (agent-04
definition) is geometric-proximity-to-centroid across modalities. In
Case 3 the only modality is "speaker" (1-modality conjunction reduces
to 1-term argmax). In Case 2 it is four modalities (visual, emotion,
speaker, voice). A reader may conflate "identity-locked at M=4" with
"identity-locked at M=1"; they are different operational strengths.

**Status**: **MITIGATED** by agent-04's definition (which is
explicitly per-modality conjunction, not a single aggregate score).
Construct clarity depends on §4 / §5.4 language being faithful to
the definition.

**Scope**: §4 construct; §5.4 derivation; §6.2 / §6.3.

**Severity**: **MEDIUM**.

**Disclosure venue**: §4 construct definition; §5.4 derivation;
§6 openers.

**Mitigation**: Definition-level discipline; agent-29 (this report)
confirms no §6 text collapses across M.

---

### VT-C6 — Derived Data Abundance: "orthogonal labeled training signals" as construct (HIGH, CONFIRMED)

**Threat (agent-24 W5, W8, upgraded)**: The headline DDA count is
"N + C(N,2) orthogonal labeled training signals per input" (e.g.,
91 for N=13). The **construct** being counted is "orthogonal labeled
signal": a per-embedder vector plus a pair-interaction feature that
carries non-redundant information beyond the union of the two
constituents. The construct presumes **embedder independence** — if
two embedders (e.g., E1 semantic + E10 paraphrase) are substantially
redundant, their pairwise interaction carries little information, and
the count 91 is an upper bound, not an observation.

No formal pairwise-MI measurement exists on Context Graph's corpus
(RO-8 future work per agent-24 W5). The construct "orthogonal signal"
is therefore **assumed** rather than measured.

**Status**: **CONFIRMED**. No empirical independence measurement has
been run.

**Scope**: §3.4 DDA counting; §4.1 Context Graph specifics; H1,
H1a, H1b; §8 Limitations.

**Severity**: **HIGH** (for the literal "91 signals/input" claim);
**MEDIUM** (if softened to "up to 91, subject to empirical
redundancy").

**Disclosure venue**: §3.4 headline must say "up to 91" not "91";
§8 L-04 (agent-24 W5 naming) paste-ready; §10 Future Work RO-8
lifted.

**Mitigation**:
- **Language tightening**: "up to 91" not "91"; "~N + C(N,2)
  orthogonal signals **assuming pairwise independence**" not
  "orthogonal signals".
- **RO-8 stretch measurement**: sample 100–1000 inputs, compute
  pairwise MI across 13 embedders, report redundancy histogram.
  Status: future work.

---

### VT-C7 — Meaning compression as a "compression category" (MEDIUM, MITIGATED by language tightening)

**Threat (agent-24 W8)**: The claim "meaning compression is
dimensionally distinct from bit-, weight-, activation-compression" is
load-bearing for the paper's taxonomic contribution. Under unit
analysis, bit-compression is bits/bits (count/count) and
meaning-compression is signals/raw-data-unit (count/count) —
**dimensionally the same form**. The distinctness is semantic (what
is counted), not dimensional.

**Status**: **MITIGATED by language tightening** (agent-24 W8
remediation).

**Scope**: §3.1; §5.2; §9 Conclusion; H5.

**Severity**: **MEDIUM**.

**Disclosure venue**: §5.2 language change: "dimensionally distinct"
→ "semantically distinct / measured as structured-signal density
rather than Shannon bits".

**Mitigation**: Targeted wording change. No new measurement needed.

---

## 4. Statistical-conclusion validity (are the stats justified?)

Statistical-conclusion validity asks: **are the statistical claims
supported by the data?** For our exploratory N=1 design,
agent-28's analysis plan has **pre-emptively excluded** the most
common statistical-conclusion threats (Type-I / Type-II error,
multiplicity, power). The threats below are the residual concerns.

### VT-S1 — N=10 is too small for inferential claims on Case 3 (MITIGATED by analysis plan)

**Threat**: N=10 held-out sentences in Case 3 is below any
conventional threshold for hypothesis testing. Parametric tests
(t-test) assume normality that 10 samples cannot establish;
non-parametric tests (Wilcoxon) lack power at N=10.

**Status**: **MITIGATED**. Agent-28 §4.4 + §4.10 explicitly
**refuse** all hypothesis tests on Case 3: no Wilcoxon, no
paired t-test, no χ², no p-values. "Parity" claims are framed as
descriptive equality within stated tolerance (±0.00 at 3 sig-fig),
not as failure-to-reject.

**Scope**: §6.3; §7.

**Severity**: **Zero** given the discipline.

**Disclosure venue**: Agent-28 §1 + §4.10 convention is enforced by
agent-42 adversarial review.

**Mitigation**: Agent-28 analysis discipline closes the threat.

---

### VT-S2 — Retrospective bootstrap CI on Case 3 N=10 (MITIGATED by verbatim framing)

**Threat**: A 95% bootstrap CI on the mean SECS over N=10 (reported as
approximately [0.951, 0.971]) may be mis-read as a power-based
inferential interval. Small-N bootstrap CIs have known coverage
issues at the boundary and under non-iid structure (the 10 sentences
are author-authored, may share linguistic structure).

**Status**: **MITIGATED**. Agent-28 §4.3 mandates verbatim
framing: *"retrospective 95% bootstrap CI, N=10; not a power-based
interval"*. The CI is explicitly documented as a descriptive
summary, not an inferential claim.

**Scope**: §6.3 table; §7 discussion if cited.

**Severity**: **Zero** given the verbatim framing.

**Disclosure venue**: §6.3 table row; footnote on the CI cell.

**Mitigation**: Verbatim framing discipline (agent-28).

---

### VT-S3 — No multiplicity correction across 9 STYLE_CENTROID embedders (MITIGATED by framing)

**Threat**: Case 1's 9-embedder guard produces 9 per-embedder
cosines. Reporting 9 means / medians without multiplicity correction
would be a Type-I error inflation risk — **if** we made significance
claims per embedder. We do not. Agent-28 §1 (multiplicity convention)
specifies: conjunction metric is primary; per-embedder is
descriptive-only. **No per-embedder significance claim** is made.

**Status**: **MITIGATED**. Agent-28 discipline closes the threat.

**Scope**: Case 1 §6.1.

**Severity**: **Zero** given the discipline.

**Disclosure venue**: N/A; agent-42 enforces no per-embedder
significance claims slip in.

**Mitigation**: Agent-28 §1 convention; agent-42 adversarial review.

---

### VT-S4 — Encoder-matched evaluation inflates apparent SECS (CRITICAL, CONFIRMED — overlaps VT-I3 / VT-C1)

**Threat**: Same root cause as VT-I3 (internal) and VT-C1 (construct):
using the guard encoder as the evaluator inflates apparent SECS by
the "self-consistency signature" — the selection process guarantees
survivors score high on the selector. This is a **statistical**
threat distinct from internal/construct framings: even if we accepted
WavLM as a valid identity measure, the *number* 0.961 is
systematically higher than what an unselected sample would show on
the same encoder.

**Status**: **CONFIRMED**. Same remediation (W1 cross-encoder, W2
pre-selection distribution) closes the statistical form too:
- W2 gives pre-selection mean (un-inflated by selection).
- W1 gives cross-encoder value (un-inflated by same-encoder
  dependence).

**Scope**: Case 3; overlaps VT-I3, VT-C1.

**Severity**: **CRITICAL** (triple-threat: internal, construct,
statistical).

**Disclosure venue**: §6.3 L-SECS-A paste-ready (single sentence);
§8 L-01 + L-02 paste-ready paragraphs.

**Mitigation**: W1 + W2 remediation (PENDING). Disclosure fallback if
remediation does not land.

---

### VT-S5 — Bootstrap non-iid sentence structure (LOW, MITIGATED by framing)

**Threat**: The 10 held-out sentences were authored by the same
person and may share linguistic structure (author's vocabulary,
phrasing preferences). Bootstrap resampling assumes iid; non-iid
structure underestimates the true CI width.

**Status**: **MITIGATED** by framing (same as VT-S2: descriptive,
not inferential).

**Scope**: §6.3 CI cell.

**Severity**: **LOW**.

**Disclosure venue**: Footnote on CI cell if raised.

**Mitigation**: Verbatim "retrospective, not power-based" framing.

---

### VT-S6 — Ablation deltas reported without uncertainty (LOW, MITIGATED by framing)

**Threat**: Case 3 ablation table (agent-28 §4.5) reports point
estimates (+0.094, −0.013, +0.015) without CIs. A reviewer may ask
for uncertainty.

**Status**: **MITIGATED**. Agent-28 §4.5 explicitly frames deltas
as "descriptive point estimates from the forensic run set", with no
inferential claim attached. CI-on-delta is not reported because N is
too small and the deltas are not hypothesis-tested.

**Scope**: Case 3 §6.3 ablation table.

**Severity**: **LOW**.

**Disclosure venue**: Table footnote if raised.

**Mitigation**: Framing; agent-28 discipline.

---

## 5. Theoretical-claim validity (threats specific to TCT / DDA theorems)

Claims are **Axiom 1** (frozen-encoder cosine is deterministic),
**Axiom 2** (approximate embedder independence), **Axiom 3** (modern
frozen embedders are well-conditioned enough that $n_m \in [10, 10^2]$
suffices), **Theorem 1** (sample-complexity collapse), **Theorem 2**
(per-output verifiable predicate).

### VT-T1 — DDA N(N+1)/2 count assumes embedder independence (HIGH, CONFIRMED)

**Threat**: Identical to VT-C6 at the theoretical level. The
N + C(N,2) = N(N+1)/2 count (91 for N=13) presumes pairwise
independence of embedders (Axiom 2). Real embedders have unknown
redundancy. The theoretical headline count is an upper bound, not
an empirical observation.

**Status**: **CONFIRMED**. No formal independence measurement has
been run (RO-8 future work).

**Scope**: §3.4; §4.1; H1; §10 Future Work.

**Severity**: **HIGH**.

**Disclosure venue**: §3.4 language tightening ("up to 91 under
Axiom 2 pairwise independence; empirical independence is RO-8 future
work"); §8 L-DDA-Independence paste-ready; §10 Future Work RO-8
lifted.

**Mitigation**: Language tightening + RO-8 future work.

---

### VT-T2 — TCT per-output guarantee depends on threshold calibration (HIGH, CONFIRMED)

**Threat**: Theorem 2 (per-output verifiable predicate) states the
guard $G_\tau(\hat y; C_P)$ is deterministic and verifiable. True by
construction **given** $\tau$. The guarantee is contingent on $\tau$
being chosen validly. Agent-25 §3.3 specifies the "same-session-band
floor" calibration (10th percentile of a calibration set). The
calibration is **empirical** — the 0.95 threshold in Case 3, the
(0.70, 0.60, 0.80, 0.85) thresholds in Case 2 are observed values
on author-specific data, not universal floors. A different subject /
session / encoder pair will produce different calibration values.

**Status**: **CONFIRMED**. Threshold portability across subjects /
sessions / encoders is a known gap (V12 hedge, agent-27 L-SECS-C).

**Scope**: §5.3; §5.4 Theorem 2; §6.2 / §6.3; §8.

**Severity**: **HIGH** for universality claims; **zero** for
per-pipeline per-output guarantees.

**Disclosure venue**: §5.4 Theorem 2 statement must carry: "per-output
guarantee **contingent on calibrated threshold**; threshold values are
per-subject / per-session / per-encoder empirical observations, not
universal constants". §8 V12 hedge paragraph.

**Mitigation**:
- **Calibration procedure** specified (agent-25 §3.3; 10th percentile
  of calibration set) — reproducible even if values change.
- **Framing discipline**: per-output guarantee is a **per-pipeline
  property**, not a universal property of TCT.
- **Future work**: cross-subject threshold-portability study.

---

### VT-T3 — Sample-complexity claim rests on single empirical data point (HIGH, CONFIRMED)

**Threat (agent-24 W3)**: Axiom 3 asserts $n_m \in [10, 10^2]$
suffices. The empirical evidence is **one data point**: $n_m = 50$
on Case 3 (one subject, one modality). Points at $n_m \in \{10, 25,
100\}$ do not exist. The "curve" is a single observation.

**Status**: **CONFIRMED**. W3 remediation (4-point $n_m$ curve)
PENDING.

**Scope**: §3.6; §5.4 Theorem 1; §6.3; §8.

**Severity**: **HIGH**.

**Disclosure venue**: §5.4 Theorem 1 statement: "Case 3 supplies the
single T1 operating point at $n_m=50$ on one modality; the
sample-complexity curve is RO-10 future work". §8 L-03 paste-ready
for single-operating-point limitation.

**Mitigation**:
- **W3 run**: $n_m \in \{10, 25, 50, 100\}$ ablation on Case 3
  (agent-28 §4.8). Status: PENDING. If lands, 4-point curve
  replaces single-point claim; if not, §8 L-03 disclosure.

---

### VT-T4 — TCT causal attribution vs. embedder conditioning inheritance (MEDIUM, MITIGATED by language)

**Threat (agent-24 W6)**: The sample-complexity collapse is causally
attributed to "TCT operating on DDA-enriched data", but the
non-trivial content reduces to Axiom 3: *modern frozen embedders are
well-conditioned enough that $n_m = 50$ suffices*. This is a property
of **the embedders** (inherited from the retrieval / contrastive
geometry literature that trained them), not of **TCT**. TCT
operationalises the collapse; it does not cause it.

**Status**: **MITIGATED** by language tightening (agent-24 W6
remediation).

**Scope**: §3.6; §5.4; §7 Discussion; §9 Conclusion.

**Severity**: **MEDIUM**.

**Disclosure venue**: §9 Conclusion language: "TCT
operationalises the sample-complexity collapse implicit in
well-conditioned frozen embedders", not "TCT causes sample-complexity
collapse". §7 Discussion credits the frozen-embedder conditioning
literature.

**Mitigation**: Rewrite attribution language in §3.6, §5.4, §7, §9.

---

### VT-T5 — Multi-modal strictness claim has no M>1 empirical evidence (HIGH, CONFIRMED)

**Threat (agent-24 W7)**: H2b ("multi-modal conjunction is strictly
stronger than single-modality check") is derivationally true under
Axiom 2 for M>1, but we have **zero T1 measured data** at M>1. Case 3
is M=1; Case 2's M=4 is T1 PENDING end-to-end. The multi-modal
paradigm rests on a single derivational argument, not on measurement.

**Status**: **CONFIRMED**.

**Scope**: §5.4 multi-modal strictness; §6.2 Case 2 (projected);
§7 Discussion; §8 Limitations.

**Severity**: **HIGH**.

**Disclosure venue**: §5.4 must state: multi-modal strictness is
**derivational**; $M>1$ empirical measurement is pending Case 2.
§6.2 (Case 2 projected) must flag this as the critical pending
measurement. §8 L-05.

**Mitigation**:
- **Case 2 T1 run**: highest-priority pending measurement; status
  PENDING.
- **Fallback**: disclose as pending, flag as measurement that would
  most strongly validate the multi-modal framing.

---

### VT-T6 — Theorem 2 determinism up to GPU floating-point noise (LOW, MITIGATED by footnote)

**Threat (agent-24 W10)**: Theorem 2's determinism claim is true by
construction (frozen $\phi$, frozen $c_m$, fixed $\tau$, cosine
comparison) but in practice GPU fp nondeterminism can cause tiny
cosine fluctuations that may flip near-threshold decisions.

**Status**: **MITIGATED** by footnote language.

**Scope**: §5.3 guard definition.

**Severity**: **LOW**.

**Disclosure venue**: §5.3 footnote: *"Deterministic modulo GPU
floating-point reproducibility; implementation uses identical
parameters, inputs, and kernel order at training and inference time."*

**Mitigation**: Footnote + optional 10-replicate guard stability run
(agent-24 W10 remediation) reporting decision stability.

---

### VT-T7 — H4 "collapse-silent" scope (MEDIUM, MITIGATED by scope statement)

**Threat (agent-24 W9)**: H4 argues DDA is collapse-silent under
Shumailov-2024 form (no generator-in-loop recursion). If Context
Graph's ingestion uses embedder-derived signals to filter memories,
that is embedder-mediated data-selection feedback — not
generator-recursion, but also not strictly "fixed $D$".

**Status**: **MITIGATED** by scope language.

**Scope**: §3.3; §8.

**Severity**: **MEDIUM**.

**Disclosure venue**: §3.3: "collapse-silent **under Shumailov-2024
form** (generator-in-loop recursion); other forms of embedder-mediated
data selection are out of scope of those specific theorems".

**Mitigation**: Language tightening; optional Context Graph
ingestion audit to confirm fixed-$D$ boundary.

---

## 6. Cross-threat summary table

| ID | Threat | Category | Severity | Status | Remediation |
|---|---|---|---|---|---|
| VT-I1 | Case-3 sentence selection | Internal | LOW | MITIGATED | Zero-lexical-overlap + utterance-type balance |
| VT-I2 | LoRA/Qwen3-TTS confound (= W2) | Internal | CRITICAL | CONFIRMED | W2 pre-selection histogram (PENDING) |
| VT-I3 | WavLM instrumentation circularity (= W1) | Internal | CRITICAL | CONFIRMED | W1 ECAPA/Resemblyzer (PENDING) + §6.3 disclosure |
| VT-I4 | Case-2 flow-matching historical confound | Internal | MEDIUM | MITIGATED | Table label + framing |
| VT-I5 | Case-3 temperature-sweep carry-over | Internal | LOW | MITIGATED | Seed log |
| VT-I6 | Forensic-fix confound (all cases) | Internal | MEDIUM | CONFIRMED | Case-opener disclosure + §8 L-Forensic |
| VT-E1 | N=1 subject (Bias-05) | External | CRITICAL | UNCLOSEABLE | Disclosure only (agent-26 §F paste-ready) |
| VT-E2 | Single language (Bias-09) | External | HIGH | UNCLOSEABLE | Disclosure only |
| VT-E3 | Single microphone / session nuance | External | HIGH | CONFIRMED | §6.3 session paragraph + §8 L-04 |
| VT-E4 | Clean-audio ecological validity | External | HIGH | UNCLOSEABLE | Disclosure only |
| VT-E5 | Vendor-versioned scorers | External | MEDIUM | PARTIALLY MITIGATED | Weight-hash pinning (agent-27 §H) |
| VT-E6 | Case-1 single-corpus | External | MEDIUM | UNCLOSEABLE | Disclosure only |
| VT-C1 | WavLM circular construct (= W1) | Construct | CRITICAL | CONFIRMED | W1 cross-encoder (PENDING) + construct language |
| VT-C2 | DNSMOS/UTMOS neural-proxy | Construct | HIGH | CONFIRMED | Two-scorer triangulation + disclosure |
| VT-C3 | SigLIP "same face" | Construct | HIGH | CONFIRMED | Face-crop preprocessing + disclosure |
| VT-C4 | 9-embedder STYLE_CENTROID | Construct | MEDIUM | PARTIALLY MITIGATED | Architectural diversity + disclosure |
| VT-C5 | Identity-locked cross-modality | Construct | MEDIUM | MITIGATED by definition | §4 / §5.4 discipline |
| VT-C6 | DDA orthogonal-signal independence (= W5) | Construct | HIGH | CONFIRMED | Language ("up to 91") + RO-8 future work |
| VT-C7 | Meaning-compression dimensional claim (= W8) | Construct | MEDIUM | MITIGATED by language | Wording change in §3.1 / §5.2 |
| VT-S1 | N=10 inferential-claim risk | Statistical | Zero given discipline | MITIGATED | No hypothesis tests (agent-28) |
| VT-S2 | Retrospective N=10 bootstrap CI | Statistical | Zero given framing | MITIGATED | Verbatim "retrospective, not power-based" |
| VT-S3 | 9-embedder multiplicity | Statistical | Zero given framing | MITIGATED | Conjunction primary, per-embedder descriptive |
| VT-S4 | Encoder-matched inflation (= W1 statistical form) | Statistical | CRITICAL | CONFIRMED | W1 + W2 (PENDING) + disclosure |
| VT-S5 | Bootstrap non-iid structure | Statistical | LOW | MITIGATED by framing | Footnote if raised |
| VT-S6 | Ablation deltas without CI | Statistical | LOW | MITIGATED by framing | Descriptive framing |
| VT-T1 | DDA independence (= W5, = VT-C6) | Theoretical | HIGH | CONFIRMED | "up to 91" + RO-8 |
| VT-T2 | Threshold calibration portability | Theoretical | HIGH | CONFIRMED | §5.4 language + V12 hedge |
| VT-T3 | n_m single-point claim (= W3) | Theoretical | HIGH | CONFIRMED | W3 curve (PENDING) + §8 L-03 |
| VT-T4 | TCT vs embedder-conditioning attribution (= W6) | Theoretical | MEDIUM | MITIGATED by language | Attribution rewrite in §7 / §9 |
| VT-T5 | Multi-modal M>1 evidence (= W7) | Theoretical | HIGH | CONFIRMED | Case 2 T1 (PENDING) + §8 L-05 |
| VT-T6 | GPU fp determinism (= W10) | Theoretical | LOW | MITIGATED by footnote | §5.3 footnote |
| VT-T7 | H4 "collapse-silent" scope (= W9) | Theoretical | MEDIUM | MITIGATED by language | §3.3 scope statement |

**Criticality concentration**: VT-I2 / VT-I3 / VT-C1 / VT-S4 share the
same root cause (Case-3 single-encoder self-consistency combined with
missing pre-selection distribution); these are the **three tightest
§8 disclosures** and drive the arc of §8 L-01 and L-02.

**Uncloseable concentration**: VT-E1 / VT-E2 / VT-E4 / VT-E6 are all
external-validity threats from the N=1 exploratory-demonstration
design. Disclosure-only under current scope; v2 future work.

---

## 7. Confidence

**HIGH** that the four validity frames (internal, external, construct,
statistical-conclusion) + the theoretical-claim frame cover the threat
space adequately for an exploratory N=1 demonstration of TCT.

**HIGH** that the three critical threats (VT-I2 / VT-I3 / VT-C1 — all
rooted in Case-3 encoder-matched / pre-selection missing) are correctly
identified as the dominant disclosures.

**HIGH** that the uncloseable external-validity threats (single
subject, single language, single microphone, single corpus) are
correctly classified as disclosure-only under current scope.

**MEDIUM** on whether W1 / W2 / W3 remediations land pre-submission;
the plan degrades gracefully to v2 per agent-28 handoff.

## Changelog

- 2026-04-16 — Initial issue by agent-29-validity-guardian.
