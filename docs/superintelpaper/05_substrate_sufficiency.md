# 5. Substrate Sufficiency: The Existence Test for a Kernel

## 5.1 The question

§4 established that a grounding kernel exists only if verdicts are recoverable from the panel by association. There is a sharp, cheap, *method-independent* way to ask whether that is even possible: measure how much the panel tells you about the verdict, in bits.

Let `Y = Oracle_D(x) ∈ {Pass, Fail}` be the outcome and `P = Panel_D(x)` the panel observation. The outcome's uncertainty is its entropy `H(Y)`; on a balanced corpus `H(Y) ≈ 1 bit`. The amount of that uncertainty the panel resolves is the **mutual information**

```
I(P; Y) = H(Y) − H(Y | P).
```

`I(P;Y)` is the number of bits the panel carries about the verdict. If it is near `H(Y)`, the panel almost determines the outcome; if it is near `0`, the panel is nearly irrelevant to the outcome.

## 5.2 Why the test is method-independent (the data-processing ceiling)

The decisive property is that `I(P;Y)` upper-bounds what *any* predictor reading the panel can achieve. By the **data-processing inequality** [CoverThomas2006], for any (possibly stochastic) function `g`,

```
I(g(P); Y) ≤ I(P; Y).
```

A predictor is such a `g`. So is a constellation-membership operator, a kernel/MinSet search, a calibration map, or any composition of them. **No processing downstream of the panel can manufacture information the panel does not contain.** This is what makes the test method-independent: it bounds the predictor, the kernel machinery, and the calibrator simultaneously, before any of them are built.

Fano's inequality [Fano1961; CoverThomas2006] turns the bound into an accuracy statement: the minimum achievable error probability `P_e` in predicting `Y` from `P` is bounded below by a function of `H(Y|P) = H(Y) − I(P;Y)`. As `I(P;Y) → H(Y)`, the error floor `→ 0`; as `I(P;Y)` falls, the error floor rises, *regardless of the predictor*. Concretely, if the panel carries far less than `H(Y)` bits, there is a hard floor on error that no amount of training, architecture search, or calibration can breach.

This connects directly to representation theory: Tishby et al.'s information bottleneck [Tishby1999] frames a good representation as one that squeezes `X` through a bottleneck while preserving `I(·; Y)` — and a panel that fails the sufficiency test is one whose bottleneck has discarded the relevant bits before the predictor ever sees them.

## 5.3 The substrate-sufficiency predicate

Define panel sufficiency as

```
I(Panel_D(x); Oracle_D(x)) ≥ τ_MI       (τ_MI = 0.95 bits for the Python instantiation)
```

This is Tier-5.1 of §3. Its decision rule:

| Measured `I(P;Y)` | Verdict | Action |
|---|---|---|
| `≥ τ_MI` | panel sufficient | a kernel can exist; the gap (if any) is downstream — train/calibrate/search the kernel |
| `[0.7, τ_MI)` | marginally insufficient | add 1–2 informative sensors; re-measure |
| `< 0.7` | provably insufficient | the substrate is wrong; no downstream work can close the gap — rebuild the panel (§6.5) |

The test is **gate-deciding and cheap relative to a training run**: it can falsify an entire architecture before a single predictor is trained, which is why it is the first thing to run after the oracle checks of §2.

## 5.4 Estimating `I(P;Y)` in high dimensions

Mutual information between a high-dimensional continuous panel and a binary label is non-trivial to estimate; the framework uses a **stack of estimators** and reads them together, because each has a different bias/variance profile [Poole2019]:

- **MINE** [Belghazi2018] — a neural estimator using the Donsker–Varadhan representation of KL divergence; a lower bound, scalable in dimension and sample size. Trained to *maximize* the bound, its converged value is a (conservative) lower bound on `I(P;Y)`.
- **Classifier-based lower bound** — train a discriminator to separate joint `p(P,Y)` from product-of-marginals `p(P)p(Y)`; the achievable log-loss gap lower-bounds `I`. A self-test on synthetic data with known signal/no-signal validates the estimator (a strong-signal control should return near `H(Y)`; a noise control should return ≈0).
- **InfoNCE** [vandenOord2018] — the contrastive bound `I ≥ log N − L_NCE`, tight for large batch `N`, differentiable, used where a smooth downstream objective is wanted.
- **Kraskov–Stögbauer–Grassberger (KSG)** [Kraskov2004] — a non-parametric `k`-nearest-neighbor estimator; binning-free, data-efficient, used as a cross-check (typically after a deterministic dimensionality reduction for tractability).

When the estimators **agree that `I(P;Y)` is far below `τ_MI`**, the conclusion is robust: lower-bound estimators can only *under*-state `I`, so a low MINE value paired with a low KSG value is strong evidence of genuine insufficiency rather than estimator failure. (The converse — a high lower bound — would be the case demanding scrutiny for contamination, §2.4.)

## 5.5 Per-cell and per-sensor decomposition

A single scalar `I(P;Y)` says *whether* the panel is sufficient; two decompositions say *where* the deficit is and *what to do*:

- **Per-cell MI**, `I(P; Y | cell)` for each intervention×subdomain cell, shows whether sufficiency fails uniformly or only in particular cells. A panel can carry signal about syntactic interventions and none about behavioral ones — which localizes the missing modality (§6.4).
- **Drop-one-sensor ablation**, `I(P;Y) − I(P∖{sensor_s}; Y)` for each slot `s`, shows each sensor's marginal contribution. If the largest single contribution is tiny, **no sensor is load-bearing**, and the deficit is *absence* of signal across the board rather than redundancy among slots. (Redundancy would show small drop-one deltas *together with* a high joint `I`; absence shows small deltas *and* a low joint.)

These two decompositions are what turn a bare negative result ("the panel is insufficient") into an actionable diagnosis ("the panel measures form not behavior; add outcome/execution sensors"), and they are exactly the measurements reported in §7.

## 5.6 Why sufficiency must be checked before kernel computation

It is tempting to attack the kernel directly — build the association graph, search for the MinSet. But the MinSet search is a particular `g(P)`, so by §5.2 it is bounded by `I(P;Y)`. If `I(P;Y) < τ_MI`, the search cannot find a kernel that does not exist; it will report a degenerate "kernel" of essentially the whole corpus (every artifact must be oracle-queried because none is reliably derivable from the others). Running it would only re-derive the sufficiency verdict at far greater cost. **The MI test is the cheaper, stronger existence test; the kernel computation is the method you use to *find* the kernel once the MI test confirms one exists.** This ordering — oracle (§2) → substrate (§5) → kernel (§4.6) → predictor/calibration — is the operational spine of the framework, and §7 shows a real system sitting exactly at the second gate.
