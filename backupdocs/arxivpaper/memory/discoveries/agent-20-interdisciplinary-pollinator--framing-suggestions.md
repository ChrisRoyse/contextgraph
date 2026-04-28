---
namespace: discoveries
agent_id: agent-20-interdisciplinary-pollinator
timestamp: 2026-04-17T04:20:00Z
status: active
locked: false
supersedes: none
tags: framing-suggestions, section-1-introduction, section-7-discussion, paste-ready-moves, interdisciplinary-grounding
depends_on:
  - memory/patterns/agent-20-interdisciplinary-pollinator--isomorphisms.md
  - memory/patterns/agent-17-thematic-synthesizer--thematic-frameworks.md
  - memory/patterns/agent-19-opportunity-identifier--research-opportunities.md
  - memory/discoveries/agent-19-opportunity-identifier--novel-research-questions.md
downstream_consumers:
  - agent-21-theory-builder
  - agent-35-abstract-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
---

# Framing Suggestions — §1 Introduction and §7 Discussion Moves

## Scope and method

Agent-20 selected 5 body-named isomorphisms (ISO-01, ISO-02, ISO-04,
ISO-06, ISO-07) and 3 metaphor-only isomorphisms (ISO-03, ISO-05,
ISO-08) in the sibling file
`patterns/agent-20-interdisciplinary-pollinator--isomorphisms.md`.

This file issues **paste-adaptable framing moves** for:

- **§1 Introduction** — the three-paragraph opener where the paper's
  paradigm borrows land most effectively (Theme A data-wall reframe,
  Theme E RV import, Theme C scalar-vs-geometric, Theme D
  single-encoder gap).
- **§7 Discussion** — the synthesis section that connects the
  paper's three contributions (DDA / TCT / Cases) by naming the
  interdisciplinary parallels that make them legible.

The framing moves are **prose seeds**, not final prose. Section
writers (agents 35, 36, 37, 39) adapt them to their section-specific
voice and length budget.

**Register discipline carried forward.** R1 composition-not-invention;
R3 pool-relative scope (no new citations); R4 projected-vs-measured
(Case 3 is the only measured anchor).

---

## §1 Introduction framing moves

### Move §1-FM-01 — The three paradigm borrows in the opener

**Purpose.** Name the three interdisciplinary borrows that organise
the paper's contribution in the first 200 words, so the reader
enters §2 Related Work already knowing the paradigm shape.

**ISO anchors.** ISO-01 (runtime verification); ISO-02 (sensor
fusion / N independent measurements); ISO-04 (Kolmogorov / MDL).

**Theme anchors.** Theme A (data-wall reframe); Theme B (role-flip);
Theme E (runtime verification).

**Paste-ready seed (≈ 90 words):**

> We present three structural moves organised around borrows from
> adjacent disciplines. First, we treat a fixed corpus projected
> through N frozen embedders as **N independent measurements of the
> same input**, in the sense of signal-processing sensor fusion —
> yielding a counting formula for supervisory signals per input that
> we call **Derived Data Abundance (DDA)**. Second, we frame the
> resulting supervisory-signal density as **meaning compression**, a
> fourth compression category in the information-theoretic tradition
> of Kolmogorov and minimum description length. Third, we import
> **runtime verification** from formal methods into generative ML:
> each candidate output is monitored against a frozen multi-modal
> specification (the **constellation**) and rejected if it fails.

**Why this works.**

- Front-loads three paradigm borrows that the reader can use as
  mental scaffolding for the technical development in §§3, 5.
- Matches Theme A's three-paths framing (acquire / synthesise /
  decompose) by positioning DDA as the third path via the
  sensor-fusion borrow.
- Preempts the Theme E MP-09 promotion recommendation (agent-16 /
  agent-17) by naming the runtime-verification import as a primary
  move in §1.

**Register check.** No citation. Naming the tradition-families
(signal processing; Kolmogorov / MDL; formal methods) is R3-compliant.

---

### Move §1-FM-02 — The astronomical-constellation terminological commitment

**Purpose.** Justify the paper's central terminological choice — why
"constellation" rather than "prototype", "anchor", "target" — in a
single sentence, preempting reviewer objections about novel
vocabulary.

**ISO anchors.** ISO-06 (astronomical constellation).

**Theme anchors.** Theme C (scalar-reward vs. geometric alignment);
Theme D (single-encoder vs. multi-modal predicate).

