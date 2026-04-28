---
namespace: discoveries
agent_id: agent-13-quality-assessor
timestamp: 2026-04-16T23:30:00Z
status: active
locked: false
supersedes: none
tags: quality-assessment, per-citation, rubric, hot-tier, reproducibility, eval-rigor, soundness, sample-adequacy, bias-disclosure
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-07-source-tier-classifier--tier-assignments.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
  - memory/patterns/agent-12-context-tier-manager--hot-warm-cold.md
downstream_consumers:
  - agent-14-bias-detector
  - agent-29-validity-guardian
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Per-Citation Methodological Quality Scores

## Scope

Every HOT-tier citation (n=18) is scored on five dimensions (0/1/2 each, max 10).
In addition, 10 WARM-tier citations are spot-checked on the same rubric to anchor
the cross-tier distribution.

The full population (88 citations) is almost entirely ML / IR / signal-processing
empirical papers, theoretical analyses, benchmark papers, and engineering system
descriptions. **No human-subjects research exists in the pool.** The appraisal
framework therefore deviates from CASP / JBI / Cochrane RoB (which targets
clinical or social-science designs) and adopts an ML-paper-native rubric aligned
to the NeurIPS reproducibility checklist, ML Reproducibility Challenge criteria,
and the arXiv moderation policy.

## Rubric

Each citation scored 0 / 1 / 2 on five dimensions (max 10 per paper):

| Dimension | 0 | 1 | 2 |
|---|---|---|---|
| **Reproducibility** | No code, weights, or data artefacts released; methods underspecified | Partial release (either code OR weights OR data; or full release but with missing hyperparams / preprocessing) | Full release of code + weights + data + hyperparams + eval script; or foundational single-theorem paper with complete proof |
| **Evaluation rigor** | Single metric on single dataset; no ablations; no baselines or strawman baselines | Multiple metrics on 1–2 datasets; some ablations; standard baselines but no error bars / confidence intervals | Multiple metrics × multiple datasets; thorough ablations; strong baselines; error bars / confidence intervals / held-out sets |
| **Theoretical soundness** | Claims not supported by evidence; overreach; unfalsifiable | Most claims supported; some unhedged extrapolation or scope creep | Claims tightly bounded by evidence; hedges where appropriate; proofs where applicable are rigorous |
| **Sample adequacy** | Toy N; single environment; no held-out set; no diversity | Moderate N; single domain or 2–3 datasets; held-out set present | Large N (thousands to billions of tokens / samples); multiple domains or diverse eval sets; clean train-test separation |
| **Bias disclosure** | No conflict-of-interest disclosure; no benchmark-contamination discussion; promotional framing | Partial disclosure (affiliation clear, but no discussion of selection bias / contamination / benchmark gaming) | Clear conflict disclosure + explicit discussion of selection bias, benchmark contamination, and limitations; no promotional framing |

**Flag threshold.** Any hot-tier citation scoring **< 7** is flagged for caveat
in writer cue sheet. Any hot-tier citation scoring **< 5** is demotion-proposed
to warm and flagged as a blocker to coordinator.

**Floor rule.** Foundational theoretical-method papers (LoRA, Flow Matching,
HNSW) are held to the same rubric but with "Reproducibility = 2" awarded if the
method is fully specified with pseudocode / equations plus either reference
implementation OR widespread third-party implementation (the method has become
commodity). "Sample adequacy" for theoretical / methodological papers is
scored on the breadth of their empirical validation rather than absolute N.

---

## HOT-tier citations — full scoring (n=18)

### H01 villalobos2024data — Will we run out of data?

**Citation (APA).** Villalobos, P., Ho, A., Sevilla, J., Besiroglu, T., Heim, L.,
& Hobbhahn, M. (2024). Will we run out of data? Limits of LLM scaling based on
human-generated data. *ICML 2024*. arXiv:2211.04325.

**Source tier.** T1 (ICML 2024). **Cluster.** A. **GRADE.** MODERATE.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | All parameters of the estimation model are published; the Epoch AI team released an updated datasheet in 2024; estimation code path is documented. |
| Evaluation rigor | 1 | Estimation methodology is primarily a Monte-Carlo extrapolation with sensitivity analysis; comparing to prior stock estimates. No "ground-truth" possible for a forecast, so rigor here is bounded by epistemic design. |
| Theoretical soundness | 2 | Claim-to-evidence chain is tight; sensitivity analysis over prior assumptions; hedges are explicit ("assuming current scaling practices continue"). |
| Sample adequacy | 2 | Estimation sweeps across multiple corpora (Common Crawl, English Wikipedia, books, public-code). Five assumption regimes. |
| Bias disclosure | 2 | Epoch AI is a transparent non-profit forecasting org; paper explicitly states limitations of the forecast and does not advocate a policy position. |
| **Total** | **9 / 10** | **High quality.** The load-bearing uncertainty is inherent to forecasting, not methodological weakness. |

