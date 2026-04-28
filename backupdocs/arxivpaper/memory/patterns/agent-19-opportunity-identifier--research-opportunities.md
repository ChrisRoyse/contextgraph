---
namespace: patterns
agent_id: agent-19-opportunity-identifier
timestamp: 2026-04-17T03:30:00Z
status: active
locked: false
supersedes: none
tags: research-opportunities, novelty-claims, RO-ledger, gap-to-opportunity, what-is-now-possible
depends_on:
  - memory/context/mission-brief.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/discoveries/agent-18-gap-hunter--consolidated-gaps.md
  - memory/patterns/agent-18-gap-hunter--gap-to-section-map.md
  - memory/handoff/agent-18-to-19--gaps-ready.md
  - memory/discoveries/agent-16-pattern-analyst--opportunity-map.md
  - memory/patterns/agent-16-pattern-analyst--meta-patterns.md
  - memory/patterns/agent-10-theoretical-framework-analyst--theoretical-contributions.md
downstream_consumers:
  - agent-20-interdisciplinary-pollinator
  - agent-35-abstract-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-41-future-work-writer
  - agent-42-adversarial-reviewer
---

# Research Opportunities — 10 RO IDs with Gap Anchors and Contribution Claims

## Scope and method

An **opportunity** is not a gap. A gap is "what is missing from the
literature". An opportunity is "what is now *possible* for the community
given the framework this paper contributes, that was not available
before the paper." Agent-18 issued **31 gaps** (GAP-01 … GAP-31) across
7 dimensions; agent-16 issued **five opportunity clusters** (OC-1 …
OC-5); agent-10 marked each theoretical contribution with a novelty
code (EXT / CAT-EXT / BRK / ORTH).

Agent-19's job is to collapse those artefacts into a **10-RO ledger**
(RO-1 … RO-10) where each RO:

1. Names a **what-is-now-possible** object (a new measurement, a new
   benchmark, a new alignment target, a new auditing primitive, …) —
   not a gap description.
2. Anchors to the agent-18 **GAPs** that motivate it + the agent-16
   **OCs** it realises.
3. States **our contribution** (what this paper commits to) vs. the
   **follow-up opportunities** the community can take.
4. Assigns a **criticality to manuscript** tag: central (load-bearing
   for the thesis) vs. sidebar (interesting but not thesis-critical).
5. Routes to the manuscript **section(s)** that instantiate the RO.

Register rules inherited from agent-17 / agent-18:

- **R1 composition-not-invention.** ROs describe new combinations of
  existing primitives, not new theoretical atoms.
- **R2 open-scorer / disclosed-forensic.** Every RO that depends on our
  numbers carries the open-scorer posture.
- **R3 pool-relative scope.** Novelty claims bounded by the 88-citation
  LOCKED master pool.
- **R4 projected-vs-measured.** Only Case 3 is measured; Cases 1 + 2 are
  projected.

**No new citations introduced; master pool LOCKED.**

**No renumbering of GAP IDs;** ROs cite GAP IDs by reference.

---

## Opportunity ledger at a glance

