# 10. Generalization: From Verdict Oracles to World Models

The instantiation of §7 used a *binary verdict oracle* (Docker tests, `Pass`/`Fail`). The three conditions the framework reduces to — a clean oracle, a sufficient panel, a minimal grounded kernel (§4.6) — are not specific to binary verdicts. This section theorizes their extension to **predictive world models** in the sense of LeCun's Joint Embedding Predictive Architecture [LeCun2022]: a model that predicts the representation of the future from the representation of the present plus an action.

**Epistemic status.** This section is more speculative than §§1–8. It is *theory grounded in cited results and in the repository's measured findings*, not itself a measured result. It is included because the same three conditions, and the same substrate test, appear to govern world models — and because the failure mode measured for code (§7.4, "form not behavior") is exactly the failure mode a world-model encoder exhibits when it captures appearance rather than dynamics. Claims here are marked as conjecture where they are conjecture, and the genuinely uncertain ideas are given with pros and cons (§10.6).

## 10.1 The world-model setting changes the oracle and the target

In the verdict setting the oracle returns one bit (`Pass`/`Fail`) and the outcome entropy is `H(Y) ≈ 1 bit`. In the world-model setting the "oracle" is **reality's response to intervention** — you act, the world evolves, you observe — and the "verdict" is the **future state**, which is high-dimensional and high-entropy. Three things follow:

- The anchor is no longer a clean deterministic binary function. It is partially observable, stochastic, continuous, and non-stationary. The oracle axes of §2 (flakiness, validity, contamination) still apply but are harder to bound.
- The sufficiency threshold `τ_MI = 0.95 bits` does not transfer; it must be restated relative to how much of the future is *predictable at all*.
- Grounding requires **intervention**, not just observation: passive data under-determines causal structure (correlation versus causation), so the grounded examples that anchor the kernel must come, at least in part, from acting on the world.

## 10.2 The kernel becomes the predictive sufficient statistic

The dictionary's Minimal Grounding Set (§4.2) is the minimal set for *static definability* — a minimum feedback vertex set of a definitional graph. A world model needs the minimal set for *prediction of dynamics*. That object is also rigorously defined:

- **Causal states / the ε-machine** [Shalizi2001]. Group every history of the process into equivalence classes that induce the *same distribution over futures*. These causal states form the **minimal sufficient statistic for prediction** — provably the simplest representation that loses no predictive power ("every difference about the past that makes a difference to the future"). This is the dynamical generalization of the grounding kernel: not a feedback vertex set, but the minimal predictive representation of the environment.
- **Predictive information** [Bialek2001] bounds the kernel's size. The mutual information between past and future, `I_pred = I(\text{past}; \text{future})`, is the predictable structure; its growth is **sub-extensive** — it grows far slower than the raw data. "Predictability is a deviation from extensivity." In plain terms: *most of reality's bits are unpredictable detail; only a small, sub-extensive core is learnable.* That core is the world-model kernel, and predictive information is how large it is.

**A caveat that must not be skipped.** The dictionary's "≈1%" figure (§4.2) is a property of dictionary-graph topology (small-world, scale-free); it is **not** a constant that transfers to physical dynamics. What transfers is the *structure* — anchor + small grounded kernel + recovery of the rest by association — and the *existence* of a small predictable core (guaranteed sub-extensive by [Bialek2001]). The *fraction* is a measurable property of each domain's process, not a universal number.

## 10.3 The substrate test transfers directly

The substrate-sufficiency test (§5) generalizes from `I(\text{panel}; \text{oracle})` to

```
I(representation; future)   ≥   (the predictable structure, I_pred)
```

