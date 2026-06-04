# 4. The Grounding Kernel

## 4.1 The dictionary-go-round

Harnad posed the **symbol grounding problem** with an image [Harnad1990]: suppose you must learn Chinese as a *first* language with only a Chinese/Chinese dictionary. Every word is defined by other words, whose definitions are further words; "the trip through the dictionary would amount to a merry-go-round, passing endlessly from one meaningless symbol or symbol-string … to another … never coming to a halt on what anything meant." A purely symbolic system is in exactly this position: its symbols are defined only by other symbols, so meaning never bottoms out. The escape is to **ground** a subset of the symbols outside the symbol system — for Harnad, in the sensorimotor capacity to pick out the referents of those symbols.

## 4.2 The quantitative result: ≈1% suffices

Vincent-Lamarre et al. made the structure exact [VincentLamarre2016]. Model a dictionary as a directed graph with an edge from each defining word to each defined word. Then:

- Recursively removing every word that can be reached by definition but defines nothing further reduces the dictionary to a unique **Kernel of ≈10%** of the whole.
- About **75% of the Kernel** is its **Core** — a strongly connected subset with a definitional path between any two of its words.
- The remaining **25%** are **Satellites** — small strongly connected sets around the Core.
- The smallest set of words from which all the rest can be defined — the graph's **Minimum Feedback Vertex Set**, which they call the **Minimal Grounding Set (MinSet)** — is about **1% of the dictionary** (≈15% of the Kernel), part-Core and part-Satellite.

Finding a minimum feedback vertex set in a general digraph is NP-hard [Karp1972], so MinSets are NP-hard in general; but real dictionary graphs are small and favorably structured enough to compute them. The interpretation is decisive for this paper: **in principle, grounding only one MinSet's worth of words through sensorimotor contact suffices; the symbolic code can then define all the rest by recombination.** Roughly 99% of meaning is carried by association; a ≈1% grounded anchor carries the rest. The mental lexicon has the matching shape — a small-world, scale-free association graph that grows by differentiation [Steyvers2005].

## 4.3 Anchor versus kernel — the distinction the gate must not blur

Two objects must be kept separate, because conflating them is the central architectural error this paper diagnoses:

- The **anchor** (or grounding channel) is the *operator* that turns one item into a grounded fact by contact with non-linguistic reality. For the dictionary it is sensorimotor experience; for a domain predictor it is the **oracle** (§2). The anchor is not "1% of" anything — it is the mechanism that grounds whatever is sent through it.
- The **grounding kernel** (the MinSet) is the minimal *set of items* that must be sent through the anchor so that everything else is recoverable by association.

A system "grounds" a domain by sending the kernel — and only the kernel — through the anchor. A system that instead sends *every* item through the anchor (oracle-everything, then correlate over the whole corpus) is not exploiting the kernel structure at all; it is brute-forcing the anchor across 100% of the space. This is exactly how a naive ship-gate behaves, and it is the thing the kernel reframe replaces.

## 4.4 The grounding kernel of an arbitrary domain

Transpose the dictionary result to a domain `D` with oracle `Oracle_D` and panel `Panel_D`:

- **Nodes** are candidate artifacts (or `(artifact, verdict)` pairs).
- **Edge `X ← Y`** ("`X` is definable from `Y` by association") holds when, given the grounded verdict on `Y`, the panel predicts the verdict on `X` above a confidence threshold. In a JEPA-style architecture [LeCun2022] this edge is computed in representation space — predicting the *representation* of one artifact's outcome region from another's — and concretely it is a **constellation-membership** test over the panel (the `Def(t)` operator of §6): an artifact belongs to the "Pass" region if its per-sensor marginals and characteristic cross-sensor correlations match the region learned from grounded examples.
- The **grounding kernel** of `D` is the minimum feedback vertex set of this association graph: the minimal set of artifacts whose verdicts must be obtained from the oracle so that all remaining verdicts are recoverable by association.

Two consequences follow immediately.

1. **The kernel is the hard boundary cases, not the easy ones.** Artifacts whose verdict is obvious by association — an artifact that is clearly like many grounded Pass examples, or clearly like many grounded Fail examples — are the derivable 99%. The kernel concentrates on the **decision-surface** artifacts where association is locally ambiguous and only ground truth disambiguates. Operationally, the kernel is what an optimal active learner would select to query.

2. **There is no universal kernel across all knowledge.** Each domain has its own anchor and its own MinSet — language grounds ≈1% of its lexicon in sensorimotor experience; code-correctness grounds the MinSet of its code-association graph in execution; the paper's other eligible domains (§ sketches in `SuperIntelligence.md`) ground their MinSets in their own oracles. What is universal is the **structure** — (association graph) + (small grounded MinSet) + (anchor to reality) — not the kernel itself. The generality of the framework is the generality of that structure, exactly as the dictionary result is general across languages while each dictionary's MinSet is its own.

## 4.5 From symbols to vectors

A modern panel computes over vectors, not symbols, so the relevant grounding problem is Mollo & Millière's **Vector Grounding Problem** [Mollo2023]: can a system's internal vector states be *about* extra-linguistic reality, independently of the meaning a human reads into them? Their answer, which I adopt as the grounding criterion for the panel, is teleosemantic: a representation is **referentially grounded** when (1) it stands in appropriate **causal-informational relations** to a worldly outcome, and (2) it has a **history of selection** that gave it the function of carrying that information. This is precisely the condition a sensor in the panel must meet to belong in the kernel machinery: it must carry information about the oracle outcome (measured as mutual information, §5), and it must have been selected for doing so (the utility gate, §3 T6; the bit-contribution selection loop, §6.5). A sensor that merely encodes surface similarity — that was selected to carry "what this artifact looks like," not "what outcome it produces" — fails condition (1) for the outcome and is, for grounding purposes, ungrounded, however useful it is for retrieval. §7 shows this is not hypothetical: it is exactly the deficit measured in the Python instantiation.

## 4.6 The reframe of the ship-gate

Putting §2 and §4 together yields the reframe that organizes the rest of the paper. The naive gate asks: *does the predictor correlate with the oracle over the whole corpus at `τ_corr`?* The kernel reframe decomposes this into three sharper, separately-falsifiable questions, in dependency order:

1. **Is the anchor clean?** `oracle_self_consistency ≥ τ_sc`, and is the oracle valid and uncontaminated? (§2)
2. **Can a kernel exist at all?** Does the panel carry the oracle's bits — `I(panel; oracle) ≥ τ_MI`? If not, *no* kernel exists at this panel, and the fix is upstream in the sensors. (§5)
3. **What is the kernel, and how small is it?** Once the panel is sufficient, compute the MinSet, ground it through the oracle, and recover the rest by association; report the kernel fraction as the headline progress metric and re-baseline the gate as *kernel-coverage* rather than corpus-correlation.

The empirical contribution of §7 is that, on a live system, question 1 answers *yes* (the anchor is clean on the flakiness axis) and question 2 answers *no* (the panel carries ≈0.46 of the needed ≈0.95 bits) — which makes question 3 ill-posed until the panel is rebuilt, and shows exactly where the engineering effort must go.
