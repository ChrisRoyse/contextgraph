---
namespace: patterns
agent_id: agent-16-pattern-analyst
timestamp: 2026-04-17T01:15:00Z
status: active
locked: false
supersedes: none
tags: meta-patterns, cross-cluster-regularities, novelty-map, literature-absences, structural-holes
depends_on:
  - memory/context/mission-brief.md
  - memory/patterns/agent-15-evidence-synthesizer--thematic-synthesis-blocks.md
  - memory/discoveries/agent-15-evidence-synthesizer--cross-cluster-observations.md
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
  - memory/handoff/agent-15-to-16--synthesis-ready.md
downstream_consumers:
  - agent-17-thematic-synthesizer
  - agent-35-abstract-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Meta-Patterns Across the 88-Citation Literature Pool

## Scope

A **meta-pattern** is a regularity that recurs across two or more clusters
(A–I) and is **not specific to any one cluster**. These are the structural
absences, methodological monocultures, and paradigmatic inheritances that
become legible only when the literature is read as a whole rather than
per-cluster. Each pattern below names: (a) what it is, (b) ≥ 2 pieces of
evidence drawn from canonical keys in the LOCKED master pool, (c) how our
manuscript exploits or extends the pattern, and (d) where in the manuscript
the pattern is discussed.

**Discipline carried forward from agent-15.**
- Master pool LOCKED per agent-08. No new citations.
- Every grade / hedge / caveat inherited verbatim from agent-11 / 13 / 14.
- Absence claims are framed relative to the 88-citation thesis-aligned pool,
  not the global literature (agent-14 Bias-02 citation-selection scope).

---

## MP-01 — The single-encoder evaluation pattern

**What it is.** Every cited TTS and identity-locked-video paper evaluates
identity similarity with **one** speaker / face encoder at a time. The
scoring encoder is usually from the same family as the conditioning encoder
(e.g., WavLM-conditioned generator scored on WavLM). No cited paper runs a
**multi-encoder conjunction** in which an output must satisfy ≥ 2
architecturally independent encoders simultaneously at inference.

**Evidence.**
- Cluster E (TTS): `chen2024valle2` (SIM-o via one speaker encoder),
  `tan2024naturalspeech3` (single WavLM / ECAPA score), `chen2024f5tts`
  (SIM-o + WER only), `casanova2024xtts` (SECS via one speaker encoder),
  `wang2024maskgct` (single-encoder similarity). Agent-09 E-METHOD-GAP-3
  names this absence.
- Cluster F (video): `meng2025echomimicv3`, `cui2024hallo3`,
  `zheng2025hunyuanvideo`, `tian2024emo`, `tian2025emo2`, `yang2024cogvideox`
  all report **CSIM on one face-ID encoder post-hoc**; no cluster-F paper
  computes visual-ID and speaker-ID and voice-pitch and expression-plausibility
  gates in conjunction. Agent-09 F-METHOD-GAP-4 names this.
- Agent-14 Bias-06 (single-encoder transportability) quantifies this as a
  cluster-spanning bias; agent-15 Observation 1 spans clusters D, E, F, I.

**How our paper exploits it.** TCT's **constellation guard** imports a
multi-encoder conjunction (`∀ m : s_m ≥ τ_m`) as a per-output acceptance
predicate. Because the cluster literature is single-encoder, this predicate
is **BRK** (framework-breaking) in four clusters simultaneously per
agent-10's framework map. The multi-modal guard is the sharpest theoretical
novelty the manuscript makes.

**Where in the manuscript.** §2.5 (TTS comparator), §2.6 (alignment contrast),
§5.3 (constellation guard definition), §6.2 (Case 2 projected guard), §6.3
(Case 3 cross-encoder SECS protocol). Agent-15 Theme 5 and Theme 6 carry the
evidence; Observation 1 names the role-flip.

---

## MP-02 — The scalar-reward alignment pattern

**What it is.** The three canonical alignment anchors (`ouyang2022instructgpt`,
`rafailov2023dpo`, `bai2022constitutional`) all reduce alignment quality to
**a single scalar reward**, optimised under a KL-to-reference-policy
regulariser. No cited alignment paper proposes a **geometric alignment
target** (cosine to a frozen centroid) or a **multi-modal conjunction**.
The literature treats alignment as a scalar-reward-maximisation problem,
full stop.

