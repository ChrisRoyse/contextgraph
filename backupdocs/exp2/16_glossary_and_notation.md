# EXP-2 Glossary and Notation

**One place to look up any symbol in any EXP-2 doc.** Before using a symbol in the final report or Perez memo, check it's defined here.

---

## Symbols (math)

| Symbol | Definition |
|--------|------------|
| $X$ | A random input, drawn from the production distribution $\mathcal{P}$. |
| $\mathcal{P}$ | The production distribution of inputs. For EXP-2 this is live Context Graph memories at freeze time. |
| $N$ | Panel size. In EXP-2: $N = 13$. |
| $\binom{N}{2}$ | Binomial coefficient. At $N = 13$: $\binom{13}{2} = 78$. |
| $\phi_j$ | The $j$-th embedder ($1 \le j \le 13$). Alias $\phi_{E1}$ etc. for clarity. |
| $\phi_j(X)$ | The embedder output as a random variable (function of $X$). |
| $\Phi$ | The full panel: $\Phi = (\phi_1, \dots, \phi_{13})$. |
| $\Phi(X)$ | The joint panel output: the 13-tuple $(\phi_1(X), \dots, \phi_{13}(X))$. |
| $H(\phi_j)$ | Shannon entropy of $\phi_j(X)$ in bits. Shorthand for $H(\phi_j(X))$. |
| $I(\phi_j; \phi_k)$ | Mutual information between $\phi_j(X)$ and $\phi_k(X)$ in bits. |
| $\hat{I}$ | Estimated MI (any of MINE / InfoNCE / KSG). |
| $\bar{H}$ | Mean per-embedder entropy: $\bar{H} = \frac{1}{N}\sum_j H(\phi_j)$. |
| $\bar{I}$ | Mean pairwise MI: $\bar{I} = \frac{1}{\binom{N}{2}}\sum_{j<k} I(\phi_j; \phi_k)$. |
| $R$ | Total redundancy: $R = \sum_j H(\phi_j) - H(\Phi)$. |
| $\rho$ | Redundancy fraction: $\rho = R / \sum_j H(\phi_j) \in [0, 1 - 1/N]$. |
| $N_\text{eff,pair}$ | Pair-decomposed effective count (headline). See [06_analysis_and_decision_rules.md §2.1](06_analysis_and_decision_rules.md). |
| $N_\text{eff,joint}$ | Joint-entropy effective count: $H(\Phi) / \bar{H}$. |
| $N_\text{eff,min}$ | Single-worst-pair conservative: $N - \max_{j<k} I(\phi_j; \phi_k) / \bar{H}$. |
| $N_\text{eff,sum}$ | Additive (not a bound): $N - \sum_{j<k} I(\phi_j; \phi_k) / \bar{H}$. |
| $\hat{N}$ | Point estimate of $N_\text{eff,pair}$ from bootstrap. |
| CI, L, U | 95 % bootstrap confidence interval and its lower / upper bounds. |
| $G_\tau$ | Per-output cosine-similarity predicate operator with threshold $\tau$. Used in the three-role composition primitive; separate property class from pair-MI. |
| $Y_T$ | Task label (from follow-up experiment EXP-2B). |
| $T$ | Critic function in MINE. Also used for task in EXP-2B — context disambiguates. |
| $f(x, y)$ | Critic function in InfoNCE: $f = (W_X x)^\top (W_Y y)$. |
| $W_X, W_Y$ | Learned InfoNCE projection matrices. |
| $\tau$ | InfoNCE temperature (0.1 fixed) or $G_\tau$ threshold — context disambiguates. |
| $K$ | InfoNCE batch size (also the upper bound $\log K$). |
| $k$ | KSG neighbor count (fixed at 3). |
| $n$ | Sample size. EXP-2 runs at $n \in \{100, 500, 1000\}$. |
| $M$ | Total corpus size at freeze (count of live non-soft-deleted memories). |
| $B$ | Number of bootstrap samples (fixed at 1000). |
| $\epsilon_i$ | Distance from point $i$ to its $k$-th nearest neighbor (KSG). |
| $n_x(i), n_y(i)$ | Marginal neighbor counts (KSG Algorithm 1). |
| $\psi(\cdot)$ | Digamma function. |
| $D_{KL}$ | Kullback-Leibler divergence. |

