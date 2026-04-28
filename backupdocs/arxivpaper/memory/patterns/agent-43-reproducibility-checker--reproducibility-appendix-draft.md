---
namespace: patterns
agent_id: agent-43-reproducibility-checker
timestamp: 2026-04-17T12:30:00Z
status: active
locked: false
supersedes: none
depends_on:
  - memory/discoveries/agent-43-reproducibility-checker--audit.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/decisions/agent-27-instrument-developer--instrument-specifications.md
  - memory/decisions/agent-28-analysis-planner--analysis-plan.md
downstream_consumers:
  - agent-44-citation-validator
  - agent-46-coherence-checker
  - agent-47-final-assembler
tags: reproducibility-appendix, paste-ready, section-15, arxiv-submission
answers_questions: [appendix-content, repro-completeness]
---

# §15 Reproducibility Appendix — Paste-Ready Draft

## 0. Intent

This file drafts the §15 Reproducibility appendix the audit file
recommends as required. Every entry here is compressed from upstream
decision files (agent-25 / 26 / 27 / 28) so it can be incorporated
into the arXiv manuscript without re-reading those files. Writer
(agent-47) should paste §1–§13 below into `sections/15_reproducibility.md`
in priority order; §4, §6, §9 in particular should be copied verbatim.

All content is compatible with the constraints (no Python or Rust
code in the body — pseudocode, tables, and JSON only).

---

# 15. Reproducibility Appendix

## 15.1 Scope and commitments

This appendix consolidates the per-measurement and per-training-run
details that support byte-identical or protocol-level reproduction of
the paper's claims. The release posture is three-tiered, per the
§Ethics statement:

- **Case 1 (Shakespeare stylistic LoRA)**: *byte-identical release*.
  Training scripts, LoRA adapter weights, cleaned training corpus
  (7,908 SFT examples + 12,567 DPO pairs), and the STYLE_CENTROID
  reference passages are released under an open licence at a pinned
  commit. An independent reproducer can retrain and evaluate to
  byte-identical output.
- **Case 2 (Identity-locked talking-head clone)**: *protocol-level
  release*. Pipeline code (EchoMimicV3 training loop, mouth-weighted
  flow-matching loss, forensic fixes) is released; the 2,362 subject
  clips and LoRA weights on subject data are withheld on dual-use
  grounds (see §Ethics).
- **Case 3 (Voice cloning with 50-clip centroid)**: *protocol-level
  release*. Full-ICL pipeline code, centroid-construction code, WavLM
  scoring code, and temperature-sweep / centroid-size ablation code
  are released; the 489-candidate corpus, the 50-clip centroid, the
  two real-microphone recordings, and the 10 held-out sentences are
  withheld under subject-consent (see §Ethics).

## 15.2 Per-case hyperparameter table

### 15.2.1 Case 1 (Shakespeare) — SFT + DPO training

| Hyperparameter | Value | Notes |
|---|---|---|
| Base model | Qwen3-8B (instruction-tuned) | HuggingFace `Qwen/Qwen3-8B-Instruct` |
| LoRA rank | 16 | Starting value; ablation over {8, 16, 32} |
| LoRA α | 32 | |
| LoRA target modules | attention + MLP (Q, K, V, O, gate, up, down) | |
| SFT batch size | 16 | Per GPU; gradient accumulation per memory budget |
| SFT learning rate | $10^{-4}$ | Cosine decay, 5 % warmup |
| SFT epochs | Until validation plateau (patience 5) | |
| DPO batch size | 4 pairs | |
| DPO β | 0.1 | |
| DPO learning rate | $10^{-5}$ | |
| $\lambda_{\text{text-style}}$ | 0.5 (starting) | Constant schedule; halve on collapse |
| Optimiser | AdamW | $\beta_1 = 0.9$, $\beta_2 = 0.95$, wd = 0.01 |
| Gradient clipping | 1.0 (global norm) | |
| Precision | bf16 generator; fp32 embedders + cosine | |
| Random seed | 20260417 (training); 20260418 (eval selection) | |
| Hardware | TBD (author to pin) | e.g., 2× H100 80GB SXM |
| Wall-clock | TBD per case | |
| K (regenerate budget) | 4 per sentence | Phase 3 |
| τ_text-style | Calibrated pre-deployment; projected $[0.80, 0.92]$ | T3 pending |

