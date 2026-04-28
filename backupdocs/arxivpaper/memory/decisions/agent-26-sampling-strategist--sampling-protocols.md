---
namespace: decisions
agent_id: agent-26-sampling-strategist
timestamp: 2026-04-16T09:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/patterns/agent-25-method-designer--weakness-remediation.md
  - memory/handoff/agent-25-to-26--methodology-ready.md
  - memory/decisions/agent-01-step-back-analyzer--failure-modes-to-avoid.md
  - memory/discoveries/agent-14-bias-detector--bias-audit.md
downstream_consumers:
  - agent-27-instrument-developer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
tags: sampling, populations, reference-sets, eligibility, disjointness, power, exploratory-N1, locked
answers_questions: [Q06, Q07, Q08, Q15]
---

# Sampling Protocols — Cases 1–3 and TCT Reference Sets (R_m)

## 0. Purpose and scope

This file **locks** the sampling strategy the manuscript will describe
in §6 (Case studies) and §5 (TCT method, where reference-set
construction is specified). Reviewers reading §6 must be able to
answer, per case: **what population?**, **how were samples drawn?**,
**how many?**, **what stratification?**, **what was held out?**, and
**what does the power calculation say?**

The headline discipline is unambiguous: **our case studies are
exploratory N = 1 (or single-subject / single-session)
demonstrations, not power-calculated population studies.** This is
stated in Part A Point 6 below and then in §8 Limitations verbatim.
No finding is presented as generalising beyond the protocol that
measured it.

Carried hedges: Bias-05 (single-subject, Cases 2 & 3), Bias-06
(single-encoder, Case 3), Bias-08 (self-reported), Bias-09
(benchmark monoculture), FM-05 (hidden single-subject trap),
V11 / V18 / V22 / R3 / R4 from the agent-02 glossary, MP-12
(composition-not-invention).

---

## Part A — Per-case sampling

### A.1 Case 1 — Shakespeare stylistic LoRA

#### A.1.1 Source population

- **Authentic-Shakespeare corpus**: the canonical Shakespeare works
  (plays + sonnets) as edited in the Folger Digital Texts /
  Project-Gutenberg public-domain editions. Size on disk for the
  working copy: 8,857 raw (instruction, response) examples extracted
  by line-level / speech-level segmentation and cleaned per
  `docs/researchpaper/15_appendix_repro.md §151`.
- **Modern-English comparator pool**: Gemma-8B-generated compositions
  about modern topics in Shakespearean style (230 synthetic
  compositions, used as *compositional references*, **not** for
  recursive training — per MP-12 and FM-02).
- **Preference-pair source**: the same 8,857 → 7,908 cleaned set
  feeds both SFT targets and DPO chosen responses; rejected
  responses are modern-English paraphrases of the same semantic
  content.

#### A.1.2 Sampling procedure

1. **Raw extraction** from Folger / Gutenberg public-domain
   Shakespeare plays + sonnets into (instruction, response) pairs.
2. **Cleaning filter**: strip stage directions, scene headers,
   character-name prefixes, location descriptions (the echo-bug
   forensic fix; `docs/researchpaper/15_appendix_repro.md §65`).
   Drop any example whose response is empty after stripping.
3. **Duplicate scan (FSV F5)**: cosine-deduplicate against prior
   held-out evaluation prompts; reject exact-string matches.
4. **Split**: 7,908 cleaned SFT examples retained; the 230
   Gemma-generated compositions added as style exemplars separate
   from the canonical set.
5. **DPO pair construction**: for each of 12,567 pairs, chosen =
   Shakespearean completion from the cleaned corpus, rejected =
   modern-English completion of equivalent semantic content.
6. **Reference set $R_{\text{text-style}}$** (for the TCT centroid):
   50–100 authentic Shakespeare passages sampled across genres
   (plays / sonnets / letters) and character types; embedded with
   E1 (e5-large-v2) per TCT Phase 1.

#### A.1.3 Sample size

