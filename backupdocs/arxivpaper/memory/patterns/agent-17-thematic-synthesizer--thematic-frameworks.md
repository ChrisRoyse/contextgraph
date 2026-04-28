---
namespace: patterns
agent_id: agent-17-thematic-synthesizer
timestamp: 2026-04-17T02:05:00Z
status: active
locked: false
supersedes: none
tags: thematic-framework, narrative-spine, paper-structure, 6-themes, spine-per-section, hedged
depends_on:
  - memory/context/mission-brief.md
  - memory/patterns/agent-15-evidence-synthesizer--thematic-synthesis-blocks.md
  - memory/patterns/agent-16-pattern-analyst--meta-patterns.md
  - memory/discoveries/agent-16-pattern-analyst--opportunity-map.md
  - memory/handoff/agent-16-to-17--patterns-ready.md
  - memory/patterns/agent-14-bias-detector--bias-mitigations.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
downstream_consumers:
  - agent-18-gap-hunter
  - agent-35-abstract-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-systems-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Thematic Frameworks — Six Higher-Order Themes as Manuscript Spines

## Scope and method

Agent-15 issued 7 paste-ready thematic blocks (Themes 1–7) grounded in
the 9 literature clusters A–I. Agent-16 abstracted those blocks into 16
meta-patterns (MP-01 through MP-16) across three structural categories
(literature absences, monocultures/biases, architectural inversions).

This file performs the **thematic synthesis move**: collapse the 7
thematic blocks + 16 meta-patterns into **6 higher-order themes** (A
through F), each of which:

1. States a single **thesis-level sentence** that a reader should carry
   forward from the manuscript.
2. Names the **meta-patterns** (from agent-16) that support it.
3. Names the **canonical citations** (from the LOCKED master pool) that
   anchor it.
4. Names the **manuscript section(s)** it spines.
5. Carries the **hedge** per agent-14 bias mitigations and agent-13
   quality flags.

The six themes are the manuscript's **narrative skeleton**. Every
section-writer downstream (agents 35, 36, 37, 38, 39, 40) should choose
exactly one theme as the section's primary spine and may reference
other themes as secondary moves.

**Discipline preserved from upstream.**
- Master pool LOCKED (agent-08). No new citations.
- Cluster labels A–I LOCKED (agent-06).
- Evidence grades LOCKED (agent-11).
- Quality flags LOCKED (agent-13).
- Bias mitigations SOFT-LOCKED verbatim (agent-14).
- Thematic blocks SOFT-LOCKED (agent-15) — re-composed, not rewritten.
- Meta-patterns SOFT-LOCKED (agent-16) — re-composed, not rewritten.
- No novel theoretical objects introduced (agent-10 EXT / CAT-EXT /
  BRK / ORTH codes inherited verbatim).

**Validation vs. the task-brief candidate list.** The task brief
proposed six candidate themes (A data-wall reframe, B role-flip, C
scalar-reward insufficiency, D single-encoder gap, E runtime
verification, F sample-complexity collapse). Agent-17's validation:

- **Themes A, B, C, E retained as-is** (direct one-to-one mapping to
  the candidate list with minor renaming).
- **Theme D (single-encoder gap) and candidate C (scalar-reward) are
  structurally adjacent** — both name a load-bearing single-point-of-
  evaluation absence. Agent-17 keeps them distinct because D anchors
  §2.5 / §5.3 (the multi-modal constellation guard move) and C anchors
  §2.6 / §5.1 (the geometric-vs-scalar alignment move). Conflation
  would lose section-level specificity.
- **Theme F retained and reframed as the empirical-demonstration
  theme** — it is the only theme with a fully-measured author-empirical
  anchor (Case 3, 0.961 mean SECS).

Final set: **six themes (A–F)**.

---

## Theme A — The data wall is an input-supply problem, and a third
structural path exists around it

### Thesis (one sentence)

The exhaustion of accessible public training text is best read not as a
theoretical ceiling on ML capability but as an **input-supply problem**,
which admits a third structural path — **decompose a fixed corpus
through N frozen independent embedders to multiply supervisory signals
per input** — that the data-wall literature has not considered alongside
the two paths it does consider (license more; synthesise more).

