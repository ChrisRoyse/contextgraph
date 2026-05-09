# Doc 09 Phase 8 — Evaluation Against End Goal

> **Estimated effort (per doc 09 §7):** 4-6 days.
> **Status (2026-05-09):** DEFERRED — depends on all of Phase 0..7
> shipping.

## Goal

Phase 8 measures whether the system meets the doc 09 §1 end goal:
**replace human code review with reality verification at ≥0.95
correlation between ME-JEPA verdicts and ground-truth Docker oracle
verdicts on a held-out test set.**

This phase is the closing gate. If it doesn't pass, the system is not
production-ready regardless of how clean the per-phase DoDs were.

Per doc 09 §1.1's compiler analogy, Phase 8 is the **conformance test
suite**: it measures the entire pipeline end-to-end against external
reality, not just against itself.

## Why this phase matters

Each prior phase has its own DoD (training agreement ≥0.75; conformal
coverage in [0.88, 0.92]; latency <100ms; etc.). But the doc 09 §1 end
goal is a single number — Pearson/Spearman correlation between
ME-JEPA's predicted-pass-probability and Docker's real-pass-rate on a
test split that the model has never seen — and it must clear ≥0.95.

If correlation is 0.85 instead, the verifier is useful but not
load-bearing for "replace code review" — Phase 8 informs whether to
ship to production, increase panel dim, expand corpus, or change
architecture.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-eval/`** — evaluation harness binary.
2. **300-instance held-out evaluation**: take all 300 SWE-bench Lite
   tasks, generate 5 mutated variants each (different mutations from
   training corpus), run both ME-JEPA verify() and the real Docker
   oracle, compare.
3. **Per-axis evaluation reports**:
   - **Aggregate verdict agreement** (Pass-vs-Fail vs ground truth).
   - **Per-test agreement** (each individual test's predicted vs real
     pass).
   - **Confidence calibration** (predicted probability vs empirical
     pass rate).
   - **OOD precision/recall** (predicted OOD vs human-labeled OOD on
     a 50-attempt mini-set).
   - **Latency distribution** (p50, p95, p99 for verify()).
4. **Writeup**: `docs/ruvectorfindings/10_PHASE8_EVAL_REPORT.md` with
   final correlation numbers, comparison vs the doc 09 §1 ≥0.95 target,
   and ship-or-iterate decision.

## Definition of Done (per doc 09 §7)

The doc 09 §1 end goal:
- **Aggregate-verdict Pearson correlation ≥ 0.95** between predicted-
  pass-probability and Docker pass rate on the 1,500-attempt held-out
  set.
- **Per-test Spearman correlation ≥ 0.85**.
- **Calibration ECE ≤ 0.05** (Expected Calibration Error).
- **OOD precision ≥0.85, recall ≥0.85** on the human-labeled mini-set.

If the correlation gate fails, Phase 8's deliverable is the diagnostic
report explaining *which* of the 4 axes failed and pointing to the
phase that needs iteration:
- Aggregate fails → revisit Phase 2 panel dim or Phase 3 hyperparameters.
- Per-test fails → Phase 1 instruments insufficiently expressive
  (likely E_AST/E_TypeGraph).
- Calibration fails → Phase 4 conformal calibration set too small or
  miscalibrated.
- OOD fails → Phase 4 OOD detector needs richer mixture (more Gaussians).

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-eval --corpus ./data/mejepa-corpus \
    --checkpoint ./checkpoints \
    --output ./reports/phase8.json
# expected:
#   aggregate_pearson: 0.96
#   per_test_spearman: 0.87
#   ece: 0.04
#   ood_precision: 0.88
#   ood_recall: 0.86
#   median_latency_ms: 38
#   p99_latency_ms: 91
#   verdict: SHIP   # or ITERATE_PHASE_<n>
```

## Component breakdown

### A. Held-out evaluation set construction

```rust
fn build_eval_set(corpus: &MutationCorpus, rng: &mut SplitMix64) -> Vec<EvalSample> {
    let mut samples = Vec::new();
    for task in corpus.holdout_tasks() {  // 240 holdout-split tasks
        for category in MUTATION_CATEGORIES {
            // skip if (task, category) already in corpus (no leakage)
            if corpus.contains(&task.id, category) { continue; }
            // generate fresh mutation with NEW seed
            let mutated = apply_mutation(category, &task.source, rng)?;
            let real_verdict = run_docker_oracle(&task.id, &mutated)?;
            samples.push(EvalSample {
                task_id: task.id.clone(),
                category,
                source: mutated,
                ground_truth: real_verdict,
            });
        }
    }
    samples
}
```

The held-out tasks are the 10% holdout from the Phase 0 split. Generating
fresh mutations with a different RNG seed (not the corpus-generation
seed) keeps the eval set independent.

### B. Aggregate-verdict correlation

