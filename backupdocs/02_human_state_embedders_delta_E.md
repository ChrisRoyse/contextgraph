# 02 — Human State Embedders for ΔE

**Purpose.** Map every subcomponent of UTL v2's `ΔE = (plasticity_window)^0.4 · (valence_arousal)^0.4 · (stress_floor)^0.2` to a concrete frozen embedder that can produce it from raw signals on commodity hardware. ΔE is the new contribution in v2; this is where the embedder programme has the most to add.

**Architecture choice.** Following Chris's DDA / TCT pattern, every embedder is **frozen, independently trained, and used in three roles simultaneously**: retriever (search learner state by similarity), labeller (produce ΔE per moment), and runtime guard (cosine-to-regulated-centroid check). No parameter is updated. The recommended slot allocation is E14–E20 in Context Graph, on top of the existing E1–E13.

---

## 1. The plasticity-window subcomponent (EEG)

### 1.1 What we need to read

`plasticity_window ∈ [0, 1]` mapped from dominant cortical oscillation:

| State | Dominant rhythm | Value |
|---|---|---|
| High beta (anxious) | 22–30 Hz | 0.2 |
| Low beta (focused) | 13–22 Hz | 0.6 |
| Alpha (relaxed) | 8–13 Hz | 0.85 |
| Theta (light trance) | 4–8 Hz | 1.0 |
| Delta (deep sleep) | <4 Hz | 0 (online L), enters consolidation `k(τ)` |

### 1.2 The 2024–2025 EEG foundation-model landscape

A comprehensive benchmark, **EEG-FM-Bench** (arXiv:2508.17742), evaluates seven recent EEG foundation models and two general time-series baselines. Findings:

- **LaBraM** — pre-trained on ~2,500 hours of EEG; consistently strong but vector-quantisation may limit fine-grained representational capacity. ~50M parameters.
- **EEGPT** — introduced spatio-temporal alignment; performs well on emotion recognition and artifact detection; dual self-supervised pre-training yields representations less susceptible to low SNR.
- **CBraMod** — recent strong performer.
- **CSBrain** — recent strong performer.
- **REVE** — recent.
- **BIOT** — channel-independent tokenisation enables cross-dataset learning, but generic transformer backbone is moderate on spatio-temporal tasks.
- **BENDR** — early EEG foundation model adapted from speech; lags on complex cognition tasks.

The benchmark uses gradient cosine similarity, subspace affinity, and Representational Similarity Analysis to compare models — these are the same RSA tools we'll use to wire EEG-FM output into the Context Graph (Section 07).

### 1.3 Recommendation

**Primary: LaBraM** for general use; **EEGPT** for artifact-robust scenarios. Both are open-weight and ~50–100M parameters, fitting comfortably in 1–2 GB VRAM on the RTX 5090.

### 1.4 Practical reduction to `plasticity_window`

EEG foundation models output per-channel hidden representations. To reduce to a single ΔE-relevant scalar:

1. Take 10-second windows of raw EEG (8–32 channels typical; 2-channel headbands like Muse 2 / Mendi work but have higher noise).
2. Forward through frozen LaBraM/EEGPT.
3. Decode dominant band via either: (a) a pre-trained band-power classifier (5-class softmax over delta/theta/alpha/low-beta/high-beta), or (b) compute spectral power directly on the raw signal using Welch's method and feed the band ratios to a calibrated lookup. Method (b) is simpler and the band ratios are stable across subjects.
4. Map dominant band to the v2.0 lookup table.
5. Optional refinement: theta–gamma coupling (see Buzsáki 2002) is an even better predictor of consolidation; LaBraM's output supports computing this from the cross-frequency phase-amplitude coupling of its hidden states.

### 1.5 No-EEG fallback (proxies)

UTL v2 already names proxies: time of day, breathing rate, posture, recent caffeine, recent stressor exposure, self-report "settled vs. wired." These can be combined into a logistic-regression scalar in [0, 1] with a small calibration set. This is the "minimum viable measurement" path of Section 09.

