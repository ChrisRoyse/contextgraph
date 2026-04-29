# EXP-2 Methodology

The math + algorithm choices + hyperparameters. This is the single most important doc to get right; it determines whether the result is publishable.

## 1. The Quantity We Measure

For each pair of embedders $(\phi_j, \phi_k)$ and a corpus $\{x_i\}_{i=1}^n$ of $n=800$ inputs, we estimate the mutual information

$$I(\phi_j(X); \phi_k(X)) = \int p(u, v) \log \frac{p(u, v)}{p(u) p(v)} \, du \, dv$$

where $u = \phi_j(x)$, $v = \phi_k(x)$, and the joint distribution $p(u, v)$ is the empirical distribution induced by the corpus.

**Units**: nats (natural log). All numbers reported in nats. To convert to bits, multiply by $\log_2 e \approx 1.443$.

**Range**: $I \in [0, \min(H(U), H(V))]$. $I = 0$ ⟺ embedders independent. $I$ near $\min(H_j, H_k)$ ⟺ one is deterministic function of the other.

**Why MI rather than correlation**: MI captures *all* statistical dependence, not just linear. Correlation = 0 does not imply independence. For high-dimensional embeddings with potentially nonlinear dependencies, MI is the right notion.

## 2. Why Two Estimators

Both MINE/InfoNCE and KSG have known failure modes:

| Estimator | Failure mode | Detected by |
|---|---|---|
| MINE / InfoNCE | Critic underfits → MI underestimate | Validation MI lower than train MI; non-converging loss |
| MINE / InfoNCE | Critic overfits → MI overestimate (esp. with high-dim, low-sample) | Validation MI gap > 0.1 nats |
| KSG | Curse of dimensionality at d > 32 | MI estimates uniformly low; biased toward 0 |
| KSG | Sensitivity to $k$ choice | Sweep $k \in \{3, 5, 10\}$, report instability |

If both estimators agree, the estimate is robust. If they disagree, we report both and analyze.

## 3. Embedder Dimensional Equalization (Projection Step)

InfoNCE/KSG estimators operate cleanly on bounded-dimensional vectors. The 14 embedders span 12 to 1536 native dimensions plus sparse and token-level. We project everything to **32-D via PCA** *or* keep native if already ≤ 32-D. The PCA models are fit on a 1000-input *separate* training corpus (not the 800-input audit corpus) and frozen for the entire EXP-2 run.

Per-embedder projection table:

| Slot | Native | Method | Result |
|------|--------|--------|--------|
| E1 | 1024 | PCA-32 | 32-D dense |
| E2 | 512 | PCA-32 | 32-D dense |
| E3 | 512 | PCA-32 | 32-D dense |
| E4 | 512 | PCA-32 | 32-D dense |
| E5 cause | 768 | PCA-32 | 32-D dense |
| E6 | 30522 sparse | top-128 weight + L2 norm + PCA-32 | 32-D dense |
| E7 | 1536 | PCA-32 | 32-D dense |
| E8 source | 1024 | PCA-32 | 32-D dense |
| E9 | 1024 (post-projection from 10000-bit) | PCA-32 | 32-D dense |
| E10 paraphrase | 768 | PCA-32 | 32-D dense |
| E11 KEPLER | 768 | PCA-32 | 32-D dense |
| E12 token | 128/token | mean-pool to 128-D + PCA-32 | 32-D dense |
| E13 | 30522 sparse | top-128 weight + L2 norm + PCA-32 | 32-D dense |
| E14 | 1024 | PCA-32 | 32-D dense |

**All MI estimates are computed on these 32-D projections.** The audit chain logs the PCA models' SHA-256, the sparse top-K=128 selection rule, and the random projection matrix for E9.

**Why 32-D?** Empirical sweet spot:
- High enough to retain ≥85% variance (validated per-embedder; if any falls below, increase that one to 64-D).
- Low enough that KSG kNN works without partitioning beyond the partition step.
- Standard MINE practice in the literature.

### 3a. PCA Variance Validation

For each embedder, after PCA-32 fit, log:
- Cumulative explained variance ratio at d=32.
- Effective rank of the original (Schur-complement) at $\epsilon = 0.01$.

Pass criterion: explained variance ≥ 0.85. Fail criterion: a single embedder retains < 0.70 variance — that embedder is upgraded to PCA-64 with audit-chain note.

## 4. InfoNCE / MINE Estimator

