# The Universal Benchmark Predicate — Falsifiable Domain Super-Intelligence

> **One-sentence purpose.** This is the complete, self-contained domain-agnostic formula for what counts as super-intelligence in any domain: the 5 eligibility prerequisites a domain must satisfy, the 6-tier benchmark stack that has to be hit, what each tier means against physical reality, what changes in the world once all benchmarks hit simultaneously in a domain, the verification protocol that proves it, and the predictor architecture that achieves it. ME-JEPA-Code on Python is the first instantiation; the same predicate is the template for every subsequent domain.

This document depends on no other file. Every term, every threshold, every protocol, every architectural decision needed to evaluate the predicate or build a verifier against it is contained here.

**Last updated:** 2026-06-04.

---

## Part I — Why This Exists

### The plain-language statement

A **binary reality predictor** is a deterministic function that, for any candidate artifact in a domain, returns `Pass` or `Fail` against the ground-truth oracle of that domain — and can be trusted to do so without human review. When such a predictor exists for a domain, and exhibits the properties defined formally below in Parts II–VI, the operational consequence is unambiguous: **human review of AI-generated artifacts in that domain becomes statistically unjustified**, the verification bottleneck disappears, and the throughput of any agent operating in that domain becomes bounded only by the oracle's wall-clock latency, not by reviewer minutes-per-artifact.

This is what "super-intelligence in domain `D`" means in this document. It is not a claim about general intelligence, sentience, or any property that lacks a deterministic oracle. It is a falsifiable, measurable, oracle-bounded property of a specific predictor against a specific domain.

### The bottleneck this exists to dissolve

AI coding agents (Claude Code, Codex, Cursor, etc.) currently produce code at a rate humans cannot review at the same rate. The bottleneck is verification, not generation. A single agent session can produce hundreds of edits per hour; a human reviewer reads at single-edit-per-minute pace on a good day. The asymmetry compounds: the more capable the agent, the wider the verification gap.

This is solvable if and only if a **deterministic verifier exists that correlates ≥ τ_corr with the oracle on a representative corpus**, with calibrated per-cell confidence, Goodhart defenses, and online mistake-driven adaptation. Such a verifier replaces human review at the statistical level: the verifier's `Pass` carries the same information content as a human's `Pass`, the verifier's `Fail` carries the same information content as a human's `Fail`, and disagreements between the verifier and reality (when they happen) are captured as mistakes and feed back into the verifier's weights without human supervision.

The same logic applies to any domain with a deterministic oracle: medical pathology (biopsy follow-up), theorem proving (proof checker), drug binding (wet-lab assay), weather nowcasting (observation archive), chess (engine search). The formula in Part II is domain-agnostic; the operational consequence is identical across domains.

### Why this is super-intelligence-shaped (and what it is not)

Super-intelligence in this framing is **not** a model that writes better code (or solves harder theorems, or designs better drugs). Super-intelligence is a system that:

1. **Predicts reality outcomes from observable inputs.** When the inputs and outputs are well-defined, this is a tractable engineering target.
2. **Closes the loop on its own mistakes.** Disagreement with reality materializes as a training signal without human supervision.
3. **Hits a hard numerical floor that defines functional super-intelligence in its domain.** When the floor is hit, the system performs beyond average human expertise at the task it was designed for.

For ME-JEPA-Code on Python, the domain-task is binary Pass/Fail prediction on Python code changes against the Docker oracle. The hard numerical floor is the predicate in Part II below. Once that floor is hit, the system functions at super-human level in this exact domain — not because it understands Python more than any individual human, but because it predicts the Docker oracle outcome more accurately than any human reviewer empirically does on the same corpus.

This is super-intelligence in the **operational** sense: the system replaces a human function (code review) at higher accuracy and at machine-scale throughput. The fact that it does so by predicting one specific number does not make it less of a super-intelligence; it makes it a **falsifiable** super-intelligence, which is the only kind that can be engineered toward.

---

## Part II — The Universal Formula (The Entire Goal, In One Block)

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

`τ_corr(D)` is set by the cost asymmetry between false-positive Pass and false-negative Pass in domain `D`, **bounded above by `oracle_self_consistency`** — you cannot correlate with an oracle better than the oracle correlates with itself. For ME-JEPA-Code on Python: `τ_corr = 0.95`, `k_windows = 4`, `ε_drift = 0.05`, `δ_repeat = 0.05`, `ε_unrelated = 0.02`, `τ_MI = 0.95 bits`, `τ_sc = 0.97`.

When this predicate evaluates `TRUE` for domain `D`, the consequence is unambiguous and stated in Part VII: **human review of AI-generated artifacts in domain `D` becomes statistically unjustified.** That is the falsifiable definition of operational super-intelligence in `D`. There is no other goal in `D`.

---

## Part III — The 5 Eligibility Prerequisites (Before Benchmarks Even Apply)

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
| **E2 fails** (no binary form) | "Is this code performant *enough*?" | Frozen as an ambiguous-head class. Not a benchmark. |
| **E3 fails** (corpus uncoverable) | "Predict all possible future pandemics" | Restrict to a coverable sub-domain or abandon. |
| **E4 fails** (signals not slot-decomposable) | Pure single-scalar oracles with no observable substrate | Add instrumentation or reframe the artifact. |
| **E5 fails** (oracle too slow) | "Does this drug extend lifespan in humans?" (oracle latency = decades) | Use proxy oracle (e.g., model organisms) with explicit `oracle_proxy_drift` bound, or abandon. |

The 5 prerequisites are gates, not levers. If a domain fails them, the response is not "lower the bar" — it is "this domain is not eligible for the predicate."

---

## Part IV — The 6 Tiers (Domain-Agnostic)

| Tier | Name | Role | What "hit" means in domain `D` |
|---|---|---|---|
| **1** | **Ship-gate numbers** | The 4 numbers whose simultaneous truth ships `D` | Correlation + stability + cross-panel + drift in `D` |
| **2** | **Per-cell quality contract** | Per `(intervention_class × subdomain)` quality floor in `D` | Every cell in `D`'s matrix passes its appropriate metric |
| **3** | **Online mistake-loop convergence** | Proof the system gets better when wrong in `D` | Repeat-rate + control delta in `D` |
| **4** | **Doctrine invariants** | Fences, not goals — exact values at all times, **identical across all domains** | Exact equality, monitored continuously |
| **5** | **Substrate quality** | Proves Tiers 1–3 are *reachable* in `D` on current state | Audit measurements pass cheap probes |
| **6** | **Sensor/embedder gating** | Quality filter for any new/replacement sensor in `D` | Novelty + utility thresholds passed |

Tier 4 is invariant across all domains. Tiers 1, 2, 3, 5, 6 instantiate per domain with `D`-specific thresholds bounded by oracle self-consistency. Tier 4 supersedes all others: a Tier-1 pass that breaches a Tier-4 invariant is doctrine breach, not a pass.

### Tier 1 — The 4 Ship-Gate Numbers

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

### Tier 2 — Per-Cell Quality Contract

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

### Tier 3 — Online Mistake-Loop Convergence

Tiers 1 and 2 measure prediction accuracy at one point in time. Tier 3 proves the system **adapts** when it is wrong, without human supervision and without breaking unrelated state.

| Variable | Target | What it actually means |
|---|---|---|
| `mistake_repeat_rate` (rolling `N_mistake` window) | **≤ δ_repeat** | If the predictor gets the same `(cell × pattern)` wrong once, it gets it wrong at most `δ_repeat` of the time on next presentation. Computed from the durable mistake log. |
| `unrelated_cell_accuracy_delta` (control panel before/after mistake update) | **≤ ε_unrelated** | Updating the predictor on mistake `X` does not damage accuracy on unrelated cell `Y`. Protects against catastrophic forgetting. |
| `claims_strong_protection` (e.g., Fisher/EWC) | **`false` until backed by durable evidence** | Honesty bit — until durable protection state exists, the system must not advertise strong continual-learning guarantees. |

**Why this matters beyond accuracy.** Best-of-K inference papers over an under-trained predictor by spending compute at inference time. The mistake loop pays the cost at *learning* time and converges to a predictor that is right on first try. "If it needs 4 guesses to be most correct, it is already wrong." A predictor that does not close the loop on its own mistakes is not super-intelligent — it is merely accurate at a frozen point in time.

