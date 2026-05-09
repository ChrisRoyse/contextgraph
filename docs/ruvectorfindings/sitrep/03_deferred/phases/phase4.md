# Doc 09 Phase 4 — Inference + Conformal Calibration + OOD

> **Estimated effort (per doc 09 §7):** 4-6 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 3 trained checkpoint.

## Goal

Phase 4 wraps the trained predictor in a production-grade verifier:
`MeJepaCompiler::verify(patch) -> Verdict`. The verdict carries a
**conformal-calibrated confidence interval** plus an **out-of-distribution
flag**. This is the first phase whose output is callable from outside
ME-JEPA — Phase 7 wires it to the reality loop.

Per doc 09 §1.1's compiler analogy, Phase 4 is the **assembler+linker**:
takes the trained model, packages it as a callable verifier, and emits
verdicts with calibrated uncertainty.

## Why this phase matters

Without conformal calibration, the predictor's softmax-as-probability is
miscalibrated — a "0.95 predicted pass probability" might mean the real
pass rate is 0.7 or 0.99. Conformal prediction (per `06_RISKS_AND_NON_TRANSFERABLE.md`
non-transferable list ❌-3 and `04_JEPA_APPLICATIONS.md §6`) gives a
calibrated coverage guarantee: at α=0.10, the prediction set covers the
true verdict with probability ≥0.90.

Without OOD detection, panels far from training distribution will get
overconfident predictions; a `verify()` call on a wildly out-of-domain
patch should return `OutOfDistribution` rather than a confident wrong
answer.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa/`** (extends Phase 2 crate) —
   `MeJepaCompiler::verify(patch) -> Verdict` API.
2. **Split conformal predictor** (✅ shipped in `context-graph-solver`
   Tier-1) re-used here. Calibration set is the 10% holdout from Phase 0
   split. α=0.10 default.
3. **OOD detection**: density-of-latent score on `f_θ(panel)` against
   training-distribution Gaussian mixture. Threshold tuned so 5% false
   OOD rate on calibration.
4. **Verdict struct**:
   ```rust
   enum Verdict {
       Pass {
           confidence_interval: (f32, f32),  // [lo, hi] at 1-α coverage
           per_test_probabilities: Vec<f32>,
       },
       Fail {
           confidence_interval: (f32, f32),
           per_test_probabilities: Vec<f32>,
       },
       OutOfDistribution { latent_density: f32 },
       Abstain { reason: String },           // panel build failure, etc.
   }
   ```

## Definition of Done (per doc 09 §7)

- `verify(patch)` produces a verdict with conformal interval.
- Empirical coverage on calibration set: `[1-α-ε, 1-α+ε]` for ε=0.02
  (i.e., 88-92% at α=0.10).
- OOD flag fires on a synthetic OOD set (random patches outside the
  training-corpus repos) at >85% TPR with <10% FPR on in-distribution
  calibration.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-verify --checkpoint ./checkpoints \
    --corpus ./data/mejepa-corpus \
    --calibrate
# expected:
#   coverage_at_alpha_0.10: 0.901  (within [0.88, 0.92])
#   ood_tpr: 0.91
#   ood_fpr: 0.07
#   median_interval_width: 0.18
```

## Component breakdown

### A. Conformal calibration

```rust
fn calibrate_conformal<F>(
    predictor: F,
    holdout: &[(Panel, OracleVerdict)],
    alpha: f32,
) -> ConformalQuantile
where F: Fn(&Panel) -> Vec<f32>,  // returns per-test pass probabilities
{
    let mut nonconformities = Vec::new();
    for (panel, real) in holdout {
        let predicted = predictor(panel);
        let nc = nonconformity(predicted, real);  // 1 - p(real_class)
        nonconformities.push(nc);
    }
    nonconformities.sort();
    let q_idx = ((1.0 - alpha) * (holdout.len() as f32 + 1.0)).ceil() as usize;
    ConformalQuantile { value: nonconformities[q_idx.min(holdout.len() - 1)] }
}
```

