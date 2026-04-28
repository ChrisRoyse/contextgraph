# EXP-2 Analysis and Decision Rules

**How raw MI estimates become a decision.** Every rule here is pre-registered in [01_preregistration.md](01_preregistration.md). Nothing below moves after the freeze date.

---

## 1 · The counting identity, restated in information-theoretic form

The original (pre-EXP-2) claim:

> At $N = 13$ embedders with $n$ inputs each producing $N + \binom{N}{2}$ signals per input, the DDA system emits *up to* $n \cdot (N + \binom{N}{2}) = n \cdot (13 + 78) = n \cdot 91$ supervisory signals total.

The "91 per input" is a **structural ceiling**: 13 diagonal (per-embedder) signals + 78 off-diagonal (per-pair-cross-correlation) signals. Axiom 2 (approximate embedder independence) is what makes the 78 term non-trivially additive.

**Information-theoretic restatement.** Each embedder $\phi_j$ contributes $H(\phi_j(X))$ bits of information about input $X$. The joint panel contributes at most $H(\Phi(X)) = H(\phi_1(X), \dots, \phi_{13}(X))$ bits. The gap
$$R = \sum_{j=1}^{13} H(\phi_j(X)) - H(\Phi(X))$$
is the **total redundancy**. By the chain rule and pair-decomposition, $R$ is upper-bounded by $\sum_{j < k} I(\phi_j(X); \phi_k(X))$ (Han 1978 / Watanabe 1960 decomposition; see [11_prior_art_and_citations.md](11_prior_art_and_citations.md)).

If all pairs are independent ($I = 0$), then $R = 0$ and the 91 signals count as distinct. If pairs are fully redundant ($I = H$), then $R$ is maximal and most of the 91 signals are duplicates of existing ones.

---

## 2 · Three aggregation formulas (all reported)

Let $\bar{H} = \frac{1}{13}\sum_j H(\phi_j(X))$ (mean per-embedder entropy, KSG-estimated on PCA-16 representation per [04_estimator_specifications.md](04_estimator_specifications.md) §4). Let $\bar{I} = \frac{1}{78}\sum_{j<k} \hat{I}(\phi_j; \phi_k)$ (mean pairwise MI).

### 2.1 Pair-sum effective N (headline)

$$N_\text{eff,pair} = \frac{\sum_j H(\phi_j(X))}{\bar{H} + (2/N) \cdot \sum_{j<k} \hat{I}(\phi_j; \phi_k) / \bar{H}}$$

This is the pair-decomposed analog to the original 91 identity. Denominator terms: the **per-embedder cost** ($\bar{H}$) plus the average **pair-redundancy fraction** (second term). It reduces to $N = 13$ when all $\hat{I} = 0$ (no redundancy) and to $< 1$ when all pairs are fully redundant ($\hat{I} = H$).

**This is the headline $N_\text{eff}$.** All decision thresholds apply to this quantity.

### 2.2 Joint-entropy effective N (alternative, tighter)

$$N_\text{eff,joint} = \frac{H(\Phi(X))}{\bar{H}}$$

where $H(\Phi)$ is the joint entropy of the 13-tuple. Reported as a sanity check. Joint entropy is harder to estimate than pair MI; we use a **block-entropy estimator** (Paninski 2003 / MLE with Chao-Shen correction) on the PCA-16-concatenated representation ($13 \times 16 = 208$-dim; NN-based since direct histogram is infeasible at this dim).

Expected relationship: $N_\text{eff,joint} \le N_\text{eff,pair}$ (joint captures higher-order dependencies that pairwise misses).

### 2.3 Conservative and optimistic bounds

$N_\text{eff,min} = 13 - \max_{j<k} \hat{I}(\phi_j; \phi_k) / \bar{H}$

$N_\text{eff,sum} = 13 - \sum_{j<k} \hat{I}(\phi_j; \phi_k) / \bar{H}$

$N_\text{eff,min}$: the single most redundant pair. Conservative in the sense that it reports the worst-case redundancy but ignores the $+78$ pair signals entirely.

$N_\text{eff,sum}$: straight additive, NOT a bound — may go negative if redundancy exceeds 13$\bar{H}$. Reported only for comparison.

---

## 3 · Decision thresholds (pre-registered, on $N_\text{eff,pair}$ at $n = 1000$)

| Outcome | $N_\text{eff,pair}$ | Interpretation | Narrative |
|---------|---------------------|----------------|-----------|
| **Strong** | $\ge 70$ | Pair independence holds; Axiom 2 approximately true | Counting identity's 91 ceiling is empirically supported; realized floor clears 77 % of constructive |
| **Moderate** | $25 \le \cdot < 70$ | Substantial unique signal, substantial redundancy | DDA delivers 2×–5× more supervisory signals than single-encoder; extensibility claim holds |
| **Weak** | $< 25$ | PRH-consistent: encoders converge | Thesis reframes from absolute 91 to three-role composition + growth curve |

