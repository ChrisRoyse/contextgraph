---
namespace: decisions
agent_id: agent-27-instrument-developer
timestamp: 2026-04-16T16:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-25-method-designer--tct-protocol.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/discoveries/agent-26-sampling-strategist--scope-of-validity.md
  - memory/handoff/agent-26-to-27--sampling-ready.md
  - memory/patterns/agent-09-methodology-scanner--standard-metrics-per-cluster.md
downstream_consumers:
  - agent-28-analysis-planner
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-43-reproducibility-checker
tags: instruments, measurement, secs, dnsmos, utmos, siglip, wavlm, ecapa, style-centroid, locked
answers_questions: [Q06, Q08, Q15]
---

# Measurement Instrument Specifications — LOCKED

## 0. Purpose and scope

Every metric reported in §6 of the manuscript must be computable
**byte-identically** by an independent replicator given the inputs
and the specification here. This document locks:

- Model identity (exact repo name, revision / commit hash where
  canonical, weight SHA-256 policy).
- Input format (sample rate, channel count, resolution, tokeniser).
- Preprocessing steps (normalise, trim, pad, centre-crop, resample).
- Output format (dimension, dtype, range).
- Norm / pooling / scoring operation (L2-norm, cosine, argmax,
  weighted sum).
- Units and directions of merit.
- Known limitations that must be disclosed in §8.

No instrument below is novel; all are pre-existing public scorers,
parametric models, or simple mathematical operations. Agent-27's role
is to **operationalise** them for reproducibility, not to invent new
psychometrics.

Hedge register carried: V4 (encoder-naming), V12 (same-session-band
framing), V22 (projected numbers), R3 / R4 (data coverage).
Bias-05 / Bias-06 / Bias-08 / Bias-09 carried on Case 3.

---

## A. Identity / Speaker-Verification Instruments

### A.1 WavLM cross-encoder SECS — **Case 3 primary guard + evaluator**

| Field | Specification |
|---|---|
| **Instrument name** | WavLM-SECS (Speaker-Embedding Cosine Similarity) |
| **Model repo** | `microsoft/wavlm-base-plus-sv` (HuggingFace) |
| **Citation** | Chen, S., Wang, C., Chen, Z., Wu, Y., Liu, S., Chen, Z., ... Wei, F. (2022). WavLM: Large-scale self-supervised pre-training for full stack speech processing. *IEEE Journal of Selected Topics in Signal Processing*, 16(6), 1505–1518. https://doi.org/10.1109/JSTSP.2022.3188113 |
| **Weight SHA-256** | Record at pipeline run time from the downloaded safetensors; stored in `embedder_version_hash.json` |
| **Input format** | Mono WAV, 16 kHz sample rate, PCM float32 in [-1, +1] |
| **Input-length constraint** | No hard upper bound; practical range 3–30 s per clip. Shorter clips produce noisier embeddings. |
| **Preprocessing (canonical WavLM)** | 1. Resample to 16 kHz if not already. 2. Mono-mix if multi-channel (mean of channels). 3. Peak-normalise loudness to approx. −23 LUFS *or* clip-peak to 0.95 (document which; we use peak-to-0.95 in Case 3). 4. Silence-trim head/tail below −40 dBFS. 5. No further filter; do **not** apply pre-emphasis or dynamic-range compression. 6. Input tensor shape `(batch, samples)`. |
| **Forward pass** | `model.extract_features(input_values, output_hidden_states=True)` — use the **speaker-verification head output** (final CLS-pooled embedding), dim 512 per HuggingFace model card. |
| **Output format** | 512-D float32 vector per clip |
| **L2 normalisation** | Applied per-clip **before** cosine. `v̂ = v / max(‖v‖₂, 1e-6)` |
| **Reference pooling (for centroid)** | Per TCT Phase 1 §1.2: (i) L2-norm per clip, (ii) mean across kept clips, (iii) L2-norm the mean. Result is the centroid `c_speaker ∈ S^{511}`. |
| **Cosine formula** | `SECS(x, c) = dot(v̂_x, c)` where `c` is already unit-norm. Range **[−1, +1]**. |
| **Reporting scale** | Raw cosine, **not** rescaled to [0,1], because the published literature (Chen et al. 2022; chen2024valle2) reports raw cosine. Manuscript headline uses the raw value. |
| **Units** | Unitless; dimensionless cosine similarity |
| **Direction of merit** | ↑ (higher = closer match) |
| **Operating threshold (Case 3)** | τ_speaker = 0.95 (strict guard), calibrated against same-session band per TCT §3.3 10th-percentile rule. V12 hedge applies. |
| **Known limitations (disclose §8)** | (L-01) Encoder-matched: WavLM is both the guard-selection scorer **and** the evaluator → the headline is a self-consistency check, not cross-encoder identity verification. (L-SECS-A) Sensitivity to codec artefacts: SECS is inflated on TTS audio with residual neural-vocoder artefacts vs. clean microphone; mitigated by WAV decoding but not eliminated. (L-SECS-B) Trained on VoxCeleb (cross-language, varied conditions); produces systematically different values from ECAPA-TDNN on the same clip (±0.10 typical; larger on short or noisy clips). (L-SECS-C) Same-session band of [0.95, 0.99] is an **author-observed** phenomenon on Case-3 subject, not a universal benchmark (V12). |

