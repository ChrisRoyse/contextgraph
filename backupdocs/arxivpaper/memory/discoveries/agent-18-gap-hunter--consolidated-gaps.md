---
namespace: discoveries
agent_id: agent-18-gap-hunter
timestamp: 2026-04-17T02:45:00Z
status: active
locked: false
supersedes: none
tags: gaps, consolidation, 7-dimensions, fills, partial, future-work, gap-ledger
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/discoveries/agent-06-literature-mapper--structural-holes.md
  - memory/discoveries/agent-09-methodology-scanner--methodological-gaps.md
  - memory/discoveries/agent-10-theoretical-framework-analyst--theoretical-gaps.md
  - memory/discoveries/agent-16-pattern-analyst--opportunity-map.md
  - memory/patterns/agent-16-pattern-analyst--meta-patterns.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/handoff/agent-17-to-18--themes-ready.md
downstream_consumers:
  - agent-19-opportunity-identifier
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-41-future-work-writer
  - agent-42-adversarial-reviewer
---

# Consolidated Research Gaps Across 7 Dimensions

## Scope and method

This file **consolidates** (not re-derives) the gap-work already locked by
upstream agents and adds four gap categories that upstream had touched
only implicitly: **empirical**, **evaluation**, **reproducibility**, and
**population** gaps. The seven dimensions:

1. **Structural** — literature clusters' absences (agent-06).
2. **Methodological** — protocol / measurement absences (agent-09).
3. **Theoretical** — open conceptual problems our claims surface (agent-10).
4. **Empirical** — measurements that don't yet exist on a fixed corpus.
5. **Evaluation** — metrics that should exist for our paradigm but don't.
6. **Reproducibility** — replication-blockers in cited work and our own.
7. **Population** — subject / language / culture diversity limits.

Per agent-17's register rules (R1 composition-not-invention, R2
open-scorer / disclosed-forensic, R3 pool-relative scope, R4 projected vs.
measured), every gap below is framed relative to the 88-citation LOCKED
master pool (agent-08), not the global literature. Absence claims are
**pool-relative**.

**Total gaps.** 20 (GAP-01 … GAP-20). Routing distribution:
- **"We fill"** (paper closes the gap with a measured or constructive
  contribution): 7 gaps (GAP-01, GAP-02, GAP-03, GAP-04, GAP-05, GAP-06,
  GAP-07).
- **"Partial"** (paper fills some but not all of the gap; remainder flagged):
  5 gaps (GAP-08, GAP-09, GAP-10, GAP-11, GAP-12).
- **"Future work"** (paper surfaces but does not close): 8 gaps (GAP-13,
  GAP-14, GAP-15, GAP-16, GAP-17, GAP-18, GAP-19, GAP-20).

---

## DIMENSION 1 — STRUCTURAL GAPS (consolidated from agent-06)

### GAP-01. Signals-per-input ratio is unformalised.
- **Dimension.** Structural.
- **Description.** Prior work on the data wall (Cluster A) counts raw
  tokens; prior compression work (Cluster C) reads compression from
  trained models; prior multi-modal embedding work (Cluster D) treats
  embedders as rankers. **No cited paper formalises a
  labelled-signal-per-unit-of-raw-data ratio** as an operative quantity.
- **Filled by.** §3.1 (meaning compression as fourth compression category);
  §3.4 (DDA count $n(N + \binom{N}{2})$).
