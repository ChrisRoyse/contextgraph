---
namespace: discoveries
agent_id: agent-06-literature-mapper
timestamp: 2026-04-16T16:30:00Z
status: active
supersedes: none
tags: literature-map, topology, six-clusters, related-work, citations, structural-holes
depends_on:
  - memory/context/mission-brief.md
  - memory/handoff/agent-05-to-06--plan-ready.md
  - memory/patterns/agent-05-research-planner--rewoo-plan.md
  - memory/patterns/agent-05-research-planner--agent-tasking-table.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
evidence_sources:
  - docs/researchpaper/_evidence_01_data_wall.md
  - docs/researchpaper/_evidence_02_voice_cloning.md
  - docs/researchpaper/_evidence_03_video_embedders.md
  - docs/researchpaper/_evidence_04_enterprise_market.md
---

# Field Topology — Six-Cluster Literature Map

This file maps the research neighbourhood our arXiv submission lives in. Every citation
below is sourced from one of the four `_evidence_0*.md` briefs; no new citations are
invented here. Agent-07 (source-tier-classifier) will tier them; agent-08 (citation-
extractor) will canonicalise them; agent-11 (systematic-reviewer) will PRISMA-synthesise
them.

The six clusters correspond to the six sub-lines of §2 Related Work. Each cluster lists
(a) its central citations, (b) its standard framing in the literature, (c) which
essential questions Q01–Q20 it serves (owner-side), (d) which manuscript sections consume
it, (e) 1–3 **structural holes** — things missing from the cluster that our paper
explicitly fills.

Cluster density is coded qualitatively: **dense** (6+ central T1/T2 candidates; no
expansion needed), **moderate** (4–5 candidates; agent-11 may expand with one sweep),
**thin** (< 3 candidates; agent-11 must expand to pass QG1).

---

## Cluster A — Scaling Laws + The Data Wall

**Density.** Dense (8 central candidates; all Tier-1/2).
**Serves.** Q01 (what is the data wall; why now), Q03 (what mitigations prior work offers).
**Consumed by.** §1 Introduction (three-paths framing); §2.1 Related Work; §3.3 DDA-vs-
synthetic-data.
**Evidence brief.** `_evidence_01_data_wall.md` §§ 1, 4, 5.

### Central citations (8)

1. **Kaplan et al. (2020)** — Scaling Laws for Neural Language Models. arXiv:2001.08361.
   — Establishes the loss = power-law(N, D, C) form. Foundational; cited wherever
   "scaling law" first appears.
2. **Hoffmann et al. (2022)** — Training Compute-Optimal LLMs (Chinchilla). arXiv:2203.15556;
   NeurIPS 2022. — ~20 tokens/parameter compute-optimal prescription; shows prior models
   were undertrained, i.e., bottlenecked on data.
3. **Villalobos et al. (2024)** — Will We Run Out of Data? arXiv:2211.04325; ICML 2024.
   — 300T median stock of public human text; 2026–2032 exhaustion band; overtraining
   accelerates the wall by up to 5×.
4. **Longpre et al. (2024)** — Consent in Crisis. arXiv:2407.14933; NeurIPS 2024 D&B Track.
   — ~25% of high-quality C4 tokens became robots.txt-restricted in 12 months (2023→2024);
   evidence that the *accessible* corpus tightens faster than the total corpus.
5. **Woo & Efrati (2024)** — OpenAI shifts strategy as GPT improvements slow. The
   Information, Nov 9 2024. — Industry signal, T3 / T4.
6. **Franzen (2024)** — AI scaling laws showing diminishing returns. TechCrunch, Nov 20
   2024. — Industry signal, T3 / T4 corroborator.
7. **Reuters (Hu & Tong, 2024)** — Seeking new path to smarter AI as current methods hit
   limitations. Nov 15 2024. — Industry signal, T3 / T4.
8. **Sutskever NeurIPS 2024 Test-of-Time talk** — "Pre-training as we know it will
   unquestionably end … data is the fossil fuel of AI." — Oral citation; we attribute but
   do not over-claim.

