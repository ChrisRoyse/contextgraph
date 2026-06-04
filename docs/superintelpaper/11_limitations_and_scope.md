# 10. Limitations and Scope

The framework's strength is that its claim is narrow and falsifiable. This section states the boundaries explicitly, because the value of a falsifiable claim is destroyed by quiet scope creep.

## 10.1 The claim is operational and per-domain, not general

"Super-intelligence" here means: a predictor that replaces a human verification function at higher accuracy and machine throughput, in a single domain that has a deterministic oracle. It is **not** a claim about general intelligence, understanding, sentience, or any capacity that lacks a deterministic oracle (§1.4). A system that passes the predicate in Python verification has not thereby become generally intelligent; it has become a superhuman *verifier of Python patches against a test oracle*. Each new domain requires its own oracle, panel, corpus, and a fresh instantiation.

## 10.2 Everything rests on the oracle being a faithful anchor

This is the load-bearing limitation. Association is circular unless grounded (§4); the only thing that grounds the system is the oracle. Therefore:

- **Validity is assumed exactly where it is hardest.** The Python instantiation measured oracle *flakiness* (`oracle_self_consistency = 1.0`, §7.2) but has **not** bounded oracle *validity*. The SWE-bench literature shows the validity hazard is real and large: ~33% solution leakage and ~31% weak-test passes in audited instances [Aleithan2024], ~7.8% of plausible patches flipping under full-test re-validation [Wang2025], and ~59% of an audited failed subset of SWE-bench Verified having flawed tests [OpenAI2026]. A predictor that perfectly matched such an oracle would have learned to reproduce its mistakes. **High correlation against an invalid oracle is shared error, not super-intelligence.** Until validity is bounded for the corpus in use, a future Tier-1 pass must be read as conditional.
- **Contamination can masquerade as signal** (§2.4). With >94% of SWE-bench issues predating common knowledge cutoffs [Aleithan2024], a sensor can appear to carry outcome bits because it memorized the answer. The framework's defenses (overlap audit, counterfactual cutoff, cross-panel triangulation) reduce but do not eliminate this risk. Notably, in the *current* instantiation the panel is information-*starved* (§7.3), so contamination is not inflating the present result — but it becomes a first-order concern the moment the panel is strengthened and `I(panel;oracle)` rises.

## 10.3 Limitations of the substrate test

- `I(panel; oracle)` is estimated, not computed exactly. The estimators used are lower bounds or conservative (§5.4); their *agreement on insufficiency* is robust because lower bounds can only understate `I`, but a future *sufficiency* claim (high `I`) would demand tighter estimation and a contamination audit before it is trusted.
- The `τ_MI = 0.95 bits` threshold assumes a near-balanced binary outcome (`H(Y) ≈ 1 bit`); for skewed base rates the threshold must be restated relative to `H(Y)`, and per-cell base rates must be tracked (§5.5).
- The test bounds *what is recoverable*; it does not by itself construct the recoverer. A sufficient panel is necessary, not sufficient, for a working predictor — training, calibration, and the per-cell/stability/cross-panel gates (§3) remain to be passed after the substrate gate.

## 10.4 The kernel result is a structure claim, not a fixed number

The dictionary ≈1% figure [VincentLamarre2016] is specific to dictionary-graph topology (small-world, scale-free). There is **no guarantee** a domain's code-association graph has that topology, so the Python grounding kernel could be 1%, 10%, or — if the panel were insufficient — effectively 100%. The paper claims the *structure* (anchor + small grounded kernel + association) is domain-general, and that the kernel *fraction* is a measurable property of each domain's graph — not that ≈1% transfers. In the current instantiation the kernel question is moot until the panel clears the substrate gate (§7.3).

## 10.5 What is permanently out of scope

- **Domains without a deterministic oracle** (aesthetics, open-ended dialogue quality, "good judgment"): ineligible by §3.7, because there is no anchor.
- **Ambiguous prediction surfaces** that have no free binary form against reality: excluded from the binary predicate; they may be informative displays but are not part of the super-intelligence claim.
- **Generative substitution for the oracle:** the system predicts the oracle's verdict; it does not generate the artifact or simulate the oracle in a way that could leak the answer (Tier-4, §3.5).
- **Threshold relaxation as a path to passing:** lowering `τ_corr` to fit a weak substrate is a definition change, not progress (§3.4).

## 10.6 Threats to validity, summarized

| Threat | Where it bites | Mitigation status |
|---|---|---|
| Oracle invalidity (weak tests, over-specification) | the entire claim | **open** — must be measured per corpus |
| Contamination / leakage | apparent panel sufficiency | guarded (audit + cross-panel); not yet binding because panel is starved |
| Estimator error in `I(panel;oracle)` | substrate verdict | robust for *insufficiency*; needs tightening for any *sufficiency* claim |
| Topology assumption on the kernel | kernel-size expectations | claimed only as structure, measured per domain |
| Goodhart on proxy metrics | reported progress | cross-panel guard (T1.3); rank-lift rejected (§7.5) |
| Single-domain evidence | generality of the framework | framework is domain-agnostic by construction; only Python is instantiated here |

The honest one-line summary: *the method is general and the substrate result is solid, but the super-intelligence claim for any domain is conditional on an oracle whose validity must itself be measured — and for the Python instantiation that measurement is still outstanding, while the substrate gate is currently, and informatively, failed.*
