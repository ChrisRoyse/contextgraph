# 7. Worked Instantiation: ME-JEPA-Code on Python

This section instantiates the framework on a live system and reports the numbers it produced. Every figure is traceable to a source-of-truth artifact (§8); provenance is given inline as an FSV path under `/zfs/archive/contextgraph/...` or a tracked issue number.

## 7.1 The domain and the oracle

- **Domain `D`:** binary `Pass/Fail` prediction for a Python code change.
- **Oracle:** a containerized SWE-bench test harness — `python -m swebench.harness.run_evaluation` — returning a deterministic verdict for one instance [Jimenez2024].
- **Corpus:** SWE-bench Lite, **300 instances × 8 mutation categories** = 2,400 candidates. The eight categories are `known_good`, `subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error`. The MI dataset has 2,400 rows (300 per category), `feature_dim = 19209`, outcome entropy `target_bits = 0.9978` (label counts: Pass = 1267, Fail = 1133 — near-balanced, so `H(Y) ≈ 1 bit`).
- **Panel:** 9 active sensors — `e1, e6, e7, e8, e9, e10, e12, e13, e14` — encoding code structure and content; with `slot_identity_preserved = true` and `flat_vector_concat_used = false` enforced (§6.2).

This domain satisfies eligibility prerequisites 1, 2, 4, 5 (§3.7): it has a deterministic oracle, the oracle is reproducible (§7.2), the intervention×subdomain matrix is enumerable (8 categories × repos), and outcomes are feedable. The open question is prerequisite 3 — is the panel a sufficient substrate? — which §7.3 answers.

## 7.2 The anchor is clean on the flakiness axis

Oracle self-consistency was measured by replica voting: 30 instances × 10 independent Docker evaluations = 300 invocations. Result: **`oracle_self_consistency = 1.0`** — every replica agreed with its peers; 300/300 reports produced; all replica exit codes 0; no missing reports.

> Provenance: issue #760; FSV `/zfs/archive/contextgraph/fsv/inv-035-oracle-noise-floor-fsv/run-20260529T233417Z`.

Interpretation, with the §2.3 distinction held firmly: this bounds **flakiness only**. It establishes that the oracle reproduces its own verdict deterministically, so it imposes no flakiness ceiling on `τ_corr = 0.95` (the Tier-5.2 check passes, `τ_sc = 0.97`). It does **not** establish **validity** — the SWE-bench audits [Aleithan2024; Wang2025; OpenAI2026] document weak-test and solution-leak validity failures that a self-consistency probe cannot see (§7.6, §11). The honest reading is: *the anchor does not wobble; whether it points at exactly the right thing is a separate, still-open question.*

## 7.3 The substrate is insufficient — the kernel does not exist at this panel

The panel-sufficiency MI probe (§5) was run over the 2,400-row dataset using the estimator stack, with a validated self-test (a strong-signal synthetic control returned `0.977` bits; a noise control returned `0.0`). Results:

| Estimator | `I(panel; oracle)` | vs. `τ_MI = 0.95 bits` |
|---|---|---|
| MINE (Donsker–Varadhan, bounded best validation) | **0.0960 bits** | ≪ |
| Classifier-based lower bound (heldout) | **0.1254 bits** | ≪ |
| Ross/Kraskov (k-NN, after deterministic projection) | **0.4611 bits** | ≪ |

Decision: **`panel_sufficient = false`** — classification *panel-provably-insufficient*. Because these are lower-bound (or conservative) estimators, their agreement that `I` is far below threshold is robust (§5.4): the panel carries at most ~0.46 of the ~1 bit the verdict requires.

> Provenance: issue #759; FSV `/zfs/archive/contextgraph/fsv/inv-032-panel-sufficiency-mi-fsv/run-20260529T234900Z/full-candidate-map`. Dataset: 2,400 rows, `feature_dim = 19209`, `target_bits = 0.9978`, slot identity preserved, no flat-concat.

By the data-processing ceiling (§5.2), this is decisive: **no predictor, no kernel/MinSet search, and no calibration reading this panel can reach `τ_corr = 0.95`.** A grounding kernel of any useful size does not exist at this panel (§4.6, question 2 = *no*). The deficit is upstream, in the sensors.

## 7.4 Where the bits are missing: form, not behavior

Two decompositions (§5.5) localize the deficit precisely.

**Per-cell MI** (`I(panel; oracle | cell)`):

