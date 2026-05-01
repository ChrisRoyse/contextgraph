# EXP-2 Validation Harness

The forensic-engineering layer. Trust nothing; verify everything.

## 1. Validation Hierarchy

| Level | Object | Test |
|---|---|---|
| L1 | Embedder weights | sha256 match registry; produce non-zero vectors on stub inputs |
| L2 | Corpus manifest | hash matches; manifest schema-valid; deduplicated |
| L3 | Embeddings | non-zero; correct dimensions; no NaN; L2 norm > 0 |
| L4 | PCA models | hash; explained-variance ≥ threshold; round-trip consistent |
| L5 | Projected vectors | L2-normalized; no NaN; correct shape |
| L6 | InfoNCE training | converges; train-val gap < threshold; deterministic |
| L7 | KSG estimates | non-negative; consistent across $k$ sweep |
| L8 | Bootstrap CIs | tight enough; reproducible |
| L9 | Effective rank | three methods produce sane numbers |
| L10 | Pipeline replay | numerical identity within $10^{-5}$ |
| L11 | Headline number | operator approves before publication |

## 2. Replay Determinism Test (Per Constitution C6)

The pipeline produces a reference set of artefacts. Replay re-runs the entire pipeline from T-006 (extract embeddings) through T-024 (audit chain) with the same seed. Pass criterion: every reported scalar within $10^{-5}$ of reference.

### Implementation: `scripts/exp2/replay.py`

```python
def replay(reference_dir: Path, replay_dir: Path) -> ReplayResult:
    """Compare replay artefacts to reference. Return pass/fail per artefact."""
    results = []

    # Compare per-pair MI estimates.
    for pair in load_pairs(reference_dir):
        ref = load_pair(reference_dir / "pair_results" / f"{pair}.json")
        rep = load_pair(replay_dir / "pair_results" / f"{pair}.json")

        results.append(check_within_tolerance(
            ref, rep, scalars=["infonce_estimate_nats", "ksg_estimate_nats", "redundancy"],
            tolerance=1e-5,
        ))

    # Compare N_eff scalars.
    ref_n = load_n_eff(reference_dir)
    rep_n = load_n_eff(replay_dir)
    results.append(check_within_tolerance(
        ref_n, rep_n,
        scalars=["n_eff_redundancy_adjusted.point", ...],
        tolerance=1e-5,
    ))

    # Aggregate and return.
    return ReplayResult(per_artefact=results, overall_pass=all(r.pass_ for r in results))
```

If any pair fails determinism, the pipeline has a nondeterminism source we have not eliminated. We investigate; we do not paper over.

### Known Sources Of Nondeterminism

| Source | Mitigation |
|---|---|
| cuDNN convolution algorithm selection | `torch.use_deterministic_algorithms(True)`; performance penalty acceptable |
| Random seed from time | All seeds explicit and set |
| dict iteration order | Python 3.7+ guarantees insertion order; we rely on this |
| Floating-point summation order in CUDA reductions | Use deterministic reduction kernels where available |
| InfoNCE batch shuffling | Seed the DataLoader |
| Bootstrap resampling | Seed the random generator |

## 3. Synthetic Ground-Truth Validation (T-013)

Before any production pair runs, verify the pipeline on synthetic data with known MI:

### 3a. Gaussian Pair Sweep
Generate $u_i \sim \mathcal{N}(0, I_{32})$, $v_i = \rho u_i + \sqrt{1-\rho^2} \epsilon_i$ with $\epsilon_i \sim \mathcal{N}(0, I_{32})$.

Closed-form MI: $I(U;V) = -\frac{1}{2} \log(1 - \rho^2) \cdot 32 / \log e$ nats.

Sweep $\rho \in \{0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99\}$.

Pass criteria:
- InfoNCE recovery: |estimate - truth| < 0.1 nats for $\rho \leq 0.7$.
- KSG recovery: |estimate - truth| < 0.15 nats for $\rho \leq 0.7$.
- For $\rho > 0.7$: estimators may underestimate due to high-MI bias; document, don't fail.

### 3b. Independence Check
Generate $u, v$ both from $\mathcal{N}(0, I_{32})$ independently.
Expected MI: 0.
Pass: both estimators return ≤ 0.05 nats.