### Standard framing (for §2.1 verbatim reuse via agent-36)

Prior work treats the data wall as an exhaustion problem: the stock of high-quality
human-generated text is finite, its accessible fraction is shrinking, and Chinchilla-
scaling forecasts imply exhaustion in the 2026–2032 band (Villalobos et al., 2024;
Longpre et al., 2024). Industry signals in late 2024 (Woo & Efrati, 2024; Reuters, 2024)
report diminishing returns from pre-training on more of the same corpus. The field's
two standard responses are (i) acquire more licensed corpora (path 1, Cluster E) and
(ii) generate synthetic data (path 2, Cluster B).

### Structural holes (Cluster A → ours fills)

- **A-HOLE-1.** *The framing assumes the bottleneck is raw input count, not signal
  extraction from existing inputs.* No cited work formalises the signals-per-input
  ratio as the operative quantity. Our §3 introduces this ratio as the core object.
- **A-HOLE-2.** *No cited work names a third path that neither acquires nor synthesises.*
  Every mitigation catalogued is path 1 (license more) or path 2 (synthesise; see
  Cluster B) or Chinchilla-rebalancing (still path 1, just more efficient). The DDA
  third path (decompose existing inputs through N frozen embedders) is structurally
  missing from this cluster's framing.
- **A-HOLE-3.** *Industry signals about "diminishing returns" are anecdotal and
  un-formalised.* No cited source proposes what the next-generation labeled-signal
  supply could look like — only that the current supply is flattening.

---

## Cluster B — Model Collapse + Synthetic Data

**Density.** Dense (6 central candidates; all Tier-1/2).
**Serves.** Q02 (why DDA structurally avoids model collapse).
**Consumed by.** §2.2 Related Work; §3.3 DDA-vs-synthetic-data; §8 Limitations (R4
DDA-as-synthetic-data confusion mitigation).
**Evidence brief.** `_evidence_01_data_wall.md` §§ 2, 3.

### Central citations (6)

1. **Shumailov et al. (2024)** — AI models collapse when trained on recursively generated
   data. *Nature* 631(8022):755–759. DOI:10.1038/s41586-024-07566-y. — The Nature paper.
   Defines early-collapse (tails vanish) and late-collapse (variance → point mass)
   across GMM, VAE, and LLM architectures. Load-bearing for our Q02 distinction.
2. **Gerstgrasser et al. (2024)** — Is Model Collapse Inevitable? arXiv:2404.01413;
   COLM 2024. — Accumulation (vs. replacement) of real with synthetic data breaks the
   curse: test error remains bounded.
3. **Dohmatob, Feng, Yang, Charton, Kempe (2024)** — A Tale of Tails: Model Collapse as
   a Change of Scaling Laws. arXiv:2402.07043; ICML 2024. — Collapse as a scaling-law
   crossover; synthetic data has a point of diminishing return.
4. **Alemohammad et al. (2023)** — Self-Consuming Generative Models Go MAD.
   arXiv:2307.01850; ICLR 2024. — Model Autophagy Disorder for image generators; fresh
   real-data injections stabilise. Imports "self-consuming" as the precise failure
   condition DDA does not satisfy.
5. **Feng et al. (2024)** — Beyond Model Collapse: Scaling Up with Synthesized Data
   Requires Verification. arXiv:2406.07515. — Verification / filtering (oracle verifier
   or learned reward) recovers scaling; formalises why RLHF-style rejection escapes
   collapse.
6. **Seddik, Chen, Hayou, Youssef, Debbah (2024)** — How Bad is Training on Synthetic
   Data? arXiv:2404.05090; COLM 2024. — Statistical analysis of LM collapse under
   synthetic-only and mixed regimes.

### Standard framing

