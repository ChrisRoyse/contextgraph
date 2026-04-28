---
namespace: discoveries
agent_id: agent-19-opportunity-identifier
timestamp: 2026-04-17T03:35:00Z
status: active
locked: false
supersedes: none
tags: novel-research-questions, future-work, section-9, conclusion, 3-questions
depends_on:
  - memory/patterns/agent-19-opportunity-identifier--research-opportunities.md
  - memory/discoveries/agent-18-gap-hunter--consolidated-gaps.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
downstream_consumers:
  - agent-20-interdisciplinary-pollinator
  - agent-39-discussion-writer
  - agent-41-future-work-writer
---

# Three Novel Research Questions Opened by This Paper

## Scope and method

These are the **three questions this paper opens for the community** —
questions that did not exist (or were not posable) in the LOCKED pool
before the paper's framework is accepted. Each question:

1. Becomes well-formed *only* after the paper's primitives (DDA,
   meaning compression, TCT, constellation guard) are introduced.
2. Anchors to specific ROs from
   `agent-19-opportunity-identifier--research-opportunities.md` and
   GAPs from `agent-18-gap-hunter--consolidated-gaps.md`.
3. Belongs in **§9 Conclusion / future-work outlook** as a concise
   programme call, with the detailed mechanics routed to §10
   (agent-41).
4. Carries a **venue hint** (main conference / workshop / ethics track)
   to help the community decide where the follow-up lives.

Register rules R1 (composition-not-invention), R3 (pool-relative
scope), and R4 (projected-vs-measured) carry forward.

**Selection criterion.** Three is the budget (per task spec). From
ten ROs, the three with highest **(novelty × tractability × community
reach)** product are:

- RO-8 (embedder-independence benchmark) → Q1.
- RO-4 (per-output guard) extended with RO-10 (sample-complexity) →
  Q2.
- RO-9 (safety / negative constellation) → Q3.

Each question names a **central RO** as anchor, plus a **sidebar RO**
or gap as extension surface.

---

## Q1. What is the measured mutual-information yield of N frozen independent embedders on a fixed corpus, and how does it compare to the $\binom{N}{2}$ combinatorial upper bound?

### Why this question did not exist before

The LOCKED pool's closest questions were:
- Cluster C compression-capability correlations on **single trained
  LLMs** (`huang2024compression` — within-model claim).
- Cluster D retrieval-fusion lifts across encoder panels
  (`cormack2009rrf` — scalar-metric lift, not MI).

Neither asks: "How many **informationally independent** bits of
supervision does a fixed corpus yield when projected through N frozen
encoders?" That question is well-formed only after DDA (RO-1) defines
$n(N + \binom{N}{2})$ as a counting primitive and meaning compression
(RO-2) defines it as a measurable ratio.

### What answering Q1 would establish

- An **empirical** meaning-compression ratio
  $\mathrm{MC}_{\mathrm{MI}}(\Phi, D)$ on a shared corpus.
- A ranking of **canonical embedder panels** (text-only,
  audio-text-video, code-entity-graph) by effective-N yield.
- A **benchmark** — "Meaning-Compression Benchmark v1" — analogous to
  GLUE / SuperGLUE / BIG-bench for *supervision density*.

### Anchors

- **ROs:** RO-1 (DDA), RO-2 (meaning compression), RO-8 (MI
  benchmark) — **primary anchor RO-8**.
- **GAPs:** GAP-06, GAP-09, GAP-13, GAP-17.
- **OC:** OC-2.
- **Agent-10 code:** outside-paper **EXT** of cluster-C compression
  theory; establishes a new empirical niche.

### §9 draft (one-paragraph call, for agent-39)

*Teleological Constellation Training rests on a counting primitive
($n(N + \binom{N}{2})$) whose second term is a combinatorial upper
bound on pairwise cross-embedder interactions, not a measured
information yield. We invite the community to establish a
**meaning-compression benchmark**: for canonical fixed corpora and
canonical embedder panels, measure the pairwise mutual information
between frozen encoder projections using MINE, InfoNCE, or
Donsker-Varadhan estimators, and report the effective-N yield. This
would convert DDA from a counting primitive into a measured operator
and open the data-wall reframing move to empirical cross-system
comparison.*