| Split | N | Source |
|---|---|---|
| SFT examples (cleaned) | **7,908** | `researchpaper/15_appendix_repro.md §151` |
| SFT composition exemplars | 230 | Gemma-8B synthetic style exemplars |
| DPO contrastive pairs | **12,567** | `docs/FULL_SCALE_LORA_PLAN.md §82` |
| Reference set $R_{\text{text-style}}$ ($n_m$ for TCT) | 50–100 | TCT Phase 1 default (Axiom 3) |
| Held-out evaluation prompts | 20–50 (power-floored; see A.1.6) | §6.1 measurement protocol |
| Calibration set $S_{\text{text-style}}$ | 50–200 | $\tau$ calibration per TCT §3.3 |

#### A.1.4 Stratification

- **Genre stratification** across $R_{\text{text-style}}$: proportional
  representation of plays / sonnets / letters (≈ 70 / 20 / 10).
- **Character-type stratification** within play passages: soliloquy /
  dialogue / aside / letter (≈ 40 / 40 / 10 / 10).
- **DPO pair stratification** by prompt style: declarative / question
  / command / narration (≈ 40 / 20 / 20 / 20).
- **Temporal stratification** (early / middle / late Shakespeare) is
  *not* enforced; early-modern English is treated as a single stylistic
  target per agent-04 `tct-definition.md`.

#### A.1.5 Held-out / evaluation set

- **15-test FSV battery** (F1–F15): already passed; pipeline integrity,
  not quality. See `case-study-protocols.md §1.5`.
- **Content evaluation**: 20–50 held-out prompts, disjoint from SFT
  training set (verified by exact-string and cosine filters at the
  FSV F5 check).
- **Ground-truth style targets** for held-out evaluation are drawn
  from Shakespearean passages *not* used in training or reference-set
  construction.

#### A.1.6 Power considerations

- **We do not run a formal power analysis for Case 1** because the
  headline measurement is mean cosine against $c_{\text{text-style}}$,
  a bounded $[0, 1]$ statistic without a canonical reference effect
  size in the literature.
- **Informal power floor**: $N_{\text{eval,1}} \geq 20$ prompts is
  sufficient for the confidence interval on a mean cosine to be
  $\pm 0.05$ at 95 % under typical within-sample variance ($\sigma
  \approx 0.1$). $N_{\text{eval,1}} \geq 50$ tightens to $\pm 0.03$.
- **What we acknowledge**: Case 1 is an **exploratory
  training-pipeline demonstration**, not a power-calculated study of
  stylistic generalisation. §8 Limitations will state verbatim:
  > *"Case 1 evaluates one pipeline on one canonical corpus (Shakespeare)
  > with one guard encoder (E1). Held-out N is in the tens; the
  > measurement reports mean and distribution of per-prompt style
  > cosine, not a statistically-powered comparison against a published
  > benchmark."*

---

### A.2 Case 2 — Santa identity-locked talking-head clone

#### A.2.1 Source population

- **Single-subject 16 min 15 s interview** of one subject (the
  "Santa" subject). Single recording session; single camera; single
  microphone; single language (English).
- **Clip extraction**: ClipCannon's 23-stage DAG processes the
  16 min source into 2,362 clips (see `case-study-protocols.md §2.2`).
- **Embedding pool**: the Santa constellation NPZ
  (`~/.clipcannon/models/santa/embeddings/all_embeddings.npz`,
  8.8 MB) — sample counts per modality after diarisation filtering
  are documented in Part B below.

#### A.2.2 Sampling procedure

1. **Diarisation**: WavLM-large (Chen et al., 2022) speaker
   embeddings per 3 s speech segment; agglomerative clustering at
   cosine-distance 0.7; **subject-only** segments retained, interviewer
   excluded (`researchpaper/05_system_clipcannon.md §32`).
2. **Face detection**: **YOLOv5-Face** 5-point landmarks
   (TinaChen, 2021; `researchpaper/05_system_clipcannon.md §110`)
   provides the mouth bounding box directly. Face-detection confidence
   threshold applied; low-confidence frames dropped.
3. **Prosody filtering**: ClipCannon `prosody_segments` table filters
   out breath-only / silence segments; retains speaking segments.
4. **Viseme sequencing**: 15-category MPEG-4 viseme sequences derived
   from the CMU pronunciation dictionary; 7,916 real samples, 100 %
   viseme coverage.
