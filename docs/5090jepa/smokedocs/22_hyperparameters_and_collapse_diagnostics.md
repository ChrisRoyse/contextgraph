# 22 - Hyperparameter Sweeps And Collapse Diagnostics

## Goal

Replace the single-config career run with a small, principled hyperparameter
sweep, and replace the single-collapse-metric (VICReg variance) story with a
suite of representation-collapse diagnostics drawn from current SSL practice
(rank, alignment + uniformity from Wang & Isola, off-diagonal covariance,
isotropy). Both feed paper figures and appendix D.

## Why This Matters

Current ablations (A0-A5) test **panel composition** and **VICReg on/off**.
They do not test **how the model behaves under reasonable hyperparameter
variation**. Reviewers reading the paper want to know the result is not a
single lucky configuration. JEPA literature (T-JEPA, LeJEPA, "Rethinking
JEPA", Apple ML, 2025-2026) has converged on a small set of collapse
diagnostics; we should match that practice.

## Hyperparameter Sweep

Three axes, kept tractable on a single 5090. Run at `paper_med_5k` (per doc
17) so each cell is ≤45 minutes.

### Axis 1 — Learning rate

Values: `{1e-4, 3e-4, 1e-3, 3e-3}` (4 cells). 3e-4 is the current default;
this brackets it by ~10× either direction. Cosine schedule kept fixed.

### Axis 2 — Latent dimension

Values: `{16, 32, 64, 128}` (4 cells). 16 is the current smoke default; 128 is
the doc-17 paper_full_50k value.

### Axis 3 — Batch size

Values: `{32, 64, 128, 256}` (4 cells). 32 is current; 256 hits doc-17 budget.

### Sweep Strategy

Full grid (4×4×4 = 64 cells) is wall-time prohibitive (~48 hours).
**One-axis-at-a-time** sweep:

- 4 LR cells at `(latent=16, batch=32)`;
- 4 latent cells at `(LR=3e-4, batch=32)`;
- 4 batch cells at `(LR=3e-4, latent=16)`.

12 runs total, ~9 hours wall time. Centerpoint `(3e-4, 16, 32)` is repeated
across axes for sanity check.

A second pass uses the **best LR** and **best latent** from the first pass to
re-sweep batch, capturing whatever interaction matters most. Total: ≤16 runs.

### Sweep Bundle Layout

```text
tmp/mejepa_research_runs/sweep_paper_med_5k_v1/
  sweep_manifest.json
  cells/
    lr_1e-4_latent_16_batch_32/    # full bundle per cell
    lr_3e-4_latent_16_batch_32/
    ...
  paper_tables/
    table_lr_sensitivity.csv
    table_latent_sensitivity.csv
    table_batch_sensitivity.csv
  plots/
    lr_sensitivity.svg
    latent_sensitivity.svg
    batch_sensitivity.svg
```

The sweep CLI is a wrapper over `research-smoke`:

```bash
target/debug/context-graph dynamicjepa hp-sweep \
  --base-config configs/dynamicjepa/career_taxonomy_paper_med_5k.json \
  --sweep-axis lr --sweep-values 1e-4,3e-4,1e-3,3e-3 \
  --career-training-seed 42 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --output-root tmp/mejepa_research_runs/sweep_paper_med_5k_v1 \
  --json
```

Each cell is its own immutable bundle. The wrapper writes `sweep_manifest.json`
with cell paths, hashes, and the axis variation.

## Collapse Diagnostic Suite

Four diagnostics, all computed from the val-split latent representations of a
trained model. Cheap (single forward pass over val) and deterministic.

### D1 — VICReg variance (existing)

Per-dimension standard deviation of latents. Min, p25, median, mean, p75, max.
Alarm if `min < γ_var × target` where `γ_var = 0.5` and `target` is the
VICReg target std (typically 1.0).

Already implemented; expose to paper table.

### D2 — Off-diagonal covariance Frobenius norm

Frobenius norm of the off-diagonal of the latent covariance matrix.
Normalize by `d × (d-1)` so the value is dimension-comparable. Alarm if it
crosses the highest value seen across all sweep cells (i.e. a relative alarm,
not an absolute threshold; the absolute number depends on dimension).

### D3 — Effective rank

PCA on val-split latents → number of components needed to capture 90% of
variance, divided by full latent dimension. Range `[1/d, 1]`. **Closer to 1 =
healthier.** Alarm if drops below 0.4 (sharp dimensional collapse).

This metric is the most read-friendly for reviewers and goes in the headline
figure F5.

### D4 — Alignment + Uniformity (Wang & Isola, 2020; revived in 2024-2026 JEPA work)

Two scalars:

- **Alignment**: $\mathbb{E}_{(x,x^+)}\|f(x) - f(x^+)\|_2^2$ where $(x,x^+)$
  are positive pairs (same trajectory step). Lower is better.
- **Uniformity**: $\log \mathbb{E}_{x, y \sim p_{\text{data}}} e^{-2 \|f(x) -
  f(y)\|_2^2}$. Lower is better (more uniform on the unit sphere).

Both computed on L2-normalized val latents. Pair (alignment, uniformity)
plotted as a 2D scatter across sweep cells; healthy models live in the
lower-left.

This pair is the single most-cited collapse-diagnostic in current SSL papers.
Adding it puts our paper in conversation with the literature.

### Optional D5 — Isotropy / SIGReg residual

LeJEPA (2025) argues the optimal latent distribution is isotropic Gaussian and
proposes SIGReg. We do not adopt SIGReg as the loss (out of scope), but we do
compute a **residual**: KL divergence between the empirical latent
distribution and a fitted isotropic Gaussian, on val.

This is a **diagnostic only**, not a regularizer. If LeJEPA's claim holds,
healthier configurations should have lower KL. Report in appendix only;
caveat that this is not our objective.

## New Tables And Figures

### `table_collapse_diagnostics.csv`

| run_id | seed | LR | latent_dim | batch | val_var_min | val_var_median | covar_frob | eff_rank | alignment | uniformity | sigreg_kl |

Rows: every sweep cell + every multi-seed cell from doc 15.

### F5 — Collapse diagnostic figure

2x3 panel:

- (1,1) variance distribution per cell;
- (1,2) effective rank per cell;
- (1,3) covariance Frobenius per cell;
- (2,1) alignment-uniformity scatter (the key plot);
- (2,2) per-cell val cosine vs effective rank (collapse predicts performance);
- (2,3) per-cell val cosine vs alignment (positives quality predicts
  performance).

## Sensitivity Reporting

For each axis, the paper appendix reports:

- best, median, worst val cosine across cells on that axis;
- whether all cells stayed in healthy zone (D3 ≥ 0.4, D4 alignment ≤ 0.5);
- the headline range of val cosine (this is the reviewer-friendly number).

If the headline range is tight (e.g. ±0.02 across all 12 cells), the paper
makes the strong claim *"the result is not sensitive to reasonable HP
variation"*. If it is wide (>0.10), the paper drops that claim and reports the
sensitivity honestly.

## Promotion Rule

Any HP value used in a paper-headline run (multi-seed paper_small or
paper_full) must:

- come from this sweep, not from a prior smoke;
- have its sweep bundle path recorded in the headline run's `run_manifest.json`
  under `hyperparameter_origin`.

This prevents accidental drift between sweep choices and headline
configurations.

## CI Integration (Coordinates With Doc 23)

A reduced 4-cell sweep (one axis, two values × two seeds) runs nightly. It is
**not** the headline sweep; it just catches regressions in the diagnostics
themselves (e.g. a code change that breaks effective-rank computation).

## Edge Cases (Fail-Closed)

- Sweep axis unknown to the schema: typed `VALIDATION` error.
- Cell directory already exists: typed `VALIDATION` error.
- Latent dim incompatible with predictor heads (e.g. a frozen-decoder run): typed
  `SCHEMA_MISMATCH` error.
- D3 / D4 / D5 NaN due to degenerate latent (all zeros): record value as
  literal `nan`, set `latent_collapsed=true` in the manifest, and **continue**
  (not a typed error; the failure is the result).
- KL divergence in D5 numerically unstable: clamp to 1e10 and flag in
  `numerical_warnings` in the cell manifest.

## Acceptance

This work is done when:

- `dynamicjepa hp-sweep` exists with all three axes;
- `table_collapse_diagnostics.csv` is generated automatically per run;
- F5 figure generator exists and renders deterministically;
- the sensitivity claim in the paper is backed by a real sweep bundle path;
- doc 13 is updated with the centerpoint sensitivity numbers.
