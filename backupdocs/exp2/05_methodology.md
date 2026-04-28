# EXP-2 Methodology — Step-by-Step Procedure

**The end-to-end procedure.** Follow it literally. A fresh collaborator with GPU access should be able to reproduce the full run by reading this file plus the preregistration.

---

## 0 · Preconditions

Before any step in §1 runs, verify:

- [ ] Preregistration document [01_preregistration.md](01_preregistration.md) is frozen (git tag `v0.exp2.preregistered` exists)
- [ ] Corpus manifest (`docs/exp2/.corpus_manifest`) lists all expected `.npz` artifact hashes
- [ ] `/home/cabdru/contextgraph/exp2_artifacts/` exists and is on local (not network) storage with > 20 GB free
- [ ] Production daemon is healthy on `127.0.0.1:3100` (`daemon_status` MCP tool responds)
- [ ] RTX 5090 reports > 28 GB free VRAM (production daemon uses ~12 GB; leaves ~20 GB for EXP-2 workers)
- [ ] `nvidia-smi` shows CUDA 13.x runtime
- [ ] Python env has `numpy`, `scipy`, `sklearn`, `torch` + CUDA wheel, `tqdm`, `matplotlib`; no additional dependencies added after this point

If any precondition fails: halt. Fix. Re-run this checklist.

---

## 1 · Corpus build (one-time per freeze)

Performed at preregistration freeze (2026-04-29). Not re-run during MI estimation unless the panel or stratification is amended.

```
1.1  Read frozen seeds from 01_preregistration.md (stratify_seed=42, sketch_seed=2026, split_seed=137)
1.2  Connect to daemon via MCP:
       memory_list --limit all --namespace default
     -> returns live memory UUIDs
1.3  For each UUID, fetch SemanticFingerprint (ensure non-quantized path)
1.4  Classify into strata (code/prose/mixed/short) per 03_sampling_corpus.md §2
1.5  Apply temporal and session diversity filters
1.6  Sample n in {100, 500, 1000} with rejection sampling
1.7  Construct 13-vector panel per 02_panel_specification.md §3
     - Concat dual-asymmetric (E5/E8/E10)
     - Dense-ify sparse (E6/E13) -> JL sketch to 2048D
     - Mean-pool E12 to 128D
1.8  Validate every vector finite + L2-normalized (except sketched sparse)
1.9  Save to exp2_corpus_n<n>_<date>.npz
1.10 Compute sha256 of .npz; append to .corpus_manifest
1.11 Fit PCA on 20% PCA partition:
     - 256D PCA for MINE/InfoNCE inputs
     - Top-16 components for KSG
1.12 Serialize PCA matrices + partition indices to exp2_pca_n<n>_<date>.npz
1.13 Run dry-run validation (02_panel_specification.md §5); commit .panel_dryrun_ok sentinel
```

Expected wall-clock: ~2 hours for n=1000 (dominated by production embedder calls).

**Sensitivity corpora** (Wikipedia-5k, Code-5k, Shakespeare-2k) follow the same pipeline but with their own source-loading step. They are built in parallel once the primary corpus is validated.

---

## 2 · Controls (before any real pair)

Performed immediately after corpus build. If any control fails, no real-pair MI is trusted.

### 2.1 E1-copy control

```
corpus = load exp2_corpus_n1000_<date>.npz
phi_E1 = corpus.phi_E1          # (1000, 1024)
phi_copy = phi_E1.copy()         # identity copy

run_mine_pair(phi_E1, phi_copy, seeds=[0..4])
run_infonce_pair(phi_E1, phi_copy, seeds=[0..4])
run_ksg_pair(phi_E1, phi_copy)

# Expected: hat_I ~= H(phi_E1) ~= (high: 10+ bits given PCA to 256)
# Compare against: hat_H = ksg_entropy_estimator(phi_E1)
# Pass: |hat_I_MINE - hat_H| < 0.5 bits
```

### 2.2 Random-projection control

```
R1, R2 = seeded_random_projections(seeds=[9001, 9002], shape=(256, 1024))
phi_R1 = R1 @ phi_E1.T; phi_R2 = R2 @ phi_E1.T

run_mine_pair(phi_R1.T, phi_R2.T, seeds=[0..4])

# Expected: hat_I ~= H(phi_E1) (projections are invertible-ish)
# Pass: hat_I > 0.95 * H(phi_E1)
```

### 2.3 Zero-MI control

```
# Independent halves: first 500 inputs vs last 500 inputs
phi_A = phi_E1[0:500]; phi_B = phi_E1[500:1000]

# Pair as if they were two embedders of disjoint inputs
run_mine_pair_independent(phi_A, phi_B, seeds=[0..4])
run_ksg_pair_independent(phi_A, phi_B)

# Expected: hat_I ~ 0 (unbiased estimator, small positive finite-sample bias)
# Pass: -0.1 < hat_I < 0.3 bits
```

