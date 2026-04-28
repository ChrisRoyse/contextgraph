---
namespace: patterns
agent_id: agent-08-citation-extractor
timestamp: 2026-04-16T17:50:00Z
status: active
tags: claim-to-citation, q01-q20, primary-supporting, writer-ready
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
---

# Claim → Citation Index (Q01–Q20)

Downstream writers cite by **key** (e.g., `villalobos2024data`). Every key below is
defined in `decisions/agent-08-citation-extractor--master-reference-pool.md`.
Each question lists **primary** (must-cite for the claim to stand) and **supporting**
(strengthens / corroborates) citations. Minimum body-cite count of 3 met for every
question; most are 5–8.

Legend:
- **P** = Primary (the claim fails its provenance check without this citation).
- **S** = Supporting (corroborates, contextualises, or illustrates).
- **(T1)** / **(T2)** / **(T3)** = source tier from the master pool.

---

## Q01 — What exactly is "the data wall"? Why reframe it now?

**Owner sections.** §1 Introduction; re-cited §3.3.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | villalobos2024data | T1 | 300T median stock; 2026–2032 exhaustion band |
| P | longpre2024consent | T1 | accessible fraction shrinking (≈25% high-quality C4 restricted) |
| P | hoffmann2022chinchilla | T1 | prior models undertrained; Chinchilla-scaling → wall arrives faster |
| S | kaplan2020scaling | T2 | power-law scaling foundation |
| S | sutskever2024neurips | T3 | oral remark on "pre-training as we know it will end" |
| S | woo2024information | T3 | industry signal, diminishing returns |
| S | metz2024bloomberg | T3 | industry signal, struggle to build more advanced AI |
| S | hu2024reuters | T3 | industry signal, seeking new path |
| S | franzen2024techcrunch | T3 | industry signal, scaling laws flattening |

**Body-pool target: 4–6 citations.** Primary triad (villalobos / longpre / hoffmann)
plus at most 2 T3 industry signals. Kaplan optional if §1 first-uses "scaling law".

---

## Q02 — Model-collapse risk (Shumailov 2024) and why DDA structurally avoids it

**Owner sections.** §2.2 Related Work; §3.3; §8.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | shumailov2024nature | T1 | the Nature paper; early/late collapse; GMM/VAE/LLM |
| P | alemohammad2023mad | T1 | "self-consuming" failure mode (image generators) |
| S | gerstgrasser2024colm | T2 | accumulation mitigation (still generator-in-loop) |
| S | dohmatob2024tails | T1 | scaling-law crossover under synthetic contamination |
| S | dohmatob2024regression | T1 | closed-form regression collapse analysis |
| S | feng2024verification | T2 | verification recovers scaling (still generator-in-loop) |
| S | seddik2024collapse | T2 | statistical analysis of LM collapse |

**Body-pool target: 3–5 citations.** The "DDA is silent on collapse" sentence needs
shumailov2024nature + alemohammad2023mad as the two primaries; the others corroborate.

---

## Q03 — Existing mitigations for the data wall; what gap remains

**Owner sections.** §2.1 Related Work; §1 three-paths framing.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | villalobos2024data | T1 | path 1 (acquire more) framing |
| P | hoffmann2022chinchilla | T1 | data-efficient-training path |
| P | shumailov2024nature | T1 | path 2 (synthesize) risks |
| S | gerstgrasser2024colm | T2 | path 2 mitigations (accumulate) |
| S | feng2024verification | T2 | path 2 mitigations (verify) |
| S | longpre2024consent | T1 | accessible stock declining (path 1 headwind) |
| S | henderson2022pileoflaw | T1 | path-1 licensed-corpus exemplar (optional) |
| S | openai2024reddit | T3 | path-1 licensing-deal exemplar (use sparingly) |

**Body-pool target: 5–7 citations.**

---

## Q04 — Derived Data Abundance (DDA), formal definition; $N + \binom{N}{2}$

**Owner sections.** §3.1 concept; §3.4 counting.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2022wavlm | T1 | example frozen independent embedder (Context Graph E-slot) |
| P | zhai2023siglip | T2 | example frozen independent embedder (visual) |
| P | deletang2024compression | T1 | compression-as-intelligence background |
| S | huang2024compression | T2 | linear compression–intelligence correlation |
| S | nussbaum2024nomic | T2 | reproducible text embedder (substrate evidence) |
| S | santhanam2022colbertv2 | T1 | late-interaction retrieval precedent (substrate) |

**Body-pool target: 3–5 citations.** The $N + \binom{N}{2}$ counting argument itself is
theoretical (T4); citations establish that the embedders exist and are independent.

---

