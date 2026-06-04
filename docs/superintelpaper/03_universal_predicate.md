# 3. The Universal Super-Intelligence Predicate

This section states the domain-agnostic predicate. It is the formal core of the companion document `docs/SuperIntelligence.md`, reproduced here with the threshold derivation and the oracle-ceiling and substrate-sufficiency conditions made explicit.

## 3.1 Objects

For a domain `D`:

- `Oracle_D : Artifact_D → {Pass, Fail}` — the deterministic ground-truth function (§2).
- `Panel_D : Artifact_D → Array⟨Vec_i⟩` — the observation function, packing an artifact into an **array of per-sensor vectors**, each in its own sensor's space. No flat concatenation; slot identity is preserved (§6).
- `Corpus_D` — a representative set of `(artifact, oracle_label)` pairs covering an `(intervention_class × subdomain)` matrix.
- `Predictor_D : Panel_D(x) → {Pass, Fail}` — a deterministic learned function with calibrated confidence.

## 3.2 The predicate

`D` admits **superhuman binary verification** iff there exists `Predictor_D` such that:

```
corr(Predictor_D(x), Oracle_D(x)) ≥ τ_corr(D)                       # T1.1 ship-gate correlation
  STABLE over k_windows consecutive rolling windows                  # T1.2 stability
AND corr(Predictor_D^B(x), Oracle_D(x)) ≥ τ_corr(D)  on Panel B     # T1.3 cross-panel (Goodhart guard)
AND |corr_A − corr_B| ≤ ε_drift                                     # T1.4 panel drift
AND per-cell quality contract holds ∀ (intervention × subdomain)    # T2  per-cell
AND mistake_repeat_rate ≤ δ_repeat over rolling N-mistake window    # T3.1 online closure
AND unrelated_control_delta ≤ ε_unrelated                           # T3.2 no collateral drift
AND every doctrine invariant equals its required value exactly      # T4  invariants
AND I(Panel_D(x); Oracle_D(x)) ≥ τ_MI bits                          # T5.1 substrate sufficiency
AND oracle_self_consistency(D) ≥ τ_sc                               # T5.2 oracle reachability
AND inter_sensor_distinctness gate passes ∀ active slots            # T5.3 no sensor aliasing
AND ∀ new/replaced sensor: novelty + utility gates pass             # T6  sensor gating
```

When this evaluates `TRUE`, **human review of machine-generated artifacts in `D` becomes statistically unjustified.** That is the operational definition of super-intelligence in `D`.

## 3.3 The six tiers, in plain terms

- **Tier 1 — the ship-gate numbers.** The predictor agrees with the oracle at `τ_corr`, stably across time (T1.2), and the agreement survives re-encoding the same artifacts through an independent **Panel B** (T1.3) without drifting (T1.4). Panel B is the Goodhart guard: a number that holds on one panel but collapses on a disjoint one was a panel artifact, not a fact about reality [Manheim2018].
- **Tier 2 — per-cell quality.** The contract holds **in every cell** of the intervention×subdomain matrix, not just in aggregate. Aggregate correlation can hide a cell at chance; the per-cell contract (per-cell correlation, Brier ≤ threshold [Brier1950], ECE ≤ threshold [Guo2017], class recalls with confidence-interval lower bounds) forbids that.
- **Tier 3 — online mistake closure.** When the predictor disagrees with reality, the mistake becomes a training signal and the *same* mistake does not recur (T3.1), without degrading unrelated behavior (T3.2). This is the loop that lets the system improve without human supervision.
- **Tier 4 — doctrine invariants.** Architectural fences that must hold *exactly* (§3.5). These are not optimization targets; they are correctness conditions whose violation voids the claim.
- **Tier 5 — substrate quality.** The cheapest, most decisive tests: is the panel information-theoretically sufficient (T5.1, §5), is the oracle reachable (T5.2, §2), and are the sensors non-aliased (T5.3)? If Tier 5 fails, no Tier-1 work can succeed; the substrate must change first.
- **Tier 6 — sensor gating.** Any sensor added or replaced must pass a **novelty** gate (it is not a near-duplicate of an existing slot) and a **utility** gate (it lifts per-cell correlation by at least a floor). This prevents both redundancy and Goodhart-by-sensor-proliferation, and it is the gate the embedder-assembly loop of §6.5 must satisfy.

