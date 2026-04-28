---
namespace: discoveries
agent_id: agent-09-methodology-scanner
timestamp: 2026-04-16T18:15:00Z
status: active
supersedes: none
tags: methodology, scan, cluster-analysis, empirical-benchmark, theoretical-derivation, forensic, ablation, method-theory-alignment
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/discoveries/agent-06-literature-mapper--structural-holes.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md
  - memory/handoff/agent-08-to-09--citation-pool-ready.md
downstream_consumers:
  - agent-10-theoretical-framework-analyst
  - agent-11-systematic-reviewer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
---

# Methodology Scan — Cluster-by-Cluster Analysis

## Scope statement

This scan covers **88 citations** across nine clusters A–I in the LOCKED master
reference pool (`decisions/agent-08-citation-extractor--master-reference-pool.md`).
Every citation is read **for its methodology** — not its result — and classified by
methodological tradition, evaluation metric, and method-theory alignment.

**Do NOT add citations.** Any missing comparator method triggers a blocker; this scan
does not silently extend the pool.

Clusters (from master pool):
- **A** Scaling laws + data wall (9 refs)
- **B** Model collapse + synthetic data (7 refs)
- **C** Compression-as-intelligence (5 refs)
- **D** Multi-modal embedders + retrieval (10 refs)
- **E** Zero-shot TTS + voice cloning (13 refs)
- **F** Identity-locked video generation (12 refs)
- **G** Auxiliary pipeline components (9 refs)
- **H** Enterprise / market signals (20 refs, ≤ 6 body quota)
- **I** Alignment — RLHF / DPO / CAI (3 refs)

Methodological tradition codes used below:
- **EB** — Empirical benchmark (quantitative metric on a held-out set)
- **TD** — Theoretical derivation (theorem, bound, closed-form analysis)
- **FC** — Forensic case study (documented bug + diagnostic method + fix + verification)
- **AB** — Ablation study (controlled component removal)
- **MA** — Meta-analysis / correlation across many models
- **SR** — Systematic review / position paper
- **SI** — Statistical / industry signal (trade press, oral remarks)
- **SA** — System-artefact release (code / checkpoint / dataset, method documented
  in a model card)
- **CT** — Controlled trial (randomised-sample held-out evaluation with named protocol)

---

## Cluster A — Scaling laws + data wall

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **TD** (scaling-law derivation / extrapolation) | 3 | kaplan2020scaling, hoffmann2022chinchilla, villalobos2024data |
| **SR** (position / consent measurement) | 1 | longpre2024consent (structural measurement of the accessible corpus) |
| **SI** (trade press / oral) | 5 | woo2024information, metz2024bloomberg, hu2024reuters, franzen2024techcrunch, sutskever2024neurips |

### Standard evaluation metrics

- **Perplexity / loss** (validation cross-entropy on held-out text) — the Kaplan /
  Chinchilla currency.
- **Tokens-per-parameter ratio** — the Chinchilla compute-optimal prescription
  (~20 tokens/parameter).
- **Corpus-stock estimate in tokens** — Villalobos et al. use public-text estimates
  (300T median band, 2026–2032 exhaustion).
- **Accessible-fraction delta** — Longpre et al. measure robots.txt-restricted
  percentage change over 12 months (~25% of high-quality C4).
- **Downstream benchmarks** (MMLU, HellaSwag, etc.) — used as secondary corroborators
  of "more data → more capability."

### Method-theory alignment in this cluster

The cluster's theory is **scaling-law compute/data/parameter extrapolation** and its
method is **fitting power laws to observational training runs + extrapolating to
future regimes**. Alignment is tight: Kaplan fits a power law; Chinchilla re-fits
under a compute constraint; Villalobos extrapolates the same family forward against
the corpus-stock estimate. The SI refs (woo, metz, hu, franzen, sutskever) do NOT
follow a formal methodology — they are *industry signals* cited for framing, not
evidence. Agent-05 planning explicitly accepts this asymmetry: T3 refs are
framing-only, T1 refs carry the empirical weight.

### Methodological gaps