## Q05 — Meaning compression vs. bit / weight / activation compression

**Owner sections.** §3.1 four-category taxonomy; §3.5.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | deletang2024compression | T1 | bit-compression anchor (prediction = compression) |
| P | huang2024compression | T2 | r≈0.95 compression–intelligence correlation |
| P | li2025lossless | T1 | LLM arithmetic coding across modalities |
| S | valmeekam2023llmzip | T2 | LLMZip 0.71 bpc |
| S | jiang2023gzip | T1 | gzip+k-NN low-resource classification |

**Body-pool target: 3–5 citations.** All Cluster-C citations are in-scope.

*Note.* Weight-compression (GPTQ, SparseGPT) and activation-compression (TurboQuant)
referents are descriptively named in §3.5 but not cited to this pool — agent-11 may
add if depth of the taxonomy table needs it.

---

## Q06 — TCT vs. RLHF / DPO / CAI scalar-reward alignment

**Owner sections.** §5 Method; re-cited §2.6 Related Work.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | ouyang2022instructgpt | T1 | RLHF / InstructGPT canonical |
| P | rafailov2023dpo | T1 | DPO canonical |
| P | bai2022constitutional | T2 | Constitutional AI canonical |
| S | lipman2023flowmatching | T1 | TCT geometry substrate (flow-matching) |
| S | hu2022lora | T1 | LoRA as TCT fine-tuning substrate |

**Body-pool target: 5 citations** (this is the Q06 anchor that agent-07 flagged as
missing; the three primaries are **mandatory**).

---

## Q07 — Three-phase TCT protocol (centroids → training → runtime guard)

**Owner sections.** §5.1, §5.2, §5.3.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | lipman2023flowmatching | T1 | flow-matching training substrate |
| P | hu2022lora | T1 | LoRA fine-tuning substrate |
| P | meng2025echomimicv3 | T2 | Case 2 instantiation (per-modality centroid targets on human animation) |
| S | chen2022wavlm | T1 | audio-speaker modality centroid scorer |
| S | reddy2022dnsmos | T2 | supplementary evaluator in guard |
| S | saeki2022utmos | T2 | supplementary evaluator in guard |

**Body-pool target: 4–6 citations.**

---

## Q08 — Per-modality thresholds $\tau_m$ — calibration

**Owner sections.** §5.3 guard; §6.3 Case 3 calibration numbers; §8 Limitations.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2022wavlm | T1 | WavLM same-session band observed on Case 3 speaker |
| S | reddy2022dnsmos | T2 | DNSMOS scale anchoring |
| S | saeki2022utmos | T2 | UTMOS scale anchoring |
| S | desplanques2020ecapa | T2 | cross-encoder future work reference |

**Body-pool target: 3–4 citations.** §8 Limitations must flag that the same-session band
is the authors' own empirical observation (not externally cited).

---

## Q09 — TCT vs. CLIP / SigLIP / WavLM SV / VALL-E 2

**Owner sections.** §2 Related Work sub-lines.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | zhai2023siglip | T2 | CLIP/SigLIP contrastive alignment |
| P | chen2022wavlm | T1 | speaker verification cross-encoder |
| P | chen2024valle2 | T2 | VALL-E 2 single-prompt zero-shot TTS; "human parity" claim |
| P | wang2023valle | T2 | VALL-E precursor (in-context learning for TTS) |
| S | tan2024naturalspeech3 | T1 | NS3 factorized-codec comparator |
| S | wang2024maskgct | T1 | MaskGCT masked-codec comparator |
| S | chen2024f5tts | T2 | F5-TTS flow-matching comparator |
| S | jiang2023megatts2 | T2 | single-prompt convention |
| S | shen2023naturalspeech2 | T2 | latent-diffusion TTS precedent |
| S | le2023voicebox | T2 | flow-matching TTS precedent |
| S | casanova2024xtts | T2 | multilingual zero-shot TTS precedent |

**Body-pool target: 6–8 citations.** SigLIP + WavLM + VALL-E 2 are the minimum comparators.

---

## Q10 — Context Graph instantiation (13 embedders, 59 CFs, 75 MCP tools)

**Owner sections.** §4.1 Systems / Context Graph.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2022wavlm | T1 | E-slot audio embedder |
| P | zhai2023siglip | T2 | E-slot visual embedder |
| P | nussbaum2024nomic | T2 | E-slot text embedder |
| P | santhanam2022colbertv2 | T1 | E12 late-interaction |
| P | formal2021splade | T2 | E13 sparse SPLADE |
| P | wang2021kepler | T2 | E11 entity embedder |
| P | bordes2013transe | T1 | KEPLER/TransE lineage |
| P | malkov2018hnsw | T1 | HNSW index substrate |
| P | cormack2009rrf | T2 | RRF fusion substrate |
| P | khattab2020colbert | T2 | ColBERT predecessor |
| S | mcp2024spec | T3 | MCP protocol for 75 tools |

