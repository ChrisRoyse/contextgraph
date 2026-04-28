---
namespace: discoveries
agent_id: agent-10-theoretical-framework-analyst
timestamp: 2026-04-16T19:30:00Z
status: active
supersedes: none
tags: theoretical-framework, cluster-map, framework-compatibility, epistemology, framework-breaking, method-theory-alignment
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md
  - memory/handoff/agent-09-to-10--methodology-mapped.md
downstream_consumers:
  - agent-11-systematic-reviewer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Cluster-by-Cluster Theoretical Framework Map

This file complements agent-09's methodology scan with a **theoretical-framework**
(not methodological) analysis. A methodology is *how* a cluster measures; a
theoretical framework is *what* a cluster believes about the phenomenon. The two
are distinct: two papers can share a methodology while being theoretically
incompatible (e.g., RLHF and TCT both compute gradients on a generator, yet
differ radically on what alignment means).

**Scope discipline.** No new citations are introduced. Every framework anchor is
named by a citation already in the LOCKED master reference pool. Where the
citation is absent from our pool (e.g., a broader foundational reference for a
family), this is flagged explicitly as a gap for agent-11.

**Framework-compatibility codes.**
- **EXT** — our construct *extends* the cluster's framework along one axis the
  framework permits but does not enforce.
- **COMP** — our construct is *framework-compatible*: the cluster's theory
  neither predicts nor prohibits the construct; it is silent.
- **BRK** — our construct is *framework-breaking*: the cluster's theory would
  need to be amended (not merely extended) to accommodate the construct.
- **ORTH** — our construct is *orthogonal*: it lives on a different axis of the
  same research neighbourhood and does not interact with the cluster's theory
  at the level of first principles.

---

## Cluster A — Scaling Laws + The Data Wall

### Dominant theoretical framework(s)

1. **Compute-optimal scaling-law theory** (Kaplan et al., 2020;
   Hoffmann et al., 2022). Loss is a joint power law over parameters $N$,
   data $D$, and compute $C$; under a compute budget, the optimal
   tokens-per-parameter is $\sim 20$ (Chinchilla). The underlying theoretical
   claim is that **loss is a monotone function of `(tokens-seen,
   parameters)`** and that data is counted in **raw tokens**.
2. **Finite-corpus scaling theory** (Villalobos et al., 2024). The
   accessible human-generated text stock is a bounded resource; under
   current usage rates, the scaling-law regime collides with the data ceiling
   in 2026–2032. This is a conservation-law-style framing applied to the
   Kaplan axis.
3. **Accessible-fraction theory** (Longpre et al., 2024). The accessible
   fraction of the corpus is itself decaying (robots.txt restrictions, TDM
   opt-outs); the effective denominator shrinks faster than the theoretical
   stock.

### Epistemological stance

**Positivist / extrapolative.** Scaling-law theory treats model behaviour as
lawful and extrapolable; the data wall is an empirical ceiling derived from
observational extrapolation. The three industry signals (Woo & Efrati, 2024;
Reuters / Hu & Tong, 2024; Franzen, 2024; Sutskever NeurIPS 2024) contribute
no new theoretical content; they corroborate the extrapolation.

### Our DDA contribution vs. Cluster A

- **DDA → EXT (extends Kaplan/Chinchilla, compatible after an axis change).**
  DDA does not contradict the scaling-law form
  $L(N, D, C) = a N^{-\alpha} + b D^{-\beta} + \ldots$; it **re-interprets
  the $D$ axis**. Under DDA, $D$ is not "raw token count" but
  "structured-signal count $n(N + \binom{N}{2})$". The scaling-law form is
  preserved; the axis units change. This is an **EXT** because the original
  theory permits the re-interpretation (it never forbade signals-per-input as
  a unit) but did not elect it.
- **Third-path framing (DDA vs. paths 1/2) → COMP.** Cluster A's data-wall
  theory is silent on whether decomposing inputs through frozen embedders
  multiplies the effective signal supply. Our §3 §1 three-paths positioning
  is compatible: it adds a path the theory does not mention, without
  asserting that path 1 (acquire more) or path 2 (synthesise) are broken.
