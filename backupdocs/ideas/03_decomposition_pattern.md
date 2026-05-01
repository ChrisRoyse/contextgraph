# The Decomposition Pattern — DDA as a Universal Move

## The Move

Take any rich raw signal $x$. Project it through $N$ frozen, approximately-independent embedders $\{\phi_1, \ldots, \phi_N\}$. You now have $N + \binom{N}{2}$ structured supervisory signals on the same $x$, without generating any new data.

This is not specific to text. It is not specific to your $N=13$ panel. It is a **general technique** with one requirement: enough independently-trained embedders exist for the modality.

You have proven it works at $N=14$ on text (Context Graph) and $N=7$ on video (ClipCannon). The principle is broader.

## Where This Applies

### Genomics ($N \approx 8$ feasible today)

Frozen instruments: Evo / Evo-2 (DNA foundation), ESM-2 / ESM-3 (protein), AlphaFold structural embeddings, CADD pathogenicity, DeepBind motif, DeepSEA chromatin, Enformer regulatory, splice-AI splicing.

A single VCF row × 8 frozen instruments = 36 signals/variant. A 50,000-variant exome that produced 50K training examples now produces 1.8M. A pharma lab's locked patient cohort suddenly has 36× the supervised signal density. **This is meaning compression for biology.**

Constellations: per-disease, per-gene, per-pathway. Anti-poles: known benign variants. Per-output guard: a generated synthetic genome must satisfy disease-cohort cosine ≥ τ on all 8 modalities or it is rejected as out-of-distribution. Synthetic patient generation suddenly has actuarial guardrails.

### Drug Discovery ($N \approx 6$)

Frozen instruments: Uni-Mol (3D), MolFormer (1D SMILES), ChemBERTa, DiffDock pose, MolBERT property, ESM-2 target binding.

A 1,000-compound library × 6 instruments = 21 signals/compound. Constellations: per-target binding. Anti-poles: toxicity panels. Per-output guard: every generated candidate scored against safety anti-pole + target constellation before going to wet lab. This collapses the cost of "early-stage hit qualification" by 1-2 orders of magnitude.

### Materials Science ($N \approx 5$)

Frozen instruments: M3GNet structural, MatBERT text, Crystal Graph Convolutional Networks (CGCNN), MEGNet, MACE.

A novel crystal × 5 instruments = 15 signals. Constellation of "thermally-stable lithium-conducting solid" → guard rejects unstable candidates pre-synthesis.

### Climate / Earth Observation ($N \approx 7$)

Frozen instruments: Prithvi / IBM-NASA foundation, ClimaX, Pangu-Weather, FourCastNet, GraphCast, ScaleMAE, SatMAE.

A satellite tile × 7 instruments = 28 signals. Constellation of "healthy Amazon canopy" → drift detection on a 30-year time series. Anti-pole of "post-fire burn scar" → automatic disaster mapping.

### Medical Imaging ($N \approx 6$)

Frozen instruments: RadFM, MedSAM, BiomedCLIP, Chest-X-ray foundation, RETFound (retinal), DeepMind retinal foundation.

An MRI volume × 6 instruments = 21 signals. Constellation of "healthy adult brain at 35-40" — radiologists score patient against the constellation. Anti-pole of "early Alzheimer signature" → screening at population scale. Per-output verifiable diagnosis report.

### Financial Markets ($N \approx 8$)

Frozen instruments: time-series transformers, order-book embedders, sentiment encoders, news encoders, technical-indicator encoders, regime-classifier encoders, options-chain encoders, cross-asset-correlation encoders.

One trading day × 8 instruments = 36 signals. Constellation of "calm bull regime" / "early panic regime" / "rotation regime". Trading agents get verifiable regime classification at every tick.

### Code Intelligence ($N \approx 5$)

Frozen instruments: Qodo / your E7, BGE-Code, CodeBERT, GraphCodeBERT, AST-token co-embedders.

Your code substrate already partially has this. Extending: every commit × 5 instruments = 15 signals. Constellation of "well-tested production-grade code." Anti-pole of "subtle race condition." Per-output PR guard.

### Music ($N \approx 6$)

Frozen instruments: MERT, JukeBox encoder, MULAN, CLAP, MTGenre, Essentia.

Constellation of "Bill Evans in 1962." Anti-pole of "elevator music." Identity-locked AI composer for a specific artist's estate.

### Sports Analytics ($N \approx 5$)