### Tier 4 — Doctrine Invariants (Universal — Identical Across All Domains)

These do not move. They equal the values below **exactly, at every moment, including during tests and in dev branches**, in every domain `D`. A Tier-1 pass that breaches a Tier-4 invariant is doctrine breach, not a pass.

| Variable | Required value | Why it cannot move |
|---|---|---|
| Frozen-target gradient leak (`ΔΞ.target_collapse`) | **exactly `0`** | Target-side instruments must carry zero trainable params; non-zero ⇒ instrument gradient leak. Enforced structurally via empty `parameters()` on the target adapter. |
| Ambiguous-head loss coefficient (`Q4_LOSS_COEFFICIENT_MAX`) | **exactly `0.0`** | Ambiguous heads (continuous heuristics without binary form against reality — perf, accuracy, cost, taste judgments, reasoning class) have zero loss contribution. Frozen by doctrine; they have no free binary form against reality. |
| Ambiguous-head inputs to ship-gate (`q4_inputs_to_ship_gate()`) | **exactly `0`** | Ambiguous surfaces must not feed the ship-gate metric. |
| Ambiguous-head row delta over time (weekly) | **exactly `0`** | No new ambiguous-head producer writes after freeze. |
| Flat-vector concat over panel (`flat_vector_concat_used`) | **exactly `false`** | Predictor never operates over the byte-layout buffer as a flat vector. Panel is always an *array of per-sensor vectors*. The contiguous byte layout (e.g. 5,120-f32 for the Python panel) is transport-only; no single linear/conv/attention layer ever operates over it as a flat tensor. |
| Slot identity preserved (`slot_identity_preserved`) | **exactly `true`** | Same-sensor dim compare only. Cross-sensor dim compare is forbidden. You may compare two E_AST vectors, never an E_AST dim with an E_Trace dim. |
| Target labels used as live inputs (`target_labels_used_as_live_inputs`) | **exactly `false`** | Leakage policy — oracle / oracle-derived labels are target-side only, never live predictor inputs. |
| Cross-panel Goodhart detected (when `corr_A ≥ τ_corr AND corr_B < τ_corr − 0.10`) | fail-closed → block promotion | Panel A pass without Panel B agreement is treated as memorization. |
| Panel B resident during user-facing inference (`panel_b_resident_during_normal_inference`) | **exactly `false`** | Panel B is loaded only during eval worker, never during serving (cost + isolation). |
| Inner-generator ban | **compile-error guard active** | The system is a predictor, not a generator. Any code path that would reintroduce a generative LLM inside the predictor's serving loop must fail to compile. (For ME-JEPA-Code: a `feature = "llm"` cargo feature triggers `compile_error!` at the MCP server boundary.) |
| Durable storage root invariant | **domain-pinned production path** | All production durable state lives on the pinned root. Forbidden roots blocked by CI lint. |

**Why invariants are sacred.** Numbers (Tiers 1/2/3/5/6) can be Goodhart'd — optimizing the proxy until the underlying property is lost. Invariants cannot — they either hold or they don't. Reaching the targets via invariant violation is **not** reaching them; it is cheating with a deployment-blocking failure mode.

### Tier 5 — Substrate Quality (Are Tiers 1–3 Reachable?)

Tier 5 measurements are the cheapest highest-leverage tests. If they fail, **no amount of model work moves Tier 1** — the system has the wrong substrate and must change state before more model work.

| Variable | Target | What it actually means |
|---|---|---|
| `I(Panel_D; Oracle_D)` (panel-sufficiency MI) | **≥ τ_MI bits** | Probability-theoretic upper bound on what *any* predictor reading this panel can know about the oracle outcome. If less than `τ_MI`, the panel literally cannot determine the verdict, and adding training data is wasted. |
| `oracle_self_consistency(D)` (n-replica per-artifact) — **flakiness axis** | **≥ τ_sc** | Run the oracle n× on the same artifact; how often does it agree with *itself*? Bounds reproducibility only. If `< τ_corr`, the threshold is unreachable as stated. |
| `oracle_validity(D)` (all-tests re-validation + weak-test/leak audit) — **validity axis** | **≥ τ_sc** | Does the verdict track *real correctness*? Self-consistency can be 1.0 while validity is low (weak tests pass wrong artifacts; solution leakage). A panel matched to an invalid oracle fits noise — silent catastrophic failure. Sets the true ceiling `τ_corr = min(τ_corr, oracle_validity)`. |
| `association_sufficiency` (k-NN verdict locality / MinSet fraction) | high locality / small MinSet | `I(panel;oracle) ≥ τ_MI` guarantees the kernel *exists* but not that it is *small*. Bits can be present yet scrambled (MinSet → 100%). Same-verdict artifacts must be co-located so the constellation edge derives one from another. The **second** substrate gate, beyond raw MI. |
| `inter_sensor_distinctness` gate | passes for all active slots | No two sensors alias each other's vector space. Aliased spaces produce a sensor-identity collapse and silent failure. |
| Per-sensor pretraining-corpus overlap with eval corpus | documented; counterfactual cutoff experiment if overlap > 30% | Pretraining-corpus Goodhart/contamination audit — were the test artifacts in the sensor's training data? |

**State-change levers if Tier 5 fails:** add/replace sensors (selected by *marginal* ΔI, **not** standalone bits — mutual information is not additive: prune redundant sensors, keep synergistic ones), expand corpus, refine the oracle harness (replica voting, flakiness modeling), **audit oracle validity** (all-tests re-validation, weak-test/leak detection) and revise `τ_corr` to the valid ceiling, or improve panel *geometry* for association-sufficiency. **Lowering `τ_corr` arbitrarily because the current state cannot reach it is forbidden** — that is doctrine breach, not progress.

### Tier 6 — Sensor/Embedder Gating

Tier 6 is the quality filter for *any* sensor addition or replacement. It prevents bloat and Goodhart on sensor proliferation.

| Variable | Target | What it actually means |
|---|---|---|
| `τ_novelty` (max-correlation of new sensor vs existing slots) | **< 0.7** for promotion | New sensor must add information distinct from existing slots. Above 0.7 is redundancy. |
| `δ_utility` (per-cell oracle correlation lift) | **≥ 0.005** for promotion | New sensor must improve per-cell oracle correlation by at least 0.005. Below this is noise. |
| `holdout_corr_delta_when_dropped` (slot ablation) | **< 0.001** + variance ≈ 0 → replace slot; **≥ 0.005** → keep slot | Ablation test: drop one slot, measure holdout correlation. Low-impact slots become replacement candidates. |
| Domain-specific directionality / structural tests | per domain | E.g., asymmetric retrieval Spearman ≤ 0.5 for causal sensors; positional invariance for spatial sensors; etc. |

---

## Part V — Why These Specific Thresholds (Not 0.90, Not 0.99)

The threshold values in Part II derive from empirical sufficiency, not preference:

- **`τ_corr ≈ 0.95` (default for code verification).** This means the predictor disagrees with the oracle on at most ~5% of holdout instances. At this threshold, the expected cost of false-positive Pass (predictor says Pass, oracle says Fail) is bounded by the cost of human-review-Pass under realistic operator throughput assumptions. Below `0.95`, false-positive rate climbs faster than the human-review-of-uncertain-cases bandwidth can absorb. Above `0.97`, marginal cost climbs faster than the marginal benefit because oracle self-consistency itself may cap around `0.97–0.98` for typical CI/Docker test harnesses.

- **`k_windows = 4` consecutive windows.** This is the smallest interval at which the time-series statistic `corr ≥ τ_corr` cannot be a flake under realistic distribution shift between calibration and serving. Fewer windows → drift-vulnerable. More windows → diminishing returns.

- **`ε_drift = 0.05` Panel B agreement.** Means the predictor cannot have memorized Panel A. If two independently-encoded panels reach the same Pass/Fail verdict within 5%, the predictor learned about the domain, not about the panel encoders.

- **`τ_brier = 0.05`, `τ_ece = 0.05`.** Standard calibration cutoffs below which probabilistic outputs are "well-calibrated" in the strict-proper-scoring-rule sense. Calibration matters because the predictor's `Pass with 0.93 confidence` must mean 93% of those cases are actually Pass in reality.