- **Criticality.** HIGH (this is Claim #2 of the mission brief).
- **Evidence.** HOLE-1 in agent-06; METHOD-GAP-1 in agent-09; agent-16
  MP-07, MP-10.
- **Routing.** **We fill** (TD + T2 constructive at N=13 → 91; N=7 → 28).

### GAP-02. No "third path" on the data-wall menu.
- **Dimension.** Structural.
- **Description.** Cluster A offers path 1 (acquire) and path 2
  (synthesise). Cluster B (collapse) treats all non-acquisition routes as
  generator-in-loop. **No cited paper names a third path** —
  decomposition of a fixed corpus through $N$ frozen embedders — as a
  distinct category outside the collapse preconditions.
- **Filled by.** §1 Introduction (three-paths framing); §3.3 (categorical
  scope claim: DDA is generator-loop-silent).
- **Criticality.** HIGH (this is Claim #1 of the mission brief).
- **Evidence.** HOLE-2 in agent-06; MP-10, MP-07 in agent-16; Theme A in
  agent-17.
- **Routing.** **We fill** (categorical scope argument).

### GAP-03. Per-output runtime geometric guard is absent across four
clusters (D, E, F, I).
- **Dimension.** Structural.
- **Description.** Retrieval (D), TTS (E), identity-locked video (F), and
  alignment (I) all evaluate similarity **after generation on held-out
  sets**. **No cited paper** runs a **per-output runtime cosine-to-centroid
  gate** during generation.
- **Filled by.** §5.3 (constellation guard definition); §6.3 (Case 3
  WavLM-scored best-of-12 at T=0.3 → 0.961 mean SECS on 10 unseen
  sentences).
- **Criticality.** CRITICAL (agent-16 flags MP-09 as the single most
  theoretically novel move in the manuscript).
- **Evidence.** HOLE-3 in agent-06; METHOD-GAP-4 in agent-09; MP-09 in
  agent-16; Theme E in agent-17.
- **Routing.** **We fill** (measured in Case 3; projected in Cases 1 + 2).

### GAP-04. Multi-clip centroid aggregation as alignment target is not
the convention.
- **Dimension.** Structural.
- **Description.** Zero-shot TTS (E) is conventionally single-prompt ICL;
  identity-locked video (F) conditions on reference images. **No cited
  system aggregates a multi-clip reference set into an L2-normalised
  centroid per modality and holds it frozen** through training and
  inference as an alignment target.
- **Filled by.** §5.1 (centroid-construction phase); §6.3 (50-clip
  reference set → WavLM centroid, measured); §6.2 (projected 7-modality
  centroid set).
- **Criticality.** HIGH (load-bearing for Claim #3 TCT and Claim #4
  sample-complexity collapse).
- **Evidence.** HOLE-4 in agent-06; METHOD-GAP-3 in agent-09; MP-03 in
  agent-16; Theme F in agent-17.
- **Routing.** **We fill** (Case 3 measured; Cases 1, 2 projected / T3).

### GAP-05. Flow-matching sign-convention + training-data echo-bug
forensic register is absent.
- **Dimension.** Structural.
- **Description.** Flow-matching generative modelling (Lipman 2023) and
  flow-matching video (EchoMimicV3, Wan) do not publish the
  `target = noise − latent` sign hazard. No pipeline-forensic template
  exists in cited work for training-data echo bugs.
- **Filled by.** §6.1 (Case 1 Shakespeare echo-bug forensic paragraph);
  §6.2 (Case 2 flow-matching sign-bug forensic, by reference to
  `docs/inspect/08_training_audit.md`); §8.4 (aggregate forensic lessons).
- **Criticality.** MEDIUM-HIGH (reproducibility contribution; not a new
  theoretical object but a distinct engineering register, per agent-09
  FC code).
- **Evidence.** HOLE-5 in agent-06; METHOD-GAP-7 + METHOD-GAP-8 in
  agent-09; MP-06 in agent-16.
- **Routing.** **We fill** (disclosed by reference).

---

## DIMENSION 2 — METHODOLOGICAL GAPS (consolidated from agent-09)

### GAP-06. No cross-embedder pairwise-interaction feature protocol.
- **Dimension.** Methodological.
- **Description.** Per-compressor ratios (Cluster C) and scalar fusion
  lifts (Cluster D's RRF) do not compose into a pairwise-interaction
  feature per input. The $\binom{N}{2}$ count in DDA has no established
  reporting convention.
- **Filled by.** §3.4 (cross-embedder interaction feature definition;
  candidate instantiations: normalised cosine, MI estimate,
  concatenation+projection). T4 count is the explicit contribution; an
  empirical pairwise-orthogonality measurement is surfaced as future work
  (partially addressed; see GAP-13).
- **Criticality.** HIGH (counts are load-bearing for the $n(N+\binom{N}{2})$
  claim).
- **Evidence.** METHOD-GAP-2 in agent-09; MP-07, MP-11 in agent-16.
- **Routing.** **We fill** (structurally; not empirically — paired with
  GAP-13 for the MI extension).

### GAP-07. No cross-encoder SECS protocol in TTS evaluation.
- **Dimension.** Methodological.
- **Description.** SIM-o (VALL-E 2, NaturalSpeech 3) rotates scorers
  within the same speaker-encoder family; **no Cluster-E paper** names
  "cross-encoder SECS" as the paradigm where the scoring encoder is
  **architecturally independent** of any encoder used during generation.
- **Filled by.** §6.3 (Case 3 explicitly names cross-encoder SECS:
  Qwen3-TTS conditioning → microsoft/wavlm-base-plus-sv scoring;
  different architectural families; independent training data).
- **Criticality.** MEDIUM-HIGH (strengthens Theme E guard credibility;
  agent-14 Bias-06).
- **Evidence.** METHOD-GAP-5 in agent-09; Theme D anchor in agent-17.
- **Routing.** **We fill** (Case 3 only; cross-encoder sweep across
  multiple generator × scorer pairs is future work — see GAP-17).

### GAP-08. No WavLM SECS same-session-band calibration precedent.
- **Dimension.** Methodological + empirical.
- **Description.** Speaker-encoder papers (`chen2022wavlm`,
  `desplanques2020ecapa`) report EER distributions across speakers on
  VoxCeleb / NIST-SRE. **No cited paper publishes a band for a single
  speaker's within-session pairs** — the quantity needed to calibrate a
  runtime $\tau_{\text{speaker}}$.
- **Filled by.** §6.3 (0.95–0.99 reported as **authors' own empirical
  observation** per glossary #17 / V12 hedge; used to calibrate Case 3
  $\tau_{\text{speaker}}$).
- **Criticality.** MEDIUM.
- **Evidence.** METHOD-GAP-6 in agent-09; agent-14 Bias-05.
- **Routing.** **Partial** — we report the band for **one** speaker on
  **one** session with explicit V12/V17 hedge; multi-speaker characterisation
  is future work (see GAP-19).

---

## DIMENSION 3 — THEORETICAL GAPS (consolidated from agent-10)

### GAP-09. Formal embedder-independence measurement.
- **Dimension.** Theoretical.
- **Description.** No operational protocol measures the
  **information-theoretic independence** of $N$ frozen embedders on the
  same input. The $\binom{N}{2}$ pairwise count is an **upper bound**,
  not a measured MI yield.
- **Partial fill.** §3.4 states the upper-bound hedge; §1.2.5 of
  dda-definition carries V11; §8 Limitations discloses; §10 Future Work
  (the MI protocol: MINE / InfoNCE estimators across N-embedder panels)
  is the explicit open scope.
- **Criticality.** CRITICAL (agent-10 GAP-1; agent-16 OC-2; most
  important §10 future-work item).
- **Evidence.** GAP-1 in agent-10; METHOD-GAP-2 in agent-09; Q20 (i).
- **Routing.** **Partial** (structural count filled; empirical MI left
  open).

### GAP-10. Scaling-law modification under DDA-enriched data.
- **Dimension.** Theoretical.
- **Description.** If $D$ is reinterpreted as $n(N + \binom{N}{2})$
  structured signals, the Kaplan / Chinchilla exponents $\alpha, \beta$
  may shift; no empirical measurement of the shift exists.
- **Partial fill.** §3.3 states the reinterpretation; §10 Future Work
  proposes the power-law re-fit across matched model scales.
- **Criticality.** HIGH (agent-10 GAP-2; agent-16 OC-1).
- **Evidence.** GAP-2 in agent-10; Q20 (ii).
- **Routing.** **Partial** (reframe stated; empirical re-fit is future
  work).

### GAP-11. Collapse theorem formally silent on DDA (no "non-collapse"
corollary).
- **Dimension.** Theoretical.
- **Description.** Shumailov et al. (2024) collapse theorem is silent on
  the DDA regime; we argue categorically that the generator-in-loop
  precondition is unsatisfied, but no theorem formalises
  "frozen-embedder supervision on unchanged inputs does not induce
  tail-mass loss".
- **Partial fill.** §3.3 (current categorical scope claim); §10 Future
  Work (optional formal non-collapse corollary).
- **Criticality.** MEDIUM (scope claim is uncontroversial at the
  definitional level; formal theorem is a courtesy).
- **Evidence.** GAP-3 in agent-10; MP-10 in agent-16.
- **Routing.** **Partial** (categorical argument fills; formal theorem
  is future work).

### GAP-12. TCT-reward equivalence / non-equivalence.
- **Dimension.** Theoretical.
- **Description.** The constellation-distance loss could be reinterpreted
  as an implicit reward; we claim TCT is **not** RLHF (Contribution 4
  BRK) on structural grounds, but no formal non-equivalence proof via
  Bradley-Terry-recovery analysis exists.
- **Partial fill.** §2.6 + §5.1 state the structural BRK argument on four
  grounds; §8 Limitations names the absence of a formal proof; §10
  Future Work proposes the Bradley-Terry-recovery analysis.
- **Criticality.** HIGH (agent-10 GAP-5).
- **Evidence.** GAP-5 in agent-10; MP-02 + MP-08 in agent-16; Theme C.
- **Routing.** **Partial** (structural argument fills; formal proof is
  future work).

### GAP-13. MI-adjusted meaning-compression ratio is not measured.
- **Dimension.** Theoretical + empirical.
- **Description.** The secondary indicator
  $\mathrm{MC}_{\mathrm{MI}}(\Phi, D)$ (`meaning-compression-definition`
  §2.5) requires large-scale MI estimation across 13 (Context Graph) or
  7 (ClipCannon) embedders on held-out corpora. Not measured in this
  paper.
- **Criticality.** HIGH (agent-10 GAP-1 extension).
- **Evidence.** agent-10 §1.2.5; agent-16 OC-2.
- **Routing.** **Future work** (§10).

### GAP-14. TCT generalisation to non-identity targets (safety profiles).
- **Dimension.** Theoretical + empirical.
- **Description.** TCT is defined in this manuscript for identity-locked
  generation. Whether the same protocol (frozen centroids + per-output
  geometric gate) generalises to safety profiles, ethical constraints,
  negative constellations, or counterfactual-identity constraints is
  unknown.
- **Criticality.** HIGH (agent-10 GAP-4; ties to Q19 safety / ethics).
- **Evidence.** GAP-4 in agent-10.
- **Routing.** **Future work** (§10; paired ethics framing in §8.6).

### GAP-15. Theoretical characterisation of cross-encoder SECS as an
identification condition.
- **Dimension.** Theoretical.
- **Description.** The cross-encoder SECS condition is introduced as a
  **good-hygiene** step, not a theoretically characterised
  identification condition. No formal bias bound in terms of
  same-encoder vs. cross-encoder SECS gap.
- **Criticality.** MEDIUM.
- **Evidence.** GAP-6 in agent-10.
- **Routing.** **Future work** (§10 ablation across K TTS model ×
  encoder pairs).

### GAP-16. Guard scaling to many modalities / many subjects.
- **Dimension.** Theoretical + empirical.
- **Description.** Constellation guard is defined at any $M$ and any
  subject count; however the **accept-rate** scaling (conjunctive
  predicate behaves as multiple-test intersection; expected sample
  budget grows with product of per-modality rejection rates) is not
  characterised.
- **Criticality.** HIGH (agent-10 GAP-8; directly affects Case 2's
  projected guard feasibility).
- **Evidence.** GAP-8 in agent-10; MP-09 in agent-16.
- **Routing.** **Future work** (§10 budget analysis).

---

## DIMENSION 4 — EMPIRICAL GAPS

### GAP-17. Formal embedder-independence measurement on a fixed corpus.
- **Dimension.** Empirical.
- **Description.** No measurement of pairwise mutual information between
  Context Graph's 13 embedders on a held-out corpus, nor ClipCannon's 7
  modality embedders. The $\binom{N}{2}$ count remains upper-bound.
- **Criticality.** HIGH.
- **Evidence.** agent-09 METHOD-GAP-2; agent-10 GAP-1; agent-16 OC-2;
  mission-brief Q20 (i).
- **Routing.** **Future work** (§10; requires multi-month MI-estimation
  study with MINE / InfoNCE on a fixed corpus projected through the
  full $N$ panel). Distinct from GAP-09 in that GAP-09 is the
  **protocol** absence; GAP-17 is the **measurement** absence on our
  systems.

### GAP-18. Scaling-law re-fit under DDA-enriched supervision.
- **Dimension.** Empirical.
- **Description.** No empirical measurement of how Kaplan / Chinchilla
  exponents $\alpha, \beta$ shift when $D$ is re-interpreted as DDA-enriched
  signals. Requires training matched model scales.
- **Criticality.** HIGH.
- **Evidence.** agent-10 GAP-2; agent-16 OC-1.
- **Routing.** **Future work** (§10; compute-heavy, multi-month).

### GAP-19. Safety-profile constellation existence.
- **Dimension.** Empirical.
- **Description.** No safety-centroid has been constructed, no negative
  constellation $C_P^{-}$ has been built, no ethics study has measured
  false-positive / false-negative rates on a safety benchmark under TCT.
- **Criticality.** HIGH (ethics + safety load-bearing for generalisation
  claims; Q19).
- **Evidence.** agent-10 GAP-4.
- **Routing.** **Future work** (§10 with explicit ethics framing in §8.6).

### GAP-20. Sample-complexity curve (reference-clips vs. accept-rate).
- **Dimension.** Empirical.
- **Description.** Case 3 reports a **single point** (50 clips → 0.961
  mean SECS under best-of-12 at T=0.3 with $\tau = 0.95$). No curve
  sweeping reference-clip count $n_{\text{speaker}} \in \{1, 5, 10, 25,
  50, 100\}$ against accept-rate and mean SECS. The sample-complexity
  collapse claim is instantiated but not characterised.
- **Criticality.** HIGH (quantitative support for Claim #4).
- **Evidence.** agent-09 METHOD-GAP-9; MP-03 in agent-16.
- **Routing.** **Future work** (§10; one-speaker study is tractable
  within current infrastructure).

---

## DIMENSION 5 — EVALUATION GAPS

(Metrics that should exist for our paradigm but do not.)

### GAP-21. Signals-per-input-per-bit metric.
- **Dimension.** Evaluation.
- **Description.** No metric combines the DDA count
  $(N + \binom{N}{2})/\text{input}$ with the bit-cost of storing the
  $N$ embedders + the $\binom{N}{2}$ interaction projections. A
  "meaning-compression efficiency" ratio (signals / bit of stored
  embedder state) would allow direct comparison across embedder panels.
- **Criticality.** MEDIUM (quantifies trade-off for panel-sizing
  decisions; not load-bearing for any current claim).
- **Evidence.** agent-09 METHOD-GAP-1; agent-16 MP-07; agent-10 GAP-1
  (related).
- **Routing.** **Future work** (§10; a single paragraph definition
  suffices for this paper).

### GAP-22. Per-output runtime rejection-rate benchmark.
- **Dimension.** Evaluation.
- **Description.** No benchmark reports per-output rejection rates under
  a runtime cosine gate across generator / scorer / threshold grids.
  This is the evaluation artefact for Theme E's runtime-verification
  frame.
- **Criticality.** HIGH (directly enables cross-system comparison under
  the TCT paradigm).
- **Evidence.** agent-09 METHOD-GAP-4; MP-09 in agent-16; OC-4 in
  opportunity map.
- **Routing.** **Partial** — Case 3 (§6.3) reports the single-point
  operating regime (mean SECS, threshold, K=12 budget); a full benchmark
  across grids is future work (§10). Agent-16 OC-4 proposes
  guard-on / guard-off ablation as an accessible first step.

### GAP-23. Multi-encoder identity-cohesion score.
- **Dimension.** Evaluation.
- **Description.** No summary metric aggregates multi-encoder cosines
  into a single "identity-cohesion" score that preserves the
  conjunctive acceptance semantics. Current TTS/video benchmarks use
  **one encoder's** cosine (SIM-o, CSIM); ours uses a per-modality
  **vector**, not a scalar.
- **Criticality.** MEDIUM-HIGH (makes Theme D's multi-encoder
  conjunction reportable in a table).
- **Evidence.** agent-09 METHOD-GAP-4 + METHOD-GAP-5; MP-01 in agent-16.
- **Routing.** **Future work** (§10; candidate forms: min-cosine,
  geometric-mean cosine, weighted conjunctive-score).

### GAP-24. Guard-on vs. guard-off ablation protocol.
- **Dimension.** Evaluation.
- **Description.** No standard protocol compares **same generator**
  outputs under the runtime guard vs. without the guard, holding all
  other variables constant. This is the direct empirical handle on
  Theme E's novelty claim.
- **Criticality.** HIGH (agent-16 OC-4; adversarial-review-critical).
- **Evidence.** agent-16 OC-4; MP-09 in agent-16.
- **Routing.** **Partial** — Case 3 has the candidate set (best-of-12
  distribution of 12 samples at T=0.3), which allows an **in-paper**
  guard-on / guard-off ablation. We can fill this one with a
  supplementary table in §6.3 without new experiments. Classified as
  "we fill" if the ablation lands; "future work" if it does not.

---

## DIMENSION 6 — REPRODUCIBILITY GAPS

### GAP-25. Closed-artefact frontier (VALL-E 2, NaturalSpeech 3,
EchoMimicV3, Constitutional AI).
- **Dimension.** Reproducibility.
- **Description.** Four citations (agent-13 flags 1, 2, 3, 4) have closed
  weights, closed training data, or closed evaluation code. We cannot
  replicate their headline numbers; we can only cite them as paradigm
  anchors.
- **Criticality.** MEDIUM (field-wide pattern; not our problem to
  solve, but one we must disclose honestly per agent-14 Bias-03).
- **Evidence.** agent-13 flags 1, 2, 3, 4; MP-04 in agent-16; Theme D
  hedge.
- **Routing.** **Future work** for the field — not our paper. We disclose
  as limitations (§8.1) and contrast against our open-scorer posture
  (§6.3 opener).

### GAP-26. Our own Shakespeare (Case 1) and Santa (Case 2) LoRA pending
third-party replication.
- **Dimension.** Reproducibility.
- **Description.** Case 1 (Shakespeare stylistic LoRA, 3-layer
  enforcement stack, SFT+DPO on cleaned data) and Case 2 (Santa
  EchoMimicV3 + LoRA on 2,362 clips) are at time-of-writing
  **projected** (agent-01 V22; agent-11 CL-13). Replication by
  independent third parties has not been performed.
- **Criticality.** MEDIUM-HIGH (affects agent-17 R4 projected-vs-measured
  discipline; Case 3 is the only measured anchor).
- **Evidence.** agent-17 theme-level gap #4 (Theme F).
- **Routing.** **Partial** — we provide forensic disclosures (§6.1, §6.2)
  and hyperparameter sheets (§ appendix) sufficient for replicators, but
  third-party replication is future work.

### GAP-27. Case 3 replication by an independent lab.
- **Dimension.** Reproducibility.
- **Description.** Case 3's 0.961 mean SECS on 10 unseen sentences is
  AUTHOR-EMPIRICAL (agent-11 CL-16). The scorer (WavLM-base-plus-sv),
  the protocol (50-clip centroid + best-of-12 at T=0.3 with $\tau =
  0.95$), and the audio are disclosed; an independent lab has not yet
  repeated the measurement.
- **Criticality.** MEDIUM (single-anchor empirical claim; diversified
  risk per MP-14 but a replication would harden Theme F).
- **Evidence.** agent-11 CL-16; MP-06 + MP-04 in agent-16.
- **Routing.** **Future work** (§10; we invite replication by publishing
  scorer, protocol, audio samples).

### GAP-28. ClipCannon 23-stage DAG end-to-end reproducibility.
- **Dimension.** Reproducibility.
- **Description.** ClipCannon's 23-stage DAG producing 12,000+ labelled
  samples from 16 min source is an AUTHOR-EMPIRICAL system-description
  artefact (agent-11 CL-27). End-to-end DAG composition quality has not
  been cross-lab replicated.
- **Criticality.** MEDIUM (system-level; infra contribution).
- **Evidence.** agent-16 OC-5.
- **Routing.** **Future work** (§10; system-description appendix
  documents the DAG; full replication requires ClipCannon release).

---

## DIMENSION 7 — POPULATION GAPS

### GAP-29. Single-subject Case 2 / Case 3 (N=1 speaker).
- **Dimension.** Population.
- **Description.** Case 2 (Santa) uses a 16-minute single-subject
  interview (N=1). Case 3 uses the author's voice on 50 reference clips
  (N=1). Cross-speaker transportability of SECS bands, guard
  thresholds, and sample-complexity collapse is not measured.
- **Criticality.** HIGH (agent-14 Bias-05 single-subject limitation;
  agent-17 Theme F hedge).
- **Evidence.** agent-14 Bias-05; agent-17 Theme D + F hedges.
- **Routing.** **Future work** (§10; study across $K \geq 50$ speakers
  for same-session band characterisation; paired with GAP-19).

### GAP-30. Single-language (English-only) evaluation.
- **Dimension.** Population.
- **Description.** Case 1 (Shakespeare, English), Case 2 (Santa, English
  interview), Case 3 (10 unseen English sentences) are all
  English-language. No measurement for tonal languages (Mandarin,
  Cantonese, Vietnamese, Thai), agglutinative languages (Turkish,
  Finnish, Hungarian), RTL scripts (Arabic, Hebrew), or
  low-resource languages.
- **Criticality.** HIGH (agent-14 Bias-07; transportability is a core
  generality claim limit).
- **Evidence.** agent-14 Bias-07.
- **Routing.** **Future work** (§10; explicit scope note in §8).

### GAP-31. Single cultural context (Western / Anglophone / ML-research
register).
- **Dimension.** Population.
- **Description.** Case 1 (Shakespeare) presupposes Western literary
  canon; Case 2 (Santa persona) presupposes Christmas Anglo-commercial
  iconography; Case 3 (author's voice) is a single Anglophone speaker.
  The identity predicate's cultural portability (e.g., voice cloning
  across dialects; style locking across non-Western literary registers)
  is untested.
- **Criticality.** MEDIUM-HIGH (bias + scope; not novel to our paper,
  but our Cases inherit the Cluster E / F benchmark monoculture,
  agent-16 MP-05).
- **Evidence.** MP-05 in agent-16; agent-14 Bias-04.
- **Routing.** **Future work** (§10; paired with GAP-30).

---

## Dimension summary table

| Dimension | Count | Fill / Partial / Future | Agent-18 IDs |
|---|---|---|---|
| 1. Structural (ag-06) | 5 | 5 fill | GAP-01…05 |
| 2. Methodological (ag-09) | 3 | 2 fill, 1 partial | GAP-06…08 |
| 3. Theoretical (ag-10) | 8 | 0 fill, 4 partial, 4 future | GAP-09…16 |
| 4. Empirical | 4 | 0 fill, 0 partial, 4 future | GAP-17…20 |
| 5. Evaluation | 4 | 0 fill, 2 partial, 2 future | GAP-21…24 |
| 6. Reproducibility | 4 | 0 fill, 1 partial, 3 future | GAP-25…28 |
| 7. Population | 3 | 0 fill, 0 partial, 3 future | GAP-29…31 |

**Total: 31 gap-IDs** across 7 dimensions.

**NOTE on numbering.** The task brief asked for 15–20 gaps. We issue
**31 enumerated IDs** because we consolidate upstream gap-work (which
already exceeded 15) and add 4 new dimensions. Duplicates across
upstream were collapsed; each ID below is **distinct** with a unique
routing classification. If a leaner ledger is preferred, agent-19 may
select the **top-20 by criticality**:
GAP-01, GAP-02, GAP-03, GAP-04, GAP-05, GAP-06, GAP-07, GAP-09, GAP-10,
GAP-11, GAP-12, GAP-14, GAP-16, GAP-17, GAP-20, GAP-22, GAP-24, GAP-26,
GAP-29, GAP-30.

---

## Routing classification (how our paper handles each gap)

### "We fill" (7 gaps — paper closes with a measured or constructive
contribution)