---

## 2. The valence-arousal subcomponent

### 2.1 What we need to read

A 2D vector (valence, arousal) per moment, then `valence_arousal = sigmoid(valence) × inverted_U(arousal)`. The inverted-U is Yerkes-Dodson (1908) — too low = drowsy, too high = stress-locked.

### 2.2 Multi-modal sources for affect

**Speech (audio).** Wav2Vec2-large-robust-12-ft-emotion-msp-dim from audeering (HuggingFace) outputs 3D dimensional emotion (arousal, dominance, valence) directly. CCC = 0.676 valence on MSP-Podcast in a 2024 benchmark; valence is the hardest dimension to recognise (Wav2Small at 72K params reaches CCC = 0.37; full-size models reach 0.6+). WavLM won the 2024 SER categorical challenge.

**Face (video).** OpenFace 3.0 (CMU MultiComp Lab, 2025) is a lightweight multi-task system: facial landmarks, action-unit detection, eye-gaze estimation, emotion recognition. Apache-2.0; runs at 30+ fps on consumer GPUs. Py-Feat (Affective Science 2023) is a Python toolkit with comparable AU detection plus visualisation. MARLIN is a self-supervised facial video encoder pre-trained on YouTube Faces with masked-autoencoder objective; its 768-D embeddings are useful as a face-specific representation alongside generic vision transformers.

**Text (typed input).** Sentence-transformers + EmoBank / GoEmotions classifiers give valence/arousal on textual responses or chat input. This is the lowest-hardware-cost channel and the right place to start for chat-based learning.

**Multimodal fusion.** AffWild2 (594 videos, ~3M frames, 584 subjects, valence/arousal annotated) is the canonical benchmark. CMU-MOSEI extends to multimodal (audio + video + text). The 2024 ABAW competition winners use Wav2Vec2 + a face encoder + cross-modal attention.

### 2.3 Recommendation

**Three independent affect embedders, one per modality:**

| Slot | Embedder | Modality | Output | VRAM |
|---|---|---|---|---|
| E14 | Wav2Vec2-large-MSP-DIM (audeering) | speech | (arousal, dominance, valence) | ~1.5 GB |
| E15 | OpenFace 3.0 emotion head | face video | (valence, arousal) + 7-class categorical + 17 AUs | ~0.5 GB |
| E16 | sentence-transformers/all-MiniLM-L6-v2 + linear regressor on EmoBank | text | (valence, arousal) | ~0.3 GB |

