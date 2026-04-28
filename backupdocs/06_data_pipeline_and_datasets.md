# 06 — Data Pipeline and Datasets

**Purpose.** Itemise every dataset, sensor stream, and protocol needed to bootstrap UTL v2's measurement programme and validate it. This file is the data-side companion to file 02 (which lists the embedders) — what raw signals do those embedders need, where do we get them, and what consent / ethics constraints apply.

---

## 1. The signal inventory

For each UTL term, what raw data do we need to read it?

| UTL term | Raw signal | Sample rate | Storage size |
|---|---|---|---|
| `plasticity_window` | EEG, 8–32 channels | 256–1000 Hz | ~5 MB / hour @ 256 Hz × 8 ch |
| `HRV_coherence` | PPG (wrist or finger) or ECG | 64–256 Hz PPG; 256+ Hz ECG | ~1 MB / hour @ 64 Hz |
| `valence_arousal` (face) | Webcam video | 30 fps, 720p+ | ~250 MB / hour |
| `valence_arousal` (voice) | Microphone audio | 16 kHz mono | ~30 MB / hour |
| `valence_arousal` (text) | Typed input | event-driven | <1 MB / hour |
| `stress_floor` | EDA + HR + temp + respiration | 4–64 Hz | ~5 MB / hour |
| `cortisol_z` (research-grade) | Salivary cortisol assay | 5/day | <1 KB / day |
| `predicted` | learner-typed/spoken response | event-driven | <1 MB / session |
| `actual` | task ground-truth | event-driven | <1 MB / session |
| `simulated` | learner-typed/spoken pre-attempt | event-driven | <1 MB / session |
| `recent_outcomes` | session log | event-driven | <1 MB / session |
| `sleep_stage` | wearable sleep API | 5-min | <1 MB / day |

**Total per learner per day** (assuming 4 h of monitored study + 8 h of sleep): ~3 GB/day with full-fidelity webcam, ~50 MB/day without webcam (audio + EEG + wearable only). Storage tiering follows Chris's existing pattern: sacred (raw signals + analysis-DB), regenerable (frame extracts, derived embeddings), ephemeral (intermediate logs).

---

## 2. The datasets to bootstrap each embedder

### 2.1 Public/research datasets — already accessible

| Dataset | Modality | Subjects | Use for | License |
|---|---|---|---|---|
| **WESAD** | wrist + chest physio (ECG, EDA, EMG, RESP, ACC, T) | 15 | calibrate WESAD-CNN E18; stress classifier baseline | research |
| **DAiSEE** | webcam video, 4-class affect (boredom, confusion, engagement, frustration) | 112 (9068 clips) | engagement-detection baseline | research |
| **EngageNet** | webcam video, 4-level engagement | 127 | engagement-detection cross-dataset | research |
| **CMOSE** | multi-modal online student engagement | mixed | learning-context affect calibration | research (CVPR 2024) |
| **AffWild2** | audiovisual valence/arousal in-the-wild, 594 videos, ~3M frames, 584 subjects | 584 | valence-arousal model tuning | research |
| **MSP-Podcast** | speech, dimensional emotion (A/D/V) | thousands | Wav2Vec2-MSP-DIM target | research |
| **IEMOCAP** | multimodal audio+video, dyadic emotion | 10 actors | multimodal affect baseline | research |
| **MELD** | multimodal TV-show emotion | many | dialog-affect baseline | research |
| **MAHNOB-HCI** | EEG + PPG + face + emotion-eliciting videos | 27 | EEG ↔ affect cross-modal baseline | research |
| **DEAP** | 32-channel EEG + peripheral, music-video emotion | 32 | EEG-emotion classifier baseline | research |
| **SEED / SEED-IV** | 62-channel EEG, emotion | 15 / 15 | EEG-emotion baseline | research |
| **ASCERTAIN** | EEG + ECG + GSR + face, personality-aware emotion | 58 | individual-difference modelling | research |
| **AMIGOS** | EEG + ECG + GSR, mood + personality + social | 40 | individual-difference modelling | research |
| **Sleep-EDF / SHHS** | polysomnography | hundreds | sleep-stage k(τ) calibration | open |
| **EmoBank** | text, valence-arousal-dominance | 10K sentences | text affect regressor | open |
| **GoEmotions** | text, 27 emotion categories | 58K | text affect classifier | open |
| **ALICE / IAM / Hadza** | classroom-video and learning datasets | mixed | naturalistic learning baselines | mixed |