- **`δ_repeat = 0.05` mistake-repeat rate.** After observing a mistake on `(cell, pattern)`, the next presentation of the same `(cell, pattern)` should produce the correct verdict at least 95% of the time. Otherwise the loop is not actually closing.

- **`ε_unrelated = 0.02` unrelated-cell perturbation.** When the predictor learns from a mistake, accuracy on unrelated cells should not drop by more than 2%. Larger drops indicate catastrophic forgetting.

- **`τ_MI = 0.95 bits` panel-sufficiency MI.** The mutual information between the slot-decomposable panel and the oracle outcome must exceed 0.95 bits — i.e., the panel carries enough information that *some* predictor reading it could in principle determine the oracle outcome at ≥ 0.95 accuracy.

- **`τ_sc = 0.97` oracle self-consistency.** The oracle must agree with itself 97% of the time when run on the same artifact twice. Below this, `τ_corr = 0.95` is mathematically unreachable (you cannot correlate with the oracle better than the oracle correlates with itself).

None of these are operator preference. They derive from the cost asymmetry between false-positive Pass (production regression) and false-negative Pass (wasted review of working patches), bounded by the oracle's intrinsic noise floor. For a different domain with different cost asymmetry (e.g., medical diagnosis with much higher false-positive cost), `τ_corr` would be set higher (perhaps 0.99) with correspondingly higher `τ_sc` required.

---

## Part VI — State-Change Levers Per Tier (The State Is Not The Constraint)

**The current state of any project is the starting point, not the goal and not the constraint.** If reaching the numerical targets requires expanding the test corpus, adding instrument slots, scaling the substrate, refining the oracle harness, or any other change to current state — that is the acceptable path. Only Tier-4 doctrine invariants are immovable.

### What CAN change to reach the targets

| Lever | Examples | Tier(s) it addresses |
|---|---|---|
| **Corpus expansion** | Code: 300×8 SWE-bench Lite → SWE-bench Verified (~500) → SWE-bench Full (2,294) → CommitPack (4TB) → BugsInPy. Medical: TCGA expansion → CAMELYON → tissue-type matrix expansion. Theorem proving: mathlib → coq-contribs → multi-system corpus. | Tier-1 statistical power; Tier-2 per-cell support |
| **Panel expansion** | Add instrument slots; in-slot encoder replacement; new domain-specific slots | Tier-5 panel sufficiency; Tier-1 ceiling |
| **Architectural refinement** | Iterative predictor head; multi-chunk aggregation; causal embedders; pathway exploration | Tier-1 ceiling; Tier-2 calibration |
| **Calibration refinement** | Per-slot OOD; conformal recalibration; isotonic/Platt per-cell | Tier-2; Tier-3 |
| **Oracle refinement** | Replica-mode voting; per-instance flakiness modeling; per-test isolation | Tier-1 ceiling against oracle self-consistency floor |
| **Threshold revision** | `τ_corr` raised (e.g., 0.95 → 0.97) if oracle self-consistency proves higher; or admit irreducible-noise carve-out for flaky cells | Tier-1 if substrate measurement justifies |
| **Substrate refinement** | Incremental forward cache; v2 cache rebuild; production-host migration | Tier-5 substrate quality |

### What CANNOT change (Tier-4 doctrine invariants — fences, not levers)

- Slot identity preservation
- Anti-compensation invariant
- Binary contract (Pass/Fail only; ambiguous heads frozen)
- No inner generator (compile guard)
- Domain-pinned durable root
- Slot-preserving architecture (no flat-vector concat)
- Leakage policy (target labels not as live inputs)
- ΔΞ.target_collapse = exactly 0

These are not levers because changing them would invalidate the entire architectural claim. Reaching the targets via Tier-4 violation = the system has not worked; it has cheated.

### Specific corpus-scale options (Tier-1 / Tier-2 lever, with concrete code-verification numbers)

If statistical power on the current corpus is the bottleneck (Pearson CI width too loose, per-cell support insufficient, or distribution coverage too narrow), these are the known corpus-scale levers in order of cost-effectiveness for the ME-JEPA-Code Python case:

| Corpus | Instances | Rows (× 8 mutations) | Holdout (~10%) | Pearson 95% CI at r=0.95 | Cost estimate |
|---|---|---|---|---|---|
| **Current: SWE-bench Lite** | 300 | 2,400 | 240 | ±0.062 | shipped baseline |
| **+ SWE-bench Verified** | +500 | +4,000 | +400 | ±0.036 (combined ~640) | ~3–5 days compute |
| **+ SWE-bench Full** | +2,294 | +18,352 | +1,835 | ±0.014 (combined ~2,075) | ~10–14 days compute |
| **+ BugsInPy** | +493 | +3,944 | +394 | covers real-Python bugs (different oracle contract) | ~3–5 days + oracle-adapter work |
| **+ TSSB-3M** | +3M single-statement bugs | +24M | impractical for full holdout | training corpus only | weeks; training only |
| **+ CommitPack** | 4TB of paired state | ~742M pairs | training corpus only | training corpus only | months; training only |

**Minimum corpus expansion for statistically-tight Pearson CI at the 0.95 target:**

- For `±0.01` confidence interval at r=0.95: ~1,100 holdout rows needed → 4-5× current corpus (Lite + Verified + Full).
- For `±0.025` confidence interval (operational tolerance): ~440 holdout rows → 2× corpus (Lite + Verified).
- For `±0.05` confidence interval (loose but defensible): current 240 holdout sufficient.

**Decision rule for corpus expansion:** if 4-window stability fails because per-window variance exceeds `0.03` at r ≥ 0.95, expand to the next corpus tier. If per-cell support drops below 50 in mixed cells, expand to Full.

### State-change is not threshold-change

Lowering `τ_corr` from `0.95` to `0.85` because the current corpus can't support `0.95` is **not** a state-change — it's a doctrine change, and a regression. The correct response to a corpus limitation is **corpus expansion**, not threshold relaxation. The only legitimate threshold revision is **upward** (e.g., `0.95 → 0.97` if oracle self-consistency measurement justifies it).

---

## Part VII — What "All Benchmarks Hit in Domain `D`" Means

This is the actual answer to "what does success look like." The events are sequential and binding.

### The activation moment

```
=== Ship-Gate Final-State Verification — All Targets Met (Domain: D) ===

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

### The 8 sequential consequences

1. **Ship-gate status reports `green`** per-cell across all `(intervention_class × subdomain)` cells in `D`.
2. **Promotion approval engages** operator first-promotion review — the one human-in-the-loop step left. Per-tier oversight: any first-time promotion of a verifier checkpoint requires operator approval; subsequent promotions on the same model line can be auto-promoted once mistake-loop convergence is sustained.
3. **After operator approval, the active-pointer flips** and the trained checkpoint becomes the serving predictor in `D`.
4. **Prediction surface begins returning `Verdict::Pass | Fail`** with calibrated confidence on every artifact in `D`. Named failure mode on `Fail`, closest exemplars on both. Latency bounded by the predictor's forward pass (typically milliseconds on a single GPU).
5. **Capture hooks record every reality-outcome event** into the verification log. Any disagreement between predictor and oracle still materializes as a mistake row — the loop never sleeps.
6. **The mistake loop continues consuming** new mistake rows. The predictor's online head corrects same-context future predictions. Convergence target `mistake_repeat_rate ≤ δ_repeat` is monitored against the rolling window.
7. **Per-cell drift guardrails engage.** Any cell whose correlation drops below `τ_corr − 0.02` triggers retraining; below `τ_corr − 0.05` triggers automatic rollback to the previous active pointer.
8. **The post-ship creativity engine begins** in `D`: novelty oracle, mistake-driven sensor demand, retirement audit, domain-routed sparse panel, episodic-to-semantic consolidation. Autonomous bounded growth into the corpus's intrinsic dimensionality.

### What changes for users of the system in domain `D`

| Before | After |
|---|---|
| Human reviews every artifact in `D` | Predictor returns `Pass/Fail` in milliseconds; human reviews only `Fail` + cold-cell `Abstain` cases |
| Production throughput bottlenecked by reviewer minutes-per-artifact | Production throughput bottlenecked by oracle wall-clock |
| Wrong artifact ⇒ caught in human review (or later in production) | Wrong artifact ⇒ flagged with named failure mode + closest exemplar before commit |
| No falsifiable definition of "is this AI agent reliable in `D`" | Agent reliability in `D` = `prediction_oracle_correlation` against the predictor — a number on a dashboard |
| Verification cost grows linearly with agent throughput | Verification cost is per-artifact constant (predictor latency) |
| Manual triage of which change broke what | Predictor's mistake log directly identifies the cell + named failure mode |

### What does NOT change

- **The product is still binary in `D`.** Pass/Fail is the surface. Explanation surfaces (named failure mode, closest exemplar) are labels on Pass/Fail, not separate predictions. Ambiguous heads remain out of scope.
- **Tier-4 invariants are still continuously enforced.** Slot identity, no flat-vector concat, no inner generator, durable storage root — none of these unlock or relax post-ship. They tighten.
- **Out-of-`D` review is unchanged.** The predictor predicts `Oracle_D` outcome, not human-judgment outcome on properties outside `D`'s oracle (e.g., for a code domain, this excludes architecture, security policy beyond the test suite, business-logic intent, taste).
- **Tier 4 is identical across all domains.** Shipping `D` does not relax invariants for `D'`.