### 3c. Real-Embedder + Random Test
Take E1 vectors from the audit corpus + 32-D random Gaussian.
Expected MI: 0 (no statistical relationship between random noise and real text).
Pass: both estimators return ≤ 0.1 nats.

### 3d. Self-MI
Take E1 vectors paired with themselves: $(u, u)$.
Expected: $I(U; U) = H(U)$ (max possible).
Both estimators should return high MI; KSG more stable than InfoNCE in this regime.

### Validation Report

`synthetic_validation_report.json`:
```json
{
  "gaussian_sweep": {
    "rho_0.0": {"truth_nats": 0.0, "infonce": 0.04, "ksg": 0.02, "pass": true},
    "rho_0.5": {"truth_nats": 4.59, "infonce": 4.51, "ksg": 4.62, "pass": true},
    ...
  },
  "independence": {"infonce": 0.03, "ksg": 0.01, "pass": true},
  "e1_random": {"infonce": 0.04, "ksg": 0.02, "pass": true},
  "e1_self": {"infonce": 9.2, "ksg": 11.4, "h_ksg": 11.5, "pass": true},
  "overall_pass": true
}
```

If `overall_pass=false`, the production run is blocked.

## 4. Failure Modes & Diagnostic Procedures

### F1: Pair MI Estimate Implausibly Low
*Example*: $I(E1; E10) = 0.05$ nats (expected high — both dense semantic encoders).

**Diagnostic**:
1. Check PCA explained variance for E1 and E10. If either < 0.85, the projection lost critical information.
2. Plot 2D projection (TSNE/UMAP) of (E1, E10) joint space; visually inspect for structure.
3. Re-run KSG with $k = 10$ (less local-noise-sensitive).
4. Re-run InfoNCE with longer training (10k steps).

**Resolution**:
- If PCA was the issue: refit PCA at d=64 for the affected embedder, re-run from T-009.
- If methodology was the issue: document and re-run.

### F2: Pair MI Estimate Implausibly High
*Example*: $I(E2; E14) = 8$ nats (expected low — temporal vs multilingual).

**Diagnostic**:
1. Check for data leakage: did the same input contribute to both embedders' training data?
2. Check InfoNCE for overfitting: train-val gap > 0.1 nats?
3. Check KSG for $k$ sensitivity.

**Resolution**:
- Re-run diagnostics to identify the cause.
- Production remains blocked until the pair succeeds with both estimators under a pre-registered method.

### F3: InfoNCE Doesn't Converge
*Symptom*: training loss flat or oscillating after warmup.

**Diagnostic**:
1. Plot loss curves.
2. Check input statistics: are u, v near-zero? (PCA failed?)
3. Check learning rate.

**Resolution**:
- Diagnose optimizer, projection, and data-quality state.
- Any hyperparameter change requires a documented methodology revision before a new full production run.
- Production remains blocked until the pair succeeds with both estimators.

### F4: KSG Numerical Issues
*Symptom*: NaN, Inf, or extreme values from KSG.

**Diagnostic**:
1. Verify input is finite, no NaN.
2. Check `npeet` version + commit.
3. Sweep $k$.

**Resolution**:
- Switch to alternative implementation (sklearn-based).
- If implementation-stable but value-stable, document.

### F5: Bootstrap CIs Wider Than Expected
*Threshold*: median CI width > 0.30 nats.

**Diagnostic**:
1. Check corpus diversity: are all 800 inputs near-similar?
2. Check sample-size sufficiency.
3. Re-run with `n_resamples = 50000`.

**Resolution**:
- Increase corpus to 1200 inputs (extension).
- Document smaller corpus's CI width as a known limitation.

### F6: Cross-Validation Agreement < 80%
*Threshold*: per Constitution C3.

**Diagnostic**:
1. List failing pairs.
2. Check whether failures cluster by embedder type.
3. Check whether estimator hyperparameters are wrong.

**Resolution**:
- Methodology revisited; potentially adjust hyperparameters.
- Production run repeated with revised hyperparams.
- The decision to revise hyperparams is **logged with rationale** and is a methodology deviation, not a tuning operation.

### F7: $N_{\mathrm{eff}}$ Scalars Diverge Across Methods
*Threshold*: $N_{\mathrm{eff}}^{(1, 2, 3)}$ disagree by > factor of 2.