**Evidence.**
- `ouyang2022instructgpt` — reward model on preference pairs → PPO.
- `rafailov2023dpo` — closed-form reward via `β log(π / π_ref)`.
- `bai2022constitutional` — AI-feedback preferences, still scalar-reward
  structured (agent-13 Flag 3 closed-artefact).
- Agent-09 Cluster I I-METHOD-GAP-1 / GAP-2 / GAP-3 name the three absences;
  agent-10 tags TCT as **BRK** relative to all three scalar-reward anchors.
- Agent-15 Theme 7 treats the three absences as **definitional of the
  scalar-reward paradigm**, not incidental.

**How our paper exploits it.** §5 defines TCT as the **non-scalar-reward**
alternative: geometric-proximity loss + per-output cosine gate + multi-modal
conjunction. The three moves are each individually absent from cluster I;
the three together are the full structural complement.

**Where in the manuscript.** §2.6 (scalar-reward contrast table), §5.1 (TCT
theoretical framing), §5.4 (per-output predicate). Agent-15 Theme 7 carries
the paste-ready prose.

---

## MP-03 — The reference-set-as-anchor pattern (single-shot ICL)

**What it is.** Voice-cloning (cluster E) and identity-locked video
(cluster F) condition on **a single reference** — one prompt utterance
(TTS) or one reference image + one audio clip (video). No cited paper
aggregates **N reference clips into a centroid** and uses that centroid as
the alignment target. The in-context-learning paradigm has not been
re-expressed as a centroid-reference paradigm.

**Evidence.**
- Cluster E: `wang2023valle`, `chen2024valle2`, `tan2024naturalspeech3`,
  `shen2023naturalspeech2`, `le2023voicebox`, `chen2024f5tts`,
  `casanova2024xtts`, `wang2024maskgct`, `jiang2023megatts2` — all condition
  on a **single prompt**. Agent-09 E-METHOD-GAP-1 names centroid aggregation
  as absent.
- Cluster F: `cui2024hallo3`, `zheng2025hunyuanvideo`, `tian2024emo`,
  `tian2025emo2`, `meng2025echomimicv3` — all condition on a **reference
  image + audio**, never a centroid per modality. Agent-09 F-METHOD-GAP-1
  confirms.
- Agent-10 cluster-E framework table: "no cited Cluster-E paper uses a
  multi-clip centroid as the alignment target".

**How our paper exploits it.** Case 3 builds a **50-clip L2-normalised
centroid** of WavLM-large speaker embeddings and uses it both as the Phase-2
geometric-proximity loss target and as the Phase-3 runtime-gate reference.
Case 2 (projected) constructs seven per-modality centroids over 2,362 clips
from the 16-minute interview. The EXT move (centroid over N references) is
where the sample-complexity collapse claim originates.

**Where in the manuscript.** §3.6 sample-complexity framing, §5.2
centroid construction, §6.2 (Case 2, 7 × centroid), §6.3 (Case 3, 50-clip
centroid). Agent-15 Theme 5 carries the TTS evidence; Observation 8 ties
centroid construction to the sample-complexity claim.

---

## MP-04 — The closed-artefact industry pattern

**What it is.** The highest-headline-SIM-o / FVD / win-rate numbers in
clusters E, F, and I come from papers with **closed weights, closed
training data, and closed evaluation code**. Four citations are flagged
RED / AMBER by agent-13. The cluster concentration is Microsoft × 4 (TTS),
Alibaba × 4 (video), US-only × 3 (alignment). No benchmark held constant
across open and closed releases.

**Evidence.**
- `chen2024valle2` RED (VALL-E 2, no weights, no code, no data).
- `meng2025echomimicv3` AMBER (EchoMimicV3, partial code, no training data).
- `bai2022constitutional` RED (Constitutional AI, closed RLAIF data, Elo-
  only evaluation).
- `tan2024naturalspeech3` AMBER (NaturalSpeech 3, no weights).
- Agent-14 Bias-03 quantifies the lab-concentration pattern.
- Agent-15 Observation 2 spans clusters E / F / I and surfaces this as a
  cross-cluster regularity, not a per-paper issue.

