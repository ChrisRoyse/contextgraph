# THE PERFECT STATE EXPLAINED — Every variable, why it matters, what unlocks

> **This doc explains, variable by variable, what each number in `docs/futurebuild/specs/TARGET-METRICS.md` actually measures, why that specific value is the threshold, and what changes when it hits. Then it describes — in operational and second-order terms — what the world looks like when every one of these variables holds simultaneously. That world is the goal.**
>
> Companion to:
> - **`docs/futurebuild/specs/NORTH-STAR.md`** — why this is the goal (the framing)
> - **`docs/futurebuild/specs/TARGET-METRICS.md`** — what the goal is numerically (the sheet)
> - **This file** — what each variable means and what their simultaneous satisfaction unlocks (the depth)
>
> Read order: `NORTH-STAR.md` first, then `TARGET-METRICS.md` for the numbers, then this file for the meaning.

---

## §1 — TIER 1: The 4 ship-gate numbers explained

### 1.1 `prediction_oracle_correlation ≥ 0.95`

**What it literally measures.** The Pearson product-moment correlation coefficient between two paired vectors:
- The predictor's verdict score per instance (a float in `[0.0, 1.0]` representing predicted probability of Pass)
- The Docker oracle's actual outcome per instance (Pass = 1.0, Fail = 0.0)

across the 240-row holdout split of the 300×8 SWE-bench Lite Python corpus. Range is `[-1.0, +1.0]`; perfect agreement is `+1.0`; random is `0.0`; perfect anti-correlation is `-1.0`.

**Why specifically 0.95 (not 0.90, not 0.99).**