---

## Acronyms

| Acronym | Expansion |
|---------|-----------|
| **MI** | Mutual Information |
| **CMI** | Conditional Mutual Information |
| **MINE** | Mutual Information Neural Estimation (Belghazi 2018) |
| **InfoNCE** | Info Noise Contrastive Estimation (Oord 2018) |
| **KSG** | Kraskov-Stögbauer-Grassberger (2004) |
| **NCE** | Noise Contrastive Estimation |
| **NWJ** | Nguyen-Wainwright-Jordan (variational MI bound) |
| **SMILE** | Self-Normalized Importance Sampling MI Estimator |
| **CKA** | Centered Kernel Alignment (Kornblith 2019) |
| **RSA** | Representational Similarity Analysis |
| **CCA** | Canonical Correlation Analysis |
| **PRH** | Platonic Representation Hypothesis (Huh 2024) |
| **SSL** | Self-Supervised Learning |
| **DDA** | Derived Data Abundance |
| **HNSW** | Hierarchical Navigable Small World (ANN index) |
| **PCA** | Principal Component Analysis |
| **JL** | Johnson-Lindenstrauss (sketch / projection) |
| **BERT** | Bidirectional Encoder Representations from Transformers |
| **LoRA** | Low-Rank Adaptation |
| **HDC** | Hyperdimensional Computing |
| **ColBERT** | Contextualized Late Interaction over BERT |
| **SPLADE** | Sparse Lexical and Expansion |
| **KEPLER** | Knowledge Embedding + Pretrained Language Representation |
| **EMA** | Exponential Moving Average |
| **MaxSim** | Maximum Similarity (ColBERT scoring) |
| **BSL** | Business Source License |

---

## Key terms

| Term | Definition |
|------|------------|
| **Counting identity** | $n \cdot (N + \binom{N}{2})$: the constructive upper bound on supervisory signals per input at $N$ embedders. |
| **Constructive ceiling** | The 91 number in the counting identity: a structural upper bound. |
| **Realized floor** | The measured $N_\text{eff,pair}$ — the information-theoretically effective count. |
| **Axiom 2** | "Approximate embedder independence." The load-bearing assumption for the counting identity. EXP-2 quantifies this axiom. |
| **Pre-registration** | Locked protocol before data collection. Prevents p-hacking. Format: [01_preregistration.md](01_preregistration.md). |
| **Three-role composition** | Retriever + Labeller + $G_\tau$ per-output predicate. A compositional primitive independent of panel independence. |
| **Platonic Representation Hypothesis** | Huh et al. 2024: modern SSL encoders converge on a common underlying representation. |
| **Panel** | The 13-embedder set $\Phi$. Specified in [02_panel_specification.md](02_panel_specification.md). |
| **Asymmetric embedder** | Embedder storing two vectors (e.g., E5 cause/effect, E8 source/target, E10 paraphrase/context). |
| **Weight-gated embedder** | Embedder active only when profile assigns non-zero weight (E2/E3/E4 in production). |
| **Quantized embedder** | Each embedder in production has a stored quantization (PQ-8, Float8, Binary, Sparse, TokenPruning). For EXP-2, full-precision vectors are used. |
| **Dual-vector concatenation** | For EXP-2, E5 / E8 / E10 halves are concatenated into a single vector. |
| **JL sketch** | Random Gaussian projection used in EXP-2 to dense-ify E6 / E13 sparse 30522-dim vectors to 2048-dim. |
| **PCA fit** | PCA projection computed on a disjoint 20 % partition of the corpus, frozen before MI estimation. |
| **Test split** | The 30 % held-out partition; MI estimates are reported on this split. |
| **Train split** | The 50 % used for MINE / InfoNCE critic training. |
| **Bootstrap CI** | 1000 resamples; 2.5 / 50 / 97.5 percentile reported. |
| **Seed std** | Standard deviation of MI estimates across the 5 random seeds. |
| **DISAGREE_STRONG** | Flag: estimators differ by > 2 bits on the pair. |
| **MINE_NONCONV** | Flag: MINE training loss does not plateau in 2000 steps. |
| **NCE_SAT** | Flag: InfoNCE saturated at $\log K$. |
| **Stratum** | Input category for sampling: code, prose, mixed, short. |
| **Sensitivity corpus** | Non-production corpus (Wikipedia-5k, Code-5k, Shakespeare-2k) used to cross-check the production $N_\text{eff}$. |
| **Compliance gate** | Steve's mandatory review of reporting language against [07_reporting_template.md §12](07_reporting_template.md) rules before any external surface. |
| **Amendment log** | Dated + signed changes to the preregistration after freeze. Lives in [01_preregistration.md](01_preregistration.md) Appendix A. |
| **Freeze date** | 2026-04-29. After this date the preregistration is immutable. |
| **Action #3** | EXP-2 kickoff in the 14-day action list. Day 7 (2026-04-30). Deadline Day 21 (2026-05-14). |