Recursive training on generator-produced samples causes irreversible distribution drift:
early collapse erases the tails, late collapse flattens variance (Shumailov et al.,
2024; Alemohammad et al., 2023). Mitigations fall into two families: **accumulate**
(keep real data across generations; Gerstgrasser et al., 2024) and **verify** (filter
synthetic samples via an oracle or reward model; Feng et al., 2024). Both operate
*within* the generator-feedback loop; both concede that the loop itself is the hazard.

### Structural holes (Cluster B → ours fills)

- **B-HOLE-1.** *The literature's unit of analysis is "was this sample generated by a
  prior model".* It does not distinguish between (a) generating new inputs from a
  generator (which triggers collapse) and (b) computing new labels from frozen,
  independently-trained embedders on unchanged real inputs (which has no generative
  feedback loop). DDA sits in category (b); the collapse theorem is silent on it. Our
  §3.3 makes this structural silence explicit.
- **B-HOLE-2.** *No cited mitigation treats the frozen-embedder projection of a fixed
  corpus as a distinct category.* Accumulation and verification both assume a generator
  in the loop; neither framework applies to our setting.
- **B-HOLE-3.** *No cited work estimates the pairwise-information content of multiple
  independent embedders applied to the same input.* The $\binom{N}{2}$ pairwise signal
  that is load-bearing for our DDA count has no precedent in this cluster.

---

## Cluster C — Compression-as-Intelligence

**Density.** Dense (6 central candidates; all Tier-1/2).
**Serves.** Q05 (meaning compression vs. bit / weight / activation compression).
**Consumed by.** §2.3 Related Work; §3.1 four-category compression taxonomy; §3.5.
**Evidence brief.** `_evidence_01_data_wall.md` § 6.

### Central citations (6)

1. **Delétang et al. (2024)** — Language Modeling Is Compression. arXiv:2309.10668;
   ICLR 2024. — Formalises LM = arithmetic-coded compressor; Chinchilla 70B compresses
   ImageNet to 43% raw / LibriSpeech to 16.4%; beats PNG / FLAC. The anchor for
   "prediction ≡ compression".
2. **Huang et al. (2024)** — Compression Represents Intelligence Linearly.
   arXiv:2404.09937; COLM 2024. — Linear correlation r ≈ 0.95 between compression ratio
   and downstream benchmarks across 30+ LLMs. Empirical anchor.
3. **Li et al. (2025)** — Lossless data compression by large language models. *Nature
   Machine Intelligence*. DOI:10.1038/s42256-025-01033-7. — ~2× better than gzip/bzip2
   across text, image, audio, video with LLM arithmetic coding.
4. **Valmeekam et al. (2023)** — LLMZip: Lossless Text Compression using Large Language
   Models. arXiv:2306.04050. — LLaMA-7B at ~0.71 bits/char on text8; beats prior SOTA
   ~0.94 bpc.
5. **Jiang et al. (2023)** — "Low-Resource" Text Classification: A Parameter-Free
   Classification Method with Compressors. ACL 2023 Findings. — gzip + k-NN beats BERT
   on several low-resource benchmarks. Shows the boundary between compression and
   learning is thinner than expected; relevant precedent for *using an external
   compressor as a labeler*.
6. **(Adjacent)** *Weight-and-activation-compression referents cited for the taxonomy
   table but not central*: GPTQ (Frantar et al. 2022), SparseGPT (Frantar & Alistarh
   2023), TurboQuant, KV-cache quantisation. These are named in `_evidence_01_data_wall.md`
   as adjacent categories but are not deeply cited; agent-11 may expand if §3.5 needs
   the four-row taxonomy table to be citation-dense.

### Standard framing

Compression and intelligence are empirically and theoretically linked: strong predictors
*are* strong compressors (Delétang et al., 2024; Huang et al., 2024; Li et al., 2025).
The field recognises three compression categories relevant to LLMs: **bit compression**
(gzip/PNG/FLAC — bits/bits), **weight compression** (GPTQ, SparseGPT, distillation —
accuracy / parameters), **activation compression** (TurboQuant, KV-cache quantisation
— memory / output quality).