| RO | Short name | Gap anchors | OC anchor | Criticality | Primary §§ |
|---|---|---|---|---|---|
| **RO-1** | DDA counting formula as standalone primitive | GAP-01, GAP-02 | — | **Central** | §1, §3.1, §3.3, §3.4 |
| **RO-2** | Meaning compression as fourth compression category | GAP-01 | — | **Central** | §3.1 |
| **RO-3** | Multi-encoder constellation as alignment target | GAP-03, GAP-04, GAP-12 | — | **Central** | §5.1, §5.2, §5.3 |
| **RO-4** | Per-output runtime geometric guard (inference-time verification in generative ML) | GAP-03, GAP-16, GAP-22, GAP-24 | OC-4 | **Central** | §5.3, §5.4, §6.3 |
| **RO-5** | Cross-encoder SECS as identification hygiene | GAP-07, GAP-15, GAP-29 | OC-3 | **Central** | §6.3 |
| **RO-6** | Multi-clip centroid aggregation with open audio + open scorer | GAP-04, GAP-27 | — | **Central** | §5.1, §6.3 |
| **RO-7** | Forensic-disclosure register for generative-ML training | GAP-05, GAP-26 | — | Sidebar (HIGH) | §6.1, §6.2, §8.4 |
| **RO-8** | Empirical embedder-independence benchmark (MI across N frozen encoders) | GAP-06, GAP-09, GAP-13, GAP-17 | OC-2 | Sidebar (opens sub-field) | §3.4, §10.2 |
| **RO-9** | Safety-profile / negative-constellation programme | GAP-14, GAP-19 | — | Sidebar (ethics-paired) | §8.6, §10.3 |
| **RO-10** | Sample-complexity-curve protocol for identity-locked generation | GAP-20, GAP-22, GAP-24, GAP-29 | OC-4 | Sidebar (in-scope extension) | §6.3, §10.1 |

**Central (thesis-critical): RO-1, RO-2, RO-3, RO-4, RO-5, RO-6.**
**Sidebar (future-work and cross-field outgrowths): RO-7, RO-8, RO-9, RO-10.**

---

## RO-1. DDA counting formula as a standalone primitive

### Short name
Derived-Data-Abundance counting (`n(N + C(N,2))`) as an operative
quantity in training-data economics.

### What is now possible (that was not before)
A numerical statement of **labelled-signal yield per unit of raw
input** for a fixed training corpus projected through N frozen
embedders. For the first time in the LOCKED pool, the
training-data-economics literature has a primitive that *counts
supervision* rather than tokens: 91 signals/input for Context Graph
(N=13), 28 signals/input for ClipCannon (N=7), and an explicit
cross-embedder interaction term $\binom{N}{2}$ beyond the per-encoder
projections N.

### Anchor gaps
- **GAP-01** (signals-per-input ratio unformalised).
- **GAP-02** (no "third path" on the data-wall menu).

### Our contribution (this paper)
- §3.1 — meaning compression as a fourth compression category,
  orthogonal to bit-, weight-, and activation-compression.
- §3.3 — categorical scope argument showing DDA is silent on
  generator-in-loop collapse preconditions (`shumailov2024nature`,
  `dohmatob2024tails`, etc.).
- §3.4 — the `n(N + C(N,2))` count, worked at N=13 and N=7.
- §1 — three-paths framing (acquire / synthesise / decompose) that
  places DDA alongside paths 1 and 2.

### Future-work opportunities (for others)
- **F1a** Measure actual mutual-information yield across N frozen
  encoders on a fixed corpus (GAP-17; this is RO-8).
- **F1b** Re-fit Kaplan / Chinchilla exponents $\alpha, \beta$ under
  DDA-enriched supervision (GAP-10, GAP-18; this feeds Novel Question
  Q1 in the future-work file).
- **F1c** Extend DDA to modalities beyond Context Graph's and
  ClipCannon's encoder choices; publish a canonical N-panel for audio /
  video / text.

### Agent-10 novelty code
**EXT** of scaling-law data-axis (Cluster A) + **ORTH** of collapse
theory (Cluster B). Not BRK.

### Criticality
**Central.** Load-bearing for Mission-Brief Claim #1. Without RO-1 the
paper has no counting argument and no data-wall reframing move.

---

## RO-2. Meaning compression as a fourth compression category

### Short name
Meaning-compression as a distinct compression taxon alongside
bit-compression, weight-compression, and activation-compression.

### What is now possible
A taxonomic **placement** for the DDA operator that prevents confusion
with existing compression categories (e.g., `huang2024compression`'s
compression-capability correlation, or weight-compression like
TurboQuant / activation-compression like KV-cache quantisation). The
category has:
- A signal-yield indicator: `(N + C(N,2)) / input` (primary).
- An MI-adjusted yield indicator: $\mathrm{MC}_{\mathrm{MI}}(\Phi, D)$
  (secondary; future work per RO-8 / GAP-13).