These thresholds are **chosen ex ante** based on:
- **$N_\text{eff} \ge 70$** corresponds to ~85 % pair-independence (realized floor = ceiling × 0.85).
- **$N_\text{eff} \ge 25$** is roughly twice the single-encoder baseline (13), which is the minimum commercial defensibility bar for DDA.
- **$N_\text{eff} < 25$** is the PRH-predicted zone where encoder convergence dominates.

---

## 4 · Boot-strap-adjusted thresholds

Because the point estimate is noisy, decisions use the **95 % bootstrap CI** on $N_\text{eff,pair}$:

| Outcome | CI lower bound | Reporting language |
|---------|----------------|--------------------|
| Strong | $\ge 70$ | "Measured $N_\text{eff} = \hat{N}$ (95 % CI [$L$, $U$]); panel independence confirmed within the bounds of MINE / InfoNCE / KSG agreement." |
| Moderate | 25 ≤ lower bound < 70 | "Measured $N_\text{eff} = \hat{N}$ (95 % CI [$L$, $U$]); panel yields $N_\text{eff}$-fold supervisory multiplication over single-encoder baseline (N = 1)." |
| Weak | < 25 | "Measured $N_\text{eff} = \hat{N}$ (95 % CI [$L$, $U$]). The structural ceiling of 91 is not realized on the measured distribution; the pair term is compressed by embedder convergence. The counting identity remains a valid structural upper bound and the growth rate with respect to $N$ is still super-linear; the headline shifts from absolute count to substrate-level composition." |

**The CI lower bound is what drives the decision.** If $\hat{N}_\text{eff} = 28$ but the 95 % lower CI is 18, the decision is "Weak" (the data does not rule out Weak).

**Exception for Strong**: we require the CI lower bound to clear the threshold. If $\hat{N}_\text{eff} = 72$ but the lower CI is 58, the decision is "Moderate" (not strong enough to claim the ceiling).

---

## 5 · Within-estimator agreement gate

The decision above applies only if:
- MINE and InfoNCE medians agree within 2 bits on at least 60 out of 78 pairs (77 %)
- KSG medians agree with MINE within 2 bits on at least 9 of 12 sampled pairs (75 %)
- At most 5 out of 78 pairs carry the `DISAGREE_STRONG` flag

If any of these fail, the headline decision is replaced with **"Estimator disagreement; results not published without amendment"** and the protocol is amended with dated sign-off before any external surface.

---

## 6 · Sensitivity analyses (do not change decision, but accompany headline)

At the same $n = 1000$, $N_\text{eff,pair}$ is also computed on the three sensitivity corpora:

- `N_eff_wikipedia5k`
- `N_eff_code5k`
- `N_eff_shakespeare2k`

These are reported **alongside** the production estimate. If any sensitivity $N_\text{eff}$ differs from production by more than 15, it is flagged and discussed (does not change the production decision).

Interpretation framework:
- If production $N_\text{eff}$ is substantially higher than Wikipedia $N_\text{eff}$: production distribution has more unique signals per input, which is consistent with the multi-domain mixing in the Context Graph corpus.
- If production $N_\text{eff}$ is substantially lower than Wikipedia $N_\text{eff}$: production distribution has stronger embedder correlations, possibly due to session-level content repetition.
- If the three sensitivity $N_\text{eff}$ are all in the same band (within 10 of each other): the estimate is distribution-robust.

---

## 7 · Per-pair interpretation heat-map

The full $13 \times 13$ MI matrix is reported as a heatmap (see [07_reporting_template.md](07_reporting_template.md) §4.2). Diagonal cells show $H(\phi_j)$. Off-diagonals show $\hat{I}(\phi_j; \phi_k)$.

Expected high-MI cells (from [02_panel_specification.md](02_panel_specification.md) §2):
- E1–E8 (shared backbone; weight-sharing ceiling)
- E1–E10, E8–E10 (e5 family)
- E6–E13 (twin SPLADE)
- E2–E3, E2–E4, E3–E4 (timestamp functions)

Expected low-MI cells:
- (E2, E3, E4) × (E1, E5, E7, E8, E9, E10, E11) — temporal vs content
- E9 × learned embedders — HDC vs neural

**If a high-expected pair shows low MI**: evidence of decorrelation mechanism we didn't anticipate — surprising. Flag for deeper analysis.

**If a low-expected pair shows high MI**: evidence of unexpected redundancy — flag for PRH-style discussion.

---

## 8 · Additional reported numbers

For the final report, we also compute and report:

### 8.1 Redundancy fraction

$\rho = R / \sum_j H(\phi_j) = 1 - H(\Phi) / \sum_j H(\phi_j)$