**Paste-ready seed (≈ 50 words):**

> We choose the name **constellation** deliberately: the object is a
> collection of **fixed reference points** used to orient generation,
> much as celestial constellations orient navigation. A single Pole
> Star is a scalar target; a full constellation — Orion's belt,
> Cassiopeia's "W" — requires multi-reference conjunction, exactly
> the move we want the paper's central term to carry.

**Why this works.**

- Preempts the "is this just a prototype?" reviewer critique before
  §5 arrives.
- Frames the scalar-vs-geometric contrast (Theme C) with a
  reader-friendly metaphor: Pole Star vs. constellation.
- Is **economical**: one sentence's worth of astronomy borrow
  commits the terminology.

**Register check.** No specific cultural astronomy (Polynesian,
European, Chinese, Navajo) invoked; the borrow is at the functional
level.

---

### Move §1-FM-03 — The three-paths data-wall framing

**Purpose.** Replace the conventional data-wall binary (license more
vs. synthesise more) with an explicit three-paths framing that
names DDA as the third path.

**ISO anchors.** ISO-02 (sensor fusion as the intuition source);
ISO-04 (MDL as the information-theoretic source).

**Theme anchors.** Theme A (data-wall reframe) — primary spine.

**Paste-ready seed (≈ 80 words):**

> The data-wall literature enumerates two paths: acquire more raw
> data, or synthesise more. Both paths have load-bearing limits.
> Accessible high-quality text is drying up on a 2026–2032 horizon
> (Villalobos et al., 2024); generator-in-loop synthesis degrades
> under well-characterised collapse preconditions (Shumailov et al.,
> 2024; Dohmatob et al., 2024). We propose a third path: rather than
> add corpora, **decompose the corpus we already have through N
> frozen independent embedders** and count supervisory signals per
> input. This is the sensor-fusion move applied to training-data
> economics.

**Why this works.**

- Matches Theme A's Tier-1 / Tier-2 citation structure
  (Villalobos; Shumailov; Dohmatob — all in LOCKED pool).
- Opens with the two conventional paths (so the reader knows we are
  not ignorant of them) before introducing the third.
- Ends with the sensor-fusion borrow as the enabling intuition.

**Register check.** Tier-1 citations are from the LOCKED pool; R3
compliant. No claim that DDA is a new theoretical primitive
(R1 composition-not-invention).

---

## §7 Discussion framing moves

### Move §7-FM-01 — The three-field synthesis sentence

**Purpose.** In the Discussion synthesis, name the three body-named
ISOs as a single **composition move** — the paper's coherence comes
from composing borrows from three distinct fields.

**ISO anchors.** ISO-01 (runtime verification); ISO-02 (sensor
fusion); ISO-04 (Kolmogorov / MDL).

**Theme anchors.** Theme B (role-flip composition); Theme E
(runtime verification).

**Paste-ready seed (≈ 70 words):**

> The paper's three contributions — DDA, meaning compression, TCT —
> form a single composition: **sensor-fusion** over frozen embedders
> multiplies supervisory signals; **minimum-description-length**
> reasoning reframes the multiplication as meaning compression;
> **runtime verification** reframes the resulting multi-encoder
> geometry as a per-output acceptance predicate. Each borrow is
> unremarkable on its own; their composition over the same frozen
> encoder instances is what produces the paper's claim of structural
> novelty.

**Why this works.**

- Makes the paper's coherence legible. Each theme (A, B, C, D, E, F)
  is a projection of this composition; the composition itself is the
  thesis.
- Respects MP-12 composition-not-invention: the three borrows are
  existing primitives; the composition is the contribution.
- Primes the reader for the §8 Limitations reading: if any of the
  three borrows is weak, the composition weakens accordingly.

**Register check.** Names the three traditions without citing; R3
compliant. MP-12 framing carries R1.

---

### Move §7-FM-02 — The SPC / quality-assurance reframe of TCT

**Purpose.** In §7 Discussion, reframe TCT as a **quality-assurance
layer** for generative pipelines (using the SPC borrow), making the
deployment implications concrete for engineering readers.

**ISO anchors.** ISO-07 (statistical process control); ISO-01
(runtime verification).

**Theme anchors.** Theme E (runtime verification); Theme F
(sample-complexity collapse).

**Paste-ready seed (≈ 90 words):**