**A-METHOD-GAP-1.** No paper in Cluster A treats **signals extractable per existing
input** as the operative quantity. Every paper counts raw tokens, tokens-per-parameter,
or loss-per-token. **None formalises signals-per-input.** This is the methodological
kernel of HOLE-1 in structural-holes.md.

**A-METHOD-GAP-2.** No paper in Cluster A runs an **ablation over data-preparation
pipelines**; all ablations are over model architecture / scale. The "what if we
projected the fixed corpus through N frozen embedders and counted labelled signals"
experiment has no precedent here.

### How OUR methodology fits

- **§3.1 meaning compression** introduces the new unit (signals per unit of raw data)
  that fills A-METHOD-GAP-1. This is a **TD** contribution in Cluster A's style
  (derive a ratio, state its form, compute its instantiation at N=13 and N=7).
- Our cited Cluster-A refs (villalobos2024data, hoffmann2022chinchilla) are used as
  **framing anchors, not as methodology comparators** — TD lineage only, we do not
  claim to re-derive scaling laws.

---

## Cluster B — Model collapse + synthetic data

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **TD + EB** (closed-form + simulation across GMM/VAE/LLM) | 5 | shumailov2024nature, dohmatob2024tails, dohmatob2024regression, alemohammad2023mad, seddik2024collapse |
| **EB + AB** (mitigation experiments: accumulation / verification) | 2 | gerstgrasser2024colm, feng2024verification |

### Standard evaluation metrics

- **KL divergence** between the original data distribution and generation-N's
  distribution.
- **Variance / tail-mass** of the generator distribution across iterations (early /
  late collapse signatures).
- **Validation perplexity / cross-entropy** on held-out real data across successive
  synthetic generations.
- **Classifier accuracy** on tasks whose label distribution should have been
  preserved.
- **FID / IS** for image generators (alemohammad2023mad).

### Method-theory alignment

Alignment is **tight and explicit**: every paper declares a recursive-training
feedback loop (generator → synthetic samples → train next generator on those
samples), establishes a theoretical decay argument, and verifies it with an
iterative simulation + held-out perplexity / variance measurement. Mitigations
(gerstgrasser accumulate; feng verify) follow the same method with one lever added.

### Methodological gaps

**B-METHOD-GAP-1.** Every paper in Cluster B assumes a **generator-in-loop**. No
method in this cluster has been run on a setting where the "new signal" is derived
from **frozen independently-trained embedders applied to unchanged real inputs**.
The methodology — iterated training on samples drawn from a generator — does not
apply to DDA.

**B-METHOD-GAP-2.** No paper in Cluster B measures **pairwise signal content across
independent labellers**; the unit is generator fidelity, not multi-labeller
orthogonality.

### How OUR methodology fits

- **§3.3** uses Cluster B's own methodology (define the feedback condition; check
  whether DDA satisfies it) to argue that the collapse theorem is **silent** on
  DDA — not that DDA is immune by a new theorem. This is a **categorical move**, not
  an empirical refutation. Our methodology in §3.3 is a **SR-style clarification**,
  not an EB counter-experiment.
- Cluster B remains the right comparator for §2.2 Related Work and §8 Limitations
  (collapse-confusion is a threat to validity per Q16; flagged R4 in mission brief).

---

## Cluster C — Compression-as-intelligence

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB** (compression ratio on text / audio / image) | 3 | deletang2024compression, valmeekam2023llmzip, li2025lossless |
| **MA** (correlation across 30+ LLMs) | 1 | huang2024compression |
| **EB + AB** (non-learning classifier using compressor as signal) | 1 | jiang2023gzip |

### Standard evaluation metrics

- **Compression ratio** in bits/bits (PNG, FLAC baselines) or bits/char (LLMZip
  vs. text8 SOTA).
- **Correlation coefficient** (r ≈ 0.95 in huang2024compression, cross-model).
- **Classification accuracy** on a benchmark, with gzip + k-NN vs. BERT
  (jiang2023gzip low-resource setting).
- **Input-domain coverage breadth** — li2025lossless demonstrates across text /
  image / audio / video modalities.

### Method-theory alignment

