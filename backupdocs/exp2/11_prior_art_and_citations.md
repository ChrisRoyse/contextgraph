# EXP-2 Prior Art and Citations

**Every claim in EXP-2 stands on named prior work.** This document lists the full citation set for the experimental protocol and the theoretical framing. It is the reference list for the final report and the Perez memo revision.

---

## 1 · Information-theoretic foundations

### 1.1 Shannon's entropy and mutual information
Shannon, C. E. (1948). *A Mathematical Theory of Communication.* Bell System Technical Journal, 27(3), 379–423.
- Original definition of $H(X) = -\mathbb{E}_{x \sim p}[\log p(x)]$ and $I(X; Y) = H(X) - H(X|Y)$.

### 1.2 Watanabe total correlation
Watanabe, S. (1960). *Information Theoretical Analysis of Multivariate Correlation.* IBM Journal of Research and Development, 4(1), 66–82.
- Total correlation $C(X_1, \dots, X_N) = \sum_i H(X_i) - H(X_1, \dots, X_N)$ — the aggregated redundancy that $N_\text{eff,pair}$ upper-bounds.

### 1.3 Han's inequality (pair decomposition upper bound)
Han, T. S. (1978). *Nonnegative entropy measures of multivariate symmetric correlations.* Information and Control, 36(2), 133–156.
- Establishes $\sum_{i<j} I(X_i; X_j)$ as a finite-ordered decomposition bound on total correlation when higher-order interactions are bounded.

### 1.4 KSG estimator
Kraskov, A., Stögbauer, H., & Grassberger, P. (2004). *Estimating mutual information.* Physical Review E, 69(6), 066138.
- The non-parametric $k$-NN-based estimator used in EXP-2 as the tertiary cross-check. Algorithm 1.

### 1.5 MINE
Belghazi, M. I., Baratin, A., Rajeshwar, S., Ozair, S., Bengio, Y., Courville, A., & Hjelm, R. D. (2018). *Mutual Information Neural Estimation.* ICML 2018. arXiv:1801.04062.
- Donsker-Varadhan dual form with neural critic. Primary estimator in EXP-2. The 2-layer MLP 256-width critic is the reference-implementation default used here.

### 1.6 InfoNCE
van den Oord, A., Li, Y., & Vinyals, O. (2018). *Representation Learning with Contrastive Predictive Coding.* arXiv:1807.03748.
- Lower bound $\hat{I}_\text{NCE}(X; Y) \le I(X; Y)$ derived from noise-contrastive classification. Saturates at $\log K$; used as the second estimator.

### 1.7 MI estimation pitfalls
Poole, B., Ozair, S., van den Oord, A., Alemi, A., & Tucker, G. (2019). *On Variational Bounds of Mutual Information.* ICML 2019. arXiv:1905.06922.
- Theoretical comparison of MINE, InfoNCE, NWJ, DV bounds. Documents the high-variance failure mode of MINE, the bias of InfoNCE, and recommends ensemble estimation.

### 1.8 Formal limits on MI estimation
McAllester, D., & Stratos, K. (2020). *Formal Limitations on the Measurement of Mutual Information.* AISTATS 2020.
- Shows high-dimensional MI is PAC-non-estimable from polynomial samples without additional assumptions. Motivates the pre-registered protocol + three-estimator defense.

### 1.9 Paninski entropy estimator
Paninski, L. (2003). *Estimation of entropy and mutual information.* Neural Computation, 15(6), 1191–1253.
- Asymptotic bias and variance of the MLE entropy estimator. Used (with Chao-Shen correction) in EXP-2 for joint block entropy $H(\Phi)$ in $N_\text{eff,joint}$.

### 1.10 Chao-Shen entropy correction
Chao, A., & Shen, T.-J. (2003). *Nonparametric estimation of Shannon's index of diversity when there are unseen species in sample.* Environmental and Ecological Statistics, 10, 429–443.
- Correction for finite-sample bias in entropy estimation. Applied in EXP-2 to the block-entropy estimator.

---

## 2 · Representation convergence — the PRH literature

### 2.1 Platonic Representation Hypothesis (THE falsification candidate for Axiom 2)
Huh, M., Cheung, B., Wang, T., & Isola, P. (2024). *The Platonic Representation Hypothesis.* ICML 2024. arXiv:2405.07987.
- Central empirical claim: representations learned by vision and language models increasingly converge to a common underlying representation as models and data scale. Tests via CKA over encoder pairs.
- This paper is the predicted null for EXP-2 if H1 wins. Directly cited in [01_preregistration.md §1](01_preregistration.md) as the alternative hypothesis.

