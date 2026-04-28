# Evidence Brief 01: AI Training Data Wall, Model Collapse, and Meaning Density

*Source: researcher agent ad07cb67016c50d87. APA 7th edition, all URLs verified at time of retrieval.*

---

## 1. Data Wall Projections

- **Villalobos, Ho, Sevilla et al. (2022/2024)** — "Will we run out of data? Limits of LLM scaling based on human-generated data." arXiv:2211.04325 (v1 Oct 2022, revised Jun 2024). ICML 2024.
  - Estimates the effective stock of public human-generated text at approximately **300 trillion tokens** (median estimate, indexed web text, with uncertainty range spanning ~100T–3,000T).
  - Projects that if current LLM training dataset growth trends continue, models will **fully utilize this stock between 2026 and 2032**, with a median exhaustion date of ~2028.
  - Notes that **overtraining** (training on more tokens per parameter than Chinchilla-optimal, e.g., Llama 3 at ~200 tokens/param vs. Chinchilla's ~20) **accelerates the exhaustion timeline by up to ~5x**, pulling the wall forward toward 2025–2027.
  - Quote (abstract, 2024 revision): "if current LLM development trends continue, models will be trained on datasets roughly equal in size to the available stock of public human text data between 2026 and 2032, or slightly earlier if models are overtrained."

- **Longpre, Mahari, Chen et al. (2024)** — "Consent in Crisis: The Rapid Decline of the AI Data Commons." arXiv:2407.14933. NeurIPS 2024 Datasets & Benchmarks Track.
  - Documents that **~25% of high-quality C4 tokens and ~5% of all C4 tokens** became restricted by robots.txt and ToS changes between 2023 and mid-2024, tightening the effective accessible web corpus.

## 2. Model Collapse

- **Shumailov, Shumaylov, Zhao, Gal, Papernot, Anderson (2024)** — "AI models collapse when trained on recursively generated data." *Nature* 631(8022):755–759. DOI: [10.1038/s41586-024-07566-y](https://doi.org/10.1038/s41586-024-07566-y). (Earlier preprint: "The Curse of Recursion: Training on Generated Data Makes Models Forget," arXiv:2305.17493, 2023.)
  - Demonstrates that generative models trained recursively on their own outputs exhibit **irreversible degenerative processes** where learned distributions drift away from the original data distribution.
  - Defines **early model collapse**: the model loses information about the **tails** of the distribution (low-probability events disappear first).
  - Defines **late model collapse**: variance collapses and the distribution **converges to a point-mass-like estimator** with little resemblance to the original, often with small variance.
  - Architectures tested: **Gaussian Mixture Models (GMMs), Variational Autoencoders (VAEs), and Large Language Models (OPT-125M fine-tuned on wikitext2)** — collapse is shown to be universal across all three.
  - Quote: "We find that indiscriminate use of model-generated content in training causes irreversible defects in the resulting models, in which tails of the original content distribution disappear."

## 3. Synthetic Data Mitigation

- **Gerstgrasser, Schaeffer, Dey et al. (2024)** — "Is Model Collapse Inevitable? Breaking the Curse of Recursion by Accumulating Real and Synthetic Data." arXiv:2404.01413. COLM 2024.
  - Shows that **accumulating** (rather than replacing) real data with synthetic data **prevents model collapse**: test error remains bounded across iterations, in contrast to the replace-only regime which diverges.
  - Quote: "We find that if each generation's data accumulates alongside the original real data, model collapse is avoided."

- **Dohmatob, Feng, Yang, Charton, Kempe (2024)** — "A Tale of Tails: Model Collapse as a Change of Scaling Laws." arXiv:2402.07043. ICML 2024.
  - Derives **modified neural scaling laws under synthetic-data contamination**, showing that collapse manifests as a "scaling-law crossover": beyond a critical dataset size, additional synthetic data yields no further loss reduction (scaling law flattens).

- **Dohmatob, Feng, Kempe (2024)** — "Model Collapse Demystified: The Case of Regression." arXiv:2402.07712. NeurIPS 2024.
  - Provides closed-form analysis of collapse in linear regression and confirms that **unbounded iterative synthetic training yields strictly worse test risk** than a single-generation mix.

- **Feng, Dohmatob, Yang, Charton, Kempe (2024)** — "Beyond Model Collapse: Scaling Up with Synthesized Data Requires Verification." arXiv:2406.07515.
  - Demonstrates that **verification/filtering of synthetic data** (via an oracle verifier or learned reward model) recovers and can exceed real-data scaling, formalizing why RLHF/rejection-sampling pipelines escape collapse.

- **Alemohammad, Casco-Rodriguez, Luzi et al. (2023)** — "Self-Consuming Generative Models Go MAD." arXiv:2307.01850. ICLR 2024.
  - Introduces "Model Autophagy Disorder (MAD)" for image generators; shows fully-synthetic loops degrade quality/diversity while **fresh real data injections each generation stabilize training**.

## 4. Scaling Laws

- **Kaplan, McCandlish, Henighan et al. (2020)** — "Scaling Laws for Neural Language Models." arXiv:2001.08361.
  - Establishes that LM test loss follows a **power law in model size (N), dataset size (D), and compute (C)**, with compute-optimal allocation favoring larger models over more data.

- **Hoffmann, Borgeaud, Mensch et al. (2022)** — "Training Compute-Optimal Large Language Models" (Chinchilla). arXiv:2203.15556. NeurIPS 2022.
  - Revises Kaplan's prescription: for a fixed compute budget, **model parameters and training tokens should scale in roughly equal proportion (~20 tokens per parameter)**, showing prior models were undertrained.

## 5. Industry Signals on Pre-Training Plateau (2024–2025)

- **The Information (Efrati & Holmes, Nov 9, 2024)** — "OpenAI Shifts Strategy as Rate of 'GPT' AI Improvements Slows." https://www.theinformation.com/articles/openai-shifts-strategy-as-rate-of-gpt-ai-improvements-slows
- **Bloomberg (Metz, Ghaffary, Love, Nov 13, 2024)** — "OpenAI, Google and Anthropic Are Struggling to Build More Advanced AI." https://www.bloomberg.com/news/articles/2024-11-13/openai-google-and-anthropic-are-struggling-to-build-more-advanced-ai
- **Reuters (Hu & Tong, Nov 15, 2024)** — "OpenAI and others seek new path to smarter AI as current methods hit limitations." https://www.reuters.com/technology/artificial-intelligence/openai-rivals-seek-new-path-smarter-ai-current-methods-hit-limitations-2024-11-11/
- **TechCrunch (Wiggers, Nov 20, 2024)** — https://techcrunch.com/2024/11/20/current-ai-scaling-laws-are-showing-diminishing-returns-forcing-ai-labs-to-change-course/
- Sutskever, NeurIPS 2024 Test of Time talk: "Pre-training as we know it will unquestionably end … we have but one internet … data is the fossil fuel of AI."

## 6. Counter-Argument: Meaning Density / Compression

- **Delétang, Ruoss, Duquenne et al. (2024)** — "Language Modeling Is Compression." arXiv:2309.10668. ICLR 2024.
  - Formalizes **prediction–compression equivalence**: Chinchilla 70B compresses ImageNet to 43% and LibriSpeech to 16.4% of raw size, beating PNG/FLAC. Information density, not token count, is the operative quantity.

- **Huang, Vincent, Ye et al. (2024)** — "Compression Represents Intelligence Linearly." arXiv:2404.09937. COLM 2024.
  - Linear correlation (r ≈ 0.95) between compression ratio and downstream benchmark performance across 30+ LLMs.

- **Li, Yang, Zhang et al. (2025)** — "Lossless data compression by large language models." *Nature Machine Intelligence*. DOI: [10.1038/s42256-025-01033-7](https://doi.org/10.1038/s42256-025-01033-7).
  - LLM arithmetic coding achieves ~2x better compression than gzip/bzip2 across text, image, audio, video.

- **Valmeekam, Narayanan, Tuli et al. (2023)** — "LLMZip: Lossless Text Compression using Large Language Models." arXiv:2306.04050.
  - LLaMA-7B achieves ~0.71 bits/char on text8, below prior SOTA ~0.94 bpc.

- **Jiang, Yang, Cui, Han (2023)** — "'Low-Resource' Text Classification: A Parameter-Free Classification Method with Compressors." ACL 2023 Findings. https://aclanthology.org/2023.findings-acl.426/
  - gzip-based distance outperforms BERT on several low-resource benchmarks.

- **Seddik, Chen, Hayou, Youssef, Debbah (2024)** — "How Bad is Training on Synthetic Data? A Statistical Analysis of Language Model Collapse." arXiv:2404.05090. COLM 2024.