### Supporting meta-patterns

- **MP-10** (DDA-as-structural-third-path gap) — primary.
- **MP-07** (compression-reframing opportunity) — secondary;
  legitimises the operator direction.
- **MP-15** (temporal-narrative-moment) — hedge-bearer; distinguishes
  peer-reviewed stock-exhaustion claims from Q4-2024 industry signal.

### Anchor citations (from the LOCKED pool)

- `villalobos2024data` (T1, MODERATE CL-02) — effective high-quality
  text stock exhausted 2026–2032.
- `longpre2024consent` (T1, SINGLE-SOURCE) — ~25 % of C4 crawl
  inaccessible since 2023 via robots.txt / ToS opt-outs.
- `hoffmann2022chinchilla` (T1) + `kaplan2020scaling` (T2) — ~20
  tokens/parameter compute-optimal ratio sharpens the stock tension.
- `shumailov2024nature` (T1, Nature, CL-04 STRONG) +
  `dohmatob2024tails` (T1, ICML) + `dohmatob2024regression` (T1,
  NeurIPS) + `alemohammad2023mad` (T1, ICLR) — generator-in-loop
  synthetic-data collapse theorems close path 2.
- `gerstgrasser2024colm` (T2) + `feng2024verification` (T2) — collapse
  mitigations that remain *inside* the generator-in-loop precondition.
- Tier-3 Q4-2024 industry signal (`woo2024information`,
  `metz2024bloomberg`, `hu2024reuters`, `franzen2024techcrunch`,
  `sutskever2024neurips`) — framed as **one dated narrative**, not
  five independent confirmations (Bias-10 mitigation).

### Manuscript section spined by this theme

- **Primary spine:** §1 Introduction (three-paths framing and the
  reframing move); §3.3 (categorical scope argument: collapse-silent
  because preconditions unsatisfied).
- **Secondary placements:** §2.1 (literature positioning; path 1 vs.
  path 2); §8 Limitations (temporal-narrative-moment hedge).

### Hedge and scope-bound

- Pool-relative absence claim per MP-16 (no exhaustive global search).
- DDA is **EXT** of scaling-law data-axis (agent-10), not BRK; we do
  not claim to replace scaling-law theory.
- DDA is **ORTH** to collapse theory (agent-10 cluster-B row); the
  categorical argument is that generator-in-loop preconditions are not
  satisfied by frozen-embedder decomposition of real inputs, not that
  DDA has a new immunity theorem.
- No AGI claim; no "imminent plateau" claim (mission-brief hard
  constraint).
- Industry signals carry Bias-10 "one dated narrative" hedge verbatim.

### Novelty delta

- EXT of cluster A (scaling-law data-axis) + ORTH of cluster B
  (collapse theory) per agent-10.

---

## Theme B — Role-flip: frozen embedders serve three semantic roles
(retriever → labeller → runtime guard) without retraining

### Thesis (one sentence)

The modern embedder family, cited throughout the retrieval literature
as a **ranking** substrate, admits two additional roles the cited
literature has not assigned it — **labeller at training-data
preparation time** and **runtime acceptance predicate at inference
time** — and the single architectural move that unifies this paper's
three system contributions (DDA, Context Graph, TCT's constellation
guard) is the disciplined composition of those three roles over the
*same* frozen embedder instances.

### Supporting meta-patterns

- **MP-11** (role-flip: retriever → labeller → guard) — primary;
  agent-16 identifies this as one of three load-bearing novelty
  patterns.
- **MP-01** (single-encoder evaluation) — enabling absence; the
  cluster-D / E / F literature never composes multiple encoders at
  runtime, which is what the guard role does.
- **MP-09** (per-output runtime verification gap) — enabling absence;
  the guard role imports runtime verification into generative ML.

### Anchor citations

- `khattab2020colbert` (T2) + `santhanam2022colbertv2` (T1 NAACL) —
  late-interaction retrieval.