5. **Scene bounding**: `scene_map` from ClipCannon provides scene
   boundaries; clips clipped to one scene each.
6. **Quality filter**: audio-video sync by SyncNet LSE-D baseline;
   facial-region occlusion excluded; audio SNR floor applied.
7. **Result**: 2,362 curated clips feed the LoRA training pool;
   modality-specific reference subsets are drawn from this pool.

#### A.2.3 Sample size

| Entity | N | Source |
|---|---|---|
| Source video duration | 16 min 15 s | Mission brief §3 Case 2 |
| Curated clips (post-filter) | **2,362** | `case-study-protocols.md §2.2` |
| Visual embeddings (global) | 1,725 | Part B below |
| Semantic embeddings | 200 | Part B below |
| Emotion embeddings | 343 | Part B below |
| Speaker embeddings | 300 | Part B below |
| Prosody data | 188 | Part B below |
| Sentiment embeddings | 202 | Part B below |
| Voice embeddings | 188 | Part B below |
| FLAME parametric head fits | 3,477 frames × 100 expression coefficients | `appendix_schemas.md §A.6` |
| Aggregate constellation | 3,146 samples × 4,044 dim | `appendix_schemas.md §A.6` |
| Held-out visual scenes (T1 PENDING) | 20–100 (power-floored; see A.2.6) | §6.2 measurement protocol |
| Held-out speech prompts (T1 PENDING) | 20–100 | §6.2 measurement protocol |

**Note on $n_m \in [10, 10^2]$ Axiom 3 guidance**: Case 2 deviates
upward (e.g., visual $n_m = 1{,}725$; emotion $n_m = 343$). The
deviation is disclosed: the 16 min source naturally yields more
than $10^2$ clip-derived samples per modality after filtering. Axiom 3
is the *recommended* range for centroid stability, not a hard upper
bound. No adverse effect is expected at $n_m = 1{,}725$; LOO-stability
check per TCT §1.2 step 6 will verify.

#### A.2.4 Stratification

- **Viseme-rarity oversampling (2–4×)**: `WeightedRandomSampler` at
  training time oversamples rare visemes OY, AW, UH by factor 2–4×
  (`researchpaper/08_case_santa_lora.md §96`;
  `researchpaper/15_appendix_repro.md §53–57`). **This is the only
  stratified sampler in the Santa pipeline** and it operates at batch
  composition time, not at reference-set construction time.
- **Emotion balancing**: `emotion_curve` table used to balance clip
  selection across emotion tags for the emotion reference subset.
- **Speaker diarisation (already filtered)**: all 2,362 clips are
  subject-only; interviewer excluded. This is filtering, not
  stratification.
- **Facial-region stratification**: not explicitly stratified; the
  mouth-weighted flow-matching loss weights mouth region 10× at
  training time in lieu of reference-set stratification.
- **Scene-diversity stratification**: proportional to scene length in
  `scene_map`; no enforced equal-scene quota.

#### A.2.5 Held-out / evaluation set

- **Forensic pipeline integrity**: flow-matching sign-bug diagnosed
  and fixed pre-training (`researchpaper/08_case_santa_lora.md §77`);
  the Case-1 15-test FSV battery does not apply directly — Case 2 FSV
  is represented by the six training-blocker fixes documented in
  `researchpaper/08_case_santa_lora.md §77–80` and
  `researchpaper/15_appendix_repro.md §33, §53–57`.
- **Held-out visual scenes**: **must be drawn from a different scene
  (scene_map segment) than any clip in $R_m$** (clip-level and
  scene-level disjointness; see disjointness matrix in §D below).
- **Held-out speech prompts**: unseen sentence content; disjoint
  from any transcript in the 2,362 training clips.
- **Sample size target**: 20–100 scenes / prompts (T1 PENDING;
  measurement defers to post-submission runs per
  `case-study-protocols.md §2.6`).

#### A.2.6 Power considerations

- **We do not run a formal power analysis for Case 2** because (i)
  end-to-end metrics are PENDING, (ii) there is no population-level
  comparator — this is a single-subject single-session demonstration,
  and (iii) the headline statistic is a four-modality conjunction
  accept-rate whose sampling distribution depends on four correlated
  cosines simultaneously.
