# Sherlock Holmes #2 — Calibration shape and the active/abstain gate

## 1. Mandate and scope

This investigation owns one angle: **the math by which evidence already present in the system is fused into a per-cell calibrated binary probability, and the math by which the active/abstain surface is currently scored, and the math by which those two interact with the ship-gate over rolling windows**. The operator's hypothesis is that the inputs (cross-embedder DDA signals, foundationality scalars, temporal trajectory features) are real and the gate failure is on the consumption side. I read the calibration stack as it is wired in the live tree and compared it to the literature on small-sample, group-conditional, multi-source calibration. The other two Sherlocks own raw-feature excavation and the foundationality / CP_φ / entropy loss inputs respectively — I stayed in lane. I do not propose any of the forbidden surfaces (sparse out-of-fold probability bins, blanket pass/context skill expansion, wider evidence-key ladders, sparse bins, final aggregate bins, score blending, logit calibration, ECE-overweighted selection, support-only shrinkage, holdout-selected thresholds, monotone logit calibration, affine/rank-lift blending, mean-bias calibration — all explicitly burned upstream).

---

## 2. Forensic map of the calibration stack

The system has **three architecturally distinct calibration paths** that the operator's recent ECE evidence treats as one. Disambiguating them is the first move of the investigation.

### 2.1 Path A — Conformal split-calibration over multi-test scores

- **File:** `crates/context-graph-mejepa/src/conformal.rs` (lines 1–355).
- **What it does:** Builds the *outcome set* {Pass, Fail, OutOfDistribution, Abstain} that the predictor commits to. The non-conformity score is the L1 absolute residual between the predicted per-test pass probabilities and the actual per-test labels (`non_conformity_score`, `conformal.rs:45-74`). `calibrate_tau` (`conformal.rs:76-103`) computes `tau` as the `⌈(n+1)(1-α)⌉` order statistic of the sorted scores. Default `α = 0.10` (`config.rs:113` constant `INFER_DEFAULT_ALPHA`).
- **What it does NOT do:** It does not produce a calibrated *probability of pass* per row. It produces a coverage-controlled **set** of allowable verdicts. The `oracle_pass_confidence` that flows into `verdict_assembly.rs:103` is the raw `predicted_oracle_pass` score, NOT a conformal-calibrated probability.
- **Stratification:** `calibrate` (`conformal.rs:105-171`) stratifies only by `Language` (Python only on the active grid). It does **not** stratify by `MutationCategory`, so the conformal `tau` is a global Python-wide quantile.
- **Cold-start:** `cold_start_conformal_policy` (`conformal.rs:26-43`) widens to `[0, 1]` until `oracle_outcome_count ≥ 1000`. On 300×8 with ~30 holdout rows per cell, every per-cell conformal interval that tried to be per-cell would be at maximum width — explaining why this surface is not where the lift can come from.

### 2.2 Path B — TCT panel-level threshold calibration (per-embedder slot Gτ)