### 2.2 Pretrained-foundation-model corpora — already absorbed by the embedders

These corpora are already built into the embedders' weights. We don't need to re-train; we only need to download the pretrained models.

| Corpus | Embedder | Hours/scale |
|---|---|---|
| LaBraM pretraining | EEG foundation model | ~2,500 h EEG |
| EEGPT pretraining | EEG foundation model | mixed |
| PaPaGei pretraining | PPG foundation model | 57,000 h PPG (20M segments) |
| Pulse-PPG pretraining | PPG foundation model | field-collected |
| MSP-Podcast (audeering Wav2Vec2) | speech emotion | 100+ h dimensional |
| Multimodal Sleep Foundation Model | sleep | 500K h |

All open weights, all runnable offline. Total disk for the foundation-model weights: ~25 GB (including LaBraM, EEGPT, PaPaGei, Wav2Vec2-MSP-DIM, OpenFace 3.0, MARLIN, MiniLM, plus Chris's production E1-E14 content stack).

### 2.3 Calibration data we need to collect

For each individual learner, before the system can make personalised predictions, run a **Phase-1 calibration session** (analogous to TCT Phase-1 centroid construction):

| What | Duration | Outputs |
|---|---|---|
| Baseline regulated session (paced 0.10-Hz breathing, neutral content, morning) | 10–20 min | personal regulated centroid C_L over E15-E21 |
| Baseline relaxed session (low-arousal music, eyes-closed, no task) | 5 min | personal alpha-dominant EEG signature |
| Baseline focus session (sustained attention task) | 10 min | personal beta-dominant EEG signature |
| Stressor session (TSST or equivalent — controlled stressor) | 15 min + 30 min recovery | personal cortisol-z baseline + recovery rate |
| Cognitive baseline (n-back, Stroop, working-memory tasks) | 15 min | per-task baseline accuracy |
| Sleep diary (1 week minimum) | event-based | individual sleep architecture |

Total bootstrap: ~3 h spread over 1 week. Repeat at 6-month intervals.

The Phase-1 session data goes into `CF_LEARNER_CONSTELLATIONS` (see file 07). Without it, all measurements are population-baseline rather than personalised, and the system's predictions are 30–50 % less accurate.

---

## 3. Hardware tiers

Three deployment tiers depending on what hardware the learner has:

### 3.1 Tier 0 — Software only

- Webcam-derived rPPG (heart rate from face video)
- Webcam-derived facial AU (OpenFace 3.0)
- Microphone (speech emotion via Wav2Vec2)
- Typed responses (text affect)
- Self-report Likert scales

Cost: $0 marginal hardware. Limitation: no EEG; no EDA; rPPG is ~3 bpm noisier than wearable.

### 3.2 Tier 1 — Consumer wearable

Add one of:

- **Apple Watch** (Series 4+) — HRV, sleep stage, blood oxygen. ~$300.
- **Polar H10** — chest strap ECG, gold-standard HRV. ~$90.
- **Empatica EmbracePlus** — research-grade wrist-worn EDA, HR, temp, ACC. ~$1,500 research; $200/month subscription.
- **Oura Ring 4** — HRV, sleep, temperature. ~$400.
- **WHOOP 4.0** — HRV, recovery, sleep. $30/month subscription.

Cost: $0–1,500 depending on choice. Adds reliable HRV + sleep + EDA (Empatica only). The full ΔE picture except EEG.

### 3.3 Tier 2 — Consumer EEG

Add one of:

- **Muse 2** — 4-channel EEG (TP9, AF7, AF8, TP10), commercial. ~$300. Limited spatial resolution but adequate for gross alpha/beta/theta classification.
- **Mendi** — 2-channel forehead fNIRS (technically not EEG; measures blood-oxygen rather than electrical). Cheaper, simpler. ~$300.
- **Emotiv Insight 2** — 5-channel EEG. ~$500.
- **OpenBCI Cyton + 8-electrode headset** — research-grade. ~$500–1500 depending on configuration.

Cost: $300–1500. Adds the plasticity_window EEG signal at 4-8 channel fidelity. LaBraM/EEGPT both work at this channel count, though full performance is at 16+ channels.

### 3.4 Tier 3 — Research-grade

- **g.tec g.HIamp / Brain Products actiCHamp** — 32-128 channel EEG with active electrodes. $10K–50K.
- **fMRI access** — university lab or contracted scan time. Not practical for routine monitoring.
- **Salivary cortisol assay kits** — ~$50/sample, lab analysis required.

Cost: institutional. Unlocks the highest-fidelity ground truth for protocol validation but is not the deployment target.

---

## 4. Recommended starter kit (Phase 0, file 09)

For a first deployment that demonstrates the framework without breaking the bank:

| Component | Cost | Provides |
|---|---|---|
| Webcam (any 1080p+) | $50 | face video |
| USB lapel microphone | $30 | clean audio |
| Polar H10 chest strap + Bluetooth dongle | $90 | HRV, ECG |
| Muse 2 headband | $300 | 4-channel EEG |
| Self-report Likert UI in dashboard | $0 | valence, arousal, stress confirmation |

**Total: ~$470 per learner.** Fits the entire ΔE pipeline including EEG. Software runs on any RTX-series GPU; the entire embedder stack fits in ~5 GB VRAM (file 02).

---

## 5. Ethics and consent

UTL measurement is high-bandwidth physiological monitoring. The following are non-negotiable:

### 5.1 Informed consent

- Explicit consent for *each modality* (webcam, mic, EEG, wearable). No bundled consent.
- Right to revoke at any time, with retroactive deletion of identifiable data.
- Plain-language explanation of what each signal reveals and how it will be used.
- Per-session consent: the learner can pause or block specific modalities for a single session.

### 5.2 Data minimisation

Following Chris's TCT ethics posture (PAPER.md Ethics section):

- **Raw audio/video** retained only briefly (24 h) for re-extraction under audit; deleted after.
- **Embeddings** retained as the primary persistent representation. Per-session embeddings carry timestamps and learner ID but not biometric raw.
- **Constellation centroid C_L** persists indefinitely as the learner's personalised baseline; this is the only long-lived biometric-derived artefact.
- **Identifiable cross-references** (face, voice) deleted after centroid construction unless explicitly opted in for longitudinal personalisation.

### 5.3 The dual-use risk

Voice + face + behavioural embeddings are exactly the inputs needed for identity-locked deepfake generation (Chris's Santa pipeline). The same data that makes UTL personalised makes a high-fidelity clone possible.

**Mitigations:**

- The constellation centroid is one-way: centroid → score, not centroid → reconstruction. The learner's centroid cannot be inverted to recover the face/voice that built it (centroid is L2-normalised mean of L2-normalised samples; many distinct samples can produce the same centroid).
- However, the *raw* samples used to build the centroid *are* invertible. Hence the 24-h retention rule.
- Strict access controls on `CF_LEARNER_CONSTELLATIONS` and per-modality data: the learner is the only entity with full access to their own constellation.
- Deployment in commercial settings (Leapable.ai) requires a commercial-grade audit trail (provenance chain on every learner-data access).
- If any raw learner audio/video is later used to generate voice, face, avatar, or meeting-agent media, that use requires a separate explicit opt-in and the generated media should carry C2PA / Content Credentials or an equivalent cryptographic provenance manifest. UTL measurement consent is not clone-generation consent.

### 5.4 Local-first deployment default

Chris's OCR Provenance architecture should be the product default for UTL:

- Raw physiological and work-product data stays on the learner's device by default.
- Processing can run in local GPU services or an explicitly selected cloud-GPU mode, but raw export is never implicit.
- Vendor-visible state is limited to license/billing records, machine identity, balances, charges, and cryptographic hashes unless the learner exports a research bundle.
- The learner can export a portable "learning evidence bundle" containing embeddings, audit logs, consent state, threshold versions, and provenance hashes without exporting raw audio/video unless they choose to.

This turns the data architecture into a privacy feature rather than an afterthought.

### 5.5 EU / regulated deployment constraints

The EU AI Act raises two immediate constraints for productization:

- AI systems used to evaluate learning outcomes or steer a learner's educational path can be Annex III high-risk systems.
- Emotion recognition in education and workplace contexts is prohibited except narrow medical/safety cases.

**Operating rule:** in regulated education or workplace deployments, UTL should be framed as learner-controlled self-measurement and human-reviewed support. Do not expose educator/employer dashboards that infer emotions, do not automate admissions/grading/proctoring, and do not make physiological state a gate for access to education or employment. If a deployment crosses into outcome evaluation or learning-path steering, treat it as high-risk from the start: risk management, data governance, technical documentation, logs, transparency, human oversight, accuracy/robustness/cybersecurity, and registration obligations.

### 5.6 Special populations

- **Minors (under 16):** UTL programme not deployed without parental consent + age-appropriate explanation. Aligns with Chris's Algorithmic-Foreclosure thesis stance: minors are particularly vulnerable to algorithmic mediation.
- **Trauma-affected learners (PTSD, etc.):** UTL v2 Section 11.4 specifically prescribes state-first protocols. Deployment to this population requires clinical co-design.
- **Workplace deployment:** opt-in only; never as a condition of employment; no employer access to individual learner data, only aggregate cohort statistics.

### 5.7 IRB / ethics-board review

For any deployment that intends to publish or that involves research data collection beyond product use, IRB review is required. UTL's measurement profile is similar to existing affective-computing research and should be approvable under standard human-subjects protocols.

---

## 6. The data flow

End-to-end signal-to-storage pipeline:

```
Sensors → Raw signals (24h retention) →
  Per-modality embedder forward pass (E15-E21) →
  Per-session embedding (persistent in CF_FINGERPRINTS_LEARNER) →
  Aggregation into ΔS, ΔC, ΔE (persistent in CF_LEARNER_M_PER_TRACE) →
  Update to learner constellation C_L (persistent in CF_LEARNER_CONSTELLATIONS) →
  MCP tools expose per-learner queries (audit log in CF_LEARNER_AUDIT) →
  Dashboard displays L, M, T, diagnostic states
```

Storage tiers map to Chris's existing pattern:

| Tier | Items | Lifetime |
|---|---|---|
| Sacred | analysis DB, constellation centroids, audit log | indefinite |
| Regenerable | per-session embeddings (can be re-derived from raw if available) | retained while raw retained |
| Ephemeral | raw audio/video/EEG (24-h auto-delete unless extended consent) | 24 h default |

---

## 7. Naturalistic vs. lab-controlled measurement

Two complementary protocols:

### 7.1 Lab-controlled (high SNR, low ecological validity)

- Use during initial calibration (Phase 1) and during validation experiments (file 08).
- Constrained environment: same room, same lighting, same time of day, same equipment.
- Per-session consent and instrumentation refresh.
- Yields high-quality data for embedder calibration and threshold setting.

### 7.2 Naturalistic (lower SNR, high ecological validity)

- Use during ongoing learning sessions in the wild.
- Variable environment, variable equipment, variable distractions.
- Per-session calibration check (a 30-second baseline reading at session start).
- Yields long-term real-world data for the M(t) integral and T(train, deploy) measurements.

The two are linked: lab-controlled data anchors the embedder thresholds; naturalistic data tracks the trajectory through embedder space across days/weeks/months.

---

## 8. Cohort design for validation

To validate UTL v2 predictions (file 08 lays out the experiments), the recommended cohort design:

| Group | N | Variation | Purpose |
|---|---|---|---|
| Calibration cohort | 20 | full Phase-1 + naturalistic 4-week | embedder threshold calibration |
| Intervention cohort | 50 | UTL-aware curriculum (regulate before content) vs. control (standard curriculum) | Experiment 1: state regulation effect |
| Cross-state transfer cohort | 30 | train in calm condition, test in stress condition (or vice versa) | Experiment 2: T term validation |
| Long-term consolidation cohort | 20 | 12-week multi-modal retention curve | Experiment 3: M(t) integral fit |
| FSRS-vs-UTL cohort | 50 | UTL-state-aware FSRS vs. vanilla FSRS | Experiment 4: state-aware spaced repetition |
| AI-native process cohort | 80 | AI-allowed process assessment vs. answer-only grading | Experiment 7: process traces predict transfer |

Total: 250 subjects if cohorts are independent, ~6 months for the full battery. Cost: $60K–170K depending on stipend rates, equipment, and whether Experiment 7 is embedded in an existing class. This is publication-grade.

---

## 9. Datasets we already have inside Chris's stack

A side-effect of Chris's existing infrastructure: ClipCannon's Santa training data is *exactly* the structure UTL v2 wants for cross-modal calibration. The 16-minute single-subject interview, with seven modalities embedded, plus the FLAME action-unit time series, is a learner-state dataset of unusually high quality. It can serve as:

- The first **regulated-state centroid** baseline for the learner identified as Santa.
- A **time-aligned multi-modal benchmark** for cross-encoder ΔE consistency checks (do all 7 modalities agree on the affective state at each timepoint?).
- A **proof-of-concept dataset** for the AffWild2-style validation but at much higher per-modality fidelity than AffWild2 provides.

This is essentially a free deliverable from existing work. Section 09 (roadmap) Phase 1 starts here.

---

## Sources

- [WESAD: Wearable Stress and Affect Detection](https://dl.acm.org/doi/10.1145/3242969.3242985)
- [DAiSEE: Towards User Engagement Recognition in the Wild](https://arxiv.org/abs/1609.01885)
- [EngageNet — Do I Have Your Attention dataset](https://www.researchgate.net/publication/374552867_Do_I_Have_Your_Attention_A_Large_Scale_Engagement_Prediction_Dataset_and_Baselines)
- [CMOSE: Comprehensive Multi-Modality Online Student Engagement Dataset (CVPR 2024)](https://openaccess.thecvf.com/content/CVPR2024W/ABAW/papers/Wu_CMOSE_Comprehensive_Multi-Modality_Online_Student_Engagement_Dataset_with_High-Quality_Labels_CVPRW_2024_paper.pdf)
- [AffWild2 / ABAW Challenges](https://ibug.doc.ic.ac.uk/resources/aff-wild2/)
- [MSP-Podcast Corpus](https://msp.utdallas.edu/Resources/MSP-PODCAST/)
- [IEMOCAP Database](https://sail.usc.edu/iemocap/)
- [MELD Multimodal EmotionLines Dataset](https://affective-meld.github.io/)
- [MAHNOB-HCI Database](https://mahnob-db.eu/hci-tagging/)
- [DEAP Dataset](http://www.eecs.qmul.ac.uk/mmv/datasets/deap/)
- [SEED Database](https://bcmi.sjtu.edu.cn/home/seed/)
- [ASCERTAIN Dataset](http://mhug.disi.unitn.it/wp-content/ASCERTAIN/ascertain.html)
- [AMIGOS Dataset](http://www.eecs.qmul.ac.uk/mmv/datasets/amigos/)
- [Sleep-EDF (PhysioNet)](https://physionet.org/content/sleep-edfx/)
- [EmoBank](https://github.com/JULIELab/EmoBank)
- [GoEmotions](https://github.com/google-research/google-research/tree/master/goemotions)
- [Datasets for Valence and Arousal Inference: A Survey](https://arxiv.org/html/2510.00738v1)
- [A review and critical analysis of multimodal datasets for emotional AI](https://link.springer.com/article/10.1007/s10462-025-11271-1)
- [PAPER.md TCT framework, ethics posture](../PAPER.md)
- [ClipCannon Provenance System](../../../clipcannon/docs2/clipcannondocs/05_provenance_system.md)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
- [C2PA Content Credentials Explainer](https://spec.c2pa.org/specifications/specifications/2.2/explainer/Explainer.html)
- [EU AI Act Service Desk: Annex III](https://ai-act-service-desk.ec.europa.eu/en/ai-act/annex-3)
- [EU AI Act Service Desk FAQ](https://ai-act-service-desk.ec.europa.eu/en/faq)