- `chen2022wavlm` (T1 IEEE JSTSP) — speaker self-supervised retrieval.
- `zhai2023siglip` (T2) — sigmoid CLIP retrieval.
- `formal2021splade` (T2) — sparse lexical-expansion retrieval.
- `wang2021kepler` (T2) + `bordes2013transe` (T1) — KG structural
  embeddings.
- `malkov2018hnsw` (T1, SINGLE-SOURCE foundational, CL-24) — ANN
  infrastructure enabling the role composition at scale.
- `cormack2009rrf` (T2) — reciprocal-rank fusion across heterogeneous
  projections.
- `nussbaum2024nomic` (T2) — long-context text embeddings.
- Evaluation across all cited embedder papers is a **retrieval
  metric** (NDCG, Recall@k, EER, MAP) — role-flip absent (agent-15
  Observation 1).

### Manuscript section spined by this theme

- **Primary spine:** §3 Framework opener (introduce role-flip as the
  unifying architectural move before systems §§); §4.1 Context Graph
  (retriever role instantiated); §5 TCT (guard role instantiated).
- **Secondary placements:** §3.4 DDA counting (labeller role is what
  `N + C(N,2)` counts over); §7 Discussion (composition-simplicity
  claim).

### Hedge and scope-bound

- V11 "approximately independent; strict formal orthogonality is an
  open question" carries through every use of the role-flip argument
  (agent-10 D-FG-2; agent-11 CL-11 MODERATE).
- MP-11 is a survey-level absence claim relative to the 88-citation
  thesis-aligned pool (MP-16). A single pool-external paper that
  assigns embedders to two of the three roles would weaken the
  novelty claim; it would not invalidate it because the **three-role
  composition over the same frozen instances** is the specific move.
- Role-flip is a **composition-level** move (MP-12), not a new
  theoretical primitive. Agent-10 marks DDA (role-flip in cluster D)
  as **EXT**, constellation guard (runtime-gate role) as **BRK**
  across D / E / F / I.

### Novelty delta

- EXT of cluster D (embedder substrate, role re-assignment) +
  BRK of clusters D, E, F, I via the runtime-gate role (agent-10).

---

## Theme C — Scalar-reward alignment is structurally insufficient for
embedding-definable targets; geometric multi-modal alignment is the
complementary paradigm

### Thesis (one sentence)

Canonical alignment theory — InstructGPT, DPO, Constitutional AI —
reduces alignment quality to optimisation of **a single scalar reward**
under a KL-to-reference-policy regulariser, and the three absences this
paradigm *definitionally* carries (no geometric target, no multi-modal
conjunction, no per-output runtime predicate) are precisely the three
structural moves Teleological Constellation Training (TCT) makes, so
TCT reads as a structural complement to the scalar-reward paradigm
rather than an incremental improvement on it.

### Supporting meta-patterns

- **MP-02** (scalar-reward alignment pattern) — primary.
- **MP-08** (constellation-is-a-prediction-target opportunity) —
  primary; the geometric-alignment-target absence.
- **MP-12** (composition-not-invention) — framing; TCT is a
  composition of cluster-D geometry and cluster-I alternative, not a
  new theoretical primitive.

### Anchor citations

- `ouyang2022instructgpt` (T1 NeurIPS 2022) — reward model on
  preference pairs → PPO.
- `rafailov2023dpo` (T1 NeurIPS 2023) — closed-form reward via
  `β log(π / π_ref)`.
- `bai2022constitutional` (T2; agent-13 Flag 3 RED closed-artefact) —
  AI-feedback preferences, still scalar-reward structured.
- `zhai2023siglip` (T2) + `chen2022wavlm` (T1) + `khattab2020colbert`
  (T2) — cluster-D geometric substrate TCT extends.

### Manuscript section spined by this theme

- **Primary spine:** §2.6 Related Work (scalar-reward contrast
  table); §5.1 TCT theoretical framing; §5.2 Phase-2 loss definition.
- **Secondary placements:** §7 Discussion (geometric-vs-scalar
  alignment); §8 Limitations (scope-device note per agent-14 Bias-02).

