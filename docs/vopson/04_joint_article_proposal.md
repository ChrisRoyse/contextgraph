# 04 — Joint Article Proposal (one-pager to send AFTER the talk is scheduled)

**Purpose:** Vopson already floated a joint article ("if we could clearly identify the behaviour you mentioned… I propose we should write a joint article"). This is the concrete one-pager he can react to. **Don't lead with it** — let the talk build trust, then send this. He is Editor-in-Chief of IPI Letters, so the publication path is short.

---

## Working title
**"The Second Law of Infodynamics in a Self-Optimising Learning Machine: Information-Entropy Reduction During Mistake-Driven Verifier Training"**

*(Alt: "A Third Substrate for the Second Law of Infodynamics: Digital Storage, Genomes, and Now Learning Machines.")*

## Proposed authors
Chris Royse (Leapable.ai) · Melvin M. Vopson (University of Portsmouth / IPI). Add Serban Lepadatu if he wants the second-law co-author involved.

## Target venue
**IPI Letters** (fast, he's EiC) for the first communication; **AIP Advances** if the full experiment lands a clean result and he wants the bigger venue. His own second-law papers are in AIP Advances — that's the aspirational home.

## The one-sentence thesis
The second law of infodynamics — *information entropy of a closed information system stays constant or decreases over time* (Vopson & Lepadatu, AIP Adv. 12, 075310, 2022) — has been demonstrated on digital storage and on RNA genomes. We test it in a **third, fully controllable substrate**: a self-optimising machine-learning verifier whose internal state is an array of frozen per-channel measurement vectors, and we report whether information entropy falls (and channel symmetry rises) as the verifier learns.

## Why it's novel and worth co-authoring
1. **New substrate.** Storage and genomes are the only published demonstrations. A learning machine is qualitatively different — and unlike a genome or the cosmos, it is **replayable and controllable**: thousands of runs, controlled ablations, logged certificates.
2. **Falsifiable both ways.** Pre-registered confirmation *and* falsification criteria (`03 §C`). Positive result extends the law's domain; negative result bounds it. Either is publishable.
3. **It connects two literatures** — Vopson's infodynamics and Schmidhuber-style compression-progress / JEPA world models — that have not been joined.
4. **It already has instrumentation.** The compression-progress metric `CP_Φ` is logged today (`CF_MEJEPA_TRAIN_CERTS`); the cross-channel correlation/MI tensor exists (#388). This is not a from-scratch build.

## Hypotheses (pre-registered)
- **H1:** `Σ_i H_i(t)` (summed per-channel information entropy) decreases monotonically toward a floor over training windows.
- **H2:** the entropy decrease **tracks** the rise in predictor–oracle correlation (zero-lag positive cross-correlation).
- **H3:** trained channel configurations are **more symmetric** (lower information entropy) than untrained — operationalising Vopson 2023's "high symmetry ⇔ low information entropy."
- **H0 (falsification):** correlation rises while entropy is flat/rising → law does not govern this learner.

## Methods (full recipe in `03 §C`)
Substrate: 300×8 SWE-bench Lite, frozen 15-channel panel, deterministic Docker oracle. Per-window logging of `H_i(t)` (same-channel Kraskov estimator), `CP_Φ(t)`, symmetry score, oracle correlation. Controls: shuffled-label run, frozen-predictor run. All certificates released.

## Division of labour
- **Royse:** system, instrumentation, training runs, raw logs, engineering write-up.
- **Vopson:** information-theoretic measurement design to a physics standard, the infodynamics framing and derivation, connection to the symmetry/cosmology results, venue/review.
- **Joint:** interpretation, falsification adjudication, the paper.

## Honest scope statement (put this in the proposal — he'll respect it)
- This is **not** a claim that the verifier is finished (it's at 0.48 Pearson vs a 0.95 gate). The infodynamics experiment is **orthogonal** to deployment readiness — it studies the *training dynamics*, which exist and are loggable now.
- This is **not** a claim about consciousness, souls, simulation-hacking, or "the Oracle." It is a measurement of information entropy in a learning system. (The internal `INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md` §4 explicitly fences these out — cite that discipline to him; it signals you police your own over-reach.)

## Stretch collaborations (flag as optional/speculative)
- Apply **Gates-style self-dual error-correcting codes** to the 15-channel panel occupancy (your #512) — Vopson cites the adinkra/ECC simulation work; this is a shared interest with real math behind it.
- A short note on whether a measurably entropy-reducing learner has any energy/Landauer signature relevant to the mass–energy–information program (clearly labelled speculative).

## The ask in the email
> "You suggested a joint article if the infodynamics behaviour shows up in the data. Here's a one-page proposal with pre-registered hypotheses and a falsification condition. If the framing works for you, I'll run the experiment and send raw logs; IPI Letters seems the natural first home. Happy to adjust scope to whatever you'd co-author."
