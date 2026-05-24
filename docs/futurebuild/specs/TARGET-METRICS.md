# TARGET METRICS — The Perfect-State Numerical Predicate

> **This document is the single source of truth for what "the system works" means numerically. Every commit, every issue, every FSV harness, every architectural decision exists to move the project toward the numbers below. Nothing else is the goal. These are not aspirational targets — they are concrete, falsifiable, and AI-achievable on the existing aiwonder substrate.**
>
> When all numbers in this doc hit simultaneously, human review of AI-generated Python patches becomes statistically unjustified. That is the definition of done for ME-JEPA-Code.

**Authority:** This file outranks every other doc in `docs/futurebuild/` on the numerical predicate. If any other doc states a target inconsistent with this file, this file wins. Doctrine in `CLAUDE.md` outranks this file only on operational invariants (slot identity, anti-compensation, ambiguity boundary, no inner LLM); this file outranks `CLAUDE.md` on numerical targets.

**Pinned GitHub context:** see the `[P0] CONTEXT — Numerical target sheet (the perfect-state predicate)` issue. All agents must read both files at session start.

**Aiwonder-only execution context:** every measurement of every number below executes on the aiwonder datacenter PC per `CLAUDE.md §6.7`. Production durable state at `/zfs/archive/contextgraph/...`; hot state at `/zfs/hot/contextgraph/...`; no `/mnt/d` / `/mnt/c`.

---

## 0. One-line predicate

```
THE_SYSTEM_WORKS ⇔
  prediction_oracle_correlation ≥ 0.95
  STABLE × 4 consecutive rolling windows
  AND cross_panel_oracle_correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
  AND per-cell quality contract holds for every (mutation_category × language) cell
  AND online mistake-loop convergence holds
  AND every doctrine invariant equals its required value exactly
```

When this predicate evaluates `true` for 4 consecutive rolling windows, the predictor replaces human review. That is the goal. Nothing else is the goal.

---

## 1. Tier 1 — The 4 ship-gate numbers (the "if these hit, we ship" predicate)

| # | Variable | Perfect-world target | Current measurement | Source |
|---|---|---|---|---|
| 1 | `prediction_oracle_correlation` (raw Pearson, predictor verdict vs Docker oracle Pass/Fail) | **≥ 0.95** | `0.4843192995` | `swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` |
| 2 | Rolling-window stability | **4 consecutive windows** with target #1 holding | 0 / 4 | `mejepa_ship_gate_status` |
| 3 | Cross-panel `corr_B` (Panel B with non-overlapping encoders re-scoring same instances) | **≥ 0.95** | blocked behind #683 (closed 2026-05-23) + #715 (v2 cache) | `01 §1.5`, `#405` |
| 4 | `\|corr_A − corr_B\|` panel drift | **≤ 0.05** | partial parity on #392 active-subset: `0.0376` (selection bias #692) | `#405`, `#521` |

**Honest gap snapshot:** target #1 = `0.95 − 0.4843 = 0.4657` against raw Pearson. Targets #2/#3/#4 cannot be assessed until target #1 starts passing per-window.

**Two cheap probes determine if targets #1 + #3 are reachable at all:**
- **#734 (BLINDSPOT-010 panel sufficiency MI proof)** — measures `I(panel; oracle_outcome)`. If < 0.95 bits, no amount of #712 work moves target #1.
- **#723 (BLINDSPOT-001 oracle noise floor re-measurement)** — measures `oracle_self_consistency`. If < 0.95, target #1 is mathematically unreachable.

---

## 2. Tier 2 — Per-cell quality contract (#532, applies to every cell)

For each `(mutation_category × language)` cell:

| Cell type | Metric | Target | Current (aggregate) |
|---|---|---|---|
| **Mixed cells** (both Pass and Fail oracle rows) | Per-cell Pearson correlation | **≥ 0.95** + Fisher-z CI passes | aggregate `0.4843` |
| Mixed cells | Brier score | **≤ 0.05** | aggregate `0.0924` |
| Mixed cells | Expected Calibration Error (ECE) | **≤ 0.05** | aggregate `0.0240` ✓ |
| **All-pass cells** (e.g., `over_engineer::python`, `known_good::python`) | Pass-class recall | **≥ 0.95** | partial |
| All-pass cells | Pass-class actual-class confidence mean | **≥ 0.95** | partial |
| **All-fail cells** (e.g., `compile_error::python`, `wrong_file::python`) | Fail-class recall | **≥ 0.95** + Wilson lower bound | partial |
| All-fail cells | Fail-class actual-class confidence mean | **≥ 0.95** | partial |
| **All cells** | Conformal coverage (split-conformal predictor) | **88-92%** (`INFER_DEFAULT_ALPHA`) | enforced |
| All cells | Cell support (`n_supporting`) | **≥ 50** (else `Abstain` via cold-cell) | enforced via #532 |

**Current weak cells (per #611 / #615):** `delete_test_call`, `off_by_one`, `subtle_flip`. `delete_test_call` has 0 calibration-split failures — corpus-starved, addressed by #615 regeneration plan.

---

## 3. Tier 3 — Online mistake-loop convergence (#406 / #523 supporting target)

| Variable | Perfect-world target | Current |
|---|---|---|
| `mistake_repeat_rate` (rolling 100-mistake window) | **≤ 0.05** | LUT structurally inert (#689); fix prereq for measurement |
| `unrelated_cell_accuracy_delta` (control panel before/after mistake update) | **≤ 0.02** (no perturbation of unrelated rows) | enforced fail-closed when consumed |
| `claims_fisher_ewc_protection` | **`false`** (do not claim Fisher without durable Fisher row; #524 guard) | `false` (correctly) |

---

## 4. Tier 4 — Doctrine invariants (numbers that must equal exactly X — non-negotiable)

| Variable | Required value | Why |
|---|---|---|
| `ΔΞ.target_collapse` | **exactly `0`** | Frozen-target instruments must carry zero trainable params; any non-zero = `MEJEPA_INSTRUMENT_GRADIENT_LEAK` |
| `Q4_LOSS_COEFFICIENT_MAX` | **exactly `0.0`** | Q4 doctrine freeze (#408 closed) — ambiguous heads contribute zero loss |
| `q4_inputs_to_ship_gate()` | **exactly `0`** | Q4 must not feed ship-gate metric |
| `q4_row_count_delta` weekly | **exactly `0`** | No new Q4 producer writes after freeze |
| `flat_vector_concat_used` | **exactly `false`** | Doctrine §1.5ter — no flat-buffer projection over the 5,120-byte panel |
| `slot_identity_preserved` | **exactly `true`** | Same-embedder dim compare only; cross-embedder dim compare forbidden |
| `target_labels_used_as_live_inputs` | **exactly `false`** | Leakage policy: oracle-derived labels are target-side-only |
| `cross_panel_goodhart_detected` (when corr_A ≥ 0.95 AND corr_B < 0.85) | fail-closed → block promotion | Anti-Goodhart fence |
| `panel_b_resident_during_normal_inference` | **exactly `false`** | Panel B never resident at serving time; only loaded during eval worker |
| `feature = "llm"` compile guard | **`compile_error!` triggers** at `crates/context-graph-mcp/src/lib.rs:13` | Inner-LLM ban |
| Aiwonder durable root | **`/zfs/archive/contextgraph/...`** | Production must execute on aiwonder ZFS pool; no `/mnt/d` / `/mnt/c` |

These are not "targets to hit." They are **invariants the system must preserve at every moment**. Any violation is a doctrine breach.

---

## 5. Tier 5 — Substrate quality (BLINDSPOT audit targets, currently unmeasured)

These determine whether Tier 1 is reachable at all on the current substrate. **The two starred ones are the cheapest highest-leverage tests in the project.**

| Variable | Perfect-world target | Status | Why this matters |
|---|---|---|---|
| ⭐ `I(panel; oracle_outcome)` (mutual information; #734 BLINDSPOT-010) | **≥ 0.95 bits** | not yet measured | If < 0.95, panel is provably underpowered. No amount of #712 work moves Tier 1 target #1. **Cheap probe (~3-5 days).** |
| ⭐ `oracle_self_consistency` (10-replica per-instance; #723 BLINDSPOT-001) | **≥ 0.97** | not yet measured | If < 0.95, Tier 1 target #1 is mathematically unreachable. **Cheap probe (~2h aiwonder Docker).** |
| Per-embedder inter-embedder distinctness (#709 gate) | passes for all 9 static AST slots (`e1,e6,e7,e8,e9,e10,e12,e13,e14`) | partial — fixes for E7 (#720/#721), E9 (#716), E14 (#719) landed; pending v2 cache rebuild | Per-embedder vector spaces must be distinct; aliased spaces produce `e8 ≡ e1` failures |
| Per-embedder asymmetric AUC on directional pairs (#744 TASK-CR-006) | **≥ 0.7** on holdout | not yet measured | Validates whether auto-labeler (#413) extends to causal direction. Gate for in-slot causal embedder replacement (#745) |

---

## 6. Tier 6 — Embedder gating numbers (creativity engine + in-slot replacement, #516/#518/#745/#746)

| Variable | Perfect-world target | Use |
|---|---|---|
| `τ_novelty` (max-correlation of new embedder vs existing slots; #516) | **< 0.7** for promotion | Novelty oracle — rejects redundant embedders |
| `δ_utility` (per-cell oracle correlation lift; #518) | **≥ 0.005** for promotion | Utility oracle — rejects dominated embedders |
| `holdout_corr_delta_when_dropped` (slot ablation; #746) | **< 0.001** + variance ≈ 0 → replace; **≥ 0.005** → keep | Low-signal embedder audit decision rule |
| Asymmetric retrieval Spearman correlation between cause-neighborhood and effect-neighborhood (#745) | **≤ 0.5** | Confirms causal embedder is actually directional |
| Per-embedder pretraining-corpus overlap with SWE-bench Lite repos (#732) | **documented + counterfactual cutoff experiment if overlap > 30%** | Pretraining-corpus Goodhart audit |

---

## 7. The perfect-world FSV report

This is what the ship-gate FSV looks like when every number above hits simultaneously:

```
=== ME-JEPA Ship-Gate FSV — All Targets Met ===

Tier 1 (ship gate):
  prediction_oracle_correlation:      0.95+    ← target #1 ✓
  window_stability:                   4/4      ← target #2 ✓
  panel_b_correlation:                0.95+    ← target #3 ✓
  panel_drift:                        ≤ 0.05   ← target #4 ✓

Tier 2 (per-cell calibration):
  per_cell_pearson_min:               ≥ 0.95   ← every mixed cell
  per_cell_brier_max:                 ≤ 0.05   ← every cell
  per_cell_ece_max:                   ≤ 0.05   ← every cell
  all_fail_cell_recall_min:           ≥ 0.95   ← every all-fail cell
  all_pass_cell_recall_min:           ≥ 0.95   ← every all-pass cell
  conformal_coverage:                 0.88-0.92
  cold_cells_remaining:               0

Tier 3 (mistake loop):
  mistake_repeat_rate:                ≤ 0.05
  unrelated_control_delta:            ≤ 0.02

Tier 4 (doctrine — exact equality):
  delta_xi_target_collapse:           0          ← exactly
  q4_loss_coefficient:                0.0        ← exactly
  q4_inputs_to_ship_gate:             0          ← exactly
  q4_row_count_delta_weekly:          0          ← exactly
  flat_vector_concat_used:            false      ← exactly
  slot_identity_preserved:            true       ← exactly
  target_labels_used_as_live_inputs:  false      ← exactly
  cross_panel_goodhart_detected:      false      ← exactly
  panel_b_resident_during_inference:  false      ← exactly
  inner_llm_compile_guard_active:     true       ← exactly
  aiwonder_durable_root:              /zfs/archive/contextgraph/...

Tier 5 (substrate quality):
  I(panel; oracle):                   ≥ 0.95 bits
  oracle_self_consistency:            ≥ 0.97
  inter_embedder_distinctness_pass:   9/9 active static AST slots
  asymmetric_auc_min (if causal track) ≥ 0.7

Tier 6 (embedder gating; for any embedder add/replace):
  novelty_oracle_pass:                max_corr < 0.7
  utility_oracle_pass:                per_cell_lift ≥ 0.005

=== Ship-gate predicate evaluates: TRUE ===
=== Human review of AI-generated Python patches: STATISTICALLY UNJUSTIFIED ===
```

---

## 7.5 State-change levers per target tier (the state is not the constraint)

**The current state of the project is the starting point, not the goal and not the constraint.** If a target stalls, the question is "which state-change lever moves it" — not "is the target too high." See `NORTH-STAR.md §9.5` for the full state-mutability principle.

| Target | Primary state-change lever if stalled | Secondary lever | Aiwonder cost estimate |
|---|---|---|---|
| Tier-1 #1 `prediction_oracle_correlation ≥ 0.95` (statistical power) | **Corpus expansion** (300×8 → SWE-bench Verified ~500 → SWE-bench Full 2,294) | Panel expansion if MI insufficient | ~3-5 days re-bake per expansion tier |
| Tier-1 #2 (4-window stability) | **Window resampling refinement** (`#167` burn-in protocol) | Corpus expansion for per-window power | ~per-window measurement cost |
| Tier-1 #3 (Panel B `≥ 0.95`) | **Panel B encoder training** (`#405`/`#521`) — currently blocked | Larger Panel B corpus | ~4-6 weeks |
| Tier-1 #4 (drift `≤ 0.05`) | Improve predictor generalization across encoders | Cross-panel regularization | composes with #405 |
| Tier-2 per-cell Pearson | **Per-cell corpus expansion** for weak cells (`#615` delete_test_call regeneration) | Per-cell calibration refinement | ~3-5 days per cell |
| Tier-2 Brier/ECE | **Calibration refinement** (per-cell conformal, isotonic/Platt per `#695`) | Calibration corpus expansion | ~1 week |
| Tier-3 `mistake_repeat_rate` | **Mistake-loop architecture fix** (`#689` LUT inertness) | Online-head retraining | ~2-3 weeks |
| Tier-4 invariants | **NOT MOVABLE** — these are fences, not levers | — | — |
| Tier-5 `I(panel; oracle) ≥ 0.95 bits` | **Panel expansion** (in-slot causal embedder replacement `#745`; new instrument slots if `#734` proves insufficiency) | Substrate refinement | ~3 weeks per slot |
| Tier-5 `oracle_self_consistency ≥ 0.97` | **Oracle harness refinement** (replica voting, flakiness modeling BLINDSPOT-014) | Threshold revision if floor lower than 0.95 | ~1-2 weeks |
| Tier-5 inter-embedder distinctness | **Per-embedder fix or replacement** (`#716` E9, `#720`/`#721` E7, `#719` E14) | `#715` v2 9-slot cache rebuild | ~6-8h per slot |
| Tier-6 embedder gating | **Per-embedder audit + replacement** (`#516`/`#518`/`#746`) | Novelty/utility threshold tuning | per-embedder |

### Specific corpus-scale options (Tier-1/Tier-2 lever)

If statistical power on the 300×8 corpus is the bottleneck (Pearson CI width too loose, per-cell support insufficient, or distribution coverage too narrow), these are the known corpus-scale levers in order of cost-effectiveness:

| Corpus | Instances | Rows (× 8 mutations) | Holdout (~10%) | Pearson 95% CI at r=0.95 | Aiwonder cost (forward+oracle) |
|---|---|---|---|---|---|
| **Current: SWE-bench Lite** | 300 | 2,400 | 240 | ±0.062 | shipped baseline |
| **+ SWE-bench Verified** | +500 | +4,000 | +400 | ±0.036 (combined ~640) | ~3-5 days |
| **+ SWE-bench Full** | +2,294 | +18,352 | +1,835 | ±0.014 (combined ~2,075) | ~10-14 days |
| **+ BugsInPy** | +493 | +3,944 | +394 (different oracle contract) | covers real-Python bugs | ~3-5 days; oracle-adapter work needed |
| **+ TSSB-3M** | +3M single-statement bugs | +24M | impractical for full holdout | training corpus, not gate corpus | weeks; training only |
| **+ CommitPack** | 4TB of paired-state | ~742M pairs | training corpus | training corpus, not gate corpus | months; training only |

**Minimum corpus expansion for statistically-tight Pearson CI at the 0.95 target:**

- For `±0.01` confidence interval at r=0.95: ~1,100 holdout rows needed → 4-5× current corpus (SWE-bench Lite + Verified + Full).
- For `±0.025` confidence interval (current operational tolerance): ~440 holdout rows → 2× corpus (Lite + Verified).
- For `±0.05` confidence interval (loose but defensible): current 240 holdout sufficient.

**Decision rule for corpus expansion:** if 4-window stability ($Tier-1 #2$) fails because per-window variance exceeds `0.03` at r ≥ 0.95, expand to SWE-bench Verified minimum (target ~640 holdout, CI ±0.036). If per-cell support drops below 50 in mixed cells, expand to SWE-bench Full minimum.

### State-change is not threshold-change

Lowering the Tier-1 #1 target from `0.95` to `0.85` because the current corpus can't support `0.95` is **not** a state-change — it's a doctrine change, and a regression. The correct response to a corpus limitation is **corpus expansion**, not threshold relaxation. The only legitimate threshold revision is **upward** (e.g., `0.95` → `0.97` if BLINDSPOT-001 / #723 measures oracle floor higher than `0.97`).

---

## 8. What this means for every other artifact

Every issue in the GitHub ledger, every commit in the repo, every FSV harness, every architectural choice, every embedder train, every corpus regeneration, every cache rebuild — **exists to move at least one of these numbers toward its target**. If a proposed change does not move at least one of these numbers, it must justify its existence on doctrinal-invariant grounds (Tier 4) or be deferred / rejected.

This is the **evidence-weighted ordering** that #383 (locked rule) operationalizes. The #712 critical path is the operator-authorized sequence of moves toward these numbers. Every BLINDSPOT issue (#723–#739) is an audit of whether the path is on track. Every TASK-CR issue (#744–#746) is a follow-up lever conditional on the audit outcomes.

**There is no other goal. There is no other definition of "the system works."** Once these numbers hit, the predictor replaces human review, the project is shipped, and the post-0.95 creativity engine begins.

---

## 9. Doctrine cross-references

| Topic | Doc |
|---|---|
| Project doctrine + invariants | [`CLAUDE.md`](../../../CLAUDE.md) |
| Universal protocol | [`docs2/AICodingAgentSuperPrompt.md`](../../../docs2/AICodingAgentSuperPrompt.md) |
| Per-turn agent contract | [`AGENTS.md`](../../../AGENTS.md) |
| FSV verification protocol | [`FSV-PROTOCOL.md`](FSV-PROTOCOL.md) |
| System overview | [`../01_overview.md`](../01_overview.md) |
| Critical-path roadmap | [`../08_roadmap_and_open_work.md`](../08_roadmap_and_open_work.md) |
| Current measurement history | [`../07_results_and_benchmarks.md`](../07_results_and_benchmarks.md) |

---

**Last updated:** 2026-05-24. Update this doc whenever a Tier-1/2/3/4 target changes, or when a Tier-5 substrate measurement lands.