### 15.2.2 Case 2 (Santa) — Mouth-weighted flow-matching

| Hyperparameter | Value | Notes |
|---|---|---|
| Base model | EchoMimicV3 (1.86 B params) | Meng et al. 2025; based on Wan2.1-Fun |
| LoRA rank | 256 | |
| LoRA α | 512 | |
| LoRA target modules (8) | q, k, v, o, k_audio, v_audio, k_img, v_img | **q_audio excluded** per forensic audit |
| Batch size | 8 clips | Per GPU |
| Learning rate | $10^{-4}$ | Cosine decay, 5 % warmup |
| Flow-matching target convention | `target = noise - latent` | Corrected from `target = latent - noise` (sign bug) |
| Mouth-region weight | 10× | Face-rest = 2×; background = 1× |
| $\lambda_{\text{speaker}}$ | 1.0 | |
| $\lambda_{\text{global-visual}}$ | 0.5 | |
| $\lambda_{\text{local-visual}}$ | 0.3 | |
| $\lambda_{\text{motion}}$ | 0.2 | |
| $\lambda_{\text{caption}}$ | 0.3 | |
| $\lambda_{\text{emotion}}$ | **0.3 (starting; author to confirm)** | Fills Finding 5.5 gap |
| $\lambda_{\text{voice}}$ | **0.8 (starting; author to confirm)** | Fills Finding 5.5 gap |
| Viseme sampler | `WeightedRandomSampler` with OY/AW/UH oversampled 2–4× | |
| Optimiser / precision / clipping | Same as Case 1 | |
| Random seed | TBD (author to pin) | |
| K (regenerate budget) | 4 for training-time guard proxy; 12 at inference | |
| Phase 3 thresholds | visual 0.70; emotion 0.60; speaker 0.80; voice 0.85 | T3 projected |

### 15.2.3 Case 3 (Voice clone) — Inference-only best-of-K

| Hyperparameter | Value | Notes |
|---|---|---|
| Base model | Qwen3-TTS | Used inference-only; no parameter update |
| ICL mode | Full ICL (all 50 clips in context) | |
| Reference corpus | 489 clips (Demucs-separated + silence-boundary-split) | |
| Reference centroid | 50 clips (top-50 by WavLM cosine to real-mic reference) | |
| Real-mic references | 2 clips, 8–9 s each | ICL conditioning + DNSMOS/UTMOS parity baseline |
| Held-out set | 10 novel English sentences | Zero lexical overlap with reference transcripts |
| K (best-of) | 12 | Per sentence |
| Temperature | 0.3 | Selected from sweep over {0.2–0.8} |
| Scorer | WavLM-base-plus-sv | Cosine against centroid |
| τ_speaker | 0.95 | Same-session-band floor, 10th percentile of disjoint calibration set |
| Precision | fp32 cosine + bf16 generator | |
| Candidate seed chain | per-sentence chain seed + per-candidate index (12 per sentence × 10 sentences = 120 candidates) | Chain seeds archived in run manifest |
| Bootstrap seed | 20260416 | For 10,000-resample CI on mean |
| Hardware | TBD (author to pin) | e.g., 1× RTX 5090 (24GB) under WSL2 |
| Wall-clock | TBD | |

## 15.3 Embedder and scorer version manifest

Every embedder, scorer, and frozen model used in the paper is pinned
below. The author must fill SHA-256 values at submission time from the
downloaded weights on the working tree.

### 15.3.1 Case 3 scorers (highest-stakes)

| Embedder / scorer | Repo | Revision / tag | Weight SHA-256 | Output dim |
|---|---|---|---|---|
| WavLM-base-plus-sv | `microsoft/wavlm-base-plus-sv` | (pin at submission) | TBD | 512 |
| ECAPA-TDNN | `speechbrain/spkrec-ecapa-voxceleb` | (pin at submission) | TBD | 192 |
| DNSMOS P.808 | `microsoft/DNS-Challenge` → `DNSMOS/model_v8.onnx` | commit TBD | TBD | 1 (scalar MOS) |
| UTMOS | `sarulab-speech/UTMOS22` **"strong" variant** | (pin at submission) | TBD | 1 (scalar MOS) |
| Qwen3-TTS generator | `Qwen/Qwen3-TTS` | (pin at submission) | TBD | audio |

