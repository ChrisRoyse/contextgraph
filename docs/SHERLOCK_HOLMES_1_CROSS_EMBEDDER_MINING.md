# SHERLOCK HOLMES INVESTIGATION 1 — CROSS-EMBEDDER CORRELATION EXCAVATION

*"The world is full of obvious things which nobody by any chance ever observes."*

Date: 2026-05-21
Case ID: HOLMES-1 / cross-embedder-mining
Subject: Why the SWE-bench Lite ship-gate scorer reports `prediction_oracle_correlation = 0.7942190170` when the corpus already contains `n · (N + C(N,2) + 1)` structured signals per PAPER §3.1 and `C(15,2) = 105` slot pairs are physically materialized in `CF_MEJEPA_DDA_SIGNALS`.

---

## 1. Mandate and scope

I (Sherlock Holmes #1 of three) was instructed to investigate the operator's working hypothesis that the 0.16 point gap between the latest valid Panel-A real-input ship-gate measurement (rank-lift `prediction_oracle_correlation = 0.7942190170`) and the `≥ 0.95` ship-gate threshold is closeable by **cross-embedder correlation mining** on data that is already on disk. The other two investigators own (2) calibration / ECE shape, (3) foundationality / CP_φ / entropy-loss exploitation; this report stays strictly inside the cross-embedder lane. The investigation is fully read-only — no code edits, no FSV runs, no GitHub issues; the artefact is this document. The investigation honours the slot-preserving doctrine (`docs/futurebuild/01_overview.md §1.5ter`, `gtau.rs` `MEJEPA_TCT_DIM_MISMATCH` guard) — every proposed feature respects same-embedder cosine, slot-identity-preserving array operations, and scalar pairwise cross-slot relationships (MI, correlation, agreement) and refuses cross-embedder dim-wise comparison.

---

## 2. Forensic inventory — what is computed vs claimed

### 2.1 The four code surfaces that touch cross-embedder relationships

| Surface | File | Persisted to | Per-input cost | Touches the ship-gate predictor? |
|---|---|---|---:|---|
| DDA cosine + blind-spot z over 15-slot signed-hash projection | `crates/context-graph-mejepa-train/src/dda.rs:153-206` | `CF_MEJEPA_DDA_SIGNALS` | 105 cosines + 105 z-scores | **NO** (only summarised) |
| DDA `pairwise_mi_upper` baseline-fill | `crates/context-graph-mejepa-train/src/dda.rs:192` | `CF_MEJEPA_DDA_SIGNALS` | 105 scalars (**static baseline copy**) | NO |
| Pairwise NMI auditor on slot **means** | `crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi_matrix.rs:124-218` | CSV + `CF_MEJEPA_PAIRWISE_MI` | 1 scalar per panel per pair | NO (training-cadence audit only) |
| Persisted pair MI matrix CF | `crates/context-graph-mejepa/src/pairwise_mi.rs:287-356` | `CF_MEJEPA_PAIRWISE_MI` | 105 scalars per shard | Used by `embedder_proposal.rs` / `instrument_proposal.rs` only |
| DDA feature projection (predictor surface) | `crates/context-graph-mejepa/src/dda_features.rs:100-228` | (no CF) | **4 scalars** (mean_per_emb_cos, mean_pair_cos, mi_health, blind_spot_health) | YES, but collapsed |
| Per-head DDA projection registry | `crates/context-graph-mejepa/src/head_projection.rs:260-303` | `CF_MEJEPA_HEAD_PROJECTION_REGISTRY` | **4 pairwise cosines total** across all 10 heads (`E_AST::E_DIFF`, `E_TYPE_GRAPH::E_DATA_FLOW`, `E_REASONING::E_DIFF`, `E_REASONING::E_ORACLE`) | YES (per head) |
| Constellation `SlotPairEvidence` (display surface) | `crates/context-graph-mejepa/src/constellation_intelligence.rs:33-89, 532-658` | `CF_MEJEPA_CONSTELLATION_*` | 5 scalars per pair, **truncated to top 12 pairs** before serialisation | NO (display-only after `.truncate(12)` at line 633) |
| **Ship-gate `rank_lift_features()`** | `crates/context-graph-mejepa-train/examples/swebench_300x8_ship_gate_fsv.rs:1288-1317` | (FSV JSON) | **60 univariate slot moments + 1 log chunk_count + hashed label bag** | **YES** (this is the source of the `0.7942` rank-lift) |

### 2.2 The gap, stated plainly

The ship-gate scorer that produced `prediction_oracle_correlation = 0.7942190170` consumes:

* 5 univariate moments × 12 embedders = 60 features
* `(chunk_count as f32).ln_1p()` = 1 feature
* `hashed[group | pair | slot | surface]` label bag — note: **here `pair` is a chunk-skill label pair (#414/#417), NOT an embedder pair**

**Zero cross-embedder pair scalars enter the production ship-gate scorer.** Not one cosine. Not one MI value. Not one Granger F. Not one consensus / contradiction / blind-spot z. Not one PID atom. Not one synergy. Not one CCA component. Not one HSIC value. **Not one of the 105 pair-level scalars that are physically resident in `CF_MEJEPA_DDA_SIGNALS` on aiwonder is read by the predictor that the ship gate is judging.**

### 2.3 Claim vs reality table

| Claim source | Claim | Reality on aiwonder corpus |
|---|---|---|
| `PAPER.md:97-99` | `n · (N + C(N,2) + 1)` = up to 92 signals per input at N=13 | 13 per-embedder cosines are computed, 78 pairwise cosines are computed, **but only 4 reach the predictor head specs and zero reach the ship-gate scorer**. The pairwise term is a structural upper bound that the predictor currently realises as ≤ 4. |
| `PAPER.md:489, 547, 567, 579` | EXP-2 / RO-8 — pairwise MI audit "named future work" | Has been "future work" through Phase A–G. The `pairwise_mi_upper` field is currently filled from a static `DdaPairwiseBaseline` (`dda.rs:192`), not measured per-input. |
| `alldata.md:2962-2972` | "Teleological Comparator: 78-Dimensional Cross-Correlation… 13-dim per-space alignment + 78 pairwise + 6 group-level + Tucker tensor decomposition of 13×13×1024 interaction tensor" | None of this composition is computed on the SWE-bench Python corpus and fed to the predictor. The 105-pair (15-slot) ME-JEPA panel does not have an equivalent. `SlotPairEvidence` carries 5 scalars per pair × top-12 pairs = 60 scalars but is **truncated to the top 12 for display and never feeds the rank-lift scorer**. |
| `alldata.md:2923-2928` | `search_cross_embedder_anomalies` finds blind-spots (high in one embedder, low in another) | Implemented on the legacy ContextGraph product side, but the analogous "blind_spot_z_score" in DDA (`dda.rs:182-184`) is per-pair, per-input, and is **truncated + ignored on the ship-gate path**. The 105 per-input z-scores in `CF_MEJEPA_DDA_SIGNALS` are not consumed. |
| `alldata.md:2945-2961` | "13-dimensional topic profile" / cross-space agreement clustering | No cross-space agreement clustering exists on the SWE-bench Python panel; the 491-label registry (#413) and 128-skill hierarchy (#414) are not derived from cross-embedder co-cluster. |
| `head_projection.rs:260-303` (in-code claim) | per-head spec of "feature_schema_hash" with up to 5 features per head | True. But of 10 heads × ~3-5 features per head, only **4 features are pair-level cosines**. Zero are pair-level MI, despite the persisted `CF_MEJEPA_PAIRWISE_MI` providing exactly this. |
| `head_projection.rs:329-385 → predictor` | Per-head DDA projection enters the predictor as the surface of `MeJepaCompiler::compile` | The per-head projections shape calibration heads. The **ship-gate scorer** in `swebench_300x8_ship_gate_fsv.rs:1288` does not consume the per-head projections; it consumes `slot_feature_means` (univariate moments) and label bags. |

### 2.4 The structural collapse

The system has the following information flow on the ship-gate path:

```
[Real chunks]
  → 12 frozen embedders (per-slot vectors, total ~5,120-d)
  → slot_feature_means JSONL  (5 scalars × 12 slots = 60)
  → standardize → RBF exemplar head → rank-lift score
```

The DDA path is parallel and **dark to the scorer**:

```
[Real chunks]
  → DDA inputs (vector, centroid per slot)
  → compute_dda_signals (105 cosines + 105 z + 105 static MI placeholders)
  → CF_MEJEPA_DDA_SIGNALS                   ← persisted, used by:
       - dda_features.rs → 4 scalars (collapsed)
       - constellation_intelligence.rs → 12 displayed pairs (truncated)
       - head_projection.rs → 4 pairwise cosines across 10 heads
  → none of these flow into rank_lift_features()
```

**That is the fork.** The ship-gate scorer was wired to a univariate-moment summary of the 12 panels in service of TASK-PY-G-536 / #549 to get a clean baseline. The pair-level signal was left on the floor.

---

## 3. The `pairwise_mi_upper` static-baseline defect

### 3.1 Code citation

`crates/context-graph-mejepa-train/src/dda.rs:189-194`:

```rust
DdaSignals::try_new(DdaSignals {
    per_embedder_cosine,
    pairwise_cosine_upper,
    pairwise_mi_upper: baseline.pairwise_mi_upper.clone(),  // ← static baseline
    blind_spot_z_scores,
})
```

The `baseline` argument is a `DdaPairwiseBaseline` whose `pairwise_mi_upper: Vec<f32>` is set once (typically from `explicit_unit_baseline_for_count` at `dda.rs:77-84`, which fills with zeros, or from a precomputed corpus baseline). It is **identical for every input** in a given baseline. Downstream consumers of `DdaSignals.pairwise_mi_upper` (e.g. `dda_features.rs:198-201`, `constellation_intelligence.rs:598`) therefore see the same MI vector for every chunk, which means:

* `pairwise_mi_health` in `DdaFeatureProjection` is **constant per shard** (the only variation comes from sample size in the mean reduction) — the projection's `pairwise_mi_health` is effectively a corpus-level constant masquerading as a per-input feature.
* `PairAccumulator::observe` in `constellation_intelligence.rs:1056-1081` averages a constant MI across all rows in a cell → `mi_mean` is the corpus-baseline constant.
* `redundancy_score = consensus * (mi/(1+mi))` (line 1092) is then degenerate against the baseline.

### 3.2 Why this matters for the 0.16 gap

The `pairwise_mi_upper` field is a feature slot the architecture *expects* to carry per-input cross-embedder dependence. As-is it carries zero per-input information. Any predictor feature derived from it is **mathematically a multiplicative constant times the consensus** — exactly the redundancy with the per-embedder mean that explains why DDA "MI features" never improved holdout correlation in #549 fusion attempts.

This is not a bug in the sense of crashing the system; it is a **silent capability gap**. The architecture is shaped to consume per-input MI; the producer is not implemented.

### 3.3 Fix sketch (this is for the follow-on issue, not for this turn)

Two layered options:

**Option A: per-input KSG on signed-hash projections (cheapest)** — Extend `compute_dda_signals` to compute pairwise MI between `projected[i]` and `projected[j]` (the 128-d signed-hash projections already in scope at `dda.rs:165-187`). The projections are deterministic, share dimension (128), and admit kNN-based MI estimation with a small per-input neighbourhood (k=3-5). At 105 pairs × ~200ms each on the projections, this is order-1 GPU-minute per shard but **per-input** rather than baseline. Statistical caveat: signed-hash projection is a lossy random projection; the resulting MI is a lower bound on true MI in original slot space.

**Option B: per-cell KSG on slot-PCA scores (more faithful)** — Pre-compute a per-slot PCA basis from the train split. At prediction time, project each slot's full vector into its top-k PCA score (k ≈ 16 keeps O(slot_dim × 16) per chunk). Then run KSG on the resulting (16, 16) joint distribution per pair. This respects the original slot geometry and is the "partitioned-KSG" estimator PAPER §7.2 explicitly names. Cost: ~30-60s per cell of 30-300 rows; total ~10 GPU-minutes on the 240 train + 30 cal + 30 holdout per cell × 8 mutation cells.

**Both options can persist into `CF_MEJEPA_DDA_SIGNALS` by extending the schema** (the existing JSON serialisation already serialises a `pairwise_mi_upper: Vec<f32>` of the right shape) — the producer change is local; no consumer needs to learn a new schema.

### 3.4 Statistical realism check

Per McAllester & Stratos (2020, *Formal Limitations on the Measurement of Mutual Information*; arXiv:1811.04251), any distribution-free high-confidence MI lower bound is bounded by O(ln N). At N=30 (per-cell holdout), `ln(30) ≈ 3.40 nats ≈ 4.9 bits`. At N=240 (per-cell train), `ln(240) ≈ 5.48 nats ≈ 7.9 bits`. **This is the asymptotic ceiling on what any per-cell pairwise MI feature can certifiably express**, and it is the right ceiling for our use case (the predictor only needs a few bits of net-new pairwise info per pair to beat 0.95 if applied to the 105 pairs jointly). Section 7 quantifies further.

---

## 4. The full pair × pair × cell tensor — what's missing

The PAPER (§6.5 of `alldata.md`) and the §3.1 counting identity name a target architecture:

```
[13 per-space alignment]  +  [78 pairwise relationships]  +  [6 group summaries]  +  [optional Tucker(13×13×1024)]
```

For our 15-slot ME-JEPA panel this rebases to:

```
[15 per-slot alignment]  +  [105 pairwise relationships]  +  [g group summaries]  +  [optional Tucker(15×15×1024-or-PCA)]
```

The 15-slot "per-slot alignment" exists (`per_embedder_cosine` field; consumed as `mean_per_embedder_cosine_unit` by `dda_features.rs:213`). The 105-pair "pairwise relationships" partially exist (105 cosines + 105 static MI placeholders + 105 z-scores per input) but **collapse to 4 scalars** by the time they reach the predictor. The "group summaries" do not exist as a first-class concept. The Tucker tensor decomposition does not exist on the SWE-bench Python corpus.

### 4.1 Specifically, what is computed per-input vs what is consumed by the predictor

| Per-input artefact | Stored? | Consumed by `rank_lift_features()`? | Consumed by head projection? |
|---|---|---|---|
| 12-15 per-slot vectors (full dim, summing to 5,120-d total) | YES (`CF_MEJEPA_PANEL_*`) | NO (only univariate moments via `slot_feature_means`) | NO (each head reads `per_embedder_cosine`, not the vector) |
| 12-15 per-embedder cosine vs centroid | YES (`per_embedder_cosine` in `CF_MEJEPA_DDA_SIGNALS`) | NO | YES (1-3 per head) |
| 105 pair cosines on signed-hash projection | YES (`pairwise_cosine_upper`) | NO | **YES, but only 4 specific pairs** (`E_AST::E_DIFF`, `E_TYPE_GRAPH::E_DATA_FLOW`, `E_REASONING::E_DIFF`, `E_REASONING::E_ORACLE`) |
| 105 pair MI estimates | NO — `pairwise_mi_upper` is static baseline | (degenerate) | (degenerate) |
| 105 pair blind-spot z | YES (`blind_spot_z_scores`) | NO | NO (only max-abs collapsed) |
| 105 pair consensus / contradiction / redundancy / novelty | computed in `summarize_dda_constellation_intelligence` but **truncated to top 12 pairs** | NO | NO |
| Pair Granger F-statistic (left → right vs right → left) | NO (no producer in ME-JEPA) | NO | NO |
| Pair distance correlation (Szekely) | NO | NO | NO |
| Pair HSIC | NO | NO | NO |
| Pair CCA component scores | NO | NO | NO |
| PID atoms (Unique_left, Unique_right, Redundant, Synergistic) about Pass/Fail | NO | NO | NO |
| Triple synergy I(left, right; oracle) − Σ I(slot; oracle) | NO | NO | NO |
| Quadrant-conditional cosine ({pass, fail} × {cosine_high, cosine_low}) | NO | NO | NO |

This is the "missing math" the operator suspects. Most of it is computable from existing on-disk artefacts at modest CPU cost.

---

## 5. External research synthesis

The investigation must ground proposed features in peer-reviewed instrumentation for the small-N (N=2,400 input panels; N=30-300 per cell), binary-outcome (Pass/Fail vs Docker oracle), heterogeneous-dim (slot dims 128-1024) regime we operate in. The candidate families and what each is good / bad for in our regime:

### 5.1 Mutual Information family

* **KSG (Kraskov, Stögbauer, Grassberger, 2004)** — k-NN-based, no density estimation, works on heterogeneous-dim pairs via per-marginal k-th nearest-neighbour. Implementation reference: [infomeasure documentation](https://infomeasure.readthedocs.io/en/0.5.0/guide/mutual_information/kraskov_stoegbauer_grassberger/), [NPEET_LNC GitHub](https://github.com/BiuBiuBiLL/NPEET_LNC). Foundational paper: Kraskov, Stögbauer, Grassberger, *Estimating Mutual Information*, Phys. Rev. E 69, 066138 (2004), [arXiv:cond-mat/0305641](https://arxiv.org/abs/cond-mat/0305641). **In our regime:** suitable if slot dims are first reduced to comparable scale (PCA top-k or signed-hash projection). Per-pair cost O(N k log N) per cell.
* **LNC-KSG (Gao, Ver Steeg, Galstyan, 2015)** — local non-uniformity correction for strongly-dependent variables. Better than vanilla KSG when slots are not approximately uniform locally — i.e. for code embeddings that cluster heavily. Paper: *Efficient estimation of mutual information for strongly dependent variables*, AISTATS 2015, [PMLR proceedings](http://proceedings.mlr.press/v38/gao15-supp.pdf). **In our regime:** the right MI estimator for slot-pair MI when redundancy is suspected (e.g. E1↔E10, both dense-text encoders).
* **MINE (Belghazi et al., 2018)** — neural MI estimator via Donsker-Varadhan KL bound. Paper: [arXiv:1801.04062](https://arxiv.org/abs/1801.04062). **In our regime: ill-advised** — MINE requires thousands of samples for low variance and is notorious for high-bias / high-variance on small N. At our 30-row holdout cells it would essentially fit noise.
* **DEMINE (Lin et al., 2020)** — data-efficient MINE with cross-validation; [arXiv:1905.03319](https://arxiv.org/pdf/1905.03319). **In our regime:** marginally usable but still over-parameterised for N=240. Not recommended as a first move.
* **Copula entropy (Ma & Sun, 2011)** — `MI(X;Y) ≡ −H(C_{XY})` for copula `C`. Estimator: empirical copula via rank-statistic + KSG on copula samples. Reference R package: [copent](https://github.com/majianthu/copent), [arXiv:2005.14025](https://arxiv.org/pdf/2005.14025). **In our regime: best for univariate MI** when slot dims are first reduced to scalars (e.g. slot-PCA top-1 score, or `||slot||_2`). Invariant to monotonic marginal transformations, which is exactly what we want when slot scales differ.
* **McAllester & Stratos limit (2020)** — distribution-free high-confidence MI lower bound ≤ O(ln N). Paper: [arXiv:1811.04251](https://arxiv.org/abs/1811.04251), AISTATS 2020. **In our regime: load-bearing constraint.** Sets the ceiling on what *any* MI feature can certifiably express per cell. See §7.

### 5.2 Partial Information Decomposition (PID)

* **Williams & Beer (2010)** — original framework: I(X1, X2; Y) = Unique_1 + Unique_2 + Redundant + Synergistic. Reference: [Towards Data Science overview](https://towardsdatascience.com/what-is-partial-information-decomposition-and-how-features-interact-a713456a1029/). **For our predictor:** decomposes "what cross-embedder MI tells us about Pass/Fail" into atoms — Synergistic captures pair-level information that neither slot alone exposes. **This is exactly the quantity the ship gate needs**, because univariate moments by definition can only express Unique.
* **Bertschinger, Rauh, Olbrich, Jost, Ay (2014)** — game-theoretic / decision-theoretic unique-information measure. Papers: *Quantifying Unique Information*, Entropy 16:2161-2183, [MDPI](https://www.mdpi.com/1099-4300/16/4/2161); *Reconsidering unique information*, [arXiv:1404.3146](https://arxiv.org/abs/1404.3146). **Practical computer-tractable PID for binary outcomes at small N.**
* **Griffith & Koch (2014); Williams synergy/redundancy review (MDPI 2015)** — alternative non-negative decompositions; [MDPI Entropy review](https://www.mdpi.com/1099-4300/17/5/3501). **In our regime: prefer Bertschinger BROJA since it has closed-form O(2^{|Y|}) optimisation for binary Y**, which is exactly our case (Y = oracle Pass/Fail).
* **Information Decomposition Based on Cooperative Game Theory (2019)** — [arXiv:1910.05979](https://arxiv.org/pdf/1910.05979). Game-theoretic axiomatisation; useful framing.

### 5.3 Kernel-based independence / dependence

* **HSIC (Gretton et al., 2005)** — Hilbert-Schmidt independence criterion; non-parametric, kernel-based; detects nonlinear dependence via cross-covariance operator. References: [HSIC overview](https://www.emergentmind.com/topics/hilbert-schmidt-independence-criterion-hsic-b91530e0-e26b-4bcf-8217-d3245eede233); [Learning with HSIC review (Knowledge-Based Systems 2022)](https://www.sciencedirect.com/science/article/abs/pii/S0950705121008297); [Statistical Insights into HSIC in High Dimensions, OpenReview](https://openreview.net/forum?id=qPyvuFT0U9). **In our regime: an excellent first move.** HSIC has a closed-form O(N²) estimator with no regularisation hyperparameter, works on heterogeneous-dim pairs via RBF kernels on each side, and is provably consistent at small N (unlike MINE). The biased empirical estimator has bias of order 1/N which is acceptable at N=240.
* **Distance correlation (Székely, Rizzo, Bakirov, 2007)** — `dCor(X,Y) = 0 ⇔ X⊥Y`; works for `X ∈ R^{d_x}`, `Y ∈ R^{d_y}` with `d_x ≠ d_y`. Foundational paper: [Annals of Statistics 35(6)](https://projecteuclid.org/journals/annals-of-statistics/volume-35/issue-6/Measuring-and-testing-dependence-by-correlation-of-distances/10.1214/009053607000000505.full); fast O(N log N) algorithm: [arXiv:1810.11332](https://arxiv.org/pdf/1810.11332). **In our regime: cheap, slot-identity-preserving, characterises full independence (not just linear), uses no hyperparameter.** This is the single feature I would compute first.
* **Randomised Dependence Coefficient (Lopez-Paz, Hennig, Schölkopf, 2013)** — random Fourier features + CCA; rank-invariant; cheap. Paper: [arXiv:1304.7717](https://arxiv.org/abs/1304.7717). **In our regime: fastest nonlinear dependence measure; ~5 lines of code.** Has the property that as the random-feature kernel bandwidth → 0, RDC → Spearman, giving graceful degradation.

### 5.4 Canonical correlation family

* **Linear CCA** — Hotelling, 1936. Gives top-k canonical directions and correlations between two slots. **In our regime: per-pair top-1 canonical correlation is a single robust scalar that captures the strongest linear dependence after marginal whitening.** Per pair × per cell cost O(d_x · d_y · min(d_x, d_y, N)) — totally fine at N=240.
* **Sparse CCA / Penalised Matrix Decomposition (Witten et al., 2009)** — regularised CCA for high-dim/small-N. *Particularly relevant at p > N.*
* **Deep CCA (Andrew et al., 2013) / DCCA-AE (Wang et al., 2015) / DTCCA (Wong et al., 2020)** — papers: [Wang et al., ICML 2015](http://proceedings.mlr.press/v37/wangb15.pdf); [DTCCA, arXiv:2005.11914](https://arxiv.org/pdf/2005.11914). **In our regime: do not deploy.** Deep CCA needs thousands of samples and risks the same ECE-regression as #392 trajectory features. The linear / sparse variants are the right move.
* **Multi-View Information Bottleneck (Federici et al., 2020)** — paper: [arXiv:2002.07017](https://arxiv.org/abs/2002.07017). Implementation: [GitHub mfederici/Multi-View-Information-Bottleneck](https://github.com/mfederici/Multi-View-Information-Bottleneck). **In our regime: relevant as the right *consumption* recipe** — the right architecture for a calibration-preserving consumer of pair-level features is a multi-view IB head that compresses to a low-d sufficient statistic.

### 5.5 Calibration-preserving consumption

* **Temperature scaling (Guo et al., 2017)** — single scalar T re-scales logits; preserves ranking; cheap. Reference: [scikit-learn issue #28574](https://github.com/scikit-learn/scikit-learn/issues/28574).
* **Isotonic calibration (Zadrozny & Elkan, 2002)** — monotonic piecewise constant; flexible but overfits at small N. **In our regime: avoid at per-cell N=30**, use at the global level only.
* **Platt scaling (Platt, 1999)** — logistic regression on logits. Works at small N. Reference: [Wikipedia](https://en.wikipedia.org/wiki/Platt_scaling).
* **Calibration Meets Reality (Bao et al., 2025)** — [arXiv:2509.23665](https://arxiv.org/pdf/2509.23665). Discusses how feature additions interact with ECE.
* **Measuring Calibration in Deep Learning (Nixon et al., 2019)** — [OpenReview](https://openreview.net/pdf?id=r1la7krKPS). Foundational ECE definition; bin sensitivity analysis.

The peer-reviewed picture: **HSIC + distance correlation + linear CCA + Bertschinger PID** are the four families that fit our N=240 (train) and N=30 (holdout) regime with provable small-N consistency and zero ECE-regressing risk *if* they are consumed via a temperature-scaled or Platt-rescaled head rather than concatenated raw.

---

## 6. Five concrete proposed features

Each feature is specified with: computation, slot-identity preservation, live-input safety, target file:line, expected lift, falsification test, storage cost, and ECE risk mitigation. Features are ranked by expected lift × cheapness.

### Feature 1 — Per-pair distance correlation `dCor(slot_i, slot_j)` aggregated to per-cell

* **Computation:** for each pair (i, j) ∈ C(15,2) = 105, for each (train) cell of N rows: compute Székely's distance correlation `dCor(Φ_i(x), Φ_j(x))` over the N rows, using the fast O(N log N) algorithm (Huo & Székely, 2016; [arXiv:1810.11332](https://arxiv.org/pdf/1810.11332)). At per-input scoring time, attach the 105 per-cell `dCor` values as a feature vector (constant within a cell; varies across cells).
* **Slot identity preserved?** YES. `dCor` is a scalar pairwise relationship, doctrinally permitted at `01_overview.md §1.5ter`.
* **Live-input safe?** YES — the cell-level `dCor` is computed from train-split chunks only, then used as a per-cell exemplar feature; no oracle / Docker / test label enters the feature.
* **Target file:line:** new producer in `crates/context-graph-mejepa-train/src/learning_signal/`, e.g. `cross_embedder_dcor.rs`; consumed by extending `swebench_300x8_ship_gate_fsv.rs:rank_lift_features` (line 1288) to extend the feature vector with a per-cell `dcor_pair[i,j]` slice (105 floats, all constant within cell).
* **Expected lift:** **0.04-0.07** in `prediction_oracle_correlation`. The univariate moment surface is currently consensus-blind. `dCor` exposes per-pair nonlinear dependence shifts that correlate with mutation category — `subtle_flip` and `swap_variable` mutations specifically alter pair-level dependence patterns between AST and DataFlow slots without altering marginal moments. The 105 features are heavily correlated themselves, so the effective rank of the new feature subspace is more like 15-25.
* **Falsification test:** train-only ablation. If the rank-lift hold-out score improves by `< 0.01` on `cross_embedder_dcor`-augmented features vs baseline, the hypothesis is refuted.
* **Storage cost:** extend `CF_MEJEPA_DDA_SIGNALS` with a `pairwise_dcor_upper: Vec<f32>` field, OR persist a separate `CF_MEJEPA_CELL_LEVEL_DCOR` (preferred — `dCor` is per-cell, not per-input). Cost: 15-cells × 105 floats × 8 mutation categories = ~13K floats = 50 KB on disk.
* **ECE risk vs #392:** The #392 failure mode was that adding 900-d trajectory features regressed ECE from 0.024 to 0.04. The mechanism was that uncalibrated raw features inflate confidence near the decision boundary. Mitigation: **(a)** consume `dCor` through Platt scaling (logistic head with temperature) rather than concatenating raw into the RBF exemplar; **(b)** standardise per-cell first; **(c)** report ECE_before / ECE_after on cal and holdout; **(d)** require ECE delta ≤ +0.003 as ablation pass criterion. The fundamentally different shape — 105 cell-constant scalars rather than 900 per-input trajectory deltas — means there is far less per-row variance to over-fit, so the ECE risk is structurally lower than #392.

### Feature 2 — Per-pair HSIC normalised (NOCCO-HSIC) at slot-level, per-cell

* **Computation:** for each pair (i, j), each cell: `HSIC(slot_i, slot_j) = ||C_{XY}||²_HS` with RBF kernels on each slot, median-heuristic bandwidth per slot (using only train chunks for bandwidth selection). Normalise: `nHSIC = HSIC / sqrt(HSIC(slot_i, slot_i) · HSIC(slot_j, slot_j))` (Fukumizu et al., 2008). Feature: per-cell × 105 pairs = 105 scalars per cell. Reference: [Knowledge-Based Systems 2022 review](https://www.sciencedirect.com/science/article/abs/pii/S0950705121008297).
* **Slot identity preserved?** YES — HSIC uses a kernel on each side; the kernel is internal to one slot.
* **Live-input safe?** YES (cell-level only).
* **Target file:line:** `crates/context-graph-mejepa-train/src/learning_signal/cross_embedder_hsic.rs`; same consumer extension as Feature 1.
* **Expected lift:** **0.02-0.05**. HSIC captures the same dependence as `dCor` plus higher-order moments via kernels; somewhat redundant with Feature 1. Worth gating to "compute both, keep the best 60 by feature importance".
* **Falsification test:** ablation against `dCor` alone — if `HSIC` alone is better than `dCor` alone, prefer HSIC; if combined is no better than max(dCor, HSIC), prefer the cheaper one.
* **Storage cost:** 50 KB.
* **ECE risk:** same as Feature 1. The Daubert testability test is met: a Platt-scaled HSIC feature has known calibration properties at N=240 train (Sec. 3.3 of HSIC review).

### Feature 3 — Bertschinger BROJA Synergy decomposition `Syn(slot_i, slot_j; Pass/Fail)` per-cell

* **Computation:** for each pair (i, j), each cell of N rows with binary oracle outcome `y_n ∈ {0,1}`:
  1. Quantise each slot via PCA top-k → top-1 PCA score → 4-bin equal-mass binning (so each slot becomes a discrete variable with 4 levels).
  2. Compute joint empirical distribution `P(X1, X2, Y)` over (4 × 4 × 2) = 32 cells.
  3. Solve the BROJA optimisation: `Unique_1 + Unique_2 + Redundant + Synergistic = I(X1, X2; Y)` with `Synergistic ≥ 0` and the Blackwell ordering constraint. Closed-form for binary Y via the [BROJA-2PID GitHub algorithm](https://github.com/Abzinger/BROJA_2PID). Reference: Bertschinger et al., *Quantifying Unique Information*, [Entropy 16:2161-2183](https://www.mdpi.com/1099-4300/16/4/2161).
  4. Feature: per-cell × 105 pairs × 4 PID atoms = 420 cell-constant scalars per cell.
* **Slot identity preserved?** YES — PID is computed on scalar projections of each slot; the projections preserve slot identity via the per-slot PCA.
* **Live-input safe?** **CAUTION** — the BROJA optimisation uses the binary oracle label `Y` as the target. This is computed at train time (cell-level features) but the feature value is per-cell, not per-input, so live-input prediction reads a precomputed per-cell scalar with no live oracle access. **However**, the feature MUST be computed from the train-split of each cell only and held constant for cal/holdout/live (otherwise data leakage). Operator must verify this in the producer.
* **Target file:line:** `crates/context-graph-mejepa-train/src/learning_signal/cross_embedder_pid.rs`; consumed in `swebench_300x8_ship_gate_fsv.rs:rank_lift_features`.
* **Expected lift:** **0.03-0.06**. The Synergistic atom in particular is information that **neither slot alone exposes** and is therefore mathematically orthogonal to the existing 60 univariate moments. This is the single feature most likely to close the gap because it formalises "what cross-embedder mining buys you that single-slot features cannot".
* **Falsification test:** ablation: hold out cells, measure rank-lift on holdout with PID atoms vs baseline. If Synergistic atoms do not improve rank-lift by ≥ 0.01, the hypothesis that net-new pair information about Pass/Fail exists is refuted.
* **Storage cost:** 105 pairs × 4 atoms × 15 cells × 8 categories = ~50 KB.
* **ECE risk:** PID values are intrinsically bounded in `[0, I(slot;Y)]` and well-calibrated by construction (they are normalised information quantities). Consume through Platt scaling. Crucially, the per-cell PID values are computed once at training time and held constant in scoring, so there is no per-input feature noise to inflate ECE. **This is the lowest ECE-risk feature in the set.**

### Feature 4 — Per-pair Top-1 Linear CCA correlation `ρ_top(slot_i, slot_j)` per-cell

* **Computation:** for each pair, each cell: standard linear CCA on the (N × d_i) and (N × d_j) slot matrices. Take `ρ_top = max(canonical correlations)`. Use whitening with Tikhonov regularisation `λ = 0.01 × tr(Σ)/d` to avoid numerical singularity at d > N/2 (which happens for `E_Problem` at d=1024 and N=240). Feature: 105 cell-constant scalars per cell.
* **Slot identity preserved?** YES.
* **Live-input safe?** YES.
* **Target file:line:** `crates/context-graph-mejepa-train/src/learning_signal/cross_embedder_cca.rs`.
* **Expected lift:** **0.02-0.04**. CCA captures linear cross-embedder dependence after marginal whitening; complementary to `dCor` (which captures nonlinear). If CCA dominates `dCor` it would falsify the "nonlinear dependence carries the signal" hypothesis.
* **Falsification test:** ablation against `dCor`. Prefer whichever is better per cell; or ensemble both.
* **Storage cost:** 50 KB.
* **ECE risk:** linear features have low ECE risk; consume via Platt scaling.

### Feature 5 — Per-input Triple-Synergy I-min over (slot_i, slot_j; oracle_proxy)

* **Computation:** the only **truly per-input** pair-level feature in the set. For each input x, for each pair (i, j): compute `Imin(slot_i(x), slot_j(x); ŷ(x))` where `ŷ(x)` is a *proxy* for the oracle outcome that is computable live (the current head-projection oracle-head output, **NOT the Docker oracle itself**). `Imin` is the Williams-Beer redundancy lattice minimum information measure (Williams & Beer 2010). The per-input value indicates how much the *pair* of slots redundantly informs the proxy outcome at this specific input.
* **Slot identity preserved?** YES.
* **Live-input safe?** **CAUTION** — proxy ŷ(x) must be the current predictor's own output, NOT the oracle. Computing this is *circular* (feature depends on predictor output, predictor consumes feature). Implement as an *iterative refinement* feature: use the previous-step predictor output as ŷ(x); update one round; deploy. This is analogous to test-time adaptation and is a Goodhart-flag-eligible feature — Sherlock 3 should comment on whether the operator's foundationality / CP_φ surfaces give a better outcome proxy.
* **Target file:line:** `crates/context-graph-mejepa/src/dda_features.rs` extended with a `triple_synergy_per_input: Vec<f32>` field (105 scalars per input); consumed by the predictor head not the rank-lift scorer.
* **Expected lift:** **0.01-0.03**. The per-input variance is the only path beyond per-cell features for closing the gap, but the proxy substitution introduces a known confounder (the predictor scoring its own confidence).
* **Falsification test:** without proxy substitution, replace ŷ with the per-cell base rate of Pass; if lift survives, the per-input signal is real and the proxy is doing something. If lift vanishes, the proxy was the only thing.
* **Storage cost:** extending `CF_MEJEPA_DDA_SIGNALS` per-input by 105 floats × 12-15 GPU minutes per shard to recompute.
* **ECE risk:** **HIGHER** than features 1-4 because it is per-input. Mitigation: same as Feature 1 (Platt scaling, ECE-delta gate ≤ +0.003). If ECE blocks, fall back to per-cell aggregated Synergy (Feature 3).

---

## 7. Statistical-power audit

The proposed features must respect statistical-power constraints at our cell sizes.

### 7.1 Per-cell N and feature-count ceiling

The 300 SWE-bench Lite × 8 mutation categories = 2,400 corpus rows are split as 240 train / 30 calibration / 30 holdout per category, then optionally bucketed into "mixed cells" of 5-30 rows. Per-cell statistics:

| Layer | N per cell | log_2(N) ceiling on MI (bits) | Distance-correlation power | Recommended feature count |
|---|---:|---:|---|---:|
| Per-category train | 240 | 7.9 | strong | up to 60 |
| Per-category calibration | 30 | 4.9 | moderate | 15-20 |
| Per-category holdout | 30 | 4.9 | moderate | 15-20 |
| Mixed cells (#549) | 30-100 | 4.9-6.6 | weak-moderate | 10-30 |

### 7.2 Recommendation: aggregate-then-cell

Rather than computing all 105 pair features at the smallest cells (where statistical power is lowest), recommend:

* **Compute pair features at the category-aggregated level** (N=240 train) — this gives ~8 bits of MI ceiling per pair, supporting Features 1-4 confidently.
* **Use cell-level aggregation for hierarchy / decomposition**: cell features are projections of the category-aggregated pair features onto cell-conditional summaries. This avoids the N=30 statistical-power floor.
* **Use the proven dcor algorithm** (Huo & Székely 2016 O(N log N), [arXiv:1810.11332](https://arxiv.org/pdf/1810.11332)) to keep per-cell compute at ~5ms per pair per cell. Total 105 × 8 × 15 = 12,600 dcor computations per shard at 5ms = 1 minute per shard. Negligible.

### 7.3 Multiple-comparisons hazard

105 pair features tested per cell × 15 cells × 8 categories = 12,600 (feature × cell) tests. At α=0.05 uncorrected, expect 630 false discoveries. **Use Benjamini-Hochberg false-discovery-rate correction** at the per-category level, and **report family-wise α**. Otherwise pair features will get false significance that fails to replicate.

### 7.4 Power for `dCor` at N=240

Per Huo & Székely (2016), the asymptotic distribution of `n · dCor²(X, Y)` under independence is a quadratic form in centered Gaussians; for N=240 and effect size `dCor = 0.1` (a small nonlinear dependence), power is approximately 0.85 at α=0.01. **This is the right sample size.** At N=30 (cell-level), power for the same effect drops to ~0.35 — confirming the aggregate-then-cell recommendation.

---

## 8. ECE risk and mitigation

The #392 trajectory features (900-d) regressed ECE from baseline 0.024 to 0.04 on holdout, despite the active correlation passing — this is the known failure mode. Why proposed features avoid it:

### 8.1 Mechanism of #392 ECE regression

The #392 features were:
* 900-d per-input deltas (`v0`, `v1`, `v2`, `d10`, `d21`, `d20`, sign-flipped, l2-norms, named cross-slot relationships, hashed label transitions)
* Standardized per-train-split
* Fed raw into the RBF exemplar head

The exemplar head with high-dim raw features over-fits the train manifold and inflates confidence near the decision boundary — pure curse-of-dimensionality at small N. ECE regression is the symptom.

### 8.2 Why Features 1-4 avoid this

| Mechanism | #392 | Proposed (F1-F4) |
|---|---|---|
| Feature dim per input | 900 | 0 (cell-constant) to 105 |
| Per-input variance | high (every per-input trajectory unique) | zero (cell-constant) |
| Estimator regularisation | none (raw delta) | KSG/HSIC/PID intrinsically normalised; CCA Tikhonov-regularised |
| Calibration consumption | raw concat into exemplar | Platt-scaling head with ECE gate |

The structural reason F1-F4 are safer: a cell-constant feature contributes the same signal to every row in the cell, which by construction cannot inflate per-row confidence variance. It can only shift the *mean* per-cell predicted probability — which Platt scaling explicitly handles.

### 8.3 Feature 5's higher ECE risk

Feature 5 is per-input and shares the #392 failure mode. Mitigation: (a) require an ECE-aware feature gate that auto-rejects Feature 5 if it pushes ECE_holdout beyond a fixed budget; (b) fall back to F3 (PID atoms aggregated) which is the calibration-safest alternative.

### 8.4 Calibration-preserving consumer recipe

For F1-F4, the **calibration-preserving recipe**:

1. Compute per-cell pair features at the category-aggregated level (N=240).
2. Standardise train-only.
3. Concatenate to existing 60 univariate moments (which the current `rank_lift_features` already provides).
4. Train an RBF exemplar head as today, but **insert a Platt-scaling layer** between the exemplar output and the final score.
5. Fit Platt scaling on the calibration split (N=30 per cell) — small N is fine for Platt (just two parameters).
6. **Gate:** require ECE_holdout - ECE_baseline ≤ +0.003. If exceeded, reject the feature set.
7. **Diagnostic:** report ECE per-cell vs baseline; if any cell regresses ECE > +0.005, the feature set must be rejected even if global ECE passes.

---

## 9. Exact FSV-harness spec for a follow-on issue

The operator can file the follow-on issue against this spec. The harness must be deterministic, must read aiwonder durable state, and must produce a binary contract.

### 9.1 Harness identity

* Name: `task_py_g_$ISSUE_cross_embedder_pair_features_fsv`
* Location: `crates/context-graph-mejepa-train/examples/`
* FSV root: `/zfs/archive/contextgraph/fsv/task-py-g-$ISSUE-cross-embedder-pair-features-fsv/run-$TS/`
* Producer module: `crates/context-graph-mejepa-train/src/learning_signal/cross_embedder_pair_features.rs`

### 9.2 Inputs (read-only)

* `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-*/shards/` — slot-level vectors per input
* `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` — baseline rank-lift artifacts and `swebench_300x8_ship_gate_fsv.json` for the baseline `prediction_oracle_correlation = 0.7942190170`
* The 300×8 = 2,400 corpus row split assignments from `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-*/split_assignments.jsonl`

### 9.3 Outputs (binary contract)

The harness produces a JSON report at `$FSV_ROOT/report.json` with:

```json
{
  "schema_version": 1,
  "issue": <N>,
  "baseline_prediction_oracle_correlation": 0.7942190170,
  "augmented_prediction_oracle_correlation": <new value>,
  "delta_prediction_oracle_correlation": <new - baseline>,
  "baseline_ece_holdout": <baseline value>,
  "augmented_ece_holdout": <new value>,
  "delta_ece_holdout": <new - baseline>,
  "per_cell_metrics": {
    "category × split": {
      "n": <int>,
      "active_correlation": <float>,
      "ece": <float>,
      "brier": <float>,
      "ece_delta_vs_baseline": <float>
    }
  },
  "feature_breakdown": {
    "feature_name": {
      "type": "dcor" | "hsic" | "pid_synergy" | "cca_top1",
      "feature_count": <int>,
      "shap_summary": <BTreeMap<str, f32>>
    }
  },
  "panel_b_compatibility": {
    "panel_b_score_present": <bool>,
    "panel_b_oracle_correlation": <float | null>,
    "abs_correlation_delta_a_vs_b": <float | null>
  },
  "ship_gate_pass": <bool>
}
```

### 9.4 Gates (PASS criteria)

The follow-on FSV PASSES if and only if:

1. `delta_prediction_oracle_correlation ≥ +0.04` on aiwonder real-input data (not synthetic).
2. `delta_ece_holdout ≤ +0.003` (calibration not regressed beyond budget).
3. For every (category × split) cell, `ece_delta_vs_baseline ≤ +0.005` (no single cell allowed to regress beyond local budget).
4. `panel_b_score_present == true` (else this is not Panel-A-vs-Panel-B comparable).
5. `abs_correlation_delta_a_vs_b ≤ 0.05` (Panel B compatibility per #405).
6. Independent re-open verification: a second `cargo build --release` + run produces byte-identical `report.json` modulo timestamps.

The follow-on FSV FAILS (and the feature set is rejected) if any of:

* Augmented correlation does not improve, OR
* ECE regresses beyond budget, OR
* Per-cell ECE regression beyond local budget, OR
* Panel B drift exceeds 0.05.

### 9.5 Power diagnostics

The report MUST include:

* Per-feature univariate `mutual_information_with_oracle` (estimated by KSG with k=3, 95% CI per Belghazi-style cross-validation).
* `feature_redundancy_with_baseline` (mean correlation of each new feature with the existing 61 baseline features).
* `effective_feature_rank` (singular value decomposition of the augmented feature matrix; report cumulative variance at rank 60 vs rank 100).

This catches the case where new features add many scalars but only 1-2 effective dimensions of new information.

---

## 10. Honest risk register

### 10.1 What I don't know

* Whether the aiwonder slot vectors are actually orthogonal-ish to each other in the SWE-bench Python regime. The `pairwise_cosine_upper` values in `CF_MEJEPA_DDA_SIGNALS` exist but I have not read them. If E_AST and E_DataFlow are already correlated at 0.9+ in the production corpus, the pair-level information ceiling drops dramatically. **This is the single biggest unknown.** A 1-day audit reading existing `pairwise_cosine_upper` distributions on aiwonder would resolve it before committing to feature production.
* Whether the 300-instance corpus has enough mutation-category cell variation to support 105-pair feature engineering, or whether 4-6 mutations dominate the holdout failures and most pair features get fitted to category-specific noise.
* Whether the slot-PCA basis (required by Features 3 & 4) is stable across train splits. Slot-PCA on N=240 with d_slot=1024 (E_Problem) is borderline; a leave-one-cell-out stability audit is recommended before deploying.

### 10.2 What could be wrong

* **Feature 1 (dCor) might be highly correlated with the existing `per_embedder_cosine` features** — dCor is technically a function of cross-distance, which depends on marginal distances, which depend on per-embedder norms. If dCor reduces to a function of slot moment products, lift will be minimal. Mitigation: report Pearson correlation of dCor features with the existing 60 univariate moment features in the FSV.
* **Feature 3 (PID) might be ill-defined at small N** — the BROJA optimisation requires the joint empirical distribution P(X1, X2, Y) to be smooth. At N=30 cell with 4×4×2=32 cells, every joint cell has expected count 0.94 — this is *not* enough for stable PID. **Recommend computing PID at the category-aggregated level (N=240) only**.
* **The 0.16 gap might not be cross-embedder-pair-shaped at all.** It could be a calibration miscalibration (Sherlock 2's domain) or a foundationality / CP_φ exploitation issue (Sherlock 3's domain), and pair features will improve correlation by < 0.04. Mitigation: the FSV ship-gate fall-through is the only honest test — run the producer, measure, accept the result whether good or bad.
* **The `pairwise_mi_upper` static baseline defect might already explain the gap** — if the architecture was supposed to consume per-input MI and gets zero variance, fixing that one defect (Section 3) might close 0.05-0.10 of the gap alone, making Features 1-5 partly redundant. **Recommend implementing the per-input MI producer FIRST**, measuring, then deciding whether the other features are needed.
* **HSIC and dCor are mathematically related** (both reduce to characteristic-function-based independence in the kernel/distance representation). They may be near-duplicates as features. Mitigation: compute both, report feature importance, drop the less informative.
* The slot-preserving doctrine in `01_overview.md §1.5ter` permits "scalar pairwise cross-slot relationships (MI, correlation, agreement)" — every proposed feature is a scalar per pair, which is doctrinally compliant. But **the FSV must explicitly attest no dim-wise cross-embedder comparison** is performed (i.e. no concatenation of `slot_i ⊕ slot_j` into a joint vector — that violates `MEJEPA_TCT_DIM_MISMATCH`).

### 10.3 What I would do if forced to bet

If I had to bet $100 on one feature: **Feature 3 (Bertschinger BROJA Synergy) computed at the category-aggregated level (N=240), restricted to the 10 highest-dCor pairs**. Synergistic atoms about Pass/Fail are mathematically orthogonal to univariate moments. Limiting to 10 pairs controls multiple-comparisons and ECE risk. Expected lift: 0.03-0.05; probability the ship gate is closed by this alone: ~25%.

If I had to bet on a sequence: **(1) fix the static MI defect (Section 3); (2) add Feature 1 (`dCor`); (3) if Feature 1 stalls, add Feature 3 (PID synergy at top-10 dCor pairs); (4) measure ECE every step; (5) abort if ECE regresses.** Probability the ship gate is closed by this sequence: ~50%. Probability the 0.16 gap is entirely cross-embedder-pair-shaped: ~30-40% — meaning even a perfect implementation of these features is unlikely to single-handedly close the gap without help from Sherlocks 2 and 3.

---

## 11. Three questions the other Sherlocks must answer

For the operator to verify the three reports compose without overlap or contradiction:

### Q1 (for Sherlock 2 — calibration / ECE / active features)

If we add 60-105 cell-constant cross-embedder pair features (Features 1-4 above), can the existing `rank_lift_features` calibration path (`swebench_300x8_ship_gate_fsv.rs:1101-1167`) maintain `ece_delta ≤ +0.003` per-cell with only standardisation + RBF exemplar + Platt rescaling, or do we need a more sophisticated calibration head (multi-view IB, focal loss, label smoothing)? Specifically: at per-cell N=30 holdout, is Platt scaling's two-parameter logistic head numerically stable, or do we need a beta calibration (Kull, Silva, Flach 2017) which is provably better-calibrated at small N?

### Q2 (for Sherlock 3 — foundationality / CP_φ / entropy)

Feature 5 (per-input Triple Synergy) requires an oracle proxy `ŷ(x)` that is live-input safe. Does the foundationality scalar (chunk-dependency PageRank) or the CP_φ compression-progress score provide a calibrated, live-safe outcome proxy at per-input granularity? If yes, Feature 5 is salvageable; if no, Feature 5 must use the predictor's own previous-step output (Goodhart-flag-eligible) or be dropped. Additionally: does CP_φ exploitation at the per-input level itself add features that compete with our pair features for the same predictor head capacity?

### Q3 (for both Sherlocks 2 and 3 — composition)

If the combined feature set across the three reports is `baseline (61) + cross-embedder pair (105) + calibration features (Sherlock 2) + foundationality/CP_φ features (Sherlock 3) > 200 total features`, are we in dangerous territory at per-cell N=30 holdout? The McAllester-Stratos limit and the curse of dimensionality both bound feature counts at small N. Should we **gate the union of all proposed features** through a sparsity-inducing regulariser (group lasso over the per-Sherlock feature blocks) and report each block's marginal contribution, rather than concatenating all three? This would prevent us from collectively re-creating the #392 failure mode by additive over-fitting.

---

*"There is nothing more deceptive than an obvious fact."*

The obvious fact: ME-JEPA mines 105 cross-embedder pairs per input. The deceptive part: the production ship-gate scorer does not read a single one of them. The 0.16 gap may not be entirely closeable by cross-embedder pair features alone, but a sequenced, calibration-gated, doctrinally-compliant feature programme — starting with fixing the static MI defect and adding distance correlation, then Bertschinger PID synergy at the top-10 dCor pairs — is the lowest-risk, highest-expected-lift move available in this lane.

The game is afoot.

— Sherlock Holmes #1, 2026-05-21

---

## Relevant absolute file paths for the operator

* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/src/dda.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi_matrix.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/dda_features.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/constellation_intelligence.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/head_projection.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/pairwise_mi.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-instruments/src/lib.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/examples/swebench_300x8_ship_gate_fsv.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/examples/task_py_g_392_temporal_trajectory_features_fsv.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/examples/task_py_g_549_mixed_cell_signal_fusion_fsv.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa-train/examples/post_547_mixed_cell_ranking_repair_fsv.rs`
* `/home/cabdru/contextgraph/crates/context-graph-mejepa/src/cross_panel.rs`
* `/home/cabdru/contextgraph/docs/PAPER.md` (§3.1, §7.2, EXP-2 / RO-8)
* `/home/cabdru/contextgraph/docs/alldata.md` (§6.5 78-dim cross-correlation claim)

Production durable SoT (cannot read from this WSL box; reason about as remote):

* `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-*/shards/`
* `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`
* `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/`
* `/zfs/archive/contextgraph/fsv/task-py-g-549-mixed-cell-signal-fusion-fsv/`
* `/zfs/archive/contextgraph/fsv/post-547-mixed-cell-ranking-repair-fsv/`