```rust
fn aggregate_correlation(samples: &[EvalSample], compiler: &MeJepaCompiler) -> f32 {
    let predicted: Vec<f32> = samples.iter()
        .map(|s| match compiler.verify_source(&s.task_id, &s.source) {
            Verdict::Pass { per_test_probabilities, .. } => mean(&per_test_probabilities),
            Verdict::Fail { per_test_probabilities, .. } => mean(&per_test_probabilities),
            Verdict::OutOfDistribution { .. } => 0.5,  // abstain → neutral
            Verdict::Abstain { .. } => 0.5,
        })
        .collect();
    let real: Vec<f32> = samples.iter()
        .map(|s| s.ground_truth.pass_rate())
        .collect();
    pearson(&predicted, &real)
}
```

### C. Per-test Spearman

For each `(task, mutation)` in the eval set, both predicted
per-test-pass-probabilities and real per-test outcomes are aligned by
test name. Spearman rank correlation across the joined vector measures
whether the predictor preserves the *ordering* of test difficulties.

### D. Calibration ECE

```rust
fn expected_calibration_error(samples: &[EvalSample], compiler: &MeJepaCompiler) -> f32 {
    let mut bins = vec![(0u64, 0u64, 0.0f32); 10];  // (count, n_correct, prob_sum)
    for s in samples {
        let predicted_p = ...;
        let real_pass = s.ground_truth.aggregate_pass();
        let bin = (predicted_p * 10.0) as usize;
        let bin = bin.min(9);
        bins[bin].0 += 1;
        bins[bin].1 += real_pass as u64;
        bins[bin].2 += predicted_p;
    }
    let total = samples.len() as f32;
    bins.iter().map(|(n, k, p_sum)| {
        if *n == 0 { return 0.0; }
        let avg_p = p_sum / *n as f32;
        let avg_real = *k as f32 / *n as f32;
        (*n as f32 / total) * (avg_p - avg_real).abs()
    }).sum()
}
```

### E. OOD precision/recall

Hand-label 50 attempts as IN/OOD by reading the patch (a human-time
investment of ~2 hours). Score the detector against the labels.

## Critical path within Phase 8

1. Phase 0..7 must all be shipped + meet their per-phase DoDs.
2. Build the 1,500-attempt held-out eval set (fresh mutations on
   holdout tasks; run Docker oracle on each).
3. Compute the 4 metrics.
4. Write the `10_PHASE8_EVAL_REPORT.md` with ship/iterate decision.

## Dependencies

- ⏳ All of Phase 0..7 shipped.
- ⏳ Docker oracle (Phase 0 deferred item A) since the eval set's ground
  truth is real Docker verdicts.
- ⏳ Phase 0 holdout split with cross_cut_weights = [0,0,0] (✅ shipped
  primitive).

## Implementation order (recommended)

**Sub-phase 8.A**: Eval-set construction with fresh-seed mutations.
~1 day.

**Sub-phase 8.B**: Run Docker oracle on the 1,500 attempts. **Operational
batch — multi-hour run.** ~2 days.

**Sub-phase 8.C**: Compute the 4 metrics. ~1 day.

**Sub-phase 8.D**: Hand-label 50 attempts for OOD ground truth. ~0.5 day
of human work.

**Sub-phase 8.E**: Write `10_PHASE8_EVAL_REPORT.md`. ~0.5 day.

## Outcome scenarios

### Scenario A: All 4 metrics pass (ship-able)
- Aggregate ≥0.95, Per-test ≥0.85, ECE ≤0.05, OOD ≥0.85/0.85.
- **Action**: ship to production. The reality loop's
  `mcp__cgreality__mejepa_verify` is now the primary verifier.

### Scenario B: Aggregate ≥0.90 but <0.95 (close)
- Likely fix: increase panel dim 5,120 → 8,192 (Phase 2 Risk #2),
  retrain (Phase 3), re-evaluate.
- Cost: ~2× compute; ~5 additional days.

### Scenario C: Per-test fails but aggregate OK
- Indicates instruments aren't expressive enough at the per-test level.
- Likely fix: train richer E_AST and E_TypeGraph encoders (Phase 1
  Sub-phase 1.E and 1.F) instead of hashed-feature placeholders.

### Scenario D: Calibration fails
- Likely fix: increase calibration set (use Phase 0 cal split's 360
  entries — was using only holdout's 240) OR adopt Mondrian-style
  per-task calibration.

### Scenario E: OOD fails
- Likely fix: increase mixture components K=8 → K=32; or replace
  Gaussian mixture with normalizing flow density estimator.

## Risks (per doc 09 §8)

1. **Bench fixture drift**: SWE-bench Lite repos may update during the
   weeks between Phase 0 corpus build and Phase 8 eval. Pin Docker
   image SHA for both corpus generation and eval to prevent silent
   ground-truth shifts.
2. **Operational batch length**: 1,500 Docker runs at ~3 min each = ~75
   hours serial. Run in parallel (10× concurrent) → ~7.5 hours.
3. **Hand-labeling OOD**: human inter-rater agreement on "is this
   patch OOD" may be poor. Have 2 humans label independently and
   measure Cohen's kappa; if <0.7, refine the labeling rubric.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §1, §7`
- All upstream phases: `phase0.md`..`phase7.md`
- Final writeup target: `docs/ruvectorfindings/10_PHASE8_EVAL_REPORT.md`
  (created during this phase, not yet authored)
