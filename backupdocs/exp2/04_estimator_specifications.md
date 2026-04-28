# EXP-2 Estimator Specifications

**Three estimators. Full math. Frozen hyperparameters.** No estimator is tuned on EXP-2 data — all choices come from the published references or standard defaults.

---

## 1 · Why three estimators

MI is a notoriously hard functional to estimate in high dimensions. Every estimator has failure modes:

| Estimator | Known failure mode |
|-----------|--------------------|
| **MINE** | Variance explosion on heavy-tail ratios; overestimates on deterministic near-copies |
| **InfoNCE** | Lower bound only; saturates at $\log K$ bits; underestimates for high-MI pairs |
| **KSG** | Dimension curse; biased for $d > 20$; sensitive to outliers |

**Running all three on the same panel is the anti-cherry-pick defense.** If all three agree within 1 bit, we trust the number. If they disagree, we report all three and the disagreement is the finding.

Precedent: Poole et al. 2019 (*On Variational Bounds of Mutual Information*, ICML) and McAllester & Stratos 2018 document the theoretical bounds and failure modes. Huh et al. 2024 (PRH) uses CKA as a representation-similarity proxy that is not calibrated to bits; EXP-2 explicitly computes bits.

---

## 2 · MINE — Mutual Information Neural Estimator (primary)

**Reference**: Belghazi et al. 2018, *Mutual Information Neural Estimation*, arXiv:1801.04062.

### 2.1 Objective

Donsker-Varadhan representation of KL divergence:
$$D_{KL}(P \| Q) = \sup_T \{\mathbb{E}_P[T] - \log \mathbb{E}_Q[e^T]\}$$

Applied to $P = p(x, y)$ (joint) and $Q = p(x)p(y)$ (product of marginals):
$$\hat{I}_\text{MINE}(X; Y) = \mathbb{E}_{p(x,y)}[T(x,y)] - \log \mathbb{E}_{p(x)p(y)}[\exp T(x,y)]$$