### A.2 ECAPA-TDNN SECS — **Case 3 W1 remediation (cross-encoder)**

| Field | Specification |
|---|---|
| **Instrument name** | ECAPA-TDNN SECS |
| **Model repo** | `speechbrain/spkrec-ecapa-voxceleb` (HuggingFace / SpeechBrain) |
| **Citation** | Desplanques, B., Thienpondt, J., & Demuynck, K. (2020). ECAPA-TDNN: Emphasized channel attention, propagation and aggregation in TDNN-based speaker verification. *Proc. Interspeech 2020*, 3830–3834. https://doi.org/10.21437/Interspeech.2020-2650 |
| **Weight SHA-256** | Record at run time |
| **Input format** | Mono WAV, 16 kHz, float32 [-1, +1] |
| **Preprocessing** | Identical to A.1 (resample 16 kHz mono, peak-to-0.95, silence-trim). SpeechBrain's `classifier.encode_batch` applies internal MFCC + VAD. |
| **Forward pass** | `EncoderClassifier.encode_batch(wavs)` → speaker embedding |
| **Output format** | 192-D float32 vector per clip |
| **L2 normalisation** | Applied per-clip before cosine |
| **Reference pooling (for centroid)** | Same double-L2 normalisation as A.1, applied to the **same 50 reference clips** used for WavLM centroid (so the only thing that changes is the encoder). |
| **Cosine formula** | Identical to A.1; range [−1, +1] |
| **Reporting scale** | Raw cosine |
| **Units / direction** | Unitless / ↑ better |
| **Operating threshold** | Not set; used as audit only. Report mean / max / 9-of-10-above-0.95-or-other-calibrated-floor for agreement with WavLM. |
| **Known limitations (disclose §8)** | (L-ECAPA-A) Trained on **VoxCeleb 1+2** (celebrity YouTube; compressed speech across languages/conditions) — **not** on LibriSpeech (clean studio reading). SECS values are **not directly comparable** to WavLM on the same clip; agreement (|ΔSECS| ≤ 0.10) and monotone ranking are the audit signal, not absolute value. (L-ECAPA-B) Smaller embedding dim (192 vs. 512) → finer-grained but noisier single-clip scores. (L-ECAPA-C) Pooling on short (<3 s) clips degrades faster than WavLM. |

### A.3 Qwen3-TTS 2048-D internal speaker embedding — **Case 3 vendor-internal, NOT a cross-encoder**

| Field | Specification |
|---|---|
| **Instrument name** | Qwen3-TTS internal speaker encoder |
| **Model repo** | `Qwen/Qwen3-TTS` (Alibaba; proprietary weights distributed on HuggingFace) |
| **Citation** | Qwen Team. (2024). *Qwen3-TTS technical report* (model card). https://huggingface.co/Qwen/Qwen3-TTS |
| **Weight SHA-256** | Record at run time (vendor-versioned; may change across releases) |
| **Input format** | Mono WAV, 16 kHz (vendor preprocessing internal) |
| **Preprocessing** | Vendor-internal; not exposed. |
| **Forward pass** | Not independently callable — the speaker encoder is **bundled inside** the Qwen3-TTS generator. |
| **Output format** | 2048-D internal speaker latent used by the generator to condition synthesis |
| **L2 normalisation / cosine** | **NOT USED AS A SCORER IN THIS MANUSCRIPT.** The Qwen3-TTS speaker encoder conditions generation but is **not** read out as a cross-encoder SECS. |
| **Documentation role** | This encoder is mentioned in §6.3 **only** to disclose that the generator's internal speaker representation is **architecturally independent** from the WavLM guard/evaluator. It is explicitly **not** an independent cross-encoder verification of identity. That role is reserved for A.2 (ECAPA-TDNN). |
| **Known limitations (disclose §8)** | (L-Qwen3-A) Vendor-internal; not independently auditable. (L-Qwen3-B) Must **not** be cited as a cross-encoder check — confusing vendor-internal speaker conditioning with independent verification is a category error (FM-04). |