**Body-pool target: 8–10 citations.** §4.1 is citation-dense on purpose because the
embedder set IS the DDA substrate (Q04).

---

## Q11 — ClipCannon instantiation (23-stage DAG, 7 modalities, 58 MCP tools)

**Owner sections.** §4.2 Systems / ClipCannon; §6.2 Case 2 linkage.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | meng2025echomimicv3 | T2 | downstream generator |
| P | hu2022lora | T1 | LoRA fine-tune on clips |
| P | baevski2020wav2vec | T1 | audio-speaker embedder |
| P | bain2023whisperx | T2 | ASR stage |
| P | li2017flame | T1 | FLAME/expression modality |
| P | ekman1978facs | T3 | FACS emotion modality |
| P | rouard2023htdemucs | T2 | HTDemucs music separation stage |
| P | campello2013hdbscan | T2 | HDBSCAN clustering stage |
| P | zhang1996birch | T1 | BIRCH clustering stage |
| S | google2020mediapipe | T3 | face-mesh tooling |
| S | tinachen2021yolov5face | T3 | face-detection tooling |
| S | resembleai2024enhance | T3 | speech-enhancement tooling |
| S | cui2024hallo3 | T2 | portrait-animation comparator |
| S | zheng2025hunyuanvideo | T2 | multi-character comparator |
| S | tian2024emo | T2 | EMO predecessor |
| S | tian2025emo2 | T2 | EMO2 successor |
| S | yang2024cogvideox | T2 | text-to-video diffusion comparator |
| S | mcp2024spec | T3 | MCP for 58 tools |

**Body-pool target: 10–14 citations.** §4.2 is the most citation-dense section; the
seven-modality framing requires all modality-anchor cites.

---

## Q12 — Case 3 measurements (T1 anchor)

**Owner sections.** §6.3 Case 3 (header tagged "T1").

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2022wavlm | T1 | SECS scorer (microsoft/wavlm-base-plus-sv) |
| P | reddy2022dnsmos | T2 | DNSMOS P.808 parity anchor |
| P | saeki2022utmos | T2 | UTMOS parity anchor |
| S | chen2024valle2 | T2 | parity comparator (VALL-E 2) |
| S | christop2025cloneval | T2 | benchmark framing (future comparability) |

**Body-pool target: 4–5 citations.**

---

## Q13 — Case 1 (Shakespeare) and Case 2 (Santa) projections

**Owner sections.** §6.1, §6.2.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | meng2025echomimicv3 | T2 | Case 2 base model |
| P | lipman2023flowmatching | T1 | flow-matching sign-convention context |
| P | hu2022lora | T1 | LoRA for Case 1 and Case 2 |
| S | li2024latentsync | T2 | lip-sync metric (projected) |
| S | cui2024hallo3 | T2 | portrait-animation baseline |
| S | ouyang2022instructgpt | T1 | Case 1 SFT context |
| S | rafailov2023dpo | T1 | Case 1 DPO context |

**Body-pool target: 5–7 citations.** Projected numbers must be flagged V22.

---

## Q14 — Case 3 vs. VALL-E 2 comparability scoping

**Owner sections.** §6.3 (comparator paragraph); §8 Limitations.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2024valle2 | T2 | VALL-E 2 parity claim |
| P | chen2022wavlm | T1 | the cross-encoder used |
| P | christop2025cloneval | T2 | open benchmark framing |
| S | wang2023valle | T2 | VALL-E precursor |
| S | tan2024naturalspeech3 | T1 | NS3 parity reference |
| S | wang2024maskgct | T1 | MaskGCT parity reference |
| S | chen2024f5tts | T2 | F5-TTS reference |

**Body-pool target: 4–6 citations.**

---

## Q15 — Sample-complexity collapse claim (three case studies)

**Owner sections.** §3.6 or §5.4 (claim); §6.1–§6.3 (per-case evidence).

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | villalobos2024data | T1 | baseline (thousands-of-examples convention) |
| P | hu2022lora | T1 | LoRA low-data fine-tune mechanism |
| P | chen2024valle2 | T2 | VALL-E 2 single-prompt baseline |
| P | meng2025echomimicv3 | T2 | Case 2 subject-clip count |
| S | tan2024naturalspeech3 | T1 | comparator |
| S | wang2024maskgct | T1 | comparator |

**Body-pool target: 4–5 citations.**