- **Informal power floor**: $N_{\text{eval,2}} \geq 20$ scenes gives
  a 95 % CI of $\pm 0.20$ on a conjunction-accept-rate; $N = 50$
  tightens to $\pm 0.14$; $N = 100$ to $\pm 0.10$. Agent-26
  recommends **$N_{\text{eval,2}} \in [20, 100]$** per the handoff from
  agent-25.
- **What we acknowledge**: Case 2 is an **exploratory
  training-pipeline demonstration on 2,362 clips from one 16 min
  single-subject interview, single session, single language, single
  camera, single microphone**. §8 Limitations will state verbatim:
  > *"Case 2 trains on 2,362 clips from a single 16-minute interview
  > of one subject. The case demonstrates that EchoMimicV3 + LoRA with
  > mouth-weighted flow-matching runs end-to-end at this data scale,
  > that the flow-matching sign-bug and mouth-bbox errors can be
  > forensically diagnosed and fixed, and that a seven-modality
  > constellation can be constructed from the NPZ pool. It does not
  > support any cross-subject, cross-session, cross-language, or
  > cross-condition generalisation claim."*

---

### A.3 Case 3 — Voice clone beyond 0.95 WavLM SECS

#### A.3.1 Source population

- **Single-subject speaker corpus**: the same subject as Case 2 (the
  author). The corpus is **489 short audio clips** extracted from
  the author's video footage via Demucs source separation and
  silence-boundary splitting (`researchpaper/15_appendix_repro.md §153`).
- **Real-microphone references**: **two clean microphone recordings
  of 8–9 s each**, used as ICL references and as ground-truth
  recordings for SECS parity measurement.
- **Held-out sentence pool**: **10 novel English sentences with
  zero lexical overlap** to the ICL reference transcript
  (`researchpaper/15_appendix_repro.md §153`).
- Single subject, single language (English), single dialect, single
  microphone for reference, single (or shared) session. See
  Bias-05 / Bias-06 / Bias-09 and §A.3.6 below.

#### A.3.2 Sampling procedure

1. **Demucs source separation** on the 16 min video to isolate the
   subject's voice from interviewer, background music, and room noise.
2. **Silence-boundary splitting** to yield candidate clips.
3. **Length filter**: retain clips with length $\geq 3$ s.
4. **SNR filter**: retain clips with SNR $\geq 20$ dB.
5. **Single-speaker filter**: drop any clip with background speech
   detected by diarisation.
6. **WavLM-similarity ranking against the real-mic recording**:
   each of the 489 candidates is scored by cosine to the
   two 8–9 s real-mic clips; top-50 retained as the
   "highest-quality" subset that forms the **50-clip reference
   centroid** (`researchpaper/14_appendix_schemas.md §A.8`).
7. **Held-out sentence generation**: 10 novel sentences authored
   with zero lexical overlap to the transcript of any ICL clip
   (verified by exact-string and lemma overlap checks).
8. **Calibration set $S_{\text{speaker}}$**: disjoint subset of the
   489 corpus held aside for $\tau$ percentile calibration, per
   TCT §3.3. Target 50–200 clips.

#### A.3.3 Sample size

| Entity | N | Source |
|---|---|---|
| Source video duration | 16 min 15 s | Mission brief §3 Case 3 |
| Speaker corpus (author-derived clips) | **489** | `researchpaper/15_appendix_repro.md §153` |
| Reference centroid clips ($R_{\text{speaker}}$) | **50** (top-50 by WavLM similarity to real-mic) | `appendix_schemas.md §A.8` |
| Real-microphone ICL references | **2** (8–9 s each) | `researchpaper/15_appendix_repro.md §153` |
| Held-out evaluation sentences | **10** | Mission brief §3; `case-study-protocols.md §3.4` |
| Candidate TTS generations per sentence | **K = 12** (at T = 0.3) | `case-study-protocols.md §3.3` |
| Candidate pool total | 120 (12 × 10) | Pre-selection distribution per W2 |
| Calibration set $S_{\text{speaker}}$ | 50–200 | TCT §3.3 calibration procedure |
| $n_m$ curve subsets (W3 remediation, optional) | {10, 25, 50, 100} | `weakness-remediation.md §3.5` |