The shipped `context-graph-solver::conformal` (Tier-1) provides the
generic split conformal primitive. Phase 4 wraps it with the JEPA-specific
nonconformity function: `1 - p(real_aggregate_verdict)`.

### B. OOD detection

```rust
struct OodDetector {
    mu: Vec<Vec<f32>>,         // K Gaussian means in latent space
    inv_sigma: Vec<Mat>,        // K precision matrices
    log_det: Vec<f32>,
    threshold: f32,             // chosen so 5% FPR on calibration
}

fn ood_score(latent: &[f32], detector: &OodDetector) -> f32 {
    // log p(latent) under best-of-K Gaussian
    detector.mu.iter().zip(&detector.inv_sigma).zip(&detector.log_det)
        .map(|((m, ps), ld)| log_gaussian(latent, m, ps, *ld))
        .fold(f32::NEG_INFINITY, f32::max)
}
```

Fit K=8 Gaussians to the training-set latent encodings; the threshold is
the 5th percentile of training-set log-density.

### C. The verify() API

```rust
impl MeJepaCompiler {
    pub fn verify(&self, patch: &Patch) -> Verdict {
        let panels = match self.build_panels(patch) {
            Ok(p) => p,
            Err(e) => return Verdict::Abstain { reason: e.to_string() },
        };
        let latent = self.context_encoder.forward(&panels.t01_concat);
        let ood = ood_score(&latent, &self.ood_detector);
        if ood < self.ood_detector.threshold {
            return Verdict::OutOfDistribution { latent_density: ood };
        }
        let predicted_panel = self.predictor.forward(&latent);
        let probs = self.oracle_head.forward(&predicted_panel).softmax();
        let interval = self.conformal.interval_for(&probs, self.alpha);
        if probs.argmax_is_pass() {
            Verdict::Pass { confidence_interval: interval, per_test_probabilities: probs }
        } else {
            Verdict::Fail { confidence_interval: interval, per_test_probabilities: probs }
        }
    }
}
```

## Critical path within Phase 4

1. Phase 3 checkpoint must satisfy holdout-agreement gate ≥0.75.
2. Conformal calibration on the 240-entry holdout split.
3. OOD detector fit on training-set latents.
4. Wrap into `verify(patch)` API.
5. Verify the doc 09 §7 DoD: coverage in [0.88, 0.92], OOD TPR>0.85.

## Dependencies

- ⏳ Phase 3 trained checkpoint with holdout-agreement ≥0.75.
- ✅ `context-graph-solver::conformal` (Tier-1, shipped 2026-05-08).
- ⏳ K-means clustering on latents (small util, ~100 LOC).

## Implementation order (recommended)

**Sub-phase 4.A**: Plumb the trained checkpoint into `MeJepaCompiler::new`.
~0.5 day.

**Sub-phase 4.B**: Reuse `context-graph-solver::conformal` to fit the
quantile on the holdout. ~1 day.

**Sub-phase 4.C**: K-means + Gaussian-mixture OOD detector. ~1.5 days.

**Sub-phase 4.D**: `verify(patch)` API + Verdict struct + abstain logic.
~1 day.

**Sub-phase 4.E**: Empirical coverage + OOD eval. ~1 day.

## Risks (per doc 09 §8)

1. **Coverage falls outside [0.88, 0.92]**: indicates calibration
   distribution diverges from prediction distribution. Re-fit quantile
   on a larger holdout split (Phase 0 produces 240 holdout entries; can
   borrow from cal split if needed).
2. **OOD threshold is brittle**: fix by adaptive thresholding per
   per-task density distribution rather than global.
3. **Latency**: each `verify(patch)` runs panel build + 2 encoder
   forwards + predictor forward + OOD score. Target <100ms on RTX 5090
   for batch-1.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.6,
  §7`
- Conformal primitive: `02_shipped/tier1_foundations.md` (`context-graph-solver::conformal`)
- Doc 06 non-transferable list: `06_RISKS_AND_NON_TRANSFERABLE.md` ❌-3
  (split conformal generic, not LLM-bias-baked-in)
- Phase 3 dependency: `phase3.md`