### Hedge and scope-bound

- Citing the three alignment anchors (InstructGPT, DPO, Constitutional
  AI) is a deliberate **scope device** per agent-14 Bias-02, not a
  survey claim. We do not speak to RLAIF, KTO, IPO, ORPO or the
  broader reward-modelling landscape — that extension is named future
  work (§9, per OC-3 agent-16 opportunity map).
- Constitutional AI is cited only as **paradigm anchor**, never as
  reproducible empirical evidence (agent-13 Flag 3, closed weights,
  closed RLAIF data).
- Cluster-I triad is US-only (OpenAI, Stanford, Anthropic) per agent-14
  Bias-03 — also a deliberate scope device.
- TCT is **BRK** with respect to scalar-reward theory (agent-10) and
  **EXT** of cluster-D contrastive geometry (positive-only, frozen
  metric). It is not a new theoretical primitive (MP-12).

### Novelty delta

- BRK of cluster I (scalar-reward alignment) + EXT of cluster D
  (contrastive geometry) per agent-10.

---

## Theme D — The single-encoder evaluation gap: multi-modal
conjunction is a stronger identity predicate than any cited TTS or
identity-locked-video benchmark uses

### Thesis (one sentence)

Every cited TTS and identity-locked-video paper scores identity
similarity with **one speaker / face encoder at a time**, often from
the same family as the conditioning encoder, so the field's notion of
"identity-locked" is a within-encoder property — and replacing it with
a **multi-encoder conjunction predicate** (`∀ m : cos(φ_m(ŷ), c_m) ≥
τ_m`) gives a strictly stronger identity claim, which is a fresh
evaluation move across four simultaneous clusters.

### Supporting meta-patterns

- **MP-01** (single-encoder evaluation) — primary.
- **MP-05** (benchmark monoculture) — secondary; narrow benchmark
  vocabulary per cluster entrenches the single-encoder habit.
- **MP-04** (closed-artefact industry) — secondary; closed artefacts
  make cross-encoder audit infeasible on frontier numbers.

### Anchor citations

- **Cluster E (TTS):** `chen2024valle2` (T2; agent-13 Flag 1 RED
  closed-artefact), `tan2024naturalspeech3` (T1 ICML; agent-13 Flag 4
  AMBER), `chen2024f5tts` (T2), `casanova2024xtts` (T2),
  `wang2024maskgct` (T1 ICLR), `shen2023naturalspeech2` (T2),
  `le2023voicebox` (T2), `wang2023valle` (T2), `jiang2023megatts2` (T2).
- **Cluster F (video):** `meng2025echomimicv3` (T2; agent-13 Flag 2
  AMBER), `cui2024hallo3` (T2), `zheng2025hunyuanvideo` (T2),
  `tian2024emo` (T2), `tian2025emo2` (T2), `yang2024cogvideox` (T2),
  `wan2025video` (T2), `li2024latentsync` (T2).
- **Evaluation infrastructure:** `chen2022wavlm` (T1),
  `desplanques2020ecapa` (T2), `reddy2022dnsmos` (T2),
  `saeki2022utmos` (T2), `christop2025cloneval` (T2).

### Manuscript section spined by this theme

- **Primary spine:** §2.5 Related Work (TTS and video comparators);
  §5.3 TCT constellation guard definition; §6.3 Case 3 cross-encoder
  SECS protocol (Qwen3-TTS conditioning → WavLM scoring).
- **Secondary placements:** §6.2 Case 2 projected 7-modality guard
  (Santa identity-locked clone).

### Hedge and scope-bound

- Bias-06 "single-encoder transportability not asserted" carries
  forward: Case 3 uses cross-encoder (Qwen3-TTS → WavLM) but is
  **single-speaker / single-encoder-pair / English-only**
  (agent-14 Biases 05, 06, 07). Generalisation to additional
  generator × scorer pairs is named future work (OC-3).
- VALL-E 2 and NaturalSpeech 3 are cited only as paradigm anchors,
  never as independently reproducible benchmarks (agent-13 Flags 1
  and 4).