#### A.3.4 Stratification

- **Utterance-type stratification** across the 10 held-out sentences:
  declarative / interrogative / exclamatory balanced (soft target; the
  10 sentences are fixed and the distribution is documented, not
  enforced by resampling).
- **Reference-centroid stratification**: selected by **top-50 WavLM
  cosine to real-mic recording**, not by explicit prosodic or
  content stratification. This is a *quality*-based selection
  rather than a balanced random selection (documented as such, not
  as principled stratification).
- **Calibration-set stratification**: drawn uniformly at random
  from the 489 \ {50 reference clips}, with SNR / length filters
  re-applied; no further stratification.
- **$n_m$ curve subsets** (W3): for each target size $n \in \{10,
  25, 50, 100\}$, draw the **top-$n$** WavLM-similar clips (monotone
  nested selection) so the curve is a controlled ablation, not
  four independent random draws.

#### A.3.5 Held-out / evaluation set

- **10 sentences, English, novel, zero lexical overlap** to ICL
  reference transcript. Content and identity disjointness audit:
  - Sentence-level content: **verified disjoint** at string and
    lemma level from all 50 reference clips' transcripts and both
    real-mic recordings' transcripts.
  - Speaker: **same subject** as reference set (by construction —
    voice-cloning task).
  - Session: **MANDATORY W4 AUDIT RESULT** — the 10 held-out
    sentences were recorded in the **same studio session** as the
    two real-mic reference recordings (which serve as ground-truth
    for parity). The 489 speaker-corpus clips and the 50-clip
    reference centroid are derived from the *video* 16 min source,
    a **different recording session** than the real-mic reference
    session (same subject, different day, different microphone,
    different room). This is a **cross-session scenario with
    session-specific nuance**: reference-centroid-session ≠
    evaluation-session for the generator's input, but
    evaluation-session = ground-truth-parity-session for the DNSMOS
    / UTMOS / WavLM parity scorers. §8 L-04 records this exactly.
  - Temporal separation: reference centroid clips recorded months
    before the real-mic + held-out session (documented in the
    author's notes).
- **W4 resolution**: documented above; §6.3 and §8 L-04 of the
  manuscript will state the session structure verbatim.

#### A.3.6 Power considerations

- **Formal power analysis is not run for Case 3.** The N = 10
  held-out sentences is the fixed observed sample; we report the
  mean (0.961), max (0.975), 9-of-10-above-0.95, and the
  per-sentence distribution.
- **95 % bootstrap CI on the mean** (not a power calculation, but a
  retrospective CI): with N = 10 and empirical $\sigma \approx 0.02$
  on the observed cosines, the CI on the mean is approximately
  $0.961 \pm 0.012$. This is reportable in §6.3 without a power
  claim.
- **What we acknowledge**: Case 3 is an **exploratory N = 1 subject,
  N = 10 sentence, single-session-pair, single-microphone,
  English-only demonstration**, scored by a single guard encoder
  (WavLM-base-plus-sv) that is the same encoder used for
  best-of-$K$ selection. §8 Limitations will state verbatim
  (from agent-14 Bias-05, Bias-06, Bias-08 and agent-01 FM-05):
  > *"One speaker (the author), 50-clip reference centroid
  > (top-50 by WavLM similarity to the real-microphone reference
  > out of 489 candidate clips), 10 held-out sentences, WavLM-SECS-
  > scored best-of-12 at T = 0.3. DNSMOS / UTMOS parity computed
  > against the same speaker's real-microphone recordings recorded
  > in the same session as the held-out sentences. We do not
  > generalise beyond this protocol. N = 1 subject; N = 10 sentences;
  > N = 1 language; N = 1 dialect; N = 1 guard encoder."*

---

## Part B — TCT reference-set (R_m) construction

The Santa constellation (Case 2) and the voice centroid (Case 3) are
the two instantiated reference sets in this manuscript. Case 1's
reference set is textual and follows the same normalisation-twice
procedure on 50–100 Shakespeare passages.

### B.1 Target size per modality

Numbers below come from the Santa constellation NPZ
(`~/.clipcannon/models/santa/embeddings/all_embeddings.npz`;
8.8 MB on disk) as documented in
`researchpaper/14_appendix_schemas.md §A.6`:

| Modality | $n_m$ (samples) | Dim | NPZ key |
|---|---|---|---|
| Visual (global) | **1,725** | 1,152 | `vis_emb` |
| Emotion | **343** | 1,024 | `emo_emb` |
| Speaker (Case-2 audio-speaker) | **300** | 512 | `spk_emb` |
| Prosody / voice data | **188** | 12 | `pro_data` |
| Sentiment | **202** | 384 | `sent_emb` |
| Semantic | **200** | 768 | `sem_emb` |
| Voice (identity) | **188** | 192 | `voice_emb` |
| **Aggregate (Case 2)** | **3,146 samples × 4,044 dim** | — | — |
| Case 3 speaker reference ($R_{\text{speaker}}$) | **50** | 1,024 (WavLM-base-plus-sv) | (separate from Case-2 NPZ; constructed per A.3.2) |
| Case 1 text-style reference ($R_{\text{text-style}}$) | 50–100 | 1,024 (E1 / e5-large-v2) | (Shakespeare passages; no NPZ artifact, constructed on demand) |
| FLAME parametric head fits | 3,477 frames × 100 expression coefficients | — | Case 2 auxiliary |

### B.2 Quality criteria

Applied during ClipCannon's 23-stage DAG before embedding extraction:

| Criterion | Threshold / rule | Rationale |
|---|---|---|
| Audio SNR | $\geq 20$ dB | Exclude noisy segments that would bias speaker / voice / prosody centroids |
| Face-detection confidence (YOLOv5-Face) | Model default + mouth-landmark presence required for visual / voice modalities | Remove occluded / partial-face frames |
| Video frame-rate consistency | VFR normalised to CFR (stage `vfr_normalize`) | Prevent temporal artefacts |
| Speaker diarisation | Subject-only (cosine-distance 0.7 agglomerative cluster, WavLM-large) | Exclude interviewer |
| Silence / breath-only | Filtered out by `prosody_segments` | Segments with no speech drop from speaker / voice / prosody |
| Length floor | Per-modality; $\geq 3$ s typical for speaker modality | Stability of speaker-embedding cosine |
| Lip-sync baseline | SyncNet LSE-D thresholded | Face-audio alignment for visual modality |
| Duplicate detection | Cosine-deduplication within modality | Avoid over-representation of repeated frames |

### B.3 Diversity requirements

- **Cross-session diversity**: **NOT ACHIEVED** in Case 2 (single
  session — one 16 min interview) or Case 3 reference centroid
  (derived from the same video session, no cross-session clips
  included). This is a documented limitation (§6.2 / §6.3 / §8
  L-04), not an oversight.
- **Scene diversity (Case 2)**: proportional to `scene_map` length;
  scenes of different subjects of discourse within the interview
  yield different visual backgrounds, expressions, prosody.
- **Viseme diversity (Case 2)**: 15 MPEG-4 viseme classes, 100 %
  coverage across 7,916 samples; rare visemes (OY, AW, UH)
  explicitly oversampled 2–4× at training time.
- **Emotion diversity (Case 2)**: `emotion_curve` tagging covers
  multiple emotion classes; reference subset for emotion modality
  is drawn with emotion-class balancing.
- **Utterance-type diversity (Case 3 held-out)**: soft-balanced
  declarative / interrogative / exclamatory across the 10 sentences.
- **Genre diversity (Case 1 $R_{\text{text-style}}$)**: plays /
  sonnets / letters stratified ≈ 70 / 20 / 10.
- **Speaker diversity**: **N = 1 across all three cases.** No
  multi-speaker diversity is claimed or attempted.
- **Language diversity**: **English-only across all three cases.**
  No cross-language diversity is claimed.

### B.4 Normalisation and provenance (per TCT Phase 1)

For every $(m, c_m) \in C_P$:

1. **Preprocess** per $\phi_m$'s canonical pipeline (WavLM: 16 kHz
   mono, silence trim, loudness normalise; CLIP / SigLIP: 224×224
   centre crop, normalise; E1: e5-large-v2 tokeniser).