---

## B. Naturalness / Quality Predictors

### B.1 DNSMOS P.835 (and P.808 single-score) — **Case 3 naturalness parity**

| Field | Specification |
|---|---|
| **Instrument name** | DNSMOS (Deep Noise Suppression MOS predictor) |
| **Variants** | **P.808** (single overall MOS score) and **P.835** (triad: SIG = signal quality, BAK = background quality, OVRL = overall). Case 3 headline reports P.808; P.835 is optional auxiliary. |
| **Model repo** | `microsoft/DNS-Challenge` → `DNSMOS/DNSMOS/sig_bak_ovr.onnx` and `DNSMOS/DNSMOS/model_v8.onnx` for P.808. Public ONNX distributed with the DNS-Challenge repository. |
| **Citation** | Reddy, C. K. A., Gopal, V., & Cutler, R. (2022). DNSMOS P.835: A non-intrusive perceptual objective speech quality metric to evaluate noise suppressors. *Proc. ICASSP 2022*, 886–890. https://doi.org/10.1109/ICASSP43922.2022.9746108 |
| **Input format** | Mono WAV, **16 kHz**, PCM float32 in [−1, +1] |
| **Preprocessing** | 1. Resample to 16 kHz if not already. 2. Mono-mix if needed. 3. Clip-segment into **9 s frames with 1 s overlap** per DNSMOS specification. 4. No loudness normalisation (DNSMOS was trained without it; normalising alters scores). |
| **Scoring model** | ONNX inference on each 9 s frame → per-frame MOS; aggregate by **mean over frames** per clip. |
| **Output format** | P.808: single scalar MOS in [1, 5]. P.835: triad (SIG, BAK, OVRL), each in [1, 5]. |
| **Units** | MOS (Mean Opinion Score) on the 1–5 scale |
| **Direction of merit** | ↑ (5 = excellent, 1 = bad) |
| **Reporting (Case 3)** | P.808 clone vs. real parity: 3.93 vs. 3.93 (measured). P.835 triad is optional auxiliary reporting. |
| **Known limitations (disclose §8)** | (L-DNSMOS-A) Trained on noise-suppression output (DNS-Challenge) — **not** on TTS. Extrapolation to TTS-generated audio is not validated in the source paper; parity with same-speaker real recording is the only defensible use. (L-DNSMOS-B) Sensitive to **bandwidth / sample-rate mismatch** — 16 kHz ≠ 22.05 kHz ≠ 44.1 kHz produce systematically different scores; always report sample rate. (L-DNSMOS-C) Sensitive to codec artefacts (MP3 / Opus / neural-vocoder residuals); Case 3 uses WAV throughout to avoid codec confound. |

### B.2 UTMOS — **Case 3 naturalness parity (second scorer)**

| Field | Specification |
|---|---|
| **Instrument name** | UTMOS (learned naturalness MOS predictor) |
| **Model repo** | `sarulab-speech/UTMOS22` (HuggingFace) — "UTMOS strong learner" variant (the VoiceMOS-Challenge-winning ensemble) |
| **Citation** | Saeki, T., Xin, D., Nakata, W., Koriyama, T., Takamichi, S., & Saruwatari, H. (2022). UTMOS: UTokyo-SaruLab system for VoiceMOS Challenge 2022. *Proc. Interspeech 2022*, 4521–4525. https://doi.org/10.21437/Interspeech.2022-439 |
| **Input format** | Mono WAV, **16 kHz**, PCM float32 |
| **Preprocessing** | 1. Resample to 16 kHz. 2. Mono-mix. 3. No loudness normalisation. 4. Pad to a multiple of the model's window (model handles internally). |
| **Scoring model** | HuggingFace UTMOS 22 "strong" variant on the full waveform → single MOS |
| **Output format** | Single scalar MOS in approximately [1, 5] (model occasionally exceeds 5.0 by small amounts; clip or report raw — we report raw). |
| **Units / direction** | MOS / ↑ better |
| **Reporting (Case 3)** | Clone 3.012 vs. real 2.997 (measured parity). |
| **Known limitations (disclose §8)** | (L-UTMOS-A) Trained on **BVCC** (blizzard voice conversion challenge) + VCC data — English-centric, speaker-mix heavy on TTS synthetic. (L-UTMOS-B) The "strong" ensemble is the only variant with published leaderboard numbers; the "baseline" and "light" variants produce systematically different values. (L-UTMOS-C) Like DNSMOS, the headline interpretation depends on parity with same-speaker real recording; raw values across different speakers / corpora are not directly comparable. |

