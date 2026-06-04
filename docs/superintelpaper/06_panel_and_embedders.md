# 6. The Panel and Its Sensors

## 6.1 The panel as an array of per-sensor vectors

The observation function `Panel_D` does not produce a single embedding. It produces an **array of per-sensor vectors**, one per sensor (or "slot"), each living in its own sensor's representation space. This is the JEPA stance [LeCun2022]: an artifact is encoded by multiple encoders that "are not required to possess the same architecture nor … to share their parameters," precisely so that heterogeneous views — structure, behavior, dependency context — can each contribute in their own space, and prediction happens in representation space rather than by reconstructing the artifact.

The panel is read as a set of coordinates against one underlying reality. The license for treating `N` heterogeneous encoders as commensurable coordinates rather than `N` incompatible spaces is the **Platonic Representation Hypothesis** [Huh2024]: networks "trained with different objectives on different data and modalities are converging to a shared statistical model of reality," with the converged kernel approaching the **pointwise mutual information over the latent events that generate observations.** That is the same PMI structure the grounding kernel (§4) and the substrate test (§5) are built around — which is why a multi-sensor panel can be read as triangulating one association structure, and why cross-panel agreement (Tier-1.3) is, in effect, a local test of that convergence.

## 6.2 Slot identity is sacred

Because each sensor's vector lives in its own space, **cross-sensor dimension-wise comparison is meaningless and forbidden** (Tier-4, §3.5). The only permitted operations are: (a) *same-sensor* similarity (sensor `i`'s vector for `x` versus sensor `i`'s vector for `x'`); (b) *array-level structured* operations (per-slot similarity maps, slot-identity-preserving projections); and (c) *scalar pairwise cross-sensor relationships* (mutual information, correlation, agreement between slots). The panel is **never** flattened into one long vector for similarity, distance, OOD residual, calibration, or predictor input. Flattening would assert that dimension `k` of a syntax encoder is commensurable with dimension `k` of a dependency encoder — a type error that silently destroys sensor identity and invites Goodhart. This is why the substrate measurements of §5 and §7 are reported with `slot_identity_preserved = true` and `flat_vector_concat_used = false` as explicit invariants.

## 6.3 Constellation-as-definition: the association edge made concrete

The association edge of §4.4 is realized as a **constellation** object. The "Pass" outcome class in a cell is defined not by a single centroid but by a structured pair:

```
Def(t) = ( { (c_m, τ_m) }_{m=1..N} ,  { ρ_jk }_{j<k} )
```

— the per-sensor marginal regions `(centroid c_m, threshold τ_m)` (indexical: where each sensor sits), **and** the characteristic cross-sensor correlations `ρ_jk` (constitutive: how the sensors co-vary for that class). An artifact is judged a member by both its marginals *and* its cross-sensor signature. This is the operator that says "given the grounded examples, does this artifact's panel constellation match the Pass region?" — i.e. the edge that lets one artifact's verdict be derived from grounded neighbors, and therefore the machinery the kernel/MinSet computation runs on. It is also the binary-outcome restatement of a teleological-constellation guard: membership in a frozen region learned from `O(k)` grounded examples, which is the "fast skill acquisition" reading of intelligence [Chollet2019] applied to a Pass/Fail region.

## 6.4 Form versus behavior: the deficit a sufficiency test exposes

The substrate test (§5) and its per-sensor decomposition can reveal a specific, important pathology: a panel whose sensors are **referentially grounded in the wrong thing** [Mollo2023]. Many strong code encoders are trained for *similarity / retrieval* — "find code like this" — and they encode *what an artifact looks like*. But a Pass/Fail oracle measures *what an artifact does* when executed. These are different functions of the artifact, and a sensor selected to carry the first can carry almost none of the second. The signature of this pathology, visible in the per-cell MI decomposition, is that signal survives for **syntactic** interventions (a renamed variable visibly changes the tokens) while **behavioral** interventions (does it compile? was a needed call deleted? is it the wrong file?) read near zero — because for the behavioral classes the verdict is determined by runtime behavior the static representation never observed.

This splits the deficit into two compounding causes, each with a distinct fix:

1. **Wrong objective.** The sensors optimize similarity, not outcome. Fix: **causal / outcome sensors** trained on the relation *edit → outcome*, not *code → similar code*.
2. **Wrong modality (an observability floor).** A static encoding of source code *under-determines* runtime pass/fail — the determining bits are not present in the input at all, only obtainable by execution. No encoder, however good, can extract a bit the input does not contain (the data-processing ceiling again, §5.2). Fix: **execution-derived sensors** — traces, partial-run/coverage signals, exception signatures, dependency/runtime/environment state.

## 6.5 Assembling a sufficient panel by measured bit-contribution

When the substrate test fails, the framework does not guess at new sensors; it **selects them by their measured contribution to `I(panel; oracle)`**. The loop:

1. **Acquire candidates** across the axes the panel lacks: outcome/causal encoders, execution-trace and coverage extractors, dependency/runtime/environment encoders, and structural encoders beyond the current view. (Acquisition and research use the network; the action/serving path does not — the only network surface is the product interface.)
2. **Run each candidate over the corpus** to produce per-candidate panel vectors, preserving slot identity, no flat-concat.
3. **Measure bit-contribution** with the estimator stack of §5.4: standalone `I({candidate}; oracle)`; greedy marginal `ΔI = I(panel ∪ {candidate}; oracle) − I(panel; oracle)`; and **per-cell** `ΔI`, watching the behavioral cells that currently read zero.
4. **Greedily assemble** the panel that maximizes `I(panel; oracle)` subject to the Tier-6 gates (§3): a candidate must clear the **novelty** gate (max-correlation with existing slots below a ceiling — it adds information, not redundancy) and the **utility** gate (per-cell correlation lift above a floor — it is not noise). Stop when `I(panel; oracle) ≥ τ_MI` or candidates are exhausted.
5. **Accept** by re-running the substrate test: require `I(panel; oracle) ≥ τ_MI` **and** behavioral-cell MI lifted off zero.

This is the Tier-5 → Tier-6 loop operationalized, and it is the teleosemantic grounding criterion [Mollo2023] made into an engineering procedure: a sensor earns a slot exactly when it is *measured* to carry oracle-outcome information (causal-informational relation) and is *kept because of* that measurement (selection history). Only after this loop reaches sufficiency does the kernel computation (§4.6) become well-posed, and only then can predictor training and calibration [Guo2017] move the Tier-1 number.

## 6.6 Predictor, calibration, and online closure (briefly)

Downstream of a sufficient panel, the predictor is a deterministic head over the panel (and the constellation operator of §6.3), emitting `Pass/Fail` with a calibrated confidence — calibration measured by Brier score [Brier1950] and ECE and corrected by isotonic/Platt/temperature scaling [Guo2017], with conformal intervals [Angelopoulos2021] for distribution-free coverage and an explicit **abstain** verdict when the artifact is out-of-distribution per-slot. Mistakes (disagreements with the oracle) are logged and fed back under protection against catastrophic forgetting [Kirkpatrick2017], closing the Tier-3 loop. None of this machinery can substitute for substrate sufficiency: by §5.2 it is all bounded by `I(panel; oracle)`. Calibration makes a sufficient panel's predictor trustworthy; it cannot make an insufficient panel's predictor correct.