- Open positions for other operators (e.g., multi-view projection,
  paraphrase cascades) to claim the same taxon.

### Anchor gaps
- **GAP-01** (signals-per-input ratio unformalised).

### Our contribution
- §3.1 — taxonomic placement + bit/weight/activation/meaning table.
- §3.4 — primary indicator worked at N=13 and N=7.
- §10.2 — secondary indicator (MI) flagged as future work.

### Future-work opportunities
- **F2a** Publish the first MI-measured meaning-compression ratio on a
  fixed corpus (pairs with RO-8).
- **F2b** Characterise meaning-compression for modalities not in our
  N-panel (e.g., music, tactile, olfactory — i.e., beyond text / audio
  / video).
- **F2c** Connect meaning-compression to information-theoretic
  bounds on supervised learning sample complexity.

### Agent-10 novelty code
**CAT-EXT** of compression-as-intelligence (cluster C). Not BRK.

### Criticality
**Central.** Load-bearing for Mission-Brief Claim #2.

---

## RO-3. Multi-encoder constellation as an alignment target

### Short name
**Constellation** (frozen, L2-normalised, multi-modal centroid set)
$C_P = \{c_1, c_2, \ldots, c_M\}$ as a **prediction target** in
alignment, replacing scalar reward.

### What is now possible
A structurally **geometric** alignment specification that replaces
InstructGPT / DPO / Constitutional AI's single-scalar-reward object
with an **M-vector centroid set**, one per modality. The alignment
loss becomes a multi-modal cosine-proximity sum (Phase-2 TCT loss,
§5.2); per-output acceptance becomes a conjunctive predicate
$\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m$ (RO-4).
Structurally, this **complements** scalar-reward alignment rather than
competing with it on the same axis — the two paradigms optimise
different objects.

### Anchor gaps
- **GAP-03** (per-output runtime geometric guard absent).
- **GAP-04** (multi-clip centroid aggregation not the convention).
- **GAP-12** (TCT-reward non-equivalence is structural, not yet
  formally proved).

### Our contribution
- §5.1 — TCT theoretical framing (Phase 1 centroid construction;
  Phase 2 geometric-alignment loss).
- §5.2 — Phase-2 loss definition.
- §5.3 — constellation guard definition.
- §2.6 — scalar-vs-geometric contrast table against InstructGPT / DPO
  / Constitutional AI.

### Future-work opportunities
- **F3a** Formal proof of non-equivalence between TCT and RLHF via
  Bradley-Terry-recovery analysis (GAP-12).
- **F3b** Theoretical characterisation of the constellation as a
  Bayesian prior vs. a reward-model-in-disguise.
- **F3c** Combinations: hybrid TCT + RLHF losses; ablations that turn
  off individual modality dimensions.
- **F3d** Application to non-identity targets (safety, ethics,
  factuality) — this is RO-9.

### Agent-10 novelty code
**BRK** of scalar-reward alignment (cluster I) + **EXT** of cluster-D
contrastive geometry.

### Criticality
**Central.** Load-bearing for Mission-Brief Claim #3. This is the
manuscript's second-most-novel move (after RO-4).

---

## RO-4. Per-output runtime geometric guard — inference-time verification in generative ML

### Short name
**Constellation guard**: per-output acceptance predicate
$\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m$ evaluated **at
inference time** over frozen embedders, rejecting non-conforming
outputs.

### What is now possible
For the first time in the LOCKED pool across clusters D, E, F, and I,
a **runtime verification import from formal methods** into generative
ML. The guard supplies:
- A **per-output** (not batch-metric) acceptance semantics.
- A **multi-encoder conjunction** (not single-encoder) identity
  predicate (replaces the single-encoder habit of Cluster E/F
  benchmarks).