We use **InfoNCE bound** (Oord et al. 2018) as the primary estimator. InfoNCE is more stable than MINE in finite-sample regimes and integrates cleanly with batch construction.

### 4a. The InfoNCE Bound

For a batch of $N$ joint samples $\{(u_i, v_i)\}_{i=1}^N$ and a learnable critic $f_\theta : \mathbb{R}^{32} \times \mathbb{R}^{32} \to \mathbb{R}$:

$$\hat I_{\mathrm{InfoNCE}}(\theta) = \log N + \frac{1}{N} \sum_{i=1}^N \log \frac{\exp f_\theta(u_i, v_i)}{\frac{1}{N} \sum_{k=1}^N \exp f_\theta(u_i, v_k)}$$

The estimator is a **lower bound** on $I(U; V)$. Maximizing it over $\theta$ tightens the bound.

### 4b. Critic Architecture

Symmetric two-tower critic:

```
g_u: u → MLP(32 → 64 → 64) → z_u (64-D)
g_v: v → MLP(32 → 64 → 64) → z_v (64-D)
f(u, v) = ⟨z_u, z_v⟩ / sqrt(64)   # scaled inner product
```

LayerNorm after each linear, ReLU activations, fp32 throughout. ~16K parameters. Fits in <1 MB GPU memory; 91 critics fit easily on RTX 5090.

### 4c. Training Procedure

- Optimizer: AdamW, lr=$3 \times 10^{-4}$, weight_decay=0.01, $\beta_2=0.98$.
- Batch size: 128 (positive pairs).
- Negatives: in-batch (each anchor contrasts against the other 127 in the batch).
- Steps: 5,000 (≈ enough for convergence on 800-input dataset).
- Warmup: 200 linear-warmup steps.
- Cosine learning-rate decay.
- Train/val split: 80/20 (640 train, 160 val), fixed seed.
- Early stopping: patience 500 steps on val InfoNCE; best-on-val checkpoint kept.
- Stopping criterion: train-val gap > 0.1 nats → flagged as overfit, run with smaller critic (32 → 32 → 32).

### 4d. Reported Quantity

For each pair, we report the **median val InfoNCE** over the last 1000 steps of training. The median (rather than mean) is robust to spike outliers. The reported MI is this median, *not* the maximum (which over-states due to estimator variance).

### 4e. Bootstrap

After training the critic, resample the 160-val set with replacement 10,000 times, fixed seed 42. For each resample, recompute val InfoNCE on the resampled batch. The 2.5–97.5 percentile is the reported CI.

## 5. Partitioned-KSG Estimator

We use **KSG-1** (Kraskov et al. 2004 with $k=5$) as the secondary estimator. The estimator is non-parametric, has no neural-network training, and serves as a sanity check on InfoNCE.

### 5a. The KSG-1 Estimator

For samples $\{(u_i, v_i)\}_{i=1}^N$ in $\mathbb{R}^{d_u} \times \mathbb{R}^{d_v}$:

1. For each $i$, find the $k$-th nearest neighbor $(u_{i^*}, v_{i^*})$ in joint space, distance $\epsilon_i$ in max-norm.
2. Count $n_u(i)$ = points with $\|u_j - u_i\|_\infty < \epsilon_i$ (excluding $i$).
3. Count $n_v(i)$ = points with $\|v_j - v_i\|_\infty < \epsilon_i$ (excluding $i$).
4. Estimate

$$\hat I = \psi(k) - \frac{1}{N} \sum_i [\psi(n_u(i) + 1) + \psi(n_v(i) + 1)] + \psi(N)$$

where $\psi$ is the digamma function.

### 5b. Why $k = 5$

$k = 5$ is the literature standard. Sensitivity is logged via a sweep $k \in \{3, 5, 10\}$ on a 4-pair pilot before the production run. If pilot disagreement > 0.1 nats across $k$, methodology revisits.

### 5c. Implementation

`scikit-learn` provides KSG via `sklearn.metrics.mutual_info_score` for discrete; for continuous, we use `npeet` (NumPy Entropy Estimation Toolbox, Steeg lab) or implement directly with `sklearn.neighbors.NearestNeighbors`.

Pin to a specific commit hash of the implementation. Log in audit chain.

### 5d. Bootstrap

Resample the 800-input corpus 10,000 times with replacement, fixed seed 42. For each resample, recompute KSG. 2.5–97.5 percentile CI.

## 6. Pre-Registered Hyperparameters

Per Constitution C5, these are committed before the production run:

```yaml
exp2_hyperparameters:
  pca:
    target_dim: 32
    fallback_dim: 64
    variance_threshold: 0.85
  infonce:
    critic_hidden_dim: 64
    critic_depth: 2
    optimizer: AdamW
    learning_rate: 3e-4
    weight_decay: 0.01
    batch_size: 128
    train_steps: 5000
    warmup_steps: 200
    train_val_split: 0.8
    early_stopping_patience: 500
    overfit_gap_threshold: 0.1
  ksg:
    k: 5
    sensitivity_sweep: [3, 5, 10]
    distance_metric: max_norm
    implementation: npeet
  bootstrap:
    n_resamples: 10000
    seed: 42
    method: percentile
    confidence: 0.95
  cross_validation:
    method_disagreement_threshold_nats: 0.15
    convergence_required_pairs: 91
    expected_agreement_rate: 0.80
```

Hash this YAML. Log the hash in every artefact.

## 7. Effective Signal Count $N_{\mathrm{eff}}$

We compute three measures and report all three.

### 7a. Redundancy-Adjusted Count (Primary)

For each pair, define redundancy:

$$r_{jk} = \frac{I(\phi_j; \phi_k)}{\min(H(\phi_j), H(\phi_k))}, \quad r_{jk} \in [0, 1]$$

Then:

$$N_{\mathrm{eff}}^{(1)} = N + \sum_{j < k} (1 - r_{jk})$$

Interpretation: the $N$ marginal embedders contribute $N$ signals; each pair adds a signal proportional to its non-redundancy $1 - r_{jk}$. If all pairs are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$. If all pairs are redundant ($r=1$), $N_{\mathrm{eff}}^{(1)} = 14$.

We need $H(\phi_j)$ for each embedder. We estimate via the same KSG-1 entropy estimator on the 32-D projections, with $k = 5$, on the 800-input corpus.

### 7b. Spectrum Entropy

Construct the symmetric MI matrix $M_{jk} = I(\phi_j; \phi_k)$ for $j \ne k$, $M_{jj} = H(\phi_j)$. Normalize Frobenius: $\tilde M = M / \|M\|_F$.

Compute eigenvalues $\{\lambda_i\}_{i=1}^{14}$. Normalize: $\hat\lambda_i = |\lambda_i| / \sum_j |\lambda_j|$.

Spectral entropy: $H(\hat\lambda) = -\sum_i \hat\lambda_i \log \hat\lambda_i$.

Effective rank: $N_{\mathrm{eff}}^{(2)} = \exp H(\hat\lambda)$.

If all eigenvalues equal: $H = \log N$, $N_{\mathrm{eff}}^{(2)} = N = 14$. If one dominates: $N_{\mathrm{eff}}^{(2)} \to 1$. Higher = more uniform information distribution.

### 7c. Schur-Complement Numerical Rank

Compute the singular values $\sigma_i$ of $M$. Numerical rank at threshold $\epsilon = 0.05 \sigma_{\max}$:

$$N_{\mathrm{eff}}^{(3)} = |\{i : \sigma_i > 0.05 \sigma_{\max}\}|$$

This is integer-valued, $\in \{1, ..., N\}$.

### 7d. Reporting

Headline number = $N_{\mathrm{eff}}^{(1)}$ (the redundancy-adjusted count) with bootstrap CI. The other two are reported in a table for triangulation.

## 8. Bootstrap Strategy For $N_{\mathrm{eff}}$

For each of 10,000 bootstrap resamples (corpus level), recompute the entire MI matrix + entropy estimates + $N_{\mathrm{eff}}^{(1, 2, 3)}$. The resulting distribution of $N_{\mathrm{eff}}^{(1)}$ gives the headline CI.

Compute time: 10,000 × (91 InfoNCE runs + 91 KSG runs + 14 entropy estimates) is too much. Instead:

- Train each InfoNCE critic *once* on the full 800-input corpus.
- For each bootstrap resample, evaluate the (already-trained) critic on the resample. This is fast.
- KSG: re-run from scratch on each bootstrap (cheap, ~1s per pair).

Total bootstrap compute: ~3-4 hours wall-clock.

## 9. Cross-Validation Between InfoNCE And KSG

For each pair, log:
- $\hat I_{\mathrm{IN}}$ (InfoNCE median)
- $\hat I_{\mathrm{KSG}}$ (KSG-1 with $k=5$)
- $\Delta = \hat I_{\mathrm{IN}} - \hat I_{\mathrm{KSG}}$
- Agreement: $|\Delta| \leq 0.15$ nats.