### Venue hint

Main conference (ICLR / NeurIPS datasets-and-benchmarks track) with a
tutorial / workshop companion on MI-estimation at scale.

### Hedge

R3 (pool-relative scope): absence of MI measurements is relative to
the 88-citation LOCKED pool; pool-external MI studies exist but have
not been composed with the N-frozen-encoder supervision-decomposition
frame.

---

## Q2. Under a runtime constellation guard, how does the per-output accept rate scale with the reference-set size, the number of modalities, and the generator's native diversity?

### Why this question did not exist before

Before RO-4 (per-output guard) and RO-6 (multi-clip centroid),
cluster E's quality measure was a held-out-set batch metric (SIM-o,
mean SECS on VoxCeleb / LibriSpeech test pools); cluster F's was CSIM
/ FVD; cluster I's was win-rate vs. a reference policy. No paradigm
composed **per-output acceptance** with **multi-clip centroids** with
**multi-encoder conjunction**, so the question "how does accept-rate
scale?" was not a question one asked — the acceptance predicate was
the new object.

### What answering Q2 would establish

- A **sample-complexity curve** for identity-locked generation:
  $n_{\text{clips}}$ vs. accept-rate under the guard at fixed
  thresholds — tractable one-speaker study (RO-10).
- A **modality-scaling law** for the guard: as $M$ (modalities) grows,
  accept-rate falls as the product of per-modality rejection rates;
  quantify and characterise the budget (GAP-16).
- A **generator-diversity × guard interaction**: do high-temperature /
  long-tail generators need larger K (best-of-K budget) to produce
  one accepted sample? Case 3 uses K=12 at T=0.3; the curve across
  (T, K) is open.

### Anchors

- **ROs:** RO-4 (guard) + RO-6 (centroid) + RO-10
  (sample-complexity curve) — **primary anchor RO-4**, secondary RO-10.
- **GAPs:** GAP-16, GAP-20, GAP-22, GAP-24, GAP-29.
- **OCs:** OC-4.
- **Agent-10 code:** **BRK** across clusters D, E, F, I; scaling
  characterisation is **EXT** of RO-4.

### §9 draft (one-paragraph call, for agent-39)

*The constellation guard in §5.3 is evaluated at a single operating
point in §6.3 (50-clip centroid, K=12, T=0.3, $\tau=0.95$, 0.961
mean WavLM SECS). We invite the community to characterise the guard's
scaling in three directions: (i) **reference-set size**
($n_{\text{clips}}$ vs. accept-rate; tractable one-speaker study);
(ii) **modalities** ($M$-fold conjunctive-predicate behaviour as the
guard composes across audio, video, text-style, prosody, gesture,
identity-embedding, and context); and (iii) **generator diversity** (K
and T regimes under which best-of-K saturates vs. is wasteful).
Answers to (i)–(iii) would turn the guard from an existence-proof move
into a deployable quality-assurance layer with an engineering budget.*

### Venue hint

Main conference (ICML / ICLR) + ML-systems workshop (MLSys) for the
budget / deployment dimension.

### Hedge

R4 (projected-vs-measured): Case 3 is the only measured guard
instantiation; Case 2 is projected. Q2's answers are likely to
generalise, but the measured anchor is single-subject /
single-encoder-pair / English-only (agent-14 Biases 05/06/07).

---

## Q3. Does the constellation guard generalise from identity targets to safety targets, and if so, what is the false-positive / false-negative profile of a negative-constellation rejection predicate?

### Why this question did not exist before

TCT is defined in this paper for **identity-locked generation** (voice
identity in Case 3; persona identity in Case 2). The constellation is
always a **positive** target (accept if close). Cluster I's alignment
literature (InstructGPT, DPO, Constitutional AI) defines safety as a
**scalar reward shaping** of a reference policy — no geometric target,
no per-output runtime predicate, no negative-constellation semantics.
The question "could safety be specified as a constellation and
enforced at runtime?" is well-formed only after RO-3 (constellation
alignment target) and RO-4 (runtime guard) are accepted.

### What answering Q3 would establish