---

## C. Visual-Identity Instruments

### C.1 Visual-centroid cosine (SigLIP-SO400M) — **Case 2 global visual modality**

| Field | Specification |
|---|---|
| **Instrument name** | SigLIP visual-centroid cosine |
| **Model repo** | `google/siglip-so400m-patch14-384` (HuggingFace) |
| **Citation** | Zhai, X., Mustafa, B., Kolesnikov, A., & Beyer, L. (2023). Sigmoid loss for language image pre-training. *Proc. ICCV 2023*, 11975–11986. https://doi.org/10.1109/ICCV51070.2023.01100 |
| **Weight SHA-256** | Record at run time |
| **Input format** | RGB image tensor; **384 × 384** pixels, patch size 14 |
| **Preprocessing (canonical SigLIP)** | 1. Decode frame to RGB. 2. Resize short side → 384, centre-crop to 384 × 384. 3. Normalise using SigLIP's published mean `[0.5, 0.5, 0.5]` and std `[0.5, 0.5, 0.5]` (see `preprocessor_config.json`). 4. Convert to float32 `(B, 3, 384, 384)`. |
| **Forward pass** | `vision_model(pixel_values).pooler_output` → 1,152-D float32 per frame |
| **Output format** | 1,152-D float32 vector per frame |
| **Per-frame vs. per-clip pooling** | **Per-frame**: one embedding per video frame. **Per-clip pooling**: mean across sampled frames (sampling policy: 1 frame / 0.5 s; modal frame count typically 20–40 per clip). |
| **Reference pooling (for centroid)** | (i) L2-norm per-frame, (ii) mean across kept frames within a clip → per-clip vector, (iii) L2-norm per-clip, (iv) mean across kept clips → centroid vector, (v) L2-norm the centroid. Double L2 per TCT §1.2 with an extra intra-clip pooling layer. |
| **Cosine formula** | `cos(v̂_frame, c_visual) = dot(v̂, c)`. Range [−1, +1]. Per-clip cosine aggregates by mean-of-per-frame-cosines. |
| **Reporting scale** | Raw cosine |
| **Units / direction** | Unitless / ↑ better |
| **Operating threshold (Case 2)** | τ_visual = 0.70 (operating value; T3 projected) |
| **Known limitations (disclose §8)** | (L-SigLIP-A) Trained on web-scale image-text pairs (WebLI) — may carry demographic biases in identity / expression representation. (L-SigLIP-B) 384×384 centre crop may drop peripheral subject context (hands / shoulders outside bbox). (L-SigLIP-C) Visual centroid cosine **does not measure lip-sync**; the mouth-region correctness is covered by C.2 (FLAME) + E.1 (LSE-D, pending). |

### C.2 FLAME parametric head fit — **Case 2 action-unit vocabulary, NOT the guard**

