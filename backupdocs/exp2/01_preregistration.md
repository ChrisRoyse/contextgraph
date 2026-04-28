# EXP-2 Pre-Registration

**Status**: DRAFT. Freezes 2026-04-29 end-of-day. After that date every field below is immutable; any deviation is reported as a protocol amendment, dated, signed by Chris and Steve, and disclosed in the final report.

**Registration identifier**: `exp2-cg-13panel-v1.0`
**Pre-registration hash** (to be inserted on freeze): `sha256:<tbd>`
**Freeze commit tag** (to be set on freeze): `v0.exp2.preregistered`

---

## 1 · Hypotheses

### H0 (null; corresponds to Axiom 2 of the counting identity as stated)
For the 13-embedder production panel $\Phi = \{\phi_1, \dots, \phi_{13}\}$ and the production input distribution $\mathcal{P}$, the pairwise mutual information $I(\phi_j(X); \phi_k(X))$ is sufficiently small relative to per-embedder entropy that the effective signal count $N_\text{eff} \ge 70$.

### H1 (Platonic Representation Hypothesis, Huh et al. 2024)
For the production panel $\Phi$ and distribution $\mathcal{P}$, pairwise MI between learned embedders is high enough that $N_\text{eff} \le 15$.

### H2 (moderate; most likely prior given the panel composition)
$N_\text{eff} \in [25, 50]$. Mixed pairs: some near-reparameterization (shared backbones), some approximately independent (non-learned or cross-family).

### Prior over outcomes (for calibration only; does not affect analysis)
- $P(\text{H0}) = 0.10$ — would surprise us; panel includes E1/E8 weight-sharing which is definitionally high-MI
- $P(\text{H1}) = 0.35$ — consistent with PRH prediction on learned SSL encoders
- $P(\text{H2}) = 0.55$ — most likely; panel mixes E2/E3/E4 (non-learned temporal, expected near-zero-MI-with-content), E9 (HDC, categorically different paradigm), with transformer-family pairs

---

## 2 · Panel (locked)

Exactly the 13 production embedders as defined in `docs2/contextgraphspecs/05_embedding_subsystem.md` section 3 as of commit `<freeze-commit>`. Full specification with dimensions and dual-vector handling in [02_panel_specification.md](02_panel_specification.md).

Every embedder is represented by its **stored fingerprint vector(s)** — the exact tensor that enters production storage, after L2 normalization and after any learned projection. No raw pre-projection outputs are used.

Pairs considered: all $\binom{13}{2} = 78$ ordered-by-index pairs $(j, k)$ with $j < k$, plus 13 diagonal entropy estimates $H(\phi_j(X))$ for normalization.

**Legacy `ModelId::Entity` (384D MiniLM) is excluded.** It is not loaded at runtime. `ModelId::Kepler` represents E11.

**E11 (Kepler) is included** even though `E11_ENTITY_ENABLED = false` at runtime. Rationale: EXP-2 measures structural redundancy, not retrieval activation. Excluding a loaded model would bias the count.

**Asymmetric embedders (E5, E8, E10)**: each contributes **one** $\phi$ per embedder by concatenating the two 768D / 1024D / 768D halves into a single $2 \times d$ vector, L2-normalized as a whole. This preserves the cause/effect, source/target, paraphrase/context asymmetry as part of the signal.

**Sparse embedders (E6, E13)**: converted to dense representations by keeping the full 30 522-dim value vector (values 0 where not active). Sparse top-k pruning is NOT applied for EXP-2, to avoid introducing a retrieval-specific decorrelation.

**Token-level E12 (ColBERT, variable-length)**: pooled to a single 128D vector via mean pool over token vectors, L2-renormalized. Rationale: MI requires fixed-dim input to all three estimators. The pooling loss is reported as a caveat in the final write-up.

---

## 3 · Estimators (locked)

Three estimators run in parallel on the same corpus and same pairs. All hyperparameters frozen here.

### 3.1 MINE (primary) — Belghazi et al. 2018

Critic architecture: 2-layer MLP, width 256, ReLU activations, no dropout, no batch norm. Inputs to the critic are **PCA-reduced to 256 dimensions** per embedder (computed from a disjoint 20 % holdout of the corpus; PCA matrices frozen before MI estimation and serialized with the preregistration).

Objective: Donsker-Varadhan dual
$$\hat{I}_\text{MINE}(X; Y) = \mathbb{E}_{p(x,y)}[T(x,y)] - \log \mathbb{E}_{p(x)p(y)}[\exp T(x,y)]$$

Training: 2 000 gradient steps, Adam ($\text{lr} = 10^{-4}$, $\beta_1 = 0.5$, $\beta_2 = 0.999$), batch size 512, exponential moving average on the denominator log term ($\alpha = 0.01$) to stabilize gradient. EMA smoothing on the reported MI ($\alpha = 0.9$) to report a stabilized steady-state value.