- A **threshold-based rejection** mechanism compatible with best-of-K
  sampling and rejection sampling (measured in Case 3 at K=12, T=0.3,
  $\tau=0.95$ → 0.961 mean SECS).

### Anchor gaps
- **GAP-03** (per-output runtime geometric guard absent across four
  clusters).
- **GAP-16** (guard scaling to many modalities / many subjects —
  future-work budget analysis).
- **GAP-22** (per-output runtime rejection-rate benchmark — future).
- **GAP-24** (guard-on vs. guard-off ablation — can be run in-paper
  from Case 3's K=12 candidate distribution).

### Anchor OC
- **OC-4** (constellation-guard ablation).

### Our contribution
- §5.3 — guard definition.
- §5.4 — per-output predicate.
- §6.3 — measured instantiation: cross-encoder SECS
  (Qwen3-TTS → WavLM-base-plus-sv), 50-clip centroid, best-of-12 at
  T=0.3, $\tau=0.95$ → 0.961 mean SECS (max 0.975) on 10 unseen
  English sentences; DNSMOS P.808 parity (3.93 vs 3.93); UTMOS parity
  (3.012 vs 2.997) vs. real recordings.
- §6.3 supplement — guard-on vs. guard-off ablation from K=12
  candidate distribution (this closes GAP-24 in-paper if feasible
  before freeze).
- §1 preview + §7 Discussion synthesis.

### Future-work opportunities
- **F4a** Multi-modality guard scaling: accept-rate budget analysis as
  M grows (GAP-16).
- **F4b** Per-output rejection-rate benchmark across a grid of
  (generator × scorer × threshold) tuples (GAP-22).
- **F4c** Guard as a quality-assurance layer in production
  safety-critical systems (ties to RO-9).
- **F4d** Theoretical characterisation of the guard's false-positive /
  false-negative trade-off as threshold $\tau$ varies — this is
  Novel Question Q2.

### Agent-10 novelty code
**BRK** across clusters D, E, F, I. Agent-16 names MP-09 (per-output
runtime verification gap) as the single most theoretically novel
absence in the pool.

### Criticality
**Central.** This is the manuscript's single most theoretically novel
move. RO-4 **is** the paper's paradigm-import claim.

---

## RO-5. Cross-encoder SECS as identification hygiene

### Short name
**Cross-encoder SECS**: scoring identity similarity with an encoder
architecturally and data-independent from any encoder used during
generation.

### What is now possible
A **scoring hygiene** discipline that was never named as a distinct
evaluation move in cluster E. Within-family scorers (SIM-o in
`chen2024valle2`; SECS in `tan2024naturalspeech3`) rotate among WavLM-
/ ECAPA-variants, risking circular evaluation when the generator
already conditions on a WavLM-family embedder. Our protocol uses
Qwen3-TTS for conditioning and `microsoft/wavlm-base-plus-sv` for
scoring — architecturally independent, independently trained — as a
**good-hygiene** baseline for identity-predicate claims.

### Anchor gaps
- **GAP-07** (no cross-encoder SECS protocol in cluster E).
- **GAP-15** (theoretical characterisation of cross-encoder SECS as
  an identification condition — future).
- **GAP-29** (multi-subject same-session band study — future).

### Anchor OC
- **OC-3** (cross-encoder SECS sweep).

### Our contribution
- §6.3 — first explicit statement of cross-encoder SECS as a
  reproducibility-grade protocol; Case 3 measured instantiation.
- §2.5 — related-work contrast: cluster E within-family evaluation.
- §8.1 — single-subject / single-encoder-pair / English-only scope
  hedge (agent-14 Biases 05/06/07).

### Future-work opportunities
- **F5a** Cross-encoder sweep: K TTS models × J speaker encoders; map
  the SECS gap as scorer rotates (GAP-15).
- **F5b** Multi-subject same-session band characterisation across
  $\geq 50$ speakers (GAP-29).
- **F5c** Cross-language extension: tonal, agglutinative, RTL, and
  low-resource languages (GAP-30).
- **F5d** Theoretical bias bound between same-encoder and
  cross-encoder SECS under worst-case shortcut-learning assumptions.

### Agent-10 novelty code
**EXT** of cluster-E evaluation convention; hygiene-not-novelty at the
theoretical level, but **pool-novel** at the protocol level.

### Criticality
**Central.** Without RO-5 the 0.961 Case-3 SECS headline would be
vulnerable to adversarial reviewers (MP-01 single-encoder critique
returns). RO-5 neutralises that critique.

---

## RO-6. Multi-clip centroid aggregation with open audio + open scorer

### Short name
**50-clip L2-normalised centroid** as an identity-reference object
(replaces single-prompt ICL); accompanied by **open audio + open
scorer** to enable third-party replication.

### What is now possible
An alignment-reference object that is not a single audio prompt
(cluster E convention) nor a single reference image (cluster F
convention), but a **centroid over many clips per modality**,
computed by a frozen encoder, held constant through training and
inference. The companion open-scorer posture (WavLM-base-plus-sv
scorer disclosed; audio samples releasable; hyperparameters published)
gives third-party labs everything they need to replicate Case 3.

### Anchor gaps
- **GAP-04** (multi-clip centroid aggregation not the convention).
- **GAP-27** (Case 3 replication by an independent lab — invitation).

### Our contribution
- §5.1 — centroid-construction phase (Phase 1 of TCT).
- §6.3 — measured: 50-clip centroid, WavLM-base-plus-sv, 0.961 mean
  SECS on 10 unseen English sentences.
- §6.3 opener — open-scorer posture contrasted against cluster-E's
  closed-artefact frontier (VALL-E 2, NaturalSpeech 3 — agent-13
  Flags 1 + 4).
- §6.2 — projected: 7-modality centroid for Case 2 (Santa), 2,362
  clips.

### Future-work opportunities
- **F6a** Independent-lab replication of Case 3 (GAP-27).
- **F6b** Reference-set-size sweep:
  $n_{\text{clips}} \in \{1, 5, 10, 25, 50, 100\}$ vs. accept-rate
  and mean SECS — this is RO-10.
- **F6c** Cross-modality centroid studies (audio → video → text style
  → gesture → prosody), informed by Case 2's projected 7-modality
  setup.
- **F6d** Centroid-as-prior connection: interpret centroids in a
  Bayesian sense.

### Agent-10 novelty code
**EXT** of clusters E and F (centroid aggregation over single-prompt
ICL); **author-empirical** instantiation anchor (Case 3).

### Criticality
**Central.** Load-bearing for Mission-Brief Claim #4
(sample-complexity collapse): the collapse move **requires** that
"one reference prompt" is replaced with "one reference *set* per
modality + frozen centroid". RO-6 is the construct that makes that
replacement concrete.

---

## RO-7. Forensic-disclosure register for generative-ML training

### Short name
**Engineering-forensic register**: inline disclosure of training-data
echo-bugs, flow-matching sign-convention errors, and other pipeline
pathologies that are conventionally hidden from publication.

### What is now possible
A **publication register** — adjacent to the architecture and results
register — that names concrete pipeline pathologies, how they were
diagnosed, and how they were fixed. Cluster-E and cluster-F papers
typically publish head-line metrics without disclosing the
non-trivial pipeline forensics required to hit them (MP-06
"forensic-failure-hidden" in agent-16). Our paper adopts the opposite
discipline.

### Anchor gaps
- **GAP-05** (flow-matching sign-convention + training-data echo-bug
  forensic register is absent).
- **GAP-26** (Cases 1 + 2 are projected at time of writing; forensic
  disclosures + hyperparameter sheets make them reproducibility-ready
  ahead of the measurement).

### Our contribution
- §6.1 — Case 1 (Shakespeare LoRA) echo-bug disclosure: training-data
  echo in the LoRA adapter caused apparent stylistic conformity that
  was actually memorisation; diagnosis + fix documented.
- §6.2 — Case 2 (Santa EchoMimicV3 + LoRA) flow-matching sign-bug
  disclosure by reference to
  `docs/inspect/08_training_audit.md`; $target = noise - latent$
  versus $target = latent - noise$ sign hazard named.
- §8.4 — aggregate forensic-lessons paragraph.

### Future-work opportunities
- **F7a** Publish a **forensic-register template** that other groups
  can adopt; propose as a workshop track at NeurIPS /
  ICML / ICLR system-paper venues.
- **F7b** Collect and taxonomise generative-ML pipeline pathologies
  in a living document (pair with model cards / data sheets).
- **F7c** Automated detection of echo-bugs and sign-convention errors
  as a CI/CD tool for ML pipelines.

### Agent-10 novelty code
**Methodological-reproducibility** contribution; not a theoretical
novelty (per agent-09 FC "forensic contribution" code).

### Criticality
**Sidebar (HIGH).** Not thesis-critical, but agent-14 Biases-08/09
(publication-positivity, methodology-survivorship) identify this
register as a **load-bearing differentiator** for reproducibility
posture. Reviewer-credibility-boosting.

---

## RO-8. Empirical embedder-independence benchmark — MI across N frozen encoders

### Short name
**Meaning-compression-measurement benchmark**: mutual-information
estimates across N frozen encoders' projections of a shared corpus,
establishing an empirical characterisation of the $\binom{N}{2}$
pairwise interaction term in DDA.

### What is now possible
A **new sub-field** — empirical meaning-compression measurement —
where the central quantities are pairwise MI estimates between
embedder outputs. Baselines: MINE (Belghazi et al. 2018), InfoNCE
(van den Oord et al. 2018), Donsker-Varadhan bounds. Target
quantities:
- $I(\phi_i(X); \phi_j(X))$ for all
  $\binom{N}{2}$ encoder pairs.
- Effective-N estimate:
  $N_{\text{eff}} = N - \sum_{i<j} I(\phi_i; \phi_j) / H(\phi_i)$
  or similar.
- MI-adjusted meaning-compression ratio
  $\mathrm{MC}_{\mathrm{MI}}(\Phi, D)$.

### Anchor gaps
- **GAP-06** (cross-embedder pairwise-interaction feature protocol —
  structurally filled in §3.4; empirical MI future).
- **GAP-09** (formal embedder-independence measurement — future).
- **GAP-13** (MI-adjusted meaning-compression ratio — future).
- **GAP-17** (empirical MI measurement on our fixed corpus — future).

### Anchor OC
- **OC-2** (cross-compressor interaction / MI theory).

### Our contribution (this paper)
- §3.4 — upper-bound hedge stated ($\binom{N}{2}$ counts pairs, not
  MI); V11 independence assumption disclosed.
- §8.1 — hedge surfaced as limitation.
- §10.2 — named as primary future-work item under "theoretical
  extensions" / "meaning-compression measurement".

### Future-work opportunities (most of them are for others)
- **F8a** Baseline MI-estimation on Context Graph's 13-embedder panel
  using MINE / InfoNCE on a held-out corpus.
- **F8b** Benchmark comparison of MI estimators under high-dim
  embeddings (11,008-dim dense + 2 × 30,522 sparse + late-interaction).
- **F8c** Publish $N_{\text{eff}}$ values per canonical embedder panel
  (text, audio, video, code, entity).
- **F8d** Methodological-paper release: "**Meaning-Compression
  Benchmark v1**" with held-out corpora + scoring scripts +
  leaderboard.

### Agent-10 novelty code
Outside this paper's scope; **EXT** of cluster-C compression theory +
**ORTH** of cluster-D retrieval evaluation; opens a new empirical
niche.

### Criticality
**Sidebar (opens a sub-field).** Not thesis-critical for *this* paper
(our upper-bound claim is conservative; MI-adjusted is flagged
future). **CRITICAL** for the community — this is the research
programme that converts DDA from a counting primitive into a measured
operator. Novel Question Q1 in the future-work file.

---

## RO-9. Safety-profile / negative-constellation programme

### Short name
**Safety constellations** and **negative constellations** $C_P^{-}$ —
TCT generalised from identity targets (Case 3: voice identity) to
safety targets (e.g., refusal constellations, factuality
constellations, persona-drift prevention constellations).

### What is now possible
A **research programme** pairing TCT with safety /
alignment-evaluation literatures (cluster I). The constellation-guard
predicate (RO-4) becomes a runtime safety acceptance predicate over
multi-modal frozen safety-encoder projections. A negative
constellation $C_P^{-}$ flips the guard into a rejection predicate:
$\forall m : \cos(\phi_m(\hat y), c_m^{-}) < \tau_m^{-}$.

### Anchor gaps
- **GAP-14** (TCT generalisation to non-identity targets).
- **GAP-19** (safety-profile constellation existence — no current
  measurement).

### Our contribution (this paper)
- §8.6 — ethics paragraph naming safety constellations as an open
  research programme with care-points (dual-use, false-refusal bias,
  constellation-of-what-whose-values).
- §10.3 — paired with GAP-11 non-collapse corollary as "paradigm
  extensions".

### Future-work opportunities
- **F9a** Construct a safety centroid from verified refusal examples;
  measure false-positive / false-negative rates on a safety benchmark.
- **F9b** Adversarial probing: can adversarial inputs pass the
  constellation guard while violating intended safety properties? (ties
  to Q19 Constellation Defender from the mission brief.)
- **F9c** Cross-cultural safety constellations — who holds the
  centroid? This opens into governance + policy territory.
- **F9d** Ethics-workshop paper venue (FAccT / AIES / NeurIPS Ethics
  track) rather than main conference, given the dual-use posture.

### Agent-10 novelty code
**EXT** of RO-3 (TCT as alignment target) into safety; not a new
theoretical primitive.

### Criticality
**Sidebar (ethics-paired).** Not thesis-critical for *this*
(technical) paper. **CRITICAL** for the responsible-deployment
conversation around TCT. Novel Question Q3 in the future-work file.

---

## RO-10. Sample-complexity-curve protocol for identity-locked generation

### Short name
**Sample-complexity curve**: sweep of reference-clip count
$n_{\text{clips}} \in \{1, 5, 10, 25, 50, 100\}$ vs. runtime
accept-rate and mean SECS under constellation guard.

### What is now possible
A **quantitative characterisation** of Claim #4 (sample-complexity
collapse). Case 3 reports a **single point** (50 clips → 0.961 mean
SECS). The curve would characterise the collapse's shape: is the
point at 50 clips near a plateau, or mid-slope? Does 10 clips already
hit 0.95+? Is there a sharp phase transition or a smooth curve?