Per Constitution C3 and PRD §9 success metric: ≥80% of pairs (≥73/91) must agree to within 0.15 nats. If the rate falls below, the methodology document is updated and the production run is repeated.

## 10. Why The Headline Number Is Probably Lower Than 91

Three reasons we should expect $N_{\mathrm{eff}}^{(1)} < 91$:

1. **E1 / E10**: both dense semantic encoders trained on overlapping corpora (e5-large-v2 and e5-base-v2). Pairwise MI likely high.
2. **E5 / E1**: E5 is nomic-causal but with overlap on the semantic substrate; pairwise MI > 0 expected.
3. **E13 / E6**: both SPLADE-family sparse-expansion encoders. Pairwise MI likely high.

If $N_{\mathrm{eff}}^{(1)}$ comes in around 30–50, that is **the right answer** for a 14-panel of overlapping-trained embedders. The defense in the paper becomes "DDA produces $N_{\mathrm{eff}} \approx X$ signals where $X = N + \mathrm{partial\ pair\ contribution}$, far above $N = 14$ alone." The framework is *not* invalidated; the upper bound 91 was always honest.

## 11. Sanity Checks (Run Before Production)

Pilot pass on 4 pairs to verify the methodology before paying for the full 91:

- (E1, E1) — should give $\hat I \approx H(E1)$ (auto-MI).
- (E1, random-vector-of-same-dim) — should give $\hat I \approx 0$.
- (E1, E1 + small-Gaussian-noise) — should give $\hat I < H(E1)$ but > 0.
- (E1, E10) — expected high MI per §10.

If any of these fail to behave as expected, the methodology is wrong, the production run is delayed, and the bug is fixed first.

## 12. Validating Against Synthetic Ground Truth

Before running on the production embedders, validate the entire pipeline on synthetic data with known MI:

- Generate $u \sim \mathcal{N}(0, I_{32})$.
- Generate $v = A u + \sqrt{1 - \rho^2} \epsilon$ where $\epsilon \sim \mathcal{N}(0, I_{32})$, $A$ orthogonal, $\rho$ controls dependence.
- Closed-form MI: $I(U; V) = -\frac{1}{2} \log(1 - \rho^2) \cdot \mathrm{rank}$.
- Sweep $\rho \in \{0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99\}$.
- Verify InfoNCE recovers ground truth within 0.1 nats; KSG within 0.15 nats.

Pass criterion for the pipeline: synthetic recovery within tolerance across the full $\rho$ sweep.

## 13. Audit Chain Per Pair Result

Each pair's result row carries:

```json
{
  "embedder_j": "E1_Semantic",
  "embedder_j_weight_sha256": "...",
  "embedder_k": "E10_Multimodal",
  "embedder_k_weight_sha256": "...",
  "pca_j_sha256": "...",
  "pca_k_sha256": "...",
  "corpus_manifest_sha256": "...",
  "infonce_estimate_nats": 0.847,
  "infonce_ci_low": 0.762,
  "infonce_ci_high": 0.931,
  "infonce_train_loss_final": -0.847,
  "infonce_val_loss_final": -0.844,
  "infonce_train_val_gap": 0.003,
  "ksg_estimate_nats": 0.812,
  "ksg_ci_low": 0.734,
  "ksg_ci_high": 0.889,
  "agreement_nats": 0.035,
  "agreement_pass": true,
  "redundancy": 0.514,
  "code_commit_sha": "...",
  "hyperparams_sha256": "...",
  "run_seed": 42,
  "run_timestamp": "2026-04-XX T XX:XX:XX UTC"
}
```

This row is the unit of replicable evidence.

## 14. References

- Belghazi et al. 2018, *MINE: Mutual Information Neural Estimation* — https://arxiv.org/abs/1801.04062
- Oord et al. 2018, *Representation Learning with Contrastive Predictive Coding* (InfoNCE) — https://arxiv.org/abs/1807.03748
- Kraskov, Stögbauer, Grassberger 2004, *Estimating mutual information* — https://arxiv.org/abs/cond-mat/0305641
- Khan, Bandyopadhyay, Ganguly 2007, *Relative performance of mutual information estimation methods* — Phys Rev E.
- Steeg & Galstyan 2014, *Demystifying Information-Theoretic Clustering* (NPEET reference).
- Poole et al. 2019, *On Variational Bounds of Mutual Information* — https://arxiv.org/abs/1905.06922 (covers InfoNCE / Donsker-Varadhan tradeoffs).