- The **evaluation monoculture** (LibriSpeech / LibriTTS / VoxCeleb
  test pools) is precisely what makes a centroid-aggregated,
  cross-encoder, runtime-gated protocol both unprecedented and
  informative — but also limits cross-corpus transportability claims
  (agent-15 Theme 5 caveat).
- Microsoft × 4 TTS affiliations paired deliberately with open
  comparators per agent-14 Bias-03 mitigation.

### Novelty delta

- BRK across clusters D / E / F (multi-encoder runtime predicate
  absent from all three) per agent-10.

---

## Theme E — Runtime verification is a paradigm import from formal
methods; per-output geometric guarantees are novel in generative ML

### Thesis (one sentence)

Formal methods has a mature tradition of **runtime verification** —
monitoring each output against a specification and rejecting violators
— and ML evaluation has a mature tradition of **batch metrics** over
held-out test sets; no cited paper in clusters D, E, F, or I composes
the two, so a **per-output geometric acceptance predicate at inference
time** is a structurally novel move in generative ML, and the
constellation-guard formulation is its first concrete instantiation
over multi-modal frozen-embedder targets.

### Supporting meta-patterns

- **MP-09** (per-output runtime verification gap) — primary; agent-16
  identifies this as the single most theoretically novel move in the
  manuscript.
- **MP-08** (constellation-as-prediction-target) — primary; the
  geometric target the runtime check references.
- **MP-01** (single-encoder evaluation) — enabling absence; the
  runtime check asserts multi-encoder conjunction, which is absent
  from cluster-E / F evaluation.

### Anchor citations

- **Cluster D (retrieval):** `khattab2020colbert`, `chen2022wavlm`,
  `zhai2023siglip`, `nussbaum2024nomic`, `formal2021splade`,
  `wang2021kepler`, `malkov2018hnsw`, `cormack2009rrf` — all
  retrieval-metric evaluation; no per-output acceptance semantics
  (agent-10 D-FG-1).
- **Cluster E (TTS):** SIM-o / SECS / DNSMOS / UTMOS all post-hoc on
  test sets (`chen2024valle2`, `tan2024naturalspeech3`,
  `chen2024f5tts`, `casanova2024xtts`); agent-09 E-METHOD-GAP-2 names
  the runtime-cosine-gate absence.
- **Cluster F (video):** CSIM / FVD / LSE-D post-hoc
  (`meng2025echomimicv3`, `cui2024hallo3`, `tian2024emo`); agent-09
  F-METHOD-GAP-2 confirms.
- **Cluster I (alignment):** `ouyang2022instructgpt`,
  `rafailov2023dpo`, `bai2022constitutional` — distribution-shaped
  policies; no inference-time gate (agent-10 I-FG-2).

### Manuscript section spined by this theme

- **Primary spine:** §5.3 TCT constellation-guard definition; §5.4
  per-output predicate; §6.3 Case 3 measured instantiation
  (WavLM-SECS best-of-12 at T=0.3, 0.95 threshold → 0.961 mean SECS).
- **Secondary placements:** §7 Discussion (runtime verification meets
  generative ML as a paradigm import); §1 Introduction framing move
  (per agent-16 open item 1 — MP-09 promotion recommendation).

### Hedge and scope-bound

- Case 3 is **AUTHOR-EMPIRICAL** (CL-16) on a **single subject**
  (agent-14 Bias-05); the runtime-guard demonstration is existence-
  proof, not generality (agent-14 Bias-05 + 06 + 07).
- Case 2 (video, 5-modality guard) is T3-projected; Case 1
  (Shakespeare text-style guard) is T3-projected. Only Case 3's
  guard is measured.
- Runtime-verification terminology is imported from formal methods,
  not from papers in the LOCKED pool; this is a **paradigmatic frame
  borrow**, not a citation within the pool. §7 Discussion must name
  this frame borrow explicitly and not claim it as a novel theoretical
  object (MP-12 composition-not-invention).