### Anchor gaps
- **GAP-20** (sample-complexity curve for Case 3 — future).
- **GAP-22** (per-output runtime rejection-rate benchmark — future
  for the grid; in-paper for the operating point).
- **GAP-24** (guard-on vs. guard-off ablation — in-paper from K=12
  distribution if feasible).
- **GAP-29** (multi-subject same-session band study — future).

### Anchor OC
- **OC-4** (constellation-guard ablation).

### Our contribution (this paper)
- §6.3 — single operating-point measurement (50 clips → 0.961).
- §6.3 supplement (if feasible) — guard-on vs. guard-off ablation
  from best-of-12 distribution (closes GAP-24 in-paper and
  reclassifies it as "fill").
- §10.1 — sample-complexity curve flagged as tractable single-speaker
  future-work study on existing infrastructure.

### Future-work opportunities
- **F10a** Run the reference-clip-count sweep on the author's voice
  (tractable within current infrastructure; one-speaker scope
  honest).
- **F10b** Extend to $\geq 50$ speakers for cross-subject
  generalisation (GAP-29).
- **F10c** Compare curve shapes across modalities (audio vs. video
  vs. text style) when Case 2's 7-modality pipeline becomes measured.
- **F10d** Theoretical question: is there a minimum
  $n_{\text{clips}}$ below which the centroid is too noisy to
  function as an alignment target?