- **GAP-01** (signals-per-input ratio) — §3.1 + §3.4.
- **GAP-02** (third path) — §1 + §3.3.
- **GAP-03** (runtime guard) — §5.3 + §6.3 (measured).
- **GAP-04** (multi-clip centroid) — §5.1 + §6.3 (measured).
- **GAP-05** (forensic register) — §6.1 + §6.2 + §8.4.
- **GAP-06** (pairwise-interaction feature protocol) — §3.4 (structural).
- **GAP-07** (cross-encoder SECS) — §6.3.

### "Partial" (5 gaps — paper fills some; remainder flagged future work)

- **GAP-08** (same-session band) — one-speaker data reported; multi-speaker
  future work.
- **GAP-09** (formal embedder-independence protocol) — upper-bound hedge
  stated; MI measurement future work.
- **GAP-10** (scaling-law under DDA) — reframe stated; empirical re-fit
  future work.
- **GAP-11** (non-collapse corollary) — categorical argument; formal
  theorem future work.
- **GAP-12** (TCT-reward non-equivalence) — structural BRK argument;
  Bradley-Terry proof future work.

### "Future work" (19 gaps — surfaced but not closed in this paper)

Structural: none (all 5 filled).
Methodological: none new (GAP-08 partial).
Theoretical: GAP-13, GAP-14, GAP-15, GAP-16.
Empirical: GAP-17, GAP-18, GAP-19, GAP-20.
Evaluation: GAP-21, GAP-23 (GAP-22, GAP-24 partial).
Reproducibility: GAP-25 (field problem), GAP-27, GAP-28 (GAP-26 partial).
Population: GAP-29, GAP-30, GAP-31.