| Field | Specification |
|---|---|
| **Instrument name** | FLAME 100-D expression coefficients |
| **Model repo** | `MPI-IS/FLAME` (Max Planck Institute); fitting via **DECA** (`yfeng95/DECA`) or **EMOCA** (`radekd91/emoca`) |
| **Citation** | Li, T., Bolkart, T., Black, M. J., Li, H., & Romero, J. (2017). Learning a model of facial shape and expression from 4D scans. *ACM Transactions on Graphics*, 36(6), 1–17. https://doi.org/10.1145/3130800.3130813 |
| **Input format** | RGB face-crop image (detected by YOLOv5-Face or MediaPipe face-mesh) |
| **Preprocessing** | 1. Detect face bounding box. 2. Crop to 224×224 face region. 3. DECA / EMOCA preprocessor. |
| **Forward pass** | DECA / EMOCA inference → parametric head decomposition (pose, shape, expression) |
| **Output format** | 100-D **expression coefficient vector** (FLAME's 100 expression basis components), per frame |
| **Usage in this manuscript** | Per mission brief and Case 2 appendix: FLAME expression coefficients are used as the **Level-0 / Level-1 action-unit vocabulary** for ClipCannon's action-unit channel — **not** as the constellation-guard scorer. Agent-26 §B.1 records 3,477 frames × 100 coefficients. |
| **Cosine / scoring** | Not used as a guard. Centroid cosine on FLAME coefficients is **possible** (and would be an auxiliary action-unit similarity metric) but is not reported as a primary Case 2 result. |
| **Units / direction** | Dimensionless coefficients; magnitudes reflect expression intensity relative to neutral face |
| **Known limitations (disclose §8)** | (L-FLAME-A) FLAME is a mesh-based parametric model; expression coefficients are **not** directly interpretable as psychophysical action units (FACS) without additional mapping. (L-FLAME-B) DECA / EMOCA fitting is pose- and lighting-sensitive; out-of-distribution camera angles degrade coefficient stability. (L-FLAME-C) Because this is **not** a guard in our pipeline, it carries **no operating threshold** in §6.2. Any future use as a guard would require calibration per TCT §3.3. |

---

## D. Text-Style Instruments (Case 1)

### D.1 Shakespeare STYLE_CENTROID — **Case 1 primary guard (multi-embedder)**

| Field | Specification |
|---|---|
| **Instrument name** | Shakespeare STYLE_CENTROID — 9-embedder text-style constellation |
| **Component embedders** | E1, E4, E6, E7, E8, E9, E10, E12, E13 of the Context Graph (9 of 13 total). The 4 omitted embedders (E2, E3, E5, E11) are temporal / causal / entity encoders whose signal is not stylistic. |
| **Per-embedder citation** | Context Graph internal documentation (`docs2/contextgraphspecs/`); the underlying encoders each have their own provenance (e5-large-v2 for E1, Qodo-Embed for E7, KEPLER for E11, ColBERT for E12, SPLADE for E13, etc.). Each encoder's version hash recorded separately. |
| **Input format** | UTF-8 text (sentence-level or paragraph-level) |
| **Preprocessing** | Per-embedder canonical pipeline (E1 uses e5-large-v2 tokeniser with "query:" prefix for retrieval-style or none for document; Case 1 uses document style). Text normalised: lowercase is **not** applied (Shakespeare style depends on capitalisation). |
| **Forward pass (per embedder)** | Context Graph's `embed_all_with_metadata` — each embedder produces its dense vector. |
| **Output format** | Per embedder: the embedder's native dim. Concatenated usage is **not** applied; each embedder is scored independently and the guard is a **conjunction** of per-embedder cosine thresholds. |
| **Reference pooling (for centroid)** | For each embedder $j \in \{E1, E4, E6, E7, E8, E9, E10, E12, E13\}$: apply TCT Phase 1 double-L2 to the 50–100 Shakespeare reference passages → per-embedder centroid $c_j$. |
| **Cosine formula** | Per-embedder: $\cos(\phi_j(\hat y), c_j)$. Sparse embedders (E6, E13) use their **inverted-index scoring** — specifically cosine over sparse vector pair, producing a scalar in the same [−1, +1] range. Late-interaction (E12) uses **MaxSim** (sum of per-query-token max similarities across document tokens). |
| **Per-embedder thresholds $\tau_j$** | Calibrated per-embedder using TCT §3.3 (10th percentile of a disjoint calibration set). Initial projected range $[0.80, 0.92]$ (H2c). The manuscript reports **per-embedder CV values** (coefficient of variation across calibration set) to document threshold stability. |
| **Guard decision** | $G_\tau(\hat y) = \mathbb{1}\!\left[ \bigwedge_{j \in 9 \text{ embedders}} \cos(\phi_j(\hat y), c_j) \geq \tau_j \right]$. Strict conjunction. Case 1 also allows relaxed best-of-$K$ fallback with warning. |
| **Units / direction** | Unitless cosine (and MaxSim for E12) / ↑ better |
| **Known limitations (disclose §8)** | (L-STYLE-A) Cross-encoder validation for Case 1: the 9-embedder set is architecturally diverse (dense retrieval, late-interaction, sparse), but cross-encoder W1 remediation additionally targets a **ColBERT / BGE-reranker-from-scratch** (instrument ready, run pending). (L-STYLE-B) Shakespeare style ≠ "early modern English" uniformly; character-type stratification (soliloquy vs. dialogue vs. letter) is in the reference set but is **not** broken into per-character centroids. (L-STYLE-C) E2 / E3 / E4 / E5 / E11 are deliberately excluded; their inclusion would degrade stylistic discrimination (temporal / causal / entity signals are not stylistic). |

### D.2 Vocabulary-density prefilter — **Case 1 fast local gate (pre-centroid)**

| Field | Specification |
|---|---|
| **Instrument name** | Vocabulary-density prefilter (archaic-word-ratio threshold) |
| **Citation** | Author-constructed; no canonical literature citation. Documented in `scripts/shakespeare_full_pipeline.py` and `researchpaper/15_appendix_repro.md`. |
| **Input format** | UTF-8 text, tokenised on whitespace + punctuation |
| **Preprocessing** | 1. Tokenise. 2. Lowercase tokens for matching against the archaic-word dictionary. 3. Strip punctuation. |
| **Scoring** | `density = |tokens ∩ archaic_dict| / |tokens|`. Archaic dictionary is the author-curated list of early-modern-English markers (thee, thou, thy, hast, doth, ere, nay, yea, hither, thither, whence, etc.; approximately 200 entries). |
| **Output format** | Scalar ratio in [0, 1] |
| **Operating threshold** | $\rho_{\text{arch}} \geq 0.02$ (at least 2 % archaic tokens per passage). Soft gate, not a strict guard. |
| **Units / direction** | Ratio / ↑ better (more Shakespearean) |
| **Role in pipeline** | **Layer-2 (constrained decoding) auxiliary** — used as a fast local check at generation time; failing the prefilter triggers regeneration before the expensive 9-embedder STYLE_CENTROID runs (Layer 3). |
| **Known limitations (disclose §8)** | (L-VOCAB-A) Dictionary-based; misses syntax- and rhythm-level signals (iambic pentameter, inversions, rhetorical devices). (L-VOCAB-B) Fails on semantic Shakespearean-ness that avoids archaic vocabulary (e.g., a Shakespearean insult built from common words). (L-VOCAB-C) Author-defined dictionary — not a published standard. |

---

## E. Lip-Sync Instruments (Case 2; PENDING)

### E.1 LSE-D / LSE-C — **PENDING / future work**

| Field | Specification |
|---|---|
| **Instrument name** | LSE-D (Lip-Sync Error — Distance) / LSE-C (Lip-Sync Error — Confidence) |
| **Model repo** | SyncNet (`joonson/syncnet_python`) via Wav2Lip-evaluation protocol (`Rudrabha/Wav2Lip`) |
| **Citation** | Chung, J. S., & Zisserman, A. (2017). Out of time: Automated lip sync in the wild. *ACCV 2016 Workshops*, 251–263. https://doi.org/10.1007/978-3-319-54427-4_19 |
| **Input format** | Video (25 fps assumed by SyncNet) + synchronised mono audio at 16 kHz |
| **Preprocessing** | SyncNet canonical: face-crop per frame, mel-spectrogram per 0.2 s audio window, 5-frame visual + 16-step audio alignment windows |
| **Output format** | LSE-D = mean Euclidean distance between audio and visual SyncNet embeddings across aligned windows. LSE-C = confidence (mean-minus-offset or equivalent normalised score). |
| **Units / direction** | LSE-D in arbitrary SyncNet units; ↓ better. LSE-C similarly arbitrary; ↑ better. Both typically reported in Wav2Lip-style papers on lip-sync. |
| **Status in this manuscript** | **PENDING / NOT RUN.** Flagged in §6.2 and §8 as future-work; part of Case 2's T1 measurement deferral. |
| **Known limitations (disclose §8)** | (L-LSE-A) SyncNet is trained on natural speaker-camera setups (TV talk shows); performance on stylised or non-frontal clips is less reliable. (L-LSE-B) Frame-rate assumption: SyncNet expects 25 fps. (L-LSE-C) Not run in this manuscript; explicitly named as L-02-Case2-equivalent future work. |

---

## F. Perceptual-Hash / Reproducibility Instruments

### F.1 Content SHA-256 — **ingest provenance chain**

| Field | Specification |
|---|---|
| **Instrument name** | SHA-256 content hash |
| **Standard** | FIPS 180-4 (NIST). |
| **Input format** | Any bytestream (WAV bytes, MP4 bytes, JSONL bytes, text UTF-8 bytes). |
| **Preprocessing** | **None.** Hash is computed on the exact bytes at rest. For audio, compute on the decoded PCM float32 bytes *after* canonical preprocessing (resample to 16 kHz mono, float32 little-endian) to make the hash reproducible. This variant is documented as `pcm16k_float32_le_sha256`. Also compute `raw_file_sha256` on the original on-disk bytes. |
| **Output format** | 64-character lowercase hex string |
| **Usage** | (i) `reference_set_manifest`: each $r_{m,i}$ has a `raw_file_sha256` and `pcm16k_float32_le_sha256` pair. (ii) `embedder_version_hash`: SHA-256 over model safetensors / ONNX weights on disk. (iii) `centroid_hash`: SHA-256 over serialised centroid NPY. (iv) `threshold_set_hash`: SHA-256 over JSON of $\{\tau_m\}$. |
| **Units / direction** | N/A |
| **Known limitations (disclose §8)** | (L-SHA-A) Content-hash identity is byte-exact; two waveforms that sound identical but differ by a single sample bit will hash differently — this is the intended property for audit (V18 frozen-encoder discipline) but is not semantic identity. (L-SHA-B) Hash does not protect against deliberate pre-image attacks if the pipeline operator is adversarial; the manuscript assumes a non-adversarial operator. |

---

## G. Screening-gate operationalisations (per agent-26 §E)

### G.1 Case 1 screening

| Gate | Instrument |
|---|---|
| Text authenticity | Folger / Gutenberg canonical-passage validator (string match against canonical edition text; exact sub-string ≥ 50 chars or BLEU-4 ≥ 0.95 against canonical segment). |
| Post-cleaning non-empty | `len(stripped_response.strip()) > 0` check. |
| Duplicate-free | FSV F5: exact-string test AND E1 cosine < 0.98 against the entire held-out eval pool. |
| Cosine-novelty | Per-embedder novelty check using the STYLE_CENTROID embedder panel; rejects responses with $\max_j \cos(\phi_j(\hat y), \phi_j(r_{\text{eval}})) \geq 0.98$ for any eval example. |

### G.2 Case 2 screening

| Gate | Instrument |
|---|---|
| Clip duration | Per-modality floor check (speaker ≥ 3 s; visual ≥ 0.5 s frame coverage). |
| Audio SNR | SNR estimator: Demucs source-separated vocal track vs. residual; SNR = 10·log10(Var(vocal) / Var(residual)). Threshold ≥ 20 dB. |
| Face-detection confidence | YOLOv5-Face default + presence of at least 4 of 5 landmarks (eyes, nose, mouth corners). |
| Diarisation label | WavLM-large (`microsoft/wavlm-large`) + agglomerative clustering at cosine-distance 0.7. Subject-only segments retained. |
| Background speech | WebRTC VAD + secondary-speaker check via diarisation embedding variance within clip. |
| Scene disjointness | `scene_map` lookup: held-out scenes must have scene-ID disjoint from any training-clip scene-ID. |

### G.3 Case 3 screening

| Gate | Instrument |
|---|---|
| Clip length | `len(audio_samples) / sr ≥ 3.0`. |
| SNR | As G.2; ≥ 20 dB. |
| Single-speaker | Diarisation on the clip; reject if ≥ 2 detected speakers. |
| Lexical overlap (held-out vs. reference) | Exact-string check + lemma-set intersection (spaCy `en_core_web_sm` lemmatiser) on transcripts; held-out sentence accepted iff lemma-overlap ratio < 0.10 against every reference clip transcript. |
| Calibration-set disjointness | Clip-ID set difference against $R_{\text{speaker}}$ and held-out recordings. |

---

## H. Provenance and reproducibility manifest (per TCT §1.5 / §3.5)

Every run writes a **single JSON manifest** with the following schema:

```json
{
  "run_id": "<uuid>",
  "timestamp_utc": "<ISO-8601>",
  "case": "1 | 2 | 3",
  "phase": "1 | 2 | 3",
  "embedders": {
    "<embedder_name>": {
      "repo": "<huggingface_id_or_path>",
      "revision": "<commit_or_tag>",
      "weight_sha256": "<hex>",
      "preprocessing": {
        "sample_rate_hz": 16000,
        "channels": 1,
        "normalisation": "peak_to_0.95",
        "silence_trim_dbfs": -40.0
      },
      "output_dim": <int>
    }
  },
  "reference_set_manifest": {
    "modality": "<speaker | visual | text-style | ...>",
    "n_m": <int>,
    "n_m_kept": <int>,
    "items": [
      {"id": "<clip_id>", "raw_file_sha256": "<hex>", "pcm16k_float32_le_sha256": "<hex>"}
    ],
    "loo_stability": <float_or_null>
  },
  "centroid_hash": "<hex>",
  "threshold_set": {"<modality>": <float>},
  "threshold_set_hash": "<hex>",
  "hyperparameters": {
    "K": 12, "T": 0.3, "lambda": {"speaker": 1.0, "visual": 0.5},
    "rejection_policy": "regenerate | fallback | escalate",
    "training_mode": "inference-only | frozen-with-LoRA | full-fine-tune"
  },
  "per_output_trace": [
    {
      "eval_id": "<int>",
      "cosines": {"<modality>": <float>},
      "attempts_used": <int>,
      "decision": "accept | fallback_flag | reject_no_output"
    }
  ],
  "calibration": {
    "percentile": 10,
    "calibration_set_size": <int>,
    "tau_chosen": <float>,
    "cosine_distribution_sha256": "<hex>"
  },
  "derived_metrics": {
    "wavlm_secs_mean": 0.961,
    "wavlm_secs_max": 0.975,
    "dnsmos_p808_clone": 3.93,
    "dnsmos_p808_real": 3.93,
    "utmos_clone": 3.012,
    "utmos_real": 2.997,
    "ci95_mean_bootstrap": [0.949, 0.973]
  }
}
```

This manifest is the audit record for each case's headline numbers.
Agent-43 (reproducibility-checker) validates its completeness.

---

## I. Instrument-to-metric crosswalk

| Metric (as reported in §6) | Instrument | Case | Tier |
|---|---|---|---|
| WavLM SECS mean (0.961) / max (0.975) | A.1 WavLM-SECS | 3 | T1 MEASURED |
| ECAPA-TDNN SECS agreement (W1) | A.2 ECAPA-SECS | 3 | T1 PENDING (instrument ready) |
| DNSMOS P.808 parity (3.93 / 3.93) | B.1 DNSMOS | 3 | T1 MEASURED |
| UTMOS parity (3.012 / 2.997) | B.2 UTMOS | 3 | T1 MEASURED |
| Bootstrap 95 % CI on N = 10 (≈ 0.961 ± 0.012) | Non-parametric bootstrap on A.1 output vector | 3 | T1 MEASURED |
| Pre-selection SECS distribution (W2) | A.1 on all 12 × 10 = 120 candidates | 3 | T1 PENDING |
| $n_m$-curve (W3) | A.1 with top-$n$ reference pools | 3 | T1 PENDING |
| Case-2 visual cosine | C.1 SigLIP-SO400M | 2 | T3 PROJECTED / T1 PENDING |
| Case-2 speaker cosine | A.1 WavLM-SECS (re-used) | 2 | T3 PROJECTED / T1 PENDING |
| Case-2 emotion cosine | (emotion encoder — out of agent-27 scope, deferred) | 2 | T3 PROJECTED / T1 PENDING |
| Case-2 voice cosine | Voice-identity encoder (agent-22 master arch; out of agent-27 primary scope) | 2 | T3 PROJECTED / T1 PENDING |
| Case-2 lip-sync (LSE-D / LSE-C) | E.1 | 2 | PENDING / future work |
| Case-1 text-style cosine (9 embedders) | D.1 STYLE_CENTROID | 1 | T1 PENDING |
| Case-1 vocabulary density prefilter | D.2 | 1 | T1 PENDING |
| Content SHA-256 (all reference manifests) | F.1 | 1 / 2 / 3 | T1 operational |

---

## J. Confidence

**HIGH** that every instrument above is re-implementable by an
independent replicator from this specification alone, given public
HuggingFace model IDs and the published citations.

**HIGH** that the A.1 / A.2 / B.1 / B.2 specifications reproduce the
Case 3 headline numbers (0.961 / 0.975 / 3.93 / 3.012) byte-identically
from the same held-out inputs, because (i) each scorer is a
deterministic forward pass, (ii) preprocessing is fully locked, and
(iii) the random variation is restricted to Qwen3-TTS sampling (seed
+ temperature), which is outside the scorer.

**HIGH** that the limitations section correctly identifies:
(i) WavLM encoder-matching self-consistency, (ii) ECAPA training on
VoxCeleb (systematic offset from WavLM), (iii) DNSMOS bandwidth
sensitivity, (iv) UTMOS variant dependence, (v) SigLIP crop / bias
risks, (vi) FLAME-is-not-the-guard, (vii) vocabulary-prefilter
dictionary brittleness.

**MEDIUM** on Case-2 emotion and voice-identity encoders: these were
referenced by agent-22 / agent-26 at the constellation level but have
not been nailed to specific HuggingFace model IDs in upstream notes.
Agent-27 flags this as an open carry for agent-28 / agent-37 to
resolve before §6.2 is written.

## Changelog

- 2026-04-16 — Initial issue by agent-27-instrument-developer. Locked.