Alignment is **tight on the compressor-as-predictor thesis**: every paper instruments
the model/compressor on a held-out distribution and reports a ratio. The methodology
is **arithmetic-coded compression of held-out sequences using a predictive model's
next-token distribution** (Delétang) or **correlation studies across open models**
(Huang) or **substitution into a classification pipeline** (Jiang). The object of
analysis is always the trained predictor; the training data and the embedder
substrate are treated as given.

### Methodological gaps

**C-METHOD-GAP-1.** **No paper in Cluster C measures cross-embedder interaction
signal.** Compression is reported per-compressor on a sequence. The $\binom{N}{2}$
pairwise-interaction term in DDA has no analogue in this cluster's methodology.

**C-METHOD-GAP-2.** **No paper in Cluster C uses a compressor at training-data
preparation time** (as a *labeller* for upstream training data). Compression is
read *from* a trained model, not used to prepare data for a new model.

**C-METHOD-GAP-3.** No paper runs an **ablation over embedder count** (how many
independent embedders before signal saturation). This ablation is the natural
empirical companion to our $N + \binom{N}{2}$ formula; it is methodologically absent
from the cited work.

### How OUR methodology fits

- **§3.1 four-category taxonomy** extends the cited work's compression framing with
  a new category (meaning compression). Methodologically this is **TD-style
  category construction**, not an EB experiment. We do NOT claim to re-benchmark
  bit compression.
- **§3.4 counting argument** ($N + \binom{N}{2}$) is a **TD** derivation. An EB
  instantiation is open work (future ablation across N) and explicitly called out
  in §10 / Q20.

---

## Cluster D — Multi-modal embedders + retrieval (the DDA substrate)

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB + AB** (embedder training + ablation on pretext task) | 5 | zhai2023siglip, nussbaum2024nomic, chen2022wavlm, formal2021splade, wang2021kepler |
| **EB** (retrieval benchmark; pair classifier / late interaction) | 4 | khattab2020colbert, santhanam2022colbertv2, cormack2009rrf, bordes2013transe |
| **TD** (ANN algorithm + complexity analysis) | 1 | malkov2018hnsw |

### Standard evaluation metrics

- **Top-k retrieval accuracy** (MS MARCO, BEIR, BIG-bench-style IR).
- **NDCG / MAP / MRR** for ranking quality.
- **EER (Equal Error Rate)** for speaker verification (WavLM SV on VoxCeleb).
- **Recall@k** for nearest-neighbour search (HNSW).
- **ImageNet zero-shot accuracy** (SigLIP) and cross-modal retrieval.
- **BLEU / F1** for generation tasks downstream of embeddings (rare; mostly
  upstream).
- **Fusion quality (RRF)** reported as delta vs. individual rankers.
- **Sparse-lexical recall** (SPLADE FLOPS / efficiency + recall).

### Method-theory alignment