2. **Per-sample L2 normalise** (first normalisation);
   degenerate-norm guard $\epsilon = 10^{-6}$.
3. **Mean across kept samples**.
4. **L2 normalise the mean** (second normalisation).
5. **LOO stability check**: warn if LOO cosine $< 0.85$.
6. **Persist** alongside:
   - `embedder_version_hash` (SHA-256 of weights)
   - `reference_set_manifest` (list of sample IDs + SHA-256 content
     hashes)
   - $n_m$, $n_m^{\text{kept}}$
   - Construction timestamp
   - LOO stability score (if computed)
7. **Freeze** for the lifetime of the pipeline.

---

## Part C pointer

See `discoveries/agent-26-sampling-strategist--scope-of-validity.md`
for the paste-ready one-paragraph scope statements (what each case
does and does not support).

---

## D. Disjointness matrix (sample-level, set-level, session-level)

Per TCT §3.3 and handoff row 4 (W4):

### D.1 Case 1

| Set pair | Sample disjoint? | Session disjoint? | Notes |
|---|---|---|---|
| $R_{\text{text-style}}$ ∩ SFT training | ∅ expected | N/A (text corpus) | R set drawn from canonical passages reserved for TCT centroid |
| $R_{\text{text-style}}$ ∩ DPO training | ∅ expected | N/A | Same rule applied to DPO chosen responses |
| $R_{\text{text-style}}$ ∩ held-out eval | ∅ required | N/A | FSV F5 duplicate-scan enforces |
| SFT ∩ DPO | Overlap allowed (same corpus source) | N/A | DPO pairs use SFT corpus as chosen + modern paraphrase as rejected |
| SFT ∩ held-out eval | ∅ required | N/A | FSV F5 + cosine dedup |
| $S_{\text{text-style}}$ (calibration) ∩ $R_{\text{text-style}}$ | ∅ required | N/A | Per TCT §3.3 |
| $S_{\text{text-style}}$ ∩ held-out eval | ∅ required | N/A | Per TCT §3.3 |