- A **safety constellation** $C_P$ from verified refusal / safe-output
  examples, with a **guard** that accepts outputs close to the safety
  centroid.
- A **negative constellation** $C_P^{-}$ from violating examples, with
  a **rejection predicate** that flips the guard's direction.
- A **false-positive / false-negative characterisation** on a safety
  benchmark (e.g., HarmBench, SALAD-Bench, BeaverTails).
- An **adversarial-probing** result: can adversarial inputs pass the
  constellation guard while still violating the intended safety
  property? (This is the Constellation Defender question from the
  mission brief Q19.)

### Anchors

- **ROs:** RO-9 (safety / negative constellation) + RO-4 (guard) +
  RO-3 (constellation alignment target) — **primary anchor RO-9**.
- **GAPs:** GAP-14 (TCT generalisation to non-identity targets),
  GAP-19 (safety-profile constellation existence).
- **Agent-10 code:** **EXT** of RO-3 into safety; not a new
  theoretical primitive.

### §9 draft (one-paragraph call, for agent-39)

*Teleological Constellation Training is defined in this paper for
identity-locked targets. Whether the same primitive — **frozen
multi-modal centroids + per-output geometric guard** — generalises to
**safety targets** (refusal constellations, factuality
constellations), and whether a **negative constellation** $C_P^{-}$
can serve as a **rejection predicate**, are open research questions.
Answering them would supply alignment with a per-output
acceptance semantics it currently lacks (clusters D, E, F, I all
evaluate post-hoc on held-out sets), while opening well-documented
dual-use, false-refusal, and governance concerns. We flag these as
explicit future work for a **paired ethics + technical study**
(FAccT / AIES / NeurIPS Ethics track rather than main conference),
with explicit dual-use review and community input on whose safety
centroids are used.*

### Venue hint

Ethics-workshop paper (FAccT / AIES / NeurIPS Ethics track). Main
conference only if paired with a clear dual-use / governance analysis;
standalone technical publication without the governance frame is
discouraged per agent-14 Bias-11 (dual-use / deployment silence).

### Hedge

R2 (open-scorer / disclosed-forensic): Q3's programme must adopt the
same open-scorer posture we use for identity targets, but scale it to
safety-evaluation encoders that are themselves contested. **R1
(composition-not-invention)** discipline is load-bearing here: Q3 is
**not** a claim that TCT replaces existing alignment; it is a question
about compositional extension.

---

## Summary table

| Q | Short form | Primary RO | Primary GAPs | Venue | Paper-scope |
|---|---|---|---|---|---|
| **Q1** | Measured MI across N frozen encoders | RO-8 | GAP-09, GAP-13, GAP-17 | Main conf + MI workshop | Community (RO-8) |
| **Q2** | Guard accept-rate scaling (refs / modalities / diversity) | RO-4 (+ RO-10) | GAP-16, GAP-20, GAP-22, GAP-24 | Main conf + MLSys | Community (RO-10 one-speaker tractable now) |
| **Q3** | TCT for safety / negative constellations | RO-9 | GAP-14, GAP-19 | Ethics workshop (paired) | Community (ethics-paired) |

## How §9 uses these questions

Agent-39 (discussion-writer) should use all three questions as the
**forward-looking paragraph** of §9 Conclusion. Recommended ordering:

1. **Q1** first — most tractable; establishes the
   measurement-benchmark call before behavioural-science questions.
2. **Q2** second — most directly tied to RO-4 (the manuscript's
   single most novel technical move).
3. **Q3** third — most ethics-sensitive; place last so the paragraph
   ends with the responsible-deployment note.

Each question appears as **one sentence** in §9, with the full
mechanical detail routed to §10 (agent-41).

## Confidence

**HIGH.**

- All three questions are pool-novel: they are well-formed only after
  this paper's primitives are accepted (agent-17 register R1).
- Each question anchors to at least one central RO.
- Each question has a venue hint consistent with agent-14 bias
  mitigations (Bias-11 dual-use for Q3 especially).
- Each question has a §9 draft ready for agent-39 to adapt.

## Changelog

- 2026-04-17 — Initial issue by agent-19-opportunity-identifier.