Alignment is **tight on the retrieval-quality thesis**: embedders are trained on
contrastive / pretext objectives, then evaluated on held-out retrieval benchmarks.
The underlying theory is **representation quality measured by task transfer**
(SigLIP's sigmoid contrastive loss → ImageNet; WavLM's masked speech prediction →
SUPERB; ColBERT's late interaction → passage retrieval). Each paper pairs method
with a named benchmark and reports the delta.

### Methodological gaps

**D-METHOD-GAP-1.** **No paper in Cluster D uses the embedder as a per-output
acceptance gate at inference.** Embedders score similarity, they do not reject
outputs. D-HOLE-3 in structural-holes.md has no methodological analogue in this
cluster. (This is the gap TCT's constellation guard fills.)

**D-METHOD-GAP-2.** **No paper quantifies labelled-signal yield from N embedders.**
Fusion methods (RRF) measure ranking lift; they do not count new supervisory
channels. Our $\binom{N}{2}$ pairwise-interaction measurement has no established
protocol in Cluster D.

**D-METHOD-GAP-3.** **No paper runs multi-embedder ablation specifically to measure
signal-space redundancy** (e.g., "what does each embedder add when the previous
N-1 are frozen"). The standard ablation style (per-component lift) is architectural,
not signal-theoretic.

### How OUR methodology fits

- **§4.1 Context Graph (Q10)** is Cluster-D-citation-dense. It instantiates 13
  frozen embedders + HNSW + RRF. Our methodology **inherits the cited work's
  retrieval methodology** (fusion, HNSW search) but **flips the role** — embedders
  become labellers, not rankers.
- **§5.3 constellation guard** is a **methodological novelty across Cluster D**:
  the cited work has no per-output cosine-gate at inference. This is our primary
  methodological contribution in the embedder family.
- **Ablation opportunity (open work).** An ablation that removes embedders one at
  a time from the constellation would follow the AB tradition of Cluster D exactly
  while measuring the new signal-theoretic quantity. This is called out in §10 /
  Q20 future work.

---

## Cluster E — Zero-shot TTS + voice cloning (Case 3 comparator)

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB + AB** (TTS architecture + comparator benchmark) | 8 | wang2023valle, chen2024valle2, tan2024naturalspeech3, wang2024maskgct, chen2024f5tts, le2023voicebox, shen2023naturalspeech2, jiang2023megatts2 |
| **EB + SA** (reproducible public model) | 1 | casanova2024xtts |
| **CT** (named protocol, non-intrusive scorer) | 2 | reddy2022dnsmos, saeki2022utmos |
| **EB + AB** (speaker encoder training + VoxCeleb eval) | 1 | desplanques2020ecapa |
| **SA + EB** (open benchmark dataset release) | 1 | christop2025cloneval |

### Standard evaluation metrics

- **SIM-o** (objective speaker similarity via a fixed speaker-encoder; VALL-E 2 headline
  0.643) — the dominant Cluster-E headline.
- **WavLM-large SV cosine** (secondary; ~0.86–0.89 across the frontier).
- **WER** (word-error-rate on transcribed output) — intelligibility proxy.
- **DNSMOS P.808 / P.835** (non-intrusive naturalness predictor; reddy2022dnsmos is
  the method paper itself).
- **UTMOS** (learned naturalness MOS; saeki2022utmos is the method paper).
- **CMOS** (comparative listener MOS; subjective, small N).
- **EER** on speaker-verification benchmarks (desplanques2020ecapa).
- **Ablations**: prompt length, temperature, sampling strategy.

### Method-theory alignment

Alignment is **tight on the "condition on single prompt + evaluate similarity +
evaluate naturalness" paradigm**. Every cited system: (a) generates TTS output from
a single reference utterance, (b) scores SIM-o via a fixed encoder, (c) reports
WER + DNSMOS + sometimes CMOS. VALL-E 2 (chen2024valle2) frames this as "human
parity" via a parity-table across SIM-o / WER / DNSMOS — a parity claim, not a
Turing test (Q14 comparability is narrow).

### Methodological gaps

**E-METHOD-GAP-1.** **No paper in Cluster E aggregates a multi-clip reference set
into a centroid.** Every cited system uses a single reference utterance or an
arbitrary-length single prompt. Centroid aggregation (L2-normalised mean of
L2-normalised embeddings over N clips) is methodologically absent.

**E-METHOD-GAP-2.** **No paper applies a runtime cosine-gate resampling loop.**
SIM-o is reported *after* generation on a held-out set; no cited TTS system
resamples at T=0.3 best-of-12 against a per-utterance WavLM-SECS threshold. This
is the **constellation guard** (glossary #12) in Case 3 and is methodologically
novel in Cluster E.

**E-METHOD-GAP-3.** **No paper uses a cross-encoder SECS protocol** (the scoring
encoder is different from any encoder the TTS model was conditioned on). Almost
all SIM-o scorers are from the same speaker-encoder family as the conditioning
signal, which can bias similarity upward. Our methodology in Case 3 **enforces
cross-encoder SECS** (generation conditioned on Qwen3-TTS; scoring by WavLM-base-plus-sv;
the two are architecturally independent).

**E-METHOD-GAP-4.** **No paper reports a WavLM-SECS same-session band** (within
one speaker's own recordings, on the encoder). The 0.95–0.99 band in Case 3 is
**authors' own empirical observation** (glossary #17, V12 hedge mandated).

### How OUR methodology fits

- **§6.3 Case 3 (T1)** instantiates Cluster E's standard EB methodology **plus
  three methodological novelties**: (a) 50-clip centroid aggregation (fills
  E-METHOD-GAP-1); (b) WavLM-scored best-of-12 at T=0.3 runtime guard (fills
  E-METHOD-GAP-2); (c) cross-encoder SECS scoring (fills E-METHOD-GAP-3). Reports
  WavLM SECS mean 0.961 / max 0.975, DNSMOS P.808 parity (3.93 vs. 3.93), UTMOS
  parity (3.012 vs. 2.997) — all following Cluster E's own protocol.
- The **comparability scope (Q14)** with VALL-E 2 is narrow: we do NOT claim
  parity on LibriSpeech continuation (which is VALL-E 2's protocol). We report
  our own metric set on our own held-out 10-sentence set, under our own encoder.
  §8 Limitations must flag this explicitly.
- **Ablations required in §6.3** (following Cluster E's AB tradition): (i) N=1
  vs. N=50 reference set; (ii) T=0 greedy vs. T=0.3 best-of-12; (iii) no-guard
  vs. guarded output distributions.

---

## Cluster F — Identity-locked video generation (Case 2 comparator)

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB + AB** (diffusion / flow-matching video; audio-visual conditioning) | 7 | meng2025echomimicv3, cui2024hallo3, li2024latentsync, zheng2025hunyuanvideo, tian2024emo, tian2025emo2, yang2024cogvideox |
| **SA + EB** (open-weights flow-matching video) | 1 | wan2025video |
| **TD + EB** (LoRA low-rank derivation + downstream benchmarks) | 1 | hu2022lora |
| **TD + EB** (flow-matching theory + unconditional benchmarks) | 1 | lipman2023flowmatching |
| **TD + EB** (FLAME 4D statistical facial model) | 1 | li2017flame |
| **SR** (coding taxonomy, not empirical) | 1 | ekman1978facs |

### Standard evaluation metrics

- **LSE-D / LSE-C** (lip-sync error; distance and confidence from SyncNet).
- **FID / FVD** (Fréchet Video Distance) for video generation quality.
- **Visual quality MOS** (subjective listener/viewer rating).
- **CSIM** (cosine similarity to a reference face-identity embedder).
- **WER on lip-read output** (for lip-sync specifically).
- **Ablations**: with/without audio conditioning; with/without emotion embedding;
  with/without motion prior; sign convention.
- **Compute / parameter cost** (EchoMimicV3: 1.3B; CogVideoX larger; a common
  axis of comparison).

### Method-theory alignment

Alignment is **tight on the flow-matching / diffusion-video paradigm**: every cited
system couples a generator (DIT; flow matching; latent diffusion) to an audio +
image conditioning pipeline, trains on 100K+ video clips, then evaluates lip-sync
and FVD on LRS / LRW / HDTF held-outs. LoRA (hu2022lora) provides the
parameter-efficient fine-tune substrate; flow matching (lipman2023flowmatching)
provides the training-objective substrate. The theory-method link is
**architecture → probability-path objective → metric** and is methodologically
coherent across the cluster.

### Methodological gaps

**F-METHOD-GAP-1.** **No paper builds a multi-modal frozen-centroid set** from a
single-subject corpus and uses it as the alignment target. Identity conditioning is
always via reference images (+ audio) fed through the generator's conditioning
pathway. Centroid aggregation per modality is methodologically absent.

**F-METHOD-GAP-2.** **No paper applies a runtime per-modality cosine-gate across
N ≥ 2 frozen embedders** to accept / reject each output. CSIM is reported after
the fact on the whole test set. (This is the constellation guard projected for
Case 2.)

**F-METHOD-GAP-3.** **No paper documents the flow-matching sign-convention
forensic** (target = noise − latent vs. target = latent − noise). Flipping the
sign causes anti-denoising. Our glossary #30 + §6.2 forensic paragraph disclose
this by reference to `docs/inspect/08_training_audit.md` in the Santa project.
**Methodologically novel across Cluster F.**

**F-METHOD-GAP-4.** **No paper in the cluster runs a single-subject evaluation
with multiple per-modality embedders simultaneously** (visual CSIM + audio-speaker
SECS + audio-voice pitch consistency + emotion coherence + FLAME expression
plausibility). The standard is per-modality reporting, not per-output multi-modal
gating.

### How OUR methodology fits

- **§4.2 ClipCannon (Q11)** is Cluster-F-citation-dense. Our seven-modality DAG
  decomposes the 16-min interview through 7 independent embedders (hence 7 + 21 = 28
  DDA signals per input). The DAG methodology is **EB + SA**, aligned with Cluster
  F's own pipeline-paper conventions (file counts, module descriptions, stage
  counts).
- **§6.2 Case 2 (T2 + T3)** applies TCT across the seven modalities: centroids
  from the 2,362 clips → mouth-weighted flow-matching LoRA on EchoMimicV3 →
  projected runtime constellation guard (T3 pending measurement). Methodologically
  this **inherits Cluster F's flow-matching + LoRA stack** but **adds centroid
  aggregation (F-METHOD-GAP-1) and runtime gating (F-METHOD-GAP-2)**.
- **Forensics disclosure (P10 / FM-08)** fills F-METHOD-GAP-3 by reference.
- **Required ablations in §6.2** (following Cluster F's AB tradition): (i)
  centroid-present vs. centroid-absent; (ii) mouth-weighted vs. uniform
  flow-matching loss; (iii) per-modality guard threshold sweep.

---

## Cluster G — Auxiliary pipeline components

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB + AB** (speech / ASR representations) | 2 | baevski2020wav2vec, bain2023whisperx |
| **EB + AB** (music source separation) | 1 | rouard2023htdemucs |
| **TD + EB** (clustering algorithm + benchmarks) | 2 | campello2013hdbscan, zhang1996birch |
| **SA** (software release, no metric paper) | 3 | google2020mediapipe, tinachen2021yolov5face, resembleai2024enhance |
| **SR** (protocol spec) | 1 | mcp2024spec |

### Standard evaluation metrics

- **WER** (WhisperX transcription).
- **SI-SDR / SDR** (HTDemucs source separation).
- **SUPERB / MOS** (wav2vec 2.0 downstream).
- **Cluster purity / silhouette / ARI** (HDBSCAN, BIRCH).
- **Face-detection mAP** (YOLO face).
- **No metric** for MCP spec (protocol paper).

### Method-theory alignment

Alignment is cluster-appropriate but **heterogeneous**: each auxiliary component is
evaluated against its own domain's convention. No unifying methodology — this is
the "glue" cluster.

### Methodological gaps

**G-METHOD-GAP-1.** **No single paper in Cluster G evaluates end-to-end DAG
behaviour.** The cluster provides individual stages; composition quality on a
single-subject corpus is not cited work's unit of evaluation.

### How OUR methodology fits

- **§4.2 ClipCannon** explicitly composes Cluster-G components into a 23-stage DAG
  + 7 modalities. The composition methodology is **EB + SA** (stage counts, module
  descriptions, output-data volumes). It is **not a contribution of Cluster G**;
  it is a *claim about pipeline engineering* whose citations are in Cluster G.
- **Nothing in §5 Methods is inherited from Cluster G directly**; Cluster G is a
  tool list.

---

## Cluster H — Enterprise / market signals

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **SI** (trade press / corporate release / press release) | 14 | anthropic2024mcp, field2024techcrunch, gartner2023trends, openai2024reddit, openai2024newscorp, reuters2024reddit, cerullo2024cbs, shutterstock2023openai, variety2024newscorp, verma2023washpost, harvard2024caselaw, sec2024edgar, sec2024edgarsearch, nci2024thesaurus, nlm2024mesh, snomed2024ct (some SR) |
| **EB + SA** (LLM for specialised domain, clinical / legal / financial) | 3 | henderson2022pileoflaw, singhal2023nature, tu2025nature, wu2023bloomberggpt |

### Standard evaluation metrics

- **Licensing-deal dollar value** (SI only).
- **Clinical QA accuracy** (MedQA, MedMCQA; singhal / tu).
- **Legal dataset-size and filtering coverage** (henderson pile-of-law).
- **Financial-NER / FPB accuracy** (wu BloombergGPT).

### Method-theory alignment

Heterogeneous; the cluster is anchored on industry signals, not methodological
contribution. The T1 sub-cluster (singhal / tu / henderson) does carry proper EB
methodology.

### Methodological gaps

**H-METHOD-GAP-1.** **No paper in Cluster H applies DDA to a licensed corpus.** The
deal-economics papers treat the corpus as raw input; no paper decomposes the
licensed corpus through N frozen embedders. (This is orthogonal to the market
story.)

### How OUR methodology fits

- Cluster H is **quota-limited to ≤ 6 body citations** per agent-07's rule. Body
  slots: **H05 henderson2022pileoflaw** (consent-aware data curation),
  **H13 singhal2023nature** (responsible domain LLM), **H15 tu2025nature**
  (expert medical LLM), **H20 wu2023bloomberggpt** (domain LLM economics).
- **No methodology inherited from H into §3–§7.** H citations anchor §8.6 Ethics
  and §2.1 path-1 framing only.

---

## Cluster I — Alignment (RLHF / DPO / CAI)

### Dominant methodological traditions

| Tradition | Count | Representative refs |
|---|---|---|
| **EB + AB** (RLHF pipeline + preference-model training + PPO) | 1 | ouyang2022instructgpt |
| **TD + EB** (closed-form DPO derivation + comparative benchmarks) | 1 | rafailov2023dpo |
| **EB + SR** (Constitutional AI; AI-feedback preference simulation) | 1 | bai2022constitutional |

### Standard evaluation metrics

- **Preference-model win-rate** (crowdworker A-vs-B preference rates).
- **Reward-model score** on held-out prompts (scalar).
- **Harmlessness / helpfulness MOS** (human raters; small N; HH-RLHF bench).
- **MMLU / TruthfulQA / BBH** (indirect capability proxies).
- **KL-to-reference-policy** (DPO / PPO regularisation target).

### Method-theory alignment

Alignment is **tight on the scalar-reward thesis**: pretrain → SFT → reward-model
training on human preferences → PPO (Ouyang) or DPO closed-form (Rafailov) or
AI-feedback self-critique (Bai). The common output is a **scalar preference
direction**, not a geometric target.

### Methodological gaps

**I-METHOD-GAP-1.** **No paper in Cluster I uses a geometric alignment target** — a
frozen centroid in an embedding space the generator must place its outputs near.
Alignment is *learned reward*, not *frozen distance*.

**I-METHOD-GAP-2.** **No paper in Cluster I has a per-output runtime acceptance
predicate.** The reward model scores continuously at training time; at inference
time, outputs are sampled from the aligned policy without per-output gating.

**I-METHOD-GAP-3.** **No paper in Cluster I uses multiple independent alignment
modalities simultaneously.** One scalar reward per output; no multi-modality
conjunction.

### How OUR methodology fits

- **§5 TCT** is defined in §2.6 / §5 explicitly as the **non-scalar-reward**
  alternative. Cluster I is the primary comparator for Q06. Our methodological
  contrast table (§5.4 or §2.6) lists I01 / I02 / I03 as the scalar-reward
  triad against which TCT's geometric-alignment-to-constellation is positioned.
- **Methodological novelty across Cluster I**: per-modality cosine gate at
  inference (I-METHOD-GAP-2), multi-modality conjunction (I-METHOD-GAP-3),
  frozen-centroid target (I-METHOD-GAP-1). These three together constitute TCT's
  delta over RLHF / DPO / CAI.

---

## Cross-cluster methodological summary

| Cluster | Primary tradition(s) | Dominant metrics | Method-theory alignment | # Method gaps | Our case study that engages |
|---|---|---|---|---|---|
| A | TD + SI | tokens, perplexity, accessible-fraction | tight (TD) / loose (SI) | 2 | §3.1 meaning compression |
| B | TD + EB | KL, variance, perplexity | tight | 2 | §3.3 (categorical, not EB) |
| C | EB + MA | compression ratio, correlation | tight | 3 | §3.1, §3.4 |
| D | EB + AB, TD | top-k, NDCG, EER | tight | 3 | §4.1, §5 (all cases) |
| E | EB + AB, CT | SIM-o, WER, DNSMOS, UTMOS | tight on single-prompt | 4 | §6.3 Case 3 (T1) |
| F | EB + AB, TD | LSE, FVD, CSIM | tight on DIT / flow-matching | 4 | §6.2 Case 2 (T2+T3) |
| G | heterogeneous | per-component | n/a (glue) | 1 | §4.2 pipeline composition |
| H | SI + EB | domain accuracy | heterogeneous | 1 | §8.6 Ethics only |
| I | EB + AB, TD | win-rate, KL, reward | tight scalar-reward | 3 | §2.6 / §5 comparator |

**Total methodological gaps identified across clusters: 23.** See companion file
`agent-09-methodology-scanner--methodological-gaps.md` for the consolidated gap list
(the nine novelty-deltas our paper fills methodologically) and the cross-reference
to structural-holes (HOLE-1 … HOLE-5).

---

## Method-theory alignment — where our paper's cases sit

| Case | Theory carried | Methods in our paper | Cluster that sets the standard | Our deltas |
|---|---|---|---|---|
| **Case 1 Shakespeare** (T3 projected) | Compression-as-intelligence (C); LoRA + DPO (F, I) | SFT + DPO on cleaned corpus (G standard); echo-bug FSV forensic (F-METHOD-GAP-3 analogue for text) | I + F (LoRA) | Constellation guard at text-modality level projected; echo-bug forensic disclosed |
| **Case 2 Santa** (T2+T3) | Flow matching (F10); LoRA (F09); ClipCannon 7-modality DAG (G + F) | Mouth-weighted flow-matching LoRA on EchoMimicV3; projected runtime constellation guard across 5 modalities | F | Centroid aggregation (F-METHOD-GAP-1); runtime multi-modal gate (F-METHOD-GAP-2); flow-matching sign forensic (F-METHOD-GAP-3) |
| **Case 3 Voice Cloning** (T1 measured) | Zero-shot TTS (E); WavLM SV (D03); DNSMOS / UTMOS (E10, E11) | Qwen3-TTS + Full ICL + 50-clip centroid + WavLM best-of-12 at T=0.3; cross-encoder SECS | E | Centroid from 50-clip reference (E-METHOD-GAP-1); runtime cosine-gate (E-METHOD-GAP-2); cross-encoder SECS (E-METHOD-GAP-3); WavLM same-session band (E-METHOD-GAP-4 — disclosed as authors' observation) |

Each case's methodology is **standard-within-cluster plus one or more methodological
novelties** traced to the gaps above. The TCT constellation guard is the **only
methodological element that is novel across all four relevant clusters (D, E, F, I)**.

---

## Confidence statement

**HIGH.** The scan covers all 88 citations, classifies each by method, aligns each
cluster's standard methodology with its own theory, and traces every methodological
gap to at least one structural hole in the agent-06 gap map. The four methodological
novelties that carry our paper (signals-per-input formalisation; centroid
aggregation; runtime constellation guard; forensic disclosures) each map to one
or more specific clusters' gaps.

**Locked elements (do not re-debate without blocker).**
- 88 citations are in-scope; no new citations added by this scan.
- Method tradition codes (EB / TD / FC / AB / MA / SR / SI / SA / CT) are the
  scanner's working set.
- Per-cluster dominant tradition + dominant metrics are the scanner's finding.

**Open for agent-11 systematic review.**
- Ablation specifics for §6.2 and §6.3 (what levels of N, T, guard thresholds).
- Whether agent-11's PRISMA expansion adds methods not yet in the scan (currently
  none; all clusters pass density).

## Changelog

- 2026-04-16 — Initial issue by agent-09-methodology-scanner.