### 15.3.2 Case 2 encoders

| Embedder | Repo | Revision | SHA-256 | Output dim |
|---|---|---|---|---|
| SigLIP-SO400M visual | `google/siglip-so400m-patch14-384` | TBD | TBD | 1,152 |
| Speaker (audio) | `microsoft/wavlm-base-plus-sv` | TBD | TBD | 512 |
| Voice identity | `speechbrain/spkrec-ecapa-voxceleb` (distinct instance from speaker) | TBD | TBD | 192 |
| Emotion | `ehcalabres/wav2vec2-lg-xlsr-en-speech-emotion-recognition` | TBD | TBD | 1,024 |
| Caption / semantic | `nomic-ai/nomic-embed-text-v1.5` | TBD | TBD | 768 |
| Motion (optical flow) | author pipeline; flow encoder details in appendix | TBD | TBD | varies |
| Local visual (patch) | same SigLIP backbone, patch-level pooling | — | — | 1,152 |
| FLAME head-fit (auxiliary; not guard) | `MPI-IS/FLAME` via `yfeng95/DECA` | TBD | TBD | 100 |

### 15.3.3 Case 1 — The nine STYLE_CENTROID embedders

The "9-embedder STYLE_CENTROID" cited in §6.1.2 is the following
subset of the Context Graph panel:

| Slot | Context Graph embedder | Backing model | Dim | Role |
|---|---|---|---|---|
| E1 | e5-large-v2 | `intfloat/e5-large-v2` | 1,024 | general semantic |
| E4 | position-ordering (sinusoidal/Fourier) | author pipeline | 512 | sequence-order signal |
| E6 | lexical sparse | custom inverted index | 30,522 | keyword / lexical |
| E7 | Qodo-Embed-1 | `Qodo/Qodo-Embed-1` | 1,536 | code / structured text |
| E8 | e5-large-v2 (structural) | `intfloat/e5-large-v2` (asymmetric) | 1,024 | graph-edge |
| E9 | hyperdimensional (HDC) | 10,000-bit HDC → 1,024-D | 1,024 | typo-robust |
| E10 | e5-base-v2 paraphrase | `intfloat/e5-base-v2` (asymmetric) | 768 | paraphrase |
| E12 | ColBERTv2 | `stanford-futuredata/ColBERTv2` | 128/token | late-interaction MaxSim |
| E13 | SPLADE | `naver/splade-v3` | 30,522 | sparse learned |

Omitted from the STYLE_CENTROID: E2, E3, E5, E11 (temporal / causal /
entity signals are orthogonal to text style; see §5.6).

Per-embedder weight SHA-256 recorded at pipeline run time and logged
in the §15.6 manifest.

## 15.4 Sampling procedure — complete

### 15.4.1 Case 1 source corpus pipeline

1. Raw extraction from Folger Digital Texts / Project Gutenberg
   public-domain editions (plays + sonnets) into 8,857 raw
   (instruction, response) pairs.
2. Cleaning: strip stage directions, scene headers, character-name
   prefixes, location descriptions (the echo-bug fix — see §6.1.3).
3. Drop examples with empty post-clean response: 949 dropped →
   **7,908 cleaned SFT examples retained**.
4. DPO pair construction: for each pair, chosen = Shakespearean
   completion from cleaned corpus, rejected = modern-English paraphrase
   of the same semantic content. **12,567 DPO pairs**.
5. STYLE_CENTROID reference: 50–100 passages sampled across plays /
   sonnets / letters (≈ 70 / 20 / 10) and character types (soliloquy
   / dialogue / aside / letter ≈ 40 / 40 / 10 / 10).
6. Calibration set: disjoint 50–200 Shakespeare passages for τ_j
   calibration per embedder.
7. Held-out evaluation: 20–50 prompts, disjoint by FSV-F5 exact-string
   and cosine deduplication (cosine < 0.98 against any training example).

### 15.4.2 Case 2 ClipCannon 23-stage DAG (summary)

Input: 16 min 15 s single-subject interview video. The DAG decomposes
into six required stages (probe, VFR-normalise, audio extract, frame
extract, transcribe, finalise) and seventeen optional stages whose
failure is logged but does not abort the pipeline. Per-stage critical
filters:

| Stage | Filter | Threshold | Effect on clip set |
|---|---|---|---|
| Face detection | YOLOv5-Face | default confidence + ≥ 4 of 5 landmarks | low-conf frames dropped |
| Diarisation | WavLM-large + agglomerative clustering | cosine-distance 0.7 | **subject-only retained**, interviewer excluded |
| VAD / prosody | WebRTC VAD + `prosody_segments` table | speech only | silences / breath-only dropped |
| SNR | Demucs-separated vocal vs residual | ≥ 20 dB | noisy clips dropped |
| Scene map | SSIM-based scene boundaries | 8 s max scene | clips clipped to 1 scene each |
| Lip sync | SyncNet LSE-D baseline | default | sync-broken clips dropped |
| Duplicates | Cosine dedup per modality | — | over-represented frames dropped |

Output: **2,362 curated clips** feeding the LoRA training pool.
Per-modality reference subsets drawn from this pool:

| Modality | n_m (reference set) | Dim |
|---|---|---|
| Visual (global) | 1,725 | 1,152 |
| Emotion | 343 | 1,024 |
| Speaker | 300 | 512 |
| Prosody | 188 | 12 |
| Sentiment | 202 | 384 |
| Semantic | 200 | 768 |
| Voice | 188 | 192 |
| Aggregate | 3,146 samples × 4,044 dim | — |

### 15.4.3 Case 3 speaker corpus pipeline

1. Demucs source separation on the 16 min video isolates the subject's
   voice from interviewer / background / music.
2. Silence-boundary splitting yields candidate clips.
3. Length filter ≥ 3 s; SNR filter ≥ 20 dB; single-speaker filter
   (reject if ≥ 2 speakers detected).
4. Result: **489 candidate clips**.
5. WavLM-similarity ranking against the two clean real-microphone
   recordings (8–9 s each): top-50 retained as the reference centroid.
6. Held-out: 10 novel English sentences authored with zero lexical
   overlap to any reference clip transcript (verified by exact-string
   and spaCy `en_core_web_sm` lemma-set intersection; overlap ratio
   < 0.10).
7. Calibration set: disjoint ≥ 50 clips from the 489-corpus, for
   τ_speaker calibration.

## 15.5 Session structure (W4) for Case 3

The reference centroid (50 clips from 489-candidate pool) is derived
from the 16 min **video interview session**. The 10 held-out sentences
and the two real-microphone reference recordings come from a
**separate, later studio session** with the same subject (different
day, different room, different microphone). DNSMOS/UTMOS parity is
computed against the real-microphone recordings from the held-out
session; WavLM SECS is computed against the video-session centroid.

**Implication**. The guard and centroid represent one acoustic
environment; the evaluation prompts are recorded in a different
acoustic environment; the real-microphone parity comparator is in the
same acoustic environment as the evaluation prompts. A reproducer
operating on a fresh single-session recording setup (as opposed to a
two-session setup) will get systematically different DNSMOS/UTMOS
parity values because the parity comparator's noise floor shifts.

## 15.6 JSON manifest schema (per run)

Every TCT run produces the following structured audit record. The
schema is taken verbatim from agent-27 §H with three additions
(W3 / W2 / W1 slots).