- **File:** `crates/context-graph-mejepa-tct/src/calibrator.rs` (lines 1–217) and `shrinkage_engine.rs` (lines 1–138).
- **What it does:** Per-embedder `tau` chosen to bound a *false-positive rate* against known-good centroids, at target `TARGET_FPR = 0.05`, with `MIN_SAMPLES_PANEL_LEVEL = 30` and `MIN_SAMPLES_CHUNK_TYPE = 50` (`calibrator.rs:12-14`). `choose_threshold` (`calibrator.rs:180-217`) scans candidates in ascending cosine order and picks the highest candidate whose strict-less-than FPR is ≤ target.
- **Shrinkage** (`shrinkage_engine.rs:27-92`): A cell below `SHRINKAGE_FULL_THRESHOLD = 50` (lines 11-12) falls back to a fixed cascade: `OwnCell → LanguageAggregate → EntityAggregate → CategoryAggregate`. Below `SHRINKAGE_HARD_FLOOR = 5` it fails closed.
- **The first defect of consumption math:** The fallback cascade is **discrete**, not partial-pooling. A cell with 6 calibration rows pretends to be a language aggregate; a cell with 50 rows uses pure own-cell statistics. There is no smooth shrinkage strength between these two regimes. James-Stein / empirical Bayes literature ([Hoff 2025](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf), [Reid 2021](https://utstat.toronto.edu/reid/sta2212s/2021/LSIChapter1.pdf)) shows the optimal shrinkage weight is a continuous function of within-group / between-group variance; the current cascade implicitly forces shrinkage strength to 1.0 below the floor and 0.0 above it. This is the kind of step function that lifts correlation cheaply (using neighboring cells when starved) but **inflates ECE** in cells just above the floor (where own-cell statistics with N=51 are noisier than the partially-pooled estimate would be).
- **What it does NOT do:** It does not produce a binary pass probability. It produces a Gτ guard pass/fail per (mutation × entity × language × embedder) cell. The Gτ outcome is consumed by `verdict_assembly.rs:83-88` as `guard_violation_count`, which is a hard reject, not a calibrated probability.

### 2.3 Path C — Per-prediction-class ECE / reliability bins

- **File:** `crates/context-graph-mejepa/src/eval/per_head_calibration_tracker.rs` (lines 1–274).
- **What it does:** Aggregates predictions over the whole holdout into three buckets — `q2_oracle_pass`, `q2_oracle_fail`, `ood_rejection` — and computes equal-width 10-bin reliability diagrams (`DEFAULT_CALIBRATION_BIN_COUNT = 10`, `DEFAULT_ECE_TOLERANCE = 0.02`, `per_head_calibration_tracker.rs:10-11`).
- **ECE formula:** Standard binned ECE — `expected_calibration_error += (count_b / N) · |mean_confidence_b − accuracy_b|` (lines 221-231).
- **Where the ECE numbers in the FSV come from:** This path. The `0.0240316074 / 0.0169885978 / 0.0402133949` numbers in roadmap doc `08 §0.5` are produced by this single global-bucket binner. **There is no per-cell ECE.** A per-cell ECE measurement does not exist in the eval output; the system only tracks correlation per cell.

### 2.4 Path D — OOD score sigmoid (`ood.rs`)

- **File:** `crates/context-graph-mejepa/src/ood.rs` (lines 1–187). `ood_score_from_norm_sq` (lines 32-37) maps `‖predicted - target‖²` through `1 - exp(-norm_sq / σ²)`. `tune_sigma_squared` (lines 39-90) binary-searches `σ²` to hit a target mean OOD on calibration set. This is the only place the system has any **continuous mapping from a distance to a probability**, and it does not feed the binary head's `predicted_oracle_pass` score at all — it only feeds the OOD-vs-in-distribution gate.

### 2.5 What features the calibrator sees vs what features the predictor sees

This is the load-bearing observation. The **predictor** consumes:

- 12 active per-embedder slot vectors (slot-preserving panel; doctrine §6.2 protected; `head_projection.rs:260-303` defines per-head feature lists drawn from `per_embedder_cosine`, `pairwise_cosine_upper`, and a tiny set of scalars per head).
- DDA scalars: `per_embedder_cosine`, `pairwise_cosine_upper`, `pairwise_mi_upper`, `blind_spot_z_scores` (`head_projection.rs:339-358`).
- For the Panel head specifically (the one whose calibrated_confidence drives `oracle_pass_confidence`): `cosine_to_centroid:E_AST`, `cosine_to_centroid:E_DIFF`, `cosine_to_centroid:E_Oracle`, `pairwise_cosine:E_AST:E_DIFF` (`head_projection.rs:262-267`). **Four features.**

The **calibrator** sees:

- Path A: an L1 residual of per-test pass probabilities vs actual labels. No DDA, no foundationality, no trajectory.
- Path B: the cosine between the slot vector and the cell centroid. No DDA cross-pair, no foundationality, no trajectory.
- Path C: only the scalar `predicted_oracle_pass` and `ood_score`. **The calibration of confidence is conditioned only on the confidence itself.**

**The asymmetry is structural.** The predictor learns a high-dimensional projection of slot evidence into a score in `[0,1]`. The calibrator then asks "how often, at the global level, has score = 0.85 corresponded to actual = pass?" That mapping is fitted **with no awareness of the features the predictor used**. Under any distribution shift in *which* features were dominant (and trajectory features are exactly a new feature group), the calibrator's mapping ages instantly: the predictor now puts mass at 0.85 because of trajectory evidence, but the historical 0.85-bucket accuracy was learned under pre-trajectory feature mix. Standard literature on this is unambiguous — see [Park et al. 2020, "Calibrated Prediction with Covariate Shift via Unsupervised Domain Adaptation"](http://proceedings.mlr.press/v108/park20b/park20b.pdf): "as covariate drift grows, standard models exhibit roughly linear degradation in accuracy and calibration." The covariate here is the *feature mix used to produce a given confidence value*.

### 2.6 Active/abstain has no calibration path of its own

`crates/context-graph-mejepa/src/eval/types.rs:357-363` exposes `ActiveLearningSummary` with only three counters (`queued_count`, `evicted_count`, `ood_escalation_count`). The latest ship-gate FSV (`run-20260521T132308Z/`) reports the `205 / 240` active-row sub-population metrics, but I cannot find any code path that re-calibrates Path C reliability bins **conditional on active**. The active surface piggybacks on the existing global ECE: the 0.0169885978 number is the binner of `per_head_calibration_tracker.rs` re-run on a filtered observation list, not a Mondrian-style per-sub-population calibrator.

The verdict-assembly side does carry abstain decisions (`compiler.rs:1119-1140` cold-cell abstain; `verdict_assembly.rs:96-101` constellation-pressure abstain; `abstain_for_cold_cell` at `compiler.rs:2164-2191`), but these set `calibrated_confidence = 0.0` (line 2176) and `degraded_status = true` (line 2177). They produce **no calibration information** that re-enters the binner. They are purely operational.

### 2.7 Where Brier and ECE are measured

- Brier: only at the aggregate / per-class level via `per_head_calibration_tracker.rs::compute_calibration_for_samples` infrastructure (Brier itself is not computed there — it is computed in the FSV harness from the per-row `(prob, label)` pairs, then attached as a top-level scalar; the report ECE in §0.5 of the roadmap is read from this scalar path).
- ECE: only the global binned ECE at the prediction-class level (path C).
- **Per-cell:** neither Brier nor ECE is computed per cell. `EvalReport.per_cell_correlation` exists; `EvalReport.per_cell_brier` / `EvalReport.per_cell_ece` do not. This is the largest structural reason the operator cannot see *where* the ECE inflation is happening.

---

## 3. Root cause hypothesis for the #392 ECE inflation

The #392 experiments lifted correlation `0.7942 → 0.8939` while inflating ECE `0.0240 → 0.0402`. Four candidate mechanisms, ranked by plausibility against the evidence:

### H1 (strongest, 45% prior) — Feature-mix covariate shift broke the global binner's monotonic mapping

The Path C binner assumes the function `confidence → P(pass | confidence)` is stable across re-trainings. Trajectory features re-route which slot-pairs dominate the predictor's confidence output. Under [Gong et al. 2021, "Confidence Calibration for Domain Generalization under Covariate Shift"](https://arxiv.org/pdf/2104.00742), the binner's `mean_confidence_b − accuracy_b` term is biased non-uniformly across bins after a feature-mix shift: bins that used to be populated mainly by easy AST-only cases now also contain trajectory-supported cases with different actual-accuracy. Bin-level ECE inflates even when both correlation and overall accuracy improve. **Why this fits the data:** correlation rose (the predictor's *ordering* improved), accuracy stayed flat or rose (`0.9375` in the cause-bin run), but ECE inflated — exactly the signature of a calibration mapping aging out under a working feature shift.

### H2 (strong, 30% prior) — Cause-bin calibration injected non-monotonicity that the global binner cannot unwind

The #392 evidence-semantics hardening run selected `cause-bin calibration` at ECE 0.0402 over mean-bias at 0.0299 because correlation lifted. Cause-bin is a **piecewise** correction over cause clusters. The Path C ECE binner is then computed over the corrected scores. A piecewise corrector applied before a 10-bin equal-width binner can create **bin boundary jumps**: a row whose cause-bin correction moves it from `0.78 → 0.82` crosses from bin 7 to bin 8 with no other context, producing local non-monotonicities in `mean_confidence_b` versus `accuracy_b`. The literature on this is [Naeini et al. 2015, "Obtaining Well Calibrated Probabilities Using Bayesian Binning"](https://pmc.ncbi.nlm.nih.gov/articles/PMC4410090/): "by considering only one fixed bin discretization, one may not be able to guess correctly the optimal bin width" — they propose Bayesian model averaging over many binnings precisely to avoid this artifact. The current binner is a fixed-bin estimator with no averaging.

### H3 (moderate, 15% prior) — Mixed-cell sparsity inflates per-cell sigmoid tails

The #548 typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED` is direct evidence. The system has ~30 calibration rows per (mutation × language) cell. Once trajectory features push the predictor toward sharper confidences (which is what makes correlation rise), the tails of the global score distribution become more populated. With 30 rows per cell and a high-confidence tail, even one miscalibrated row in a cell can shift the bin's `accuracy_b` by 1/n_bin (often > 0.1). [Tibshirani / Ding 2023, "Class-Conditional Conformal Prediction With Many Classes"](https://www.stat.berkeley.edu/~ryantibs/papers/classconf.pdf) reports the analogous problem in conformal: per-class quantiles "produce noisy quantile estimates due to limited data, leading to erratic behavior such as large sets." The equivalent here: per-cell empirical bin accuracies are noisy enough that the aggregate binned ECE is dominated by the noisiest cells.

### H4 (weaker, 8% prior) — Holdout-leakage prevention forced wider conformal intervals

Path A is constrained by `min_samples_per_stratum` and the global Python-level tau. As trajectory features pushed individual predictions away from the calibration-set distribution, more rows fell into the conformal outcome set `{OOD, Abstain}` than before; rows that were previously crisp Pass became `{Pass, Abstain}` sets. The active/abstain ratio `205 / 240 = 85.4%` suggests ~35 rows are now in an abstain-class. If those abstained rows contributed Brier mass at `0.5` in the binner, they would inflate ECE. **Why weaker:** active subset ECE is `0.0169` (below baseline) which suggests the abstained rows are NOT what's hurting ECE — they would normally be excluded. So H4 explains only the residual non-active ECE.

### H5 (weakest, 2% prior) — Active/abstain selection bias on the gate metric

A subtle confound: when 35 rows are abstained from the active subpopulation, the binner has 35 fewer rows. Confidence intervals on per-bin accuracy widen by √(240/205) ≈ 1.08. If the selection is informative (the abstained rows are the easy-mixed-feature rows the predictor was unsure about and pushed to abstain), then the binner over the active subset is computing accuracy on a **selected** sub-population. Coverage curves are well-known to magnify calibration error if abstention is not coverage-calibrated ([Liu et al. 2023, "Selective Classification Can Magnify Disparities Across Groups"](https://arxiv.org/pdf/2010.14134)). The 0.0169 active ECE on a selected sub-population is **not directly comparable** to the 0.0240 baseline computed on the full population — the gate is currently comparing apples to oranges.

**Composite conclusion.** H1 + H2 together explain most of the ECE inflation: the binner is a fixed 10-bin estimator that assumes confidence semantics are stable, but trajectory features and cause-bin calibration both shifted those semantics. H3 explains why per-cell metrics fail even when the aggregate is fine. H5 is the gate-design issue that needs to be addressed before active ECE is allowed to count for the ship gate.

---

## 4. External research synthesis

### Calibration methods that survive small per-cell sample sizes

- **Beta calibration** ([Kull, Silva Filho, Flach 2017, "Beta calibration: a well-founded and easily implemented improvement on logistic calibration for binary classifiers"](https://www.abzu.ai/data-science/calibration-introduction-part-2/); also [Abzu's calibration tutorial part II](https://www.abzu.ai/data-science/calibration-introduction-part-2/)). Two-parameter parametric (a, b, c), strictly more flexible than Platt scaling, fits well at N ≈ 30 because of low parameter count, monotone-by-construction so it cannot create the H2 binner artifact. **Concrete recommendation:** beta calibration per cell with empirical-Bayes shrinkage on (a, b) across cells.
- **Venn-Abers predictors** ([Vovk & Petej 2014, "Venn-Abers predictors"](https://arxiv.org/pdf/1211.0025); [Toccaceli 2017 COPA tutorial](https://cml.rhul.ac.uk/copa2017/presentations/VennTutorialCOPA2017.pdf)). The only post-hoc calibrator with a **provable** calibration guarantee under exchangeability. Returns a *pair* of probabilities (lower / upper) per row. **Why this matters here:** the operator-facing surface would carry a calibration interval per row natively, exposing the underpowered cells as wide intervals rather than miscalibrated point probabilities. Fits the existing conformal-set output shape.
- **Bayesian Binning into Quantiles (BBQ)** ([Naeini, Cooper, Hauskrecht 2015](https://pmc.ncbi.nlm.nih.gov/articles/PMC4410090/)). Averages over many equal-frequency binnings via Bayesian model averaging. Closed-form marginal under beta-binomial. Resolves the H2 bin-boundary artifact. The current Path C binner is the degenerate "one fixed binning" case BBQ explicitly fixes.
- **Smooth ECE / kernel-smoothed calibration** ([Błasiok & Nakkiran 2024, "Smooth ECE: Principled Reliability Diagrams via Kernel Smoothing"](https://arxiv.org/html/2309.12236); [apple/ml-calibration relplot](https://github.com/apple/ml-calibration)). Replaces the fixed-bin ECE with RBF-kernel-smoothed ECE. Hyperparameter-free (bandwidth auto-chosen). Removes the binning artifact entirely. The accompanying paper ["How Flawed Is ECE? An Analysis via Logit Smoothing"](https://arxiv.org/html/2402.10046) proves the fixed-bin ECE can yield false positives — exactly the failure mode that may be inflating the operator's current numbers.

### Conformal prediction at small group-conditional calibration set sizes

- **Mondrian Conformal Prediction** ([Vovk et al.; MAPIE 1.0 Mondrian docs](https://mapie.readthedocs.io/en/latest/theoretical_description_mondrian.html); [Boström & Johansson 2021, "Mondrian Conformal Predictive Distributions"](https://proceedings.mlr.press/v152/bostrom21a/bostrom21a.pdf)). Per-group split conformal with group-conditional coverage guarantee. The natural fit for per-cell coverage but breaks down under small group sizes (which is exactly the #548 blocker).
- **Class-Conditional Conformal Prediction with Many Classes / Clustered Conformal** ([Ding, Angelopoulos, Bates, Jordan, Tibshirani 2023](https://www.stat.berkeley.edu/~ryantibs/papers/classconf.pdf)). The closest analogue to the operator's situation: "Standard conformal prediction provides marginal coverage guarantees even in limited-data settings but does not yield class-conditional coverage, while classwise conformal prediction achieves class-conditional coverage but produces noisy quantile estimates due to limited data." The fix is **cluster classes by conformal-score similarity, run conformal at cluster level, get cluster-conditional coverage**. Direct analogue: cluster mutation-category cells by their non-conformity-score distribution, run conformal per cluster — promises cluster-conditional (not cell-conditional, but better than marginal) coverage at N≈30/cell.
- **Conformal Risk Control** ([Angelopoulos, Bates, Fisch, Lei, Schuster 2024](https://arxiv.org/html/2208.02814v4)) and **Selective Conformal Risk Control** ([2025 arXiv 2512.12844](https://arxiv.org/html/2512.12844v1)). Generalizes split conformal to any monotone loss; selective variant lets you control conditional risk on the **accepted** subpopulation while bounding coverage — directly applicable to the active/abstain surface.

### Empirical Bayes / hierarchical calibration for the underpowered cells

- **James-Stein and Empirical Bayes** ([Hoff 2025, "Shrinkage and Empirical Bayes" lecture notes](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf); [Reid 2021, "Empirical Bayes and the James-Stein Estimator"](https://utstat.toronto.edu/reid/sta2212s/2021/LSIChapter1.pdf); [Berkeley's Stat210A on James-Stein](https://stat210a.berkeley.edu/fall-2024/reader/jamesstein.html)). The classical result: when estimating multiple cell-specific parameters, the James-Stein estimator dominates the per-cell MLE in mean squared error. The shrinkage factor is `1 − (k−2)σ²/‖x̄‖²` for k cells with within-cell variance σ². Closed-form, no holdout needed.
- **Empirical Bayes for multiple sample sizes** ([Chris Said 2017](https://chris-said.io/2017/05/03/empirical-bayes-for-multiple-sample-sizes/)). Practical treatment of heteroscedastic-bin empirical Bayes for proportions, which is exactly the per-cell pass-rate / per-cell ECE setup.

### Selective classification and abstention

- **SelectiveNet** ([Geifman & El-Yaniv 2019, "SelectiveNet: A Deep Neural Network with an Integrated Reject Option"](https://arxiv.org/pdf/1901.09192)). Joint training of predictor + selector + auxiliary head; given target coverage `c`, optimizes accuracy over the top-c · 100% most confident inputs. Risk-coverage trade-off is the operator surface.
- **Selective Recalibration** ([Improving Predictor Reliability with Selective Recalibration, OpenReview](https://openreview.net/pdf?id=Aoj9H6jl6F)). Re-calibrates probabilities conditionally on the selection rule. Directly addresses H5 — active ECE measured against the selected sub-population, not the full population.
- **Coverage-magnified disparities** ([Liu et al. 2020, "Selective Classification Can Magnify Disparities Across Groups"](https://arxiv.org/pdf/2010.14134)). Warns that selection rules can hide poor calibration in sub-groups by abstaining specifically on hard sub-group members.
- **Confidence-calibrated covariate shift correction** ([Wenger et al. 2020-ish work and follow-ups, e.g. 2025 arXiv 2502.07847](https://arxiv.org/html/2502.07847)).

### Multi-source / multi-classifier evidence fusion

- **Dempster-Shafer classifier fusion** ([Denoeux et al. 2024, "Feature Fusion for Improved Classification: Combining Dempster-Shafer Theory and Multiple CNN Architectures"](https://arxiv.org/pdf/2405.20230); [Liu et al. 2020, "A novel multi-classifier information fusion based on Dempster-Shafer theory"](https://arxiv.org/pdf/2012.02481); [Tong et al. 2021, "An evidential classifier based on Dempster-Shafer theory and deep learning"](https://www.sciencedirect.com/science/article/abs/pii/S0925231221004525)). Belief functions can mass-assign over sets of hypotheses including {Pass, Fail, Abstain} simultaneously, with explicit conflict-mass that distinguishes "evidence agrees" from "no evidence."
- **Log-linear opinion pooling** ([Bayesian Classifier Fusion with an Explicit Model of Correlation, arXiv 2106.01770](https://arxiv.org/pdf/2106.01770); [Combining Expert Opinions, Cambridge Episteme](https://www.cambridge.org/core/journals/episteme/article/an-interpretation-of-weights-in-linear-opinion-pooling/531946371BB0BF93399E70EA60C824C7)). The standard principled way to fuse multiple calibrated classifiers without re-introducing correlated-source double counting: weighted geometric mean of probabilities, where weights are elicited from per-source reliability.
- **Mukhoti 2020 focal loss for calibration** ([Calibrating Deep Neural Networks using Focal Loss, NeurIPS 2020](https://omegafragger.github.io/publication/calibration_neurips_2020/); [follow-up Tao et al. 2023 "Dual Focal Loss for Calibration"](https://proceedings.mlr.press/v202/tao23a/tao23a.pdf); [Improving Calibration by Relating Focal Loss, Temperature Scaling, and Properness, arXiv 2408.11598](https://arxiv.org/html/2408.11598v1)). Loss-side fix; would re-train the predictor. Out of scope for the consumption-side ask but is the cleanest training-time path if the operator wants to revisit it.
- **Pereyra confidence penalty / label smoothing** ([Pereyra et al. 2017; review at arXiv 2402.03979](https://arxiv.org/html/2402.03979); [arXiv 2508.00264, "Calibrated Language Models and How to Find Them with Label Smoothing"](https://arxiv.org/html/2508.00264v1)). Loss-side regularizer; lower ECE without sacrificing accuracy. Same caveat — training-time.

### Why the typical "just use isotonic" answer is wrong here

[Multiple sources](https://www.abzu.ai/data-science/calibration-introduction-part-2/) — including the survey [arXiv 2601.19944, "Classifier Calibration at Scale"](https://arxiv.org/html/2601.19944) — agree: **isotonic regression overfits at N ≈ 30 per cell**. The literature near-unanimously prefers parametric (Platt / beta) or Bayesian (Venn-Abers, BBQ) calibrators at the operator's per-cell sample size.

---

## 5. Proposed improvements (seven concrete, doctrine-compatible)

Each proposal: mechanism, pipeline placement, forbidden-surface compliance, per-cell power requirement, active/abstain compatibility, falsification test, storage cost.

### Proposal P1 — Replace global binner with kernel-smoothed ECE (Smooth ECE) plus per-cell drill-down

**Mechanism.** Replace the fixed 10-bin estimator at `eval/per_head_calibration_tracker.rs:115-176` with the [SmoothECE](https://arxiv.org/html/2309.12236) estimator: kernel-density-smoothed reliability diagram, no bin choice, no boundary artifacts. Implement per-cell SmoothECE additionally, conditional on cell support ≥ a power floor (10 rows, with empirical-Bayes shrinkage below — see P4). Both old binned ECE and new SmoothECE are computed and persisted in `EvalReport`, but **SmoothECE replaces binned ECE as the gate metric** once the FSV evidence shows the binned number was a false-positive in the H2 bin-boundary sense.

**Pipeline placement.** Drop in at `eval/per_head_calibration_tracker.rs:178-256`, expose `compute_smooth_ece_for_samples` alongside the current `compute_calibration_for_samples`. Add `pub per_cell_smooth_ece: BTreeMap<String, Option<f32>>` to `EvalReport`.

**Forbidden compliance.** ✓ Not sparse-bin; not aggregate-bin (smooths over all data); not holdout-tuned (bandwidth is data-driven, [Błasiok–Nakkiran 2024 proves it's hyperparameter-free in the relevant sense](https://arxiv.org/html/2309.12236)); not blend; not logit-cal; not selection.

**Power requirement.** SmoothECE on N=240 is well-defined; per-cell SmoothECE needs ≥ 10 rows (below that, return `None` and surface as `MIXED_CELL_CALIBRATION_UNDERPOWERED`).

**Active/abstain compatibility.** Works on both. The active sub-population SmoothECE will be directly comparable to the full-population SmoothECE because the kernel-smoothed estimator doesn't suffer the bin-population shift artifact.

**Falsification test.** Compute SmoothECE on the `#536` baseline run and on the `#392 evidence-semantics hardening` run. If SmoothECE moved in the *opposite* direction of binned ECE (i.e., it stayed flat or improved), the binned ECE was a false-positive artifact (H2 confirmed). If both moved the same way, H2 is rejected and H1/H3 dominate.

**Storage.** No new CF — extend `CF_MEJEPA_CALIBRATION_HISTORY` row schema with new field.

### Proposal P2 — Add Venn-Abers calibration as the operator-facing surface (replaces calibrated_confidence point estimate)

**Mechanism.** Per (Python-only) mutation cell, fit two isotonic regressions ([Vovk-Petej Venn-Abers](https://arxiv.org/pdf/1211.0025); reference impl: [github.com/ip200/venn-abers](https://github.com/ip200/venn-abers)): one over rows with actual=Pass, one over actual=Fail. At inference, return `(p_lower, p_upper) = (g_0(s), g_1(s))`. Surface as `RealityPrediction.calibration_interval` alongside the existing `confidence_interval` (which is the conformal interval over outcome sets, not a probability interval).

**Pipeline placement.** Insert after `compiler.rs:compile_with_panel` returns the prediction and before the verdict is assembled. The `oracle_pass_confidence` fed into `verdict_assembly.rs:103` becomes the midpoint of the V-A interval; the width is an extra input to the abstain decision.

**Forbidden compliance.** ✓ Venn-Abers is provably calibrated under exchangeability ([Toccaceli 2017 tutorial](https://cml.rhul.ac.uk/copa2017/presentations/VennTutorialCOPA2017.pdf)); the two isotonic regressors fit at N=30 per cell will be noisy, which is why P3 (empirical Bayes shrinkage) is the necessary partner.

**Power requirement.** V-A needs ≥ 20 calibration rows per cell to be informative; below that, fall back to the language-level V-A pair plus a cell-specific shift estimated under P4.

**Active/abstain compatibility.** Native: a wide V-A interval (e.g., `[0.45, 0.75]` for `oracle_pass_confidence = 0.6`) is itself the abstain signal. The active surface becomes "row commits iff V-A interval width ≤ threshold."

**Falsification test.** Empirical V-A coverage on the holdout: the actual oracle outcome falls inside `[p_lower, p_upper]` at least `100 · (1 − α)%` of the time. If it does not, V-A's exchangeability assumption is violated and the active/abstain coverage curve is unreliable.

**Storage.** New CF `CF_MEJEPA_VENN_ABERS_CELLS` keyed by cell_id, value = serialized isotonic-pair (small; max a few KB per cell).

### Proposal P3 — Beta-calibration head per cell with empirical-Bayes shrinkage on (a, b)

**Mechanism.** Per cell, fit a 3-parameter beta-calibration `g(s) = 1 / (1 + 1/(c · s^a · (1-s)^{-b}))` ([Kull, Silva Filho, Flach 2017](https://www.abzu.ai/data-science/calibration-introduction-part-2/)). Per-cell parameter triples shrunk James-Stein-style toward the language-level fit. For cell *i* with n_i calibration rows: `θ̂_i = w_i · θ̂_i^{MLE} + (1 − w_i) · θ̂_lang` where `w_i = n_i / (n_i + κ)` and κ is the empirical-Bayes "prior strength" estimated from cross-cell variance ([Hoff 2025](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf) §3-4; closed form). This is **smooth shrinkage**, not a discrete cascade.

**Pipeline placement.** New module `crates/context-graph-mejepa/src/calibration_beta.rs`. Called from `compile_with_panel` to convert raw `oracle_pass_confidence` into calibrated probability. Reads cell-id from the panel's `(mutation_category, language)` tuple.

**Forbidden compliance.** ✓ Beta calibration is monotone — produces a strictly increasing mapping, so it cannot reverse ranks (compatible with the #547 / #549 rank-reversal block). It is **not** sparse-bin (continuous parametric). It is not logit cal (it's a beta sigmoid). It is not score blend (operates on a single score). EB shrinkage uses no holdout (κ estimated from calibration-set cross-cell variance).

**Power requirement.** N_i ≥ 5 per cell to fit MLE; below 5 use pure language-level estimate. The shrinkage weight w_i is automatically small for small n_i.

**Active/abstain compatibility.** Beta calibration produces a single point probability; active/abstain is decided by V-A interval width (P2) or by predictor-side OOD score (existing path).

**Falsification test.** Train beta-calibration on the 240-row calibration split, evaluate on the 240-row holdout. If holdout ECE (SmoothECE under P1) does not improve over the current Path C number, the parametric assumption is wrong.

**Storage.** New CF `CF_MEJEPA_BETA_CALIBRATION_CELLS`. Tiny — 3 floats + cell key per cell.

### Proposal P4 — Smooth shrinkage replacement for the discrete TCT cascade

**Mechanism.** Replace `shrinkage_engine.rs::apply_shrinkage` (lines 27-92) discrete-cascade logic with a James-Stein-style closed-form partial pooling. For each cell centroid `c_i` with within-cell variance σ²_i estimated from `bucket.sum / bucket.count`, the shrunk centroid is `ĉ_i = w_i · c_i + (1 − w_i) · c_lang_aggregate` where `w_i = n_i · σ²_lang / (n_i · σ²_lang + k · σ²_i)` and k is the number of cells. Same `SHRINKAGE_HARD_FLOOR = 5` lower bound, no upper threshold (smooth all the way to N = 50 and beyond).

**Pipeline placement.** `crates/context-graph-mejepa-tct/src/shrinkage_engine.rs:27-92`.

**Forbidden compliance.** ✓ Pure cross-cell pooling, no holdout, no rank reversal. Monotone in the trivial sense (a cell with more data gets weighted more toward its own centroid).

**Power requirement.** Same as current — `SHRINKAGE_HARD_FLOOR = 5`. Cells below floor still fail closed.

**Active/abstain compatibility.** TCT shrinkage feeds the Gτ guard, which is a hard reject in verdict_assembly. Smoother centroids → fewer false rejects → fewer abstains for the wrong reason → more rows available for the calibrator.

**Falsification test.** Run the existing Phase B/C TCT calibration FSV harness, compare per-cell Gτ pass rate before and after. Expected: pass rate strictly higher on cells with 5 ≤ n_i < 50 (the ones that were being demoted to language-aggregate); unchanged on cells with n_i ≥ 50.

**Storage.** No new CF.

### Proposal P5 — Clustered conformal prediction at the cell level

**Mechanism.** [Ding, Angelopoulos, Bates, Jordan, Tibshirani 2023, "Class-Conditional Conformal Prediction with Many Classes"](https://www.stat.berkeley.edu/~ryantibs/papers/classconf.pdf). Cluster the 8 mutation cells × Python by similarity of their per-cell non-conformity-score distribution (k-means or hierarchical with k ≈ 3-4). Run conformal split at the cluster level instead of marginally over all Python rows. This gives cluster-conditional coverage with N_cluster ≈ 60-80 rows per cluster, well above the 30-row noise floor.

**Pipeline placement.** Modify `crates/context-graph-mejepa/src/conformal.rs:calibrate` (lines 105-171) to stratify by cluster (computed deterministically from calibration-set non-conformity scores), not just by language.

**Forbidden compliance.** ✓ Per the Tibshirani paper's marginal-vs-classwise discussion: this is the *Goodhart-safer* generalization of per-cell conformal (better coverage than marginal, less noise than per-cell). Not sparse-bin, not threshold-tune, not holdout-tune (clustering computed only on calibration partition).

**Power requirement.** k ≈ 3 clusters → N ≈ 80 calibration rows per cluster, which clears the 30-row noise floor.

**Active/abstain compatibility.** Provides per-cluster conformal intervals; active rows have narrower intervals.

**Falsification test.** Hold out one mutation cell entirely, fit clustering on the remaining 7. The held-out cell's conformal coverage must be within 5% of the target (1 − α = 0.9). If not, clustering is over-specializing.

**Storage.** New CF `CF_MEJEPA_CONFORMAL_CLUSTER_ASSIGNMENT`. Tiny — cell_id → cluster_id.

### Proposal P6 — Log-linear pooling of per-head calibrated probabilities

**Mechanism.** The 10 prediction heads (`HeadId::ALL`) each produce a per-head score consumed only via `head_projection`. Currently their outputs are NOT fused into a probability — the predictor head produces `predicted_oracle_pass` from its own learned weights. Replace the implicit fusion with **calibrated log-linear pool**: per-head V-A calibration (P2), then fuse the calibrated probabilities by weighted geometric mean `p_pool = Π p_h^{w_h}` with weights from per-head reliability ([Cambridge Episteme, "An Interpretation of Weights in Linear Opinion Pooling"](https://www.cambridge.org/core/journals/episteme/article/an-interpretation-of-weights-in-linear-opinion-pooling/531946371BB0BF93399E70EA60C824C7); [Bayesian Classifier Fusion with an Explicit Model of Correlation, arXiv 2106.01770](https://arxiv.org/pdf/2106.01770)). Heads with high test-time agreement get downweighted (correlation-aware).

**Pipeline placement.** New module `crates/context-graph-mejepa/src/fusion_pool.rs` invoked after per-head V-A. Replaces the current `oracle_pass_confidence` input to `verdict_assembly.rs:103`.

**Forbidden compliance.** ✓ Log-linear pooling is monotone (weighted geometric mean preserves order), not sparse-bin, not aggregate-bin, not logit cal, not holdout-tuned (weights estimated from calibration-split per-head reliability).

**Power requirement.** Per-head V-A needs P2 per-cell support; the pool itself is fine on any head set with at least 2 heads.

**Active/abstain compatibility.** Native — a row where the heads disagree (high pool-conflict) is a natural abstain candidate. The pool conflict measure equals the cross-head Jensen-Shannon divergence, directly usable as the active/abstain threshold.

**Falsification test.** Ablation: fix all per-head probabilities to 0.5; check that pool returns 0.5 (sanity). Fix all heads to actual labels; check that pool achieves SmoothECE → 0.

**Storage.** New CF `CF_MEJEPA_HEAD_FUSION_WEIGHTS`. Tiny — 10 floats.

### Proposal P7 — Selective conformal risk control for the active/abstain gate

**Mechanism.** [Angelopoulos, Bates et al. 2024 Conformal Risk Control](https://arxiv.org/html/2208.02814v4) + [Selective Conformal Risk Control 2025](https://arxiv.org/html/2512.12844v1). Two-stage: (1) selector decides commit/abstain on each row using V-A interval width (P2) or pool disagreement (P6); (2) on the selected subpopulation, conformal risk control bounds the *conditional* miscalibration. The ship gate then asks: "what coverage level keeps active SmoothECE ≤ target, and what is the per-cell active correlation at that coverage?" — a coverage curve `(c, SmoothECE_c, corr_c)`.

The per-cell active gate becomes "at coverage `c_cell`, per-cell-active SmoothECE ≤ τ_cell AND per-cell-active correlation ≥ 0.95." Each cell can choose its operating coverage as long as it discloses it. This unlocks the underpowered cells: an `off_by_one` cell with 20 mixed rows can commit on its 12 highest-V-A-confidence rows, get correlation 0.95+ on those 12, and surface the remaining 8 as "abstain — no commit."

**Pipeline placement.** New module `crates/context-graph-mejepa/src/selective_gate.rs`. Reads V-A intervals (P2) per row, applies selector, computes per-cell coverage / SmoothECE / correlation curves, persists as `EvalReport.per_cell_selective_curve`.

**Forbidden compliance.** ✓ Critical: this is NOT holdout threshold tuning. The coverage threshold per cell is decided on the calibration partition by minimizing conditional risk subject to a fixed marginal coverage budget. The active subset is a function of V-A interval width (calibration-fit), not of holdout label.

**Power requirement.** Each cell must have ≥ 10 calibration rows for the conditional risk bound to be meaningful.

**Active/abstain compatibility.** This **is** the active/abstain framing. The ship gate explicitly reasons about coverage and conditional ECE / correlation, defending against H5.

**Falsification test.** Liu et al. 2020 group-disparity test: verify that the abstained sub-population does not have a *systematic* failure-mode-class bias (precision/recall computed on abstained rows ≈ overall precision/recall, not skewed toward a particular failure class).

**Storage.** New CF `CF_MEJEPA_SELECTIVE_COVERAGE_CURVES`. Per cell: ~50 (coverage, smoothECE, correlation) triples → ~600 bytes per cell, ~5KB per Python window.

---

## 6. Empirical Bayes pooling proposal (formal closed-form)

The current TCT shrinkage (`shrinkage_engine.rs:27-92`) is a discrete cascade. The closed-form James-Stein partial pooling replacement:

### Setup

- k cells (here k = 8 Python mutation cells × 12 embedders × 4 entity types ≈ 384 cells, with most underpowered).
- Cell *i*: n_i calibration rows, observed centroid `c_i ∈ S^d`, within-cell variance σ²_i (estimated as `tr(Cov) / d`).
- Language-aggregate centroid `c_lang` (computed over all rows in the same language), with between-cell variance τ² = `Var_i(c_i)`.

### James-Stein-style shrunk centroid

For each cell *i*:

```
ĉ_i = w_i · c_i + (1 - w_i) · c_lang

w_i = (n_i / σ²_i) / (n_i / σ²_i + 1 / τ²)
```

This is the Bayes posterior mean under a normal-normal hierarchical model: `c_i ~ N(μ_lang, τ² · I)`, observations `~ N(c_i, σ²_i · I / n_i)`. Closed form. No holdout. Reduces to:

- `w_i → 1` (no shrinkage) when n_i is large or σ²_i is small (cell is well-informed about itself).
- `w_i → 0` (full pooling) when n_i is small or σ²_i is large (cell knows little; defer to language).

### Estimating τ²

[Hoff 2025 §3.2](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf) — empirical Bayes estimator:

```
τ̂² = max(0, (1/(k-1)) Σ_i (c_i - c̄)² · (c_i - c̄)²  -  (1/k) Σ_i σ²_i / n_i)
```

Computed once per calibration cycle from the calibration split. Stored in `CF_MEJEPA_EMPIRICAL_BAYES_PRIOR` (one row per Python language × embedder, holding `τ̂²` and `μ̂_lang`).

### Why this is safer than the discrete cascade

- The discrete cascade `OwnCell → LanguageAggregate → EntityAggregate → CategoryAggregate` has step-function discontinuities at n_i = 50 and n_i = 5. A cell at n_i = 51 uses pure own-cell; a cell at n_i = 49 uses 100% language aggregate. The James-Stein estimator smooths this transition with w_i continuous in n_i.
- The discrete cascade also commits one of three different aggregates without justification. The James-Stein estimator commits to a single hierarchical level (language) and lets the data decide how much to use it. This is the [Hoff §3](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf) "stein-meta-loss-dominance" result: shrinkage with the partial-pool weight strictly dominates the per-cell MLE in expected squared error.
- It is **not** holdout-tuned: τ̂² is estimated from the calibration partition only.

### Falsification test

1. Compute current discrete-cascade centroids on the calibration partition; measure Gτ pass rate on the holdout partition.
2. Compute James-Stein-shrunk centroids on the same calibration partition; measure Gτ pass rate on the same holdout.
3. **Pass criterion:** the James-Stein version has strictly higher Gτ pass rate on cells with 5 ≤ n_i < 50, and within ±2% of current on cells with n_i ≥ 50. If it fails on n_i ≥ 50 cells, the hierarchical normal-normal model is mis-specified and the proposal is wrong.

---

## 7. Selective-classifier framing of the active/abstain surface

### The reframe

The active/abstain surface is currently a *post-hoc filter* on a predictor that committed to all rows. The literature on selective classification ([SelectiveNet, Geifman & El-Yaniv 2019](https://arxiv.org/pdf/1901.09192); [Selective Recalibration, OpenReview](https://openreview.net/pdf?id=Aoj9H6jl6F); [Uncertainty-Driven Reliability, arXiv 2508.07556](https://arxiv.org/pdf/2508.07556)) treats it as a **first-class joint optimization**: pair `(f, g)` where f predicts and g abstains, both trained / calibrated to minimize selective risk at a chosen coverage `c`.

### The ship-gate generalization

Define for each cell:

```
SelectiveCorrelation(c) = corr(prediction_oracle_pass, oracle) over top-c · 100% confidence rows
SelectiveSmoothECE(c)   = SmoothECE over top-c · 100% confidence rows
SelectiveCoverage(c)    = c
```

Plot the three curves over c ∈ [0.5, 1.0] per cell. Current 100%-coverage point: SelectiveCorrelation(1.0) = 0.7942, SelectiveSmoothECE(1.0) ≈ 0.024 (binned baseline). #392 active point: SelectiveCorrelation(0.854) = 0.9526, SelectiveSmoothECE(0.854) = ?

### Replacement gate rule

**Old gate:** "active correlation ≥ 0.95 on 100% of holdout rows, per cell, four windows."

**New gate (defensible):**

1. For each cell, find the smallest coverage `c_cell` such that SelectiveCorrelation(c_cell) ≥ 0.95 AND SelectiveSmoothECE(c_cell) ≤ τ_cell (where τ_cell is the calibration-derived smooth-ECE threshold for that cell).
2. The system commits to coverage `c_cell` per cell (operator visible).
3. Ship gate passes iff all cells have `c_cell ≥ c_min_floor` (where `c_min_floor` is a doctrine constant like 0.5 — the system must commit on at least half of any cell's rows).
4. **Cross-window check:** `c_cell` must be stable across 4 rolling windows (no oscillation between coverages).

### Why this defends against H5

[Liu et al. 2020](https://arxiv.org/pdf/2010.14134) proved coverage curves can hide sub-group disparity. The defense is per-cell-conditional coverage: a cell-balanced operating-point selection prevents the selector from systematically over-abstaining on one mutation category. The required-cell list (`required_active_python_ship_gate_cells`, `eval/types.rs:664-675`) is already in place; the gate becomes "each required cell has its own coverage report and meets cell-specific calibration thresholds."

### Per-cell gate-pass path that survives #548 `MIXED_CELL_CALIBRATION_UNDERPOWERED`

The underpowered cell currently fails because forcing 100% commit on N=30 rows with mixed outcomes produces a noisy per-cell Pearson estimate. Under selective:

- A cell with N=30 and a clean cell-conditional ranking can commit on its top 20 (highest V-A confidence) rows, achieving corr ≈ 0.97 on those 20 with high statistical power (lower bound of a Pearson CI at N=20 is much tighter than the same Pearson at N=30 with three outlier rows).
- The 10 abstained rows are not gate-blocking; they are explicitly surfaced as "abstain — system declines to commit."
- The operator-facing surface gains the coverage column: "for `delete_test_call::python`, we commit on 19 / 30 rows at corr 0.96, SmoothECE 0.018, and abstain on 11."

The active-population aggregate stays apples-to-apples comparable to the full-population baseline only because both SmoothECE estimators are population-size-corrected.

---

## 8. Mistake-loop / online-head adjustment recommendations

The online head (`crates/context-graph-mejepa-train/src/online_head_state.rs:181-295`) only handles binary verdicts (Pass / Fail) and operates under `ExactPanelKeyIsolation` (line 270). The active/abstain machinery introduces three new mistake categories:

1. **Active-mistake** — system committed (active), got it wrong. Existing path handles these.
2. **Abstain-mistake** — system abstained but the truth was binary. Two sub-types:
   - **Wrong abstain (false abstain):** ground truth was a clear Pass or Fail and the predictor had high V-A confidence; the abstain logic over-triggered. **High-value training signal for the selector**, not the predictor head.
   - **Right abstain:** ground truth was ambiguous (e.g., test was actually flaky); the system was correct to abstain. **No training update; quarantine the row.**

The current mistake-log row schema (`mistake_log.rs:610-line file`) does not carry a `(committed | abstain)` flag. I recommend:

### M-1. Extend MistakeLogRow with `commit_decision: CommitDecision { Active, Abstain }`

`crates/context-graph-mejepa-train/src/mistake_log.rs` — add a field. The replay buffer consumer would then split into two consumers:

- `consume_active_mistake` → updates the predictor's online head as today (`online_head_state.rs:181`).
- `consume_abstain_mistake` → updates the **selector's** threshold rather than the predictor's correction_bias.

### M-2. Selector-side online head (parallel structure)

The current `OnlineHeadStateRow` is keyed by `panel_signature_hash` and stores a binary `corrected_verdict`. A parallel `SelectorOnlineHeadStateRow` keyed by the same panel_signature_hash would store a `selector_threshold_correction` (a real-valued shift on the V-A interval-width threshold). Mistake of type "Wrong abstain" decrements the threshold (commit more aggressively); mistake of type "Active-mistake at high V-A confidence" increments it (commit more conservatively).

### M-3. Active mistakes contribute to per-cell beta-calibration (P3)

Each active-mistake row carries (predicted_score, actual_outcome, cell_id). These can incrementally update the per-cell beta-calibration (a, b, c) via online Newton steps without retraining from scratch. The empirical Bayes prior strength κ is held constant; only per-cell parameters shift. This is the trainer-side mechanism for the operator's `mistake_repeat_rate ≤ 0.05` target.

### M-4. Cite `online_head_state_keys.rs::unrelated_control_panel_signature_hash`

The existing isolation check (`online_head_state.rs:188-191`, using `unrelated_control_panel_signature_hash` from `online_head_state_keys.rs`) is the right pattern. Apply the same isolation to the selector head: when a mistake updates `panel_signature_hash = X`, verify an unrelated `Y` cell's commit/abstain rate is unchanged.

---

## 9. Forbidden-surface compliance checklist

| Proposal | Sparse OOF bins | Blanket skill expansion | Wider evidence ladders | Sparse bins | Final aggregate bins | Score blending | Logit calibration | ECE-overweighted selection | Support-only shrinkage | Holdout-selected thresholds | Monotone logit cal | Affine/rank-lift blending | Mean-bias calibration |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P1 SmoothECE | ✓ | ✓ | ✓ | ✓ (replaces binned) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| P2 Venn-Abers | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| P3 Beta-calib + EB | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (beta-sigmoid ≠ logit) | ✓ | ✓ (uses both N and σ²) | ✓ | ✓ | ✓ | ✓ |
| P4 JS shrinkage | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (uses cross-cell variance) | ✓ | ✓ | ✓ | ✓ |
| P5 Clustered conformal | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (clustering on calibration only) | ✓ | ✓ | ✓ |
| P6 Log-linear pool | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ Marginal risk — pool IS fusion, but it's calibrated per-head first, not score-blend on raw scores. Argue it's geometric-mean fusion of calibrated probabilities, not blend. | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| P7 Selective CRC | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (selector threshold from calibration partition only) | ✓ | ✓ | ✓ |

**Single watchpoint — P6.** The operator's `08 §0.5` forbids "score blending." Log-linear pooling IS a fusion mechanism, but the textbook distinction matters: it pools **calibrated probabilities** (post-V-A), not raw heterogeneous scores. If the operator reads "no score blending" as "no fusion of any kind," P6 must be deferred. If the operator reads it as "no raw-score blending," P6 is in. **Question for the operator** (raised below).

---

## 10. Honest risk register

### Things I don't know

- I have not measured per-cell sample counts in the live `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` artifact (operator instructed not to FSV). I inferred ~30 rows/cell from 240 / 8 mutation cells. If the actual distribution is heavily skewed (e.g., `compile_error` cell with 60 rows and `subtle_flip` with 12), the proposals' power requirements have different implications per cell. P4 and P7 explicitly need the operator to confirm per-cell N.
- I have not run any of these proposals. The H1 / H2 / H3 ranking is forensic, not empirical. The falsification test under P1 is specifically designed to discriminate H1 vs H2 once it is run.
- Whether the existing predictor's `predicted_oracle_pass` is mathematically appropriate as input to a beta-calibration. Beta calibration is most natural when the predictor's output is itself a probability estimate (i.e., the predictor was trained with a probabilistic loss). If `predicted_oracle_pass` is a learned score with no probabilistic interpretation, P3 still works (beta calibration on any monotone score is well-defined), but the fitted (a, b, c) may be more extreme.

### Things that could be wrong

- **P3 + P4 stack may double-shrink.** If the predictor's score is itself a function of TCT cells (P4 input), and the per-cell beta-calibration shrinks across cells (P3), the same data is being pooled twice. The correct architecture is **either** P4 shrinking the predictor inputs OR P3 shrinking the calibrator output, not both. If both are applied, the calibration map will be over-smoothed and miss true cell-specific signal. Resolution: estimate Σ(cell-specific signal) before-and-after P4; if it shrinks meaningfully, gate P3 to use only language-level priors, not cell-level priors.
- **P7's selector threshold can adapt around H1.** If the selector learns to abstain on exactly the rows where the calibration mapping has aged out, the active subset's SmoothECE will look great by construction. The Liu 2020 group-disparity test (`failure_mode_class` precision/recall on abstained subset) is the structural defense, but if the disparity test passes and ECE still looks "too good", we should suspect this loop. The cold-load FSV (`docs/05_predictor_pipeline.md:547` planned #514) is the right cross-check.
- **V-A intervals can collapse under perfect predictor.** If for some cell every calibration row has `confidence ≥ 0.9` and actual=Pass, both V-A isotonic regressors are trivial (constant 1.0). The interval collapses to `[1.0, 1.0]` and the active threshold accepts all rows — but this is the right answer if the cell is truly easy (e.g., `known_good`). The check is: a cell with collapsed V-A interval AND holdout-observed mixed outcomes is a calibration *failure*, not a calibration *success*. P7's falsification test catches this.
- **Cases where lowering ECE regresses correlation.** Any monotone post-hoc calibrator preserves the rank order of predictions, so per-row correlation is invariant to monotone calibration. P1 / P3 / P6 are monotone → no correlation regression possible. P4 (TCT shrinkage) can change the centroids and therefore the predictor's input features, which can move correlation. P5 (clustered conformal) changes outcome sets only, not point scores → correlation invariant. P7 changes the active subset → can change active correlation; the gate definition is explicitly per-coverage so this is expected.

### One self-contradiction I want flagged

The doctrine §6.2 says "slot identity is sacred" and forbids cross-embedder dim-wise compare. P6 (log-linear pool) pools per-head probabilities, not per-slot vectors — so it does not violate §6.2. **But:** P3 / P4 work at the calibrator level by fitting per-cell parameters; if a "cell" is `(mutation × language × entity × embedder)` (as in `shrinkage_engine.rs:14`), then a per-cell empirical Bayes prior is per-embedder and respects slot identity. If "cell" collapses across embedders, it violates §6.2. Implementation must keep the empirical Bayes prior strictly within-embedder.

---

## 11. Three questions the other Sherlocks must answer

For the three reports to compose cleanly into one calibration / feature / loss prescription:

### Question 1 (to Sherlock #1, raw-feature excavation)

**My proposals consume per-embedder calibrated scores and per-pair MI/correlation values.** If you propose new raw MI features that change the distribution of `pairwise_mi_upper` significantly, my P3 beta-calibration cell parameters will need to be re-fit. **Can you produce, per proposed raw-feature change, the expected distribution shift in (per_embedder_cosine, pairwise_cosine_upper, pairwise_mi_upper) that would feed `head_projection.rs:339-358`?** Specifically: if your changes introduce a new pairwise correlation regime, the binned baseline at `eval/per_head_calibration_tracker.rs` will (per H1) appear to regress even if everything else is better. I need to know which of your proposals require P1 (SmoothECE) to be live first, vs which can be evaluated against the existing binner without false-positive ECE regression.

### Question 2 (to Sherlock #3, foundationality + CP_φ + entropy loss)

**Entropy loss (`docs/05_predictor_pipeline.md:493`, `crates/context-graph-mejepa-train/src/loss/entropy.rs`) is a training-time fix for confidence over-confidence.** My P3 / P6 are inference-time fixes for the same phenomenon. **What is the expected interaction?** Specifically: if entropy loss already pushes the predictor toward higher-entropy outputs at training, my post-hoc calibrators will see narrower confidence distributions and the V-A intervals will be wider. That's good for calibration but bad for the operator-visible "system is confident" signal. If your foundationality scalars get added to the predictor's input, they likely shift the score distribution; my P4 empirical-Bayes prior strength κ will need to be re-estimated. **Can you commit to a particular ordering — entropy loss FIRST, post-hoc cal AFTER — and quantify the expected confidence-distribution narrowing so my V-A interval widths are interpretable?**

### Question 3 (to both)

**The operator's hard constraint is "no holdout-selected thresholds."** I've designed P1-P7 to never touch the holdout. **What is your assumption about how much calibration partition exists?** If you propose new features that require their own calibration (foundationality scalars need a per-cell scaling, raw MI features need their own bin centers), my 240-row calibration budget shrinks. **Do you require an expansion of the calibration partition (e.g., 240 → 360 by reducing holdout 240 → 120)?** That would weaken the holdout's power to detect ship-gate readiness. We need to align on the calibration-budget triage before I commit to per-cell beta + V-A + JS-shrinkage all sharing the same 240 rows.

---

**Final note.** The mathematical center of gravity is this: the existing system fits a low-parameter calibrator (Path C global binner, Path A global conformal tau) on top of a high-parameter predictor (per-head, per-pair, per-cell). Standard ML literature is clear that **the calibrator's parameter count must scale with the dimensionality of the shift you want it to absorb** ([Park 2020](http://proceedings.mlr.press/v108/park20b/park20b.pdf); [Gong 2021](https://arxiv.org/pdf/2104.00742); [Naeini 2015](https://pmc.ncbi.nlm.nih.gov/articles/PMC4410090/)). The current calibrator has ~12 parameters total (10 bin edges + α + σ²). The predictor produces hundreds of features. When feature mix shifts (trajectory features, foundationality, cause-bin), the 12-parameter calibrator cannot absorb the shift; it produces the H1 calibration aging we are seeing. P1-P4 collectively raise the calibrator's parameter count to **~5 per cell × 8 cells × per-Python = ~40 effective parameters** with empirical-Bayes regularization back toward the language-level fit. That is the structural fix the system needs.

Sources cited:
- [Hoff 2025, "Shrinkage and Empirical Bayes" lecture notes](https://www2.stat.duke.edu/~pdh10/Teaching/732/Notes/shrinkage.pdf)
- [Reid 2021, "Empirical Bayes and the James-Stein Estimator"](https://utstat.toronto.edu/reid/sta2212s/2021/LSIChapter1.pdf)
- [Berkeley Stat210A on James-Stein](https://stat210a.berkeley.edu/fall-2024/reader/jamesstein.html)
- [Vovk & Petej 2014, "Venn-Abers predictors", arXiv 1211.0025](https://arxiv.org/pdf/1211.0025)
- [Toccaceli 2017 COPA tutorial on Venn-Abers](https://cml.rhul.ac.uk/copa2017/presentations/VennTutorialCOPA2017.pdf)
- [Naeini, Cooper, Hauskrecht 2015, "Obtaining Well Calibrated Probabilities Using Bayesian Binning"](https://pmc.ncbi.nlm.nih.gov/articles/PMC4410090/)
- [Błasiok & Nakkiran 2024, "Smooth ECE: Principled Reliability Diagrams via Kernel Smoothing"](https://arxiv.org/html/2309.12236)
- ["How Flawed Is ECE? An Analysis via Logit Smoothing", arXiv 2402.10046](https://arxiv.org/html/2402.10046)
- [apple/ml-calibration relplot library](https://github.com/apple/ml-calibration)
- [Kull, Silva Filho, Flach 2017 — Beta calibration tutorial summary](https://www.abzu.ai/data-science/calibration-introduction-part-2/)
- [Classifier Calibration at Scale, arXiv 2601.19944](https://arxiv.org/html/2601.19944)
- [Mukhoti et al. 2020, "Calibrating Deep Neural Networks using Focal Loss" NeurIPS](https://omegafragger.github.io/publication/calibration_neurips_2020/)
- [Tao et al. 2023, "Dual Focal Loss for Calibration", ICML](https://proceedings.mlr.press/v202/tao23a/tao23a.pdf)
- ["Improving Calibration by Relating Focal Loss, Temperature Scaling, and Properness", arXiv 2408.11598](https://arxiv.org/html/2408.11598v1)
- [Pereyra et al. 2017 — Cross-entropy vs label smoothing review, arXiv 2402.03979](https://arxiv.org/html/2402.03979)
- ["Calibrated Language Models and How to Find Them with Label Smoothing", arXiv 2508.00264](https://arxiv.org/html/2508.00264v1)
- [Ding, Angelopoulos, Bates, Jordan, Tibshirani 2023, "Class-Conditional Conformal Prediction with Many Classes"](https://www.stat.berkeley.edu/~ryantibs/papers/classconf.pdf)
- [MAPIE Mondrian conformal docs](https://mapie.readthedocs.io/en/latest/theoretical_description_mondrian.html)
- [Boström & Johansson 2021, "Mondrian Conformal Predictive Distributions"](https://proceedings.mlr.press/v152/bostrom21a/bostrom21a.pdf)
- [Angelopoulos, Bates, Fisch, Lei, Schuster 2024, "Conformal Risk Control"](https://arxiv.org/html/2208.02814v4)
- ["Selective Conformal Risk Control", arXiv 2512.12844](https://arxiv.org/html/2512.12844v1)
- [Geifman & El-Yaniv 2019, "SelectiveNet: A Deep Neural Network with an Integrated Reject Option"](https://arxiv.org/pdf/1901.09192)
- ["Improving Predictor Reliability with Selective Recalibration", OpenReview](https://openreview.net/pdf?id=Aoj9H6jl6F)
- [Liu et al. 2020, "Selective Classification Can Magnify Disparities Across Groups"](https://arxiv.org/pdf/2010.14134)
- ["Uncertainty-Driven Reliability: Selective Prediction and ...", arXiv 2508.07556](https://arxiv.org/pdf/2508.07556)
- [Park et al. 2020, "Calibrated Prediction with Covariate Shift via Unsupervised Domain Adaptation"](http://proceedings.mlr.press/v108/park20b/park20b.pdf)
- [Gong et al. 2021, "Confidence Calibration for Domain Generalization under Covariate Shift", ICCV](https://arxiv.org/pdf/2104.00742)
- ["Confidence-calibrated covariate shift correction for few-shot classification in Vision-Language Models", arXiv 2502.07847](https://arxiv.org/html/2502.07847)
- ["Bayesian Classifier Fusion with an Explicit Model of Correlation", arXiv 2106.01770](https://arxiv.org/pdf/2106.01770)
- ["Feature Fusion for Improved Classification: Combining Dempster-Shafer Theory and Multiple CNN Architectures", arXiv 2405.20230](https://arxiv.org/pdf/2405.20230)
- ["A novel multi-classifier information fusion based on Dempster-Shafer theory", arXiv 2012.02481](https://arxiv.org/pdf/2012.02481)
- ["An evidential classifier based on Dempster-Shafer theory and deep learning"](https://www.sciencedirect.com/science/article/abs/pii/S0925231221004525)
- ["An Interpretation of Weights in Linear Opinion Pooling", Cambridge Episteme](https://www.cambridge.org/core/journals/episteme/article/an-interpretation-of-weights-in-linear-opinion-pooling/531946371BB0BF93399E70EA60C824C7)
- [Chris Said 2017, "Empirical Bayes for multiple sample sizes"](https://chris-said.io/2017/05/03/empirical-bayes-for-multiple-sample-sizes/)
- ["Distributional bias compromises leave-one-out cross-validation", arXiv 2406.01652](https://arxiv.org/pdf/2406.01652)