Held-out estimation: after training, $\hat{I}$ is recomputed on a disjoint 30 % test split with the critic frozen. The test-split estimate is the reported number.

Seeds: five independent seeds $\{0, 1, 2, 3, 4\}$, reported as the seed mean with standard deviation.

### 3.2 InfoNCE (backup) — Oord et al. 2018

Symmetric InfoNCE lower bound:
$$\hat{I}_\text{NCE}(X; Y) = \log K + \frac{1}{N} \sum_{i=1}^N \log \frac{\exp f(x_i, y_i)}{\frac{1}{K}\sum_{k=1}^K \exp f(x_i, y_k)}$$

Critic $f(x, y) = \phi^T_X x \cdot \phi^T_Y y / \tau$, where $\phi_X, \phi_Y$ are learned $256 \to 64$ projections, $\tau = 0.1$ (fixed, not tuned).

Training: same 2 000 steps, Adam $\text{lr} = 10^{-4}$, batch $K = 256$ (bound is $\log 256 \approx 5.55$ bits; applicable only when true MI $\le 5$ bits — documented as per-pair caveat). For pairs where InfoNCE saturates at $\log K$, we note the lower-bound saturation and rely on MINE + KSG for the actual magnitude.

Seeds: same five seeds as MINE.

### 3.3 KSG (tertiary) — Kraskov, Stögbauer, Grassberger 2004

k-NN entropy estimator with $k = 3$. Applied to **PCA-reduced 16-dim** inputs (lower dim than MINE/InfoNCE because KSG scales poorly with dimension). Same PCA matrices as MINE, truncated to the top 16 components.

KSG is run on a **fixed subset of 12 pairs** (not all 78), selected **before any MI is computed** by stratified sampling over the panel: 4 "expected-high" (E1–E8, E1–E10, E5–E11, E7–E11), 4 "expected-low" (E2–E1, E3–E5, E4–E7, E9–E11), 4 "mixed" (E1–E5, E7–E8, E6–E13, E10–E11). This stratification is frozen here.

Rationale: KSG on all 78 pairs at all sample sizes is compute-prohibitive and adds no information beyond the sanity check it provides on the 12 representative pairs.

Sample size for KSG: fixed 500 points per pair.

---

## 4 · Corpus and sampling (locked)

**Production distribution $\mathcal{P}$**: the union of user memories currently stored in `CF_FINGERPRINTS`. At time of freeze the count is $M$ stored memories (to be recorded at freeze time).

**Sample size**: three sample sizes are run.
- $n_1 = 100$ — sanity / pilot (~1 GPU-hour end-to-end)
- $n_2 = 500$ — primary (~8 GPU-hours)
- $n_3 = 1000$ — high-resolution (~24 GPU-hours)

All three sizes are reported. The decision rules in [06_analysis_and_decision_rules.md](06_analysis_and_decision_rules.md) apply to $n_3$; $n_1$ and $n_2$ are used only to track the scaling curve and confirm estimator convergence.

**Stratification**: sampled memories must hit four strata with equal probability:
- Code memories (identified by `e7_query_type == Code2Code` tag or by `source_metadata.content_type`)
- Prose / narrative memories
- Mixed (technical prose with code snippets)
- Short-form (< 200 chars; e.g., session notes, chat turns)

Sampling is **without replacement** within a stratum and **stratified-proportional** across strata (target 25 % each; residual filled by over-represented stratum if a stratum is undersized). The stratification seed is frozen here: `stratify_seed = 42`.

**Disjoint splits**:
- 20 % of sample → PCA fitting (fit once, serialized before MI estimation)
- 50 % of sample → MINE / InfoNCE critic training
- 30 % of sample → held-out MI estimation

Split seed: `split_seed = 137`. Frozen.

**Input preprocessing**: memories are passed through the production embedding provider via `ProductionMultiArrayProvider::embed_all_with_metadata`. The returned `MultiArrayEmbeddingOutput` is persisted to a write-once artifact (`exp2_embeddings_<date>.bin`) so all 3 estimators × all 78 pairs × all 5 seeds operate on identical tensors.

---

## 5 · Controls (locked)

### 5.1 Upper-bound control ("copy of E1")

Define $\phi_{copy}(x) := \phi_{E1}(x)$. Run the full MINE/InfoNCE/KSG pipeline on the pair $(\phi_{E1}, \phi_{copy})$. Expected result: $\hat{I} \to H(\phi_{E1}(X))$ (finite entropy of the PCA-reduced E1 vector under the corpus). **Pass criterion**: MINE estimate within 0.5 bits of an explicit entropy estimate $\hat{H}(\phi_{E1}(X))$ obtained via KSG on $\phi_{E1}$ alone.