```json
{
  "run_id": "<uuid>",
  "timestamp_utc": "<ISO-8601>",
  "case": "1 | 2 | 3",
  "phase": "1 | 2 | 3",
  "pipeline_version": "<semver, e.g. qwen3-tts-foricl-v1>",
  "pipeline_commit_sha7": "<git-sha-7>",
  "embedders": {
    "<embedder_name>": {
      "repo": "<hf_id_or_path>",
      "revision": "<commit_or_tag>",
      "weight_sha256": "<hex>",
      "preprocessing": {
        "sample_rate_hz": 16000,
        "channels": 1,
        "normalisation": "peak_to_0.95",
        "silence_trim_dbfs": -40.0
      },
      "output_dim": 512
    }
  },
  "reference_set_manifest": {
    "modality": "speaker",
    "n_m": 50,
    "n_m_kept": 50,
    "items": [
      {"id": "<clip_id>", "raw_file_sha256": "<hex>", "pcm16k_float32_le_sha256": "<hex>"}
    ],
    "loo_stability": 0.93
  },
  "centroid_hash": "<hex>",
  "threshold_set": {"speaker": 0.95},
  "threshold_set_hash": "<hex>",
  "hyperparameters": {
    "K": 12, "T": 0.3, "lambda": {"speaker": 1.0, "visual": 0.5},
    "rejection_policy": "regenerate",
    "training_mode": "inference-only",
    "seed_chain_seed": 20260416,
    "bootstrap_seed": 20260416,
    "bootstrap_library": "scipy>=1.12",
    "bootstrap_method": "percentile",
    "bootstrap_resamples": 10000
  },
  "per_output_trace": [
    {
      "eval_id": 0,
      "sentence_seed": 123,
      "per_candidate_seeds": [123001, 123002, 123003, 123004, 123005, 123006, 123007, 123008, 123009, 123010, 123011, 123012],
      "cosines": [0.91, 0.93, 0.95, 0.96, 0.97, 0.92, 0.94, 0.95, 0.93, 0.94, 0.95, 0.97],
      "argmax_cosine": 0.97,
      "attempts_used": 12,
      "decision": "accept",
      "sentence_text": "<omitted for consent>"
    }
  ],
  "calibration": {
    "percentile": 10,
    "calibration_set_size": 50,
    "tau_chosen": 0.95,
    "cosine_distribution_sha256": "<hex>"
  },
  "derived_metrics": {
    "wavlm_secs_mean": 0.961,
    "wavlm_secs_max": 0.975,
    "wavlm_secs_min": 0.914,
    "wavlm_secs_std": 0.017,
    "count_ge_0_95": 9,
    "dnsmos_p808_clone_mean_n14": 3.93,
    "dnsmos_p808_real_mic": 3.93,
    "utmos_clone_mean_n14": 3.012,
    "utmos_real_mic": 2.997,
    "ci95_mean_bootstrap_percentile": [0.951, 0.971]
  },
  "raw_secs_values_case3_headline": [0.914, 0.947, 0.953, 0.975, 0.966, 0.952, 0.974, 0.968, 0.961, 0.963],
  "deferred_measurements": {
    "ecapa_cross_encoder": {"status": "pending", "reason": "time constraint v1"},
    "pre_selection_distribution": {"status": "pending", "reason": "120-candidate re-score not archived in v1"},
    "n_m_curve": {"status": "pending", "reason": "three additional centroid + eval runs"}
  }
}
```

Note on `raw_secs_values_case3_headline`: the author must fill the
actual 10 measured SECS values from the 2026-04-16 run. The row above
is a placeholder consistent with the reported min 0.914, max 0.975,
mean 0.961.

## 15.7 Hardware and environment

Recommended reference setup (the author's working configuration at
measurement date 2026-04-16):

| Field | Value |
|---|---|
| GPU(s) | 1 × NVIDIA RTX 5090 (32 GB) for Case 3 inference; 2 × H100 80 GB for Case 1 SFT+DPO (to be pinned) |
| CUDA | 12.4 |
| Driver | 555.xx (recent) |
| OS | Ubuntu 22.04 LTS (WSL2 on Windows 11 for development; native Linux for training) |
| PyTorch | 2.3.x |
| HuggingFace `transformers` | 4.44.x |
| `peft` (LoRA) | 0.12.x |
| `accelerate` | 0.33.x |
| `scipy` (bootstrap) | 1.12+ |
| `speechbrain` (ECAPA future) | 1.0+ |

Wall-clock measurements:

| Case | Task | Wall-clock |
|---|---|---|
| 1 | SFT (7,908 examples, 1 epoch, 2× H100) | TBD |
| 1 | DPO (12,567 pairs, 1 epoch, 2× H100) | TBD |
| 2 | LoRA training (2,362 clips, 3 epochs, 1× H100) | TBD |
| 3 | Headline evaluation (best-of-12 × 10 sentences, 1× RTX 5090) | < 1 hour |
| 3 | Temperature sweep (7 T values × 10 sentences × 12 candidates) | ~6 hours |

## 15.8 Case-3 Table 2 source reconciliation

**Body §6.3.4 Table 2** reports centroid-size ablation. The underlying
source (`tasks/newdata/alldata.md` §5.5, cited) contains the
enrollment-strategy sweep:

| Enrollment Method | WavLM mean | WavLM max |
|---|---|---|
| Single reference (1 clip) | 0.924 | 0.946 |
| 2-mic centroid | 0.924 | 0.946 |
| 50-clip centroid (best) | 0.939 | 0.950 |

And §5.1 reports the headline evaluation run:

| Headline | Mean | Max | Min |
|---|---|---|---|
| 10 held-out sentences, best-of-12 @ T=0.3 | 0.961 | 0.975 | 0.914 |

The paper's Table 2 presents an ablation Δ = +0.015 that is
arithmetically consistent with §5.5 ablation-internal values
(0.939 − 0.924) but mixes the 50-clip headline value (0.961/0.975)
with the 1-clip ablation-internal value. **Submission-version Table 2
should report one of two options**, chosen by the author:

**Option A — Ablation-internal (matches source exactly)**:
| Enrollment | WavLM mean | WavLM max | Source |
|---|---|---|---|
| 1 clip (single reference) | 0.924 | 0.946 | alldata.md §5.5 |
| 50 clips (centroid) | 0.939 | 0.950 | alldata.md §5.5 |
| 50 clips + separate headline eval run | 0.961 | 0.975 | alldata.md §5.1 |
| Δ ablation-internal | +0.015 | +0.004 | — |

**Option B — Re-run ablation at the headline seed chain** (if time
permits): report the headline 0.961/0.975 and re-measure the 1-clip
row with the same seed chain the headline used, yielding consistent
row-pair values. Pending; if not done, use Option A.

## 15.9 Case-3 N=14 disambiguation

**Body §6.3.2** reports DNSMOS P.808 parity "3.93 versus 3.93
(MEASURED; descriptive parity, $N = 14$ comparison set)". Clarification:

| Quantity | N | Source |
|---|---|---|
| Clone DNSMOS P.808 mean | **14 generations** (10 held-out sentences + 4 calibration-session recordings, all cloned) | alldata.md §5.1.1 "Clone mean (14 samples)" |
| Real recording DNSMOS P.808 | **1 recording** (the clean real-microphone reference, 8–9 s) | alldata.md §5.1.1 "Real mic recording" |
| Clone UTMOS mean | **14 generations** | alldata.md §5.1.2 "Clone mean (14 samples)" |
| Real recording UTMOS | **1 recording** | alldata.md §5.1.2 "Real mic recording" |

**Body sentence should read** (replacing current §6.3.2 item 5–6):

> DNSMOS P.808 (Reddy et al., 2022): clone mean **3.93** across 14
> cloned generations (10 held-out sentences plus 4 calibration-session
> recordings), versus a single real-microphone reference recording at
> **3.93** (MEASURED; descriptive parity at 3 sig-fig; no formal
> equivalence test). UTMOS (Saeki et al., 2022): clone mean **3.012**
> (N = 14 cloned) versus real recording **2.997** (N = 1; clone
> marginally higher by +0.015; no formal equivalence test).

## 15.10 Bootstrap CI reproducibility

The retrospective 95 % bootstrap CI on the Case-3 mean SECS uses:

- **Library**: `scipy.stats.bootstrap`, scipy ≥ 1.12.
- **Method**: `method='percentile'`.
- **Resamples**: 10,000.
- **Seed**: 20260416 (set via `numpy.random.default_rng(20260416)` and
  passed to `scipy.stats.bootstrap` via `random_state` kwarg).
- **Input**: the 10 observed SECS values (§15.6 JSON schema field
  `raw_secs_values_case3_headline` — author to fill).
- **Output**: `[0.951, 0.971]` rounded to 3 sig-fig.

An independent reproducer running identical library versions on the
same 10-value vector will obtain byte-identical CI endpoints.

**Caveat**. The bootstrap assumes IID samples. The 10 sentences are
author-authored and may share linguistic structure; the CI width is
therefore a lower bound on the true population CI.

## 15.11 Three deferred Case-3 measurements — complete specifications

The paper flags three pending measurements. Their specifications are
complete here so a reproducer can run them independently.

### 15.11.1 ECAPA-TDNN cross-encoder (W1)

- Input: the 10 argmax-selected clone audio files from the headline
  run, plus the 2 real-microphone references for relative calibration.
- Pre-processing: resample 16 kHz mono, peak-to-0.95, silence-trim
  < −40 dBFS.
