# The Universal Benchmark Predicate — Falsifiable Domain Super-Intelligence

> **One-sentence purpose.** This document is the domain-agnostic formula for what counts as super-intelligence in *any* domain, the 6-tier benchmark stack that has to be hit for a domain to be ship-eligible, what each tier means against physical reality, and exactly what changes in the world once all benchmarks hit simultaneously in a domain. ME-JEPA-Code on Python is the first instantiation; the same predicate is the template for every subsequent domain.

**Authority.** This file is the universal generalization of the predicate concretized in [`docs/futurebuild/specs/TARGET-METRICS.md`](futurebuild/specs/TARGET-METRICS.md) (numerical) and [`docs/futurebuild/specs/NORTH-STAR.md`](futurebuild/specs/NORTH-STAR.md) (rationale). When this file and `TARGET-METRICS.md` disagree on Python-specific numbers, `TARGET-METRICS.md` wins. When this file and `CLAUDE.md` disagree on an invariant, `CLAUDE.md` wins. This file's role is to give every future domain an off-the-shelf benchmark stack and to define what the project's claim of "super-intelligence" actually means.

**Last updated:** 2026-05-26.

---

## 0. The Universal Formula (the entire goal, in one block)

For an arbitrary domain `D`, define:

- `Oracle_D : Artifact_D → {Pass, Fail}` — a deterministic, machine-readable ground-truth function over candidate artifacts in `D`.
- `Panel_D : Artifact_D → Array<Vec_i>` — an observation function that packs an artifact into an array of per-sensor vectors, **each vector in its own sensor's space**, no flat-buffer concatenation.
- `Corpus_D` — a representative set of `(artifact, oracle_label)` pairs covering an `(intervention_class × subdomain)` matrix of `D`.
- `Predictor_D : Panel_D(x) → {Pass, Fail}` — a deterministic learned function.

**Then `D` admits superhuman binary verification iff:**

```
SUPERHUMAN_VERIFIER(D) ⇔ ∃ Predictor_D such that:

  corr(Predictor_D(x), Oracle_D(x)) ≥ τ_corr(D)                                 // Tier 1.1
  STABLE × k_windows consecutive rolling windows                                // Tier 1.2
  AND corr(Predictor_D_B(x), Oracle_D(x)) ≥ τ_corr(D)  with Panel_B            // Tier 1.3 (Goodhart guard)
  AND |corr_A − corr_B| ≤ ε_drift                                              // Tier 1.4 (panel drift)
  AND per-cell quality contract holds for every (intervention_class × subdomain) cell  // Tier 2
  AND mistake_repeat_rate ≤ δ_repeat over rolling N-mistake window             // Tier 3.1
  AND unrelated_control_delta ≤ ε_unrelated                                    // Tier 3.2
  AND every doctrine invariant equals its required value exactly               // Tier 4
  AND I(Panel_D(x); Oracle_D(x)) ≥ τ_MI bits                                   // Tier 5.1 (substrate sufficiency)
  AND oracle_self_consistency ≥ τ_sc                                            // Tier 5.2 (oracle reachability)
  AND inter_sensor_distinctness gate passes for all active slots               // Tier 5.3
  AND for any new/replaced sensor: novelty + utility gates pass                // Tier 6
```

`τ_corr(D)` is set by the cost asymmetry between false-positive Pass and false-negative Pass in domain `D`, **bounded above by `oracle_self_consistency`** — you cannot correlate with an oracle better than the oracle correlates with itself. For ME-JEPA-Code on Python, `τ_corr = 0.95`, `k_windows = 4`, `ε_drift = 0.05`, `δ_repeat = 0.05`, `ε_unrelated = 0.02`, `τ_MI = 0.95 bits`, `τ_sc = 0.97`.

When this predicate evaluates `TRUE` for domain `D`, the consequence is unambiguous and stated in §4: **human review of AI-generated artifacts in domain `D` becomes statistically unjustified.** That is the falsifiable definition of operational super-intelligence in `D`. There is no other goal in `D`.

This is super-intelligence in the **operational, falsifiable** sense: a system that replaces a human function at higher accuracy and at machine-scale throughput, in a domain where ground truth is recoverable. It is *not* a claim about general intelligence, sentience, or any property without a deterministic oracle.

---

## 1. The 5 Eligibility Prerequisites — Before Benchmarks Even Apply

A domain is **not eligible** for the predicate unless all five hold. Attempting to claim super-intelligence in an ineligible domain is doctrine breach.