---

## Part VIII — What This Framework Is NOT

- **Not a generative model.** No LLM inside the predictor's serving loop (`feature = "llm"` triggers `compile_error!` at the MCP server boundary). The system predicts whether an externally-generated artifact works; it does not generate the artifact.
- **Not a code reviewer / artifact reviewer in the human-judgment sense.** It does not assess style, maintainability, idiomaticity, taste, or "is this a good thing." Those are ambiguous-head questions and explicitly out of scope.
- **Not a benchmark harness.** The corpus is the training/eval substrate, not the product. The product is the predictor that runs in milliseconds on any candidate artifact.
- **Not a research project.** Every architectural decision is gated by the numerical predicate. If it does not move at least one of the Tier-1/2/3/5/6 numbers, or preserve a Tier-4 invariant, it must justify its existence on those grounds or be rejected.
- **Not multi-domain at ship-gate time.** Each domain ships independently against its own oracle. The first domain (e.g. Python on SWE-bench Lite) ships first; other languages, other domains follow with their own instantiations.

---

## Part IX — Verification Protocol (FSV — Final-State Verification)

This is the binding verification contract for every task that contributes evidence to the predicate. Universal protocol:

> *A return value is a claim. The Source of Truth is the verdict. Read the verdict.*

### The 4 questions

Before declaring any implementation complete, answer all four:

1. **What state changed?** File, database column-family row, metric, queue item, report, or external system record.
2. **Where does that state live?** Exact path, database name and key, metric name, or external identifier.
3. **How is it read back independently?** Separate database open, file read, checksum, source command, or equivalent.
4. **What exact value is expected?** Literal value, schema, count delta, hash, or bounded range.

If any answer is unknown, the feature is not ready to call done.

### Trigger → Process → Outcome

Every FSV-able feature has this shape:

```
[Trigger]   ─►   [Process]   ─►   [Outcome]
 observable      measurable        verifiable at source-of-truth
```

Document these before implementation:
- **Trigger:** API call, CLI command, scheduler tick, hook event, file event, or operator action.
- **Process:** Code path and functions that execute.
- **Outcome:** Physical end state to read back, including exact path, database column, key, and expected value.

### Hard rules

1. No mocks for completion gates.
2. No fallbacks that hide required-output failure.
3. No verifier weakening, ignored tests, or silent bypasses.
4. Tests must fail when the feature is broken.
5. Missing infrastructure (CFs, files, models, env vars, malformed inputs) fail closed with structured `SCREAMING_SNAKE_CASE` errors.
6. Assume the task is wrong until live files, symbols, and infrastructure names prove it is current.
7. Prefer the smallest implementation that satisfies the requirement.

### Synthetic data properties

Synthetic FSV input is allowed when it is deterministic, distinguishable, representative, boundary-rich, privacy-safe, and cleanup-tagged. Mocked subsystems are not acceptable completion evidence.

### Boundary cases (cover at least 3 per code path)

1. Empty or missing input
2. Single item
3. Maximum allowed
4. Maximum plus one
5. Minimum allowed
6. Minimum minus one
7. Wrong type or malformed input
8. Unicode, control-byte, or very long string edges
9. Duplicate or replayed event
10. Out-of-order event
11. Concurrent writers or sessions
12. Authorization or tenant-scope variants
13. Time edges: skew, negative timestamp, DST, leap second, month boundary
14. Resource exhaustion: full disk, OOM, pool exhaustion, rate limit

Each boundary case needs: before state, trigger/action, after state, expected value, actual value, pass/fail.

### Required FSV evidence per task

1. **Build evidence:** compile output without warnings.
2. **Test evidence:** all tests pass, zero failed, zero ignored without justification.
3. **Lint evidence:** static-analysis output clean.
4. **Source-of-truth readback artifact:** production FSV must be written to the domain's pinned durable root, with a pass field, exact source-of-truth path, happy path, boundary cases, and physical artifact hashes or counts.
5. **Independent verification** for column-family / database writes: flush/drop/reopen through the production opener or an equivalent separate handle, then decode the same row back.

Minimum SoT evidence shape:

```json
{
  "task": "TASK-EXAMPLE-NNN",
  "sourceOfTruth": "<CF/key or absolute file path>",
  "trigger": "<exact command or action>",
  "expected": "<expected value>",
  "actual": "<actual value read independently>",
  "cases": [],
  "allPassed": true
}
```

### Failure modes that mean NOT done

Green tests are not sufficient if any of these are true:
- The test checks a constant instead of exercising the implementation.
- The tested branch is inactive.
- A column-family row is written but never flushed or reopened.
- The test uses a different opener or serializer than production.
- Missing values are hidden with defaults.
- Async producer tasks are spawned but not joined before readback.
- Evidence proves existence only, not field-level correctness.

### Evidence that cannot be skipped

The evidence must include:
- A true pass verdict in the artifact (`allPassed` or local equivalent).
- Counts before/after for any column-family the task writes.
- Field-level readback, not merely "row exists".
- Physical file/database artifact proof.
- Boundary-case detail strings naming the invariant tested.
- A clear source-of-truth path that a fresh agent can read without trusting the claim.

Anything less is not FSV; it is a claim waiting to be verified.

---

## Part X — Predictor Architecture (Slot-Preserving Late-Fusion)

The predictor that achieves the Tier-1 numbers has a specific architecture. The architecture is **load-bearing** for the Tier-4 invariants — alternative architectures that flatten the panel into a single vector cannot satisfy the slot-identity invariant.

### Three architectural primitives back the binary contract

1. **Labeling layer.** Raw slot violations + cross-embedder correlations → 4-level hierarchy of named failure modes. Grounded by closest historical exemplar.
2. **Online mistake loop.** Disagreement with reality materializes as a mistake-log row; the trainer consumes through exact-panel online-head state.
3. **Cross-panel triangulation.** Panel B (non-overlapping instruments) re-scores every gate-relevant artifact. The Tier-1 #1 threshold on Panel A counts only when Panel B agrees within `ε_drift`.

### The Panel — array of per-embedder vectors

The Panel is **an array of N per-embedder/per-instrument vectors**, one per slot, each in its own embedder's space. The array IS the constellation, not a fused embedding.

**The locked-in rule:**

1. Each slot's vector lives in its own embedder's space.
2. Same-embedder cosines valid; cross-embedder dim compare forbidden (`MEJEPA_INFER_DIM_MISMATCH` typed error fails the predictor closed).
3. Whole arrays can be compared array-vs-array (per-slot cosine map, strict-conjunction gate, per-pair matrix with diagonal cosines).
4. Cross-slot relationships are **scalar pairwise metrics** (MI, correlation), not vector dim compares.
5. The contiguous byte-layout dimension (e.g., 5,120 f32 for the Python panel) is **byte-layout only**. Forbidden: single linear/conv/attention over packed buffer; flat residual; any operation where a strong slot compensates for a weak slot.

