# Learn Plan — Operationalising the Unified Theory of Learning with Embedders

**Status:** Living document. Built from Steve's UTL v2 (`../UTL_v2_embodied.md`), Chris's Context Graph + ClipCannon + DDA/TCT framework (`../PAPER.md`, `../alldata.md`, `../leapableai_youtube_transcripts.md`), the current Context Graph / ClipCannon implementation specs, and a sweep of 2024–2026 peer-reviewed literature on biosignal foundation models, multimodal learning analytics, knowledge tracing, and ML-training dynamics.

**Scope.** Two parallel deliverables:

1. **Measure learning in *humans*** using a multi-embedder panel that operationalises UTL v2's three-factor product `L = ΔS · ΔC · ΔE` and its consolidation integral `M(t)`. The substrate is Chris's production 14-embedder Context Graph content registry plus seven new biosignal/affect learner-state embedders (E15-E21). E14 is the existing `BgeM3Dense` content embedder. The TCT paradigm (frozen centroid + per-output cosine guard) maps cleanly: a learner's "state" is a point in multi-modal embedding space, the "regulated learner" is the frozen centroid, and the runtime guard fires when state drifts outside threshold.
2. **Measure learning in *AI systems*** by reading the same three terms off training-time and inference-time signals: `ΔS` from prediction-error / surprisal / loss residuals, `ΔC` from representational stability (CKA, SVCCA, sparse-autoencoder feature emergence), `ΔE` from training-dynamics health (loss-landscape curvature, gradient noise scale, edge-of-stability, plasticity loss). The same embedder-comparison machinery that diagnoses a human learner can diagnose a training run.

The thesis carried throughout: **UTL is the same mathematical object regardless of substrate. What changes is the choice of embedder for each term.**

---

## File Index

