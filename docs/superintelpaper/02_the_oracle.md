# 2. The Oracle: The Grounding Anchor

## 2.1 Definition

For a domain `D`, an **oracle** is a deterministic, machine-runnable function

```
Oracle_D : Artifact_D → {Pass, Fail}
```

that returns ground truth for a candidate artifact without human judgment in the loop. The oracle is the single point at which the entire system touches **non-linguistic reality**: everything else the system computes — embeddings, panels, predictions, explanations — is internal representation, and acquires meaning only through its statistical relation to oracle outcomes. In the language of §4, the oracle is the **anchor**: the channel through which a symbol (or a vector) is connected to something outside the symbol system.

This is why the oracle, not the predictor, is the first object of study. A predictor is trained, calibrated, and judged *against* the oracle. If the oracle is the wrong instrument, every downstream number is measuring the wrong thing with spurious precision.

Examples of domain oracles: a containerized test suite returning pass/fail for a code patch (this paper's instantiation); a proof checker accepting or rejecting a formal proof; a wet-lab binding assay; the meteorological observation archive for a nowcast; engine search to fixed depth for a chess move; a calibrated test rig for a manufactured part.

## 2.2 The oracle bounds the predictor: self-consistency as a ceiling

The central structural fact about oracles is a ceiling theorem, stated informally:

> **You cannot agree with a measuring instrument more reliably than the instrument agrees with itself.**

Let `oracle_self_consistency` be the probability that two independent evaluations of the oracle on the *same* artifact return the same verdict. If a predictor were to correlate with the oracle at `ρ > oracle_self_consistency`, it would have to predict, for the disputed artifacts, an answer more stable than the oracle's own — which is impossible when the oracle's own verdict is the definition of truth. Hence the ship-gate correlation target is not free: it is **bounded above by the oracle's self-consistency**,

```
τ_corr(D) ≤ oracle_self_consistency(D).
```

Setting `τ_corr = 0.95` is only coherent if `oracle_self_consistency ≥ 0.95`. If the oracle is, say, 0.93-self-consistent, then `0.95` is mathematically unreachable and the honest gap is computed against `0.93`, not against an idealized `1.0`. This is why measuring oracle self-consistency is a Tier-5 substrate test (§3, §5): cheap, and capable of invalidating the entire gate framing before any model work.

## 2.3 Two independent axes: flakiness and validity

"Oracle quality" is not one number. It decomposes into two axes that fail independently, and conflating them is the most common error in benchmark-driven work.

**Axis A — flakiness (self-consistency / reproducibility).** *Does the oracle reproduce its own verdict on re-evaluation?* Sources of failure: non-deterministic tests, timing/race conditions, network flakiness, resource contention, container nondeterminism. This is measured directly by replica voting — run the oracle `n` times per artifact and compute pairwise agreement. A flaky oracle puts a hard ceiling on `τ_corr` (§2.2).

**Axis B — validity (faithfulness).** *Does the oracle's verdict track the property one actually cares about?* An oracle can be perfectly reproducible yet systematically wrong: a test suite too weak to catch a real defect will deterministically pass an incorrect artifact; a test that over-specifies implementation details will deterministically fail a functionally-correct one. The recent SWE-bench audits quantify exactly this on the most-used code oracle: Aleithan et al. find **32.67% solution leakage and 31.08% weak-test passes** in audited SWE-bench instances, with the same defects present in SWE-bench Lite and Verified [Aleithan2024]; Wang et al. show that running *all* repository tests rather than only the PR-modified tests flips **7.8% of "plausible" patches to incorrect** [Wang2025]; and OpenAI's audit of a failed-task subset of SWE-bench Verified found **59.4% with flawed test design or problem statements** [OpenAI2026]. None of these are flakiness; all are validity failures.

The two axes bound the predictor differently:

| Axis | Question | Measured by | Effect on the gate |
|---|---|---|---|
| **Flakiness** | Does the oracle agree with itself? | n-replica pairwise agreement | Caps `τ_corr` at `oracle_self_consistency` |
| **Validity** | Does the oracle track the real property? | test-augmentation / mutation / differential testing; leakage audits | Even at `ρ=1.0`, predictor inherits the oracle's *bias*; the super-intelligence claim is void where the oracle is invalid |

A predictor that perfectly matches an invalid oracle has learned to reproduce the oracle's mistakes. High correlation against a contaminated or weak oracle is not super-intelligence; it is shared error. This is the honest boundary of the whole framework (§1.4, §11): the claim is *conditional on the oracle being a faithful anchor*, and that condition is itself a measurement, not an assumption.

## 2.4 Contamination: a third hazard that disguises itself as signal

Beyond flakiness and validity sits **contamination**: the artifacts (or their gold solutions) appeared in a sensor's pretraining data. Aleithan et al. report that **>94%** of SWE-bench issues predate common LLM knowledge cutoffs [Aleithan2024], and OpenAI report that frontier models could reproduce gold patches verbatim [OpenAI2026]. Contamination inflates apparent panel sufficiency: a sensor may "carry bits" about the verdict because it memorized the answer, not because it encodes a generalizable relation. The framework's defenses are (i) a pretraining-corpus overlap audit per sensor with a counterfactual-cutoff experiment when overlap is high (Tier-5, §3), and (ii) cross-panel triangulation with disjoint encoders (Tier-1.3, §3): a verdict that survives re-encoding by independent sensors is less likely to be a memorization artifact.

## 2.5 The oracle as the unit of generalization across domains

Because the oracle is the only contact with reality, **the framework is exactly as general as the supply of deterministic oracles.** The predicate (§3), the kernel theory (§4), and the substrate test (§5) are domain-agnostic; the only domain-specific commitments are *which* oracle, *which* panel of sensors, and *which* intervention×subdomain matrix the corpus must cover. A domain with no deterministic oracle — aesthetic quality, open-ended dialogue, "good taste" — is **ineligible** (§3), not because the method is weak there but because there is no anchor, and an unanchored association engine is precisely Harnad's dictionary-go-round [Harnad1990]: an efficient way of computing nothing.

## 2.6 Summary

The oracle is the grounding anchor; its self-consistency caps the achievable correlation; its validity conditions the entire claim; and its contamination can masquerade as panel signal. These are four measurements, not assumptions. Only once they are made does it become meaningful to ask whether the *predictor* and *panel* can reach the gate — which is the subject of the rest of the paper.