> Read as a deployment primitive, TCT is a **per-output
> quality-assurance layer** for generative pipelines, structurally
> analogous to statistical process control in manufacturing: each
> unit of output is inspected against per-dimension tolerance bounds
> before release. The canonical generative-ML convention — batch
> post-hoc metrics on held-out sets (SIM-o; CSIM; FVD; win-rate) —
> is analogous to end-of-shift quality averaging, which SPC
> abandoned in the 1930s. We do not claim this as novel reasoning;
> we claim that this reasoning has not yet been imported into
> generative ML.

**Why this works.**

- Makes the per-output-predicate move concrete for engineering
  readers.
- Sharpens the batch-metric critique (Theme D / MP-01 single-encoder
  evaluation) with a historical analogy: SPC already solved
  end-of-shift averaging.
- Explicit self-demotion: "we do not claim novel reasoning" — R1
  composition-not-invention.

**Register check.** No specific SPC citation; tradition-family
invocation only. R3 compliant.

---

### Move §7-FM-03 — The scalar-vs-geometric alignment reframe

**Purpose.** In §7 Discussion, use the Pole Star vs. constellation
metaphor (from §1-FM-02) to make Theme C's scalar-vs-geometric
alignment contrast concrete and visual.

**ISO anchors.** ISO-06 (astronomical constellation); ISO-02 (sensor
fusion for the multi-modality structure).

**Theme anchors.** Theme C (scalar-reward insufficient); Theme D
(single-encoder gap).

**Paste-ready seed (≈ 80 words):**

> Scalar-reward alignment — InstructGPT, DPO, Constitutional AI —
> optimises a single scalar, which is structurally a **Pole Star**:
> useful for orientation, but a single reference. Geometric
> multi-modal alignment optimises proximity to a **constellation**:
> a conjunctive predicate over multiple frozen reference points.
> The two paradigms are not alternatives on the same axis; they
> optimise different objects. Our claim is not that TCT replaces
> RLHF / DPO, but that the geometric constellation target adds an
> inference-time acceptance semantics that scalar reward
> definitionally cannot.

**Why this works.**

- Uses the already-named astronomical metaphor (§1-FM-02) to
  visualise the scalar-vs-geometric contrast.
- Explicitly declines the "TCT replaces RLHF" reviewer critique.
- Carries the MP-12 composition framing and the Theme C
  scope-device hedge (agent-14 Bias-02).

**Register check.** No new citations. Paradigm claim is bounded by
the LOCKED pool (InstructGPT / DPO / Constitutional AI). R3 compliant.

---

### Move §7-FM-04 — The "three paradigm borrows" closing synthesis

**Purpose.** Close the §7 Discussion synthesis paragraph with an
honest declaration of the three framing borrows, so the reader
leaves §7 knowing exactly what this paper imported from where.

**ISO anchors.** ISO-01; ISO-02; ISO-04; ISO-06; ISO-07 (all five
body-named).

**Theme anchors.** All six (A, B, C, D, E, F).

**Paste-ready seed (≈ 110 words):**

> This paper's vocabulary deliberately borrows from four adjacent
> traditions: **astronomical constellations** for the
> fixed-reference-point object (§5.1); **signal-processing sensor
> fusion** for the N-independent-measurements intuition underlying
> DDA (§3.1); **Kolmogorov / MDL** for the
> structured-signals-per-bit framing of meaning compression (§3.1);
> **formal-methods runtime verification** for the per-output
> acceptance predicate (§5.3); and **statistical process control**
> for the tolerance-bound vocabulary of the guard's thresholds
> (§5.4). Each borrow is at the topology-of-mechanism level, not
> at the mathematics level; we do not claim any of the borrows as
> novel. We claim that their composition over frozen multi-modal
> embedder instances is an architectural move no paper in the
> LOCKED pool has previously made.

**Why this works.**

- Honest enumeration of the five body-named borrows.
- Clarifies that borrows are **topology-level**, preempting
  "did you reinvent Kalman filtering?" style critiques.
- Closes with the MP-12 composition-not-invention commitment, which
  is the paper's structural-honesty register.

**Register check.** R1, R3 preserved. Bounds the novelty claim to
the LOCKED 88-citation pool (R3 pool-relative scope).

---

## Minor / optional framing moves

### Move §7-FM-O1 — Biological multi-modal identity recognition parenthetical (OPTIONAL)