| # | Prerequisite | Why it is non-negotiable |
|---|---|---|
| **E1** | **Deterministic, machine-readable oracle exists.** `Oracle_D : Artifact_D → {Pass, Fail}` must return a label without human adjudication. | Without an oracle, there is no ground truth to correlate with. "Human judgment" is not an oracle; it is variance. |
| **E2** | **Question reduces to free binary form against reality.** Pass/Fail must be derivable without an arbitrary threshold over a continuous heuristic. | Continuous quantities (perf wall-clock, mutation survival, beauty score) only become binary via thresholds. Threshold = heuristic = not reality. Mistake-loop convergence requires real disagreement with reality. |
| **E3** | **Representative corpus is constructible.** The `(intervention_class × subdomain)` matrix can be enumerated and sampled at scale supporting Tier-2 per-cell power (n ≥ 50 per cell). | Without coverage, Tier-2 per-cell metrics are uncomputable and the system silently fails on under-sampled cells. |
| **E4** | **Reality emits slot-decomposable signals.** Observable evidence can be packed into an *array of per-sensor vectors* with preserved sensor identity (no flat-buffer fusion). | Cross-sensor dim compare is forbidden (slot identity invariant). Domains where evidence is irreducibly scalar (a single number) or irreducibly entangled cannot run the architecture. |
| **E5** | **Oracle latency is bounded at training scale.** Running `Oracle_D` at the rate the mistake loop demands is economically feasible. | Without a closeable loop, the predictor never adapts to its own mistakes. Tier 3 becomes unmeasurable. |

**Failure modes per prerequisite:**