- **`< 0.85`** means the predictor's signal is weak enough that human review costs are not statistically defensible to skip. False-positive Pass rate climbs faster than human-review-of-uncertain-cases bandwidth can absorb.
- **`0.90`** is the threshold below which the predictor adds value but cannot replace review. Auto-trust is still unjustified.
- **`0.95`** is the threshold at which expected cost of false-positive Pass under realistic operator throughput assumptions matches or beats the cost of human review at the same throughput. This is the empirical sufficiency threshold.
- **`> 0.97`** is bumping against the Docker oracle's own self-consistency floor (see Tier-5 BLINDSPOT-001 / #723). Targeting higher than this is asking the predictor to beat the measurement device.
- **`0.99`** is unreachable on this corpus — even perfect human reviewers would not achieve it because of irreducible flakiness in the test corpus (see Tier-3 / #TASK-G-019 / BLINDSPOT-014 flakiness modeling).

**Current measurement.** Raw Pearson `0.4843192995` on the 240-row holdout, latest ship-gate FSV `swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. **Honest gap to ship: `0.4657`.** The previously cited `0.7942` rank-lift figure is a category-local RBF exemplar-similarity composite per #695, not the predictor's score.

**What changes when it hits.** Every candidate AI-generated patch can be evaluated in milliseconds on aiwonder's RTX 5090 with calibrated confidence. The predictor's `Pass` and `Fail` outputs carry information content statistically equivalent to a human reviewer's Pass / Fail at this corpus's distribution. The economic asymmetry between AI generation speed and human review speed inverts — verification is no longer the bottleneck.

**Failure modes if it stalls.**
- Substrate insufficient: panel doesn't carry enough information (BLINDSPOT-010 / #734 — `I(panel; oracle) < 0.95 bits` makes target unreachable).
- Oracle noise floor too high: target mathematically unreachable (BLINDSPOT-001 / #723 — `oracle_self_consistency < 0.95`).
- Predictor underpowered: more training data + better architecture needed.
- Goodhart cheat: rank-lift transforms inflate the reported number without moving the actual signal (#695).

### 1.2 Stable × 4 consecutive rolling windows

**What it literally measures.** Target 1.1 must hold for 4 consecutive non-overlapping evaluation windows of the holdout corpus. Each window samples a different ~60-instance partition; window stability means the per-window correlation never drops below `0.95` across 4 successive samples.

**Why specifically 4 (not 1, not 10).**

- **`1`** window passing is a flake. A predictor that hits `0.95` once and `0.85` next time has not converged.
- **`2`** windows is suggestive but doesn't bound the variance.
- **`4`** is the smallest interval at which the time-series statistic `corr ≥ 0.95` cannot be a flake under realistic distribution shift between calibration and serving. Statistical power requirement: detect drift > `0.03` with `α = 0.05` and `β = 0.20` — the 4-window threshold derives from this.
- **`10`+** windows is diminishing returns; the additional information about temporal drift is bounded by what 4 windows already provide.

**What changes when it hits.** The predictor's behavior is stable enough that any per-window drop signals true drift, not sampling noise. The heal scheduler (#173 post-green drift guardrails) can trigger automatic rollback if `corr` drops below `0.93` mid-deployment — false-alarm rate is bounded because the 4-window baseline is well-characterized.

**Failure modes if it stalls.**
- High per-window variance: predictor over-fits to specific instance subsets.
- Concept drift: corpus distribution moves faster than training updates can compensate.
- Calibration drift: per-cell calibration ages out of currency (`INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS`).

### 1.3 `cross_panel_oracle_correlation ≥ 0.95` on Panel B

**What it literally measures.** Panel B is the same 15-slot panel architecture as Panel A, but with **non-overlapping learned encoders**:

| Slot | Panel A encoder | Panel B encoder |
|---|---|---|
| AST | tree-sitter → 384-d | CodeBERT → 384-d |
| CFG | attention encoder → 256-d | Pythia program-graph → 256-d |
| Data-flow | def-use attention → 256-d | joern → 256-d |
| Type-graph | tree-sitter type → 256-d | pytype JSON → 256-d |
| Code-semantic | Qodo-Embed-1 → 1536-d | Voyage-code-2 → 1536-d |
| Text | e5-large-v2 | bge-large-en-v1.5 |
| (deterministic slots) | unchanged | unchanged |

The predictor head is the same. The encoders differ. **Both panels are scored against the same Docker oracle on the same instances.** This target is whether `corr(predictor_verdict_on_panel_B, oracle_outcome) ≥ 0.95`.

**Why this exists.** Panel A is the substrate the predictor trained against. If the predictor learns to recognize the panel rather than to recognize code reality, Panel A correlation can hit `0.95` even when the predictor has memorized panel-specific artifacts. Panel B is the Goodhart guard: independent encoders observing the same code should reach the same verdict if the predictor learned about code, not about the encoders.

**Why specifically 0.95 (same as Panel A).**

- Asymmetric thresholds (e.g., `0.95` on A but `0.85` on B) are not defensible. If the predictor can hit `0.95` on A but only `0.85` on B, the `0.10` gap is direct evidence of encoder-Goodhart and the system has not actually learned code.
- The threshold must equal Panel A's to make the comparison meaningful.

**What changes when it hits.** Encoder-level Goodhart is eliminated. The predictor demonstrably generalizes across alternative encodings of the same input — meaning it has learned something about the code, not something about the specific embedder weights it was trained against.

**Failure modes if it stalls.**
- Panel A `≥ 0.95` AND Panel B `< 0.85` → `CROSS_PANEL_GOODHART_DETECTED` typed reason; predictor blocked from promotion.
- Panel B encoders themselves are too weak: false-Goodhart-flag because Panel B can't actually score `0.95` even on a fair predictor. #581 already falsified the "bigger Panel B fixes it" reflex.

### 1.4 `|corr_panel_a − corr_panel_b| ≤ 0.05`

**What it literally measures.** The absolute difference between Tier-1 target #1 (Panel A) and Tier-1 target #3 (Panel B) on the same holdout. Both must clear `0.95`, AND their gap must be ≤ `0.05`.

**Why specifically 0.05 (not 0.02, not 0.10).**

- **`0.02`** is below the noise floor of the per-window measurement; would generate false drift alarms.
- **`0.05`** is the empirical bound on between-encoder variance for a predictor that has learned the underlying code reality rather than encoder artifacts. Encoders that disagree by more than 0.05 on real-code prediction are detectably encoder-dependent.
- **`0.10`** is loose enough that encoder-Goodhart can hide. Cross-panel test would lose detection power.

**What changes when it hits.** Targets #3 and #4 together close the Goodhart fence at the encoder level. Combined with Tier-1/2 calibration and Tier-4 doctrine invariants, the system's `Pass` and `Fail` outputs become **transferable**: applicable to instances generated by tools other than the ones the predictor was trained against, applicable to repos other than the SWE-bench Lite corpus, and applicable across time as the foundation models behind the encoders update.

**Failure modes if it stalls.**
- Single-panel learning: predictor learned Panel A but generalizes weakly to Panel B.
- Panel B underpowered: failure to reach 0.95 even on a fair predictor (Voyage-code-2 or pytype don't represent code as cleanly as the Panel A encoders).
- Distribution mismatch between encoders' pretraining corpora.

---

## §2 — TIER 2: Per-cell quality contract explained

For every `(mutation_category × language)` cell in the 8-mutation × 1-language (Python) corpus = 8 cells today, plus future language expansion.

### 2.1 Per-cell Pearson correlation (mixed cells)

**What it measures.** Pearson correlation between predictor verdict and oracle outcome, computed **per cell**, on cells where the oracle has both Pass and Fail rows. Aggregate Tier-1 Pearson can hit 0.95 while one cell sits at 0.30; per-cell metric forces uniform quality.

**Why `≥ 0.95` per cell (not just on aggregate).** Aggregate is repo-distribution-weighted. If `django=38%` of corpus pulls aggregate up, individual cells (`delete_test_call`, `off_by_one`) can hide weaknesses. Per-cell threshold prevents this.

**Current weak cells.** `delete_test_call` (corpus-starved per #615), `off_by_one`, `subtle_flip`.

### 2.2 Brier score `≤ 0.05`

**What it measures.** Brier score = mean squared error between predicted probability and observed binary outcome. Range `[0, 1]`; lower is better; `0.25` is "predict 0.5 always"; `0.0` is perfect calibration.

**Why specifically 0.05.** Brier is a strictly proper scoring rule — it penalizes both miscalibration and incorrect predictions. A Brier of `0.05` corresponds roughly to "predicted probability matches observed frequency within 0.22 average absolute error." Tighter than this risks overfitting to the calibration split; looser allows poorly-calibrated outputs that look confident but lie.

**Current.** Aggregate `0.0924` per latest ship-gate FSV. About 2× over budget.

### 2.3 Expected Calibration Error (ECE) `≤ 0.05`

**What it measures.** ECE bins predictions by confidence (e.g., 10 bins from 0.0-0.1 to 0.9-1.0) and measures per-bin error between predicted-confidence and actual-frequency. `0.0` is perfect calibration; `0.10`+ is significant miscalibration.

**Why 0.05.** Calibration matters because `Pass with 0.93 confidence` must actually be Pass 93% of the time in that confidence bin. If ECE is `0.10`, then 0.93-confidence Pass rows are actually Pass closer to 80-85% of the time — a critical lie for operator decision-making.

**Current.** Aggregate `0.0240` — already within budget at aggregate level. Per-cell breakdown is harder; some cells exceed it.

### 2.4 All-pass cell metrics

For cells that are structurally all-Pass (e.g., `over_engineer::python` where the mutation produces extra-but-not-broken code, `known_good::python` which is unmodified):
- **Pass-class recall ≥ 0.95** — predictor correctly identifies 95%+ of Pass rows as Pass.
- **Pass-class actual-class confidence mean ≥ 0.95** — when predicting Pass, the average confidence is at least 0.95.

**Why both.** Recall alone allows over-confident-wrong predictions; confidence-mean alone allows under-confident-correct predictions. Both together force "high confidence + high accuracy" on cells the predictor should find easy.

### 2.5 All-fail cell metrics

For cells that are structurally all-Fail (e.g., `compile_error::python` where mutation introduces syntax errors, `wrong_file::python` where patch applies to wrong file):
- **Fail-class recall ≥ 0.95 + Wilson lower bound** — predictor catches 95%+ of fails, with Wilson 95% lower confidence interval also ≥ 0.95 to bound sample-size uncertainty.
- **Fail-class actual-class confidence mean ≥ 0.95**.

**Why Wilson specifically.** For small per-cell sample sizes, the normal approximation under-estimates uncertainty. Wilson lower bound is the correct power-of-evidence statistic for binomial proportions at small n.

### 2.6 Conformal coverage `88-92%`

**What it measures.** Split-conformal predictor wraps the model with calibrated confidence intervals. The coverage statistic is: across all predictions where the model returns a confidence interval `[lo, hi]`, what fraction of actual oracle outcomes fall inside `[lo, hi]`?

**Why 88-92% specifically.** Target is `1 - α` where `α = INFER_DEFAULT_ALPHA = 0.10`. So nominal coverage is 90%. The `88-92` band is the empirical tolerance — strict 90% would be over-tight given finite-sample noise.

**What this gives.** A statistically-rigorous "predictor's confidence interval contains the truth 90% of the time" guarantee. Operators can read the confidence interval as a real probability statement.

### 2.7 Cell support `≥ 50` (else Abstain)

**What it measures.** Number of training examples in the cell. If `n_supporting < 50`, the cell is "cold" — insufficient data to compute calibrated confidence. Cold-cell metric writes `CF_MEJEPA_COLD_CELL_METRICS` and the predictor returns `Abstain` rather than a verdict.

**Why 50 specifically.** Statistical power minimum for distinguishing 0.95-Pearson from 0.85-Pearson on a small holdout. Below 50, confidence-interval width swallows the discriminating signal.

**What this prevents.** Predicting on cells the system has never adequately trained on — preventing both false-Pass and false-Fail catastrophes in long-tail bug categories.

---

## §3 — TIER 3: Online mistake-loop convergence explained

### 3.1 `mistake_repeat_rate ≤ 0.05` (rolling 100-mistake window)

**What it measures.** Take the most recent 100 mistakes (predictor disagreed with oracle). For each, check: did the predictor make the same mistake on the same cell + label + skill + ability + membership context **after** the mistake was logged? Repeat rate = `(second_mistake_on_same_context) / (first_mistake_on_same_context)`.

**Why 0.05.** A perfectly-converging online learner has 0% repeat rate (every mistake teaches it forever). Reality has noise — 0.05 is the empirical bound for "the system is learning from its mistakes" vs "the system makes the same mistake repeatedly."

**Why this matters for the goal.** Tier 1 measures *current* accuracy. Tier 3 measures *learning rate*. A predictor that hits 0.95 today but doesn't learn from new failures degrades over time. The mistake loop is what keeps the predictor adapted as the agent corpus and code distribution shift in production.

**Current.** Online-head LUT structurally inert per #689 (`protection_mode=ExactPanelKeyIsolation` — panel signatures almost never collide in production, so the LUT is never hit). Requires #689 fix before this metric becomes measurable.

### 3.2 `unrelated_cell_accuracy_delta ≤ 0.02`

**What it measures.** When the mistake loop updates the predictor on cell X, measure accuracy on unrelated cell Y before and after. If `|accuracy_Y_after - accuracy_Y_before| > 0.02`, the update perturbed unrelated state — a catastrophic-forgetting violation.

**Why 0.02.** Slightly above noise floor for per-cell accuracy variance. Below this, the update is provably isolated.

### 3.3 `claims_fisher_ewc_protection = false`

**What it measures.** Not a target to achieve — a fence to maintain. The system MUST NOT claim it provides Fisher Information Matrix / Elastic Weight Consolidation catastrophic-forgetting protection unless a durable Fisher row exists in `CF_MEJEPA_FISHER_SNAPSHOTS`. #524 guard.

**Why this matters.** Fisher/EWC is a real technique with real protection guarantees — claiming it without implementing it is exactly the silent-success class of bug that #629 5-Sherlock audit caught at scale. The boolean is `false` because the current shipped primitive is exact-panel key isolation, not Fisher. Honesty about what's implemented vs claimed.

---

## §4 — TIER 4: Doctrine invariants (exact-equality fences)

These are not targets to "improve." They are conditions that must hold **at every moment**. Any violation = doctrine breach.

### 4.1 `ΔΞ.target_collapse = exactly 0`

**What it measures.** ΔΞ is the structural-collapse safety margin term in the UTML training loss `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. It measures whether gradient has leaked from the trainable predictor back into the frozen-target instrument weights. Required value: exactly zero. Any non-zero = `MEJEPA_INSTRUMENT_GRADIENT_LEAK` typed error, training rejected.

**Why exact zero.** The frozen-target architecture's entire point is that the target panel is invariant during training — the predictor learns to predict the frozen target, not to co-adapt with it. Any gradient leak means the target is moving, which means the system is collapsing toward a trivial solution where predictor and target both converge to constants.

**What this prevents.** The classic JEPA-family failure mode where the target encoder drifts toward zero output, the predictor learns to also output zero, and the loss converges to zero on a trivially-degenerate solution.

### 4.2 `Q4_LOSS_COEFFICIENT_MAX = exactly 0.0`

**What it measures.** The maximum coefficient applied to Q4 (ambiguous-head) loss terms in training. Required: exactly 0.0 — Q4 contributes nothing to training.

**Why.** Q4 questions (perf, accuracy, cost, reasoning quality) are ambiguous — they require arbitrary thresholds on continuous quantities or ask taste judgments with no ground truth. Training the predictor against Q4 supervision trains it to match the heuristic that produced the Q4 label, not reality. #408 closed Q4 doctrinally; this coefficient enforces it numerically.

**What this prevents.** The predictor learning to predict mutmut/semgrep/vulture/hyperfine output rather than Docker oracle output.

### 4.3 `q4_inputs_to_ship_gate() = exactly 0`

**What it measures.** Number of Q4-derived signals that feed the ship-gate metric. Required: zero.

**Why.** Even if Q4 doesn't contribute to training loss, leaking Q4 features into ship-gate measurement would compound the same heuristic-not-reality contamination.

### 4.4 `q4_row_count_delta` weekly `= exactly 0`

**What it measures.** Number of new Q4 rows written to `CF_MEJEPA_Q4_*` CFs in the past week. Required: zero — no new Q4 producer writes after the freeze.

**Why.** Old Q4 rows remain readable for forensic continuity. New writes would indicate a producer was reanimated post-freeze, violating #408.

### 4.5 `flat_vector_concat_used = exactly false`

**What it measures.** Whether any production code path projects the 5,120-byte packed panel buffer through a single `linear(5120, hidden)` layer or otherwise treats the buffer as a flat 5,120-d vector rather than an array of 15 per-embedder vectors.

**Why exactly false.** This is doctrine §1.5ter (now §1.6 in 01_overview): cross-embedder dim-wise comparison is forbidden because each slot's vector lives in its own embedder's coordinate system. Flat-buffer projection silently allows a strong slot to compensate for a weak slot — the predictor learns aggregate signal rather than per-slot signal, which destroys the anti-Goodhart defense.

**What this prevents.** The predictor reaching apparent 0.95 by aggregating heterogeneous embedder outputs into a single fused vector that the underlying math says is meaningless.

### 4.6 `slot_identity_preserved = exactly true`

**What it measures.** Companion invariant to 4.5. Every per-slot encoder operates only on its own slot's vector. Every per-slot decoder emits only its own slot's residual. Cross-embedder fusion happens only via late-fusion over post-encoded latents (which are typed positional features), not over raw embedder outputs.

**Why exactly true.** Same reason as 4.5 — the architectural commitment that produces meaningful per-slot OOD reasons (#389), valid same-embedder cosine similarities, and anti-compensation guarantees (#394).

### 4.7 `target_labels_used_as_live_inputs = exactly false`

**What it measures.** Whether any label in the Level-0/1/2/3 hierarchy derived from oracle outcomes (`solution_leak`, `incorrect_fix_weak_test`, anything tagged `leakage_policy: target_side_only`) appears as a feature in the live predictor's forward pass.

**Why exactly false.** Target-side labels are supervision signals. Using them as live inputs leaks the answer into the question — the predictor would learn to read the label off its own input rather than predict reality from the panel.

### 4.8 `cross_panel_goodhart_detected` fail-closed

**What it measures.** When Panel A correlation `≥ 0.95` AND Panel B correlation `< 0.85`, the typed reason `CROSS_PANEL_GOODHART_DETECTED` fires and ship-gate promotion is blocked. This is not a metric to optimize — it's a fail-closed fence.

**Why fail-closed.** If Panel A and Panel B disagree by more than `0.10`, the Tier-1 target #1 reading is unreliable. The system must refuse to promote rather than ship a predictor that has memorized Panel A.

### 4.9 `panel_b_resident_during_normal_inference = exactly false`

**What it measures.** Panel B encoders must never be loaded in VRAM during normal serving inference. Panel B is loaded only during evaluation worker passes.

**Why exactly false.** VRAM budget — Panel B's CodeBERT + Pythia + joern + pytype + Voyage + bge stack consumes ~12 GB. Forcing this into the serving path inflates inference latency and breaks the "serve in milliseconds" requirement.

### 4.10 Inner-LLM compile guard active

**What it measures.** `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. The compile fence prevents anyone from reanimating the retired Qwen / Hermes / Ollama / llama.cpp inner-actor stack.

**Why exactly active.** The system's entire architectural claim is that no LLM inference happens inside ME-JEPA. Reanimating an inner LLM would invalidate every other doctrine claim — the system would become "LLM with vector overlay" instead of "binary reality predictor."

### 4.11 Aiwonder durable root `= /zfs/archive/contextgraph/...`

**What it measures.** Every production-durable write goes to the aiwonder ZFS archive pool. No `/mnt/d`, no `/mnt/c`, no local-host RocksDB for durable state.

**Why exactly this.** ZFS snapshots provide DR; aiwonder hardware provides GPU compute; CI lint `scripts/ci/check_no_c_drive_paths.sh` enforces. Off-aiwonder writes break the durability + reproducibility chain.

---

## §5 — TIER 5: Substrate quality (BLINDSPOT audits)

These are not targets to hit during training — they are *measurements of whether the substrate is capable of the Tier-1 targets at all*.

### 5.1 `I(panel; oracle_outcome) ≥ 0.95 bits` (#734 BLINDSPOT-010)

**What it measures.** Mutual information between the panel (as random variable across the corpus) and the oracle outcome (Pass/Fail binary). Estimated via MINE (Mutual Information Neural Estimation) or Kraskov k-NN estimator.

**Why ≥ 0.95 bits specifically.** Binary oracle outcomes have entropy `H(oracle) ≤ 1.0 bit` (peaks at exactly 1.0 if 50/50 Pass/Fail). For the predictor to achieve `prediction_oracle_correlation ≥ 0.95`, the panel must carry at least ~0.95 bits of mutual information with the oracle (information-theoretic lower bound; loose Fano-style).

**Why this is the most existential target.** If `I(panel; oracle) < 0.95 bits`, no training procedure can reach Tier-1 target #1, period. The panel is **provably insufficient**. The fix is upstream: add slots, change instruments. Not downstream: more training, bigger model.

**Current.** Not yet measured. #734 is filed; it's a 3-5 day aiwonder GPU run.

### 5.2 `oracle_self_consistency ≥ 0.97` (#723 BLINDSPOT-001)

**What it measures.** Run the same SWE-bench instance through `swebench.harness.run_evaluation` 10 times. For each instance, compute per-replica pairwise agreement matrix. Aggregate: mean per-instance agreement across 30-instance sample.

**Why ≥ 0.97 specifically.** Tier-1 target #1 asks for `0.95` correlation against the oracle. If the oracle is only `0.93` self-consistent, then even a perfect predictor can only correlate up to `0.93` with the oracle on average — the predictor cannot beat the measurement device. `0.97` ensures the oracle floor is comfortably above the target.

**Why this matters.** If `oracle_self_consistency < 0.95`, Tier-1 target #1 is mathematically unreachable. The fix is upstream: oracle-flakiness modeling, replica-mode voting, exclusion of flaky instances from gate scoring.

**Current.** Not yet measured. #723 is filed; ~2h aiwonder Docker uptime.

### 5.3 Per-embedder inter-embedder distinctness (#709 / #715)

**What it measures.** For each pair of active embedders (e.g., E1 vs E7, E1 vs E8), measure that their output vectors carry distinct information — not byte-identical, not collapsed to single value, not aliased.

**Why this matters.** The legacy v1 forward cache had `e2 ≡ e3 ≡ e4` (collapsed to single value because `chunk.time` was wall-clock-injected identically per chunk) and `e8 ≡ e1` (graph-embedder bug pre-`ff4d4c03`). When embedders are aliased, the panel's effective dimensionality collapses — a 15-slot panel becomes a 9-slot panel in disguise, and `I(panel; oracle)` drops accordingly.

**Current state.** Today's distinctness gate (#709) caught E7 (#720/#721), E9 (#716), E14 (#719) all failing — fixes landed. v2 9-slot static cache rebuild pending under #715.

### 5.4 Per-embedder asymmetric AUC `≥ 0.7` (#744 TASK-CR-006)

**What it measures.** For directional pairs `(commit_before, commit_after, bug_fix_label)` from CommitPackFT, after applying the auto-labeler (#413) to both sides, measure the per-embedder asymmetric AUC of `P(after | before) vs P(before | after)`.

**Why this matters.** Validates whether the auto-labeling pipeline extends to causal direction. If `≥ 0.7`, the system can extract causal labels from public corpora at scale → in-slot causal embedder replacement (#745) becomes viable. If `< 0.6`, the causal-direction hypothesis is falsified.

---

## §6 — TIER 6: Embedder gating numbers

These govern when new or replacement embedders are admitted to the panel.

### 6.1 `τ_novelty < 0.7` (#516 Novelty oracle)

**What it measures.** For a candidate new embedder, the maximum cross-correlation between its output and any existing panel slot. Must be `< 0.7` to admit.

**Why 0.7.** Above this threshold, the new embedder is structurally redundant with an existing slot — adding it doesn't add information, just adds storage and compute cost. The novelty oracle prevents the panel from accumulating embedders that all encode the same thing under different names.

### 6.2 `δ_utility ≥ 0.005` (#518 Utility oracle)

**What it measures.** Per-cell oracle correlation lift from adding the candidate embedder. Must be `≥ 0.005` averaged across mixed cells.

**Why 0.005.** Below this, the embedder provides essentially no signal — its addition is dead weight. Above this, it earns its slot.

### 6.3 Drop-ablation thresholds (#746 audit)

**What it measures.** When a slot S is dropped from the panel during training, measure the holdout correlation delta vs full-panel baseline:
- `< 0.001` AND production variance `≈ 0` → slot is dead weight → replace it.
- `[0.001, 0.005]` → marginal signal; defer judgment.
- `≥ 0.005` → keep it.

**Why this decision rule.** Conservative default-keep. Only flags slots with provably zero contribution.

### 6.4 Asymmetric retrieval Spearman `≤ 0.5` (#745)

**What it measures.** For a causal embedder, query its cause-mode (returns likely effects) and effect-mode (returns likely causes) on the same input. Compute Spearman rank correlation between the returned neighborhoods. Must be `≤ 0.5` — the two query modes should produce meaningfully different neighborhoods.

**Why ≤ 0.5.** Above this, the embedder isn't actually directional — it's a regular embedder pretending to be causal. The directional retrieval surface only earns its existence if cause-queries and effect-queries diverge.

---

## §7 — What happens when ALL of these hit simultaneously

This is the operational definition of the system having worked.

### 7.1 Statistical replacement of human review

The verifier's `Pass | Fail` outputs carry information content statistically equivalent to a human reviewer's Pass / Fail on this corpus's distribution. Quantitatively:

- False positive rate (predictor says Pass, oracle says Fail) ≤ 5%.
- False negative rate (predictor says Fail, oracle says Pass) bounded by Brier ≤ 0.05.
- Confidence calibration error ≤ 0.05 — when the predictor says `Pass with 0.93 confidence`, the actual frequency is between 0.88 and 0.98.
- Per-cell uniform quality — no failure-mode class hides catastrophically below the aggregate.

For AI agents producing Python code at scale, the verifier becomes the review surface. Operator-level review continues for catastrophic-class events, novel failure modes, and explicit operator-override workflows — but the routine bottleneck of "human reads diff" is dissolved.

### 7.2 Compounding rate of AI code production

Currently, AI agents (Claude Code, Codex, Cursor) produce hundreds of edits per hour but ship rates are bounded by human-review throughput (~edits per minute on a good day). The asymmetry compresses the value of fast AI generation against slow human verification.

When the verifier hits the predicate, the AI generation rate uncaps. An agent producing 10× more code per hour translates to 10× more ship throughput because every patch gets calibrated `Pass | Fail` in milliseconds rather than minutes-to-hours of human attention.

This is the moment "AI coding" stops being a research demo and becomes a real software-production multiplier.

### 7.3 Activation of the post-0.95 creativity engine

The post-0.95 creativity engine (#516 / #517 / #518 / #519 / #520) is gated on Tier-1 closure. Once the predicate evaluates `TRUE`:

- **#517 mistake-driven embedder demand** activates: when the predictor mistakes accumulate with no per-slot attribution explaining them, the system emits a demand signal for a new embedder.
- **#516 novelty + utility oracle** evaluates incoming embedder proposals against the two-test gate (orthogonal to existing slots ≥ 0.3 AND per-cell oracle lift ≥ 0.005).
- **#518 retirement audit** continuously prunes dominated embedders (Pareto-frontier policy) so the panel doesn't accumulate dead weight.
- **#519 domain-routed sparse panel** ships: universal ~30 slots + per-domain ~15 extensions for non-Python languages.
- **#520 episodic-to-semantic consolidation** ships: settled chunks compress from per-chunk panels to `(cluster_id, residual)` with 99% storage reduction in high-clustering regions.

Together these form the **self-resolving observer** architecture: bounded autonomous growth that discovers its own dimensional sufficiency. The panel grows until the corpus is fully resolved (intrinsic dimensionality, ~50-500 axes for code), prunes dominated embedders, specializes per domain, compresses settled clusters. The system converges to its own optimal observer architecture without external intervention.

### 7.4 Domain-specific super-intelligence as a replicable template

ME-JEPA-Code is super-intelligence in the *operational* sense for one domain: Python code verification. Reaching the predicate establishes a **replicable template** for any domain where:

1. Inputs and outputs can be encoded into a multi-embedder panel.
2. Ground truth exists in an externally-verifiable form (oracle, test suite, physical measurement).
3. The panel-to-oracle mutual information is `≥ 0.95 bits` (Tier-5 sufficiency).
4. A calibrated predictor over the panel exists with the architectural primitives ME-JEPA-Code shipped.

The template generalizes to: chemistry (predict reaction outcome from molecule structure), finance (predict trade outcome from market state), medical diagnosis (predict treatment outcome from patient state), engineering (predict design failure from CAD), and any other domain where reality acts as the oracle.

ME-JEPA-Code is the proof-of-concept for the architecture. Once the predicate is hit here, the same architecture is shippable in other domains with retargeted instrument slots.

### 7.5 Closed-loop self-improvement

The mistake loop (#406 / #523) means the predictor doesn't stop improving when shipped. Every disagreement between predictor and reality (oracle, shift-log, operator override) becomes a training signal. The convergence target `mistake_repeat_rate ≤ 0.05` means same-context mistakes happen at most once.

In production, this means:
- New failure modes the predictor hasn't seen get added to its calibration as they're encountered.
- Drift in code distribution (new framework releases, new bug classes from emerging libraries) gets absorbed automatically.
- Operator overrides (when the human disagrees with the verifier on edge cases) feed back as supervisory signal.

The predictor doesn't atrophy — it gets sharper with use.

### 7.6 Pathway exploration becomes operator-facing

The pathway exploration surface (#416) — currently planned, will ship post-0.95 — surfaces the top-K (3-5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves. When the binary primitive is calibrated to `0.95`, the pathway tree's leaf probabilities are calibrated too. Operators can pick a desired future and the reverse-prediction tool (#335) tells the agent what patch-shape lands that future.

This is the moment "AI coding" stops being one-shot patch generation and becomes **operator-directed multi-step reality engineering**. The operator says "I want the change that lands these 4 binary outcomes"; the agent generates a patch that the predictor expects to produce exactly those outcomes; the verifier confirms.

### 7.7 Multi-language extension

The first ship gate is Python-only. Once it hits, the same architecture extends to the other 10 languages in `Language::all() = [Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php]`:

1. Generic operators (`generic_language_ops`) replace Python-parser-backed mutations for the new language.
2. Per-language Docker oracle harness is built (most exist in the SWE-bench ecosystem; some need wrappers).
3. Per-language calibration sweep mirrors the Python path.
4. Cross-language label gating (`cross_language_label_gating.rs`, shipped) ensures Python-trained patterns don't leak into Rust verdicts and vice versa.

The architectural work is done — only per-language corpus + calibration is needed.

### 7.8 The verifier becomes the agent's planning oracle

Beyond verification, the predictor becomes the agent's **planning oracle**. The agent can:

1. Generate a candidate patch.
2. Run it through `mejepa_verify` in milliseconds.
3. If `Fail`, read the named failure mode + closest exemplar.
4. Generate an alternative patch informed by the failure analysis.
5. Iterate until `Pass`.

This is local search guided by a calibrated objective. The agent stops emitting blind candidates and starts emitting candidates that have already been internally verified before consuming any human attention. This is the agent-loop the system is designed to enable.

---

## §8 — Second-order consequences of the unlocked state

### 8.1 Verification capacity becomes scarce in a new way

When the verifier replaces human review at the binary-Pass/Fail level, the bottleneck shifts. Verification is no longer expensive in human time, but it is expensive in **GPU time** — every patch verification consumes aiwonder cycles. The bottleneck becomes verifier throughput vs agent throughput.

This is a tractable engineering problem (incremental forward cache #733, distillation to smaller serving models, edge deployment). Unlike the human-review bottleneck, the verifier-throughput bottleneck has a clear scaling path.

### 8.2 Code-review culture inverts

When AI agents ship verified code at machine scale, the human role shifts from "review every change" to:

1. **Operator over the verifier.** Approving novel failure-mode promotions, catastrophic-class verdicts, edge cases where the verifier abstains.
2. **Architect of the goals.** Picking which futures the agents should land via pathway exploration.
3. **Auditor of the audit.** Periodic deep-review of the verifier itself — does its calibration still match reality, are there blind spots emerging, has it Goodhart'd a new way.

The day-to-day "review every PR" workflow becomes vestigial. The strategic work expands.

### 8.3 Information-theoretic implications

Achieving `prediction_oracle_correlation ≥ 0.95` with calibrated per-cell quality means the system has compressed code-state-to-outcome from `H(oracle_outcome) ≈ 1 bit` per instance down to a representation that the predictor can decode with ≤ 0.05 calibration error. The information-bottleneck (#511) framing this as `L_entropy = λ · H(z_latent)` formalizes the compression: the system has found the minimum-bits representation that preserves oracle correlation.

This is information-theoretically optimal binary verification for this domain.

### 8.4 The Q4 boundary becomes permanent

When the system is shipped at Tier-1, the explicit Q4 freeze (ambiguous-head exclusion) is vindicated. The system shipped without ever needing to predict perf, accuracy, cost, reasoning quality — those questions did not need to be in the prediction contract to achieve the operational goal.

This validates a doctrine choice (binary contract, ambiguity boundary) that has been intuitively right but never empirically proven. After 0.95, the choice is empirically vindicated.

### 8.5 The path to AGI looks more like this than it does like LLM scaling

The architecture pattern that wins here is:
- Multi-view per-modality embedders (panel of 15 vectors per chunk).
- Calibrated binary predictor over a frozen-target panel (slot-preserving late fusion).
- Online mistake-driven adaptation with catastrophic-forgetting protection.
- Goodhart guards at every level (encoder, panel, label, corpus, temporal).
- Doctrine invariants enforced via fail-closed structured errors.
- Domain-specific oracles that act as ground truth.

This is a different paradigm than "scale up the LLM, hope reasoning emerges." It's: build the right verifier, let agents iterate against it, achieve compounding improvement through closed-loop adaptation.

If this architecture pattern reaches its goal in Python code verification, it becomes a strong candidate for the architectural template of domain-general super-intelligence — replicated across domains with retargeted instruments and oracles, composed into multi-domain systems via shared frozen-target panels.

### 8.6 The end of "vibe-based" software engineering

Currently, software engineering at AI scale is bottlenecked by humans applying taste to AI output. "Does this look right?" is a vibe judgment. ME-JEPA replaces vibe with measurement: `Pass | Fail` against a calibrated verifier with bounded error.

This doesn't mean human taste is obsolete — it means human taste applies to architecture, design, requirements, and strategic decisions, not to per-patch verdicts. The vibe layer compresses into the verifier's calibration; the human layer expands into the strategic.

---

## §9 — The state is not the constraint: when current state is insufficient

A reader of §1-§8 might come away thinking the 300×8 SWE-bench Lite corpus, the current 15-slot panel, the current Docker oracle harness, and the `0.95` target threshold are all *fixed*. They are not. They are **the current state of the project**, which is the starting point for reaching the targets — but they are not the constraints.

The constraints are the Tier-4 doctrine invariants in §4. Everything else is movable, provided the move serves a target.

### 9.1 What CAN change (state-change levers per tier)

| State surface | Movable? | Examples of legitimate state-change |
|---|---|---|
| Corpus size and composition | Yes | SWE-bench Lite (300) → Verified (500) → Full (2,294) → CommitPack 4TB → BugsInPy + ATOMIC + CauseNet |
| Instrument panel composition | Yes | In-slot causal embedder replacement (#745); new domain slots (#519); panel B encoder set (#405) |
| Embedder weights and training corpora | Yes | Replace E5/E11/E14 (#745); train new C_TemporalCommit/C_CommonsenseCausal/C_BugFix; retire dominated embedders (#518) |
| Oracle harness | Yes | Replica-mode voting (#723); per-instance flakiness modeling (BLINDSPOT-014); per-test environment isolation |
| Predictor architecture | Yes | Iterative head (#725); PanelGraph multi-chunk (#726); hierarchical CE head for Q3-as-primary; predictive-coding head (#538 reserve) |
| Calibration procedure | Yes | Per-cell conformal; isotonic/Platt vs current rank-lift; per-slot OOD refinement |
| Ship-gate threshold itself | Yes (upward only) | `0.95` → `0.97` if BLINDSPOT-001 measures oracle floor higher than `0.97`; or admit irreducible-noise carve-out for flaky cells |
| Substrate infrastructure | Yes | Incremental forward cache (#733); v2 9-slot static cache (#715); aiwonder ZFS migration (#372 closed); future hardware tiers |
| Multi-language support | Yes | Python-only ship gate → Rust → JS/TS → all 11 languages in `Language::all()` |

### 9.2 What CANNOT change (Tier-4 invariant fences)

These are not levers because changing them invalidates the architectural claim:

- Slot identity preservation (`slot_identity_preserved = exactly true`)
- Anti-compensation invariant (`flat_vector_concat_used = exactly false`)
- Binary contract (Q1/Q2/Q5 only; Q4 frozen; no ambiguous heads)
- No inner LLM (`feature = "llm"` compile guard)
- Aiwonder durable root (`/zfs/archive/contextgraph/...` only)
- Leakage policy (`target_labels_used_as_live_inputs = false`)
- ΔΞ.target_collapse = exactly 0
- Cross-panel Goodhart fail-closed

Reaching the Tier-1 targets via Tier-4 violation is not reaching them. It is cheating. The system has not worked.

### 9.3 The corpus-expansion scenario in detail

The most likely state-change scenario is corpus expansion. Current state: 300×8 = 2,400 rows, 240 holdout. This produces a Pearson 95% CI of about ±0.062 at r=0.95 — looser than ideal for confident gate decisions.

| Add to current | Total rows | Total holdout | Pearson CI at r=0.95 | Aiwonder cost (forward + oracle) |
|---|---|---|---|---|
| (current) | 2,400 | 240 | ±0.062 | shipped baseline |
| + SWE-bench Verified (500 instances) | 6,400 | 640 | ±0.036 | ~3-5 days |
| + SWE-bench Full (2,294 instances) | 24,752 | 2,075 | ±0.014 | ~10-14 days additional |
| + BugsInPy (493 real Python bugs) | 28,696 | 2,469 | ±0.013 | ~3-5 days additional + oracle-adapter work |
| (for training only) + CommitPackFT 2GB | adds ~742k training pairs | (not gate corpus) | n/a | ~3-4 days additional |

**Minimum corpus expansion to bound Pearson CI to ±0.025 at r=0.95 (operational tolerance):** ~440 holdout rows → roughly 2× the current corpus. Adding SWE-bench Verified alone achieves this.

**Minimum corpus expansion to bound Pearson CI to ±0.01:** ~1,100 holdout rows → roughly 5× the current corpus. Adding both Verified and Full achieves this.

### 9.4 The panel-expansion scenario in detail

If BLINDSPOT-010 (#734) measures `I(panel; oracle) < 0.95 bits`, the substrate is information-theoretically underpowered. **More data does not fix this.** The fix is upstream: add instrument slots that carry independent information about the oracle.

In-slot replacement strategy (#745) avoids schema change by reusing already-empty slots (E5, E11, E14). Cost: ~3 weeks per slot for training + ~6-8h GPU per slot for re-bake. Three replacements in sequence: ~3 weeks total wall-clock on aiwonder.

Adding entirely new slots (rather than replacing existing) requires `CF_MEJEPA_PANELS` schema change + #733 incremental forward cache as hard prerequisite. Cost: substantially higher; deferred to post-0.95 creativity engine (#519).

### 9.5 The oracle-refinement scenario in detail

If BLINDSPOT-001 (#723) measures `oracle_self_consistency < 0.95`, the Tier-1 target #1 is mathematically unreachable at face value. Two legitimate state-changes:

1. **Improve oracle determinism.** Per-instance flakiness modeling (BLINDSPOT-014 / #738); replica-mode voting; environment isolation. Costs ~1-2 weeks on aiwonder.
2. **Revise the Tier-1 target downward to match floor.** `0.95` → `min(0.95, oracle_self_consistency - ε)`. This is a doctrine-level decision (operator-authorized `type:decision`) and only legitimate if oracle determinism cannot be improved.

Both are valid responses. The second is admitting that the corpus itself has irreducible noise; the first is reducing that noise.

### 9.6 The decision rule for invoking a state-change lever

Before any state change is proposed, the agent or operator answers:

1. **Which target is stalled?** Tier + variable + current measurement + gap.
2. **Why does the current state make that target unreachable?** FSV evidence — not speculation.
3. **What is the minimum state change?** Don't over-shoot. If 2× corpus expansion suffices, don't propose 10×.
4. **Are all Tier-4 invariants preserved?** Enumerate. Any violation = the proposal does not ship.
5. **What is the aiwonder execution cost?** Wall-clock GPU days, durable storage cost, operator review burden, downstream re-bake cost.
6. **What is the falsification condition?** Under what measured outcome does the state change get rolled back?

If (1)-(6) are answered, file as `type:decision` issue with operator authorization, execute on aiwonder per `CLAUDE.md §6.7`, verify via manual SoT readback per `06 §1.9`, re-measure the targeted variable.

### 9.7 What this section prevents

- **State paralysis.** "We can't reach 0.95 on the current corpus, so let's lower to 0.85." Wrong: corpus expansion is the answer.
- **Architectural ossification.** "Adding slots needs #733 first, so we can't." Wrong: #733 is a state change; build it.
- **False scarcity.** "Only 300 instances exist in SWE-bench Lite." Wrong: SWE-bench Full has 2,294; BugsInPy 493; CommitPackFT 742k; combined public causal substrate is ~5 orders of magnitude larger.
- **Doctrine creep.** "We need to relax slot identity to reach 0.95." Wrong: Tier-4 invariants are fences; reaching targets via doctrine violation is not reaching them.

### 9.8 The summary rule

> **Reaching the numerical predicate is the goal. The state of the system is what we change to reach it. The doctrine invariants are what we never change. Anything else is movable.**

---

## §10 — Closing: the goal in one paragraph

When every variable in `docs/futurebuild/specs/TARGET-METRICS.md` evaluates to its target value simultaneously — Tier 1's four ship-gate numbers, Tier 2's per-cell quality contract across every cell, Tier 3's mistake-loop convergence, Tier 4's doctrine invariants holding exactly, Tier 5's substrate quality meeting its information-theoretic floors, and Tier 6's embedder-gating discipline maintained — the system has functionally replaced human review of AI-generated Python patches with a calibrated mathematical verifier running in milliseconds on aiwonder's RTX 5090. At that moment, the bottleneck between AI code generation and shipped software dissolves. The post-0.95 creativity engine activates, the architecture replicates across domains, the mistake loop keeps the verifier sharp through production drift, and the path to broader autonomous engineering becomes a tractable engineering problem rather than a research conjecture. Every issue in the GitHub ledger, every commit in the repo, every embedder trained on aiwonder, every FSV evidence file under `/zfs/archive/contextgraph/fsv/...` exists to move at least one of these numbers toward its target. That is the goal. There is no other goal.

---

**Cross-references:**
- [`docs/futurebuild/specs/NORTH-STAR.md`](../docs/futurebuild/specs/NORTH-STAR.md) — explicit north star + worthlessness test
- [`docs/futurebuild/specs/TARGET-METRICS.md`](../docs/futurebuild/specs/TARGET-METRICS.md) — the numerical predicate
- [`AICodingAgentSuperPrompt.md`](AICodingAgentSuperPrompt.md) §0.0 — universal worthlessness test pattern
- [`../CLAUDE.md`](../CLAUDE.md) §0 — project-specific north-star instantiation
- [`../AGENTS.md`](../AGENTS.md) §0 — per-turn north-star reference
- Pinned GitHub context: `[P0] CONTEXT — Numerical target sheet (the perfect-state predicate) — THE GOAL` (#753)
- Critical path: #712
- Audit aggregator: #739

**Last updated:** 2026-05-24. Update this doc when the meaning or interpretation of a target variable changes, or when the unlocked-state framing requires refinement. Numerical updates belong in `TARGET-METRICS.md`; framing updates belong in `NORTH-STAR.md`; this file owns the per-variable depth and the second-order consequences.