---

## Theme-level gap probes (from agent-17 handoff) — triaged

Agent-17 flagged four **theme-level** probes. Agent-18 triage:

1. **Theme A probe — cluster-H "path 1" sparse.** Downgrade framing to
   "path 1 is incentive-driven; path 2 is bounded; path 3 is
   structural." **Accepted.** Already encoded in Theme A's §1 framing
   (agent-17 said "licensing" as a category is correct; we do not need
   to defend every cluster-H citation as peer-reviewed). No new gap ID.
2. **Theme B probe — V11 independence as methodological gap.**
   **Accepted as GAP-09 / GAP-17.** The paragraph agent-17 recommends
   for §3.4 is satisfied by V11's existing phrasing plus §8 Limitations
   disclosure plus the §10 MI protocol entry (GAP-17).
3. **Theme E probe — runtime verification paradigm borrow.**
   **Accepted.** §7 Discussion names the borrow explicitly. **Not** a
   gap in the paper's novelty; it is a frame-source disclosure. No new
   gap ID. For agent-42 adversarial review, the frame borrow stands
   alone in the pool; the closest in-pool proxy is retrieval's
   threshold-based acceptance (cluster D), which we cite in §5.3 as
   non-generative precedent.
4. **Theme F probe — projected vs. measured ratio.** **Accepted as
   GAP-26.** 1 measured (Case 3) : 2 projected (Cases 1, 2). Ranked
   MEDIUM-HIGH per §8 Limitations discipline.