bounded above, as before, by the data-processing inequality (§5.2): no policy, planner, or controller reading the representation can know more about the future than the representation carries. The §7.4 finding — the code panel measures *form*, not *behavior*, so `I(panel; oracle) ≈ 0.46` — is **the same failure** a world-model encoder exhibits when it captures *appearance* (pixels, textures) rather than *dynamics* (what will move, what will fall). JEPA's central bet [LeCun2022] — predict in representation space, non-generatively, discarding unpredictable pixel detail — is, read through this lens, an attempt to learn exactly the causal-state representation and throw away the rest. The framework therefore offers a concrete acceptance gate for a world-model encoder: **measure `I(representation; future)` under intervention; if it is low while reconstruction loss is also low, the encoder has learned form, not dynamics — the §7 pathology, transposed.**

## 10.4 The grounded kernel has two layers

For a physical-world model the kernel decomposes into a directly-grounded prior and a learned dynamical core:

- **Layer A — the prior kernel ("start-up software").** The directly-grounded primitives, the analog of the dictionary's grounded MinSet words. Developmental science offers a candidate list: the **core knowledge systems** [Spelke2007] — objects (cohesion, continuity, solidity, persistence), agents/actions (goal-directedness), number, and space/geometry (plus a possible social system) — and the AI-facing argument that this "developmental start-up software," intuitive physics plus intuitive psychology, is what makes few-shot generalization possible [Lake2017]. Seeding Layer A as inductive bias is what buys sample efficiency.
- **Layer B — the dynamical generators.** The minimal set whose closure under composition generates predictable dynamics: symmetries and the conservation laws they imply (a handful of invariances generate vast dynamical constraint), a small set of interaction primitives at the agent's scale (contact, gravity, friction, elasticity, fluids), and the causal-intervention operators that say what the agent's actions do. Formally, Layer B is the causal-state representation (§10.2) of the environment at the agent's scale, seeded by Layer A.

## 10.5 Two kernels: the generative and the predictive

A natural and important objection is that the real kernel of the physical world is not abstract at all — that it is the physical building blocks themselves (subatomic particles, atoms, genes), and that with enough microscopic detail one could predict everything. This is the reductionist / Laplace's-demon hypothesis, and the right response is to separate **two distinct notions of kernel**:

- **The generative (ontological) kernel** — the microscale rules plus the full microstate. Run forward, it produces reality. Its *rules* are genuinely tiny, and as a statement of *what reality is made of* the reductionist hypothesis is essentially correct. This is the most compressed *generator* there is.
- **The predictive (epistemic) kernel** — the smallest representation, at the agent's scale and within the agent's resources, that retains the *predictable* structure of a target (§10.2). This is what a world model needs.

The decisive point is that **the generative kernel is not a usable predictive kernel**, and the predictive kernel is obtained by coarse-graining microscale detail *away*, not by accumulating more of it. Six independent obstacles block "predict the world from its microstate," each sufficient on its own:

1. **Reductionism does not imply constructionism** [Anderson1972]: "the ability to reduce everything to simple fundamental laws does not imply the ability to start from those laws and reconstruct the universe." New organizing laws emerge at each scale and must be discovered, not deduced.
2. **Exponential cost** [Feynman1982]: the quantum state space grows exponentially in the number of particles; quantum matter cannot be efficiently simulated on a classical computer (Feynman's original motivation for quantum computers).
3. **Unmeasurable initial conditions**: the uncertainty principle and the no-cloning theorem forbid acquiring the exact microstate even of a single particle, let alone $10^{23}$ of them. The premise "with enough subatomic information" is physically unsatisfiable, not merely expensive.
4. **Chaos**: in chaotic systems infinitesimal errors in initial conditions grow exponentially, so near-perfect microstate yields near-zero long-horizon macro-prediction.
5. **Computational irreducibility** [Wolfram2002]: for most non-trivial systems there is no shortcut — the only way to find the outcome is to run the computation; and a predictor cannot out-compute a system of comparable computational sophistication.
6. **Predictive irrelevance** [Bialek2001]: the predictable structure is sub-extensive, so almost all microscopic bits are irrelevant to macro-scale outcomes; a kernel must discard them.

The operation that legitimately turns microscale data into a usable kernel is **coarse-graining**, formalized as the renormalization group: iterative extraction of the *relevant* variables across scales. Strikingly, deep learning has been shown to implement a generalized RG-like coarse-graining [Mehta2014], which is the mechanism by which a learner flows from fine-grained input to the predictive kernel — discarding the irrelevant, keeping the relevant. The right scale is therefore not chosen a priori; it is the scale that maximizes `I(representation; future)` per unit of representation cost (§10.3), i.e. the renormalization-group idea meeting the substrate test.

**The honest concession.** This does *not* mean abstract always beats concrete. The predictive kernel sits at the **coarsest scale that still retains the predictive information for the target**, and that scale is empirical and domain-specific. For some domains it genuinely is close to a physical-building-block level — protein structure predicted from amino-acid sequence, or material properties from atomic configurations with learned effective interactions — which partly vindicates the building-block instinct. For others (everyday manipulation, weather) the kernel sits far above the microscale (objects and forces; fluid fields). The lesson is not "pick atoms" or "pick abstractions"; it is "let coarse-graining and the predictive-information measurement find the scale."

## 10.6 Pros and cons of the two conjectures

The two genuinely uncertain ideas in this section, with their trade-offs.

### Conjecture 1 — A world model is a kernel-finding problem (kernel = predictive sufficient statistic)

| Pros | Cons |
|---|---|
| A small predictable core provably exists: predictive information is sub-extensive [Bialek2001], and the causal-state representation is a well-defined, uniquely minimal target [Shalizi2001]. | The causal states are not given; reconstructing them in high dimensions is the learning problem itself, not a preprocessing step. |
| Sample efficiency: grounding a small kernel (seeded by core-knowledge priors [Spelke2007; Lake2017]) explains few-shot generalization and sidesteps "train on all of reality." | The anchor is far harder than a verdict oracle — partially observable, stochastic, requiring intervention to ground causal (not merely correlational) structure. |
| Measurability: the substrate test transfers as `I(representation; future)`, giving a falsifiable acceptance gate for a world-model encoder (§10.3) — the same instrument built for code. | The target is high-entropy and partly unpredictable (chaos, irreducible noise); the model is bounded by predictive information, never "understanding everything." |
| Transfer: a grounded dynamical kernel is a world model, not a task policy, so it generalizes across tasks. | The kernel is scale- and goal-relative; there is no single universal physical-world kernel, so the domain must be scoped. |

### Conjecture 2 — The kernel could be a concrete physical-building-block scale (atoms / cells / genes)

| Pros | Cons |
|---|---|
| Ontologically correct: the microscale plus its laws is a complete generator, and the laws are genuinely tiny — one basis in principle for all domains. | Reductionism is not constructionism [Anderson1972]; the levels cannot in practice be derived from the level below. |
| Empirically vindicated for specific domains: protein structure from sequence, material properties from atomic configurations with learned effective potentials. | Acquiring the microstate is physically forbidden (uncertainty, no-cloning) and simulating it is exponentially costly [Feynman1982]. |
| Avoids hand-designed abstractions; "learn from the bottom" is general and matches how foundation models already operate. | Chaos and computational irreducibility [Wolfram2002] mean micro-precision does not buy macro-prediction; most micro-bits are predictively irrelevant [Bialek2001]. |
| Points to the correct mechanism — coarse-graining / RG [Mehta2014] — for getting from fine data to the kernel. | The working systems that "use" this idea are already coarse-graining (learned effective maps), so it is not literally prediction-from-atoms, and it is the wrong, intractable scale for most agent-scale tasks. |

**Synthesis.** The generative kernel is real but unusable; the predictive kernel is what a world model needs; it is found by coarse-graining the microscale away and is validated by the same `I(\cdot;\cdot)` substrate test built for the verdict setting. The kernel's scale is not assumed but measured — sometimes landing near the physical building blocks, usually above them. The framework's contribution to the world-model question is therefore not a new architecture but a *measurement*: where, across scales, the predictive information concentrates, and whether a candidate representation carries it.