---

## Embedder ID references

| Slot | Name | ModelId variant | Native dim | EXP-2 input dim |
|------|------|-----------------|------------|-----------------|
| E1 | Semantic | `Semantic` | 1024 | 1024 |
| E2 | TemporalRecent | `TemporalRecent` | 512 | 512 |
| E3 | TemporalPeriodic | `TemporalPeriodic` | 512 | 512 |
| E4 | TemporalPositional | `TemporalPositional` | 512 | 512 |
| E5 | Causal | `Causal` | 768 × 2 | 1536 |
| E6 | Sparse | `Sparse` | 30522 | 2048 (after JL) |
| E7 | Code | `Code` | 1536 | 1536 |
| E8 | Graph | `Graph` | 1024 × 2 | 2048 |
| E9 | HDC | `Hdc` | 1024 | 1024 |
| E10 | Contextual | `Contextual` | 768 × 2 | 1536 |
| E11 | Kepler | `Kepler` | 768 | 768 |
| E12 | LateInteraction | `LateInteraction` | 128/token | 128 (pooled) |
| E13 | Splade | `Splade` | 30522 | 2048 (after JL) |

---

## Frozen seeds (from `scripts/exp2/lib/seeds.py`)

| Name | Value | Purpose |
|------|-------|---------|
| `stratify_seed` | 42 | Corpus stratification RNG |
| `sketch_seed` | 2026 | JL sketch matrix for E6 / E13 |
| `split_seed` | 137 | Corpus split into PCA / train / test |
| `mine_seeds` | [0, 1, 2, 3, 4] | MINE critic initialization |
| `infonce_seeds` | [0, 1, 2, 3, 4] | InfoNCE projection init |
| `bootstrap_seed` | 999 | Bootstrap resampling |
| `random_proj_control_seeds` | [9001, 9002] | Random projections in the random-proj control |

---

## Cross-document references

| Symbol or term | Defined in | Used in |
|----------------|------------|---------|
| $N_\text{eff,pair}$ | [06_analysis_and_decision_rules.md §2.1](06_analysis_and_decision_rules.md) | 00, 01, 07, 12, 13 |
| Three controls | [05_methodology.md §2](05_methodology.md) | 01, 07, 10 |
| Preregistration freeze | [01_preregistration.md](01_preregistration.md) | 00, 05, 07, 08, 09, 12 |
| Reporting format | [07_reporting_template.md](07_reporting_template.md) | 00, 05, 09, 10, 12 |
| Pair flag set | [04_estimator_specifications.md §7](04_estimator_specifications.md) | 05, 06, 07 |
| Decision thresholds | [06_analysis_and_decision_rules.md §3](06_analysis_and_decision_rules.md) | 00, 06, 13 |

---

## Notation for Chris's draft-writing

When writing the Perez memo revision or patent disclosure, the following notation shortcuts are allowed:

- "13-embedder panel" — use this in prose; expand to $N = 13$ in equations.
- "constructive ceiling 91" — use bare; expand to $N + \binom{N}{2} = 91$ on first citation per document.
- "realized floor $N_\text{eff}$" — use bare after first citation.
- "$G_\tau$" — render in LaTeX as `$G_\tau$`; in plain prose, write "the $G_\tau$ operator" or "the per-output cosine predicate."
- "the audit" — refers to EXP-2 after first use as "the EXP-2 pairwise MI audit."

When writing for diligence counsel or the patent disclosure, all symbols are expanded to full form at first use per document.