**How our paper exploits it.** §6.3 Case 3 reports on **fully open scorers**
(WavLM-large, DNSMOS, UTMOS) with **releasable audio** under a **specified
protocol**. This makes the paper's own empirical claim **structurally more
reproducible** than the frontier it is positioned against — a
novelty-adjacent transparency move (agent-15 Observation 7). §8 Limitations
then carries the honest-reporting paragraph that names the mitigation is
for *our* numbers only, not for the cited frontier's numbers.

**Where in the manuscript.** §2.5 + §2.6 (disclosed alongside citations),
§6.3 (Case 3 opener disclosing scorer choice), §8.1 (open-scorer-as-
differentiator limitations paragraph).

---

## MP-05 — The benchmark-monoculture pattern

**What it is.** Each cluster has a narrow benchmark vocabulary that it
reuses across papers, producing a **benchmark monoculture** — LibriSpeech /
LibriTTS / VoxCeleb in TTS; LRS / LRW / HDTF in video; MMLU / TruthfulQA /
HellaSwag in alignment; MS MARCO / BEIR in retrieval. Cross-language
diversity, cross-subject diversity, cross-corpus diversity are rare. The
"state of the art" is therefore a state-of-the-art **within the monoculture**.

**Evidence.**
- Cluster E: `wang2023valle`, `chen2024valle2`, `shen2023naturalspeech2`,
  `chen2024f5tts`, `le2023voicebox` all evaluate on LibriSpeech / LibriTTS
  test splits; `christop2025cloneval` (CloneEval) is the only open
  cross-corpus benchmark and is recent.
- Cluster F: `tian2024emo`, `tian2025emo2`, `cui2024hallo3`,
  `li2024latentsync` on HDTF / LRS / LRW derivatives; no single-subject
  evaluations standard.
- Cluster D: retrieval metrics on MS MARCO / BEIR / NQ across
  `khattab2020colbert`, `santhanam2022colbertv2`, `formal2021splade`.
- Cluster I: win-rate / Elo on small crowd-worker pools; no open
  preference-benchmark.
- Agent-14 Bias-05 (single-subject limitation) and Bias-04 (narrow
  evaluation suites) both map to this regularity.

**How our paper exploits it.** Case 3 evaluates on a **single author speaker,
10 held-out English sentences**, with cross-encoder SECS — deliberately
avoiding the LibriSpeech monoculture and explicitly labelled as
within-protocol. This differentiates us structurally *and* bounds the
transportability claim honestly. §8 Limitations carries the bound.

**Where in the manuscript.** §6.3 (Case 3 protocol design), §7 (Discussion
section on monoculture), §8 (Limitations; cross-speaker / cross-encoder /
cross-language as explicit future work; agent-14 §8 required paragraph).

---

## MP-06 — The forensic-failure-hidden pattern

**What it is.** Across clusters E, F, and I, **failed approaches and
sign-convention bugs are systematically omitted** from published
methodology sections. Papers in ML venues optimise for positive results;
ablations are "what worked when we turned off component X", not "we tried
Y and it silently broke for three weeks because of a sign flip". The
forensic-case-study register (FC, per agent-09 codes) is essentially
absent from the pool.

**Evidence.**
- Agent-09 methodology-tradition codes: across all 88 citations, **0 papers
  are classified FC** (forensic case study). The closest analogues are
  ablation tables (AB), which are controlled component removals rather
  than retrospective bug disclosures.