### D.2 Case 2

| Set pair | Clip disjoint? | Scene disjoint? | Session disjoint? |
|---|---|---|---|
| $R_m$ (per modality) ∩ held-out scenes | ∅ required | ∅ required | SAME (single-session corpus; documented L-04 equivalent for Case 2) |
| $R_m$ ∩ $R_{m'}$ (different modalities, same clip) | **Overlap allowed** (single clip can contribute to multiple modality centroids) | N/A | SAME |
| Held-out scenes ∩ training clips (the 2,362) | ∅ required | ∅ required | SAME |
| Calibration set ∩ $R_m$ | ∅ required | ∅ required | SAME |
| Calibration set ∩ held-out | ∅ required | ∅ required | SAME |

### D.3 Case 3

| Set pair | Clip disjoint? | Session disjoint? | Notes |
|---|---|---|---|
| $R_{\text{speaker}}$ (50 clips) ∩ held-out sentence generations | ∅ by construction | Reference session ≠ evaluation session | See A.3.5 |
| $R_{\text{speaker}}$ ∩ real-mic ICL (2 clips) | ∅ (different session) | Different session | Real-mic clips are ground-truth, not centroid inputs |
| $R_{\text{speaker}}$ ∩ calibration $S_{\text{speaker}}$ | ∅ required | SAME session (both from 489 corpus) | Per TCT §3.3 |
| Held-out 10 sentences ∩ reference transcripts | ∅ (zero lexical overlap verified) | — | `researchpaper/15_appendix_repro.md §153` |
| Held-out recording session ∩ real-mic reference session | **SAME session** (ground-truth parity) | — | DNSMOS / UTMOS / SECS parity uses this co-session recording |
| Held-out recording session ∩ $R_{\text{speaker}}$ session | **DIFFERENT session** | — | Reference centroid derived from video 16 min source; held-out recorded separately with real microphone |

**W4 session-audit verdict**: **cross-session for generator input
(reference-centroid session ≠ held-out session, same subject) AND
same-session for parity comparator (held-out session = real-mic
ground-truth session).** §8 L-04 records this structure; §6.3
reports the 0.961 headline with the session structure stated inline.

---

## E. Eligibility-screening summary (for agent-27)

Agent-27 (instrument-developer) should produce screening instruments
for each case that implement the eligibility criteria below. These
are the *sampling gates*; the measurement instruments sit downstream
of them.

### E.1 Case 1 gates

1. Text authenticity: passage present in Folger / Gutenberg canonical
   edition.
2. Post-cleaning non-empty: stripped-response length $> 0$.
3. Duplicate-free: exact-string absence from held-out eval set.
4. Cosine-novelty: E1 cosine to any held-out prompt $< 0.98$.

### E.2 Case 2 gates

1. Clip duration $\geq$ per-modality floor (modality-specific).
2. Audio SNR $\geq 20$ dB.
3. Face-detection confidence $\geq$ YOLOv5-Face default + mouth-
   landmark presence.
4. Diarisation label = subject (not interviewer).
5. No background speech in audio track.
6. Scene-disjointness from held-out set.

### E.3 Case 3 gates

1. Clip length $\geq 3$ s.
2. SNR $\geq 20$ dB.
3. Single-speaker (no background speech).
4. WavLM cosine to real-mic reference used only for top-50 ranking
   (not as a reject gate — the gate is the SNR / length / speaker
   triple above).
5. Held-out sentence: zero lexical overlap with any reference
   clip transcript (string and lemma check).
6. Calibration-set clip: disjoint from top-50 reference and from
   any held-out recording.

---

## F. Bias and honesty disclosures (mandatory paste-ready lines)

The §8 Limitations writer (agent-40) MUST paste these verbatim from
agent-14 `bias-audit.md`:

- **Bias-05** single-subject paragraph (Cases 2 and 3).
- **Bias-06** single-encoder paragraph (Case 3, WavLM-only).
- **Bias-08** self-reporting paragraph (all three cases).
- **Bias-09** benchmark-monoculture paragraph (Case 3 clean-audio
  author-read).
- **FM-05** single-subject opener paragraph (§6.3 opener).

Additional sampling-specific line (agent-26 requires in §8):

> *"Our three case studies are exploratory demonstrations. Case 3
> reports measured numbers on N = 10 held-out sentences from one
> English-speaking subject (the author) recorded in one session on
> one microphone, with a 50-clip reference centroid selected from
> 489 candidate clips by WavLM similarity to a real-microphone
> recording. Case 1 trains on 7,908 SFT examples + 12,567 DPO pairs
> drawn from the canonical Shakespeare corpus and evaluates on held-
> out prompts at a power floor of N ≥ 20; end-to-end numbers are
> PENDING. Case 2 trains on 2,362 clips from one 16-minute single-
> subject interview with a seven-modality NPZ constellation of 3,146
> samples × 4,044 dimensions; end-to-end numbers are PENDING. No
> case is a power-calculated population study, and no case supports
> generalisation beyond its documented scope."*

---

## G. Confidence

**HIGH** that the sampling protocols are re-implementable from this
file alone for Cases 1 and 3.

**HIGH** that Case 2's reference-set construction is reproducible
from the ClipCannon 23-stage DAG + YOLOv5-Face + WavLM diarisation
+ CMU viseme mapping pipeline as specified here and in the
researchpaper appendices.

**HIGH** that the disjointness matrix (§D) closes agent-24 W4 for
Case 3 (cross-session for generator input, same-session for parity
comparator) and documents the analogous same-session structural
feature for Case 2.

**HIGH** that §A.*.6 power considerations correctly frame every
case as exploratory N = 1 demonstration rather than a power-
calculated population study, which is the verbatim framing §8
Limitations must carry.

**MEDIUM** on held-out evaluation N for Cases 1 and 2 — recommended
floors (20–50 and 20–100 respectively) are reasonable but engineering
capacity at submission time determines the realised N. The
manuscript must report the realised N.

## Changelog

- 2026-04-16 — Initial issue by agent-26-sampling-strategist. Locked.