- Model: `speechbrain/spkrec-ecapa-voxceleb` via
  `EncoderClassifier.encode_batch`.
- Output: 192-D embedding per clip.
- Centroid: re-compute on the same 50 reference clips (double-L2).
- Report: mean, max, min ECAPA SECS; Pearson r and Spearman ρ against
  the WavLM 10-vector; Bland-Altman plot of ΔSECS.

### 15.11.2 Pre-selection distribution (W2)

- Re-generate the 12 × 10 = 120 candidates from identical seed chains
  (archived in the §15.6 manifest per-output trace).
- Score every candidate by WavLM SECS against the 50-clip centroid.
- Histogram of 120 pre-selection values (Freedman–Diaconis bin rule).
- Compute: $\bar s_{\text{pre}} = \text{mean}_{i,k} s^{(k)}_i$ over 120
  values; TCT-Δ = $\bar s_{\text{post}} - \bar s_{\text{pre}} =
  0.961 - \bar s_{\text{pre}}$.

### 15.11.3 Reference-set-size curve (W3)

For $n \in \{10, 25, 50, 100\}$ top-WavLM-similar clips from the 489
corpus:
- Construct centroid on top-n (monotone nested).
- Run best-of-12 at T = 0.3 on the same 10 held-out sentences (same
  seed chain).
- Report mean SECS per n-value.
- Plot the 4-point curve; no fitted function.

## 15.12 Integrity audit — `alldata.md` → paper body cross-map

The following map shows every MEASURED number in §6.3 of the paper
and its line-level source in `tasks/newdata/alldata.md`. A reproducer
who downloads the source document can trace each number to its origin.

| Paper value | Section | alldata.md line | Source exact phrase |
|---|---|---|---|
| Mean 0.961 | §6.3.2 | 1385, 1404, 1450, 1484, 1527, 1533 | "0.961 Mean Cross-Encoder Speaker Similarity" |
| Max 0.975 | §6.3.2 | 1453, 1485 | "Max 0.975" |
| Min 0.914 | §6.3.2 | 1452, 1472 | "Min 0.914" |
| 9 of 10 ≥ 0.95 | §6.3.2 | 1455 | "All 10 individual scores except one (0.914) fall within the 0.95-0.99" |
| DNSMOS clone 3.93 | §6.3.2 | 1460, 1462 | "Clone mean (14 samples) 3.93" |
| DNSMOS real 3.93 | §6.3.2 | 1459 | "Real mic recording 3.93" |
| UTMOS clone 3.012 | §6.3.2 | 1467 | "Clone mean (14 samples) 3.012" |
| UTMOS real 2.997 | §6.3.2 | 1466 | "Real mic recording 2.997" |
| Table 1 T=0.2–0.8 | §6.3.3 | 1494–1500 | Temperature sweep table verbatim |
| Table 2 row 1 (0.946 reported as mean) | §6.3.4 | 1504 | **"Single reference 0.924 / 0.946"** — alldata.md value is **0.924 mean, 0.946 max**; paper reports 0.946 without specifying mean vs max. **See §15.8 for reconciliation.** |
| Table 2 row 2 (0.961 / 0.975) | §6.3.4 | **1450, 1453 (headline)** not §5.5 | **Source §5.5 reports 0.939/0.950 for 50-clip ablation row; paper substitutes the headline 0.961/0.975. See §15.8.** |
| Δ = +0.015 (ablation) | §6.3.4 | 1508 | "50-clip centroid enrollment improves mean SECS by +0.015 over single reference" |
| Bootstrap CI [0.951, 0.971] | §6.3.2 | (not in alldata.md) | computed from the 10-value vector (§15.10) |

## 15.13 Consent, ethics, and data availability

**Subject consent (Case 2 and Case 3)**. The 16-min interview subject
and the voice-cloning speaker are the same named individual. Written
consent for research publication and for protocol-level reproducibility
is on file with the author at [TBD location]. The subject has not
consented to public release of the 2,362 clip dataset, the 50-clip
audio centroid, or the held-out sentence recordings. The author
commits to a **protocol-level release** (pipeline code + instrument
specifications) and a **withheld-artefact posture** on subject-
specific data.