| Cell | `I` (bits) | Character |
|---|---|---|
| `swap_variable` | **0.2313** | purely syntactic (tokens visibly change) |
| `off_by_one` | 0.0207 | borderline syntactic |
| `compile_error`, `delete_test_call`, `wrong_file`, `over_engineer`, `subtle_flip` | ≈ 0 | **behavioral** (verdict set by runtime behavior) |

**Drop-one-sensor ablation** (largest marginal contributions):

| Sensor | `ΔI` when dropped (bits) |
|---|---|
| `e8` | 0.0309 |
| `e6` | 0.0294 |
| `e7` | 0.0214 |
| `e14` | 0.0189 |

> Provenance: issue #759 RESOLVED readback (same FSV run as §7.3).

Two conclusions follow, exactly matching the §6.4 pathology:

1. **No sensor is load-bearing.** The single largest contribution is `0.031` bits. This is *absence* of signal across the board, not redundancy among slots — redundancy would show small drop-one deltas together with a *high* joint `I`; here both the deltas and the joint (`≤0.46`) are low.
2. **The panel sees form, not behavior.** The one cell with real signal is the pure token-level edit (`swap_variable`); the cells whose verdict is determined by *what the code does* when run (does it compile? was a needed call deleted? is it the wrong file?) read essentially zero. The sensors encode "what the code looks like" (similarity/retrieval representations) and are nearly blind to "what the code does."

This is the **Vector Grounding Problem** [Mollo2023] made empirical: the sensors are referentially grounded in surface form, not in the executed outcome the oracle measures.

## 7.5 The predictor numbers, in their proper place

For completeness, the predictor head over this panel achieves raw Pearson correlation with the oracle of `0.4843`, with aggregate Brier `0.0924` and ECE `0.0240` [Brier1950; Guo2017]. The honest gap to the gate is `0.95 − 0.4843 = 0.4657`.

> Provenance: ship-gate FSV `swebench-300x8-ship-gate-fsv/run-20260521T132308Z`.

Crucially, §7.3 reframes what this gap *is*. It is **not** primarily a predictor-quality gap to be closed by better training or calibration — those operations are bounded by `I(panel; oracle) ≤ 0.46` bits (§5.2) and cannot reach `0.95`. The Pearson number is what an information-starved panel permits, and it will not move materially until the panel does. A separate "rank-lift" diagnostic that reports `0.7942` is explicitly **not** the predictor's oracle correlation; it is a category-local exemplar-similarity transform that lifts the apparent number without improving Brier or ECE, and is treated as a Goodhart artifact [Manheim2018], not a gate pass.

## 7.6 Honest status of the instantiation

Placing the system against the framework's gates:

| Gate | Status |
|---|---|
| Oracle reachability — flakiness (T5.2) | **Pass** — `oracle_self_consistency = 1.0` |
| Oracle validity (§2.3) | **Open** — weak-test / leakage risk per [Aleithan2024; Wang2025; OpenAI2026]; not yet bounded for this corpus |
| Substrate sufficiency (T5.1) | **Fail** — `I(panel; oracle) ≤ 0.46 < 0.95` bits |
| Kernel existence (§4.6) | **No** at this panel (entailed by the substrate failure) |
| Ship-gate correlation (T1.1) | **Fail** — `0.4843 < 0.95`, and capped by the substrate failure |
| Per-cell, stability, cross-panel, online (T1.2–T1.4, T2, T3) | **Not yet assessable** — blocked behind substrate |

The instantiation is therefore a **clean negative result of the kind the framework is designed to produce early**: the cheapest decisive test (substrate MI) falsified the current architecture before expensive model work was justified, and the per-cell/per-sensor decomposition turned the falsification into a direction.

## 7.7 The forward plan this produces

The framework prescribes the next step without guessing (§6.5): **assemble a sufficient panel by measured bit-contribution.** Acquire candidate sensors along the axes the panel lacks — outcome/causal encoders (trained on *edit → outcome*) and execution-derived sensors (traces, partial-run/coverage, exception signatures, dependency/runtime/environment state); run each over the corpus; measure standalone `I` and greedy `ΔI` with per-cell attention to the behavioral cells now at zero; greedily assemble under the Tier-6 novelty and utility gates; and accept only when the substrate test returns `I(panel; oracle) ≥ 0.95` bits with behavioral-cell MI lifted off zero. Only then is the kernel computation well-posed, and only then can the predictor and calibrator move the Tier-1 number toward the gate.

> Tracked as: substrate reframe #842; kernel/MGS computation harness #843; panel sufficiency / in-slot sensor replacement #745; embedder candidate-screening and assembly loop #844; constellation-as-definition derivation engine #841. The oracle-validity question (§7.6) remains the most important open audit.
