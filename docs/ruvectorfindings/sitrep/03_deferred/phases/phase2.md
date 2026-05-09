# Doc 09 Phase 2 — Panel + Predictor + Target Encoder

> **Estimated effort (per doc 09 §7):** 7-10 days.
> **Status (2026-05-09):** DEFERRED — depends on Phase 1 instruments
> being shippable (currently 1 of 14).

## Goal

Phase 2 implements the **JEPA itself** — the predictor that learns to
map context-time-step panels to target-time-step panels in latent
space. This is the core machine-learning model of ME-JEPA-Code.

Per doc 09 §1.1's compiler analogy, Phase 2 is the **optimization
passes**: it transforms an IR (the panel at time t) into a downstream
IR (the predicted panel at time t+1) via a learned function.

## Why this phase matters

The predictor IS the verifier. Phase 4's `MeJepaCompiler::verify(patch)`
is just a wrapper around: "build panel[t=0..1] for the candidate
patch, run predictor → predicted panel[t=2], read off the
`E_Oracle` slice from the predicted panel, return predicted-pass
probability."

Without a working predictor, ME-JEPA can't make predictions; without
the EMA target encoder + VICReg loss, the predictor collapses to
trivial constant outputs.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa/`** — new crate with the JEPA
   architecture.
2. **Panel materialization**: given a task + a patch attempt, produce
   `panel[t=0]`, `panel[t=1]`, `panel[t=2]`.
3. **Predictor**: 6-layer transformer, hidden dim 1024, 8 heads,
   GeLU activations, residual + layer-norm. Input
   `panel[t=0..1]` concatenated → 8,192-d. Output predicted
   `panel[t=2]` → 4,096-d (the doc says 4,096; with the §3.5
   expansion the predictor's output is the 5,120-d panel — verify the
   doc's intended dim during implementation).
4. **EMA target encoder**: same architecture as the context encoder;
   weights updated via `ξ ← τ ξ + (1 - τ) θ` with `τ = 0.996` per
   V-JEPA standard.
5. **VICReg loss**: variance + invariance + covariance terms;
   defaults `λ_var = 25`, `λ_cov = 1`, `λ_inv = 25`, `γ = 1`.
6. **Oracle head** on top of predictor output:
   `Linear<5120, num_tests + 1>` (the +1 is the "exception" class).

## Definition of Done (per doc 09 §7)

- Forward pass: `panel[t=0..1] -> predictor -> predicted panel[t=2]`
  with output dim 5,120.
- Loss is finite on a single-batch dry-run.
- Memory footprint < 8 GB VRAM at batch 64 on RTX 5090.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-train --dry-run --batches 1 --batch-size 64
# expected:
#   forward latency: <50ms/batch
#   loss: finite, ~order(10²) before training
#   no NaN/Inf
```

## Component breakdown

### A. Panel materialization

Given an attempt's `(task_id, patch_id)`:

- **panel[t=0]** — pre-patch state. Set: E_AST, E_CFG, E_DataFlow,
  E_TypeGraph, E_Test, E_Problem on the original code; E_Diff = 0;
  E_Trace = 0; E_Witness = 0; E_Oracle = 0; E_CommitMsg = 0.
- **panel[t=1]** — post-patch, pre-execution. E_AST, E_CFG,
  E_DataFlow, E_TypeGraph re-evaluated on patched code; E_Diff
  populated; E_CommitMsg populated; E_Trace = 0; E_Witness partial
  (just patch-application entries); E_Oracle = 0.
- **panel[t=2]** — post-execution. ALL slots populated. The
  `E_Oracle` slice is the supervision signal.

Storage in RocksDB CF `CF_MEJEPA_PANELS`, keyed by
`(attempt_id, time_step)`.

### B. Context encoder `f_θ` and target encoder `f_ξ`

Both are the same architecture; `f_ξ` is an EMA shadow of `f_θ`.
Frozen during loss computation (`f_ξ.weights.requires_grad = false`,
update via the EMA rule outside the gradient).

