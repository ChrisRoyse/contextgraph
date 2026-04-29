# EXP-2 Atomic Tasks

Level-4 atomic task list. Each task: explicit inputs, explicit outputs, explicit definition-of-done, explicit dependencies. An atomic task is sized to ≤ 2 hours of solo focused work.

Format per task:
```
## T-NNN: <Title>
**Phase**: <phase> · **Estimated**: <hours> · **Deps**: <task ids>
**Owner**: operator | subagent
**Inputs**: <files / state>
**Outputs**: <files>
**Definition of Done**:
  - <bullet>
  - <bullet>
**Notes**: <free text>
```

---

## Phase A — Setup & Pre-Production (Days 1–3)

## T-001: Verify Environment
**Phase**: A · **Estimated**: 1h · **Deps**: —
**Owner**: operator
**Inputs**: existing Context Graph repo at HEAD
**Outputs**: `scripts/exp2/verify_environment.py`, `verification_log.json`
**Definition of Done**:
- [ ] CUDA available, RTX 5090 detected, ≥30 GB free VRAM.
- [ ] Python 3.12+ installed.
- [ ] `cargo build --release` of context-graph-mcp passes.
- [ ] All 14 embedders' weight files present in `models/` directory.
- [ ] Each embedder's weight SHA-256 logged.
- [ ] Test embedding extracted on a stub input through MCP server.
- [ ] All `requirements.txt` packages install cleanly.

## T-002: Build Per-Stratum Source Files
**Phase**: A · **Estimated**: 4h · **Deps**: T-001
**Owner**: subagent + operator review
**Inputs**: existing data sources per [04_data_corpus.md](04_data_corpus.md)
**Outputs**: `scripts/exp2/strata/{S1..S8}.json` (8 files, ≥120 candidate inputs each for headroom)
**Definition of Done**:
- [ ] Each stratum's JSON has ≥120 candidate inputs with text + metadata + license.
- [ ] No duplicates within or across strata (text-hash unique).
- [ ] Length bounds (50–800 tokens) checked per input.
- [ ] Encoding valid utf-8 confirmed.
- [ ] License logged per stratum.

## T-003: Sample Exactly 100 Per Stratum
**Phase**: A · **Estimated**: 1h · **Deps**: T-002
**Owner**: subagent
**Inputs**: `S1..S8.json` from T-002
**Outputs**: `S1_sampled.json ... S8_sampled.json` (8 files, 100 each)
**Definition of Done**:
- [ ] Selection seed = 42 logged.
- [ ] Each sampled file contains exactly 100 inputs.
- [ ] Selection deterministic (running twice produces identical files).

## T-004: Merge + Validate + Sign Manifest
**Phase**: A · **Estimated**: 2h · **Deps**: T-003
**Owner**: subagent + operator sign-off (per Constitution C16)
**Inputs**: `S1_sampled.json ... S8_sampled.json`
**Outputs**: `artefacts/exp2/corpus_manifest_v1.json`, `corpus_manifest_v1.json.sha256`, `MANIFEST_APPROVED.txt`
**Definition of Done**:
- [ ] Manifest JSON canonicalized (sorted keys).
- [ ] Operator spot-checks 5 inputs per stratum (40 total) and signs `MANIFEST_APPROVED.txt`.
- [ ] sha256 logged in audit chain.
- [ ] Manifest schema validated against [04_data_corpus.md](04_data_corpus.md) §5.

## T-005: Build PCA Training Corpus
**Phase**: A · **Estimated**: 2h · **Deps**: T-002 (parallel to T-003)
**Owner**: subagent
**Inputs**: same 8 strata source files (T-002 output)
**Outputs**: `artefacts/exp2/pca_training_corpus_v1.json` (1000 inputs, distinct from T-003 selection)
**Definition of Done**:
- [ ] Selection seed = 43 (≠ T-003).
- [ ] No overlap with T-003 selection (verified by text-hash difference).
- [ ] Manifest hashed.