Ranges $[0, 1-1/N]$. $\rho = 0$ fully independent; $\rho$ near $12/13 \approx 0.92$ is fully collapsed to a single signal.

### 8.2 Effective dimensionality (spectral)

PCA on the concatenated 13-tuple representation. Number of components needed to capture 95 % of the variance. Complements the information-theoretic $N_\text{eff}$ with a geometric view.

### 8.3 Cluster of near-reparameterizations

Graph over the 13 embedders with edge weight $\hat{I}(\phi_j; \phi_k) / \min(H(\phi_j), H(\phi_k))$. Threshold at 0.8. Connected components = "equivalence classes" where within-class embedders are near-reparameterizations of each other. Reported as a categorical finding.

### 8.4 Growth-rate estimate for future embedders

The counting identity's 91-per-input grows as $\binom{N+1}{2} - \binom{N}{2} = N$ new pair signals per new embedder added. Even under PRH-style partial convergence, the growth rate is $O(N)$. We report the expected $N_\text{eff}$ increment when adding a simulated $\phi_{14}$ at different redundancy levels (0.3, 0.5, 0.7 of average pair MI).

This quantifies the extensibility claim from the WINNING_PLAYBOOK §4.4: "even a moderate result keeps the thesis alive because the growth curve with $N$ remains substantively unbounded."

---

## 9 · Binding commitment

Chris and Steve pre-commit to the following:

1. **The decision rule is not tuned by the data.** If the first $n=1000$ run produces $\hat{N}_\text{eff} = 24.7$ with lower CI 19, we report it as "Weak" — we do not re-run on a different corpus to find a higher number.
2. **Every sensitivity corpus is reported**, not the most favorable one.
3. **Every flag fires in the public report.** `DISAGREE_STRONG` pairs are listed by name with their three estimator values side by side.
4. **Amendment log is always visible.** Any post-freeze change appears in [01_preregistration.md Appendix A](01_preregistration.md#appendix-a-amendment-log) before the final report is released.
5. **Realized floor and constructive ceiling accompany each other.** We never publish $N_\text{eff}$ without the 91 ceiling context; we never publish 91 without the $N_\text{eff}$ context.

---

## 10 · Worked example (synthetic)

To illustrate the decision path, here is a synthetic run (not a real EXP-2 result):

```
At n = 1000:
  13 diagonal H estimates (PCA-16, KSG):
    [8.2, 3.1, 3.4, 3.0, 7.5, 6.8, 7.9, 8.1, 7.2, 7.3, 6.9, 5.5, 6.7]
  H_bar = 6.28 bits

  78 pair MI estimates (MINE median over 5 seeds):
    max = 6.9 (E1-E8, as predicted)
    min = 0.04 (E2-E9)
    mean I_bar = 1.8

  N_eff,pair = sum_H / (H_bar + (2/13) * sum_I / H_bar)
             = 81.6 / (6.28 + (2/13) * (1.8 * 78) / 6.28)
             = 81.6 / (6.28 + 3.44)
             = 81.6 / 9.72
             = 8.39 * 13 / 13 scaling ... wait, let me redo

  Actual formula: N_eff,pair = sum_H / (H_bar + (2/N)*sum_I/H_bar)
             sum_H = 81.6
             H_bar = 6.28
             sum_I = 140.4
             denominator = 6.28 + (2/13)*(140.4/6.28) = 6.28 + 3.44 = 9.72
             N_eff,pair = 81.6 / 9.72 = 8.4

  N_eff,joint (via block-entropy on concatenated PCA-16):
             = H_joint / H_bar = 24.3 / 6.28 = 3.87

  N_eff,min = 13 - 6.9/6.28 = 11.9
  N_eff,sum = 13 - 140.4/6.28 = -9.4 (not a bound; for reference)

  Bootstrap 95% CI on N_eff,pair: [6.2, 10.8]

  Decision: Weak (lower CI = 6.2 < 25).
  Narrative: per §4 Weak row.
```

This is a **hypothetical Weak outcome**. Planned response: reframe the pitch around extensibility ($O(N)$ new signals per new embedder) and three-role composition, not the 91 headline. The constructive 91 remains quoted as a ceiling.

---

## 11 · What we will NOT do with the numbers

- We will **not** re-weight pairs post-hoc to boost $N_\text{eff}$.
- We will **not** select the sample size that gives the largest $N_\text{eff}$; the headline is always $n = 1000$.
- We will **not** drop estimators that disagree; we report all three.
- We will **not** exclude the E1-E8 shared-backbone pair to improve the headline — this pair is by construction high-MI and reporting it is the honesty signal.
- We will **not** alter Axiom 2 language after seeing the result; the axiom stays as originally framed, and the data is reported against it.