**Diagnostic**:
1. Check entropy estimates — do per-embedder entropies look right?
2. Check spectrum — are eigenvalues collapsing to one mode?
3. Methods measure slightly different things; some divergence expected.

**Resolution**:
- Document divergence in the publishable summary.
- Headline = redundancy-adjusted; note that other methods give different values for [reasons].

## 5. Sanity Checks At Each Phase

### Phase A
- [ ] Each stratum has 100 inputs.
- [ ] No duplicates anywhere.
- [ ] Manifest hash logs.

### Phase B
- [ ] All 800 × 14 = 11,200 embeddings non-zero.
- [ ] Per-embedder dimensional consistency.
- [ ] PCA explained variance ≥ 0.85 per embedder; otherwise fail and revise the pre-registered projection method before rerun.

### Phase C
- [ ] Synthetic validation passes.
- [ ] Pilot pass on 4 pairs returns expected qualitative results.
- [ ] Operator sign-off.

### Phase D
- [ ] All 91 pairs estimated by both methods.
- [ ] Cross-validation agreement ≥ 80%.
- [ ] Bootstrap CIs computed.

### Phase E
- [ ] Three N_eff methods give plausible (positive, reasonable-magnitude) values.
- [ ] Heatmap renders, all 14 labels visible.
- [ ] CSV table exports cleanly.

### Phase F
- [ ] Audit chain validates.
- [ ] FSV replay passes (within $10^{-5}$).
- [ ] Operator sign-off on headline.

Each phase has a `phase_${X}_validation.json` artefact with each check's pass/fail.

## 6. Known Limitations Documented Up Front

We **publish** these as caveats in the paper patch:

1. **PCA-32 may bias MI estimates** for embedders with native-rank > 32 due to information loss; effective MI on raw vectors may differ.
2. **InfoNCE is a lower bound**; true MI may be higher.
3. **KSG-1 underestimates MI in high-mutual-information regimes** ($\rho > 0.9$); we mitigate via two-estimator agreement.
4. **800 inputs is small** for some pair MI estimates; some bootstrap CIs may be wider than 0.20 nats.
5. **Asymmetric variants** (E5, E8, E10) are tested only on primary direction; dual-variant MI is future work.
6. **The 14-embedder panel is post-E14 migration**; the paper's original 13-embedder claim becomes a 14-embedder measurement, which is intentional ("we now have E14"; the methodology is identical).

## 7. Validation Of The FSV Process Itself

A peer (or independent reviewer) can:
1. Clone the GitHub release.
2. Run `make replay` (provided in the harness).
3. Receive `replay_log.json` showing per-pair pass/fail.
4. If all pass within tolerance, the result is independently reproducible.

Independent replication is the success criterion for the open-source release.

## 8. Audit Chain Cross-Verification

Every artefact's parent_artefact_id chain validates back to the corpus manifest. A reviewer can:
1. Take the corpus manifest hash from the paper.
2. Look up the artefact bundle.
3. Verify each artefact's sha256 matches the chain.
4. Verify the chain's structure (no orphans, no cycles).

The audit chain validation is itself testable (`tests/test_audit_chain.py`).

## 9. Operator Override

If the operator decides to publish a headline number despite a validation failure (per Constitution C16), the override is **logged with rationale**:

```json
{
  "override_id": "exp2_override_001",
  "override_at": "2026-04-XX UTC",
  "override_by": "operator",
  "override_target": "cross_validation_agreement",
  "override_value": "0.78 instead of 0.80 threshold",
  "rationale": "Pair (E6, E13) drove 12 of the 18 disagreements; both are SPLADE-family with known overlap; agreement metric is dominated by this single pair structure. Methodology is sound; the pair is correctly identified as redundant.",
  "audit_trail_continued": true
}
```

Overrides are public in the audit chain. Reviewers who disagree with an override have full evidence to argue.

## 10. Continuous Improvement

After EXP-2 ships, lessons learned populate `tasks/exp2/lessons.md` per CLAUDE.md project conventions. Future experiments (EXP-3 sample-complexity sweep, EXP-1 cross-encoder triangulation, domain-fork validations) inherit the validation infrastructure.

The validation harness *is* the experiment's most reusable asset. Treat it accordingly.