### Agent-10 novelty code
**EXT** of cluster-E / F single-prompt ICL; **author-empirical**
extension.

### Criticality
**Sidebar (in-scope extension).** Not thesis-critical but
reviewer-requested-probability high: adversarial reviewers will ask
"is 50 clips necessary or is it overkill?" RO-10 is the natural
follow-up paper.

---

## Summary — what changed from gaps to opportunities

Agent-18 issued gaps as **absences in the literature + our paper's
disposition**. Agent-19 reframes the subset of gaps that carry
**community-research-programme weight** as opportunities. Six ROs
are **central** (paper commits to filling, or establishes the
primitive needed for others to fill); four ROs are **sidebar**
(paper surfaces but invites community work).

| Central ROs (paper-critical) | Sidebar ROs (community opportunities) |
|---|---|
| RO-1 DDA counting primitive | RO-7 Forensic-disclosure register |
| RO-2 Meaning compression fourth category | RO-8 Embedder-independence benchmark (opens sub-field) |
| RO-3 Multi-encoder constellation alignment target | RO-9 Safety / negative constellation programme |
| RO-4 Per-output runtime geometric guard | RO-10 Sample-complexity-curve protocol |
| RO-5 Cross-encoder SECS hygiene | |
| RO-6 Multi-clip centroid aggregation + open scorer | |