## Phase B — Embedding Extraction (Days 2–3)

## T-006: Implement Embedding Extraction Script
**Phase**: B · **Estimated**: 4h · **Deps**: T-001
**Owner**: subagent
**Inputs**: existing MCP server, store_memory + get_memory_fingerprint tools
**Outputs**: `scripts/exp2/extract_embeddings.py`
**Definition of Done**:
- [ ] Reads corpus manifest.
- [ ] Detects already-stored inputs (CF_FINGERPRINTS lookup by content_hash).
- [ ] For new inputs, ingests via MCP store_memory in a temporary session.
- [ ] Retrieves all 14 embedder vectors per input.
- [ ] Uses primary asymmetric variant (cause / source / paraphrase) per PRD §5.
- [ ] Wipes temporary session after extraction.
- [ ] Saves to `artefacts/exp2/embeddings_per_input.npz`.
- [ ] Logs per-embedder weight SHA-256 to audit chain.
- [ ] Unit tests with stub inputs pass.

## T-007: Run Embedding Extraction
**Phase**: B · **Estimated**: 1.5h (compute) · **Deps**: T-004, T-006
**Owner**: operator (kicks off; supervises)
**Inputs**: `corpus_manifest_v1.json`, MCP server running
**Outputs**: `embeddings_per_input.npz` (~1 GB), `extraction_log.json`
**Definition of Done**:
- [ ] All 800 inputs successfully embedded.
- [ ] No zero-norm embeddings (any flagged input replaced or stratum re-sampled).
- [ ] Each embedding tensor has correct dimension per spec.
- [ ] All weight hashes match registry.
- [ ] Audit chain entry created.

## T-008: PCA Training Corpus Embedding Extraction
**Phase**: B · **Estimated**: 1.5h (compute) · **Deps**: T-005, T-006
**Owner**: subagent
**Inputs**: `pca_training_corpus_v1.json`
**Outputs**: `pca_training_embeddings.npz`
**Definition of Done**:
- [ ] All 1000 PCA training inputs embedded.
- [ ] No overlap-by-content-hash with T-007 output.
- [ ] All 14 embeddings per input.

## T-009: Fit PCA Models
**Phase**: B · **Estimated**: 2h · **Deps**: T-008
**Owner**: subagent
**Inputs**: `pca_training_embeddings.npz`
**Outputs**: `pca_models.npz`, `pca_validation_report.json`
**Definition of Done**:
- [ ] PCA-32 fit per embedder.
- [ ] Explained variance ratio at d=32 logged.
- [ ] If any embedder < 0.85 explained variance, refits with d=64; logged.
- [ ] All PCA models hashed sha256.
- [ ] Validation: re-applying PCA round-trips L2-normed within fp32 tolerance.

## T-010: Apply PCA To Audit Embeddings
**Phase**: B · **Estimated**: 30min · **Deps**: T-007, T-009
**Owner**: subagent
**Inputs**: `embeddings_per_input.npz`, `pca_models.npz`
**Outputs**: `projected.npz` (800 × 14 × 32)
**Definition of Done**:
- [ ] All 800 inputs × 14 embedders → 32-D projected vectors.
- [ ] Each projected vector L2-normalized.
- [ ] No NaNs, no infs.
- [ ] Output hash logged.

## Phase C — Estimator Implementation (Days 4–7)

## T-011: Implement InfoNCE Estimator
**Phase**: C · **Estimated**: 6h · **Deps**: T-001
**Owner**: subagent
**Inputs**: PyTorch, hyperparameters from `exp2.yml`
**Outputs**: `scripts/exp2/estimators/infonce.py`, `tests/test_infonce.py`
**Definition of Done**:
- [ ] `InfoNCEEstimator` class per [05_technical_spec.md](05_technical_spec.md) §9.
- [ ] Determinism: torch + cuDNN seeds set; same input → same output.
- [ ] Training loop: 5000 steps, early stopping on val.
- [ ] Returns: median val InfoNCE over last 1000 steps.
- [ ] Unit tests: pass synthetic ground-truth recovery (sweep $\rho \in \{0.0, 0.3, 0.5, 0.9\}$, each within 0.1 nats of analytical MI).