## 3.4 The thresholds, and why these values

Thresholds are domain-set by the **cost asymmetry** between a false-Pass and a false-Fail, and bounded by the oracle (§2.2). For the Python instantiation:

| Symbol | Value | Justification |
|---|---|---|
| `τ_corr` | `0.95` | The correlation at which the predictor's verdict carries effectively the same information as a human reviewer's on this corpus; bounded above by `oracle_self_consistency` (§2.2). |
| `k_windows` | `4` | Four consecutive rolling windows holding distinguishes a stable regime from a lucky window. |
| `ε_drift` | `0.05` | Maximum tolerated gap between Panel A and Panel B correlation; larger gaps indicate a panel-specific artifact. |
| `δ_repeat` | `0.05` | Maximum fraction of mistakes allowed to recur after they have been fed back. |
| `ε_unrelated` | `0.02` | Maximum collateral movement on unrelated controls when a mistake is corrected. |
| `τ_MI` | `0.95 bits` | The panel must share at least ~95% of the outcome's ≈1 bit; the substrate existence test (§5). |
| `τ_sc` | `0.97` | Oracle self-consistency floor; below this, `τ_corr=0.95` would be near the noise ceiling. |

**Threshold revision is allowed only upward, and only on evidence.** If oracle self-consistency is measured higher than expected, `τ_corr` may be raised (e.g. `0.95 → 0.97`). Lowering `τ_corr` because the current substrate cannot reach it is not a state-change — it is a definition change, and a regression. The correct response to "this substrate cannot reach `0.95`" is to change *state* (better oracle, better panel, larger corpus), never to move the goalpost (§3.6).

## 3.5 The Tier-4 invariants (fences, not levers)

Eight invariants must hold exactly, because each one, if violated, would let the system reach the numbers by cheating rather than by working:

1. **Binary contract** — the predictor emits `Pass/Fail` against ground truth; ambiguous "soft" heads with no free binary form against reality are out of scope.
2. **Slot identity preservation** — the panel is an array of per-sensor vectors; no cross-sensor dimension-wise comparison (§6.2).
3. **No flat-vector collapse** — the panel is never flattened into one vector for similarity, distance, OOD, or predictor input.
4. **Anti-compensation** — a strong sensor may not silently paper over a blind one; per-slot out-of-distribution detection and slot-preserving projections enforce this.
5. **Frozen-target collapse-immunity** — the prediction-target representation has zero trainable parameters, so it cannot collapse to a trivial solution.
6. **No inner generator** — the system predicts; it does not contain a hidden generative model of the artifact that could leak the answer.
7. **Leakage policy** — target labels never enter as live predictor inputs.
8. **Grounded anchor** — the only source of truth is the oracle outcome; no synthetic or self-referential label may stand in for it.

## 3.6 State is the lever; the predicate is the fence

The current corpus size, the current panel, the current oracle harness are **starting state, not constraints**. Reaching the predicate may require corpus expansion, panel expansion, oracle refinement (replica voting, flakiness modeling), or upward threshold revision — all legitimate. Only the Tier-4 invariants and the predicate itself are immovable. The discipline is: *change state to reach the numbers; never change the numbers to fit the state.*

## 3.7 Eligibility prerequisites

Before the predicate even applies, a domain must satisfy five conditions, or claiming super-intelligence in it is a category error:

1. **A deterministic oracle exists** (else no anchor; §2.5).
2. **The oracle is reproducible and valid enough** to set a coherent `τ_corr` (§2.2–2.3).
3. **Artifacts are observable through sensors** that can, in principle, carry outcome information (the substrate question, §5).
4. **A representative intervention×subdomain matrix can be enumerated and sampled** (for the per-cell contract, T2).
5. **Outcomes can be fed back** as training signal (for online closure, T3).

The remainder of the paper develops the two prerequisites that turn out to be decisive in practice — the oracle (already, §2) and the substrate (§5) — and shows, on a live system (§7), that a domain can satisfy prerequisites 1, 2, 4, 5 and still fail prerequisite 3, which is precisely the negative result the framework exists to surface early.