### Predictor pipeline (data flow)

```
[External: operator / AI agent edit + tool calls + test outcomes]
                 │
                 ▼
       Capture hooks (PostToolUse, Stop, observe ticker)
                 │  JSONL
                 ▼
   <durable-root>/runtime/shift-log.jsonl
                 │  (tail)
                 ▼
      Shift subscriber (tokio task tailing shift log)
                 │
                 ▼
     N embedders → M active content slots
                 │
                 ▼
      N frozen instruments → array of N per-embedder vectors (teleological constellation)
        Each slot in its own embedder's space; array IS the constellation,
        NOT a flat fused embedding; byte-layout dim is transport-only.
                 │  Panel builder validates: dim + finite + no double-fill
                 ▼
      Durable panel column-families + watermark
                 │
                 ▼
      Live label compiler → live-input-safe slot/pair/group labels per chunk
      Runtime ability resolver → ability context with all active memberships preserved
                 │
                 ▼
      Predictor.forward(panel, prediction-label-context)
        + per-slot input encoders + late fusion + per-slot output decoders
        + split-conformal interval + OOD gate + teleological-constellation guard
        + cold-cell metric (Abstain if n_supporting < N_min)
        + multi-head contradiction gate
        + constellation intelligence heads
        + matched fingerprint + closest exemplars HNSW
                 │
                 ▼
      Reality prediction (binary verdict + named failure mode + closest exemplars + slot violations)
                 │
                 ▼
      Durable live-predictions column-family
                 │
       ┌─────────┴───────────┐
       ▼                     ▼
   AI agent             Verification capture (PostToolUse hook
   (predictor MCP       → prediction-verifications column-family)
    tool calls)
                              │
                              ▼  Confirmed / Refuted
                       Active learning queue + replay buffer
                              │
                              ▼  Mistake loop
                       Mistake log + online-head state
                              │
                              ▼
                       Trainer (UTML L_step = ΔP·ΔK·ΔΩ·ΔΞ)
                       + EWC++ Fisher + Continual Backprop + Inverse-map head
                              │
                              ▼
                       Train certificates
                              │
                              ▼
                       Heal scheduler (drift, per-cell convergence, promote / rollback)
                              │
                              ▼
                       Model promotions + active pointers
```

### Predictor heads (per Q-question structure)

For the Python instantiation, the predictor exposes a 3-binary + 1-explanation structure:

| Head | What it predicts | Reality that renders verdict |
|---|---|---|
| Q1 (claim-exists) | Did the AI's claim physically land in bytes/AST? | Bytes on disk + AST diff after action |
| **Q2 (oracle-passes)** | **Does the artifact pass the deterministic oracle? (PRIMARY)** | **Docker oracle / proof checker / wet-lab assay / etc.** |
| Q5 (shift-event-occurred) | Per predicted side effect: did the corresponding shift-log event occur? | Durable shift log on production root |
| Q3 (explanation only) | Named failure mode + closest exemplar on Fail | Not a separate prediction — a label on Q2 |

Q4 ambiguous heads (perf, accuracy, cost, taste, reasoning class) are **frozen out** — they have no free binary form against reality.

### Verdict assembly

```
enum Verdict { Pass, Fail, OutOfDistribution, Abstain, GuardRejected }
```

**Product surface:** only `Pass | Fail` ships to the caller. Internal `OutOfDistribution`/`Abstain`/`GuardRejected` are markers that the predictor lacks confidence on `(cell × named_failure_mode)`. They MUST be projected to `Pass | Fail` at the boundary (closest-exemplar majority outcome). The projected verdict is subject to ground-truth verification + mistake-loop update.

### Anti-compensation guard

Corrupted / missing / stale / dimension-shifted slots, contradictory pair relationships, and temporal jumps cannot be accepted through aggregate evidence even when a flat normalized score would pass. The predictor fails closed on any per-slot violation; one bad slot cannot be hidden by aggregate distance.

### Cross-panel triangulation (the Goodhart guard)

Panel A is both training substrate AND the centroid set the guard evaluates against. Encoder-matched → Tier-1 #1 on Panel A alone could mean the predictor learned the panel.

**Panel B instrument map (non-overlapping):** Only learned encoder slots swap between Panel A and Panel B; deterministic slots are shared. Cross-encoder examples:
- AST: tree-sitter (Panel A) → CodeBERT (Panel B)
- Code-semantic: Qodo-Embed (Panel A) → Voyage-code (Panel B)
- Text: e5-large (Panel A) → bge-large (Panel B)

**Triangulation procedure:**

1. Every gate-relevant prediction scored on Panel A AND Panel B.
2. Both go to cross-panel agreement column-family keyed by `(prediction_id, panel_pair_id)`.
3. **Agreement metric:** `cross_panel_oracle_correlation = corr(verdict_a, oracle) AND corr(verdict_b, oracle)` — both must clear `τ_corr(D)`.
4. **Drift metric:** `|corr_a − corr_b| ≤ ε_drift` for ship eligibility.
5. **Fail-closed:** Panel A ≥ τ_corr AND Panel B < (τ_corr − 0.10) → `CROSS_PANEL_GOODHART_DETECTED`.

**Cost.** Panel B requires loading 5 new encoders (for the Python instantiation). VRAM ~12 GB during evaluation only; never resident during inference (Tier-4 invariant). Panel B worker reads live predictions periodically and writes Panel B observations.

### Failure-mode labeling hierarchy (the meaning layer)

4-level hierarchy turns raw numerical evidence into named rules. Level 0 (deterministic compiler) and Level 1/2/3 (hierarchical naming):

| Layer | Source | Output |
|---|---|---|
| Level 0 deterministic compiler | Per-chunk auto-label script (no inner LLM) | Per-chunk labels + accepted label registry + code-state constellations |
| Level 1/2/3 hierarchy | Failure-mode-hierarchy module + skill-corpus materialization | Hierarchy groups + skills + constellations + named predictions |
| Skill substrate | Skill-sequence discovery + chunk-skill membership | Named skills + memberships + reverse rows |
| Runtime resolver | Ability-resolver module | Ability context, ordered higher-ability IDs into prediction-label-context |

**Leakage policy.** Target-side labels (oracle / test outcomes / Docker labels) are marked `leakage_policy: target_side_only`. Supervision / reward inputs only — **never** live predictor inputs.

### Training (UTML — Universal Training Meta-Loss)

`L_step = ΔP × ΔK × ΔΩ × ΔΞ`:

| Term | What it measures |
|---|---|
| `ΔP` | Predictor improvement |
| `ΔK` | Catastrophic-forgetting violation (Fisher diagonal) |
| `ΔΩ` | Plasticity (continual backprop) |
| `ΔΞ` | Structural-collapse safety margin (frozen-target invariant) |

The mistake loop (Tier 3) consumes mistake rows through replay buffer + online-head state with exact-panel-key isolation. Per-fingerprint EWC + continual-backprop preserve per-fingerprint catalog stability.

### Heal scheduler

Reads drift signals, decides:
- **Retrain** — append new training step
- **Promote** — flip active pointer (operator-approval-gated for first promotion + catastrophic class)
- **Rollback** — revert active pointer
- **Pause** — pause predictions
- **Distill** — distill from champion to smaller learner

Per-cell convergence driver tracks per-cell ship-gate status. Mixed oracle cells use `prediction_oracle_correlation`; zero-variance all-pass/all-fail cells use the replacement contract (target-class recall + actual-class confidence + Brier/ECE) because Pearson is undefined there.

### Adversarial defenses

| Defense | Purpose |
|---|---|
| Negative-action ablation falsifier | Confirms the predictor is using real features, not constants |
| Multi-head contradiction | High-confidence Pass + high-severity Fail signal → `MULTI_HEAD_CONTRADICTION` abstain |
| Constellation disagreement pressure | Slot-pair consensus + contradiction + redundancy + novelty + blind-spot detection |
| OOD gate floor | Per-slot OOD residuals; strict-conjunction policy (aggregate never overrides slot violation) |
| Conformal interval floor | Per-head conformal predictor with split-α calibration |
| Adversarial-embedder-injection mitigation | Refuses hot-swapped embedders without freeze certificate |
| Secret redaction | Pre-publication scan of all artifacts |
| Cross-language label gating | Prevents Python labels from polluting other-language cells |
| Adversarial corpus | Deliberate adversarial fingerprints kept in evaluation set |