## T-012: Implement KSG Estimator
**Phase**: C · **Estimated**: 3h · **Deps**: T-001
**Owner**: subagent
**Inputs**: NPEET, scikit-learn
**Outputs**: `scripts/exp2/estimators/ksg.py`, `tests/test_ksg.py`
**Definition of Done**:
- [ ] `ksg_mi(u, v, k)` returns scalar.
- [ ] `ksg_entropy(x, k)` returns scalar.
- [ ] $k$ sensitivity: sweep $k \in \{3, 5, 10\}$ on synthetic data; instability documented.
- [ ] Unit tests: pass synthetic ground-truth recovery within 0.15 nats.

## T-013: Synthetic Ground-Truth Validation
**Phase**: C · **Estimated**: 4h · **Deps**: T-011, T-012
**Owner**: subagent
**Inputs**: InfoNCE + KSG implementations
**Outputs**: `tests/test_synthetic_validation.py`, `synthetic_validation_report.json`
**Definition of Done**:
- [ ] Sweep $\rho \in \{0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99\}$.
- [ ] At each $\rho$, generate 800 (u, v) pairs in 32-D, run both estimators.
- [ ] Compare to closed-form MI: $-\frac{1}{2} \log(1 - \rho^2) \cdot 32$.
- [ ] InfoNCE recovery within 0.1 nats; KSG within 0.15 nats.
- [ ] Pilot pass: also test (E1 from real data, random Gaussian) → near zero MI.
- [ ] Plot synthetic-recovery curve.

## T-014: Implement Bootstrap CI
**Phase**: C · **Estimated**: 2h · **Deps**: T-011, T-012
**Owner**: subagent
**Inputs**: estimator implementations
**Outputs**: `scripts/exp2/estimators/bootstrap.py`, `tests/test_bootstrap.py`
**Definition of Done**:
- [ ] `bootstrap_ci(point_fn, data, n_resamples=10000, seed=42)` returns (low, high).
- [ ] Determinism check: same seed → same CI.
- [ ] Unit tests on known distributions (Gaussian mean, sample variance) recover expected CIs.

## T-015: Pilot Pass On 4 Pairs
**Phase**: C · **Estimated**: 2h · **Deps**: T-013, T-014
**Owner**: subagent + operator review
**Inputs**: `projected.npz`, both estimators, bootstrap
**Outputs**: `pilot_results.json`
**Definition of Done**:
- [ ] Run estimator pipeline on 4 pilot pairs: (E1, E1), (E1, random), (E1, E1+noise), (E1, E10).
- [ ] (E1, E1) returns ≈ H(E1).
- [ ] (E1, random) returns ≈ 0 (≤0.1 nats).
- [ ] (E1, E1+noise) returns positive but < H(E1).
- [ ] (E1, E10) returns substantial MI per §10 expectation.
- [ ] Operator reviews; signs off on methodology before T-016.

## Phase D — Production Estimator Run (Days 6–7)

## T-016: Run InfoNCE On 91 Pairs
**Phase**: D · **Estimated**: 2-3h (compute) · **Deps**: T-015 (sign-off)
**Owner**: subagent (orchestrates)
**Inputs**: `projected.npz`, `exp2.yml`, T-015 sign-off
**Outputs**: 91 × `pair_results/E{j}_E{k}.json` (InfoNCE fields populated)
**Definition of Done**:
- [ ] All 91 pairs trained to convergence.
- [ ] Each pair logs: train loss, val loss, train-val gap.
- [ ] Each pair logs: median val InfoNCE over last 1000 steps.
- [ ] Pairs with train-val gap > 0.1 nats are flagged + re-trained with smaller critic.
- [ ] All 91 pair JSON files present.