### Structural holes (Cluster C → ours fills)

- **C-HOLE-1.** *No cited compression category has units "labeled signals per unit of
  raw data".* The three existing categories all measure output economy relative to
  inputs of the same modality; none measures *supervisory-signal yield* per fixed input.
  Our §3.1 proposes **meaning compression** as a distinct fourth category whose unit
  column is dimensionally different.
- **C-HOLE-2.** *Compression-as-intelligence is stated as a property of the LLM, not
  as an engineering principle for training-data preparation.* The cited work reads
  compression from a trained model; it does not prescribe compression *of the training
  corpus* via multiple frozen embedders to multiply labeled signal. Our DDA construct
  turns the observation into a data-engineering operation.
- **C-HOLE-3.** *No cited work addresses multi-compressor (multi-embedder) signal
  orthogonality.* Compression is discussed per-compressor; cross-compressor interaction
  is not formalised. Our $N + \binom{N}{2}$ count makes the interaction first-class.

---

## Cluster D — Multi-Modal Embeddings + Retrieval (the DDA substrate)

**Density.** Dense (10 central candidates; all Tier-1/2).
**Serves.** Q04 (DDA formal; substrate is *these* embedders), Q10 (Context Graph E1–E13),
Q11 (ClipCannon seven-modality DAG), Q09 (how TCT differs from CLIP/SigLIP).
**Consumed by.** §2.5 Related Work (embedder cluster); §4.1 Context Graph; §4.2
ClipCannon; §5 TCT (the embedder set is the method's substrate).
**Evidence brief.** `_evidence_03_video_embedders.md` (references list).

### Central citations (10)

1. **Zhai, Mustafa, Kolesnikov & Beyer (2023)** — Sigmoid Loss for Language Image
   Pre-Training (SigLIP). arXiv:2303.15343. — The vision-language embedder family for
   visual modality. Successor to CLIP.
2. **Nussbaum, Morris, Duderstadt & Mulyar (2024)** — Nomic Embed: Training a
   Reproducible Long Context Text Embedder. arXiv:2402.01613. — Open text-embedder
   referent for E1 / E10 text-embedding slots.
3. **Chen et al. (2022)** — WavLM: Large-Scale Self-Supervised Pre-Training for Full
   Stack Speech Processing. IEEE JSTSP 16(6), 1505–1518. arXiv:2110.13900. — Canonical
   speaker-encoder backbone. microsoft/wavlm-base-plus-sv is our SECS scorer for Case 3.
4. **Khattab & Zaharia (2020)** — ColBERT: Efficient and Effective Passage Search via
   Contextualized Late Interaction over BERT. SIGIR 2020. arXiv:2004.12832. — Late-
   interaction retrieval substrate; E12 of Context Graph.
5. **Santhanam, Khattab, Saad-Falcon, Potts & Zaharia (2022)** — ColBERTv2: Effective
   and Efficient Retrieval via Lightweight Late Interaction. NAACL 2022.
   arXiv:2112.01488. — The version we cite for late-interaction semantics.
6. **Formal, Piwowarski & Clinchant (2021)** — SPLADE: Sparse Lexical and Expansion
   Model for First Stage Ranking. SIGIR 2021. arXiv:2107.05720. — Sparse lexical
   expansion; E13 of Context Graph (30K-dim sparse SPLADE).
7. **Wang et al. (2021)** — KEPLER: A Unified Model for Knowledge Embedding and
   Pre-Trained Language Representation. TACL 9, 176–194. arXiv:1911.06136. — E11 entity
   embedder referent.
8. **Bordes, Usunier, Garcia-Durán, Weston & Yakhnenko (2013)** — Translating Embeddings
   for Modeling Multi-Relational Data (TransE). NeurIPS 2013. — The relational-embedding
   family (KEPLER / TransE lineage) for E11.
9. **Malkov & Yashunin (2018)** — Efficient and Robust Approximate Nearest Neighbor
   Search using Hierarchical Navigable Small World Graphs (HNSW). IEEE TPAMI 42(4).
   arXiv:1603.09320. — Index substrate that makes 13-embedder retrieval tractable
   (Context Graph uses HNSW across 15 indexes per `CLAUDE.md`).
10. **Cormack, Clarke & Büttcher (2009)** — Reciprocal Rank Fusion (RRF) Outperforms
    Condorcet and Individual Rank Learning Methods. SIGIR 2009. DOI:10.1145/1571941.1572114.
    — Fusion substrate for multi-embedder retrieval in Context Graph.

### Standard framing

Multi-modal embedders and retrieval over them are treated as a **search substrate**:
each embedder indexes inputs in its own space (CLIP / SigLIP for vision-language;
WavLM for speech; ColBERT / SPLADE for text retrieval; KEPLER / TransE for relational),
and fusion over multiple embedders (RRF, late-interaction) produces more robust
ranking. Embedder selection is usually *task-driven* ("pick the best embedder for
the task") and the embedders' outputs are the *endpoint* of the pipeline.

### Structural holes (Cluster D → ours fills)

- **D-HOLE-1.** *Embedders are positioned as retrieval tools, not as a compositional
  decomposition of meaning for training-data preparation.* The cited work treats
  multiple embedders as complementary retrievers whose outputs are ranked / fused.
  Our §3 flips the role: the embedders are *labellers*, and the composition of N of
  them yields $N + \binom{N}{2}$ training signals per input. This role-flip is
  structurally absent from the cluster.
- **D-HOLE-2.** *No cited work formalises the signal yield of N jointly-applied
  independent embedders.* Fusion literature (RRF, late-interaction) measures ranking
  quality, not supervisory signal count. The $\binom{N}{2}$ pairwise-interaction count
  is novel to us.
- **D-HOLE-3.** *Embedders are used as similarity scorers but not as runtime
  acceptance gates for generation.* No cited retrieval paper proposes a
  constellation-guard analogue (re-embed generated output through the same frozen
  embedders, compute per-modality cosines, accept iff all $\geq \tau_m$). This is
  our TCT §5.3 contribution and is structurally outside the retrieval cluster.

---

## Cluster E — Zero-Shot TTS + Voice Cloning (Case 3 comparator cluster)

**Density.** Dense (12 central candidates; all Tier-1/2).
**Serves.** Q09 (TCT vs. VALL-E 2 / zero-shot TTS), Q12 (Case 3 SECS 0.961 mean /
0.975 max), Q14 (Case 3 vs. VALL-E 2 comparator; comparability scoping).
**Consumed by.** §2.6 Related Work (voice-cloning sub-line); §6.3 Case 3 (T1 anchor);
§8 Limitations (cross-encoder caveat, V4).
**Evidence brief.** `_evidence_02_voice_cloning.md` (all sections).

### Central citations (12)

1. **Wang, Chen, Wu et al. (2023)** — VALL-E: Neural Codec Language Models Are Zero-Shot
   Text-to-Speech Synthesizers. arXiv:2301.02111. — Founding paper of neural-codec
   zero-shot TTS as in-context learning over EnCodec tokens.
2. **Chen, Liu, Zhou et al. (2024)** — VALL-E 2: Neural Codec Language Models Are Human
   Parity Zero-Shot TTS Synthesizers. arXiv:2406.05370. — Headline SIM-o ≈ 0.643;
   WavLM-large SV cosine ≈ 0.881 (secondary); first to declare "human parity" on
   LibriSpeech continuation via SIM / WER / DNSMOS parity. **The Case 3 comparator.**
3. **Tan, Ju, Wang et al. (2024)** — NaturalSpeech 3: Zero-Shot Speech Synthesis with
   Factorized Codec and Diffusion. ICML 2024. arXiv:2403.03100. — SIM ≈ 0.67; WavLM-
   large SV cosine ≈ 0.891 (secondary).
4. **Wang, Zhan, Liu et al. (2024)** — MaskGCT: Zero-Shot TTS with Masked Generative
   Codec Transformer. ICLR 2025. arXiv:2409.00750. — SIM ≈ 0.687; WavLM cosine ≈ 0.877.
5. **Chen, Niu, Ma et al. (2024)** — F5-TTS: A Fairytaler That Fakes Fluent and Faithful
   Speech with Flow Matching. arXiv:2410.06885. — SIM-o ≈ 0.66; WavLM cosine ≈ 0.862;
   non-autoregressive flow-matching TTS.
6. **Le, Vyas, Shi et al. (2023)** — Voicebox: Text-Guided Multilingual Universal
   Speech Generation at Scale. arXiv:2306.15687. — Precedent for flow-matching TTS.
7. **Shen, Ju, Tan et al. (2023)** — NaturalSpeech 2: Latent Diffusion Models Are
   Natural and Zero-Shot Speech and Singing Synthesizers. arXiv:2304.09116. — Latent-
   diffusion TTS precedent.
8. **Jiang, Ren, Li et al. (2023)** — Mega-TTS 2: Zero-Shot TTS with Arbitrary-Length
   Speech Prompts. arXiv:2307.07218. — Arbitrary-length prompt precedent; note the
   *single-prompt* convention (contrast with our 50-clip centroid).
9. **Casanova, Davis, Gölge et al. (2024)** — XTTS: A Massively Multilingual Zero-Shot
   Text-to-Speech Model. arXiv:2406.04904. — Multilingual zero-shot TTS precedent.
10. **Reddy, Gopal & Cutler (2022)** — DNSMOS P.835: A Non-Intrusive Perceptual
    Objective Speech Quality Metric. ICASSP 2022. arXiv:2110.01763. — Our DNSMOS P.808
    parity metric (3.93 vs. 3.93) for Case 3.
11. **Saeki et al. (2022)** — UTMOS: UTokyo-SaruLab System for VoiceMOS Challenge 2022.
    Interspeech 2022. arXiv:2204.02152. — Our UTMOS parity metric (3.012 vs. 2.997) for
    Case 3. VoiceMOS Challenge winner.
12. **Desplanques, Thienpondt & Demuynck (2020)** — ECAPA-TDNN: Emphasized Channel
    Attention, Propagation and Aggregation in TDNN based Speaker Verification.
    Interspeech 2020. arXiv:2005.07143. — Alternative speaker-encoder family; relevant
    for cross-encoder SECS generalisation future work (Q20).
13. **(Supplementary)** **Christop, Kuczyński & Kubis (2025)** — ClonEval: An Open
    Voice Cloning Benchmark. arXiv:2504.20581. — Benchmark referent; we do not claim
    ClonEval comparability yet.

### Standard framing

Zero-shot TTS frames voice cloning as in-context learning: one (or few) speaker prompt
token-encoded through a neural codec (VALL-E, VALL-E 2) or factorised codec (NS3) or
masked-generative codec (MaskGCT) or flow-matching model (F5-TTS), scored by WavLM-large
SV cosine or SIM-o. VALL-E 2 (Chen et al., 2024) declared "human parity" on LibriSpeech
continuation via SIM / WER / DNSMOS parity — a parity claim, not a subjective Turing
test. Headline WavLM cosines cluster around 0.86–0.89 across the frontier (VALL-E 2,
NS3, MaskGCT, F5-TTS).

### Structural holes (Cluster E → ours fills)

- **E-HOLE-1.** *All cited TTS systems use a single speaker prompt; none aggregates a
  multi-clip centroid.* VALL-E 2, NS3, MaskGCT, F5-TTS, XTTS, Mega-TTS 2 all use a
  single reference utterance or arbitrary-length single prompt. Our Case 3 uses a
  **50-clip reference set → L2-normalised centroid** (glossary #9 / #7) as the
  alignment target. The centroid-aggregation step is structurally outside the cited
  zero-shot-TTS framing.
- **E-HOLE-2.** *No cited TTS system uses a runtime cosine-gate against that centroid
  as a per-output acceptance predicate.* VALL-E 2 reports SIM-o *after* generation; it
  does not resample against a threshold. Our WavLM-scored best-of-12 at T=0.3 is a
  concrete instantiation of the constellation guard (glossary #12) and is structurally
  novel in the TTS literature.
- **E-HOLE-3.** *The WavLM same-session band (0.95–0.99) is not published; our use of
  it as the threshold calibration is our own empirical observation.* The literature
  reports EER distributions on VoxCeleb / NIST-SRE but no canonical "same-session 0.95"
  value (`_evidence_02` § 8). We state this as a hedge, not a citation.

---

## Cluster F — Identity-Locked Video Generation (Case 2 comparator cluster)

**Density.** Dense (12 central candidates; all Tier-1/2).
**Serves.** Q11 (ClipCannon seven-modality DAG for Case 2), Q13 (Case 2 projected
metrics), Q17 (Santa flow-matching sign-bug forensic — glossary #30).
**Consumed by.** §2.5 Related Work (embedder + video sub-line); §4.2 ClipCannon;
§6.2 Case 2 (T2+T3); §8.4 forensic paragraph.
**Evidence brief.** `_evidence_03_video_embedders.md` (references list).

### Central citations (12)

1. **Meng, Wang, Wu et al. (2025)** — EchoMimicV3: 1.3B Parameters Are All You Need for
   Unified Multi-Modal and Multi-Task Human Animation. arXiv:2507.03905. — **Case 2
   base model.** Flow-matching video DIT with sign convention target = noise − latent
   (glossary #30).
2. **Cui, Li, Zhan et al. (2024)** — Hallo3: Highly Dynamic and Realistic Portrait
   Image Animation with Video Diffusion Transformer. arXiv:2412.00733. — Portrait-
   animation precedent.
3. **Li, Zhang, Xu et al. (2024)** — LatentSync: Audio-Conditioned Latent Diffusion
   Models for Lip Sync. arXiv:2412.09262. — Lip-sync metric anchor for Case 2 projected
   evaluation.
4. **Zheng, Chen, Cun et al. (2025)** — HunyuanVideo-Avatar: High-Fidelity Audio-Driven
   Human Animation for Multiple Characters. arXiv:2505.20156. — Multi-character avatar
   precedent.
5. **Tian, Wang, Zhang & Bo (2024)** — EMO: Emote Portrait Alive — Generating
   Expressive Portrait Videos with Audio2Video Diffusion Model. arXiv:2402.17485.
   — Audio-to-video diffusion for emotive portrait generation.
6. **Tian, Hu, Wang, Zhang & Bo (2025)** — EMO2: End-Effector Guided Audio-Driven
   Avatar Video Generation. arXiv:2501.10687.
7. **Yang, Teng, Zheng et al. (2024)** — CogVideoX: Text-to-Video Diffusion Models with
   an Expert Transformer. arXiv:2408.06072. — T2V diffusion referent.
8. **Wan Team, Alibaba (2025)** — Wan: Open and Advanced Large-Scale Video Generative
   Models. arXiv:2503.20314. — Related flow-matching video lineage (same sign
   convention family as EchoMimicV3 per glossary #30).
9. **Hu, Shen, Wallis et al. (2022)** — LoRA: Low-Rank Adaptation of Large Language
   Models. ICLR 2022. arXiv:2106.09685. — Parameter-efficient fine-tuning substrate
   for Case 1 Shakespeare LoRA and Case 2 EchoMimicV3 LoRA.
10. **Lipman, Chen, Ben-Hamu, Nickel & Le (2023)** — Flow Matching for Generative
    Modeling. ICLR 2023. arXiv:2210.02747. — The flow-matching foundational paper; we
    cite it because Case 2 uses a mouth-weighted flow-matching loss on top of
    EchoMimicV3.
11. **Li, Bolkart, Black, Li & Romero (2017)** — Learning a Model of Facial Shape and
    Expression from 4D Scans (FLAME). ACM TOG 36(6). DOI:10.1145/3130800.3130813.
    — Facial-shape model; one of ClipCannon's seven modalities (FLAME/expression).
12. **Ekman & Friesen (1978)** — Facial Action Coding System (FACS). Consulting
    Psychologists Press. — Facial-expression modality precedent (emotion /
    expression-space). Print reference; no URL.

### Standard framing

Identity-locked video generation is positioned as a task of (a) choosing a generator
(diffusion, flow-matching, DIT; EchoMimicV3, Hallo3, HunyuanVideo-Avatar, LatentSync,
EMO/EMO2, CogVideoX, Wan), (b) conditioning on a single reference image plus audio
(EMO-style) or multiple short clips (EchoMimicV3), (c) optionally LoRA-fine-tuning on
subject clips (Hu et al., 2022), and (d) evaluating lip-sync and visual quality with
LatentSync-style metrics. The identity target is the reference image(s), consumed by
the generator's conditioning pathway; there is no runtime re-embedding check against a
frozen centroid.

### Structural holes (Cluster F → ours fills)

- **F-HOLE-1.** *None of the cited video-generation systems uses a multi-modal frozen-
  centroid set as an alignment target.* They condition on reference images and audio;
  they do not build per-modality centroids (visual, audio-speaker, audio-voice, emotion,
  FLAME/expression) from a 16-min interview and hold them frozen during training.
- **F-HOLE-2.** *None applies a runtime constellation guard (per-modality cosine gate)
  at inference.* They evaluate lip-sync and visual quality *after* generation on test
  sets; they do not accept / reject per-output against a threshold.
- **F-HOLE-3.** *Flow-matching sign-convention forensics are absent from cited work.*
  EchoMimicV3 / Wan2.1-Fun use target = noise − latent; flipping the sign causes
  anti-denoising (glossary #30). The Santa forensic file
  (`docs/inspect/08_training_audit.md` in the Santa project) documents this as a
  concrete bug encountered during Case 2; no cited paper names this hazard. P10 /
  FM-08 requires disclosure.

---

## Cross-cluster summary table

| Cluster | Density | Central citations | Q-IDs served | §-IDs consumed | # Structural holes |
|---|---|---|---|---|---|
| A Scaling laws + data wall | dense | 8 | Q01, Q03 | §1; §2.1; §3.3 | 3 |
| B Model collapse + synthetic | dense | 6 | Q02 | §2.2; §3.3; §8 | 3 |
| C Compression-as-intelligence | dense | 6 | Q05 | §2.3; §3.1; §3.5 | 3 |
| D Multi-modal embeddings + retrieval | dense | 10 | Q04, Q09, Q10, Q11 | §2.5; §4; §5 | 3 |
| E Zero-shot TTS + voice cloning | dense | 12 (13 with supp.) | Q09, Q12, Q14 | §2.6; §6.3; §8 | 3 |
| F Identity-locked video generation | dense | 12 | Q11, Q13, Q17 | §2.5; §4.2; §6.2; §8.4 | 3 |
| **TOTAL** | — | **54 central + 1 supplementary** | — | — | **18** |

**Thin-cluster count**: 0. Every cluster passes the ≥ 3 Tier-1/2 candidate criterion for
QG1. Agent-11 does not need to expand; the PRISMA review can proceed over this corpus
directly.

**FM-10 (citation padding) check.** Every citation above is attached to at least one
specific claim slot in the agent-03 question set, not to a generic "survey of the field"
block. Citations are enumerated by role, not by volume.

---

## Combined gap map (preview — full file in `--structural-holes.md`)

The 18 per-cluster holes collapse into **5 combined structural holes** that the arXiv
submission explicitly positions itself to fill. See the companion file
`agent-06-literature-mapper--structural-holes.md`.

---

## Change log

- 2026-04-16 — Initial issue by agent-06-literature-mapper.
