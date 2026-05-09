# Doc 09 Phase 5 — Self-Healing Operations

> **Estimated effort (per doc 09 §7):** 6-8 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 4 verify() API
> shipping and reality-loop integration in Phase 7.

## Goal

Phase 5 keeps the trained predictor calibrated against a moving reality.
As the harness improves, the corpus grows; as Python repos drift, panel
distributions shift; as outer-loop agents (Claude Code, external AI agents) refine their patch styles, the verifier's
input space shifts. Phase 5 closes the feedback loop:

1. Detect drift between predicted-oracle and real-oracle agreement on
   live attempts.
2. Trigger online fine-tuning when drift exceeds a threshold.
3. Use **EWC++** (Elastic Weight Consolidation) to fine-tune without
   catastrophic forgetting of the original training distribution.

Per doc 09 §1.1's compiler analogy, Phase 5 is the **garbage collector +
JIT recompiler**: it monitors runtime behavior and adapts the verifier
without restarting the system.

## Why this phase matters

The doc 09 §7 holdout-agreement gate is a **point-in-time** measurement
on the 2,400-entry corpus. A frozen verifier will drift as:

- New SWE-bench tasks land (the corpus grows beyond 300 tasks).
- Python language evolves (3.11 → 3.13 features in test fixtures).
- Outer-loop agent patches change distribution (Claude Code or external
  agents update their reasoning style, producing different panel-shapes).

Without Phase 5, the predictor's calibrated coverage decays from 0.90 →
0.70 → ... silently. Phase 5 catches the decay and fine-tunes back.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-monitor/`** — drift detector + online
   fine-tune trigger.
2. **Sliding-window agreement tracker**: maintains 1,000-attempt window
   of `(predicted_verdict, real_verdict)` tuples; flags when window
   agreement falls below `(holdout_agreement - 0.05)`.
3. **EWC++ regularizer**: per-parameter Fisher Information matrix from
   training; online fine-tune adds `λ_ewc * Σ F_i (θ_i - θ_i*)²` to the
   loss.
4. **Online fine-tune scheduler**: pulls last 1,000 drift-flagging
   attempts as a mini-corpus, fine-tunes for 100 steps, persists new
   checkpoint with `parent_checkpoint_sha` recorded.
5. **Rollback path**: if a fine-tuned checkpoint's coverage is worse
   than the parent's on the held-out conformal calibration set, revert.

## Definition of Done (per doc 09 §7)

- Drift detector triggers within 5,000 live attempts when synthetic
  drift is injected (mutate the corpus distribution by 10%).
- EWC++ fine-tune recovers ≥95% of original holdout agreement within
  100 steps.
- Catastrophic-forgetting test: after 5 consecutive drift-fine-tune
  cycles, original-corpus agreement degrades by ≤5%.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-monitor inject-drift --corpus ./data/mejepa-corpus \
    --drift-mode shift_distribution --strength 0.10
# expected:
#   drift_detected_after_attempts: <5000
#   fine_tune_steps: 100
#   recovered_agreement: >=0.95 * original
#   original_corpus_agreement_after: degradation <=0.05
```

## Component breakdown

### A. Sliding-window agreement tracker

```rust
struct AgreementWindow {
    capacity: usize,                // 1,000
    samples: VecDeque<(Verdict, OracleVerdict)>,
    cumulative_agreement: f32,
}

impl AgreementWindow {
    fn record(&mut self, predicted: Verdict, real: OracleVerdict) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back((predicted, real));
        self.cumulative_agreement = self.compute_agreement();
    }
    fn drift_score(&self, baseline: f32) -> f32 {
        baseline - self.cumulative_agreement
    }
}
```

### B. Fisher Information capture

After Phase 3 training completes, compute per-parameter Fisher
Information on the training set:

```rust
fn compute_fisher(
    predictor: &Predictor,
    train_set: &[(Panel, OracleVerdict)],
) -> Vec<f32> {  // one value per param
    let mut fisher = vec![0.0; predictor.num_params()];
    for (panel, real) in train_set {
        let logp = predictor.log_likelihood(panel, real);
        let grads = logp.backward();
        for (i, g) in grads.iter().enumerate() {
            fisher[i] += g * g;  // diagonal Fisher approx
        }
    }
    for f in &mut fisher { *f /= train_set.len() as f32; }
    fisher
}
```

Persist `(theta_star, fisher)` alongside the Phase 3 checkpoint.

### C. EWC++ loss

```rust
let total_loss = task_loss
    + lambda_ewc * (
        zip(theta_now, theta_star, fisher)
            .map(|(t, t_star, f)| f * (t - t_star).powi(2))
            .sum::<f32>()
    );
```

EWC++ is the per-task-version variant of EWC: each fine-tune cycle
records a new `(theta_star_k, fisher_k)` pair, and the regularizer sums
across all past versions:

```
L_total = L_task + Σ_k λ_k Σ_i F_{k,i} (θ_i - θ*_{k,i})²
```

### D. Rollback path

```rust
fn fine_tune_with_rollback(
    parent: Checkpoint,
    drift_corpus: &[Sample],
) -> Result<Checkpoint, FineTuneError> {
    let candidate = run_ewc_finetune(parent.clone(), drift_corpus, 100)?;
    let parent_coverage = evaluate_coverage(&parent, &calibration_set);
    let candidate_coverage = evaluate_coverage(&candidate, &calibration_set);
    if candidate_coverage >= parent_coverage * 0.95 {
        Ok(candidate)
    } else {
        Err(FineTuneError::CoverageRegression {
            parent_coverage,
            candidate_coverage,
        })
    }
}
```

## Critical path within Phase 5

1. Drift detector wired into `verify(patch)` call sites (every Phase 4
   call records the predicted verdict; pair it with the real verdict
   when Docker oracle re-confirms).
2. Fisher capture as a one-shot post-Phase-3 step.
3. EWC++ fine-tune loop reuses Phase 3 training infrastructure.
4. Coverage regression rollback gate.

## Dependencies

- ⏳ Phase 4 verify() API.
- ⏳ Phase 7 reality-loop integration (so live attempts trigger record-
  predicted-verdict + record-real-verdict pairs).
- ⏳ Phase 3 training infrastructure (reused for fine-tune).
- ⏳ `context-graph-witness` cert format extended to per-fine-tune-cycle
  certs.

## Implementation order (recommended)

**Sub-phase 5.A**: Sliding-window agreement tracker. ~1 day.

**Sub-phase 5.B**: Fisher Information capture as post-Phase-3 step. ~1 day.

**Sub-phase 5.C**: EWC++ regularizer + online fine-tune loop. ~2 days.

**Sub-phase 5.D**: Rollback path + coverage regression gate. ~1 day.

**Sub-phase 5.E**: Synthetic drift injection harness for verification.
~1 day.

**Sub-phase 5.F**: 5-cycle catastrophic-forgetting test. ~1-2 days.

## Risks (per doc 09 §8)

1. **Drift detector false positives**: tune sliding-window threshold
   conservatively; require 200 consecutive sub-threshold attempts before
   triggering.
2. **EWC++ Fisher staleness**: as more fine-tune cycles accumulate, the
   regularizer dominates and prevents adaptation. Cap `λ_ewc * total_fisher`
   relative to task loss.
3. **Catastrophic forgetting despite EWC++**: if 5-cycle test shows >5%
   degradation, increase `λ_ewc` from 1.0 → 10.0; if still degrading,
   reset to Phase 3 checkpoint.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7,
  §7`
- EWC++ reference: `06_RISKS_AND_NON_TRANSFERABLE.md` ✅ (cited as
  transferable when Fisher is approximate-diagonal).
- Phase 3 dependency: `phase3.md`
- Phase 4 dependency: `phase4.md`
- Phase 7 dependency: `phase7.md`