Positive samples $(x, y) \sim p(x, y)$ come from paired embeddings of the same input. Negative samples $(x, y') \sim p(x)p(y)$ are constructed by shuffling $y$ within the batch.

### 2.2 Critic architecture (frozen)

```
T(x, y): R^{256} x R^{256} -> R
    concat x, y              -> R^512
    Linear(512, 256) + ReLU  -> R^256
    Linear(256, 256) + ReLU  -> R^256
    Linear(256, 1)           -> R^1
```

Two hidden layers. Width 256. ReLU. No dropout. No batch norm. Scalar output. **Not tuned.** This is the Belghazi default architecture; departing would be ad-hoc.

### 2.3 PCA preprocessing

Input to the critic is each embedder PCA-reduced to **256 dimensions** from its native dim (after dual-concat / JL sketch; see [02_panel_specification.md](02_panel_specification.md)). PCA matrices are:
- Fit on the 20 % PCA partition of the corpus, **once**, at corpus-build time
- Serialized with frozen seeds
- Loaded read-only during MI estimation

If a pair of embedders has native dim $\le 256$ after the preprocessing in panel spec §3 (only E11 at 768 reduces trivially to 256; E12 at 128 is padded with zeros or run at 128 — **frozen choice: run at native 128, critic input is 128+256=384 for E12-X pairs**).

*Amendment-locked decision*: E12 runs at 128D through the critic. All other embedders run at 256D after PCA.

### 2.4 Training

| Hyperparameter | Value | Source |
|----------------|-------|--------|
| Gradient steps | 2000 | Belghazi default |
| Batch size | 512 | Belghazi |
| Optimizer | Adam | Belghazi |
| Learning rate | $10^{-4}$ | Belghazi |
| $\beta_1, \beta_2$ | 0.5, 0.999 | Belghazi |
| Log-denom EMA coefficient $\alpha$ | 0.01 | Belghazi (gradient bias correction) |
| Reported-MI EMA coefficient | 0.9 | Smooths steady-state estimate |
| Seeds | {0, 1, 2, 3, 4} | Frozen |

### 2.5 Held-out estimation

After 2000 training steps:
1. Freeze critic weights.
2. Iterate over the 30 % held-out test split.
3. For each test batch: sample $B$ positives from the true pair, and construct $B \times B$ cross-products as negatives.
4. Compute $\hat{I}$ on the test split.

The test-split $\hat{I}$ is the reported value. Training-split values are logged but not reported.

### 2.6 Convergence diagnostics

For each pair × seed, save:
- Training-loss curve (negative of the MINE objective) vs. step
- Test-split $\hat{I}$ evaluated every 200 steps
- Gradient norm history

If the training loss does **not** plateau in the final 500 steps (defined as: mean over last 500 > mean over steps 1000–1500 + 0.5 bits), flag the pair as **non-converged** and re-run with 4000 steps. If still non-converged, report the 2000-step value + a non-convergence flag.

### 2.7 Known MINE failure modes and mitigations

| Failure | Mitigation |
|---------|-----------|
| Critic saturates (reports very-high MI that is just a noisy optimization artifact) | Seed-std reporting; if seed std > 1 bit, flag |
| Overestimate on near-deterministic pairs | Cross-check with KSG on sampled pairs |
| Underestimate when batch size is small relative to true MI | Batch size 512 is large enough for MI up to ~10 bits |
| Instability from exponential term | EMA denominator correction (coefficient 0.01) |

---

## 3 · InfoNCE — Oord et al. 2018 (backup, lower bound)

**Reference**: van den Oord, Li, Vinyals 2018, *Representation Learning with Contrastive Predictive Coding*, arXiv:1807.03748.

### 3.1 Objective

Symmetric InfoNCE lower bound on $I(X; Y)$:
$$\hat{I}_\text{NCE}(X; Y) = \log K + \frac{1}{N} \sum_{i=1}^N \log \frac{\exp f(x_i, y_i) / \tau}{\frac{1}{K} \sum_{k=1}^K \exp f(x_i, y_k) / \tau}$$

where $K$ is the batch size and $\tau$ is temperature. The bound is **tight** as $K \to \infty$ for bounded MI; bounded above by $\log K$ bits.

At $K = 256$, the bound saturates at $\log 256 = 5.55$ bits. **Pairs with true MI > 5 bits will show as "InfoNCE $\approx 5.55$" — saturation is a signal, not a bug.**

### 3.2 Critic architecture (frozen)

Lightweight bilinear form:
$$f(x, y) = (W_X x)^\top (W_Y y)$$

where $W_X, W_Y \in \mathbb{R}^{64 \times 256}$ are learned projections. Total parameters: $2 \times 64 \times 256 = 32\,768$.

Lightness is intentional: InfoNCE with a strong critic becomes MINE; keeping it linear-ish preserves InfoNCE's role as an independent estimator.

### 3.3 Training

| Hyperparameter | Value |
|----------------|-------|
| Gradient steps | 2000 |
| Batch size $K$ | 256 |
| Temperature $\tau$ | 0.1 (fixed, not tuned) |
| Optimizer | Adam |
| Learning rate | $10^{-4}$ |
| Seeds | {0, 1, 2, 3, 4} (same as MINE) |

### 3.4 Reporting

Test-split $\hat{I}_\text{NCE}$ reported with the caveat that values near 5.5 are lower-bound saturations, not true values.

For pairs where $\hat{I}_\text{NCE} \ge 5.0$ bits, the **MINE + KSG** values are the more reliable magnitude estimate.

---

## 4 · KSG — k-Nearest-Neighbor (tertiary non-parametric cross-check)

**Reference**: Kraskov, Stögbauer, Grassberger 2004, *Estimating mutual information*, Physical Review E 69(6).

### 4.1 Algorithm 1 (used)

For samples $\{(x_i, y_i)\}_{i=1}^N$:
1. For each $i$, find its $k$-th nearest neighbor in joint space $(x, y)$; let $\epsilon_i$ = distance to that neighbor in the max norm.
2. Count $n_x(i)$ = number of points with $\|x_j - x_i\|_\infty < \epsilon_i / 2$ and similarly $n_y(i)$.
3. Estimator:
$$\hat{I}_\text{KSG}(X; Y) = \psi(k) - \langle \psi(n_x+1) + \psi(n_y+1) \rangle + \psi(N)$$
where $\psi$ is the digamma function and $\langle \cdot \rangle$ is sample mean.

### 4.2 Hyperparameters (frozen)

| Parameter | Value |
|-----------|-------|
| $k$ (neighbor count) | 3 |
| Norm | max (Chebyshev) |
| Sample size per pair | 500 |
| Preprocessing | PCA to 16 dimensions (from panel-spec PCA, top-16 components) |

### 4.3 Pair selection (KSG runs on 12 of 78)

KSG scales as $O(N d \log N)$ with an additional constant hidden by the digamma terms; running on all 78 pairs at $d=16, N=500$ is feasible but noisy. The stronger play is to run KSG on a **stratified 12 pairs** chosen **before the corpus exists**.

Frozen selection (see [01_preregistration.md](01_preregistration.md) §3.3):

**Expected-high-MI (4 pairs)**:
- (E1, E8) — shared e5-large-v2 backbone
- (E1, E10) — same family
- (E2, E3) — both timestamp functions
- (E6, E13) — both SPLADE

**Expected-low-MI (4 pairs)**:
- (E2, E1) — non-learned time vs learned content
- (E3, E5) — time vs content
- (E4, E7) — position vs code
- (E9, E11) — HDC vs neural entity

**Mixed / informative (4 pairs)**:
- (E1, E5) — learned retrieval vs causal
- (E7, E8) — code vs graph
- (E10, E11) — paraphrase vs entity
- (E5, E12) — causal vs pooled ColBERT

### 4.4 KSG failure modes

- **Curse of dimensionality**: KSG is reliable to $d \approx 20$; we PCA-truncate to 16. Reducing further loses signal; higher inflates noise.
- **Outlier sensitivity**: a single extreme point inflates $\epsilon_i$. Mitigated by sample size 500 (each pair ≈ 60 MB of neighbor distance work).
- **Negative estimates**: KSG is unbiased but not non-negative; values < 0 are reported as 0 with a note.

### 4.5 Bootstrap CI on KSG

1000 bootstrap samples (with replacement) over the 500-point pair sample. Report 2.5th / 50th / 97.5th percentiles.

---

## 5 · Bootstrap confidence intervals (all three estimators)

For every pair × estimator × sample size:
- 1000 bootstrap resamples on the test split
- Report 2.5, 50, 97.5 percentiles as [low, median, high]
- Report seed std across the 5 seeds

**The reported "CI" is the max of (bootstrap CI, seed-std CI)** — whichever is wider, to be conservative about uncertainty.

---

## 6 · Estimator agreement criterion

For every pair, we compute:
$$\text{disagree}(j, k) = \max(\hat{I}_\text{MINE}, \hat{I}_\text{NCE}, \hat{I}_\text{KSG}) - \min(\hat{I}_\text{MINE}, \hat{I}_\text{NCE}, \hat{I}_\text{KSG})$$

where InfoNCE is dropped from comparison when it saturates ($\hat{I}_\text{NCE} \ge 0.9 \cdot \log K$), and KSG only contributes for its 12 sampled pairs.

- **Agreement** (disagreement $\le 1.0$ bit): report median as the point estimate
- **Mild disagreement** (1.0 – 2.0 bits): report all three, flag as MILD
- **Strong disagreement** (> 2.0 bits): report all three, flag as STRONG; trigger a manual review before publication

**> 20 % of pairs showing STRONG disagreement is a halt condition.** Halt, diagnose, amend protocol with dated sign-off, rerun affected pairs.

---

## 7 · Sentinel / saturation flags

Every reported pair-estimate carries one or more flags:

| Flag | Meaning |
|------|---------|
| `OK` | All three agree within 1 bit; no issues |
| `NCE_SAT` | InfoNCE saturated at $\log K$; use MINE + KSG for magnitude |
| `MINE_NONCONV` | MINE training loss did not plateau in 2000 steps |
| `SEED_VAR_HIGH` | MINE seed-std > 1 bit |
| `DISAGREE_MILD` | Estimator max-min 1.0–2.0 bits |
| `DISAGREE_STRONG` | Estimator max-min > 2.0 bits |
| `BOOTSTRAP_WIDE` | Bootstrap 95% CI > 2.0 bits wide |
| `CONTROL_DERIVED` | Entry is a control result (E1-copy, random-proj, zero) |
| `KSG_ONLY` | Only KSG was run on this pair (not 78-pair MINE/InfoNCE sweep) |

Every pair in the final report shows its flag set. Clean panels are all `OK`; honest panels include the flags that fired.

---

## 8 · Why NOT CKA / RSA / cosine

Centered Kernel Alignment (CKA; Kornblith et al. 2019) and Representational Similarity Analysis (RSA; Kriegeskorte 2008) are widely used to compare representations and are what Huh et al. 2024 use for PRH. CKA is NOT a mutual-information estimate — it is a normalized cross-correlation on feature maps.

**EXP-2 reports MI in bits, not similarity in [0, 1].** Reasons:
1. MI is **additive** across independent sources; similarity is not. The counting identity requires additive decomposition.
2. MI is **invariant** to invertible transformations; CKA is not.
3. CKA saturates at 1.0 and loses resolution when two representations are near-identical; MI diverges to $H$, which is the quantity we actually care about.

CKA is reported as a **sanity secondary** in the final write-up ([07_reporting_template.md](07_reporting_template.md) §5), not as a substitute for MI.

Cosine similarity between paired embeddings is also reported as a triple-check, but not as a primary estimator — cosine bottlenecks at 1.0 and conflates magnitude with direction.