| # | File | What's in it |
|---|------|--------------|
| 00 | [README](00_README.md) | This index; executive summary; scope and disclosures |
| 01 | [Theory audit and optimisation](01_theory_audit.md) | What survives in UTL v2, what needs Dispenza-decoupling, refinements with 2024–2026 evidence |
| 02 | [Human state embedders for ΔE](02_human_state_embedders_delta_E.md) | PaPaGei (PPG), LaBraM/EEGPT/CBraMod (EEG), Wav2Vec2-emotion (affect), OpenFace 3 / py-feat / MARLIN (face), cortisol proxies, full table with VRAM and access status |
| 03 | [Surprise and coherence embedders](03_surprise_and_coherence_embedders.md) | ΔS via transformer surprisal + cosine prediction error; ΔC via multi-embedder agreement / RRF / contradiction detection; how to compute both from Context Graph's production E1-E14 content embedders without new infra |
| 04 | [Consolidation and state-transfer](04_consolidation_and_transfer.md) | The `M(t)` integral and `T(train, deploy)` term, including how to log it; spaced-repetition (FSRS, transformer SRS) integration; sleep / retrieval-practice scheduling |
| 05 | [AI learning as UTL](05_AI_learning_as_UTL.md) | The mapping from training dynamics to ΔS/ΔC/ΔE, applied to LoRA fine-tuning, RLHF/DPO, continual learning, and inference-time generation. Concrete reads on Chris's Shakespeare LoRA and Santa LoRA runs |
| 06 | [Data pipeline and datasets](06_data_pipeline_and_datasets.md) | WESAD, AffWild2, DAiSEE, EngageNet, MSP-Podcast, IEMOCAP, MAHNOB, ASCERTAIN, AMIGOS, SEED, DEAP — what's needed to bootstrap each embedder; ethics; consent; deployment-grade hardware |
| 07 | [Context Graph integration](07_context_graph_integration.md) | New RocksDB column families, learner-state embedder slots E15-E21, MCP tools, fingerprint extension, weight profiles, persistent learner constellations |
| 08 | [Falsification and experiments](08_falsification_and_experiments.md) | Seven pre-registered experiments that would either confirm UTL v2's predictions in measurable terms or kill them; what each requires |
| 09 | [Implementation roadmap](09_implementation_roadmap.md) | Phased plan (Phase 0 minimum viable measurement → Phase 4 full deployment), milestones, RTX 5090 VRAM budget, deferred items |
| 10 | [Integrated findings from Chris's existing stack](10_integrated_findings.md) | Findings added after reading the full local corpus: provenance-first architecture, zero-trust verification, AI-native education, productization constraints, and open engineering questions |
| 11 | [Canonical embedder matrix](11_embedder_matrix.md) | Runtime-aligned E1-E21 matrix, E14 collision resolution, source-of-truth definitions |

---

## Executive Summary (read this first)

### What Steve's theory says, condensed

`L = ΔS · ΔC · ΔE` (instantaneous learning) and `M(t) = ∫ L · ΔE · k(τ) · e^{-(t-τ)/τ_decay}` (consolidated mastery). Three multiplicative factors, range-clamped to [0,1], with a transfer factor `T = exp(-d(state_train, state_deploy)/λ)` for deployed performance. Surprise (ΔS) extends to mental rehearsal at fidelity γ ≈ 0.7. Coherence (ΔC) gains an HRV-resonance term. Embodiment (ΔE) is geometric over plasticity-window × valence-arousal × stress-floor. Predictive prior `predicted = (1-β) · f(history) + β · future_self` with bounded adaptive β ∈ [0.05, 0.20]. Reward = 0.6 · confidence + 0.4 · mean(L · ΔE).

### What survives external scrutiny

The **mathematical structure is sound and falsifiable** (Section 01). The peer-reviewed substrate for individual terms is genuinely strong:

- Motor-imagery plasticity (Pascual-Leone, Ruffino, Schuster) — robustly replicated.
- HRV biofeedback effects (Lehrer 2020 meta-analysis; Goessl 2017; Stockman 2024) — moderate-to-strong.
- Cortisol-memory inverted-U (Lupien, Sherman, Goldfarb, de Quervain) — robustly replicated.
- Sleep-gated consolidation (Diekelmann & Born, Walker, Cousins 2023) — textbook.
- State-dependent retrieval (Godden & Baddeley) — textbook.
- Implementation intentions (Gollwitzer & Sheeran, Yeager 2019) — moderate effect.

### What needs cleanup

- **Decouple from Dispenza-as-source.** Dispenza has one peer-reviewed paper in 40 years; the substantive references (HRV biofeedback, motor imagery, cortisol-memory) all stand on their own. Cite the primary literature; drop "Dispenza-validated" framing.
- **Decouple from the Free Energy Principle as load-bearing.** FEP is, per multiple 2024–2025 critiques, *unfalsifiable* in the formal sense — Karl Friston's own framing acknowledges it is a principle, like Hamilton's principle of stationary action. Operationalise with **transformer surprisal** (which has tractable issues but is at least falsifiable) instead of free-energy minimisation.
- **The β adaptive rule needs an empirical anchor.** The implementation-intention literature gives us bounds, not the update rule.
- **The differentiated-gating hint at the end of Part VIII is the right form of L.** v3 should make it the default rather than an afterthought.

### What the embedder programme adds

Section 02–05 turn the abstract terms into measurable quantities. The headline observation is that **every term in UTL v2 has a 2024–2026 frozen foundation model that can produce it**:

| UTL v2 term | Frozen embedder candidate | Output |
|---|---|---|
| `plasticity_window` (EEG state) | LaBraM / EEGPT / CBraMod | scalar in [0,1] from band-power features |
| `HRV_coherence` | PaPaGei (PPG, ICLR 2025) | scalar in [0,1] from spectral resonance |
| `valence_arousal` (face) | OpenFace 3.0 / py-feat / MARLIN | 2D vector (valence, arousal) |
| `valence_arousal` (voice) | Wav2Vec2-large-MSP-DIM (audeering) | 3D (arousal, dominance, valence) |
| `stress_floor` (cortisol proxy) | WESAD chest physiology stress head on EDA / ECG / RESP / TEMP features | scalar stress-floor signal |
| ΔS (real prediction error) | Context Graph E1/E5/E12 cosine on (predicted, actual) | scalar in [0,1] |
| ΔS (simulated/imagined) | E1 cosine on (predicted, mentally rehearsed text) | scalar in [0,1] |
| ΔC (outcome stability) | Reciprocal-rank-fusion entropy across production content embedders | scalar in [0,1] |
| ΔC (coherence with prior) | cosine to learner constellation centroid | scalar in [0,1] |
| `future_self_vector` | E1 embedding of written future-self description | 1024-D |
| `M(t)` | per-trace integral persisted to new `CF_LEARNER_M_PER_TRACE` column family | scalar |
| `T(train, deploy)` | exp(-Euclidean distance over joint state vector / λ) | scalar |

**Result:** UTL v2 stops being a framework and becomes an instrument. Every learner gets a constellation in the TCT sense; every session is a trajectory through that constellation's neighbourhood; every gradient of `M(t)` is auditable. This is the same machinery Chris already runs, just with the centroid relabelled "regulated learner state" instead of "Santa".

### What the full local corpus adds

Reading the Context Graph, ClipCannon, OCR Provenance, PAPER.md, alldata, and YouTube-transcript corpus changes the implementation posture in five ways:

- **Provenance is load-bearing, not optional.** The OCR Provenance and ClipCannon stacks already treat every transformation as a hash-linked event. UTL should do the same for every learner observation, ΔS/ΔC/ΔE computation, M update, scheduler recommendation, and experiment result.
- **The first product shape is local-first.** Raw audio, video, EEG, PPG, and learner work products should stay on the learner's machine by default. Vendor-visible state should be limited to licensing/billing metadata and tamper-evident hashes unless the learner explicitly exports a research bundle.
- **Forensic engineering is the research method.** Chris's "guilty until proven innocent" workflow maps directly to UTL: every claim needs lineage, every threshold starts as default-pending-calibration, and every generated report carries the evidence chain that produced it.
- **AI-native education is the immediate testbed.** The transcripts' "answer is no longer a metric" / "100 holes" doctrine turns into Experiment 7: assess process artifacts, iteration quality, critique, provenance, and oral defense rather than answer strings alone.
- **Productization has regulatory edges.** EU AI Act Annex III treats several education/outcome-evaluation systems as high-risk, and EU guidance prohibits emotion-recognition systems in workplace and education contexts except narrow medical/safety cases. UTL's product form must therefore be learner-controlled, human-supervised, and compliance-by-design.

### What AI learning measurement adds (Section 05)

The same three terms read out of training dynamics:

| UTL v2 term | AI training analogue | Measurement |
|---|---|---|
| ΔS | per-batch cross-entropy residual; transformer surprisal of training tokens | log-likelihood |
| ΔC | representational stability (CKA, SVCCA between checkpoints); multi-embedder agreement on the model's outputs | scalar |
| ΔE | loss-landscape curvature (Hessian sharpness, edge-of-stability proximity); gradient noise scale; plasticity-loss measure (rank of feature representations) | scalar in [0,1] |
| `M(t)` | task accuracy on a frozen evaluation set; sparse-autoencoder feature counts (Anthropic monosemanticity) | rises with consolidation |
| `T(train, deploy)` | distribution-shift distance between training mixture and deployment query distribution | scalar |
| `future_self_vector` | LoRA target adapter, or a constellation centroid in TCT | direction in weight space |

Concrete payoff: this gives Chris a **principled, framework-grounded reading of his own LoRA training runs** that explains the Shakespeare echo bug (low ΔC across multi-embedder panel before the fix) and the EchoMimicV3 flow-matching sign bug (loss curve looked clean because `ΔS` was being computed on the wrong target — the model was minimising prediction error on `noise - latent` while the scheduler expected `latent - noise`, so ΔC against the held-out trajectory collapsed). UTL v2 generalises to: *if any of three signals goes silent, training looks healthy and isn't*.

### Data and feasibility

Section 06 itemises datasets (all open or research-licensed), Section 07 lays out the Context Graph integration (14 production content embedders + 7 learner-state embedders = 21 canonical embedders, +11 column families, +8 MCP tools), Section 08 lists falsification experiments, Section 10 captures the post-read synthesis from Chris's existing stack, and Section 11 is the canonical matrix. Total RTX 5090 VRAM budget: 24-28 GB usable, fits within Chris's existing GPU guard.

---

## Disclosures

- This is a **plan**, not a paper. Section 08 is the experiment list that would need to land before publication.
- Steve's theory is sophisticated but draws on the Dispenza brand. We keep the math, swap in primary citations, and disclaim direct attribution to Dispenza for any quantitative claim.
- The TCT framework's encoder-matched-scope discipline carries: a learner-state guard built on, e.g., Wav2Vec2-emotion is a within-Wav2Vec2-family identity-of-state agreement check. Cross-encoder triangulation (e.g., add a second emotion model from a different family) is a separate question and named out as future work.
- The free-energy / predictive-processing critique is real (Section 01) but does not invalidate UTL — it just means we should ground UTL on tractable measurable surrogates, which is what every other section does.

---

## Source provenance

Every claim in Sections 01–10 is anchored to a concrete URL, local implementation document, or peer-reviewed citation. Where 2024–2026 work exists, it is cited; where the load-bearing reference is older (e.g., Lehrer's HRV-B foundations), the older citation stands and the recent meta-analysis is added. The full source list per file appears at the bottom of each file under "Sources".
