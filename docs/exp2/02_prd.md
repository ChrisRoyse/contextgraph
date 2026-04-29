# EXP-2 Product Requirements Document

## 1. Problem Statement

The TCT/DDA paper (April 2026) defends a **counting identity** in §3.1:

$$|D'| = n \cdot \left(N + \binom{N}{2}\right)$$

with the headline that, at $N = 13$ (the production Context Graph panel as paper-published), each input yields **up to 91 structured supervisory signals**. The "up to" is load-bearing; tightness depends on **Axiom 2** — that the embedders are *approximately independent* in their information axes.

Axiom 2 is documented as an engineering-operational assumption, not measured. The paper §8.L4 explicitly names this gap as the dominant remaining empirical claim ("Embedder independence is empirical, not proven"). Section §9 names EXP-2 as the falsification test: a pairwise mutual-information audit across the 13/14-embedder panel.

EXP-2 measures, for the production 14-embedder panel (post-E14 migration), the empirical pairwise MI matrix and converts the structural upper bound 91 into a measured **effective signal count** $N_{\mathrm{eff}}$. This addresses reviewer skepticism, lets domain forks proceed with calibrated DDA-yield expectations, and produces visually-publishable artefacts that compound across the YouTube channel + paper revision + blog post.

## 2. Stakeholders

| Stakeholder | Concern | Acceptance criterion |
|---|---|---|
| Paper reviewers | Is "up to 91" defensible? | Measured $N_{\mathrm{eff}}$ + redundancy heatmap with bootstrap CIs |
| Operator (you) | Which embedders are redundant? | Per-pair recommendation table with swap candidates |
| Domain-fork builders | What's the realistic DDA yield for a similar panel? | Methodology + comparison guide for new panels |
| YouTube audience | Why does this matter? | 8–12 minute video with heatmap reveal + plain-language explainer |
| Future replicators | Can we reproduce? | Open-source harness + frozen corpus manifest + seeds + hyperparameters |
| Funders / partners | Is the framework empirically grounded? | Single-page summary + arXiv revision link |

## 3. In Scope

- Computing pairwise MI for all $\binom{14}{2} = 91$ pairs of the production embedder panel (E1–E14, with E11 *included* in the audit even though `E11_ENTITY_ENABLED=false` for retrieval — its MI is informative for future panel design).
- Two estimator methods (InfoNCE/MINE + partitioned-KSG) with cross-validation.
- Bootstrap 95% confidence intervals.
- Three effective-rank computations (redundancy-adjusted count, spectrum entropy, Schur-complement).
- Visualization: heatmap, marginal entropy bar chart, spectrum plot.
- FSV harness with replay determinism.
- Paper §3.1 + §6.1 patch.
- YouTube video script + recording outline.
- Blog post (1500–2500 words).
- arXiv revision submission.
- Open-source harness release.

## 4. Out of Scope

- Cross-encoder triangulation on Case 3 voice cloning (= EXP-1).
- $n_m$ sample-complexity sweep on Case 3 (= EXP-3).
- Domain-fork validation (genomics / drug-discovery / materials panels).
- Causal-MI estimation (different problem; symmetric MI is the goal).
- Total correlation across all 14 (would multiply compute by ~1000×; pairwise is the agreed bound).
- Time-varying MI (corpus is static; longitudinal MI is future work).
- Cross-modal MI between text-embedders and learner-state embedders (E15–E21 are excluded; this is text-side only).

## 5. Asymmetric Embedder Handling

Three embedders ship dual vectors:
- E5 cause / effect.
- E8 source / target.
- E10 paraphrase / context.

For EXP-2, we use the *primary* directional variant per pair:
- E5 → cause vector.
- E8 → source vector.
- E10 → paraphrase vector.

The audit log records this choice; an extension experiment (EXP-2b) measuring MI between dual variants of the same embedder is named future work and not blocking.

## 6. Sparse / Token Embedder Handling

Sparse and token-level embedders need unification to dense space for InfoNCE/KSG:

- **E6 SPLADE-coCondenser**: project sparse vector to dense via top-K-term selection + L2-normalize the K=128 weights vector. (The dense projection becomes a 128-D vector.) Method documented in [03_methodology.md](03_methodology.md) §3.
- **E12 ColBERTv2 token-level**: pool tokens to 128-D via mean (deliberately lossy; per-token-MI is not the audit's goal).
- **E13 SPLADE-PP**: same as E6, K=128.
- **E9 HDC**: project 10000-bit binary to 1024-D via fixed random projection (the projection matrix is part of the audit chain).

These projections are **frozen** for the entire EXP-2 run. They are *not* the embedder; they are the dimensional-equalization step. The MI estimates are between $(\phi_j(x), \mathrm{proj}_k(\phi_k(x)))$ rather than raw $(\phi_j(x), \phi_k(x))$ when projection is required. This is logged in every output.

## 7. Functional Requirements

### F1. Corpus Stratification
The harness selects 800 inputs across 8 strata (100 each) per [04_data_corpus.md](04_data_corpus.md), with diversity criteria explicitly checked.

### F2. Embedding Extraction
For each input, retrieve all 14 embedder vectors from Context Graph (or compute fresh if absent). Persist as `[input_id, embedder_id] → vector` flat array.

### F3. MI Estimation (InfoNCE)
For each of 91 pairs, train a separate critic network to bound MI from below via Donsker-Varadhan or InfoNCE objective. Report MI estimate, training-loss curve hash, validation MI on held-out 20%.

### F4. MI Estimation (KSG)
For each of 91 pairs, compute KSG-1 estimator with $k = 5$ on partitioned-PCA representations (32-D each side). Report MI estimate.

### F5. Bootstrap CI
For each estimator output, resample (with replacement) the 800 inputs, recompute MI, 10000 times, report 2.5–97.5 percentile.

### F6. Effective Rank
Compute three estimates and report all three:
1. **Redundancy-adjusted count**: $N_{\mathrm{eff}} = N + \sum_{j<k} (1 - \min(I_{jk}/H_j, I_{jk}/H_k))$.
2. **Spectrum entropy**: $N_{\mathrm{eff}} = e^{H(\hat\lambda)}$ where $\hat\lambda$ is the normalized spectrum of the (Frobenius-normalized) MI matrix.
3. **Schur-complement rank**: numeric rank of the information matrix $\Sigma_{jk} = I(\phi_j; \phi_k)$ at threshold $\epsilon = 0.05$ relative to max.

### F7. Visualization
Render heatmap (log-scaled), marginal-entropy bars, spectrum plot, redundancy-vs-pair scatter. All saved as both PNG (web) and PDF (paper).

### F8. FSV
Replay the entire pipeline; assert numerical identity within $10^{-5}$ on all reported scalars.

### F9. Paper Patch
Produce drop-in markdown for paper §3.1 and §6.1 with measured numbers in place.

### F10. Audit Chain
Persist provenance per [01_constitution.md](01_constitution.md) §C7.

### F11. Distribution
Ship paper revision to arXiv, publish YouTube video, post blog.

## 8. Non-Functional Requirements

### N1. Replay Determinism
Same seed + same corpus + same code → same numbers within fp32 noise.

### N2. Open-Source Release
Harness code + corpus manifest + hyperparameters + embedder weight hashes released under Apache-2.0 within 14 days of T-021.

### N3. Audit Friendly
Every artefact carries provenance. Independent reviewers can re-derive every number from first principles given the public manifest.

### N4. Compute Budget
≤6 hours GPU compute on RTX 5090. ≤30 GB VRAM peak. ≤10 GB disk.

### N5. Documentation Coverage
Every estimator function, every bootstrap function, every visualization function has a docstring + one unit test. No dead code.

### N6. Failure Logging
Any non-converged InfoNCE run, any KSG numerical exception, any bootstrap NaN is logged with full diagnostics. We do not silently mask.

## 9. Success Metrics

| Metric | Target |
|---|---|
| Pairs with both estimators succeeding | ≥ 91/91 |
| Pairs where MINE & KSG agree to ±0.15 nats | ≥ 73/91 (≥80%) |
| Bootstrap CI width (median pair) | ≤ 0.20 nats |
| Replay determinism deviation | ≤ $10^{-5}$ |
| Time to arXiv revision | ≤ 14 days from T-021 |
| YouTube video length | 8–12 minutes |
| YouTube views in 30 days | (descriptive) |
| arXiv comment count in 30 days | (descriptive) |

## 10. Acceptance Criteria

EXP-2 is **accepted** when:
- All 91 pair MI estimates have both methods + bootstrap CI in artefact bundle.
- $N_{\mathrm{eff}}$ scalar with method tag + bootstrap CI in artefact bundle.
- Redundancy heatmap PDF/PNG in artefact bundle.
- FSV pass (T-021).
- Paper §3.1 + §6.1 patch landed (T-022).
- YouTube video published.
- arXiv revision submitted.
- Open-source harness on GitHub.

## 11. Risks (Summary; full in [07_validation_harness.md](07_validation_harness.md) §4)

| Risk | Likelihood | Mitigation |
|---|---|---|
| InfoNCE doesn't converge for some pair | Medium | Diagnostic logging + retry with longer training + fall back to KSG-only result for that pair |
| KSG curse of dimensionality (1024-D vectors) | High before mitigation | Partitioned PCA-32D before KSG; pre-validated on synthetic data |
| Compute time exceeds budget | Low | Parallelize 8 InfoNCE trainings on RTX 5090; test with 4 pairs first |
| Bootstrap CI is too wide | Medium | Increase corpus to 1200 inputs (extension); document |
| MINE and KSG disagree heavily | Medium | Document, investigate root cause, publish both with caveats |
| Headline $N_{\mathrm{eff}}$ is much lower than expected | Medium | We publish honestly. Lower $N_{\mathrm{eff}}$ is still a measurement; reframe future panel design |

## 12. Why This PRD Is Lean

Most of the PRD's normal "competitive analysis," "go-to-market," "user research" sections are absent because EXP-2 is a measurement experiment, not a product launch. The "competitive analysis" is: nobody else has done this on a 14-embedder panel; the relevant prior art is in the methodology doc. The "go-to-market" is: arXiv + YouTube + blog. The "user research" is: paper reviewers' specific objections (already known from the paper's §8 limitations list).

The risk in over-specifying a measurement experiment is exactly the risk of post-hoc rationalization the constitution forbids. The PRD is the contract between the operator and the harness; the harness produces a number, the number is reported, and the paper is patched. That is the entire product loop.