## T-017: Run KSG On 91 Pairs
**Phase**: D · **Estimated**: 30min (compute) · **Deps**: T-016
**Owner**: subagent
**Inputs**: `projected.npz`
**Outputs**: 91 × `pair_results/E{j}_E{k}.json` (KSG fields populated)
**Definition of Done**:
- [ ] All 91 pairs evaluated.
- [ ] $k$ sweep on first 10 pairs to confirm $k=5$ stability.
- [ ] All KSG estimates ≥ 0 (clamp + log if any negative).

## T-018: Bootstrap All Pairs
**Phase**: D · **Estimated**: 3-4h (compute) · **Deps**: T-016, T-017
**Owner**: subagent
**Inputs**: trained InfoNCE critics + KSG implementations + projected.npz
**Outputs**: 91 × `pair_results/E{j}_E{k}.json` (CI fields populated)
**Definition of Done**:
- [ ] InfoNCE bootstrap: re-evaluate trained critic on resampled batches.
- [ ] KSG bootstrap: re-run KSG on resampled corpora (10k resamples).
- [ ] All 91 × 2 = 182 CI ranges computed.
- [ ] Median bootstrap CI width logged.

## T-019: Per-Embedder Entropy Estimation
**Phase**: D · **Estimated**: 30min (compute) · **Deps**: T-010
**Owner**: subagent
**Inputs**: `projected.npz`
**Outputs**: `entropy_per_embedder.json`
**Definition of Done**:
- [ ] H(E1)..H(E14) estimated via KSG-1 with k=5.
- [ ] Each entropy bootstrap-CI'd.
- [ ] All entropies > 0 (sanity).

## Phase E — Aggregation, Visualization, Writeup (Days 8–10)

## T-020: Compute Cross-Validation Metrics
**Phase**: E · **Estimated**: 1h · **Deps**: T-016, T-017
**Owner**: subagent
**Inputs**: `pair_results/`
**Outputs**: `cross_validation_report.json`
**Definition of Done**:
- [ ] For each pair, |MINE - KSG| computed.
- [ ] Agreement rate (% within 0.15 nats) logged.
- [ ] Pairs failing agreement listed with hypotheses.
- [ ] Per Constitution C3: if rate < 80%, methodology revisits before T-021.

## T-021: Compute Effective Rank
**Phase**: E · **Estimated**: 2h · **Deps**: T-018, T-019
**Owner**: subagent
**Inputs**: `pair_results/`, `entropy_per_embedder.json`
**Outputs**: `n_eff_summary.json`
**Definition of Done**:
- [ ] Three N_eff methods computed per [03_methodology.md](03_methodology.md) §7.
- [ ] All three with bootstrap CIs.
- [ ] Headline N_eff = redundancy-adjusted method.
- [ ] Spectrum eigenvalue plot data exported.

## T-022: Render Visualizations
**Phase**: E · **Estimated**: 3h · **Deps**: T-021
**Owner**: subagent
**Inputs**: pair results, n_eff summary
**Outputs**: `redundancy_heatmap.{png,pdf}`, `spectrum_plot.{png,pdf}`, `entropy_bars.{png,pdf}`
**Definition of Done**:
- [ ] Heatmap: 14×14 grid, MI values, log-color-scale, embedder labels readable.
- [ ] Spectrum: eigenvalue magnitudes on log scale.
- [ ] Entropy: bar chart with CI error bars.
- [ ] All saved at 300 DPI.

## T-023: Per-Pair Recommendation Table
**Phase**: E · **Estimated**: 1h · **Deps**: T-021
**Owner**: subagent
**Inputs**: pair results, n_eff summary
**Outputs**: `per_pair_table.csv`, `embedder_swap_recommendation.md`
**Definition of Done**:
- [ ] CSV with all 91 pairs, columns per [03_methodology.md](03_methodology.md) §13.
- [ ] Swap recommendation: identify top 5 redundant pairs; recommend embedder candidates to swap.