**Fail action**: if MINE underestimates by > 1 bit on this control, the entire run is halted. Estimator is mis-calibrated.

### 5.2 Lower-bound control (independent random projections)

Define $\phi_{R1}(x) = R_1 \phi_{E1}(x)$ and $\phi_{R2}(x) = R_2 \phi_{E1}(x)$ where $R_1, R_2 \in \mathbb{R}^{256 \times 1024}$ are two **independently seeded** random projections (seeds $\{9001, 9002\}$). Run MINE/InfoNCE/KSG on $(\phi_{R1}, \phi_{R2})$.

**Pass criterion**: $\hat{I} \ge 0.95 \cdot H(\phi_{E1})$ (two invertible projections of the same source carry the same information). This is the **dual** sanity check — confirms the estimator is not artificially deflating MI on dim-matched inputs.

### 5.3 True-zero control (orthogonal sources)

Take the sample of 1000 inputs and split into two halves $A, B$. Define $\phi_{\text{zero},1}(x_A, x_B) := \phi_{E1}(x_A)$ and $\phi_{\text{zero},2}(x_A, x_B) := \phi_{E1}(x_B)$. Run MINE/InfoNCE/KSG.

**Pass criterion**: $\hat{I} \in [-0.1, +0.3]$ bits (unbiased estimator should center at 0 with small positive finite-sample bias). If MINE reports $> 0.5$ bits on this control, run has an estimator-side leak.

All three controls pass **before** any real-panel pair estimate is considered valid. If any control fails, the run is halted, the cause diagnosed and disclosed, and the protocol amended with dated sign-off.

---

## 6 · Analysis (locked)

Full decision rules in [06_analysis_and_decision_rules.md](06_analysis_and_decision_rules.md). Summary:

**Per-pair reported value**: median over 5 seeds of the MINE held-out test-split estimate, with 95 % CI from 1 000-sample bootstrap on the test split, $n_3 = 1000$.

**$N_\text{eff}$ computation**:
$$N_\text{eff} = \frac{\sum_j H(\phi_j)}{\bar{H} + \frac{2}{N} \sum_{j<k} \hat{I}(\phi_j; \phi_k) / \bar{H}}$$
where $\bar{H} = \frac{1}{N}\sum_j H(\phi_j)$ and $N = 13$. The derivation is in [06_analysis_and_decision_rules.md](06_analysis_and_decision_rules.md) §3; this formula is the one frozen here.

**Three alternative aggregations are also reported** (but not the headline number):
1. $N_\text{eff,joint} = H(\Phi) / \bar{H}$ using a block-entropy estimator on the joint 13-tuple
2. $N_\text{eff,min} = N - \max_{j<k} \hat{I}(\phi_j;\phi_k) / \bar{H}$ (conservative)
3. $N_\text{eff,sum} = N - \sum_{j<k} \hat{I}(\phi_j;\phi_k) / \bar{H}$ (not a bound; for comparison)

**Decision thresholds** (headline $N_\text{eff}$ at $n_3$):
- Strong: $N_\text{eff} \ge 70$
- Moderate: $25 \le N_\text{eff} < 70$
- Weak: $N_\text{eff} < 25$

Thresholds frozen. No post-hoc adjustment.

---

## 7 · Reporting (locked format)

Results are written in exactly the format specified in [07_reporting_template.md](07_reporting_template.md). Every pair is reported with its MINE point estimate + 95 % CI + seed std + the InfoNCE lower-bound + KSG value where applicable.

**The constructive ceiling 91 stays in every public artifact.** The realized floor $N_\text{eff}$ accompanies it. Both numbers appear together. This is the "constructive-ceiling-plus-realized-floor" format and is a condition of the pre-registration: reporting only one is a protocol violation.

---

## 8 · Amendments

If anything in §2–§7 changes after the freeze date, the change is recorded as a dated amendment in [Appendix A: Amendment Log](#appendix-a-amendment-log) below with (a) the change, (b) the justification, (c) the date, (d) Chris + Steve signatures (or initials in the commit). Amendments made after a single MI pair estimate is computed on real data are **material** and flagged in the public report.

---

## Appendix A — Amendment Log

*(empty at freeze date; amendments appended here chronologically)*

| Date | Section | Change | Justification | Signed |
|------|---------|--------|---------------|--------|
| — | — | — | — | — |

---

## Appendix B — Commit anchor

On freeze this section is replaced with the SHA of the commit containing the final state of all files under `docs/exp2/`. The final report header quotes that SHA so a reader can independently verify nothing in the protocol moved after freeze.

`<freeze-commit-sha>`