- Guard-on vs. guard-off ablation is available in Case 3 (best-of-12
  candidate distribution) and named as an opportunity by agent-16
  (OC-4).

### Novelty delta

- BRK across clusters D, E, F, I — agent-10 marks MP-09 as the single
  most theoretically novel absence in the pool.

---

## Theme F — Sample-complexity collapses under TCT: identity-locked
generation from single-subject datasets becomes tractable

### Thesis (one sentence)

Voice-cloning and identity-locked video currently depend on **many
subject-specific examples** and a **single reference prompt** at
inference, but when a fixed corpus is enriched via DDA (signals
multiplied per input) and alignment is re-expressed as **cosine
proximity to a frozen multi-modal centroid set** (TCT), the
subject-sample requirement collapses to roughly "**one reference set
per modality + frozen centroids**" — an empirical phenomenon we
demonstrate end-to-end in Case 3 (0.961 mean WavLM-SECS on 10 unseen
English sentences from a single author speaker) and project in Case 2
(7-modality Santa pipeline from 16 min of single-subject interview).

### Supporting meta-patterns

- **MP-03** (reference-set-as-anchor / single-shot ICL) — primary.
- **MP-11** (role-flip) — primary; frozen embedders produce the
  centroids without any training.
- **MP-06** (forensic-failure-hidden) — secondary; Case 1 echo-bug
  and Case 2 sign-convention disclosures supply the engineering-
  narrative register the cluster systematically omits.
- **MP-04** (closed-artefact industry) — hedge-bearer; our
  open-scorer / releasable-audio discipline is a structural
  differentiator, not a competitive parity claim.

### Anchor citations

- **Cluster E (single-reference ICL precedent):** `wang2023valle`,
  `chen2024valle2`, `tan2024naturalspeech3`, `shen2023naturalspeech2`,
  `le2023voicebox`, `chen2024f5tts`, `casanova2024xtts`,
  `wang2024maskgct`, `jiang2023megatts2` — all condition on a single
  prompt (agent-09 E-METHOD-GAP-1).
- **Cluster F (single-reference ICL precedent):** `cui2024hallo3`,
  `zheng2025hunyuanvideo`, `tian2024emo`, `tian2025emo2`,
  `meng2025echomimicv3` — reference image + audio, never a centroid
  per modality (agent-09 F-METHOD-GAP-1).
- **Infrastructure:** `hu2022lora` (T1 ICLR, SINGLE-SOURCE
  foundational CL-22), `lipman2023flowmatching` (T1 ICLR,
  SINGLE-SOURCE foundational CL-23) — underpin Case 2 identity-lock.
- **Evaluation:** `chen2022wavlm` (T1) used as Case 3 SECS scorer
  (cross-encoder w.r.t. Qwen3-TTS conditioning); `reddy2022dnsmos`
  (T2) + `saeki2022utmos` (T2) for naturalness/MOS parity with real
  recordings.
- **Author-empirical anchor:** Case 3, CL-16 AUTHOR-EMPIRICAL — 0.961
  mean SECS (max 0.975) on 10 unseen sentences, DNSMOS 3.93 vs 3.93
  parity, UTMOS 3.012 vs 2.997.

### Manuscript section spined by this theme

- **Primary spine:** §3.6 sample-complexity framing; §6.2 Case 2
  (Santa, 7 modalities × centroids from 2,362 clips, T3-projected);
  §6.3 Case 3 (voice, 50-clip L2-normalised centroid, T1-measured).
- **Secondary placements:** §5.4 identity-locked property derivation;
  §7 Discussion (sample-complexity implications for long-tail subject
  data); §8 Limitations (single-subject/single-encoder/English-only
  bound per agent-14 Biases 05/06/07).

### Hedge and scope-bound