## Phase F — FSV, Replay, Distribution (Days 11–14)

## T-024: Audit Chain Finalization
**Phase**: F · **Estimated**: 1h · **Deps**: T-022, T-023
**Owner**: subagent
**Inputs**: all artefacts
**Outputs**: `audit_chain.json`
**Definition of Done**:
- [ ] Every artefact has sha256 in chain.
- [ ] Every code commit hash logged.
- [ ] Every hyperparameter hash logged.
- [ ] Every weight hash logged.
- [ ] Chain validates: each artefact has parent_artefact_ids correctly.

## T-025: FSV Replay Test
**Phase**: F · **Estimated**: 5h (replay run + analysis) · **Deps**: T-024
**Owner**: subagent + operator review
**Inputs**: `exp2.yml`, original artefacts (reference)
**Outputs**: replay artefacts + `fsv_replay_log.json`
**Definition of Done**:
- [ ] Pipeline rerun from T-006 onward with same seed.
- [ ] All 91 pair scalar values match within $10^{-5}$.
- [ ] N_eff scalar matches within $10^{-5}$.
- [ ] If any mismatch > tolerance: investigated, root-cause documented, FSV re-run after fix.

## T-026: Operator Sign-Off On Headline
**Phase**: F · **Estimated**: 1h · **Deps**: T-025
**Owner**: operator (per Constitution C16)
**Inputs**: `n_eff_summary.json`, heatmap, paper patch draft
**Outputs**: `OPERATOR_APPROVED_HEADLINE.txt`
**Definition of Done**:
- [ ] Operator personally reviews the headline N_eff number, the heatmap, the paper patch.
- [ ] Operator signs off; committing the file unblocks T-027 onward.

## T-027: Paper Patch Drafting
**Phase**: F · **Estimated**: 4h · **Deps**: T-026
**Owner**: operator
**Inputs**: paper § 3.1 + § 6.1 source, n_eff_summary.json
**Outputs**: `PAPER_PATCH.md`
**Definition of Done**:
- [ ] Drop-in markdown for §3.1 and §6.1 of paper.
- [ ] Numbers replaced with measured.
- [ ] Bootstrap CI included.
- [ ] Methodology box added (or pointer to §A.X appendix with full method).
- [ ] No contradictions with surrounding paper text.

## T-028: Pre-Register On OSF
**Phase**: F · **Estimated**: 1h · **Deps**: T-026
**Owner**: operator
**Inputs**: hyperparams + corpus manifest hash + analysis plan
**Outputs**: OSF pre-registration record link
**Definition of Done**:
- [ ] OSF account created or signed in.
- [ ] Pre-registration form completed.
- [ ] DOI assigned.
- [ ] Link logged in audit chain.

## T-029: Open-Source Harness Release
**Phase**: F · **Estimated**: 4h · **Deps**: T-026
**Owner**: subagent + operator review
**Inputs**: `scripts/exp2/`, `docs/exp2/`, key artefacts
**Outputs**: GitHub release `contextgraph-exp2-v1.0.0`, Zenodo DOI
**Definition of Done**:
- [ ] GitHub repo created (or fork existing).
- [ ] Apache-2.0 license added.
- [ ] CITATION.cff added.
- [ ] Tagged release v1.0.0.
- [ ] README has Quick Start.
- [ ] Zenodo archive triggered, DOI obtained.
- [ ] HuggingFace model card published (for the corpus + per-pair table artefact bundle).