The three are deliberately encoder-diverse so that cross-encoder agreement gives an independent confidence signal (Section 03's coherence ΔC term). When all three converge on the same valence/arousal point, ΔE is high-confidence; when they disagree, signal a "modality conflict" warning.

### 2.4 Practical reduction to `valence_arousal`

For each modality, output (v, a) ∈ [-1, 1]². Compute weighted average across modalities by inverse-variance-weighting (modalities that are more reliable for this learner get higher weight; calibrate per learner from baseline session). Apply UTL v2's reduction:

```
v_a = sigmoid(valence_avg) · inverted_U(arousal_avg)
inverted_U(a) = 4 · a · (1 - a)        # peaks at 0.5, zero at endpoints
```

Returns scalar in [0, 1].

---

## 3. The stress-floor subcomponent (cortisol-z proxy)

### 3.1 What we need to read

`stress_floor ∈ [0.1, 1.0]` from cortisol_z mapping (UTL v2 Section 5.3 piecewise function). Direct salivary-cortisol measurement is impractical for moment-to-moment use; we need proxies that correlate with cortisol on the relevant timescale.

### 3.2 Wearable proxies

**Heart-rate variability** is the classical cortisol proxy: HRV drops as cortisol rises, and the relationship is strong enough that HRV-derived stress classifiers reach 87 % accuracy on the WESAD benchmark. PaPaGei (ICLR 2025, Nokia Bell Labs) is the **first open foundation model for PPG signals**, pre-trained on 57,000 hours of unlabelled PPG (20M segments), evaluated on 20 tasks across 10 datasets, AUROC 0.769 on cardiovascular screening (vs. 0.610 for zero-shot NormWear). It is the obvious primary embedder for HRV-based stress estimation. A companion model **Pulse-PPG** (2025) is an open-source field-trained PPG foundation model.

**Electrodermal activity (EDA / GSR)** is the second-most-reliable cortisol proxy on minute-to-hour timescales. WESAD (2018, ACM ICMI) bundles ECG, EDA, EMG, respiration, body temperature, three-axis accelerometer from 15 subjects in three states (neutral, stress, amusement). 2024 deep-learning approaches on WESAD reach 87.7 % accuracy (Kumar 2024), F1 = 85.2 %; CNNs and RNNs are both viable.

**Multimodal fusion.** A 2024 systematic review on stress detection in free-living environments confirms that ECG + EDA + RESP is the canonical multi-modal stress fingerprint.

### 3.3 Recommendation

| Slot | Embedder | Hardware | Output | VRAM |
|---|---|---|---|---|
| E17 | PaPaGei (PPG → 512-D) | wearable PPG (Apple Watch, Polar H10, Empatica E4) | 512-D HRV/cardiovascular embedding | ~0.5 GB |
| E18 | WESAD-trained CNN on EDA + HR | E4 wristband or equivalent | 4-class state classifier (baseline / stress / amusement / meditation) | ~0.2 GB |

Combine via:

```
stress_floor = piecewise_lookup(predict_cortisol_z(PaPaGei_emb, EDA_emb))
```

Where `predict_cortisol_z` is a small per-learner regressor calibrated from a baseline session with self-reported state (or an actual salivary cortisol assay if research-grade).

### 3.4 No-wearable fallback

For learners without a wearable, a reasonable proxy chain is:

1. Self-report (1–10 stress scale) → maps to cortisol_z via population distribution.
2. Webcam-derived heart rate via remote photoplethysmography (rPPG). 2024 work on Doppler-radar HR achieves <2 bpm error; rPPG is ~3 bpm. Lower fidelity than wearable but no extra hardware needed.
3. Voice tremor / shimmer / jitter from speech (acoustic features that correlate with arousal).

---

## 4. Combining subcomponents into ΔE

### 4.1 The flat form (UTL v2 default)

```
ΔE = (plasticity_window)^0.4 · (valence_arousal)^0.4 · (stress_floor)^0.2
```

Each subcomponent ∈ [0, 1]; product is in [0, 1]; any subcomponent at zero forces ΔE = 0.

### 4.2 The differentiated form (recommended for v2.1, see file 01)

Different subcomponents gate different stages:

```
ΔS_eff = ΔS · stress_floor                          # cortisol gates ΔS sensitivity
ΔC_eff = ΔC · HRV_coherence^0.5                     # HRV gates integration
k_state = plasticity_window^0.5 · k_sleep            # plasticity gates consolidation rate
M(t) = ∫ ΔS_eff(τ) · ΔC_eff(τ) · stress_floor(τ) · k_state(τ) · e^{-(t-τ)/τ_decay} dτ
```

This is stricter (each stage has its own gate) but factors more cleanly into embedder readouts. Section 07 uses this form for the Context Graph integration.

---

## 5. The personal-centroid pattern (TCT for learners)

This is the bridge to Chris's existing system. For each learner, build a **regulated-state centroid** `C_L = {(m, c_m)}` over the same panel of frozen embedders, where each `c_m` is the L2-normalised mean projection of *their own* baseline regulated sessions (Phase-1 in TCT terminology).

**Building the centroid (Phase 1).**

1. Acquire a baseline session: 10–20 minutes of paced 0.10-Hz breathing, typing or speaking neutral content, no stressors, in the morning post-wake.
2. Embed every 10-second window through E14–E20.
3. L2-normalise per sample; average; L2-normalise the mean. Result: `c_m` for each modality.
4. Persist to a new column family `CF_LEARNER_CONSTELLATIONS` (Section 07).

**Runtime guard (Phase 3).**

```
G_τ(state_t; C_L) = 1 if ∀m ∈ {plasticity, valence, arousal, HRV, EDA}: cos(φ_m(state_t), c_m) ≥ τ_m
                  = 0 otherwise
```

When the guard fires, the coaching cycle (UTL v2 Part VIII) regulates state before delivering content.

**Threshold calibration.** Same procedure as TCT Phase 3: collect a calibration set of authentic baseline samples disjoint from the centroid set, set `τ_m` to the 10th percentile of the cosine distribution. This gives a nominal 10 % false-reject rate per modality. With strict conjunction across 5 modalities, the joint false-reject rate is approximately 1 - (0.9)^5 ≈ 41 %, which is too strict; relax to 5th percentile per modality (joint ~23 %) or use a soft-rejection rule based on mean cosine.

---

## 6. Hardware and VRAM budget

Total embedder stack on the RTX 5090 (32 GB total, 28.6 GB usable per Phoenix gpu_guard):

| Slot | Embedder | VRAM |
|---|---|---|
| E14 | Wav2Vec2-MSP-DIM | 1.5 GB |
| E15 | OpenFace 3.0 | 0.5 GB |
| E16 | MiniLM emotion regressor | 0.3 GB |
| E17 | PaPaGei | 0.5 GB |
| E18 | WESAD CNN | 0.2 GB |
| E19 | LaBraM (EEG) | 1.2 GB |
| E20 | EEGPT (artifact-robust EEG) | 1.2 GB |
| **Total ΔE stack** | | **~5.4 GB** |

Plus the existing E1–E13 stack (~9.5 GB on Context Graph at default). Plus Hermes-2-Pro-Mistral-7B (~6 GB Q4) for causal/graph agents. Total: ~21 GB, within budget. This leaves headroom for the EchoMimicV3 LoRA on the same machine when needed.

---

## 7. Open-vs-closed access status (April 2026)

| Embedder | License | Weights public | Pre-trained on | Notes |
|---|---|---|---|---|
| LaBraM | open | yes (HF) | ~2,500 h EEG | strong general baseline |
| EEGPT | open | yes (HF) | mixed EEG corpora | best on artifact-noisy data |
| PaPaGei | open (CC) | yes (Nokia GitHub) | 57,000 h PPG | ICLR 2025 |
| Pulse-PPG | open | yes (GitHub) | field-collected | 2025 alternative to PaPaGei |
| Wav2Vec2-MSP-DIM | open | yes (audeering HF) | MSP-Podcast | dimensional emotion |
| OpenFace 3.0 | Apache-2.0 | yes (CMU GitHub) | mixed face datasets | lightweight, multi-task |
| Py-Feat | open | yes (PyPI) | mixed | research toolkit |
| MARLIN | open | yes | YouTube Faces | masked-autoencoder face encoder |
| WESAD | research | yes | 15 subjects, 3 states | benchmark dataset, train your own classifier |

All public weights, all runnable offline (consistent with Chris's HF_HUB_OFFLINE=1 hard rule). No vendor dependencies.

---

## 8. Evaluation: how we know the embedders are reading the right thing

For each embedder, define **two evaluation gates**:

1. **Within-encoder same-session-band.** Two recordings of the same person in the same session with the same equipment should produce cosine ≥ 0.95 in the embedder's space (this is the same-session band concept Chris's voice-cloning paper validates at 0.961 mean WavLM SECS). If this fails, the embedder is too noisy.
2. **Cross-encoder triangulation.** When two architecturally different embedders (e.g., Wav2Vec2 vs. WavLM, or OpenFace vs. MARLIN) agree on the (valence, arousal) prediction within ±0.2 cosine, treat as high-confidence; when they disagree by more, flag for human review or downweight that modality's contribution.

This is exactly the encoder-matched-scope discipline Chris's TCT paper enforces, applied to a different domain.

---

## 9. Mappings recap

Every UTL v2 ΔE term has a frozen embedder. Differentiated form:

| UTL v2 term | Embedder | Output reduction |
|---|---|---|
| `plasticity_window` | LaBraM (E19) → band-power head | scalar in [0,1] |
| `HRV_coherence` | PaPaGei (E17) → spectral resonance head | scalar in [0,1] |
| `valence` | Wav2Vec2-MSP-DIM (E14) ⊕ OpenFace 3.0 (E15) ⊕ MiniLM (E16) | scalar in [-1,1] |
| `arousal` | Wav2Vec2-MSP-DIM (E14) ⊕ OpenFace 3.0 (E15) | scalar in [-1,1] |
| `stress_floor` | PaPaGei (E17) ⊕ WESAD CNN (E18) → cortisol_z regressor | scalar in [0.1, 1.0] |
| `valence_arousal` | sigmoid(valence) · inverted_U(arousal) | scalar in [0, 1] |
| `regulated-state centroid C_L` | per-learner mean over E14–E20 baseline | constellation object |

This is the entire ΔE side of UTL v2 made operational.

---

## Sources

- [EEG-FM-Bench: A Comprehensive Benchmark for the Systematic Evaluation of EEG Foundation Models](https://arxiv.org/html/2508.17742v1)
- [EEG Foundation Models: A Critical Review of Current Progress and Future Directions](https://arxiv.org/html/2507.11783v3)
- [PaPaGei: Open Foundation Models for Optical Physiological Signals (ICLR 2025)](https://arxiv.org/abs/2410.20542)
- [PaPaGei GitHub (Nokia Bell Labs)](https://github.com/nokia-bell-labs/papagei-foundation-model)
- [Pulse-PPG: An Open-Source Field-Trained PPG Foundation Model](https://arxiv.org/html/2502.01108v1)
- [Foundation model embeddings enable cardiovascular screening using wearable signals](https://www.medrxiv.org/content/10.1101/2025.10.11.25337800v1.full)
- [audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim — HuggingFace](https://huggingface.co/audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim)
- [Wav2Small: Distilling Wav2Vec2 to 72K Parameters for Low-Resource Speech Emotion Recognition](https://arxiv.org/html/2408.13920)
- [Speaker Emotion Recognition: Leveraging Self-Supervised Models for Feature Extraction Using Wav2Vec2 and HuBERT](https://arxiv.org/abs/2411.02964)
- [emotion2vec: Self-Supervised Pre-Training for Speech Emotion Representation](https://aclanthology.org/2024.findings-acl.931.pdf)
- [OpenFace 3.0: A Lightweight Multitask System for Comprehensive Facial Behavior Analysis](https://arxiv.org/html/2506.02891v1)
- [Py-Feat: Python Facial Expression Analysis Toolbox](https://link.springer.com/article/10.1007/s42761-023-00191-4)
- [MARLIN: Masked Autoencoder for facial Representation Learning](https://github.com/ControlNet/MARLIN)
- [WESAD: Multimodal Dataset for Wearable Stress and Affect Detection](https://dl.acm.org/doi/10.1145/3242969.3242985)
- [Cross-Modality Investigation on WESAD Stress Classification (2024)](https://arxiv.org/html/2502.18733v1)
- [State-of-the-Art of Stress Prediction from Heart Rate Variability Using AI](https://link.springer.com/article/10.1007/s12559-023-10200-0)
- [Promoting cross-modal representations to improve multimodal foundation models for physiological signals](https://arxiv.org/html/2410.16424v1)
- [Towards Robust Multimodal Physiological Foundation Models: Handling Arbitrary Missing Modalities](https://arxiv.org/html/2504.19596v1)
- [A review on EEG-based multimodal learning for emotion recognition (2025)](https://link.springer.com/article/10.1007/s10462-025-11126-9)
- [An Enhanced Valence-Arousal Multimodal Emotion Dataset for Emotion Recognition (2025)](https://www.nature.com/articles/s41597-025-06214-y)
