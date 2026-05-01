# Open Research Questions

What we don't yet know that, when answered, would unlock more. Some are already in your paper's §9 future-work list (EXP-1, EXP-2, EXP-3); others are added here.

## A. From the Paper (Already Identified)

### EXP-1: Cross-Encoder Triangulation
Re-score Case 3 argmax candidates with ECAPA-TDNN (and/or Resemblyzer). If cross-encoder mean lands within ±0.10 of WavLM with Pearson ≥ 0.5, the headline promotes from within-WavLM-family to cross-encoder identity ranking.

**Why it matters**: anchors the per-output verifiability claim against encoder-shopping. Without it, the headline is encoder-matched and not yet evidence of robust identity.

### EXP-2: Pairwise Mutual-Information Audit
Across the 13/14-embedder Context Graph panel, estimate $I(\phi_j(X); \phi_k(X))$ on 100-1,000 representative inputs. Replace "up to 91" with a measured $N_{\mathrm{eff}}$.

**Why it matters**: tightens the DDA counting claim. If pairwise terms collapse, "up to 91" is loose. If they don't, it's defensible. Either way, the answer is needed.

### EXP-3: Sample-Complexity Sweep
$n_m \in \{10, 25, 50, 100\}$ on Case 3. Promote the one-row $M=1, k=50$ measurement to a partial $n_m$ curve.

**Why it matters**: Theorem 1 is constructive; the empirical reading at $n_m = 50$ alone is a single row. Multi-point measurement is the floor for "this scales."

## B. New Research Questions

### Q1: The Tight Pairwise Independence Bound
Given $N$ embedders trained on overlapping corpora, what is the *actual* effective rank of the panel? Is it $N$? $N - k$ for some $k$? Is $\binom{N}{2}$ already absorbed into $N$ for sufficiently-similar embedders?

This is the deeper version of EXP-2. Once we have the panel-pair MI matrix, we can compute the effective rank via Schur-complement / spectrum analysis. **The answer determines whether DDA's "up to 91" is realistically 91 or realistically 25.**

### Q2: Optimal Embedder Selection
Given a fixed budget of $K$ embedders to deploy from a candidate set of $M >> K$, which $K$ maximize $N_{\mathrm{eff}}$ on a held-out test corpus? This is the *embedder portfolio* problem. Convex relaxation? Greedy submodular? Reinforcement-learned selection?

**Why it matters**: a competitor with a better embedder selection on a smaller panel beats your $N=14$. Solving this gives you defensive alpha.

