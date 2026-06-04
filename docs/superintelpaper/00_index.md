# The Oracle and the Kernel: A Falsifiable, Substrate-Bounded Theory of Domain Super-Intelligence

**A generalized paper, instantiated on ME-JEPA-Code (Python / SWE-bench Lite).**

**Author:** Chris Royse (chrisroyseai@gmail.com).

*Working draft — 2026-06-04. Companion to `docs/SuperIntelligence.md` (the universal predicate) and `docs/paper_latex/main.tex` (the calculus of association). This paper synthesizes the framework with the empirical results discovered in the ContextGraph / ME-JEPA-Code project and states the general theory those results force.*

---

## Abstract

This paper defines **operational super-intelligence in a domain** as the existence of a deterministic predictor that agrees with the domain's ground-truth **oracle** at a calibrated threshold, stably, with Goodhart defenses and online mistake-closure — at which point human review of machine-generated artifacts in that domain becomes statistically unjustified. This is a falsifiable, oracle-bounded property of a specific predictor against a specific domain, not a claim about general cognition.

The paper makes three moves. First, it places the **oracle** at the center: the oracle is the *grounding anchor* — the only point where the system touches non-linguistic reality — and I show that the oracle's self-consistency is a hard ceiling on achievable correlation (you cannot agree with a measuring instrument more often than it agrees with itself). Second, it imports the **symbol-grounding** result from cognitive science (Harnad's dictionary-go-round; the Vincent-Lamarre ≈1% minimum grounding set) and shows that a domain predictor is the same object: a vast web of associations anchored at a small *grounding kernel*, where the oracle is the anchoring channel and the kernel is the minimal set of artifacts that must be sent through it. Third — and this is the load-bearing empirical contribution — it derives an **information-theoretic substrate-sufficiency test**: a grounding kernel can exist only if the predictor's observation panel carries at least the oracle's worth of information about the outcome, measured as mutual information `I(panel; oracle)`. This test is *method-independent* (a data-processing ceiling) and therefore can falsify a whole architecture before any model is trained.

I instantiate the theory on ME-JEPA-Code, a binary `Pass/Fail` predictor for Python code changes against a Docker test oracle (SWE-bench Lite, 300 instances × 8 mutation categories). The instantiation produced a clean **negative result that the theory predicts and explains**: the oracle anchor is essentially noiseless (`oracle_self_consistency = 1.0` over 300 replica evaluations), yet the 9-slot observation panel carries only `I(panel; oracle) ≈ 0.46` bits about a ≈1-bit outcome — far below the `0.95`-bit sufficiency threshold. A per-sensor analysis localizes the deficit precisely: the panel's encoders measure what code *looks like* (similarity / retrieval representations) and are nearly blind to what code *does* (behavioral outcome). No single sensor is load-bearing, and signal survives only in the one purely syntactic perturbation class. I argue this is not a tuning failure but a **substrate** failure, and that the correct response is to assemble a panel of outcome/causal and execution-derived sensors selected by their measured bit-contribution — after which the grounding kernel for Python becomes well-posed for the first time.

The general claim: **domain super-intelligence reduces to (i) a clean oracle, (ii) a panel sufficient to carry the oracle's bits, and (iii) the minimal grounded kernel from which the rest is recovered by association.** Each is separately measurable and separately falsifiable. The oracle and the substrate test, not the predictor, are where domains are won or lost.

---

## How to read this paper

The paper is split across files so each section is self-contained. Read in order for the argument; jump to §7 for the empirical results and the discovered numbers.

| § | File | What it establishes |
|---|---|---|
| 1 | [`01_introduction.md`](01_introduction.md) | The verification bottleneck; the thesis; contributions; what is and is not claimed |
| 2 | [`02_the_oracle.md`](02_the_oracle.md) | The oracle as grounding anchor; formal definition; self-consistency as a hard ceiling; oracle taxonomy |
| 3 | [`03_universal_predicate.md`](03_universal_predicate.md) | The domain-agnostic super-intelligence predicate; six tiers; eligibility prerequisites; threshold derivation |
| 4 | [`04_grounding_kernel.md`](04_grounding_kernel.md) | Symbol grounding; anchor vs. kernel; the ≈1% minimum grounding set generalized to any domain |
| 5 | [`05_substrate_sufficiency.md`](05_substrate_sufficiency.md) | The information-theoretic existence test; data-processing ceiling; Fano bound; why it is method-independent |
| 6 | [`06_panel_and_embedders.md`](06_panel_and_embedders.md) | The observation panel; slot identity; form-vs-behavior; the bit-contribution embedder-selection loop |
| 7 | [`07_instantiation_python.md`](07_instantiation_python.md) | The worked ME-JEPA-Code instantiation; **all discovered facts and numbers with provenance** |
| 8 | [`08_verification_protocol.md`](08_verification_protocol.md) | Final-State Verification; how every claim is grounded in source-of-truth readback |
| 9 | [`09_related_work.md`](09_related_work.md) | Symbol grounding, MI estimation, JEPA, SWE-bench, representation convergence, intelligence measures |
| 10 | [`10_world_models.md`](10_world_models.md) | Generalization (theory): kernel as predictive sufficient statistic; generative vs. predictive kernel; pros/cons |
| 11 | [`11_limitations_and_scope.md`](11_limitations_and_scope.md) | Threats to validity; permanently out of scope; honest boundaries |
| 12 | [`12_conclusion.md`](12_conclusion.md) | The generalized claim restated; the path forward; future domains |
| — | [`99_references.md`](99_references.md) | Bibliography (external works) + internal evidence index (FSV runs, issues) |

**Provenance convention.** External claims cite the bibliography in [`99_references.md`](99_references.md). Project-internal facts cite their **source of truth**: an FSV evidence path under `/zfs/archive/contextgraph/...`, or a tracked issue number. Every number in §7 is traceable to a readback artifact, per the project's verification doctrine (§8).