| Prerequisite failure | Domain example | What to do instead |
|---|---|---|
| **E1 fails** (no oracle) | "Is this poem beautiful?" | Permanently out of scope. Not solvable by this framework. |
| **E2 fails** (no binary form) | "Is this code performant *enough*?" | Frozen as Q4-class ambiguous head (#408). Not a benchmark. |
| **E3 fails** (corpus uncoverable) | "Predict all possible future pandemics" | Restrict to a coverable sub-domain or abandon. |
| **E4 fails** (signals not slot-decomposable) | Pure single-scalar oracles with no observable substrate | Add instrumentation or reframe the artifact. |
| **E5 fails** (oracle too slow) | "Does this drug extend lifespan in humans?" (oracle latency = decades) | Use proxy oracle (e.g., model organisms) with explicit `oracle_proxy_drift` bound, or abandon. |

The 5 prerequisites are gates, not levers. If a domain fails them, the response is not "lower the bar" — it is "this domain is not eligible for the predicate."

---

## 2. The 6 Tiers (Domain-Agnostic)

| Tier | Name | Role | What "hit" means in domain `D` |
|---|---|---|---|
| **1** | **Ship-gate numbers** | The 4 numbers whose simultaneous truth ships `D` | Correlation + stability + cross-panel + drift in `D` |
| **2** | **Per-cell quality contract** | Per `(intervention_class × subdomain)` quality floor in `D` | Every cell in `D`'s matrix passes its appropriate metric |
| **3** | **Online mistake-loop convergence** | Proof the system gets better when wrong in `D` | Repeat-rate + control delta in `D` |
| **4** | **Doctrine invariants** | Fences, not goals — exact values at all times, **identical across all domains** | Exact equality, monitored continuously |
| **5** | **Substrate quality** | Proves Tiers 1–3 are *reachable* in `D` on current state | Audit measurements pass cheap probes |
| **6** | **Sensor/embedder gating** | Quality filter for any new/replacement sensor in `D` | Novelty + utility thresholds passed |

Tier 4 is invariant across all domains. Tiers 1, 2, 3, 5, 6 instantiate per domain with `D`-specific thresholds bounded by oracle self-consistency. Tier 4 supersedes all others: a Tier-1 pass that breaches a Tier-4 invariant is doctrine breach, not a pass.

---

### 2.1 Tier 1 — The 4 Ship-Gate Numbers (per domain)

| # | Variable | Target | What it actually means in domain `D` |
|---|---|---|---|
| **1** | `prediction_oracle_correlation(D)` (raw Pearson, `Predictor_D` verdict vs `Oracle_D` Pass/Fail) | **≥ τ_corr(D)** | On a fresh holdout of representative artifacts in `D`, the predictor agrees with the oracle at Pearson ≥ `τ_corr(D)`. At this threshold, false-positive Pass rate is below the cost-asymmetry threshold for unsupervised production deployment in `D`. |
| **2** | Rolling-window stability | **`k_windows` consecutive windows** with #1 holding | Not a flake. The same correlation holds across `k_windows` disjoint windows (rotation of calibration vs holdout). |
| **3** | Cross-panel `corr_B` (Panel B with non-overlapping sensors re-encoding the same artifacts) | **≥ τ_corr(D)** | Train against Panel A (sensor set 1). Re-encode the same artifacts with Panel B (sensor set 2, disjoint encoders). Panel B clearing `τ_corr` proves the predictor learned about `D`, not about Panel A's encoders. |
| **4** | `\|corr_A − corr_B\|` panel drift | **≤ ε_drift** | The two independently-encoded panels reach the same Pass/Fail verdict within `ε_drift`. Drift > `ε_drift` implies encoder-specific learning, which is Goodhart. |

**Setting `τ_corr(D)`.** The threshold is the smallest number satisfying both:
- `τ_corr(D) ≥ cost_asymmetry_floor(D)` — false-positive Pass is rare enough that downstream human review of uncertain cases can absorb residual error at realistic throughput.
- `τ_corr(D) ≤ oracle_self_consistency(D)` — you cannot correlate with an oracle better than the oracle correlates with itself; this is the hard ceiling.

If `cost_asymmetry_floor > oracle_self_consistency`, the domain is **not yet** ship-eligible — either oracle refinement is needed (replica voting, flakiness modeling) or the cost asymmetry argues for keeping human review.

**The two cheap probes that falsify Tier 1 in advance:**

| Probe | Decision rule |
|---|---|
| Measure `I(Panel_D; Oracle_D)` (panel-sufficiency MI) | If `< τ_MI bits`, panel is provably underpowered — no model work moves Tier 1. State change required (add slots). |
| Measure `oracle_self_consistency(D)` (replica voting) | If `< τ_corr`, Tier 1 is mathematically unreachable as stated. Either refine the oracle or revise `τ_corr` downward to `oracle_self_consistency − ε` and document the carve-out. |

---

### 2.2 Tier 2 — Per-Cell Quality Contract

A single aggregate correlation can hide a broken cell. Tier 2 enforces a quality floor on **every** `(intervention_class × subdomain)` cell in `D`'s matrix, so that one strong cell cannot mask a failing cell.

Cells are classified by oracle variance:

| Cell type | Metric | Target | What it actually means |
|---|---|---|---|
| **Mixed** (both Pass & Fail oracle rows) | Per-cell Pearson + Fisher-z CI | **≥ τ_corr(D)** with CI passing | Same as Tier-1 #1, but per cell. Every mixed cell hits the bar. |
| Mixed | Brier score | **≤ τ_brier(D)** | Probabilistic verdicts are well-calibrated (`Pass with 0.93 confidence` = 93% Pass in reality). |
| Mixed | Expected Calibration Error (ECE) | **≤ τ_ece(D)** | Aggregate calibration error across confidence bins. |
| **All-pass** (structurally Pass cells) | Pass-class recall | **≥ τ_corr(D)** | Predictor labels ≥ `τ_corr` of artifacts with the correct Pass verdict (Pearson is undefined on single-class cells). |
| All-pass | Pass-class actual-class confidence mean | **≥ τ_corr(D)** | Average confidence on the (correct) Pass call is ≥ `τ_corr`. |
| **All-fail** (structurally Fail cells) | Fail-class recall + Wilson lower bound | **≥ τ_corr(D)** | Predictor labels ≥ `τ_corr` of artifacts Fail, with Wilson lower bound on small samples. |
| All-fail | Fail-class actual-class confidence mean | **≥ τ_corr(D)** | Average confidence on the (correct) Fail call is ≥ `τ_corr`. |
| **All cells** | Conformal coverage (split-conformal predictor) | **within `[1−α−ε, 1−α+ε]`** | Conformal interval covers the true outcome at the specified rate. |
| All cells | Cell support `n_supporting` | **≥ N_min(D)** else cold-cell `Abstain` | Each cell has ≥ `N_min` calibration rows; under-supported cells abstain rather than guess. |

`τ_brier(D)`, `τ_ece(D)`, `N_min(D)` are domain-tuned but follow the standard "strictly proper scoring rule well-calibrated" cutoffs (typically `0.05`, `0.05`, `50` respectively).

---

### 2.3 Tier 3 — Online Mistake-Loop Convergence

Tiers 1 and 2 measure prediction accuracy at one point in time. Tier 3 proves the system **adapts** when it is wrong, without human supervision and without breaking unrelated state.

| Variable | Target | What it actually means |
|---|---|---|
| `mistake_repeat_rate` (rolling `N_mistake` window) | **≤ δ_repeat** | If the predictor gets the same `(cell × pattern)` wrong once, it gets it wrong at most `δ_repeat` of the time on next presentation. Computed from the durable mistake log. |
| `unrelated_cell_accuracy_delta` (control panel before/after mistake update) | **≤ ε_unrelated** | Updating the predictor on mistake `X` does not damage accuracy on unrelated cell `Y`. Protects against catastrophic forgetting. |
| `claims_strong_protection` (e.g., Fisher/EWC) | **`false` until backed by durable evidence** | Honesty bit — until durable protection state exists, the system must not advertise strong continual-learning guarantees. |

**Why this matters beyond accuracy.** Best-of-K inference papers over an under-trained predictor by spending compute at inference time. The mistake loop pays the cost at *learning* time and converges to a predictor that is right on first try. "If it needs 4 guesses to be most correct, it is already wrong." A predictor that does not close the loop on its own mistakes is not super-intelligent — it is merely accurate at a frozen point in time.

---

### 2.4 Tier 4 — Doctrine Invariants (Universal — Identical Across All Domains)

These do not move. They equal the values below **exactly, at every moment, including during tests and in dev branches**, in every domain `D`. A Tier-1 pass that breaches a Tier-4 invariant is doctrine breach, not a pass.

| Variable | Required value | Why it cannot move |
|---|---|---|
| Frozen-target gradient leak | **exactly `0`** | Target-side instruments must carry zero trainable params; non-zero ⇒ instrument gradient leak. Enforced structurally via empty `parameters()` on the target adapter. |
| Ambiguous-head loss coefficient | **exactly `0.0`** | Ambiguous heads (continuous heuristics without binary form against reality) have zero loss contribution. Frozen in this project as Q4 (#408). |
| Ambiguous-head inputs to ship-gate | **exactly `0`** | Ambiguous surfaces must not feed the ship-gate metric. |
| Ambiguous-head row delta over time | **exactly `0`** | No new ambiguous-head producer writes after freeze. |
| Flat-vector concat over panel | **exactly `false`** | Predictor never operates over the byte-layout buffer as a flat vector. Panel is always an *array of per-sensor vectors*. |
| Slot identity preserved | **exactly `true`** | Same-sensor dim compare only. Cross-sensor dim compare is forbidden. |
| Target labels used as live inputs | **exactly `false`** | Leakage policy — oracle labels are target-side only, never live predictor inputs. |
| Cross-panel Goodhart detected | fail-closed → block promotion | Panel A pass without Panel B agreement is treated as memorization. |
| Panel B resident during user-facing inference | **exactly `false`** | Panel B is loaded only during eval worker, never during serving (cost + isolation). |
| Inner-generator ban | **compile-error guard active** | The system is a predictor, not a generator. Any code path that would reintroduce a generative model fails to compile. |
| Durable storage root invariant | **domain-pinned production path** | All production durable state lives on the pinned root (in this project: `/zfs/archive/contextgraph/...`). Forbidden roots blocked by CI lint. |

**Why invariants are sacred.** Numbers (Tiers 1/2/3/5/6) can be Goodhart'd — optimizing the proxy until the underlying property is lost. Invariants cannot — they either hold or they don't. Reaching the targets via invariant violation is **not** reaching them; it is cheating with a deployment-blocking failure mode.

---

### 2.5 Tier 5 — Substrate Quality (Are Tiers 1–3 Reachable?)

Tier 5 measurements are the cheapest highest-leverage tests. If they fail, **no amount of model work moves Tier 1** — the system has the wrong substrate and must change state before more model work.

| Variable | Target | What it actually means |
|---|---|---|
| `I(Panel_D; Oracle_D)` (panel-sufficiency MI) | **≥ τ_MI bits** | Probability-theoretic upper bound on what *any* predictor reading this panel can know about the oracle outcome. If less than `τ_MI`, the panel literally cannot determine the verdict, and adding training data is wasted. |
| `oracle_self_consistency(D)` (n-replica per-artifact) | **≥ τ_sc** | Run the oracle n× on the same artifact. Measure how often it agrees with itself. If `< τ_corr`, the target threshold is mathematically unreachable as stated. |
| `inter_sensor_distinctness` gate | passes for all active slots | No two sensors alias each other's vector space. Aliased spaces produce a sensor-identity collapse and silent failure. |
| Per-sensor pretraining-corpus overlap with eval corpus | documented; counterfactual cutoff experiment if overlap > 30% | Pretraining-corpus Goodhart audit — were the test artifacts in the sensor's training data? |

**State-change levers if Tier 5 fails:** add sensors, expand corpus, refine oracle harness (replica voting, flakiness modeling), or, if oracle self-consistency proves a hard ceiling lower than `τ_corr`, revise `τ_corr` downward to `oracle_self_consistency − ε` and document the carve-out. **Lowering `τ_corr` arbitrarily because the current state cannot reach it is forbidden** — that is doctrine breach, not progress.

---

### 2.6 Tier 6 — Sensor/Embedder Gating

Tier 6 is the quality filter for *any* sensor addition or replacement. It prevents bloat and Goodhart on sensor proliferation.

| Variable | Target | What it actually means |
|---|---|---|
| `τ_novelty` (max-correlation of new sensor vs existing slots) | **< 0.7** for promotion | New sensor must add information distinct from existing slots. Above 0.7 is redundancy. |
| `δ_utility` (per-cell oracle correlation lift) | **≥ 0.005** for promotion | New sensor must improve per-cell oracle correlation by at least 0.005. Below this is noise. |
| `holdout_corr_delta_when_dropped` (slot ablation) | **< 0.001** + variance ≈ 0 → replace slot; **≥ 0.005** → keep slot | Ablation test: drop one slot, measure holdout correlation. Low-impact slots become replacement candidates. |
| Domain-specific directionality / structural tests | per domain | E.g., asymmetric retrieval Spearman ≤ 0.5 for causal sensors; positional invariance for spatial sensors; etc. |

---

## 3. Instantiation Template — Adapting the Predicate to Any Domain `D`

To apply the predicate to a new domain, the operator fills in this table:

| Symbol | Meaning | Worked example (ME-JEPA-Code Python) |
|---|---|---|
| `D` | Domain name | "Python code correctness against test suite" |
| `Artifact_D` | What gets predicted | candidate code patch |
| `Oracle_D` | Deterministic ground-truth function | `swebench.harness.run_evaluation` (Docker) |
| `Panel_D` | Slot-decomposable observation function | 15 frozen instruments → 15-vector array |
| `Corpus_D` | Representative `(artifact, label)` pairs | 300×8 SWE-bench Lite (2,400 rows) |
| `intervention_class` axis | Categories of perturbation/situation | 8 mutation operators (`known_good`, `subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error`) |
| `subdomain` axis | Stratification within the domain | language (currently Python only) |
| `τ_corr(D)` | Cost-asymmetry floor bounded by oracle self-consistency | 0.95 |
| `k_windows` | Rolling-window stability count | 4 |
| `ε_drift` | Panel A vs Panel B correlation drift tolerance | 0.05 |
| `δ_repeat` | Mistake-repeat-rate ceiling | 0.05 |
| `ε_unrelated` | Unrelated-cell perturbation tolerance | 0.02 |
| `τ_MI` | Panel sufficiency MI floor | 0.95 bits |
| `τ_sc` | Oracle self-consistency floor | 0.97 |
| `N_min` | Per-cell support floor (else Abstain) | 50 |
| `Panel_B` | Disjoint-encoder Panel B for cross-panel triangulation | tree-sitter → CodeBERT, e5-large → bge-large, Qodo → Voyage, etc. |

Once filled, the universal formula in §0 instantiates to `D`'s exact ship-gate predicate. The 5 prerequisites in §1 must all hold; the 6 tiers in §2 are then the benchmark stack to be hit.

---

## 4. What "All Benchmarks Hit in Domain `D`" Means

This is the actual answer to "what does success look like." The events are sequential and binding.

### 4.1 The activation moment

```
=== Ship-Gate FSV — All Targets Met (Domain: D) ===

Tier 1 (ship gate):
  prediction_oracle_correlation:      ≥ τ_corr(D)         ✓
  window_stability:                   k_windows / k_windows  ✓
  panel_b_correlation:                ≥ τ_corr(D)         ✓
  panel_drift:                        ≤ ε_drift           ✓

Tier 2 (per-cell):
  per_cell_pearson_min:               ≥ τ_corr(D) every mixed cell
  per_cell_brier_max:                 ≤ τ_brier(D) every cell
  per_cell_ece_max:                   ≤ τ_ece(D) every cell
  all_fail_cell_recall_min:           ≥ τ_corr(D) every all-fail cell
  all_pass_cell_recall_min:           ≥ τ_corr(D) every all-pass cell
  conformal_coverage:                 within [1−α−ε, 1−α+ε]
  cold_cells_remaining:               0

Tier 3 (mistake loop):
  mistake_repeat_rate:                ≤ δ_repeat
  unrelated_control_delta:            ≤ ε_unrelated

Tier 4 (doctrine — exact equality, all invariants intact):
  frozen_target_gradient_leak:        0
  ambiguous_head_loss_coefficient:    0.0
  ambiguous_head_inputs_to_gate:      0
  ambiguous_head_row_delta:           0
  flat_vector_concat_used:            false
  slot_identity_preserved:            true
  target_labels_used_as_live_inputs:  false
  cross_panel_goodhart_detected:      false
  panel_b_resident_during_inference:  false
  inner_generator_compile_guard:      active
  durable_storage_root:               <domain-pinned production path>

Tier 5 (substrate quality):
  I(panel; oracle):                   ≥ τ_MI bits
  oracle_self_consistency:            ≥ τ_sc
  inter_sensor_distinctness_pass:     all active slots

Tier 6 (sensor gating; for any add/replace):
  novelty_oracle_pass:                max_corr < 0.7
  utility_oracle_pass:                per_cell_lift ≥ 0.005

=== Ship-gate predicate evaluates: TRUE ===
=== Human review of AI-generated artifacts in D: STATISTICALLY UNJUSTIFIED ===
```

### 4.2 The 8 sequential consequences

1. **Ship-gate status reports `green`** per-cell across all `(intervention_class × subdomain)` cells in `D`.
2. **Promotion approval engages** operator first-promotion review — the one human-in-the-loop step left.
3. **After operator approval, the active-pointer flips** and the trained checkpoint becomes the serving predictor in `D`.
4. **Prediction surface begins returning `Verdict::Pass | Fail`** with calibrated confidence on every artifact in `D`. Named failure mode on `Fail`, closest exemplars on both. Latency bounded by the predictor's forward pass.
5. **Capture hooks record every reality-outcome event** into the verification log. Any disagreement between predictor and oracle still materializes as a mistake row — the loop never sleeps.
6. **The mistake loop continues consuming** new mistake rows. The predictor's online head corrects same-context future predictions. Convergence target `mistake_repeat_rate ≤ δ_repeat` is monitored against the rolling window.
7. **Per-cell drift guardrails engage.** Any cell whose correlation drops below `τ_corr − 0.02` triggers retraining; below `τ_corr − 0.05` triggers automatic rollback to the previous active pointer.
8. **The post-ship creativity engine begins** in `D`: novelty oracle, mistake-driven sensor demand, retirement audit, domain-routed sparse panel, episodic-to-semantic consolidation. Autonomous bounded growth into the corpus's intrinsic dimensionality.

### 4.3 What changes for users of the system in domain `D`

| Before | After |
|---|---|
| Human reviews every artifact in `D` | Predictor returns `Pass/Fail` in milliseconds; human reviews only `Fail` + cold-cell `Abstain` cases |
| Production throughput bottlenecked by reviewer minutes-per-artifact | Production throughput bottlenecked by oracle wall-clock |
| Wrong artifact ⇒ caught in human review (or later in production) | Wrong artifact ⇒ flagged with named failure mode + closest exemplar before commit |
| No falsifiable definition of "is this AI agent reliable in `D`" | Agent reliability in `D` = `prediction_oracle_correlation` against the predictor — a number on a dashboard |
| Verification cost grows linearly with agent throughput | Verification cost is per-artifact constant (predictor latency) |
| Manual triage of which change broke what | Predictor's mistake log directly identifies the cell + named failure mode |

### 4.4 What does NOT change

- **The product is still binary in `D`.** Pass/Fail is the surface. Explanation surfaces (named failure mode, closest exemplar) are labels on Pass/Fail, not separate predictions. Ambiguous heads remain out of scope.
- **Tier-4 invariants are still continuously enforced.** Slot identity, no flat-vector concat, no inner generator, durable storage root — none of these unlock or relax post-ship. They tighten.
- **Out-of-`D` review is unchanged.** The predictor predicts `Oracle_D` outcome, not human-judgment outcome on properties outside `D`'s oracle (e.g., for a code domain, this excludes architecture, security policy beyond the test suite, business-logic intent, taste).
- **Tier 4 is identical across all domains.** Shipping `D` does not relax invariants for `D'`.

---

## 5. Worked Instantiation: ME-JEPA-Code on Python (the current ContextGraph project)

| Symbol | Concrete value |
|---|---|
| `D` | Python code correctness against the SWE-bench Lite test suite |
| `Artifact_D` | Candidate code patch (unified diff) |
| `Oracle_D` | `swebench.harness.run_evaluation` parsed `FAIL_TO_PASS` / `PASS_TO_PASS` / `FAIL_TO_FAIL` / `PASS_TO_FAIL` per `swebench/harness/grading.py` |
| `Panel_D` | 15-slot frozen instrument panel: `E_AST 384 + E_CFG 256 + E_DataFlow 256 + E_TypeGraph 256 + E_Test 384 + E_Trace 512 + E_Diff 256 + E_Witness 256 + E_Oracle 128 + E_Problem 1024 + E_CommitMsg 384 + E_StaticAnalysis 256 + E_Runtime 256 + E_Reasoning 384 + Scalars 128`, **array of per-sensor vectors**, never flat 5,120-d |
| `Corpus_D` | 300 instances × 8 mutation categories = 2,400 rows on SWE-bench Lite (12 repos: django, sympy, matplotlib, sklearn, pytest, sphinx, astropy, requests, +4 more); split 240 holdout / 240 calibration / 1,920 train via `InstanceAtomic` |
| `intervention_class` axis | 8 mutation operators (`known_good`, `subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error`) |
| `subdomain` axis | Programming language (currently Python only; other languages deferred to post-ship) |
| `τ_corr(D)` | **0.95** |
| `k_windows` | **4** |
| `ε_drift` | **0.05** |
| `δ_repeat` | **0.05** |
| `ε_unrelated` | **0.02** |
| `τ_MI` | **0.95 bits** (#734, not yet measured) |
| `τ_sc` | **0.97** (#723, not yet measured) |
| `N_min` | **50** |
| `Panel_B` | tree-sitter → CodeBERT, e5-large → bge-large-en-v1.5, Qodo-Embed-1 → Voyage-code-2, attention CFG → Pythia program-graph, def-use → joern, tree-sitter type → pytype JSON (deterministic slots shared) |
| Durable root | `/zfs/archive/contextgraph/...` (aiwonder ZFS) |

**Current measurement (2026-05-24).** Tier-1 #1 raw Pearson `0.4843192995` on holdout — gap to ship `0.4657`. Tier-2 Brier `0.0924`, ECE `0.0240`. Tier-1 #2/#3/#4 blocked behind Panel B encoder training (#405). Tier-5 #734 and #723 cheap probes not yet executed. Latest ship-gate FSV: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`.

**Activation event for Python.** When the §4.1 banner emits TRUE, the 8 consequences in §4.2 execute: human review of AI-generated Python patches becomes statistically unjustified, and the same predicate template begins instantiation for the next domain.

---

## 6. Worked Instantiation Sketches — Other Eligible Domains

These are sketch-level illustrations of how the universal formula applies elsewhere. Each domain would require its own full instantiation document, corpus build, sensor panel, and FSV protocol before the predicate could be claimed.

### 6.1 Medical pathology — binary malignancy classification

| Symbol | Sketch |
|---|---|
| `D` | Histopathology slide malignancy classification (per tissue type) |
| `Artifact_D` | Whole-slide image |
| `Oracle_D` | Blind 3-pathologist majority vote + biopsy follow-up where available |
| `Panel_D` | per-tile CNN features, color-deconvolution channels, nuclei-morphometry vector, spatial-statistics vector, stain-quality vector, … each in its own sensor space |
| `Corpus_D` | TCGA / CAMELYON / curated tissue-type matrix |
| `intervention_class × subdomain` | (tissue type) × (stain protocol / scanner vendor) |
| Notes | E1 satisfied via biopsy follow-up subset. E5 oracle latency is biopsy turnaround — economically feasible at training scale. `τ_sc` bounded by inter-pathologist agreement (~0.85–0.90 typical) — `τ_corr(D)` set to that ceiling minus ε, with explicit carve-out. |

### 6.2 Theorem proving — proof correctness verification

| Symbol | Sketch |
|---|---|
| `D` | Mathematical-theorem proof correctness in a decidable fragment (Lean / Coq / Isabelle) |
| `Artifact_D` | Candidate proof term |
| `Oracle_D` | Proof checker (deterministic, fast, near-perfect self-consistency) |
| `Panel_D` | proof-term AST embedding, tactic-sequence embedding, lemma-dependency graph embedding, type-signature embedding, definitional-unfolding embedding, …  |
| `Corpus_D` | mathlib / coq-contribs / archive of formal proofs |
| `intervention_class × subdomain` | (proof-class: induction / case-analysis / contradiction / construction / rewrite-only) × (mathematics subfield) |
| Notes | All 5 prerequisites trivially satisfied. `τ_sc ≈ 1.0`. Highest-fidelity domain for this framework. |

### 6.3 Drug-target binding affinity — binary "binds at threshold X" classification

| Symbol | Sketch |
|---|---|
| `D` | Drug-target binding (Kd ≤ X nM) for given protein family |
| `Artifact_D` | Drug-protein pair (SMILES + protein structure) |
| `Oracle_D` | Wet-lab assay (latency = days–weeks; proxy oracle = high-throughput screening with documented drift bound) |
| `Panel_D` | molecular-graph embedding, 3D-shape embedding, electrostatic-surface embedding, protein-binding-pocket embedding, sequence-similarity embedding, … |
| `Corpus_D` | ChEMBL / BindingDB / DUD-E |
| `intervention_class × subdomain` | (chemotype scaffold class) × (protein family) |
| Notes | E5 is the binding constraint — wet-lab is slow. Proxy oracle (HTS) is acceptable if `oracle_proxy_drift` is bounded and the carve-out is documented. |

### 6.4 Weather nowcasting — binary precipitation-in-next-hour

| Symbol | Sketch |
|---|---|
| `D` | "Precipitation > X mm/hr in cell (lat, lon) within next 1 hour" |
| `Artifact_D` | Spatio-temporal forecast claim |
| `Oracle_D` | Observed rain-gauge / radar reading at lead time |
| `Panel_D` | radar reflectivity, satellite IR, surface obs, NWP background, orography, climatology, … |
| `Corpus_D` | historical observation archive |
| `intervention_class × subdomain` | (regime: convective / stratiform / orographic / frontal) × (geographic region) |
| Notes | `oracle_self_consistency` bounded by gauge measurement noise. Long-tail regimes (e.g., flash-flood convective) need explicit per-cell support. |

### 6.5 Chess / Go / Poker — binary "this move is engine-best at depth N"

| Symbol | Sketch |
|---|---|
| `D` | "Candidate move equals engine-best move at depth N" |
| `Artifact_D` | (position, candidate move) pair |
| `Oracle_D` | Strong engine search at depth N (deterministic; near-perfect self-consistency) |
| `Panel_D` | board-state embedding, attack-defense map, material/positional features, opening-book lookup, endgame-tablebase lookup, … |
| `Corpus_D` | historical positions stratified by phase + material balance |
| Notes | Trivially eligible; `τ_sc` engine-determined. This domain is already nominally super-human; the framework here would discipline the verifier's calibration. |

---

## 7. Permanently Out of Scope

These categories cannot be benchmarked by the predicate. Claiming super-intelligence in them via this framework would be doctrine breach.

| Category | Reason | Failed prerequisite |
|---|---|---|
| **Aesthetic / taste judgments** ("is this beautiful / idiomatic / well-styled") | No deterministic oracle; human-judgment variance is not an oracle | E1 |
| **Continuous-quantity questions with no natural threshold** ("is this fast enough / cheap enough / accurate enough") | Threshold is arbitrary; converting to binary is heuristic, not reality | E2 |
| **Hidden-intent inference** ("does this match the author's mental model") | No observable ground truth | E1 |
| **Open-ended creativity / novelty** ("is this idea original / interesting") | Subjective; no oracle | E1 |
| **Welfare / policy outcomes** ("does this policy improve welfare") | Value-laden, multi-causal, unobservable counterfactual | E1 |
| **General intelligence** ("is this AI generally intelligent") | Not a single question, not binary, no single oracle | E1, E2, E3 simultaneously |
| **Consciousness / sentience claims** | No falsifiable measurement protocol | E1 |
| **Long-horizon high-confounder outcomes with no proxy oracle** ("will this company succeed in 10 years") | Oracle latency unbounded; proxy drift unbounded | E5 |
| **Code beyond test-suite scope** (architecture, security policy beyond tests, business-logic intent in a code domain) | Oracle covers only what the test suite covers; rest is out of `D` | Out of `D` (not a prerequisite failure, just a scoping bound) |

The framework deliberately does **not** attempt to answer these. Doing so would replace a falsifiable predicate with a heuristic, which collapses the entire epistemological guarantee.

---

## 8. The 5-Question Worthlessness Test (Universal)

Before filing any new issue, writing any new code, training any new sensor, or proposing any new architectural change *in any domain*, answer:

1. **Which Tier-1/2/3/5/6 number in which domain does this move?** Be specific. "Improves the predictor" is not an answer. "Increases per-cell Pearson on `<intervention_class>::<subdomain>` from `<x>` to `<y>` in domain `D`" is an answer.
2. **By how much?** If expected delta < 0.005 on any target and work cost > 1 day, defer.
3. **What's the falsification condition?** Under what measured outcome is this work classified as failed and rolled back?
4. **What's the doctrine cost?** Does this preserve every Tier-4 invariant? If not, it does not ship.
5. **What's the production execution path?** Specify the durable-root path, SHA-pinning, and manual SoT readback protocol.

If the work cannot answer (1) cleanly with a specific number it moves, **it is worthless by definition** and must be reconsidered or rejected. Activity is not the goal. Movement of the numbers in this document is the goal — in some domain, against some oracle, with every invariant intact.

---

## 9. The Universal Definition of Super-Intelligence Used in This Project

> **Super-intelligence in domain `D` is the existence of a deterministic predictor `Predictor_D` over a slot-identity-preserved observation panel of `D`, whose binary verdicts correlate with the deterministic oracle of `D` above the cost-asymmetry floor (bounded by oracle self-consistency), stably across rolling windows, with cross-panel encoder triangulation confirming the predictor learned about `D` rather than about Panel A, with per-cell quality holding on every mathematically valid cell, with online mistake-loop convergence proving adaptive correction without unrelated-state perturbation, with every architectural-invariant fence intact at every moment, and with substrate-sufficiency probes (panel MI + oracle self-consistency + sensor distinctness) confirming the predicate is reachable on the current state.**

This definition is:

- **Domain-scoped.** It says nothing about general intelligence. It says everything about replacing human review of `D` artifacts at superhuman accuracy and machine-scale throughput.
- **Falsifiable.** Every term is measurable. Failure of any term blocks the claim.
- **Oracle-bounded.** The ceiling is the oracle's self-consistency. We do not claim to exceed reality; we claim to match it.
- **Invariant-protected.** A pass that breaches Tier 4 is not a pass. Goodhart is structurally blocked.
- **Adaptive.** The system that achieves it continues to adapt to its own mistakes after deployment — Tier 3 is not satisfied at one moment, it is satisfied as a continuous property.

Anything that does not match this definition, in this project, is either a different goal or a confused goal. The first instantiation is ME-JEPA-Code on Python; the same predicate is the template for every subsequent domain.

---

## 10. Authority and Cross-References

| Topic | Canonical doc |
|---|---|
| Universal predicate (this file) | `docs/BENCHMARKS_DEFINITION_AND_CONSEQUENCES.md` |
| Python instantiation rationale | [`docs/futurebuild/specs/NORTH-STAR.md`](futurebuild/specs/NORTH-STAR.md) |
| Python instantiation numbers | [`docs/futurebuild/specs/TARGET-METRICS.md`](futurebuild/specs/TARGET-METRICS.md) |
| Verification protocol (FSV) | [`docs/futurebuild/specs/FSV-PROTOCOL.md`](futurebuild/specs/FSV-PROTOCOL.md) |
| Project doctrine + Tier-4 invariants | [`CLAUDE.md`](../CLAUDE.md) |
| Universal protocol | [`docs2/AICodingAgentSuperPrompt.md`](../docs2/AICodingAgentSuperPrompt.md) |
| Python instantiation overview | [`docs/futurebuild/01_overview.md`](futurebuild/01_overview.md) |
| Predictor pipeline (Python) | [`docs/futurebuild/05_predictor_pipeline.md`](futurebuild/05_predictor_pipeline.md) |
| Current measurement history (Python) | [`docs/futurebuild/07_results_and_benchmarks.md`](futurebuild/07_results_and_benchmarks.md) |
| Open work + critical path (Python) | [`docs/futurebuild/08_roadmap_and_open_work.md`](futurebuild/08_roadmap_and_open_work.md) |

---

**End state per domain `D`.** When the universal formula in §0 evaluates `TRUE` for domain `D` with `D`'s instantiated thresholds (§3), the predictor replaces human review of artifacts in `D`, the activation moment in §4.1 fires, the 8 consequences in §4.2 execute, and the same predicate template begins instantiation for the next eligible domain. Until that moment in any domain, every action either moves a number in that domain, preserves a Tier-4 invariant (which is universal), or should be reconsidered.