---

## Q16 — Threats to validity

**Owner sections.** §8 Limitations.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | shumailov2024nature | T1 | collapse-confusion threat |
| P | chen2022wavlm | T1 | cross-encoder scoring-bias |
| P | villalobos2024data | T1 | generalisation-scope limits |
| S | gerstgrasser2024colm | T2 | mitigation alternative |
| S | huang2024compression | T2 | compression-intelligence as proxy |
| S | christop2025cloneval | T2 | benchmark for cross-cohort evaluation |

**Body-pool target: 3–5 citations.**

---

## Q17 — Forensic failures (Shakespeare echo-bug; Santa flow-matching sign; cv2 FPS; YOLO mouth-landmark)

**Owner sections.** §6 case-study forensics; §8.4.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | lipman2023flowmatching | T1 | sign-convention reference |
| P | meng2025echomimicv3 | T2 | EchoMimicV3 flow-matching sign context |
| P | wan2025video | T2 | Wan flow-matching sign-convention lineage |
| P | tinachen2021yolov5face | T3 | YOLOv5-Face mouth-landmark index |
| S | rafailov2023dpo | T1 | Shakespeare DPO context (echo-bug context) |
| S | hu2022lora | T1 | Shakespeare LoRA context |

**Body-pool target: 4–5 citations.** Each bug paragraph cites the specific forensic file
in-text (per P10).

---

## Q18 — Reproducibility requirements

**Owner sections.** §8.5 Reproducibility + Appendix.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | chen2022wavlm | T1 | WavLM encoder version |
| P | reddy2022dnsmos | T2 | DNSMOS version |
| P | saeki2022utmos | T2 | UTMOS checkpoint |
| P | meng2025echomimicv3 | T2 | EchoMimicV3 checkpoint |
| P | hu2022lora | T1 | LoRA hyperparameters framing |
| P | mcp2024spec | T3 | MCP protocol version |
| S | nussbaum2024nomic | T2 | substitutable text embedder |
| S | casanova2024xtts | T2 | substitutable public TTS approximation |

**Body-pool target: 5–7 citations.**

---

## Q19 — Ethics (identity cloning, misuse, consent)

**Owner sections.** §8.6 Ethics.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | henderson2022pileoflaw | T1 | consent-aware data curation exemplar |
| P | longpre2024consent | T1 | AI data-commons consent decline |
| S | singhal2023nature | T1 | domain-model responsible-use exemplar (optional) |
| S | tu2025nature | T1 | clinical LLM responsibility exemplar (optional) |
| S | anthropic2024mcp | T3 | MCP safety framing (sparingly) |

**Body-pool target: 3–4 citations.** Cluster-H quota constraint enforced here: use only
H05 (T1), H13 (T1), H15 (T1); avoid low-tier H refs in body.

---

## Q20 — Future work

**Owner sections.** §9 Conclusion or §8.7 Future Work.

| Role | Key | Tier | Role in answer |
|---|---|---|---|
| P | desplanques2020ecapa | T2 | cross-encoder generalisation (ECAPA-TDNN alternative) |
| P | christop2025cloneval | T2 | open-benchmark future-work anchor |
| P | chen2022wavlm | T1 | embedder-independence future work |
| S | longpre2024consent | T1 | data-stewardship future work |
| S | li2025lossless | T1 | compression-direction future work |

**Body-pool target: 3–5 citations.**

---

## Thinnest-support questions (citations needing watch)

Ranked by risk that their minimum primary set is narrow:

1. **Q08 (thresholds τ_m)** — only 1 primary (chen2022wavlm). If agent-11 finds another threshold-calibration paper (e.g., in speaker-verification literature), add it. **Action.** Flag as yellow in agent-45 citation-validator.
2. **Q17 (forensics)** — heavily backed by *our own* forensic files; only 4 external citations. By design (the bugs are unique to our pipelines). **Action.** Accept narrow support; the claim is about our own bug history.
3. **Q08** and **Q17** are the two yellow-flag questions for agent-11 systematic-reviewer to monitor when expanding the pool. Every other question has ≥ 3 primaries.

## Coverage check (QG1 G7 per question)

- Every Q01–Q20 has ≥ 3 body-pool citations planned.
- Every Q has ≥ 1 T1 citation except Q17 (which has T1 lipman2023flowmatching as primary + hu2022lora / rafailov2023dpo among supporting).
- All primaries resolve to entries in `master-reference-pool.md`.
- Total unique body-pool keys used across Q01–Q20 = **52** (comfortably inside the agent-05 "30–60 references" target).

## Changelog

- 2026-04-16 — Initial issue by agent-08-citation-extractor.