### 2.2 CKA — Centered Kernel Alignment
Kornblith, S., Norouzi, M., Lee, H., & Hinton, G. (2019). *Similarity of Neural Network Representations Revisited.* ICML 2019.
- Defines linear CKA, the metric Huh et al. 2024 use. Reported in EXP-2 as a secondary metric for direct PRH comparability.

### 2.3 RSA — Representational Similarity Analysis
Kriegeskorte, N., Mur, M., & Bandettini, P. (2008). *Representational similarity analysis — connecting the branches of systems neuroscience.* Frontiers in Systems Neuroscience, 2, 4.
- The cognitive-neuroscience ancestor of CKA. Context: EXP-2's CKA reporting inherits this lineage.

### 2.4 Are features universal?
Morcos, A. S., Raghu, M., & Bengio, S. (2018). *Insights on representational similarity in neural networks with canonical correlation.* NeurIPS 2018.
- Early work on cross-network representation alignment via canonical correlation analysis (CCA). Supports the PRH direction but with older tooling.

---

## 3 · Scaling laws and supervisory signal density

### 3.1 Kaplan scaling laws
Kaplan, J., McCandlish, S., Henighan, T., Brown, T. B., Chess, B., Child, R., Gray, S., Radford, A., Wu, J., & Amodei, D. (2020). *Scaling Laws for Neural Language Models.* arXiv:2001.08361.
- Loss scales as $L \sim C^{-\alpha}$ with compute, data, parameters. The counting identity $n \cdot (N + \binom{N}{2})$ is framed as an extension of the data-dimension axis of Kaplan 2020 — supervisory signal density per input instead of just inputs.
- The Perez memo is written as an extension of this paper.

### 3.2 Chinchilla scaling laws
Hoffmann, J., Borgeaud, S., Mensch, A., et al. (2022). *Training Compute-Optimal Large Language Models.* arXiv:2203.15556.
- Refinement of Kaplan: compute-optimal data-to-parameter ratio. Relevant to the "data wall" framing.

### 3.3 Data-wall discourse
Villalobos, P., Sevilla, J., Heim, L., Besiroglu, T., Hobbhahn, M., & Ho, A. (2024). *Will we run out of data? Limits of LLM scaling based on human-generated data.* arXiv:2211.04325 (v3).
- Argues for a 2026–2032 data exhaustion horizon. The commercial framing for EXP-2 + the counting identity rests on whether DDA (Derived Data Abundance) is a viable third path.

### 3.4 Collapse discourse — scope-correct framing
Shumailov, I., Shumaylov, Z., Zhao, Y., Papernot, N., Anderson, R., & Gal, Y. (2024). *The Curse of Recursion: Training on Generated Data Makes Models Forget.* Nature 631, 755–759.
- Recursion-regime collapse result. Scope-correctly, EXP-2's panel is **not** trained via recursive LLM generation; the panel-level redundancy measurement is a separate property class from Shumailov's recursion curse. Noted in the Introduction scope-guard per playbook.
- **Scope claim**: EXP-2 measures encoder-encoder redundancy, not generator-training collapse. These are structurally different phenomena.

### 3.5 Collapse contested — definition framing
Kapoor, S., Narayanan, A., et al. (2025). *Model Collapse Revisited: What Counts as Collapse?* (working paper as of 2025; arXiv ID TBD).
- Fragments the collapse definition; several "collapse" scenarios are avoidable by non-DDA means. The playbook reframes DDA as **one constructive alternative to collapse-prone paths**, not a collapse solver.

---

## 4 · Multi-encoder / ensemble retrieval

### 4.1 ColBERT (for E12)
Khattab, O., & Zaharia, M. (2020). *ColBERT: Efficient and Effective Passage Search via Contextualized Late Interaction over BERT.* SIGIR 2020.
- The architecture of E12. MaxSim scoring. The mean-pool workaround for EXP-2 is explicitly a limitation.

### 4.2 SPLADE (for E6, E13)
Formal, T., Piwowarski, B., & Clinchant, S. (2021). *SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking.* SIGIR 2021.
- E6 and E13 backbone. Same MLM head + log(1 + ReLU) signature.

### 4.3 KEPLER (for E11)
Wang, X., Gao, T., Zhu, Z., Zhang, Z., Liu, Z., Li, J., & Tang, J. (2021). *KEPLER: A Unified Model for Knowledge Embedding and Pre-trained Language Representation.* TACL.
- E11 architecture. Jointly trains entity embeddings + language model with TransE-style knowledge graph loss.