If any control fails: **HALT.** Write a diagnosis memo to `docs/exp2/control_failure_<date>.md`. Check:
- PCA dimensionality
- Critic architecture matches specs
- Seed plumbing (not reusing the same seed across controls)
- Memory-layout / dtype issues (float32 throughout)

After fix, re-run all three controls. Only when all three pass does §3 begin.

---

## 3 · MINE + InfoNCE full pair sweep

For each of the three sample sizes, for each of the 78 pairs, for each of the 5 seeds, run MINE and InfoNCE. Total runs per size: $78 \times 5 \times 2 = 780$.

### 3.1 Loop structure

```
for n in [100, 500, 1000]:
    corpus = load exp2_corpus_n<n>.npz
    pca = load exp2_pca_n<n>.npz
    panel_pca256 = {phi_j: pca.transform_256(corpus.phi_j) for j in 13 slots}

    train_idx, test_idx = corpus_split(split_seed=137, corpus_size=n)

    for (j, k) in pairs(13):  # 78 pairs
        for seed in [0, 1, 2, 3, 4]:
            # --- MINE ---
            critic = build_mine_critic(seed=seed)  # 2-layer MLP 256
            losses_train, critic_trained = train_mine(
                x=panel_pca256[j][train_idx],
                y=panel_pca256[k][train_idx],
                critic=critic,
                steps=2000, batch=512, lr=1e-4,
                ema=0.01
            )
            hat_I_mine = eval_mine(critic_trained, panel_pca256[j][test_idx], panel_pca256[k][test_idx])
            save_mine_result(j, k, seed, n, hat_I_mine, losses_train, critic_trained.state_dict_hash)

            # --- InfoNCE ---
            W_x, W_y = build_infonce_projections(seed=seed)
            hat_I_nce = train_and_eval_infonce(
                x=panel_pca256[j][train_idx],
                y=panel_pca256[k][train_idx],
                x_test=panel_pca256[j][test_idx],
                y_test=panel_pca256[k][test_idx],
                W_x=W_x, W_y=W_y,
                steps=2000, batch=256, tau=0.1, lr=1e-4
            )
            save_infonce_result(j, k, seed, n, hat_I_nce)

    # Bootstrap CIs on test-split MI (AFTER the pair sweep, over already-computed estimates)
    for (j, k) in pairs(13):
        bootstrap_ci_mine[(j,k,n)] = bootstrap_1000(hat_I_mine[(j,k,[0..4],n)])
        bootstrap_ci_infonce[(j,k,n)] = bootstrap_1000(hat_I_nce[(j,k,[0..4],n)])

    save_all(hat_I, bootstrap_ci)
```

### 3.2 Resource and runtime estimate

At $n = 1000$: 780 MINE + InfoNCE runs × ~2 min each = ~26 GPU-hours. Parallelization across GPU SMs is automatic via PyTorch; no extra scheduling.

**Priority order (if compute becomes constrained)**: run all $n = 500$ first (8 hr); then $n = 1000$ (24 hr); $n = 100$ last as a sanity-check after the big runs. Rationale: $n = 500$ has acceptable CI width for the decision rules; $n = 1000$ is the headline.

### 3.3 Progress tracking

Incremental writes to `exp2_results_mine_n<n>.jsonl`, one JSON line per pair-seed-combo. `(j, k, seed, n, hat_I, losses_train_hash, critic_state_dict_hash, wall_seconds)`. Allows resume after a crash without redoing completed pairs.

### 3.4 Per-pair report

After the full sweep at each $n$:
- Median over 5 seeds + bootstrap 95 % CI
- Flag per [04_estimator_specifications.md](04_estimator_specifications.md) §7

---

## 4 · KSG cross-check on 12 stratified pairs

For each of the three sample sizes (only $n = 1000$ is reported, but KSG is cheap enough to run at all three for scaling curves):

```
for n in [100, 500, 1000]:
    corpus_pca16 = {phi_j: pca.transform_16(corpus.phi_j) for j in 13 slots}
    for (j, k) in KSG_PAIRS_12:
        hat_I_ksg = ksg_estimator(
            x=corpus_pca16[j],
            y=corpus_pca16[k],
            k=3,
            norm='chebyshev',
            subsample_size=500
        )
        ci_ksg = bootstrap_1000_ksg(corpus_pca16[j], corpus_pca16[k])
        save_ksg_result(j, k, n, hat_I_ksg, ci_ksg)
```

Expected wall-clock: < 30 minutes total across all $n$ and all 12 pairs.

---

## 5 · Secondary metrics (reported, not decision-load-bearing)

Also computed for every pair:

### 5.1 Cosine similarity