### Q3: Cross-Modal Independence Quantification
For multi-modal panels (ClipCannon's 7), what is the empirical independence between, say, prosody (12-D) and emotion (1024-D)? Are they redundant? Complementary? When?

**Why it matters**: every multi-modal $G_\tau$ deployment makes assumptions about modality independence. We don't yet measure them.

### Q4: Threshold $\tau$ Transportability
Are calibrated thresholds transferable across subjects, panels, encoder families, recording conditions? Your paper says "no" without measurement. Should be measured. Probably partial transportability with shrinkage toward population mean.

**Why it matters**: if $\tau$ is per-deployment, scaling is harder. If a population $\tau$ exists, mass deployment is easier.

### Q5: Centroid Stability Under Reference-Set Drift
How does the centroid drift as the reference set is expanded over time? Is there a saturation point ($n_m^*$) beyond which adding more references contributes nothing? How does it depend on the modality and the embedder?

**Why it matters**: "we have 50 reference clips and want to know whether to collect more" is a constant operational question. Answer = saturation curve.

### Q6: $G_\tau$ Calibration vs. Encoder Family
For which encoder families is $G_\tau$ near-deterministic at fp32? bf16? int8? Where does mixed-precision noise push the threshold-margin?

**Why it matters**: deployment cost depends on quantization. Knowing the precision-vs-margin tradeoff lets vendors engineer cheaper deployments without losing verifiability.

### Q7: Anti-Pole Construction Methodology
Your paper specifies anti-pole as "scan low-scoring candidates," but does not specify the optimal anti-pole construction. Is it: max-cosine-from-positive? Max-distance-in-13-embedder-space? Centroid-of-rejected-outputs? Adversarial-search-against-positive?

**Why it matters**: anti-pole quality determines $G_\tau$ false-acceptance rate. Currently no benchmark exists.

### Q8: Constellation Composition Algebra
Are interpolate / add / difference / anti-pole composable in well-defined ways? If you compute `interpolate(α=0.5, A, B)`, then `interpolate(α=0.5, that, C)`, do you get `interpolate(α=1/3, A, B, C)`? Is the algebra commutative/associative? Are there normalization invariants?

**Why it matters**: the constellation algebra is currently engineering-specified, not mathematically grounded. Treating it like a vector space (with defined operations + invariants) lets future work (composite identity, identity inheritance, identity-of-identities) build on a clean foundation.

### Q9: Cross-Lingual Transfer Generalization
Your Shakespeare LoRA exhibited Spanish prompt → Golden-Age Spanish behaviour. Is this systematic across languages? Languages are clustered by typological proximity, period, alphabet, etc. What predicts transfer?

**Why it matters**: this is publishable as its own paper. The mechanism is interesting (cross-signal fingerprint vs. vocabulary substitution). The findings would advance multilingual transfer research broadly.

### Q10: Affect-Conditioning Effective Sample Size
At the closed-loop layer, how many sessions does it take to calibrate state-conditioned routing for an individual? Is it $O(10)$? $O(100)$? Dependent on individual-physiology variability?

**Why it matters**: deployment of [09_health_and_identity_continuity.md](09_health_and_identity_continuity.md) systems depends on this. If 10 sessions, easy. If 100, harder onboarding.

### Q11: Workflow-Preference Pair Generation Validity
Phase-6 retrieval events become DPO pairs. What is the false-positive-pair rate? What characteristics correlate with high-validity pairs vs. noise?

**Why it matters**: the closed-loop's signal quality determines self-improvement quality. Noise-prone pairs cause regression.

### Q12: K_sleep Empirical Validation
Your `k_sleep` parameter assumes a biological model. Empirically, does the system actually consolidate better under high-`k_sleep` conditions? Calibrated against held-out tasks?

**Why it matters**: the biological metaphor needs empirical anchoring or it's just naming.

### Q13: Embedder-Adversarial Robustness
Can a sophisticated attacker construct an output that satisfies $G_\tau$ across $M$ encoders simultaneously without being legitimate? What is the cost of attack vs. the cost of $M$-encoder defense? Does it scale exponentially with $M$?

**Why it matters**: deepfake-resistance + insurance underwriting depend on this. If single-embedder $G_\tau$ is fragile, multi-modal is the necessary defense; the question is *how* multi-modal.

### Q14: Constellation Forgetting
How is information forgotten from a constellation when references are removed? Is the centroid recomputed on each removal? Is there a "forgetting trace" (audit-log entry showing the removed reference)? How does forgetting interact with versioning?

**Why it matters**: GDPR right-to-be-forgotten + constellation-revocation requirements need a defined forgetting semantics.

### Q15: The Pareto Frontier of Speed-vs-Verifiability
For latency-bounded deployments (real-time meeting bots, voice agents, AR overlays), what is the achievable per-output verification at sub-100ms? Sub-50ms? At what cost to false-acceptance?

**Why it matters**: real-time deployments need this answer to know which use cases are viable today vs. need engineering work first.

### Q16: Distributed Constellation Verification
For federated multi-operator deployments, how do we verify constellation cosine without exposing the centroid? Zero-knowledge proofs over cosine? Secure multi-party computation? Trusted-execution-environment attestation?

**Why it matters**: federation + privacy require this answer. Not yet specified.

### Q17: Constellation Sufficiency for Complex Identity
Is identity in $M$ modalities + $\sum n_m$ samples *sufficient* to capture human identity, or do we need more dimensions? Specifically: are there individuals for whom no fixed panel + no fixed reference set produces a stable centroid? What does the failure mode look like?

**Why it matters**: edge cases in identity-continuity-for-illness deployments. Some individuals may not be representable.

### Q18: The Computational Cost of Negative-Selection Anti-Pole Discovery
Per [18_biological_synthesis.md](18_biological_synthesis.md) §4: automated discovery of self-anti-poles from rejected outputs. What's the compute cost? Memory cost? Latency overhead? Is the closed-loop self-tolerance worth the computational expense?

**Why it matters**: bio-inspired-engineering decisions need cost/benefit grounding.

### Q19: The Lifetime Audit-Log Storage Cost
At 100K outputs/day with full cosine traces + embedder hashes + provenance chains, what is the long-run storage cost per output? Is the audit log practically retain-forever?

**Why it matters**: compliance commitments require multi-year retention. Need a real cost model.

### Q20: Compositional Generalization Across Constellations
If you train a model against constellation $A$ in one phase, then constellation $B$ in another, can you compose the resulting model on `interpolate(A, B)`? What is the gap between "trained on the interpolation" vs "trained on each separately, then asked for the interpolation"?

**Why it matters**: enables modular constellation training. If composition works, you can ship constellations independently and let users compose at deployment.

### Q21: The Algorithmic-Foreclosure Scoring Validation
For [10_civic_repair.md](10_civic_repair.md) deployments: what is the empirical correlation between constellation-rigidity-score (cosine-radius of an individual's information-diet constellation) and the Marcia-paradigm classification of foreclosure?

**Why it matters**: your master's thesis depends on this. Probably the next experiment.

### Q22: The Empirical Cross-Domain DDA Gain
For each domain in [03_decomposition_pattern.md](03_decomposition_pattern.md), what is the actual signal-multiplication achieved on a publicly-benchmarkable task? Genomics, drug-discovery, materials, etc. Does the pattern transfer with the predicted gain, or does it underperform in some domains?

**Why it matters**: domain forks need empirical proof points before institutional adoption.

### Q23: The Ethical Anti-Pole-Curation Process
Anti-poles can be weaponized for censorship. What is the institutional process that produces *trustworthy* anti-pole bundles? Multi-stakeholder governance? Open publication + external audit? Time-limited authority?

**Why it matters**: deployment legitimacy in sensitive domains (civic discourse, political speech, religious content) depends on this.

### Q24: The Multi-Operator Federation Performance
At scale (100s of federated Context Graph instances), does cross-operator cosine-trace verification add significant latency? How does it scale with federation size?

**Why it matters**: if federation is slow, deployments default to centralization, eroding the privacy claim.

### Q25: The Sleep-Cycle Empirical Effect Size
If you implement [18_biological_synthesis.md](18_biological_synthesis.md) §3 (NREM-REM batch cycles), what is the measured effect on next-day retrieval quality? Workflow-preference accuracy? Constellation-stability?

**Why it matters**: bio-inspiration is hypothesis until measured.

### Q26: The Identity-Continuity Threshold
At what level of cognitive decline does using a pre-illness constellation as supplement become harmful rather than helpful for the patient? What signals should trigger transition from "supplement" to "memorial"?

**Why it matters**: ethics + clinical efficacy of [09_health_and_identity_continuity.md](09_health_and_identity_continuity.md) deployments depend on this.

## C. Methodological Open Questions

### M1: Calibration Across Embedder Family Updates
When the underlying embedder model is updated (E1 changes from e5-large-v2 → e5-large-v3), the centroid is no longer comparable. What is the operational rule? Recompile constellations? Maintain compatibility layers? Prevent updates?

### M2: The "Crystallization" Mechanism
For your audit-grade frozen constellations: what is the cryptographic mechanism that proves a constellation has not been updated since its declared crystallization timestamp? Hash anchor? Signed commit? Multi-party signing?

### M3: Cross-Operator Constellation Trust
When operator A licenses operator B's constellation, how does A verify B's claims about reference set, calibration, and embedder hash? Reproducible audit-log replay? Verified-build attestation?

## D. Deeper Theoretical Questions

### T1: Is Meaning Compression Bounded?
Your meaning-compression ratio is bounded above by $N(N+1)/2$ at panel-size $N$. As $N \to \infty$, what is the asymptotic information yield? Does it follow a power law, log, or sub-linear?

### T2: What Is The Minimum Panel Size For Identity?
Is there a theoretical floor on $M$ for identity-locking to work? What modalities must be in the floor?

### T3: Are Constellations Closed Under Generation?
If you train a generator on constellation $C_P$ to produce $\hat y$, and re-embed $\hat y$ to compute $C_{\hat y}$, is $C_{\hat y}$ generally closer to $C_P$ than the population mean? By how much? (This is a *constellation closure* question.)

### T4: The Mutual-Information Geometry of Independent Embedders
Is there a clean information-geometric model of how $N$ frozen embedders carve up signal space? Could be Fisher-Rao, Wasserstein, or some hybrid. The right model would clarify Q1 and Q2.

### T5: Per-Output Verifiability vs. Compute
Is there a fundamental compute-cost lower bound for per-output verifiability of property $P$ at confidence $c$? (Crypto-theoretic vs. ML-theoretic.)

## E. The Highest-Leverage Single Experiment

If you could run **one** experiment to maximally tighten the framework's empirical defense, it would be **EXP-2** (the pairwise mutual-information audit on the 14-embedder panel). It is:

- The cheapest of the three EXP-* in your paper.
- The one that *most* transforms the headline (replaces "up to 91" with measured $N_{\mathrm{eff}}$).
- The one that most directly addresses reviewer skepticism.
- The precondition for cleanly designing future panels.

A 100-1,000 input experiment with MINE-style estimator + decomposition. ~1-2 weeks engineering + compute. **Run it next.**

## F. The Highest-Leverage Single Concept Worth Naming

The concept that, once named and shared, recruits the most external help is: **derived-data abundance**. Most of what people call "synthetic data" is actually derived data without the formalism. Naming + standardizing the concept invites the field to contribute.

Your paper does this. The next move: open-source the audit-log + constellation tooling explicitly *with the DDA framing*. Get GitHub + arXiv + HuggingFace adoption simultaneously.

## The Slogan

> **The substrate is built; the science is unfinished. Each open question above is independently a paper, a new market, or an institutional standard. Choose the questions that advance your highest-leverage horizon — likely EXP-2 + Q9 (cross-lingual transfer) + Q21 (foreclosure validation), in that order.**