### C. Predictor `g_φ`

Maps `(panel[t=0] || panel[t=1]) -> predicted panel[t=2]`. 6-layer
transformer with the standard pre-LN architecture.

### D. Loss

```
L_total = L_predict + λ_var * L_variance + λ_cov * L_covariance
                    + λ_inv * L_invariance
```

Where `L_predict = HuberLoss(g_φ(f_θ(context)), f_ξ(target), δ=1)`.

VICReg terms prevent collapse:
- **Variance**: per-dim std must exceed `γ` on a batch.
- **Covariance**: off-diagonal cov of latent must be small.
- **Invariance**: redundancy term tying context and target latents.

### E. Oracle head

Sits on top of predictor output: `OracleHead { project_to_logits:
Linear<4096, num_tests + 1> }` per doc 09 §2.5. At inference,
softmax(logits) → per-test pass probability + a single "exception"
slot. (With the §3.5.5 panel expansion, replace 4,096 with 5,120
during implementation.)

## What's deferred (entire phase)

Nothing has been shipped from Phase 2 yet. Foundation primitives
are in place:
- ✅ `context-graph-mejepa-instruments` Panel + Instrument trait —
  consumed by panel-materialization step.
- ✅ `context-graph-tier-compression` — for cold-tier panel storage.
- ✅ `context-graph-mejepa-corpus` — corpus from which `(task_id,
  patch_id)` tuples are drawn.

What needs to be built:
- All of A-E above.
- `crates/context-graph-mejepa/` new crate.
- candle CUDA setup for the RTX 5090 sm_120a.

## Critical path within Phase 2

1. Phase 1 must ship enough instruments that `panel[t=2]` can be
   materialized. Practically this means at minimum E_AST, E_CFG,
   E_DataFlow, E_TypeGraph, E_Diff, E_Witness, E_Oracle. Without
   these, all panels are mostly-zeros and the predictor learns
   nothing.
2. Phase 0 Docker oracle must ship so `E_Oracle` slot of `panel[t=2]`
   is populated with real verdicts.
3. THEN Phase 2 itself: panel materializer → context encoder →
   predictor → target encoder → loss.
4. Dry-run gate: forward pass produces finite loss at batch 64 on
   RTX 5090.

## Dependencies

- ⏳ Phase 1 instruments (at least 7 of 14 — the ones reachable at
  panel[t=0..1]).
- ⏳ Phase 0 Docker oracle (for `panel[t=2]`'s `E_Oracle` slot).
- ⏳ candle 0.9.2-alpha or later with sm_120a CUDA support (per
  CLAUDE.md model+platform pins).

## Implementation order (recommended within Phase 2)

**Sub-phase 2.A**: panel materializer. Read `(task_id, patch_id)`
from the corpus; produce panel[t=0..2] using the shipped instruments
+ deferred-but-stubbed instruments. Persist to `CF_MEJEPA_PANELS`.
~2 days.

**Sub-phase 2.B**: encoder + EMA target. ~2 days.

**Sub-phase 2.C**: predictor (6-layer transformer). ~2 days.

**Sub-phase 2.D**: VICReg loss. ~2 days.

**Sub-phase 2.E**: oracle head + dry-run verification. ~1 day.

## Risks (per doc 09 §8)

1. **Will JEPA generalize across repositories?** Open empirical
   question; the doc says train on a diverse subset, evaluate on
   held-out repos.
2. **Are 5,120-d panels expressive enough?** If test agreement
   plateaus below 0.95, increase to 8,192-d at the cost of doubled
   compute.
3. **Memory footprint at batch 64**: RTX 5090 has 32 GB VRAM. The
   predictor + 2× encoders + VICReg activations should fit under
   8 GB at batch 64; monitor during dry-run.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.3,
  §2.4, §2.5, §7`
- Implementation plan: `../../04_implementation_plans/plan_phase2_predictor.md`
- Phase 1 dependency: `phase1.md`
- Phase 0 dependency: `phase0.md`