- **Meaning compression → EXT of the accessible-fraction argument.**
  Longpre's accessible-fraction observation has a dual: if effective signal
  per accessible input rises (via DDA), the shrinking accessible corpus is
  partly compensated. This is a coherence check, not a new theorem.

### Theoretical gaps this cluster exposes

- **A-FG-1 (framework gap).** No cluster-A theory prescribes a
  signals-per-input axis; the existing theory is silent on whether the
  scaling-law crossover point moves under DDA-enriched data. **Open work
  for §10.** (Also flagged as future work in `theoretical-gaps.md`.)
- **A-FG-2.** No cluster-A theory distinguishes "more raw tokens" from
  "more labelled signals on a fixed corpus". The theory has only one
  data-axis; we argue for two.

---

## Cluster B — Model Collapse + Synthetic Data

### Dominant theoretical framework(s)

1. **Self-consumption collapse theory** (Shumailov et al., 2024 *Nature*;
   Alemohammad et al., 2023 MAD). Iterated training on samples drawn from
   the previous generation leads to tail-mass loss (early collapse) and
   variance contraction (late collapse). The framework is a dynamical-systems
   claim on the generator-in-loop configuration.
2. **Scaling-law-crossover-under-collapse** (Dohmatob et al., 2024, tails).
   Collapse is re-expressed as a change in the scaling-law exponent; the
   training-run trajectory crosses over from the real-data regime to a
   degraded regime as synthetic fraction grows.
3. **Mitigation theory — accumulation** (Gerstgrasser et al., 2024 COLM).
   Keeping real data across generations bounds the test error; the theorem
   is about which feedback configurations are benign.
4. **Mitigation theory — verification** (Feng et al., 2024). An oracle
   verifier (or learned reward) that filters synthetic samples restores
   scaling. The theoretical lever is sample rejection inside the loop.

### Epistemological stance

**Positivist with explicit feedback-loop formalism.** Cluster B's theoretical
apparatus is precise about **which loop structure** causes failure: the
generator's outputs become the next generation's training data. Every theorem
in this cluster is predicated on a **recursive feedback loop**.

### Our DDA contribution vs. Cluster B