---

## Part XI — Instantiation Template (Adapting the Predicate to Any Domain)

To apply the predicate to a new domain `D`, the operator fills in this table:

| Symbol | Meaning | Worked example (ME-JEPA-Code Python) |
|---|---|---|
| `D` | Domain name | "Python code correctness against test suite" |
| `Artifact_D` | What gets predicted | candidate code patch (unified diff) |
| `Oracle_D` | Deterministic ground-truth function | `swebench.harness.run_evaluation` (Docker), parsed FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL semantics |
| `Panel_D` | Slot-decomposable observation function | 15 frozen instruments → 15-vector array (E_AST 384 + E_CFG 256 + E_DataFlow 256 + E_TypeGraph 256 + E_Test 384 + E_Trace 512 + E_Diff 256 + E_Witness 256 + E_Oracle 128 + E_Problem 1024 + E_CommitMsg 384 + E_StaticAnalysis 256 + E_Runtime 256 + E_Reasoning 384 + Scalars 128); array of per-sensor vectors, NEVER flat 5,120-d |
| `Corpus_D` | Representative `(artifact, label)` pairs | 300 instances × 8 mutation categories = 2,400 rows on SWE-bench Lite; split 240 holdout / 240 calibration / 1,920 train via `InstanceAtomic` mode |
| `intervention_class` axis | Categories of perturbation/situation | 8 mutation operators: `known_good`, `subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error` |
| `subdomain` axis | Stratification within the domain | Programming language (currently Python only; other languages deferred to post-ship) |
| `τ_corr(D)` | Cost-asymmetry floor bounded by oracle self-consistency | **0.95** |
| `k_windows` | Rolling-window stability count | **4** |
| `ε_drift` | Panel A vs Panel B correlation drift tolerance | **0.05** |
| `τ_brier(D)` | Mixed-cell Brier ceiling | **0.05** |
| `τ_ece(D)` | Expected calibration error ceiling | **0.05** |
| `δ_repeat` | Mistake-repeat-rate ceiling | **0.05** |
| `ε_unrelated` | Unrelated-cell perturbation tolerance | **0.02** |
| `τ_MI` | Panel sufficiency MI floor | **0.95 bits** |
| `τ_sc` | Oracle self-consistency floor | **0.97** |
| `N_min` | Per-cell support floor (else Abstain) | **50** |
| `Panel_B` | Disjoint-encoder Panel B for cross-panel triangulation | tree-sitter → CodeBERT; e5-large → bge-large; Qodo-Embed-1 → Voyage-code-2; attention CFG → Pythia program-graph; def-use → joern; tree-sitter type → pytype JSON (deterministic slots shared) |
| Durable root | Pinned production storage path | `/zfs/archive/contextgraph/...` (specific to this project) |

Once filled, the universal formula in Part II instantiates to `D`'s exact ship-gate predicate. The 5 prerequisites in Part III must all hold; the 6 tiers in Part IV are then the benchmark stack to be hit.

---

## Part XII — Worked Instantiation: ME-JEPA-Code on Python (Current Project State)

**Current measurement (updated 2026-06-04 with the panel-sufficiency reframe).**