---

## Cross-references (where each RO appears in the manuscript)

| RO | §1 | §2 | §3 | §4 | §5 | §6 | §7 | §8 | §9 | §10 |
|---|---|---|---|---|---|---|---|---|---|---|
| RO-1 | X | X | **X** | | | | X | | X | |
| RO-2 | | | **X** | | | | | | | X |
| RO-3 | | X | | | **X** | | X | X | | X |
| RO-4 | X | X | | | **X** | **X** | X | | | X |
| RO-5 | | X | | | | **X** | | X | | X |
| RO-6 | | | | | **X** | **X** | | | | X |
| RO-7 | | | | | | **X** | | X | | X |
| RO-8 | | | X | | | | | X | | **X** |
| RO-9 | | | | | | | | X | | **X** |
| RO-10 | | | | | | X | | | | **X** |

**X** = mention; **bold X** = primary instantiation.

---

## Confidence

**HIGH.**

- Every RO anchors to ≥ 1 agent-18 GAP ID; most anchor to 2–4 GAPs.
- Every RO assigns criticality (central / sidebar) with explicit
  justification.
- Every RO routes to specific manuscript sections using the agent-18
  placement map.
- Every RO names future-work opportunities for the community without
  overclaiming paper-scope.
- Register rules R1–R4 preserved.
- No new citations introduced; master pool LOCKED.
- Opportunity clusters OC-1 … OC-5 (agent-16) are mapped:
  OC-2 → RO-8; OC-3 → RO-5; OC-4 → RO-4 + RO-10; OC-5 → RO-7's
  ClipCannon-DAG-disclosure move (GAP-28 travels under RO-7).
  OC-1 (N-scaling ablation) is a future-work item under RO-1 (F1b)
  rather than a standalone RO; agent-19 judgement: RO-8 is the more
  publishable opportunity bundle.

## Changelog

- 2026-04-17 — Initial issue by agent-19-opportunity-identifier.