- Case 3 is measured; Case 2 and Case 1 are projected at time of
  writing (agent-11 CL-13, CL-16; agent-01 V22 "projected;
  camera-ready will re-verify" phrasing).
- ClipCannon DAG output (12,000+ labelled samples from 16 min source)
  is an AUTHOR-EMPIRICAL system-description artefact (CL-27), not
  cross-lab replicated (OC-5 opportunity).
- Open-scorer / disclosed-forensic discipline differentiates the
  paper's own reproducibility posture but does **not** correct the
  closed-artefact posture of frontier comparators (agent-15
  Observation 7; MP-04 hedge).
- Sample-complexity collapse is an AUTHOR-EMPIRICAL phenomenon claim
  across Cases 2 and 3 (agent-11 CL-13, CL-16), not a proof.
- Case 1 (Shakespeare) is text-style only and not an identity-lock
  study; it contributes the forensic-bug disclosure (echo-bug,
  Case 1) rather than a sample-complexity measurement.

### Novelty delta

- EXT of clusters E and F (centroid aggregation + TCT-on-frozen-
  centroids) + AUTHOR-EMPIRICAL phenomenon claim per agent-10 /
  agent-11.

---

## Theme-to-section cross-walk

| Theme | Primary sections | Secondary sections | Primary meta-patterns | Agent-10 novelty code |
|---|---|---|---|---|
| **A** Data-wall reframe (third path) | §1, §3.3 | §2.1, §8 | MP-10, MP-07, MP-15 | EXT-A + ORTH-B |
| **B** Role-flip (retriever → labeller → guard) | §3, §4.1, §5 | §3.4, §7 | MP-11, MP-01, MP-09 | EXT-D + BRK-DEFI |
| **C** Scalar-reward is insufficient | §2.6, §5.1, §5.2 | §7, §8 | MP-02, MP-08, MP-12 | BRK-I + EXT-D |
| **D** Single-encoder evaluation gap | §2.5, §5.3, §6.3 | §6.2 | MP-01, MP-05, MP-04 | BRK-DEF |
| **E** Runtime verification as paradigm import | §5.3, §5.4, §6.3 | §1, §7 | MP-09, MP-08, MP-01 | BRK-DEFI |
| **F** Sample-complexity collapse | §3.6, §6.2, §6.3 | §5.4, §7, §8 | MP-03, MP-11, MP-06 | EXT-EF + AE |

**Coverage of agent-16's sixteen patterns across the six themes:**

| MP | Theme coverage |
|---|---|
| MP-01 single-encoder | B (secondary), D (primary), E (secondary) |
| MP-02 scalar-reward | C (primary) |
| MP-03 single-shot ICL | F (primary) |
| MP-04 closed-artefact | D (secondary), F (hedge) |
| MP-05 benchmark monoculture | D (secondary) |
| MP-06 forensic-hidden | F (secondary) |
| MP-07 compression-reframing | A (secondary) |
| MP-08 geometric target | C (primary), E (primary) |
| MP-09 per-output runtime | B (secondary), E (primary) |
| MP-10 DDA third path | A (primary) |
| MP-11 role-flip | B (primary), F (primary) |
| MP-12 composition-not-invention | C (framing), A (framing), B (framing) |
| MP-13 foundational-SS infra | F (citation anchor only) |
| MP-14 distributed SS risk | — (belongs to §8 Limitations; routed to agent-40) |
| MP-15 temporal-narrative | A (hedge) |
| MP-16 pool-relative scope | A (hedge), B (hedge), D (hedge) — applies to every absence claim |

**Notes on coverage.**
- MP-12 (composition-not-invention) is a **framing move** that runs
  through Themes A / B / C — it is the §1 + §7 intellectual-honesty
  register, not a standalone theme.
- MP-14 (distributed SINGLE-SOURCE risk) and MP-16 (pool-relative
  scope) are **hedge patterns** carried verbatim to §8 Limitations —
  agent-40 is the owner; they are not spines.
- MP-13 (foundational SINGLE-SOURCE infrastructure) is a **citation-
  economics** note that supports Theme F's infrastructure claims but
  does not spine a section on its own.

---

## Decision notes (agent-17 validations of the task-brief candidates)

| Task-brief candidate | Agent-17 decision | Rationale |
|---|---|---|
| A: data-wall reframe | **Retained as Theme A** | Direct MP-10 + MP-07 mapping. Anchors §1 + §3.3. |
| B: role-flip of frozen embedders | **Retained as Theme B** | MP-11 is one of agent-16's three central patterns. Anchors §3 + §4.1 + §5. |
| C: scalar-reward insufficient | **Retained as Theme C** | MP-02 + MP-08. Anchors §2.6 + §5.1. Kept distinct from D because they spine different sections. |
| D: single-encoder gap | **Retained as Theme D** | MP-01 + MP-05. Anchors §2.5 + §5.3 + §6.3 (Case 3). Not collapsed into C because §2.5 and §2.6 are distinct related-work sub-sections. |
| E: runtime verification | **Retained as Theme E** | MP-09 is agent-16's "most theoretically novel" pattern. Anchors §5.3 + §5.4 + §6.3. |
| F: sample-complexity collapse | **Retained as Theme F, reframed as empirical-demonstration theme** | MP-03 + MP-11 + MP-06. Anchors §3.6 + §6.2 + §6.3. Reframing: it is the only theme with a fully-measured author-empirical anchor. |

No candidate themes were dropped or merged. Agent-17 kept six themes to
preserve section-level specificity. A flatter 4-theme set would collapse
related-work sub-sections (§2.5 and §2.6) onto the same spine, losing
the scalar-vs-geometric and single-vs-multi-encoder distinctions.

---

## Resolved tensions (carried forward from agent-15 / agent-16)

### Tension 1 — Closed-artefact frontier vs. open-scorer posture

Resolved via **MP-04 (closed-artefact industry) + MP-06 (forensic-
failure-hidden) composition**. The industry pattern is closed-artefact
+ hidden-failure; our paper's pattern is open-scorer + disclosed-
forensic. This is a **structural differentiator** in Theme F, not a
defensive concession. §8 Limitations explicitly states the mitigation
applies to *our* numbers only, not to the cited frontier's numbers
(agent-16 handoff §"Closed-artefact tension resolves cleanly").

### Tension 2 — Huang r≈0.95 correlation vs. V11 embedder independence

Resolved via **MP-07 scope disambiguation**. The compression-capability
correlation (`huang2024compression`, SINGLE-SOURCE CL-08) is a
**within-model** claim (capability of a single trained LLM correlates
with its compression ratio). The embedder-independence hedge (V11,
agent-11 CL-11) is an **across-model** claim (N distinct architectures
trained on different corpora produce approximately independent
projections). These operate on orthogonal axes and are not
incompatible. Agent-17 surfaces this as a **clarification in §3.1 or
§3.4**, not as an unresolved tension (agent-16 handoff §"Huang r≈0.95
vs. V11 orthogonality tension resolvable via MP-07").

### Tension 3 — MP-09 promotion (§1 opener vs. §5 method)

Agent-16 flagged this as an open item. Agent-17 resolves: **use MP-09
as a §1 framing-move footnote + a §5 technical-move body**. Theme E
supplies §5 spine; Theme A's §1 opener names the runtime-verification
import as one of the paper's three load-bearing moves without
committing the full theoretical development to §1. This preserves
narrative impact without front-loading technical depth.

---

## Confidence

**HIGH.**

- Every theme names a ≥ 1-sentence thesis, ≥ 2 supporting meta-
  patterns, ≥ 4 anchor citations, and ≥ 1 primary manuscript section.
- All 16 meta-patterns from agent-16 are accounted for across the six
  themes (including hedge patterns that belong in §8 Limitations).
- All seven thematic blocks from agent-15 are re-composed into the
  six themes without dropping any source claim.
- Every hedge traces to agent-11 GRADE, agent-13 flag, or agent-14
  bias mitigation verbatim.
- No new citations introduced; master pool LOCKED.
- Three tensions carried forward from agents 15 and 16 are resolved
  with explicit method notes.
- Agent-10 novelty codes (EXT / CAT-EXT / BRK / ORTH) preserved
  per-theme.

## Changelog

- 2026-04-17 — Initial issue by agent-17-thematic-synthesizer.