**Purpose.** Add one parenthetical in §7 Discussion for reader
breathing room, naming the biological parallel without committing
to it as a citation.

**ISO anchors.** ISO-03 (biological multi-modal identity
recognition). Metaphor-only.

**Paste-ready seed (≤ 20 words):**

> (much as human identity recognition conjoins face, voice, and
> gait rather than rely on any single channel)

**Recommendation.** Use at most once, as a parenthetical aside.
Agent-20 recommendation: include at agent-39's discretion; omit if
§7 prose is already dense.

**Register check.** Metaphor-only; not a neuroscience citation.

---

### Move §7-FM-O2 — Dynamical-systems basin-of-attraction parenthetical (OPTIONAL)

**Purpose.** Add one optional parenthetical for readers with a
complex-systems background.

**ISO anchors.** ISO-08 (canalization / Waddington). Metaphor-only.

**Paste-ready seed (≤ 25 words):**

> (the generator stays within a narrow basin of attraction around
> the identity centroids, in the generic dynamical-systems sense —
> not a claim about developmental canalization)

**Recommendation.** Use only if §7 prose explicitly discusses
generator drift. Avoid the Waddington / canalization vocabulary
directly; "basin of attraction" is the generic dynamical-systems
term.

**Register check.** Explicit self-demotion from canalization; R3
compliant.

---

## Not-to-do list

Agent-20 explicitly recommends **against** the following framing
moves (they are listed here for pipeline-visibility discipline):

1. **Do not invoke Friston's free-energy principle.** ISO-05
   thermodynamics / entropy gradient is metaphor-only. Naming
   Friston invites a contested theoretical programme.
2. **Do not invoke Waddington or canalization by name.** ISO-08 is
   metaphor-only. If dynamical-systems framing is needed, use
   "basin of attraction" (generic).
3. **Do not claim Kalman-filter mathematics.** ISO-02 sensor fusion
   is at the topology level only. No linear-Gaussian claim.
4. **Do not claim DDA is a new information-theoretic theorem.**
   ISO-04 MDL is a tradition-family borrow. The MI-adjusted
   meaning-compression ratio is future work (RO-8 / Q1).
5. **Do not anchor SPC with six-sigma-specific numerics.** ISO-07
   SPC borrow is topological. Case 3's $\tau = 0.95$ is a
   SECS-domain calibration, not a 3-sigma statistical bound.
6. **Do not add new citations.** R3 is hard-locked. No new papers
   enter the 88-citation pool via ISO framing.

---

## Placement summary — section-by-section

| § | Primary framing moves | ISO anchors | Theme anchors |
|---|---|---|---|
| **§1** | §1-FM-01, §1-FM-02, §1-FM-03 | ISO-01, ISO-02, ISO-04, ISO-06 | A, B, C, E |
| **§2.5** | (inherit §1-FM-02 terminology) | ISO-02, ISO-03 (metaphor) | D |
| **§2.6** | (inherit §1-FM-02 + §7-FM-03) | ISO-06 | C |
| **§3.1** | §1-FM-01 restatement at formal depth | ISO-02, ISO-04 | A, B |
| **§3.4** | §1-FM-03 restatement at counting depth | ISO-02 | A |
| **§5.1** | §1-FM-02 restatement at technical depth | ISO-06 | C, D, F |
| **§5.3** | §7-FM-02 preview at technical depth | ISO-01, ISO-07 | E |
| **§5.4** | §7-FM-02 vocabulary commitment | ISO-07 | E |
| **§6.3** | (inherit §5.3 + §5.4 vocabulary) | ISO-07 | E, F |
| **§7** | §7-FM-01, §7-FM-02, §7-FM-03, §7-FM-04 | All five body-named | All six |
| **§10.2** | (inherit §3.1 MDL framing) | ISO-04 | A |

---

## Confidence

**HIGH.**

- Every framing move has an explicit paste-ready seed with word
  count.
- Every move names its ISO anchors, theme anchors, and register
  checks.
- Every move has a "why this works" justification tied to agent-17
  theme coverage and agent-19 RO placement.
- Register rules R1 and R3 preserved verbatim; R4 Case-3-measured
  discipline carries through.
- Not-to-do list preempts six specific framing mistakes that
  adversarial reviewers would flag.

## Changelog

- 2026-04-17 — Initial issue by agent-20-interdisciplinary-pollinator.