$\text{CosSim}(j, k) = \mathbb{E}_{x \sim \mathcal{P}}[\cos(\phi_j(x), \phi_k(x))]$

Works only when $\dim \phi_j = \dim \phi_k$, which is rare across the panel. For mismatched dims, we report cosine on the PCA-256 representations.

### 5.2 CKA (Centered Kernel Alignment)

Linear CKA per Kornblith et al. 2019. Reported for a cross-check with the PRH literature (Huh 2024 uses CKA). CKA is a similarity in $[0, 1]$, not a bit count; it is reported alongside, not instead of, MI.

### 5.3 Mean cosine of per-input pairs

$\frac{1}{n}\sum_{i=1}^n \cos(\phi_j(x_i), \phi_k(x_i))$ on PCA-256 reps. Third sanity check; PRH-style comparable.

### 5.4 Euclidean distance profile

Histogram of paired distances $\|\phi_j(x_i) - \phi_k(x_i)\|_2$ on PCA-256. Useful for detecting mode-mixture phenomena.

---

## 6 · Aggregation — compute $N_\text{eff}$

After all pair estimates exist at $n = 1000$:

```
# Headline formula (see 06_analysis_and_decision_rules.md §3)
H_bar = mean(H_hat_j for j in 13)  # per-embedder entropy, KSG-estimated
sum_H = sum(H_hat_j for j in 13)
sum_I = sum(hat_I_MINE_median[j,k] for j<k)
N_eff = sum_H / (H_bar + (2/13) * sum_I / H_bar)

# Alternatives
N_eff_joint = block_entropy_estimate / H_bar
N_eff_min = 13 - max(hat_I_MINE_median[j,k] for j<k) / H_bar
N_eff_sum = 13 - sum_I / H_bar
```

Bootstrap CI on $N_\text{eff}$: 1000 resamples of the pair-estimates, propagated through the aggregation formula.

---

## 7 · Internal review (pre-external-surface)

Before any external write-up or memo inclusion:

1. Chris reviews the numeric output + flags.
2. Steve reviews the reporting language per [07_reporting_template.md](07_reporting_template.md) (compliance gate).
3. If anything surprises either: discuss before publishing.
4. Any amendment from this step is logged in [01_preregistration.md Appendix A](01_preregistration.md#appendix-a-amendment-log).

---

## 8 · Artifacts produced

After a clean run, `exp2_artifacts/` contains:

```
exp2_artifacts/
├── corpus/
│   ├── exp2_corpus_n100_<date>.npz
│   ├── exp2_corpus_n500_<date>.npz
│   ├── exp2_corpus_n1000_<date>.npz
│   ├── exp2_pca_n{100,500,1000}_<date>.npz
│   ├── exp2_sketch_matrices_<date>.npz
│   └── exp2_wikipedia5k_<date>.npz, exp2_code5k_<date>.npz, exp2_shakespeare2k_<date>.npz
├── results/
│   ├── exp2_results_mine_n{100,500,1000}.jsonl
│   ├── exp2_results_infonce_n{100,500,1000}.jsonl
│   ├── exp2_results_ksg_n{100,500,1000}.jsonl
│   ├── exp2_secondary_cosine_<n>.jsonl
│   ├── exp2_secondary_cka_<n>.jsonl
│   └── exp2_n_eff_<n>.json
├── logs/
│   ├── exp2_training_curves_<pair>_<seed>_<n>.csv
│   └── exp2_wall_time_summary.csv
├── figures/
│   ├── mi_matrix_heatmap_n1000.pdf
│   ├── estimator_agreement_n1000.pdf
│   ├── bootstrap_ci_width_n1000.pdf
│   ├── convergence_curves_n{100,500,1000}.pdf
│   └── n_eff_outcome_panel.pdf
└── report/
    ├── exp2_report_n1000.pdf
    ├── exp2_report_n1000.md
    └── exp2_report_sensitivity.pdf (Wikipedia / Code / Shakespeare)
```

All artifacts are git-tracked via their sha256 hashes in `docs/exp2/.artifact_manifest`. The artifacts themselves are stored in local disk, not git.

---

## 9 · Reproducibility checklist

Before marking EXP-2 complete:

- [ ] Preregistration SHA recorded in report header
- [ ] All three controls passed
- [ ] All 78 pairs have MINE + InfoNCE estimates at $n = 1000$
- [ ] 12 KSG pairs match MINE within 2 bits
- [ ] < 20 % of pairs flagged `DISAGREE_STRONG`
- [ ] Bootstrap CIs computed and stored
- [ ] Flags reported for every pair
- [ ] Sensitivity corpora run + $N_\text{eff}$ reported for each
- [ ] Chris + Steve internal review complete
- [ ] Artifact manifest committed with all sha256s

Only when every box is ticked does EXP-2 enter the external-surface pipeline.