---

### H02 hoffmann2022chinchilla — Training compute-optimal LLMs

**Citation.** Hoffmann, J., Borgeaud, S., Mensch, A., et al. (2022). Training
compute-optimal large language models. *NeurIPS 2022*. arXiv:2203.15556.

**Source tier.** T1 (NeurIPS 2022). **Cluster.** A. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 1 | Neither Chinchilla weights nor training data are released. Hyperparameters and architecture fully documented. Independent replications (Epoch AI; DeepMind's own subsequent papers) confirm the ~20 tok/param ratio. |
| Evaluation rigor | 2 | Swept 400+ models across compute scales; fit three parametric scaling laws; MMLU + Big-Bench + zero-shot / few-shot eval; ablations on token budget. |
| Theoretical soundness | 2 | The compute-optimal argument is a constrained-optimization derivation; empirical fit is rigorous; hedges appropriate. |
| Sample adequacy | 2 | Trained 400+ models from 70M to 16B params, 5B to 500B tokens — among the largest controlled scaling studies ever run. |
| Bias disclosure | 1 | DeepMind conflict obvious but not explicitly declared. No discussion of benchmark contamination (a concern for later scaling papers). |
| **Total** | **8 / 10** | **High quality.** Deducted for closed weights and contamination non-disclosure. |

---

### H03 longpre2024consent — Consent in crisis

**Citation.** Longpre, S., Mahari, R., Chen, N., et al. (2024). Consent in
crisis: The rapid decline of the AI data commons. *NeurIPS 2024 D&B Track*.
arXiv:2407.14933.

**Source tier.** T1 (NeurIPS 2024 D&B). **Cluster.** A. **GRADE.** MODERATE / SINGLE-SOURCE for accessible-fraction claim.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Data Provenance Initiative releases all crawl-date logs, robots.txt dumps, and analysis notebooks publicly. |
| Evaluation rigor | 2 | Measures accessible-fraction across C4, RefinedWeb, Dolma; longitudinal over 2023–2024; robust to different tokenizers / samplers. |
| Theoretical soundness | 2 | Measurement-grounded claims; accessible-fraction decline is observed directly, not extrapolated. |
| Sample adequacy | 2 | Multiple web-scale corpora (100B+ tokens); 2 years of longitudinal data. |
| Bias disclosure | 2 | Authors declare institutional affiliations (MIT, various) and acknowledge that the study is cross-sectional by design. No industry funding for the core measurement. |
| **Total** | **10 / 10** | **Very high quality.** Single-source status reflects novelty, not deficiency. |

---

### H04 shumailov2024nature — AI models collapse when trained on recursively generated data

**Citation.** Shumailov, I., Shumaylov, Z., Zhao, Y., Gal, Y., Papernot, N.,
& Anderson, R. (2024). *Nature*, 631(8022), 755–759. DOI:10.1038/s41586-024-07566-y.

**Source tier.** T1 (*Nature*). **Cluster.** B. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Code available via Nature supplementary; reproductions by Dohmatob et al. (2024) replicate the theoretical and empirical finding on independent codebases. |
| Evaluation rigor | 2 | Theorems + simulations across GMM, VAE, and transformer LMs; ablations on loop depth, data-accumulation variants. |
| Theoretical soundness | 2 | Closed-form proof for Gaussian case; empirical validation for neural cases. Claims hedged to "recursive generator-in-loop" regimes. |
| Sample adequacy | 2 | Multi-architecture study (statistical models → neural); multiple training regimes. |
| Bias disclosure | 2 | Oxford / Cambridge academic authors; no industry affiliation; Nature's COI policy enforces disclosure; limitations section explicit. |
| **Total** | **10 / 10** | **Very high quality.** The flagship collapse paper. |

---

### H05 alemohammad2023mad — Self-consuming generative models go MAD

**Citation.** Alemohammad, S., Casco-Rodriguez, J., Luzi, L., et al. (2023).
Self-consuming generative models go MAD. *ICLR 2024*. arXiv:2307.01850.

**Source tier.** T1 (ICLR 2024). **Cluster.** B. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Open-source code repo; replicated by multiple third parties (e.g., Dohmatob 2024 extends the setup). |
| Evaluation rigor | 2 | Three loop types (fully synthetic, augmented, fresh-data); ablations on loop count; multiple generative families (GAN / VAE / diffusion). |
| Theoretical soundness | 2 | MAD mechanism well-defined; claims bounded to self-consuming regimes. |
| Sample adequacy | 2 | CelebA + FFHQ + CIFAR-10 + synthetic Gaussians; diverse architectures. |
| Bias disclosure | 2 | Rice academic authors; limitations section acknowledges that some generator-in-loop configurations may be pathological edge cases. |
| **Total** | **10 / 10** | **Very high quality.** |

---

### H06 deletang2024compression — Language modeling is compression

**Citation.** Delétang, G., Ruoss, A., Duquenne, P.-A., et al. (2024). Language
modeling is compression. *ICLR 2024*. arXiv:2309.10668.

**Source tier.** T1 (ICLR 2024). **Cluster.** C. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Code released; uses open-weight Chinchilla / LLaMA checkpoints; compression protocol implementable from paper. |
| Evaluation rigor | 2 | Image / audio / text all compressed; compares to PNG / FLAC / gzip; uses standard benchmark sets (Wikipedia, ImageNet, LibriSpeech). |
| Theoretical soundness | 2 | Shannon duality is the core mathematical claim; rigorous. Transformers-as-entropy-coder demonstrated arithmetically. |
| Sample adequacy | 2 | Three modalities × multiple benchmarks × multiple model scales. |
| Bias disclosure | 1 | DeepMind affiliation declared; no explicit discussion of whether the benchmark data is in training data (contamination concern for Chinchilla on Wikipedia). |
| **Total** | **9 / 10** | **High quality.** Deducted for contamination non-disclosure. |

---

### H07 huang2024compression — Compression represents intelligence linearly

**Citation.** Huang, Y., Vincent, J., Ye, Y., et al. (2024). Compression
represents intelligence linearly. *COLM 2024*. arXiv:2404.09937.

**Source tier.** T2 (COLM 2024). **Cluster.** C. **GRADE.** SINGLE-SOURCE.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Full protocol documented; evaluation scripts and compression pipeline available. |
| Evaluation rigor | 2 | 30+ LLMs; three capability benchmarks (knowledge, reasoning, coding); regression analysis with r ≈ 0.95 correlation. |
| Theoretical soundness | 1 | Correlation is observational; the *causal* interpretation is hedged but easy to misread. Paper explicitly warns against causal inference. |
| Sample adequacy | 1 | 30+ models is large but models span only 2023–2024 training era; no longitudinal coverage of older architectures. Benchmark contamination plausible but not rigorously tested. |
| Bias disclosure | 1 | Academic authors; no industry funding declared; does not fully address that newer models may be trained on the benchmark data. |
| **Total** | **7 / 10** | **Moderate-High quality.** Flagged — SINGLE-SOURCE status + correlation-not-causation + contamination-risk. Paper must be cited with attribution phrasing ("Huang et al. 2024 report r ≈ 0.95"). |

---

### H08 chen2022wavlm — WavLM: Large-scale self-supervised pretraining

**Citation.** Chen, S., Wang, C., Chen, Z., et al. (2022). WavLM: Large-scale
self-supervised pre-training for full stack speech processing. *IEEE JSTSP*,
16(6), 1505–1518. arXiv:2110.13900.

**Source tier.** T1 (IEEE JSTSP). **Cluster.** D. **GRADE.** STRONG + AUTHOR-EMPIRICAL substrate.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Open-source HuggingFace checkpoints (Base / Base+ / Large); preprocessing + training code released by Microsoft. |
| Evaluation rigor | 2 | SUPERB benchmark (13 speech tasks); SID / ASV / SE / ST — comprehensive full-stack eval. |
| Theoretical soundness | 2 | Masked-speech denoising loss derivation clear; multi-task SSL framing rigorous. |
| Sample adequacy | 2 | 94k hours of audio (LibriLight + GigaSpeech + VoxPopuli). Large by any standard. |
| Bias disclosure | 1 | Microsoft authors; conflict clear; SUPERB leaderboard is developed by same community, which is a mild conflict not discussed. Data itself is from public corpora. |
| **Total** | **9 / 10** | **High quality.** The most-loaded citation in the pool (9 Q-IDs). |

**Note.** When Case 3 measurements report WavLM SECS 0.961, Chen 2022 is cited
*as the scorer*, not the measurement. The measurement is AUTHOR-EMPIRICAL
(CL-16) and requires V12/V17/V22 hedge. This citation itself is clean.

---

### H09 zhai2023siglip — Sigmoid loss for language image pretraining

**Citation.** Zhai, X., Mustafa, B., Kolesnikov, A., & Beyer, L. (2023).
Sigmoid loss for language image pre-training (SigLIP). arXiv:2303.15343.

**Source tier.** T2 (arXiv, Google DeepMind). **Cluster.** D. **GRADE.** STRONG-sup.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Open weights on HuggingFace and Google research; training recipe documented; widely reproduced. |
| Evaluation rigor | 2 | Zero-shot ImageNet + ImageNet-{A, R, Sketch, V2}; COCO / Flickr retrieval; scaling ablation 86M → 4B params; batch-size ablation. |
| Theoretical soundness | 2 | Sigmoid vs. softmax contrastive loss derivation is rigorous; claims hedged to retrieval / classification domains. |
| Sample adequacy | 2 | Trained on WebLI (10B+ image-text pairs). Eval on standard benchmarks. |
| Bias disclosure | 1 | Google DeepMind conflict obvious; contamination between WebLI and downstream benchmarks not deeply analyzed. |
| **Total** | **9 / 10** | **High quality.** |

---

### H10 malkov2018hnsw — HNSW

**Citation.** Malkov, Y. A., & Yashunin, D. A. (2018). *IEEE TPAMI*, 42(4),
824–836. arXiv:1603.09320.

**Source tier.** T1 (IEEE TPAMI). **Cluster.** D. **GRADE.** SINGLE-SOURCE.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Original C++ code released by authors; now a commodity algorithm (Faiss, hnswlib, usearch, qdrant, milvus, etc.). |
| Evaluation rigor | 2 | ANN benchmarks (SIFT1M, GIST1M, DEEP1B, text8); compared against LSH, KD-tree, annoy, NN-descent; recall-QPS Pareto frontiers. |
| Theoretical soundness | 2 | Hierarchical small-world graph construction is carefully motivated; log-time claim supported by theoretical analysis + empirical fit. |
| Sample adequacy | 2 | Up to 1B-vector benchmarks; multiple domains (image features, text features, random). |
| Bias disclosure | 1 | Independent authors (Russia-based); no industry conflict; limitations discussed but not in a "bias" frame. |
| **Total** | **9 / 10** | **High quality.** Foundational single-source but the method has been commoditized into dozens of implementations. Citation for §4.1 system description is uncontroversial. |

---

### H11 chen2024valle2 — VALL-E 2: Human parity zero-shot TTS

**Citation.** Chen, S., Liu, S., Zhou, L., et al. (2024). VALL-E 2: Neural codec
language models are human parity zero-shot text-to-speech synthesizers.
arXiv:2406.05370.

**Source tier.** T2 (arXiv, Microsoft Research). **Cluster.** E. **GRADE.** STRONG (as comparator).

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 0 | **No code, no weights released.** Training data proprietary (Libriheavy + Microsoft internal). This is a significant weakness. |
| Evaluation rigor | 2 | SIM-o, WER, CMOS, SMOS on LibriSpeech + VCTK; A/B human eval with trained listeners. |
| Theoretical soundness | 1 | "Human parity" claim is marketing-adjacent; paper supports it via CMOS ~= 0, but the comparison is to a small pool of human recordings. Scope creep risk. |
| Sample adequacy | 1 | 50K hours of training audio — large. Eval on 100+ unseen sentences. Limited speaker diversity in eval set. |
| Bias disclosure | 0 | Microsoft authors; no conflict disclosure beyond affiliation; no discussion of benchmark contamination; the "human parity" framing is promotional. Safety discussion absent. |
| **Total** | **4 / 10** | **FLAGGED — LOW QUALITY (reproducibility + bias).** Paper cannot be reproduced by the community; the comparator claim our Case 3 benchmarks against must be hedged heavily. |

**Caveat for writers.** When §6.3 cites VALL-E 2 as the comparator for Case 3's
0.961 SECS, writers must (a) attribute explicitly ("Chen et al. 2024 report
SIM-o ~0.64 on their internal LibriSpeech split"); (b) acknowledge the closed
nature of VALL-E 2 (no weights, no code); (c) note that Case 3's metric is
computed on a publicly reproducible WavLM scorer rather than VALL-E 2's internal
SIM-o eval protocol — the two numbers are not directly comparable without this
caveat. This is the single most important writer-facing caveat in the paper.

---

### H12 meng2025echomimicv3 — EchoMimicV3: 1.3B parameters

**Citation.** Meng, R., Wang, Y., Wu, W., et al. (2025). EchoMimicV3: 1.3B
parameters are all you need for unified multi-modal and multi-task human
animation. arXiv:2507.03905.

**Source tier.** T2 (arXiv, Alibaba Ant Group). **Cluster.** F. **GRADE.** STRONG + AUTHOR-EMPIRICAL context.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 1 | Open-weight release (HuggingFace); inference code open; training code partial; training data proprietary. |
| Evaluation rigor | 2 | FVD, FID, Sync-C, Sync-D, CSIM, E-FID; multi-task eval; comparison to Hallo3, EMO, LatentSync, SadTalker. |
| Theoretical soundness | 1 | Flow-matching sign-convention is the author's forensic concern — the paper's own description of the training objective is thin; sign convention is under-documented (this is exactly why the Case 2 forensic exists). |
| Sample adequacy | 1 | Training corpus undisclosed; benchmarks use public test sets (CelebV-HQ). |
| Bias disclosure | 1 | Alibaba authors; affiliation clear; no discussion of training-data contamination; no COI on reviewer pool; no safety / deepfake ethics section. |
| **Total** | **6 / 10** | **FLAGGED — MODERATE quality (theoretical-soundness + bias).** The base-model paper's under-documented sign convention is the reason the Case 2 forensic exists. Writers must cite with CL-21 forensic attribution. |

---

### H13 hu2022lora — LoRA: Low-rank adaptation

**Citation.** Hu, E. J., Shen, Y., Wallis, P., et al. (2022). LoRA: Low-rank
adaptation of large language models. *ICLR 2022*. arXiv:2106.09685.

**Source tier.** T1 (ICLR 2022). **Cluster.** F. **GRADE.** SINGLE-SOURCE.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Open-source reference code; foundational method — implemented in PEFT, diffusers, trl, unsloth, and hundreds of third-party projects. |
| Evaluation rigor | 2 | RoBERTa / DeBERTa / GPT-2 / GPT-3 175B; GLUE, WikiSQL, SAMSum, E2E NLG, DART, WebNLG; comparison to adapter / prefix / prompt tuning. |
| Theoretical soundness | 2 | Low-rank decomposition motivated by the intrinsic-rank hypothesis; ablations on r = {1, 2, 4, 8, 64} support the low-rank claim. |
| Sample adequacy | 2 | Multiple model scales up to GPT-3 175B; multiple downstream task families. |
| Bias disclosure | 1 | Microsoft authors; affiliation clear; limitations section covers rank-selection but not contamination or application-specific tradeoffs in depth. |
| **Total** | **9 / 10** | **High quality.** Foundational single-source; widely replicated. |

---

### H14 lipman2023flowmatching — Flow matching for generative modeling

**Citation.** Lipman, Y., Chen, R. T. Q., Ben-Hamu, H., Nickel, M., & Le, M.
(2023). Flow matching for generative modeling. *ICLR 2023*. arXiv:2210.02747.

**Source tier.** T1 (ICLR 2023). **Cluster.** F. **GRADE.** SINGLE-SOURCE + AUTHOR-EMPIRICAL.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Reference implementation + now hundreds of third-party implementations (Meta Voicebox, Stable Audio, Hunyuan, EchoMimic, etc.). |
| Evaluation rigor | 2 | CIFAR-10, ImageNet-32 / 64 / 128 / 256; FID / IS / NLL; comparison to diffusion, score-matching, CNF. |
| Theoretical soundness | 2 | Derivation of the flow-matching objective from the continuity equation is rigorous; optimal-transport / conditional-flow-matching connections developed formally. |
| Sample adequacy | 1 | ImageNet scale is strong; extension to audio / video in subsequent papers. Paper itself is image-only. |
| Bias disclosure | 1 | Meta FAIR authors; affiliation clear; limitations section discusses sampling-speed tradeoffs but not downstream-misuse concerns. |
| **Total** | **8 / 10** | **High quality.** Foundational single-source. The sign-convention issue is a downstream-implementation concern; Lipman's derivation itself is unambiguous about the gradient field direction. |

---

### H15 ouyang2022instructgpt — InstructGPT

**Citation.** Ouyang, L., Wu, J., Jiang, X., et al. (2022). Training language
models to follow instructions with human feedback. *NeurIPS 2022*. arXiv:2203.02155.

**Source tier.** T1 (NeurIPS 2022). **Cluster.** I. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 0 | **No weights, no training data, no RLHF dataset released.** Only the architecture and high-level RLHF protocol are documented. This is a systemic issue with OpenAI papers post-GPT-3. |
| Evaluation rigor | 2 | Human evals on API prompt distribution; TruthfulQA; RealToxicityPrompts; zero-shot public benchmarks. |
| Theoretical soundness | 2 | PPO-RLHF derivation is standard RL; claims hedged to "helpfulness" measured by human raters. |
| Sample adequacy | 2 | 175B GPT-3 base; thousands of human-rated comparisons; 40 labelers. |
| Bias disclosure | 1 | OpenAI authors; affiliation clear; labeler demographics partially disclosed; commercial-deployment framing but not promotional in core methodology claim. No discussion of benchmark contamination. |
| **Total** | **7 / 10** | **Moderate quality (threshold).** Deducted for closed model + limited contamination discussion. Foundational for RLHF lineage — citation is uncontroversial as the paradigm anchor. |

---

### H16 rafailov2023dpo — Direct Preference Optimization

**Citation.** Rafailov, R., Sharma, A., Mitchell, E., Ermon, S., Manning, C. D.,
& Finn, C. (2023). Direct preference optimization: Your language model is
secretly a reward model. *NeurIPS 2023*. arXiv:2305.18290.

**Source tier.** T1 (NeurIPS 2023). **Cluster.** I. **GRADE.** STRONG.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | Open-source reference code; method commoditized in TRL / OpenRLHF / Axolotl; reproducible on open-weight models (Llama, Mistral). |
| Evaluation rigor | 2 | IMDB sentiment, TL;DR summarization, Anthropic HH; comparison to PPO, SFT, best-of-N; win-rate evals. |
| Theoretical soundness | 2 | Closed-form derivation from KL-constrained RLHF objective; Bradley-Terry assumption clearly stated; rigorous. |
| Sample adequacy | 1 | Mid-scale experiments (Pythia, GPT-2, up to 6.9B params). Later work at Llama-2-70B scale confirms; original paper is smaller. |
| Bias disclosure | 2 | Stanford academic authors; affiliations clear; limitations section explicit (distribution shift, BT assumption violations, labeler bias). |
| **Total** | **9 / 10** | **High quality.** |

---

### H17 bai2022constitutional — Constitutional AI

**Citation.** Bai, Y., Kadavath, S., Kundu, S., et al. (2022). Constitutional
AI: Harmlessness from AI feedback. arXiv:2212.08073.

**Source tier.** T2 (arXiv, Anthropic). **Cluster.** I. **GRADE.** STRONG (as contrast anchor).

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 0 | **No weights, no training data, no RLAIF corpus released.** Constitution prompts partially documented; full RLAIF pipeline unreleased. |
| Evaluation rigor | 1 | Harmlessness / helpfulness Elo via crowdworker prefs; red-teaming; but no public benchmark comparison with statistical rigor. |
| Theoretical soundness | 1 | The "constitution" is a list of heuristics, not a theory; RLAIF mechanism clear; claims about scalability to super-human AI supervision are speculative. |
| Sample adequacy | 1 | One model size studied (52B Claude-adjacent); thousands of red-team prompts. Limited replication. |
| Bias disclosure | 1 | Anthropic authors; conflict clear; limitations section present but the "safety via constitution" framing is product-adjacent. |
| **Total** | **4 / 10** | **FLAGGED — LOW QUALITY (reproducibility + theoretical soundness).** Paper is foundational *as positioning* but not reproducible. Writers must cite ONLY as a scope anchor ("among the scalar-reward methods are RLHF, DPO, and Constitutional AI"), never as empirical support. |

---

### H18 li2017flame — FLAME: Learning a model of facial shape and expression

**Citation.** Li, T., Bolkart, T., Black, M. J., Li, H., & Romero, J. (2017).
*ACM TOG*, 36(6), Article 194. DOI:10.1145/3130800.3130813.

**Source tier.** T1 (ACM TOG). **Cluster.** F. **GRADE.** MODERATE.

| Dim | Score | Justification |
|---|---|---|
| Reproducibility | 2 | FLAME model + fitting code + 33K 3D scans (CAESAR subset) released via flame.is.tue.mpg.de; commoditized (used by DECA, EMOCA, SPECTRE, MICA). |
| Evaluation rigor | 2 | Generalization to BU-3DFE, D3DFACS, Florence 2D/3D; quantitative per-vertex error vs. BFM, FaceWarehouse, Dense3DFace. |
| Theoretical soundness | 2 | Linear shape + expression + pose blendshape model; derivation from 4D scan registration rigorous. |
| Sample adequacy | 2 | 33K 3D face scans from ~3,800 subjects; CAESAR diversity across age / sex / ethnicity. |
| Bias disclosure | 1 | MPI academic authors; CAESAR population originally sampled for anthropometric purposes; demographic balance modest; paper does not discuss ethnicity bias in detail. |
| **Total** | **9 / 10** | **High quality.** Foundational facial-shape model; canonical citation for FACS-adjacent work. |

---

## HOT-tier aggregate

| Citation | Rep | Eval | Snd | Samp | Bias | **Total** | Status |
|---|---|---|---|---|---|---|---|
| H01 villalobos2024data | 2 | 1 | 2 | 2 | 2 | **9** | OK |
| H02 hoffmann2022chinchilla | 1 | 2 | 2 | 2 | 1 | **8** | OK |
| H03 longpre2024consent | 2 | 2 | 2 | 2 | 2 | **10** | OK |
| H04 shumailov2024nature | 2 | 2 | 2 | 2 | 2 | **10** | OK |
| H05 alemohammad2023mad | 2 | 2 | 2 | 2 | 2 | **10** | OK |
| H06 deletang2024compression | 2 | 2 | 2 | 2 | 1 | **9** | OK |
| H07 huang2024compression | 2 | 2 | 1 | 1 | 1 | **7** | OK (attribution) |
| H08 chen2022wavlm | 2 | 2 | 2 | 2 | 1 | **9** | OK |
| H09 zhai2023siglip | 2 | 2 | 2 | 2 | 1 | **9** | OK |
| H10 malkov2018hnsw | 2 | 2 | 2 | 2 | 1 | **9** | OK |
| **H11 chen2024valle2** | **0** | 2 | 1 | 1 | **0** | **4** | **FLAGGED** |
| **H12 meng2025echomimicv3** | 1 | 2 | 1 | 1 | 1 | **6** | **FLAGGED** |
| H13 hu2022lora | 2 | 2 | 2 | 2 | 1 | **9** | OK |
| H14 lipman2023flowmatching | 2 | 2 | 2 | 1 | 1 | **8** | OK |
| H15 ouyang2022instructgpt | 0 | 2 | 2 | 2 | 1 | **7** | OK (attribution) |
| H16 rafailov2023dpo | 2 | 2 | 2 | 1 | 2 | **9** | OK |
| **H17 bai2022constitutional** | **0** | 1 | 1 | 1 | 1 | **4** | **FLAGGED** |
| H18 li2017flame | 2 | 2 | 2 | 2 | 1 | **9** | OK |

**Hot-tier mean.** 144 / 180 = **8.0 / 10**.
**Hot-tier median.** 9 / 10.
**Hot-tier ≥ 8 fraction.** 14 / 18 = **77.8%**.
**Hot-tier ≥ 7 fraction.** 16 / 18 = **88.9%**.
**Hot-tier flagged (< 7).** **3** citations: H11 valle2 (4), H12 echomimicv3 (6), H17 constitutional (4).

---

## WARM-tier spot checks (n = 10)

### W01 dohmatob2024tails — Tale of tails

**Citation.** Dohmatob, E., Feng, Y., Yang, P., Charton, F., & Kempe, J. (2024).
A tale of tails: Model collapse as a change of scaling laws. *ICML 2024*.
arXiv:2402.07043.

**Cluster.** B. **Source tier.** T1. **Score.** 2+2+2+2+2 = **10 / 10.**
(Open code, scaling-law crossover derivation rigorous, multi-scale experiments,
NYU/FAIR authors with full disclosure.)

### W02 gerstgrasser2024colm — Is model collapse inevitable?

**Cluster.** B. **Source tier.** T2. **Score.** 2+2+2+2+2 = **10 / 10.**

### W03 nussbaum2024nomic — Nomic Embed

**Cluster.** D. **Source tier.** T2. **Score.** 2+2+2+2+2 = **10 / 10.** (Fully
reproducible text embedder; open weights, training data, training code — gold
standard for ML reproducibility.)

### W04 santhanam2022colbertv2 — ColBERTv2

**Cluster.** D. **Source tier.** T1. **Score.** 2+2+2+2+1 = **9 / 10.**

### W05 tan2024naturalspeech3 — NaturalSpeech 3

**Cluster.** E. **Source tier.** T1. **Score.** 0+2+2+1+0 = **5 / 10.**
**Flagged.** Microsoft; no weights / code released; promotional "natural speech"
framing. Same pattern as VALL-E 2. Writers must use parallel hedging.

### W06 wang2024maskgct — MaskGCT

**Cluster.** E. **Source tier.** T1. **Score.** 2+2+2+2+1 = **9 / 10.** (Amphion
team open-sources weights and code; strong comparator.)

### W07 reddy2022dnsmos — DNSMOS P.835

**Cluster.** E. **Source tier.** T2. **Score.** 2+2+2+2+1 = **9 / 10.**

### W08 saeki2022utmos — UTMOS

**Cluster.** E. **Source tier.** T2. **Score.** 2+2+2+2+1 = **9 / 10.**

### W09 baevski2020wav2vec — wav2vec 2.0

**Cluster.** G. **Source tier.** T1 (actually NeurIPS 2020). **Score.** 2+2+2+2+1 = **9 / 10.**

### W10 henderson2022pileoflaw — Pile of Law

**Cluster.** H. **Source tier.** T1. **Score.** 2+2+2+2+2 = **10 / 10.**

### Warm-tier spot-check summary

| Citation | Score | Status |
|---|---|---|
| W01 dohmatob2024tails | 10 | OK |
| W02 gerstgrasser2024colm | 10 | OK |
| W03 nussbaum2024nomic | 10 | OK |
| W04 santhanam2022colbertv2 | 9 | OK |
| **W05 tan2024naturalspeech3** | **5** | **FLAGGED (Microsoft closed; promotional)** |
| W06 wang2024maskgct | 9 | OK |
| W07 reddy2022dnsmos | 9 | OK |
| W08 saeki2022utmos | 9 | OK |
| W09 baevski2020wav2vec | 9 | OK |
| W10 henderson2022pileoflaw | 10 | OK |

**Warm spot-check mean.** 90 / 100 = **9.0 / 10**.
**Warm spot-check ≥ 8 fraction.** 9 / 10 = **90%**.
**Warm spot-check flagged (< 7).** 1 / 10 (NaturalSpeech 3).

---

## Cross-tier quality summary

| Tier | N scored | Mean | Median | ≥ 8 | < 7 flagged |
|---|---|---|---|---|---|
| HOT | 18 | 8.0 | 9 | 14 (78%) | 3 |
| WARM (spot check) | 10 | 9.0 | 9 | 9 (90%) | 1 |
| **Combined (28)** | **28** | **8.4** | **9** | **23 (82%)** | **4** |

## Systematic pattern: the closed-TTS / closed-RLHF problem

The four flagged citations across hot + warm (VALL-E 2, EchoMimicV3, Constitutional AI,
NaturalSpeech 3) share a common root cause: **industry labs release papers
without accompanying weights / code / data, claiming performance improvements
that the community cannot reproduce.** This is a field-wide issue that our
manuscript cannot solve — but we must call it out explicitly:

- In §2.5 (related TTS work) — note that VALL-E 2 and NaturalSpeech 3 results
  are not community-reproducible; Case 3 uses open scorers (WavLM, DNSMOS, UTMOS)
  so our numbers are reproducible even if the comparator numbers are not.
- In §2.6 (alignment) — note that Constitutional AI is not reproducible; the
  contrast is with the *paradigm* (scalar reward + preference), not with any
  reproducible artefact.
- In §6.2 (Case 2 Santa) — note that EchoMimicV3 is semi-reproducible
  (weights open, training code partial), and the Case 2 forensic (sign-
  convention) *is* the disclosure the base paper lacks.
- In §8 Limitations — list the closed-comparator problem as a limitation of
  comparison, not of our contribution.

## Confidence

**HIGH.** Rubric is standard ML-paper reproducibility / rigor criteria. Every
score traces to a specific paper section or commodity implementation. The
three flagged hot-tier citations (VALL-E 2, EchoMimicV3, CAI) are exactly the
papers writers would otherwise cite unreservedly — flagging them here prevents
downstream over-reliance.

## Changelog

- 2026-04-16 — Initial issue by agent-13-quality-assessor.