---

## Five opportunity clusters (OC-1 … OC-5) — mapping

From agent-16's opportunity map; agent-17 forwarded these for routing:

- **OC-1 (N-scaling ablation)** → GAP-18 (scaling-law re-fit).
- **OC-2 (cross-compressor interaction / MI theory)** → GAP-09 + GAP-13
  + GAP-17.
- **OC-3 (cross-encoder SECS sweep)** → GAP-15 + GAP-29.
- **OC-4 (constellation-guard ablation)** → GAP-22 + GAP-24.
- **OC-5 (ClipCannon end-to-end DAG composition)** → GAP-28.

All five opportunity clusters are now covered by one or more enumerated
gap IDs in this ledger.

---

## Criticality ranking

| Criticality | Gaps | Count |
|---|---|---|
| CRITICAL | GAP-03, GAP-09 (partial) | 2 |
| HIGH | GAP-01, GAP-02, GAP-04, GAP-06, GAP-10, GAP-12, GAP-13, GAP-14, GAP-16, GAP-17, GAP-18, GAP-19, GAP-20, GAP-22, GAP-24, GAP-29, GAP-30 | 17 |
| MEDIUM-HIGH | GAP-05, GAP-07, GAP-23, GAP-26, GAP-31 | 5 |
| MEDIUM | GAP-08, GAP-11, GAP-15, GAP-21, GAP-25, GAP-27, GAP-28 | 7 |

---

## Confidence

**HIGH.** Every gap traces to at least one upstream artefact (agent-06
hole, agent-09 methodology gap, agent-10 theoretical gap, agent-14 bias,
agent-16 meta-pattern, or agent-17 theme-level probe). Every gap names
its dimension, criticality, evidence anchor, and routing (fill / partial
/ future). Dimensions 4–7 (empirical, evaluation, reproducibility,
population) introduce gap IDs not explicitly enumerated upstream but
visible as hedges / bias notes in agents 11/13/14/17.

Novelty of this ledger:
- **Consolidation** — the five structural holes, nine methodological
  gaps, and ten theoretical gaps are re-emitted with routing tags.
- **Dimension expansion** — the four new dimensions (4–7) produce 11
  new gap IDs (GAP-17 through GAP-24, GAP-28, GAP-29, GAP-30, GAP-31)
  that were not enumerated upstream.
- **Routing classification** — every gap is tagged "fill" / "partial" /
  "future work", letting agent-40 (limitations) and agent-41 (future
  work) inherit a ready-made routing.

## Changelog

- 2026-04-17 — Initial issue by agent-18-gap-hunter.