| Metric | Target | Latest measurement |
|---|---|---|
| `prediction_oracle_correlation` (raw Pearson, holdout) | ≥ 0.95 stable × 4 rolling windows × all Python mutation cells | `0.4843192995` |
| Brier (aggregate) | ≤ 0.05 | `0.0923729688` |
| ECE (aggregate) | ≤ 0.05 | `0.0240316074` ✓ |
| Cross-panel `corr_B` (Panel B) | ≥ 0.95 with `|corr_a − corr_b| ≤ 0.05` | Not yet measured |
| `mistake_repeat_rate` (rolling 100-mistake window) | ≤ 0.05 | Storage/MCP shipped; full convergence measurement blocked behind trainer + telemetry repairs |
| **`I(panel; oracle)` (Tier-5 panel sufficiency)** | **≥ 0.95 bits** | **`0.461` bits (Kraskov) / `0.096` (MINE) / `0.125` (classifier LB) — `panel_sufficient=false`** |
| **`oracle_self_consistency` (Tier-5 flakiness axis)** | **≥ 0.97** | **`1.0` (300 Docker invocations, clean) ✓** |
| **`oracle_validity` (Tier-5 validity axis, #845)** | **≥ 0.97** | **Open — flakiness measured (1.0); validity (weak-test/leak) not yet measured** |
| **`association_sufficiency` (k-NN locality / MinSet)** | high / small | **Not yet measured — moot until panel is verdict-sufficient** |

**Honest gap to ship:** `0.95 − 0.4843 = 0.4657` against raw Pearson — **but this gap is not the predictor's to close yet.** The Tier-5 substrate probes (below) have now been run and they relocate the bottleneck upstream of the predictor entirely.

### XII.1 — The grounding-kernel reframe (the central 2026-06-04 finding)

The paper `docs/paper_latex/main.tex` (§ symbol-grounding, `:551-557`) separates two objects the ship-gate vocabulary routinely conflates:

- **Anchor / grounding channel** — the operator that turns a candidate into a grounded bit by contact with non-linguistic reality. Ours is the **Docker oracle** Pass/Fail. It is the *channel*, not the kernel.
- **The grounding kernel (the "1% kernel" / minimum grounding set, MGS)** — the minimal *set of artifacts* that must be pushed through the anchor so the rest is recoverable by **association alone**. For dictionaries this is a minimum feedback vertex set of ≈1% of words (Vincent-Lamarre 2016). For Python: the MGS of the code-association graph (nodes = `(patch, verdict)`; edge `X←Y` = "given the grounded verdict on `Y`, the frozen panel predicts `verdict(X)`" — the constellation-membership `Def(t)` operator).

**Consequence:** the current ship gate brute-forces the anchor across 100% of the corpus (oracle-everything, correlate). The kernel reframe says: ground only the ~1% MGS and derive the other 99% by association. The MGS concentrates on the **Pass/Fail decision-surface boundary cases**, not the obvious-pass / obvious-fail patches.

### XII.2 — Does the Python kernel exist? Already answered: NO at the current panel

A small kernel can only exist if the panel carries the bits — the data-processing ceiling. `I(panel; oracle)` is the **all-methods upper bound**: no predictor, no MGS search, no constellation operator can recover more verdict-information than the panel shares with the oracle. Measured (#759, aiwonder SoT `/zfs/archive/contextgraph/fsv/inv-032-panel-sufficiency-mi-fsv/run-20260529T234900Z/full-candidate-map`, 2,400 rows, slot-identity preserved, no flat-concat):

- `I(panel; oracle) ≤ 0.461 bits ≪ 0.95` → **kernel does not exist at the current 9-slot panel.**
- Anchor is clean: `oracle_self_consistency = 1.0` (#760) — the oracle is **not** the bottleneck.

Computing the MGS today would only re-derive `MGS ≈ 100%`. **You cannot calculate a kernel into existence — raising the panel's bits is what makes one possible.** The existence/size harness is specified (#843) and gated on panel sufficiency.

### XII.3 — Why the panel is information-starved: it measures form, not behavior

Per-slot readback from #759 names the deficiency precisely:

- **Drop-one-embedder ablation (largest deltas):** `e8=0.031, e6=0.029, e7=0.021, e14=0.019` bits. **No slot is load-bearing** — the strongest single contributor is worth 0.031 bits. This is *absence* of signal, not redundancy (redundancy → low drop-one but high joint; here the joint is also low).
- **Per-cell MINE:** `swap_variable=0.231` bits (the only non-trivial cell), `off_by_one=0.021`, and `compile_error / delete_test_call / wrong_file / over_engineer / subtle_flip` all **near zero**. The one cell with signal is the pure token-level edit; the near-zero cells are the **behavioral** ones.

**The embedders see what code *looks like*, not what it *does*.** Two distinct, compounding deficiencies:

1. **Wrong objective.** E1/E7/E8 etc. are similarity/retrieval embedders (the original memory system). Pass/fail prediction is an *outcome/causal* task; a great similarity embedder carries almost no outcome signal.
2. **Wrong modality / observability floor.** The panel is fed static AST. Static source **under-determines** runtime pass/fail, so for behavioral cells the answer is not in the input at all — no embedder can extract a bit the input lacks. Recovering those bits likely requires **execution-derived inputs** (traces, partial-run/coverage, dependency/runtime/env state).

### XII.4 — The plan: assemble a high-signal embedder panel for Python

The binding work is **building a panel whose `I(panel; oracle) ≥ 0.95 bits`** (#745/#844). The method is an empirical embedder-selection loop, not a guess:

0. **Fill the known zeros first (cheapest; do before any neural candidate).** Add near-deterministic structural features for the cells reading ~0 bits — parse/compile check (`compile_error`), removed-test-call diff (`delete_test_call`), import-closure intersection (`wrong_file`) — as **isolated slots gated per-cell with a no-global-regression rule**. (The #774 lesson: a generic execution-trace blob lifted `subtle_flip` +0.21 but regressed `off_by_one`/`swap_variable` and the global number → failed the utility gate. Targeted, isolated indicators do not dilute other cells.) Re-run #759 and confirm those cells move off zero.
1. **Acquire candidates.** Download / instantiate a broad pool of candidate encoders across modalities: code-outcome / execution-aware models, causal (edit→outcome) embedders, dependency/runtime/env encoders, AST/CFG/DFG structural encoders, plus execution-trace and coverage feature extractors. (HTTP only on the research/acquisition path, never in action paths — doctrine §10.)
2. **Run them over the corpus** to produce per-candidate panel vectors on the 2,400-row dataset (slot identity preserved; no flat-concat).
3. **Measure each candidate's marginal bit contribution** — select by **marginal `ΔI = I(panel ∪ {candidate}; oracle) − I(panel; oracle)` in the context of the current panel, never standalone bits** (mutual information is not additive: redundant sensors — the current similarity panel — carry the same bits; synergistic sensors carry little alone but much together, so a low standalone score is not grounds to discard). Use greedy forward selection + drop-one backward elimination, reusing the #759 MINE + classifier + Kraskov estimator stack. Watch **per-cell** `ΔI` on the behavioral categories (currently ~0).
4. **Greedily assemble the minimal panel** that reaches sufficiency subject to the Tier-6 gates (novelty `< 0.7` vs existing slots; per-cell utility lift `≥ 0.005`; inter-sensor distinctness). Stop at sufficiency and **prune** — past sufficiency, extra sensors add redundancy/cost and can dilute the predictor (#774); target minimal-not-maximal (#518 Pareto).
5. **Acceptance — two gates:** re-run #759 → require **(a) verdict-sufficiency** `I(panel; oracle) ≥ 0.95 bits` (against the validity-audited ceiling, #845) with behavioral-cell MI off zero, **and (b) association-sufficiency** — high k-NN verdict locality / small MinSet (bits present but scrambled give a ~100% kernel, §Tier-5). Only when both pass is the kernel (#843) well-posed and the predictor head (#683) able to move Tier-1.

This is the Tier-5 → Tier-6 loop made concrete: **measure bit contribution, keep what carries oracle signal, assemble the minimal sufficient panel, then find the kernel.**

**Activation event for Python.** When the activation banner in Part VII emits TRUE, the 8 consequences execute: human review of AI-generated Python patches becomes statistically unjustified, and the same predicate template begins instantiation for the next eligible domain.

### XII.5 — Critical path to ship (revised 2026-06-04, panel-sufficiency-first)

The earlier ordering put predictor-head repair first. The Tier-5 evidence inverts this: while `I(panel;oracle) < 0.95`, **head/training/calibration work cannot move Tier-1** — it optimizes a derivation that is information-theoretically impossible at this panel.

1. **Panel sufficiency (the binding gate-mover, #745).** Acquire → run → measure-bit-contribution → assemble a panel with `I(panel; oracle) ≥ 0.95 bits` and behavioral-cell MI off zero. Likely requires causal/outcome embedders and execution-derived inputs (XII.3–XII.4). Acceptance = re-run #759.
2. **Kernel/MGS computation (#843).** Once the panel is sufficient, compute the Python MGS (size, composition, per-cell) via the constellation-membership association edge (#841); adopt `MGS_fraction` as the headline progress metric and re-baseline the gate as kernel-coverage.
3. Inter-embedder distinctness gate; clean static cache substrate (kept current).
4. Per-cell weak-cell corpus regeneration (`delete_test_call` Docker oracle batch over 1,131 deterministic candidates → rebuild AST → labels).
5. Unified real training path with public trainer contract; production checkpoint manifest required fail-closed (#683) — **now downstream of panel sufficiency.**
6. Per-slot OOD aggregation; slot-preserving panel projections (anti-compensation repair).
7. Replace any rank-lift Goodhart metric with raw Pearson + isotonic/Platt calibration; eliminate hardcoded telemetry / pseudo-cert confidence / dead training surfaces.
8. Manual full-system FSV/hardening gate; auto-FSV retirement; heal-drill truthfulness; geometry-label bakeoff.
9. (Blocked behind 1-8) Cross-panel `corr_B` / Panel B; trajectory ECE / entropy aux loss; multi-file holdout.

**Issue map (2026-06-04):** reframe synthesis #842 · kernel/MGS harness #843 · panel sufficiency / embedder assembly #745 (+ new candidate-screening child) · derivation engine #841 · existence evidence #759 (MI) + #760 (oracle clean) · gate ordering #712/#383.

---

## Part XIII — Worked Instantiation Sketches (Other Eligible Domains)

These are sketch-level illustrations of how the universal formula applies elsewhere. Each domain would require its own full instantiation document, corpus build, sensor panel, and FSV protocol before the predicate could be claimed.

### Medical pathology — binary malignancy classification

| Symbol | Sketch |
|---|---|
| `D` | Histopathology slide malignancy classification (per tissue type) |
| `Artifact_D` | Whole-slide image |
| `Oracle_D` | Blind 3-pathologist majority vote + biopsy follow-up where available |
| `Panel_D` | per-tile CNN features, color-deconvolution channels, nuclei-morphometry vector, spatial-statistics vector, stain-quality vector, … each in its own sensor space |
| `Corpus_D` | TCGA / CAMELYON / curated tissue-type matrix |
| `intervention_class × subdomain` | (tissue type) × (stain protocol / scanner vendor) |
| Notes | E1 satisfied via biopsy follow-up subset. E5 oracle latency is biopsy turnaround — economically feasible at training scale. `τ_sc` bounded by inter-pathologist agreement (~0.85–0.90 typical) — `τ_corr(D)` set to that ceiling minus ε, with explicit carve-out. |

### Theorem proving — proof correctness verification

| Symbol | Sketch |
|---|---|
| `D` | Mathematical-theorem proof correctness in a decidable fragment (Lean / Coq / Isabelle) |
| `Artifact_D` | Candidate proof term |
| `Oracle_D` | Proof checker (deterministic, fast, near-perfect self-consistency) |
| `Panel_D` | proof-term AST embedding, tactic-sequence embedding, lemma-dependency graph embedding, type-signature embedding, definitional-unfolding embedding, … |
| `Corpus_D` | mathlib / coq-contribs / archive of formal proofs |
| `intervention_class × subdomain` | (proof-class: induction / case-analysis / contradiction / construction / rewrite-only) × (mathematics subfield) |
| Notes | All 5 prerequisites trivially satisfied. `τ_sc ≈ 1.0`. Highest-fidelity domain for this framework. |

### Drug-target binding affinity — binary "binds at threshold X" classification

| Symbol | Sketch |
|---|---|
| `D` | Drug-target binding (Kd ≤ X nM) for given protein family |
| `Artifact_D` | Drug-protein pair (SMILES + protein structure) |
| `Oracle_D` | Wet-lab assay (latency = days–weeks; proxy oracle = high-throughput screening with documented drift bound) |
| `Panel_D` | molecular-graph embedding, 3D-shape embedding, electrostatic-surface embedding, protein-binding-pocket embedding, sequence-similarity embedding, … |
| `Corpus_D` | ChEMBL / BindingDB / DUD-E |
| `intervention_class × subdomain` | (chemotype scaffold class) × (protein family) |
| Notes | E5 is the binding constraint — wet-lab is slow. Proxy oracle (HTS) is acceptable if `oracle_proxy_drift` is bounded and the carve-out is documented. |

### Weather nowcasting — binary precipitation-in-next-hour

| Symbol | Sketch |
|---|---|
| `D` | "Precipitation > X mm/hr in cell (lat, lon) within next 1 hour" |
| `Artifact_D` | Spatio-temporal forecast claim |
| `Oracle_D` | Observed rain-gauge / radar reading at lead time |
| `Panel_D` | radar reflectivity, satellite IR, surface obs, NWP background, orography, climatology, … |
| `Corpus_D` | historical observation archive |
| `intervention_class × subdomain` | (regime: convective / stratiform / orographic / frontal) × (geographic region) |
| Notes | `oracle_self_consistency` bounded by gauge measurement noise. Long-tail regimes (e.g., flash-flood convective) need explicit per-cell support. |

### Chess / Go / Poker — binary "this move is engine-best at depth N"

| Symbol | Sketch |
|---|---|
| `D` | "Candidate move equals engine-best move at depth N" |
| `Artifact_D` | (position, candidate move) pair |
| `Oracle_D` | Strong engine search at depth N (deterministic; near-perfect self-consistency) |
| `Panel_D` | board-state embedding, attack-defense map, material/positional features, opening-book lookup, endgame-tablebase lookup, … |
| `Corpus_D` | historical positions stratified by phase + material balance |
| Notes | Trivially eligible; `τ_sc` engine-determined. This domain is already nominally super-human; the framework here would discipline the verifier's calibration. |

### Component manufacturing QC — binary pass/fail against test rig

| Symbol | Sketch |
|---|---|
| `D` | Manufactured component passes QC test rig |
| `Artifact_D` | Physical component + serial number |
| `Oracle_D` | Test-rig pass/fail (deterministic; near-perfect self-consistency) |
| `Panel_D` | dimensional measurements, surface-finish scan, X-ray internals, thermal imaging, weight/density, electrical resistance, … |
| `Corpus_D` | production-line history with rig outcomes |
| `intervention_class × subdomain` | (defect class: cosmetic / dimensional / electrical / thermal / latent) × (part SKU) |
| Notes | Tier-3 mistake loop is fast (rig latency seconds-minutes). Cost asymmetry typically high → `τ_corr` often 0.99+. |

---

## Part XIV — Permanently Out of Scope

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

## Part XV — The 5-Question Worthlessness Test (Universal)

Before filing any new task, writing any new code, training any new sensor, or proposing any new architectural change *in any domain*, answer:

1. **Which Tier-1/2/3/5/6 number in which domain does this move?** Be specific. "Improves the predictor" is not an answer. "Increases per-cell Pearson on `<intervention_class>::<subdomain>` from `<x>` to `<y>` in domain `D`" is an answer.
2. **By how much?** If expected delta < 0.005 on any target and work cost > 1 day, defer.
3. **What's the falsification condition?** Under what measured outcome is this work classified as failed and rolled back?
4. **What's the doctrine cost?** Does this preserve every Tier-4 invariant? If not, it does not ship.
5. **What's the production execution path?** Specify the durable-root path, SHA-pinning, and manual source-of-truth readback protocol.

If the work cannot answer (1) cleanly with a specific number it moves, **it is worthless by definition** and must be reconsidered or rejected. Activity is not the goal. Movement of the numbers in this document is the goal — in some domain, against some oracle, with every invariant intact.

---

## Part XVI — The Universal Definition of Super-Intelligence

> **Super-intelligence in domain `D` is the existence of a deterministic predictor `Predictor_D` over a slot-identity-preserved observation panel of `D`, whose binary verdicts correlate with the deterministic oracle of `D` above the cost-asymmetry floor (bounded by oracle self-consistency), stably across rolling windows, with cross-panel encoder triangulation confirming the predictor learned about `D` rather than about Panel A, with per-cell quality holding on every mathematically valid cell, with online mistake-loop convergence proving adaptive correction without unrelated-state perturbation, with every architectural-invariant fence intact at every moment, and with substrate-sufficiency probes (panel MI + oracle self-consistency + sensor distinctness) confirming the predicate is reachable on the current state.**

This definition is:

- **Domain-scoped.** It says nothing about general intelligence. It says everything about replacing human review of `D` artifacts at superhuman accuracy and machine-scale throughput.
- **Falsifiable.** Every term is measurable. Failure of any term blocks the claim.
- **Oracle-bounded.** The ceiling is the oracle's self-consistency. We do not claim to exceed reality; we claim to match it.
- **Invariant-protected.** A pass that breaches Tier 4 is not a pass. Goodhart is structurally blocked.
- **Adaptive.** The system that achieves it continues to adapt to its own mistakes after deployment — Tier 3 is not satisfied at one moment, it is satisfied as a continuous property.

Anything that does not match this definition is either a different goal or a confused goal.

---

## Part XVII — The Behavioral Rules That Cross-Cut Every Implementation

These rules apply continuously, in every implementation, in every domain. They are the operational counterpart of the Tier-4 doctrine invariants.

1. **Do what the predicate asks; nothing more, nothing less.** Activity that does not move a number or preserve an invariant is worthless by definition.

2. **NEVER create files unless absolutely necessary.** Prefer editing existing surfaces.

3. **NEVER commit secrets, credentials, or `.env` files.** Any leaked credential must be rotated.

4. **ALWAYS read a file before editing it.** Cold reads are doctrine.

5. **Source-of-truth readback is the verdict, never the return value or the dashboard.** Vendor dashboards lag, double-count, drop events. Read the column-family directly.

6. **No mocks gate completion.** An experiment is not "successful" because the agent reports success. It is successful when the source of truth shows the metric moved.

7. **Bench-delta gate on every agent class change.** Material prompt / model / tool changes ship only after delta-tested against held-out evaluation set: non-regression on goal-relevant metrics; ≤ +5% cost-per-artifact.

8. **Reflexion memory, not amnesia.** Every failed task leaves a structured post-mortem (channel, hypothesis, expected, actual, why-it-differed, retry-conditions) so the next attempt at a similar task inherits the lesson.

9. **Fail closed on unknowns.** Missing infrastructure produces a structured `SCREAMING_SNAKE_CASE` error and halts the operation. Never silently fall back to defaults that hide failure.

10. **Production durable state has a pinned root.** All writes go to the domain's production path. Forbidden roots (development machines, scratch dirs, mounted Windows drives) are blocked by CI lint.

11. **No backwards compatibility shims in pre-production.** Wipe and rebuild on schema change rather than carrying migration code that may itself be wrong.

12. **GENERIC-ONLY in the trained model.** Every trained primitive must generalize across every domain instance. You may reason task-specifically when attempting a specific instance; never bake task-specifics into the trained model.

13. **No HTTP from action paths.** Tool gateways are the only network surface for agent action. Out-of-band fetches in action code paths are doctrine violations.

14. **No destructive git in normal operation.** Force-push, hard reset, branch deletion, etc. require explicit operator authorization on each instance.

15. **Frozen target adapters carry zero trainable parameters.** Enforced structurally via empty `parameters()` on the target adapter, not via convention.

16. **Mistake-loop is the only permitted online weight-update path.** No other code path may update predictor weights at serving time.

17. **Every architectural decision must trace to a number it moves or an invariant it preserves.** If neither, it does not ship.

---

**End state per domain `D`.** When the universal formula in Part II evaluates `TRUE` for domain `D` with `D`'s instantiated thresholds (Part XI), the predictor replaces human review of artifacts in `D`, the activation moment in Part VII fires, the 8 consequences execute, and the same predicate template begins instantiation for the next eligible domain. Until that moment in any domain, every action either moves a number in that domain, preserves a Tier-4 invariant (which is universal), or should be reconsidered.

This document is the complete specification. No external references are required to evaluate the predicate, build a verifier against it, or assess whether a claimed super-intelligence in some domain `D` is genuine. If a question about the framework cannot be answered from this document, the question is not in scope; if a claim about the framework cannot be checked against this document, the claim is not load-bearing.