## T-030: arXiv Revision Submission
**Phase**: F · **Estimated**: 2h · **Deps**: T-027
**Owner**: operator
**Inputs**: paper source + PAPER_PATCH.md
**Outputs**: arXiv submitted revision
**Definition of Done**:
- [ ] LaTeX source updated.
- [ ] Figures updated (heatmap as Fig. X).
- [ ] Replaced numbers throughout (§ 3.1, § 6.1, § 8, § 9 references, conclusion).
- [ ] arXiv submission accepted.
- [ ] Revision link tweeted + posted to channel.

## T-031: YouTube Video Production
**Phase**: F · **Estimated**: 8h (script, record, edit, upload) · **Deps**: T-026
**Owner**: operator
**Inputs**: heatmap, paper patch, n_eff numbers
**Outputs**: published YouTube video, link logged in audit chain
**Definition of Done**:
- [ ] Video script per [09_distribution_plan.md](09_distribution_plan.md) §C.
- [ ] 8–12 minutes.
- [ ] Heatmap reveal at T+0:30.
- [ ] arXiv revision link in description.
- [ ] GitHub harness link in description.
- [ ] Pinned comment with key numbers + links.

## T-032: Blog Post
**Phase**: F · **Estimated**: 4h · **Deps**: T-026
**Owner**: operator
**Inputs**: heatmap, n_eff numbers, paper patch
**Outputs**: published blog post on teleox.ai
**Definition of Done**:
- [ ] 1500–2500 words.
- [ ] Heatmap embedded.
- [ ] Numbers verifiable against paper.
- [ ] Links to arXiv revision + GitHub harness.
- [ ] Embedded in main site flow.

## T-033: Tweet Thread
**Phase**: F · **Estimated**: 1h · **Deps**: T-031
**Owner**: operator
**Inputs**: heatmap (or summary), n_eff numbers
**Outputs**: posted tweet thread
**Definition of Done**:
- [ ] 8–12 tweets.
- [ ] Heatmap in first tweet.
- [ ] Threading: arXiv link, GitHub link, YouTube link, blog link.

---

## Cross-Cutting Tasks

## T-100: Lessons File
**Phase**: continuous
**Owner**: subagent
**Inputs**: any correction or surprise
**Outputs**: `tasks/exp2/lessons.md`
**Definition of Done**:
- [ ] Every operator correction or unexpected finding logged with: what, why, fix, prevention rule.

## T-101: Todo File
**Phase**: continuous
**Owner**: subagent
**Inputs**: above task list
**Outputs**: `tasks/exp2/todo.md`
**Definition of Done**:
- [ ] Mirror of this task list with status (pending / in_progress / completed) + sign-off marks.

---

## Task Dependency Graph (Summary)

```
T-001 → T-002, T-006
T-002 → T-003, T-005
T-003 → T-004
T-005 → T-008
T-006 → T-007, T-008
T-008 → T-009 → T-010
T-007 + T-010 → T-016
T-001 → T-011, T-012
T-011 + T-012 → T-013, T-014
T-013 + T-014 → T-015 → T-016
T-016 → T-017 → T-018
T-018 → T-019 → T-021
T-016 + T-017 → T-020 → T-021
T-021 → T-022, T-023, T-024
T-024 → T-025 → T-026
T-026 → T-027, T-028, T-029, T-031, T-032 (parallel)
T-027 → T-030
T-031 → T-033
```

## Critical Path

T-001 → T-006 → T-007 → T-009 → T-010 → T-016 → T-018 → T-021 → T-024 → T-025 → T-026 → T-027 → T-030.

Estimated critical-path duration: **~12 days** including 4 hours InfoNCE compute + 30 min KSG + 4 hours bootstrap + 5 hours FSV + writeup + arXiv revision turnaround.

## Parallelism Opportunities

- T-002, T-005 can run in parallel after T-001.
- T-011, T-012 can run in parallel after T-001.
- T-022, T-023 can run in parallel after T-021.
- T-031, T-032, T-033 can run in parallel after T-026.

If you parallelize aggressively, the experiment ships in **8–10 days** end-to-end.