- Cluster F: `meng2025echomimicv3` does **not** document the
  `target = noise − latent` sign convention despite it being a
  load-bearing implementation detail (agent-10 F-FG-3; glossary #30).
  `lipman2023flowmatching` theory paper leaves the sign convention as an
  implementation choice.
- Cluster E: `chen2024valle2` and `tan2024naturalspeech3` closed-artefact
  status means failure-mode disclosures are not possible to verify
  externally (agent-13 Flags 1, 4).
- Agent-14 Bias-07 (forensic / engineering narrative vs. randomised
  ablation) flags this as a field-wide publication-incentive pattern.

**How our paper exploits it.** §6.1 (Case 1 Shakespeare **echo-bug
forensic**) and §6.2 (Case 2 Santa **flow-matching sign-convention
forensic** referencing `docs/inspect/08_training_audit.md`) disclose
two engineering bugs and their diagnostic paths. This is the FC register
agent-09 identified as absent, and agent-10 marks as an **under-used
epistemological register** (pragmatic-constructive). The disclosures are
positioned as intellectual-honesty moves (P01 / FM-08) rather than as
new theoretical content.

**Where in the manuscript.** §6.1 (echo-bug), §6.2 (sign-bug), §8.4
(Limitations: forensic disclosure is engineering narrative, not a
randomised ablation), §9 (Future work: randomised ablations are named as
the next step).

---

## MP-07 — The compression-reframing opportunity

**What it is.** Cluster C establishes a **prediction–compression duality**
anchored in Shannon source-coding (`deletang2024compression`,
`jiang2023gzip`, `li2025lossless`, `valmeekam2023llmzip`,
`huang2024compression`). All five papers read compression **from** a
trained model on held-out sequences. **None uses a compressor as a
labeller at training-data preparation time**; none develops a
cross-compressor interaction theory; none formalises signals-per-input as
a quantity distinct from bits-per-byte. The compression frame has not
been extended to training-data enrichment.

**Evidence.**
- `deletang2024compression` runs LLMs as compressors on held-out text /
  image / audio — the compressor is the object of analysis.
- `huang2024compression` reports `r ≈ 0.95` across 30+ LLMs — a held-out
  correlation claim; the training-data-preparation use is not explored
  (CL-08 SINGLE-SOURCE, agent-13).
- `jiang2023gzip` uses gzip as a classifier — still a held-out-sequence
  use; not a label generator.
- `li2025lossless` extends to video — modality breadth, same operator
  direction.
- Agent-09 C-METHOD-GAP-1, C-METHOD-GAP-2, C-METHOD-GAP-3 name the three
  absences. Agent-10 C-FG-1 and C-FG-2 frame them as theoretical gaps.

**How our paper exploits it.** §3.1 introduces **meaning compression** as
a **fourth compression category** orthogonal to bit / weight / activation
compression. DDA (§3.4) is the operational instantiation — N frozen
compressors acting as labellers over a fixed corpus, yielding the
`N + C(N,2)` count. The compression frame's operator is flipped: where the
cluster reads compression from a trained model, DDA writes compressed
labels onto training data.

**Where in the manuscript.** §3.1 (four-category taxonomy), §3.4 (counting
argument), §5 (TCT uses compressor-as-labeller substrate). Agent-15 Theme 3
is the paste-ready synthesis. Agent-10 tags meaning compression as **CAT-EXT**
of cluster C.

---

## MP-08 — The constellation-is-a-prediction-target opportunity

**What it is.** Alignment could, in principle, be **geometric rather than
scalar** — the trained policy could be asked to place each output near a
frozen geometric target in a fixed embedding space rather than to maximise
a learned scalar reward. **No cited paper proposes this explicitly.**
Geometric alignment targets appear nowhere in clusters E, F, or I, and
cluster D's geometric objects are evaluated as rankers, not as prediction
targets.

**Evidence.**
- Cluster I (`ouyang2022instructgpt`, `rafailov2023dpo`,
  `bai2022constitutional`) all use scalar rewards; agent-10 I-FG-1 names
  the absence of geometric-alignment theory.
- Cluster D (`zhai2023siglip`, `khattab2020colbert`, `chen2022wavlm`,
  `nussbaum2024nomic`, `formal2021splade`, `wang2021kepler`) — all
  retrieval / ranking evaluations; agent-15 Observation 1 confirms
  retrieval-eval is universal.
- Cluster E / F — centroid construction absent (MP-03); geometric-proximity
  loss absent.
- Agent-10 framework-compatibility table: TCT is **BRK** (framework-
  breaking) in cluster I; it does not extend scalar-reward theory, it
  proposes a different theoretical primitive.

**How our paper exploits it.** §5.2 TCT Phase-2 loss:
`L_TCT = L_generator + Σ_m λ_m · (1 − cos(φ_m(ŷ), c_m))`.
The alignment primitive is cosine distance to a frozen centroid rather
than a learned scalar reward. This is a **framework-breaking** move in
cluster I and simultaneously an **EXT** of cluster D's contrastive
geometry (positive-only, frozen-metric training).

**Where in the manuscript.** §2.6 (contrast table), §5 (TCT method), §5.2
(loss definition), §7 (Discussion of geometric-vs-scalar alignment).

---

## MP-09 — The per-output runtime verification gap

**What it is.** **Formal methods** (a field outside our pool) uses
**runtime verification** — a monitor that checks each output against a
specification and rejects violators. **ML evaluation** uses **batch
evaluation** — metrics computed over a held-out test set. The two
traditions have not been composed: no cited paper in clusters E, F, or I
applies a per-output runtime acceptance predicate during generation.
Whatever the generator produces enters the world; evaluation happens later.

**Evidence.**
- Cluster D: retrieval accepts top-k on soft ranking; no hard-threshold
  per-output gate. Agent-10 D-FG-1 ("no per-output acceptance semantics").
- Cluster E: SIM-o / SECS / DNSMOS / UTMOS all **post-hoc** on test sets.
  Agent-09 E-METHOD-GAP-2 ("no runtime cosine-gate").
- Cluster F: CSIM / FVD / LSE-D post-hoc. Agent-09 F-METHOD-GAP-2
  ("no runtime per-modality cosine-gate").
- Cluster I: generators produce distribution-shaped policies; no cited
  paper runs an inference-time gate. Agent-10 I-FG-2 ("no per-output
  runtime-acceptance predicate").

**How our paper exploits it.** §5.3 defines **the constellation guard**: at
inference time, the generator produces candidate `ŷ`, each modality's
embedder re-embeds `ŷ`, per-modality cosine is computed, and the output
is accepted only if `∀ m : s_m ≥ τ_m`. This is per-output runtime
verification imported into an ML pipeline — architecturally the runtime-
verification move, semantically the constellation-guard move. Case 3
instantiates the TTS version (WavLM-SECS best-of-12 at T=0.3, 0.95
threshold), yielding 0.961 mean SECS on 10 unseen sentences.

**Where in the manuscript.** §5.3 (guard definition), §6.3 (Case 3 guard
instantiation + measured SECS), §7 (Discussion: runtime verification meets
generative ML). Agent-10 tags this as BRK across D / E / F / I —
the **single most theoretically novel move in the manuscript**.

---

## MP-10 — The DDA-as-structural-third-path gap

**What it is.** The data-wall literature (cluster A) offers two paths:
path 1, **license / acquire more data** (cluster H evidence); path 2,
**synthesise more data** (bounded by cluster B collapse theorems). No
cited paper proposes a **third structural path**: decompose what you
already have through N independent frozen projections to multiply the
*supervisory signal* per unit of *input data*. The "input supply
multiplication via input decomposition" move is structurally absent.

**Evidence.**
- Cluster A: `villalobos2024data`, `longpre2024consent`,
  `hoffmann2022chinchilla`, `kaplan2020scaling` — all treat data as raw
  tokens; no signals-per-input axis (agent-09 A-METHOD-GAP-1, agent-10
  A-FG-2).
- Cluster B collapse-mitigation: `gerstgrasser2024colm` (accumulate),
  `feng2024verification` (verify) — both **stay inside the
  generator-in-loop precondition** (agent-15 Theme 2; Observation 4).
- Cluster H enterprise signals: `openai2024newscorp`,
  `reuters2024reddit`, `shutterstock2023openai` — path-1 licensing moves,
  not decomposition.
- Agent-10 framework-compatibility table: DDA is **ORTH** to collapse
  theory (no feedback loop), **EXT** of scaling-law data-axis (counts
  signals-per-input instead of raw-tokens). Agent-15 Theme 2 Bridge
  positions DDA as collapse-silent.

**How our paper exploits it.** §3.3 positions DDA as the **third path**
after path 1 (license) and path 2 (synthesise). The structural argument is
categorical, not empirical: the collapse theorems' preconditions are not
satisfied by frozen-embedder decomposition of real inputs. §3.4 gives the
counting derivation (`N + C(N,2)` orthogonal labelled signals per input;
~91 for N=13). Agent-15 Observation 4 is the explicit cross-cluster claim.

**Where in the manuscript.** §1 (three-paths framing), §2.1 (literature
positioning), §3.3 (categorical scope argument), §3.4 (counting argument).

---

## MP-11 — The role-flip meta-pattern (retriever → labeller → runtime guard)

**What it is.** Across our novelty-bearing moves, one architectural
inversion recurs: **existing frozen embedders are used in roles the cited
literature has not assigned them**. In cluster D, embedders are rankers;
in DDA, they are **labellers**; in TCT, they are **runtime guards**. The
same frozen artefacts carry three semantic roles across the manuscript
without retraining.

**Evidence.**
- Cluster D: `zhai2023siglip`, `khattab2020colbert`, `chen2022wavlm`,
  `nussbaum2024nomic`, `formal2021splade`, `wang2021kepler`,
  `bordes2013transe`, `malkov2018hnsw`, `cormack2009rrf` — all
  retrieval-eval (agent-15 Observation 1; agent-11 CL-12 STRONG).
- Agent-10 D-FG-1 and D-FG-2: no per-output acceptance semantics; no
  N-embedder independence theory. The theoretical gap lines up with the
  role-flip exactly.
- Our three moves in agent-15's observations: role-flip appears in
  Observation 1, 2, and Theme 4's Bridge.

**How our paper exploits it.** The three-role assignment (retriever →
labeller → guard) is the paper's single unifying architectural move. §4.1
(Context Graph: retriever role); §3 (DDA: labeller role); §5 (TCT
constellation guard: runtime-gate role). The same 13 embedders (Context
Graph) serve all three roles — role-flip without retraining is the
compositional simplicity claim.

**Where in the manuscript.** §3 (introduces the labeller role); §4.1
(retriever role, Context Graph); §5 (guard role, TCT). Agent-10 cluster-D
row: DDA **EXT** (role-flip), TCT **EXT** (positive-only frozen metric),
constellation guard **BRK** (generation accept/reject novel).

---

## MP-12 — The composition-not-invention meta-pattern

**What it is.** Our four load-bearing constructs (DDA, meaning compression,
TCT, constellation guard) are each **operational instantiations of existing
theoretical frames, not new theoretical objects**. Each construct occupies
the same kind of structural hole in the literature: a composition move
that prior art enabled but did not assemble. No new theoretical primitive
is introduced.

**Evidence.**
- Agent-10 framework map categorises each construct as **EXT**, **CAT-EXT**,
  or **BRK** — none as a new primitive. DDA **EXT** of scaling-law data-axis
  + compression frame; meaning compression **CAT-EXT** of compression-as-
  intelligence; TCT **EXT** of contrastive geometry + **BRK** of scalar-
  reward alignment; constellation guard **BRK** across D / E / F / I.
- Agent-11 GRADE classifies as MODERATE-TD / MODERATE-taxonomy / AUTHOR-
  EMPIRICAL-novel-method / STRONG-contrast (CL-10, CL-11, CL-13, CL-14).
- Agent-15 Observation 5 makes the composition-not-invention claim
  explicit across clusters C, D, and I.

**How our paper exploits it.** §1 and §3 frame the contributions as a
**coherent trio of composition-level claims**, not as three unrelated novel
primitives. Intellectual-honesty move (P01); each construct is less
extraordinary individually but more compelling in combination. Also
reduces adversarial-review surface area (agent-42) because each claim is
narrow.

**Where in the manuscript.** §1 (contributions positioned as composition);
§3 (DDA + meaning compression as composition of cluster A, C, D primitives);
§5 (TCT as composition of cluster D geometry + cluster I alternative);
§9 (honest-reporting of composition scope).

---

## MP-13 — The foundational-single-source infrastructure pattern

**What it is.** Three "foundational single-source" citations — LoRA
(`hu2022lora`), Flow Matching (`lipman2023flowmatching`), HNSW
(`malkov2018hnsw`) — carry **disproportionate structural weight** across
clusters D, E, F, and our systems §§. Each is SINGLE-SOURCE per agent-11
(CL-22, CL-23, CL-24) but is canonically cited across dozens of downstream
systems. The field's citation economics universally credit the originating
paper rather than any replication.

**Evidence.**
- LoRA underpins every identity-locked generator in our pool
  (`meng2025echomimicv3`, `cui2024hallo3`, Case 2) and Context Graph
  (§4.1 LoRA tuning of Hermes-2-Pro-Mistral-7B). Agent-11 CL-22 SINGLE-SOURCE
  foundational.
- Flow Matching underpins `chen2024f5tts`, `le2023voicebox`,
  `meng2025echomimicv3`, `wan2025video`, and Case 2. Agent-11 CL-23.
- HNSW underpins Context Graph retrieval, `khattab2020colbert` scaling,
  and almost every production vector-search system. Agent-11 CL-24.
- Agent-15 Observation 3 spans clusters D / E / F with these three
  citations.

**How our paper exploits it.** §4 (Systems) and §6 (Case Studies) trace
infrastructure claims to exactly three canonical papers, letting the
methodology section stay tight (agent-01 FM-06 pseudocode-only compliance).
§8 Limitations notes foundational-single-source status: citation-economics
artefact, not evidential deficiency (agent-11 carry-forward).

**Where in the manuscript.** §4 (Systems §§), §6 (Case Studies), §8
(honest-reporting paragraph on SINGLE-SOURCE infrastructure).

---

## MP-14 — The distributed SINGLE-SOURCE / AUTHOR-EMPIRICAL risk pattern

**What it is.** Every novelty-bearing cluster (C, D, E, F, I) has at
least one SINGLE-SOURCE or AUTHOR-EMPIRICAL load-bearing claim. The
manuscript's risk is **diversified across clusters, not concentrated** —
no single claim's refutation collapses the thesis, but the aggregate
surface area requires honest reporting.

**Evidence.**
- Cluster C: CL-08 `huang2024compression` r≈0.95 SINGLE-SOURCE.
- Cluster D: CL-24 `malkov2018hnsw` SINGLE-SOURCE.
- Cluster E: CL-16 Case 3 0.961 SECS AUTHOR-EMPIRICAL.
- Cluster F: CL-21 sign-convention forensic AUTHOR-EMPIRICAL;
  CL-22 `hu2022lora`, CL-23 `lipman2023flowmatching` SINGLE-SOURCE.
- Cluster I: CL-14 TCT-side contrast AUTHOR-EMPIRICAL.
- Agent-15 Observation 9 spans all five clusters.

**How our paper exploits it.** §8 Limitations explicitly states "load-
bearing SINGLE-SOURCE / AUTHOR-EMPIRICAL claims are distributed across
five clusters (C, D, E, F, I); no single cluster's failure collapses the
thesis". Diversification-as-honest-risk-reporting (agent-01 P01).

**Where in the manuscript.** §8 Limitations (paragraph on distributed
risk); §9 Conclusion (honest-reporting framing).

---

## MP-15 — The temporal-narrative-moment pattern

**What it is.** Five Q4-2024 industry signals (`woo2024information`,
`metz2024bloomberg`, `hu2024reuters`, `franzen2024techcrunch`,
`sutskever2024neurips`) mutually reference each other and describe the
same "data wall is near" narrative. Read as five independent confirmations,
they inflate the framing; read as **one dated narrative signal from
Q4 2024**, they inform but do not establish. This temporal-coincidence
pattern recurs in the arXiv-only video cluster (8/12 citations dated
2024Q3–2025Q1).

**Evidence.**
- Cluster A Tier-3 signals: five Q4-2024 industry reports mutually citing.
- Agent-14 Bias-10 quantifies the temporal concentration.
- Cluster F: 8 of 12 citations are arXiv-only in the Q4-2024–Q1-2025
  window; `meng2025echomimicv3`, `tian2025emo2`, `wan2025video` all 2025.
- Agent-15 Theme 1 inherits this hedge; Theme 6 applies it to cluster F.

**How our paper exploits it.** §2.1 frames industry signals as "one dated
narrative", not as N independent confirmations. The actual evidential
weight for the data wall sits on `villalobos2024data` (T1, MODERATE per
agent-11 CL-02) and `longpre2024consent` (T1 SINGLE-SOURCE). §8
Limitations carries the temporal-narrative hedge.

**Where in the manuscript.** §1 (data-wall framing), §2.1 (industry
signals hedge), §8 (temporal-narrative honest-reporting).

---

## MP-16 — The thesis-aligned-pool absence pattern (pool-relative claims)

**What it is.** Every novelty claim in the manuscript is framed relative
to the **88-citation thesis-aligned pool**, not the global literature.
This is a deliberate methodological move (agent-14 Bias-02 citation-
selection scope) that bounds claims to what is *cited* and does not
assert global literature absence.

**Evidence.**
- Agent-08 master pool LOCKED at 88 citations.
- Agent-11 GRADE applied to those 88 only.
- Agent-15 Observation 1 hedges: "this is a survey-level absence claim
  across a thesis-aligned pool, not an exhaustive systematic search".
- Agent-14 §8 required paragraph: "we do not claim no such system exists
  anywhere in the literature; we claim none appears in the LOCKED pool".

**How our paper exploits it.** Every "no cited paper" phrasing in §2, §5,
§8 is framed relative to the pool, not the literature globally. §8
Limitations explicitly states the scope. Reduces adversarial-review
exposure (agent-42) because a single counter-example in the broader
literature is acknowledged as possible and does not invalidate the
novelty-delta.

**Where in the manuscript.** §2 opener (scope note), §5 (TCT novelty
framing), §8 (honest-reporting paragraph).

---

## Meta-pattern summary table

| ID | Pattern | Evidence clusters | Agent-10 code | Where in MS |
|---|---|---|---|---|
| MP-01 | Single-encoder eval | D, E, F, I | BRK (D/E/F/I) | §2.5, §2.6, §5.3, §6.2, §6.3 |
| MP-02 | Scalar-reward alignment | I | BRK (I) | §2.6, §5, §5.4 |
| MP-03 | Reference-as-anchor (single-shot ICL) | E, F | EXT (E/F) | §3.6, §5.2, §6.2, §6.3 |
| MP-04 | Closed-artefact industry | E, F, I | — (bias) | §2.5, §6.3, §8.1 |
| MP-05 | Benchmark monoculture | D, E, F, I | — (bias) | §6.3, §7, §8 |
| MP-06 | Forensic-failure-hidden | E, F, I | — (register gap) | §6.1, §6.2, §8.4, §9 |
| MP-07 | Compression-reframing | C | CAT-EXT (C) | §3.1, §3.4, §5 |
| MP-08 | Constellation-as-prediction-target | D, E, F, I | BRK (I), EXT (D/E/F) | §2.6, §5, §5.2, §7 |
| MP-09 | Per-output runtime verification | D, E, F, I | BRK (D/E/F/I) | §5.3, §6.3, §7 |
| MP-10 | DDA structural third path | A, B, H | EXT-A / ORTH-B | §1, §2.1, §3.3, §3.4 |
| MP-11 | Role-flip (retriever→labeller→guard) | D | EXT + BRK (D) | §3, §4.1, §5 |
| MP-12 | Composition-not-invention | C, D, I | EXT / CAT-EXT / BRK | §1, §3, §5, §9 |
| MP-13 | Foundational-single-source infra | D, E, F | — (citation-economics) | §4, §6, §8 |
| MP-14 | Distributed SINGLE-SOURCE risk | C, D, E, F, I | — (hedge) | §8, §9 |
| MP-15 | Temporal-narrative-moment | A, F | — (hedge) | §1, §2.1, §8 |
| MP-16 | Thesis-aligned-pool absence | A–I | — (scope) | §2, §5, §8 |

---

## Confidence

**HIGH.** Each pattern names ≥ 2 canonical citation keys (or ≥ 1 cluster
when the pattern is about the cluster's structure); each cites a specific
agent-09 methodology-gap, agent-10 framework-gap, agent-14 bias, or
agent-15 observation; each maps to a specific §§ site; each respects the
LOCKED master pool.

## Changelog

- 2026-04-17 — Initial issue by agent-16-pattern-analyst.