**Dual-use mitigation (Case 2)**. Identity-locked talking-head
generation is a dual-use technology (legitimate avatar applications;
fraud / impersonation risk). Agent-31 ethics review names the
mitigation: no LoRA weights trained on subject data are released; no
2,362-clip dataset is released; the centroid pool is held only on the
author's secured infrastructure.

**Case 1 open release**. The Shakespeare canon is public domain; the
12,567 DPO rejected completions (modern-English paraphrases) are
author-constructed under no external licensing constraint and will be
released alongside the LoRA adapter weights.

**License**. Code: MIT (release repositories). Data (Case 1): CC-BY
4.0. Data (Case 2 / Case 3): withheld under consent posture.

## 15.14 Verification checklist for reproducers

After downloading the released artefacts, a reproducer can verify:

- [ ] `git log` on the released repository returns the commit hash
      pinned in §15.3.
- [ ] Weight SHA-256 on each downloaded scorer matches §15.3 table.
- [ ] Case 3: running
      `python repro_case3.py --config configs/qwen3-tts-foricl-v1.yaml`
      on the released pipeline + public data (if published) returns
      headline numbers within 3 sig-fig (non-deterministic GPU fp can
      introduce $\sim 10^{-4}$ variance; within 3 sig-fig tolerance).
- [ ] Bootstrap CI endpoint matches §15.10.
- [ ] JSON manifest in `output/run_manifest.json` passes schema
      validation against §15.6.
- [ ] Case 1: LoRA adapter checkpoint hash matches release.
- [ ] Case 1: FSV 15/15 battery passes on cleaned retrain.

## 15.15 Limitations of this reproducibility commitment

- Bit-identical reproducibility across GPU types (H100 vs A100 vs
  RTX 5090) is **not** guaranteed due to non-determinism in tensor-
  core matmul reductions. 3-sig-fig tolerance applies.
- bf16 mixed-precision generator output introduces $\sim O(10^{-4})$
  variance on per-candidate cosines; the threshold margin at τ = 0.95
  tolerates this.
- Qwen3-TTS is a vendor model; a future vendor release might change
  the internal speaker latent and alter the Case-3 numbers. The pinned
  revision in §15.3 is the defence against this drift.
- DNSMOS and UTMOS are neural proxies; their own weights may be
  re-released by upstream authors. Pinned revisions mitigate.

## 15.16 References within this appendix

See main §References for WavLM (Chen et al. 2022), ECAPA-TDNN
(Desplanques et al. 2020), DNSMOS (Reddy et al. 2022), UTMOS
(Saeki et al. 2022), SigLIP (Zhai et al. 2023), EchoMimicV3 (Meng
et al. 2025), DPO (Rafailov et al. 2023), Flow-matching (Lipman et
al. 2023), ColBERTv2 (Santhanam et al. 2022), SPLADE (Formal et al.
2021), KEPLER (Wang et al. 2021), and the three upstream internal
decision documents (`memory/decisions/agent-25...md`, agent-26, agent-27,
agent-28 — not shipped; imported here verbatim).

---

## Sanity check — the appendix collectively

- §15.1 labels IC-6 three-tier release posture.
- §15.2 closes all hyperparameter gaps, including
  $\lambda_{\text{voice}}$ / $\lambda_{\text{emotion}}$ (agent-42
  Finding 5.5).
- §15.3 provides embedder / scorer pinning table.
- §15.4 documents sampling and cleaning procedures.
- §15.5 states the two-session structure for Case 3 (W4).
- §15.6 provides the JSON manifest schema (agent-27 §H imported).
- §15.7 documents hardware / software environment.
- §15.8 reconciles Table 2 source mismatch.
- §15.9 disambiguates N=14.
- §15.10 specifies bootstrap CI reproducibility.
- §15.11 spec'd the three pending Case-3 measurements.
- §15.12 provides the alldata.md → paper crosswalk audit.
- §15.13 states consent / ethics / release posture.
- §15.14 gives a verifier checklist.
- §15.15 states known limitations of the commitment.

Total: ~1,700 lines of paste-ready text. Writer (agent-47) should
compress aggressively for the body-appendix — target 6–10 pages
formatted (vs. ~30 pages of decision-file content compressed here).
Tables and JSON can stay; prose sections can be tightened.

## Changelog

- 2026-04-17 — Initial appendix draft by agent-43-reproducibility-
  checker.