- **DDA → ORTH (orthogonal; the collapse theorem is silent on DDA).** DDA
  has **no generative feedback loop**: embedders are frozen (glossary #10)
  and trained independently of the system that consumes their signals;
  inputs are unchanged (never synthesised); the "signals" are deterministic
  projections of real inputs, not samples from a generator. Shumailov's
  theorem is about generator-in-loop training; DDA is not a generator-in-loop
  configuration. Thus DDA is *outside the scope* of the collapse theorem —
  **ORTH, not EXT**.
- **Meaning compression → ORTH.** Same reasoning: compressing signals per
  raw-data unit is a measurement operation on real inputs, not a generator
  on a synthetic corpus.
- **Categorical distinction in §3.3.** Our paper makes a **SR-style** claim
  (the theorem does not apply) rather than an EB counter-experiment. This
  is a *scope clarification*, not a new theory.

### Theoretical gaps this cluster exposes

- **B-FG-1.** The collapse theory's unit of analysis is "sample was produced
  by a generator". It has no framework for distinguishing
  generator-produced-samples from frozen-embedder-projections. **No theorem
  exists that covers the DDA regime.** This is an open question for §10
  rather than a deficiency of Cluster B.
- **B-FG-2.** Cluster B's verification theory (Feng 2024) and DDA superficially
  resemble each other (both add "extra labellers"), but the theoretical
  foundations differ: verification assumes a noisy generator being filtered;
  DDA assumes a frozen embedder observing a clean input. Agent-10 flags this
  as an open conceptual-distinction question.

---

## Cluster C — Compression-as-Intelligence

### Dominant theoretical framework(s)

1. **Prediction–compression duality** (Delétang et al., 2024 ICLR;
   Valmeekam et al., 2023). A good next-token predictor *is* a good
   arithmetic-coded compressor; the relation is tight via the Shannon
   source-coding theorem. Theoretical core:
   $\text{expected code length} = H(X) + D_{\mathrm{KL}}(p \| q)$.
2. **Linear compression–capability correlation** (Huang et al., 2024 COLM,
   $r \approx 0.95$). An empirical regularity claim that compression ratio
   correlates linearly with downstream capability across 30+ LLMs.
3. **Universal-compressor classification** (Jiang et al., 2023 ACL Findings).
   A model-free compressor + k-NN can match learned classifiers on
   low-resource tasks; the theoretical implication is that "intelligence"
   partially reduces to compression on the relevant distribution.
4. **Adjacent**: weight-compression theory (GPTQ, SparseGPT) and
   activation-compression theory (TurboQuant, KV-cache quantisation) are
   cited for the four-category taxonomy but carry their own theoretical
   lineages (second-order Hessian sensitivity, low-rank structure, etc.).

### Epistemological stance

**Information-theoretic / positivist.** Theorems here are Shannon-anchored;
empirical claims (Huang correlation) are observational but anchored by a
well-defined compression ratio metric.

### Our meaning-compression contribution vs. Cluster C

- **Meaning compression → EXT (extends the compression-as-intelligence
  frame; adds a fourth category).** Cluster C's theory reads compression
  *from a trained model* ("prediction is compression"). Our §3.1 flips the
  operator: compression *of the training corpus*, via $N$ frozen embedders,
  into a labelled-signal density. The information-theoretic lineage is
  preserved (the $\binom{N}{2}$ pairwise-interaction count would be
  analysable via mutual information); the novelty is in the
  dimensional-unit axis (signals/input vs. bits/bits).
- **DDA → EXT.** DDA is the data-side instantiation of meaning compression;
  the count $n(N + \binom{N}{2})$ is an upper bound on the labelled-signal
  content of $D$ under panel $\Phi$.
- **TCT → COMP with Cluster C** (compatible, not directly engaged). TCT
  uses compression-as-intelligence *substrate* (the frozen embedders are
  precisely the compressors whose quality Cluster C measured); it does
  not extend the theory.

### Theoretical gaps this cluster exposes

- **C-FG-1.** No cluster-C theory has a **cross-compressor interaction
  term**. Compression ratio is reported per-compressor; there is no
  established information-theoretic protocol for measuring the *residual*
  signal carried by $\phi_j$ given $\phi_k$ on the same input. Our
  $\binom{N}{2}$ count is an upper bound precisely because this theory
  is not yet built.
- **C-FG-2.** No cluster-C theory treats a compressor as a *labelled-signal
  producer for training-data preparation*. The operator direction is
  flipped in our paper but the theory has not been developed for the
  flipped direction.

---

## Cluster D — Multi-Modal Embedders + Retrieval

### Dominant theoretical framework(s)

1. **Contrastive representation learning** (Zhai et al., 2023 SigLIP;
   implicit in CLIP lineage the paper cites via SigLIP). A sigmoid / softmax
   contrastive loss yields embeddings where task-relevant pairs are near
   and task-irrelevant pairs are far; the underlying theory is noise-
   contrastive estimation / InfoNCE.
2. **Late-interaction retrieval theory** (Khattab & Zaharia, 2020 ColBERT;
   Santhanam et al., 2022 ColBERTv2). Per-token matrices enable expressive
   relevance scoring at the cost of index size; the theory analyses
   quality–cost trade-offs.
3. **Sparse lexical expansion** (Formal et al., 2021 SPLADE). Dense
   semantics is combined with sparse lexical match via a regulariser; the
   theoretical claim is that lexical sparsity is a FLOPs-efficient
   complement to dense recall.
4. **Knowledge-embedding theory** (Wang et al., 2021 KEPLER; Bordes et al.,
   2013 TransE). Relational triples embed as translations in a learned
   vector space; the theory is structural (entity–relation geometry).
5. **Approximate nearest-neighbour theory** (Malkov & Yashunin, 2018 HNSW).
   Hierarchical navigable small-world graphs give log-time NN queries with
   bounded recall; a combinatorial-graph theoretical lineage.
6. **Rank-fusion theory** (Cormack et al., 2009 RRF). Reciprocal rank fusion
   is empirically robust relative to Condorcet / rank-learning baselines;
   the theory is probabilistic-ranking-aggregation.

### Epistemological stance

**Pragmatic / representation-pluralist.** Embedder theory in this cluster is
instrument-specific: each embedder family has its own loss, its own pretext
task, and its own benchmark. The unifying theoretical claim is thin —
"representations transfer well to retrieval when trained on the right pretext"
— which is empirical, not deductive.

### Our DDA / TCT / constellation-guard contributions vs. Cluster D

- **DDA → EXT.** Cluster D treats embedders as **rankers / retrievers**;
  DDA treats them as **labellers**. The underlying contrastive / late-
  interaction / sparse-expansion theories are all compatible with the
  role-flip. Reciprocal rank fusion theory (Cormack 2009) is the closest
  precedent for multi-embedder composition, but RRF measures *ranking lift*
  and the DDA count measures *signal cardinality*; same substrate,
  different axis.
- **TCT → EXT of contrastive-learning theory (but emphatically NOT
  contrastive learning).** TCT's Phase-2 geometric-proximity term
  $\sum_m \lambda_m (1 - \cos(\phi_m(\hat{y}), c_m))$ reuses the cosine
  geometry that contrastive methods exploit, but (i) the embedder
  $\phi_m$ is frozen and (ii) no negatives are sampled. TCT is **positive-
  only**, **frozen-metric** training. The theoretical contrast is:
  contrastive methods *learn the metric*; TCT *inherits a fixed metric*
  from pretraining. This is an **EXT** because the contrastive framework
  could subsume TCT by setting the metric to frozen and negatives to
  empty — it never does.
- **Constellation guard → BRK / novel.** No cluster-D paper proposes a
  **runtime per-output cosine-gate against a frozen centroid** as an
  acceptance predicate for generation. The retrieval cluster's embedders
  score similarity; they do not gate generation. This is **framework-
  breaking** in the sense that the retrieval literature's theory has no
  place for "generation accept/reject"; that concept sits outside retrieval
  semantics. Our constellation-guard contribution therefore does not
  extend Cluster D's theory; it imports cluster-D geometry into a
  generation-acceptance theory that the cluster does not have.

### Theoretical gaps this cluster exposes

- **D-FG-1.** Cluster D has no **per-output acceptance semantics** for
  embedder-based scoring. Retrieval accepts the top-$k$ on a soft ranking;
  our constellation guard demands a hard threshold. The theory of
  "embedder as binary gate" is underdeveloped.
- **D-FG-2.** Cluster D has no formal theory of **N-embedder independence
  / orthogonality**. V14 hedges "approximately independent"; no cluster-D
  theorem states when two embedders carry independent signal. This is a
  foundational open question for agent-10 to flag (§10).
- **D-FG-3.** Rank-fusion theory (RRF, Condorcet) measures ranking quality,
  not labelled-signal yield. The theoretical conversion "fusion lift →
  $\binom{N}{2}$ pairwise signals" is not established.

---

## Cluster E — Zero-Shot TTS + Voice Cloning (Case 3)

### Dominant theoretical framework(s)

1. **Neural-codec TTS as in-context learning** (Wang et al., 2023 VALL-E;
   Chen et al., 2024 VALL-E 2). Voice cloning is re-framed as language
   modelling over discrete audio codec tokens; the theoretical claim is
   that sequence models generalise across speakers with a single prompt.
2. **Factorised-codec / masked-generative-codec theories**
   (Tan et al., 2024 NaturalSpeech 3; Wang et al., 2024 MaskGCT). The
   codec factorisation separates content, prosody, timbre; the generative
   theory differs (masked GCT vs. AR codec) but the factorisation is the
   common theoretical substrate.
3. **Flow-matching TTS theory** (Lipman et al., 2023 foundational; Chen
   et al., 2024 F5-TTS; Le et al., 2023 Voicebox). A probability-path
   interpolation between noise and data enables non-autoregressive TTS;
   the theory is continuous-time score / flow matching.
4. **Non-intrusive objective MOS theory** (Reddy et al., 2022 DNSMOS;
   Saeki et al., 2022 UTMOS). Naturalness and quality can be predicted
   without reference audio by learned models trained on subjective MOS
   data. The theoretical lineage is regression calibration.
5. **Speaker-encoder verification theory** (Desplanques et al., 2020
   ECAPA-TDNN; Chen et al., 2022 WavLM). Speaker embeddings form a
   discriminative space where within-speaker cosine is high and across-
   speaker cosine is low; the theory is representation-learning + metric
   verification.

### Epistemological stance

**Pragmatic, benchmark-anchored.** The field's theoretical content is
architecture-specific (AR codec vs. masked codec vs. flow-matching); the
unifying empirical claim is "condition on one prompt, score with a fixed
encoder". The parity-claim (VALL-E 2's "human parity") is itself a
benchmark-shaped claim, not a theoretical equivalence.

### Our Case-3 / TCT contribution vs. Cluster E

- **Centroid aggregation → EXT of speaker-encoder theory.** Cluster E's
  speaker-encoder literature supports centroid computation in principle
  (WavLM embeds into a cosine space), but no cited Cluster-E paper uses a
  multi-clip centroid as the *alignment target* for TTS generation. Our
  50-clip L2-normalised-mean-of-normalised construction extends speaker-
  encoder theory from discriminative verification into generative
  alignment.
- **Constellation guard (Case 3's WavLM-scored best-of-12 at T=0.3) →
  BRK / novel in Cluster E.** Cluster E's theory has no place for a
  per-output cosine gate in the TTS inference loop. Reported SIM-o and
  WavLM-cosine numbers are post-hoc evaluation, not runtime acceptance.
  Our Phase-3 guard imports TCT's framework into Cluster E; the cluster's
  own theory does not need to be broken, but it is **silent** on the
  guard — hence **BRK** in the sense of "the cluster's theory would need
  additions to accommodate per-output gating as a TTS primitive".
- **Cross-encoder SECS → EXT of speaker-verification theory.** Speaker-
  verification literature (WavLM, ECAPA) treats the encoder as the
  reference metric; our protocol insists the *scoring* encoder must be
  distinct from any encoder the TTS model was conditioned on. This
  extends verification theory with an independence condition.
- **Same-session band (0.95–0.99) → outside cluster theory.** The
  band is our own empirical observation on Case 3's speaker; no cited
  theory predicts its range. V12 / V17 enforce the hedge.

### Theoretical gaps this cluster exposes

- **E-FG-1.** Cluster E has no theoretical model of **multi-clip centroid
  vs. single-prompt conditioning**. The single-prompt paradigm is
  conventional; the centroid alternative is empirically new.
- **E-FG-2.** Cluster E has no **runtime-acceptance theory** for TTS.
  Our paper imports one from TCT; the cluster's own theoretical
  apparatus does not predict when / whether per-output gating matters.
- **E-FG-3.** No theory predicts the **cross-encoder gap** between
  SECS-score-with-conditioning-encoder and SECS-score-with-independent-
  encoder. We treat this as an identification condition, but it is not
  yet theorised.

---

## Cluster F — Identity-Locked Video Generation (Case 2)

### Dominant theoretical framework(s)

1. **Flow-matching generative theory** (Lipman et al., 2023; Meng et al.,
   2025 EchoMimicV3; Wan Team, 2025). A continuous-time probability path
   between prior and data; the loss is the conditional flow-matching MSE
   on the velocity field. EchoMimicV3 uses the target-convention
   $\text{target} = \text{noise} - \text{latent}$ (glossary #30).
2. **Diffusion-transformer video theory** (Yang et al., 2024 CogVideoX;
   Cui et al., 2024 Hallo3; Zheng et al., 2025 HunyuanVideo-Avatar).
   DiT backbones scale to video; conditioning on audio / reference image
   is architecturally standardised.
3. **Low-rank adaptation theory** (Hu et al., 2022 LoRA, ICLR). A
   low-rank decomposition of the weight update $\Delta W = BA$ with
   $\mathrm{rank}(BA) \ll \dim$ is a parameter-efficient fine-tune. The
   theoretical claim is that task-specific deltas tend to be low-rank.
4. **Facial-shape statistical theory** (Li et al., 2017 FLAME). Identity
   and expression live in a low-dimensional statistical-shape space
   fitted to 4D scans; the theory is Gaussian-mixture + linear-blend.
5. **Expression taxonomy** (Ekman & Friesen, 1978 FACS). Action-unit
   decomposition of facial expression; the theoretical content is a
   discrete coding scheme.
6. **Lip-sync theory (implicit)** — SyncNet-era contrastive audio-visual
   correspondence; cited indirectly via LatentSync (Li et al., 2024).

### Epistemological stance

**Pragmatic / architecture-driven.** Theoretical content is generator-
family-specific; identity conditioning is treated as an input pathway,
not as a theoretical object with its own calculus.

### Our Case-2 / ClipCannon / TCT contribution vs. Cluster F

- **Seven-modality constellation → EXT of flow-matching conditioning
  theory.** Cluster F conditions on reference images + audio fed through
  the generator's cross-attention; our centroid construction adds a
  *parallel* geometric alignment target (visual, speaker, voice, emotion,
  FLAME/expression, etc.). The generator is still flow-matching; the
  loss gains a geometric-proximity term. This **extends** the
  flow-matching theory with an added auxiliary objective.
- **Mouth-weighted flow-matching loss → EXT of flow-matching theory.**
  Non-uniform spatial weighting of the flow-matching MSE is theoretically
  accommodated by the cluster (the loss is a sum over the latent tensor)
  but not commonly explored; our case articulates a specific weighting
  tied to a perceptual-quality target.
- **Constellation guard for video → BRK / novel in Cluster F.** Same
  structural novelty as Cluster E: no cluster-F paper applies a runtime
  per-modality cosine-gate to accept / reject generated video. CSIM is
  post-hoc on a test set, not per-output. Our T3-projected guard is
  framework-novel for video.
- **Flow-matching sign convention (forensic) → outside cluster theory.**
  The sign ambiguity $\text{target} = \text{noise} - \text{latent}$ vs.
  $\text{latent} - \text{noise}$ is a practitioner-level hazard not
  discussed in cluster-F papers. The forensic disclosure
  (`docs/inspect/08_training_audit.md`) is P10 / FM-08 compliance; the
  theoretical content is modest (a sign-convention definition) but the
  practical content is significant.

### Theoretical gaps this cluster exposes

- **F-FG-1.** Cluster F has no **multi-modal centroid theory for identity**.
  Identity is treated as conditioning, not as a geometric alignment
  target composed of frozen per-modality centroids.
- **F-FG-2.** No **per-output runtime-acceptance theory for video
  generation** exists in the cluster. Our TCT Phase-3 guard imports one;
  the cluster is silent on the construct.
- **F-FG-3.** No theoretical treatment of **flow-matching sign-convention
  ambiguity** as a correctness condition. The convention is assumed /
  inherited from an implementation. Our §6.2 forensic disclosure is the
  right granularity for §10 to flag as an open-methods concern.

---

## Cluster I — Alignment (RLHF / DPO / CAI)

### Dominant theoretical framework(s)

1. **Scalar-reward preference theory** (Ouyang et al., 2022 InstructGPT).
   A reward model $r_\psi$ is trained on human preference pairs; the
   generator is aligned to maximise $\mathbb{E}[r_\psi(y | x)]$ under a
   KL-to-reference-policy regulariser. The theoretical lineage is
   preference elicitation → reward modelling → policy optimisation.
2. **Closed-form preference optimisation** (Rafailov et al., 2023 DPO).
   The RLHF objective re-expressed as a closed-form loss on preference
   pairs; the implicit reward is $r(y|x) = \beta \log(\pi_\theta /
   \pi_{\mathrm{ref}})$. Theoretically important because it makes the
   implicit-reward assumption explicit.
3. **AI-feedback alignment** (Bai et al., 2022 Constitutional AI). Self-
   critique against natural-language rules; preferences produced by an AI
   simulator rather than a human. The theoretical structure is still
   reward-model-based, but the preference source is machine-generated.

### Epistemological stance

**Preference-theoretic / decision-theoretic.** Cluster I's theory treats
alignment as a **reward-maximisation problem under a preference
relation**. The scalar reward is the theoretical primitive; geometry
does not enter.

### Our TCT contribution vs. Cluster I

- **TCT → BRK (framework-breaking).** TCT replaces the scalar-reward
  primitive with a **frozen geometric primitive** (a per-modality cosine
  to a frozen centroid). This is not an extension of scalar-reward
  theory; the theoretical object is different. No Bellman equation, no
  implicit reward, no preference pairs.
- **Constellation guard → BRK.** Cluster I's theory has no per-output
  acceptance predicate at inference. RLHF / DPO / CAI produce a
  distribution-shaped policy; no paper in the cluster runs a gate.
- **Multi-modal conjunction → BRK.** Cluster I uses a single scalar; TCT
  uses a per-modality conjunction ($\forall m: s_m \geq \tau_m$). The
  conjunction has no scalar-reward analogue without collapsing to a
  min/weighted-sum, which throws away the structural content.
- **Identity-locked generation → BRK.** The property "every output
  satisfies a per-output geometric predicate" has no scalar-reward
  equivalent; a reward-maximising policy does not imply per-output
  satisfaction of any specific predicate.
- **DDA / meaning compression → ORTH to Cluster I.** Cluster I operates
  on a fixed corpus + preference labels; DDA's signals-per-input axis
  does not interact with Cluster I's theory at first principles. The two
  frames are compatible (one can do RLHF on DDA-enriched data), but DDA
  does not extend nor break alignment theory.

### Theoretical gaps this cluster exposes

- **I-FG-1.** No **geometric-alignment theory** exists in Cluster I.
  Alignment is always scalar / preference-theoretic. Our §2.6 §5 contrast
  table is precisely the theoretical move that Cluster I has not made.
- **I-FG-2.** Cluster I has no **per-output runtime-acceptance
  predicate theory**. DPO / RLHF have KL-regularisers, not acceptance
  gates.
- **I-FG-3.** Multi-modality conjunction is not a cluster-I primitive.
  Our $\forall m$ conjunction generalises beyond what scalar-reward
  theory can express without a scoring projection that loses information.

---

## Cross-cluster framework-compatibility summary

| Cluster | Core framework(s) | DDA | Meaning compression | TCT | Constellation guard |
|---|---|---|---|---|---|
| A Scaling laws / data wall | Power-law data-axis | **EXT** (axis reinterpretation) | **EXT** | ORTH | ORTH |
| B Model collapse | Generator-in-loop dynamics | **ORTH** (no feedback loop) | ORTH | ORTH | ORTH |
| C Compression-as-intelligence | Prediction–compression duality | **EXT** (fourth category) | **EXT** | COMP | COMP |
| D Multi-modal embedders / retrieval | Contrastive / fusion / NN search | **EXT** (role-flip: ranker→labeller) | EXT | **EXT** (positive-only, frozen metric) | **BRK** (generation-accept novel) |
| E Zero-shot TTS | Codec / flow-matching / MOS | COMP | COMP | **EXT** (centroid, cross-encoder) | **BRK** (runtime gate novel) |
| F Identity-locked video | Flow-matching / DiT / LoRA | COMP | COMP | **EXT** (multi-modal centroid) | **BRK** (runtime gate novel) |
| G Auxiliary (ASR, source-sep, clustering) | Heterogeneous | ORTH | ORTH | ORTH | ORTH |
| H Enterprise / market | Non-theoretical (signals) | ORTH | ORTH | ORTH | ORTH |
| I Alignment (RLHF / DPO / CAI) | Scalar-reward / preference | ORTH | ORTH | **BRK** (geometric alternative) | **BRK** |

**Key outcomes.**
- **DDA**: dominantly **EXT** across the theoretical clusters that care
  about data supply (A, C, D); **ORTH** to collapse theory (B) and
  alignment (I).
- **Meaning compression**: **EXT** of compression-as-intelligence (C) and
  scaling-law data-axis (A); **ORTH** elsewhere.
- **TCT**: **EXT** of contrastive / embedder geometry (D), TTS speaker-
  encoder theory (E), video flow-matching theory (F); **BRK** relative to
  scalar-reward alignment (I). This is the cleanest break in the paper.
- **Constellation guard**: **BRK** across D / E / F / I. Novel in four
  clusters simultaneously. This is the **single most theoretically
  novel move** in the manuscript.

---

## Method-theory alignment check (carrying forward from agent-09)

Agent-09 verified method-theory **within-cluster** alignment (Q: does the
cluster's method match its theory?). Agent-10 verifies **our method-theory
alignment** (Q: does our proposed methodology match the theoretical frame we
claim?):

| Our construct | Our methodology (agent-09) | Our theoretical claim (this file) | Alignment |
|---|---|---|---|
| DDA | TD: counting argument $n(N + \binom{N}{2})$ + T2 instantiation | EXT of scaling-law data-axis; EXT of compression-as-intelligence | **Aligned** — TD method (counting) matches TD claim (axis re-interpretation). |
| Meaning compression | TD: ratio $\mathrm{MC} = N(N+1)/2$ + four-category taxonomy | EXT of compression-as-intelligence (fourth category) | **Aligned** — TD method + TD category claim. |
| TCT | FC + EB + AB: three-phase protocol, runtime guard, case-study EB | BRK of scalar-reward alignment; EXT of contrastive / embedder geometry | **Aligned**, with a caveat: the BRK claim is methodologically supported by the ablation table in §6 (agent-09 cheatsheet); the EXT claim is supported by the constellation-guard design (which reuses frozen cosine geometry). |
| Identity-locked generation | EB: per-output cosine trace on Case 3; T3-projected on Cases 1/2 | BRK (predicate concept absent in alignment / TTS / video theory) | **Aligned** — Case 3 T1 data instantiates the predicate; the BRK claim is a scope claim that the measured predicate fulfils. |
| Constellation guard | EB + SA: runtime re-embedding, per-modality cosine, per-output accept/reject | BRK across D / E / F / I | **Aligned** — the guard **is** the instantiation of the novel predicate concept; no other cluster has it. |

**No method-theory mismatches flagged.** Agent-09's methodological novelties
line up one-to-one with agent-10's theoretical novelties: counting argument
↔ axis re-interpretation; taxonomy ↔ fourth category; guard ↔ BRK in four
clusters; cross-encoder ↔ identification condition.

---

## Epistemological positioning of the manuscript

Across the nine clusters engaged by the manuscript, our paper sits at
**three overlapping epistemological registers**:

1. **Theoretical-derivation register (TD).** DDA count, meaning-compression
   ratio, four-category taxonomy. These are closed-form arguments presented
   as T4 claims.
2. **Empirical-benchmark register (EB).** Case 3 T1 numerics; Case 2 T2
   constructive counts; Case 1 T3 projected counts. All measurements are
   benchmark-shaped following Cluster E / F / I conventions.
3. **Forensic / case-study register (FC).** The Santa flow-matching
   sign-bug and the Shakespeare echo-bug are disclosed forensics; this is
   an under-used register in ML papers (not represented in any cluster
   other than as adjacent practitioner lore).

**Paradigm**: **pragmatic-constructive**. The paper is neither purely
positivist (no single measurement settles the question; theoretical
counting arguments matter) nor purely constructivist (empirical numbers
are load-bearing: Case 3 WavLM SECS 0.961 is not a social construction).
The correct paradigm label is **pragmatic-constructive**: construct +
measure + verify; multiple registers compose to make the claim defensible.

The alternative paradigms explicitly *not* adopted:
- **Critical / transformative** — absent. Our paper is not a critique of
  RLHF as ideology; it is a technical alternative.
- **Interpretive / constructivist** — absent. Our numerics are not
  socially co-produced; they are measured.
- **Pure positivist** — insufficient. Claim #1 (DDA count) is TD, not
  EB; pure positivism would require an empirical ablation we defer to
  §10.

---

## Confidence statement

**HIGH.** Every cluster's theoretical framework is anchored in citations
already in the master pool; every DDA / meaning-compression / TCT /
constellation-guard claim has been evaluated against every cluster's
framework using the EXT / COMP / BRK / ORTH codes; method-theory alignment
has been verified against agent-09's methodology scan row-by-row; no
mismatches require a blocker to agent-11.

**Locked elements (do not re-debate without a blocker).**
- Framework-compatibility codes (EXT / COMP / BRK / ORTH) for each
  (cluster × construct) cell in the summary table.
- "Constellation guard is BRK across 4 clusters" is the paper's sharpest
  theoretical novelty claim; carried forward to the related-work §2
  G3 novelty table and to §5 TCT's framing.
- "DDA is EXT of scaling-law data-axis, not BRK" — the paper does **not**
  claim to replace scaling-law theory; V and FM hedges carry.

**Open for agent-11 systematic review.**
- Whether PRISMA expansion of Cluster C could add a cross-compressor
  interaction citation that closes C-FG-1 (agent-10 found none in the
  current pool).
- Whether Cluster E has a paper that approximates a runtime-acceptance
  TTS concept (agent-10 found none; agent-11's sweep can verify).

---

## Changelog

- 2026-04-16 — Initial issue by agent-10-theoretical-framework-analyst.