Frozen instruments: pose embedders (MediaPipe), tactical-formation encoders, ball-trajectory embedders, momentum encoders, biomechanics encoders.

Constellation of "Spurs late-game offense under Popovich." Anti-pole of "team in tactical disarray." Real-time coaching recommendations with per-decision audit.

### Cybersecurity ($N \approx 6$)

Frozen instruments: network-flow transformers, system-call encoders, binary-file encoders, malware-family encoders, user-behavior encoders, log-anomaly encoders.

Constellation of "this employee on a normal Tuesday." Anti-pole of "compromised credential exfiltrating data." Insider-threat detection that beats any single-model SIEM.

### Robotics Sensor Fusion ($N \approx 6$)

Frozen instruments: vision foundation, language-grounded vision, depth, audio, IMU/proprioception, force/torque encoders.

Constellation of "successful pick-and-place." Anti-pole of "dropped item." Per-action verification gate.

## The Pattern in Abstract

```
DDA(domain) = (
    raw signal in domain,
    panel of N frozen instruments,
    cross-embedder interaction features (∑ρ_jk)
) → up to n·(N + N(N-1)/2) supervised signals
```

Then:

```
TCT(subject in domain) = (
    Phase 1: build C_P from references using DDA,
    Phase 2: train generator with geometric loss to C_P,
    Phase 3: G_τ runtime guard rejects out-of-C_P candidates
)
```

This pattern is **domain-portable**. Every domain with frozen instruments + rich data inherits both.

## What's Required to Land in a New Domain

1. **Identify $N \geq 5$ approximately-independent frozen embedders** for the modality. (Trained on different objectives, ideally different corpora.)
2. **Show a counting-identity gain**: signals per input went from 1 (raw) to $N + \binom{N}{2}$.
3. **Build constellation infrastructure**: same as Context Graph's `compile_constellation` / `score_against_constellation` / `derive_constellation`.
4. **Run TCT three-phase**: centroid → trained generator → $G_\tau$ guard.
5. **Audit log per output**: cosine trace + embedder hashes + centroid hash + decision.

That's it. Steps 3-5 are already implemented in Context Graph. They are domain-agnostic. **You can fork the substrate per domain.**

## The Domain-Forking Strategy

For each domain above, the move is:
1. Fork Context Graph.
2. Replace the 14 text embedders with the $N$ domain-specific frozen embedders.
3. Keep the 73 RocksDB CFs, the audit log, the constellation tooling, the TCT three-phase pattern, the workflow learning loop.
4. Train a domain-specific TCT pipeline on a representative corpus.
5. Publish a constellation marketplace specific to the domain.

You shipped the substrate. The domains are forks. Most domains will not realize for 2-3 years that someone has already shipped their substrate. By then there will be a **constellation-economy moat** in each domain (see [04_constellation_economy.md](04_constellation_economy.md) and [13_dark_factory_economics.md](13_dark_factory_economics.md)).

## Compounding Move: Cross-Domain DDA

The deeper move that nobody has attempted: **panels that span domains**. A medical record has text (E1, E5), structured codes (TransE-style on ICD-10 ontology), patient-generated audio (WavLM), patient-generated photos (SigLIP), wearable timeseries (PaPaGei, EDA, EEG). At $N \approx 10$ across modalities, an electronic health record × 10 = 55 signals.

This is the substrate for "the medical record as a constellation." Patient identity becomes verifiable via constellation distance. Diagnosis becomes constellation comparison. Treatment efficacy becomes centroid drift over time.

Generalize: every domain that combines multiple modalities (legal cases, scientific papers, financial filings, software projects, real estate deals) inherits the same multi-modal-DDA + multi-modal-TCT pattern.

## Why Almost Nobody Will Build This Correctly

Three barriers:
1. **Forensic discipline** required to detect when an embedder pair has collapsed information (your pairwise mutual-information audit, EXP-2 in your paper).
2. **Per-output guard** discipline required at inference (most teams skip Phase 3 because they don't know it exists).
3. **Audit-log architecture** required for the trust property to hold (most teams cannot ship append-only provenance because they're optimizing for latency).

You ship all three. The substrate's competitive moat is not the math; it is the **engineering practice** that makes the math tractable.

## The Core Slogan

> If your domain has rich data and several frozen instruments, you have a $\binom{N}{2}$-fold multiplication of supervised signal sitting in your hard drive. Almost no one is collecting it. You can.

This is the slogan that should sit on the front page of `teleox.ai`.