### 4.4 nomic-embed-v1.5 (for E5)
Nussbaum, Z., Morris, J. X., Duderstadt, B., & Mulyar, A. (2024). *Nomic Embed Training: Open-source, Fully Reproducible, Long Context English Embedders.* Nomic AI Technical Report.
- E5 base model. E5 in Context Graph adds LoRA rank 16 + projection head fine-tuned on causal discovery data.

### 4.5 E5 (for E1 and E8 backbone)
Wang, L., Yang, N., Huang, X., Jiao, B., Yang, L., Jiang, D., Majumder, R., & Wei, F. (2022). *Text Embeddings by Weakly-Supervised Contrastive Pre-training.* arXiv:2212.03533.
- E1 (e5-large-v2, 1024D) and E8 (shared backbone with learned source / target projection) share this pretraining. Direct predictor of high E1-E8 MI.

### 4.6 Qodo-Embed (for E7)
Qodo AI. (2024). *Qodo-Embed-1-1.5B.* HuggingFace model card.
- E7 (1536D, 32K context, Qwen2-based). Code-retrieval specialty.

### 4.7 Hyperdimensional computing (for E9)
Kanerva, P. (2009). *Hyperdimensional computing: An introduction to computing in distributed representation with high-dimensional random vectors.* Cognitive Computation, 1(2), 139–159.
- E9 foundation: random hypervectors + bind / bundle / permute.

### 4.8 Reciprocal Rank Fusion
Cormack, G. V., Clarke, C. L., & Buettcher, S. (2009). *Reciprocal rank fusion outperforms Condorcet and individual rank learning methods.* SIGIR 2009.
- The fusion method used in production search. Context for why the panel's effective-N matters for retrieval.

---

## 5 · Pre-registration and open science

### 5.1 Pre-registration framework
Nosek, B. A., Ebersole, C. R., DeHaven, A. C., & Mellor, D. T. (2018). *The preregistration revolution.* PNAS 115(11), 2600–2606.
- The methodological framework for locking hypotheses before data collection.

### 5.2 ICLR Reproducibility Checklist
Pineau, J., et al. (2020). *Improving Reproducibility in Machine Learning Research.* JMLR.
- The de-facto standard for ML reproducibility. EXP-2's deliverables align with this.

### 5.3 Model and data cards
Mitchell, M., Wu, S., Zaldivar, A., et al. (2019). *Model Cards for Model Reporting.* FAT* 2019.
- Format convention for model reporting. EXP-2 report incorporates a scaled-down model card for each of the 13 embedders.

---

## 6 · Johnson-Lindenstrauss bounds (for E6 / E13 sketching)

Johnson, W. B., & Lindenstrauss, J. (1984). *Extensions of Lipschitz mappings into a Hilbert space.* Contemporary Mathematics, 26, 189–206.
- Bound: random projection $\mathbb{R}^d \to \mathbb{R}^k$ with $k = O(\log n / \epsilon^2)$ preserves pairwise distances within $(1 \pm \epsilon)$. For EXP-2 at $n = 1000, k = 2048, \epsilon \approx 0.15$, the bound holds with high probability.

Indyk, P., & Motwani, R. (1998). *Approximate nearest neighbors: towards removing the curse of dimensionality.* STOC 1998.
- Practical application of JL sketching for high-dim nearest neighbors. Same technique we borrow for E6 / E13.

---

## 7 · Patent prior-art search keywords (for ACTION #19 monitoring)

These keywords are seeded on Day 4 (per action list) into Google Alerts, USPTO RSS, and Semantic Scholar:

- "multi-embedder training"
- "constellation training"
- "derived data abundance"
- "supervisory signal multiplication"
- "frozen embedder panel"
- "three-role composition retriever labeller"
- "per-output cosine predicate"
- "meaning compression"
- "deterministic LoRA"
- "pairwise mutual information embedder"
- "embedder redundancy audit"
- "effective embedder count"
- "N effective embedders"
- "counting identity embedding"
- "n(N + N choose 2) signals"

---

## 8 · Citation-format commitment

Every published EXP-2 artifact (report, Perez memo revision, patent disclosure, NeurIPS supplement) uses ACL-style in-text + alphabetized reference list. DOI + arXiv-ID required. Anonymous technical reports are allowed with URL.

Every number cited in any artifact has a reference that supports it. Numbers we measured ourselves cite [07_reporting_template.md](07_reporting_template.md) and the preregistration SHA.
