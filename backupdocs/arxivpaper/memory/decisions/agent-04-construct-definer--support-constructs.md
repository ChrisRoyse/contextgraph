---
namespace: decisions
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
tags: construct, definition, support, constellation, frozen-embedder, same-session-band, locked
answers_questions: [Q06, Q07, Q08, Q12]
---

# Support Constructs — Constellation, Frozen Embedder, Same-Session Verification Band

These are not Tier-A load-bearing constructs (those are DDA, meaning compression, TCT,
identity-locked generation), but they appear throughout the manuscript and must be defined
precisely and consistently. All three are already in the canonical glossary; this file is
the paper-ready expansion writers pull from.

**Evidence tier.** T4 for the three definitions; T2 for the constellation-guard code
artifact; T1 for the same-session band numbers (Case 3 empirical measurement).

---

## S1 — Constellation (glossary #8)

### S1.1 Conceptual definition

A **constellation** is the set of per-modality centroids for a single subject. It is a
structured collection, not a single vector; its cardinality is the number of modalities
that define the subject's identity in this protocol. Each element is a (modality,
centroid) pair where the centroid is a unit vector in the corresponding frozen embedder's
output space.

### S1.2 Formal definition

Let $P$ be a subject (identity, style, persona). Let $\mathcal{M} = \{m_1, \ldots, m_M\}$
be the modalities that constitute $P$'s identity. For each $m \in \mathcal{M}$ let $c_m$
be the centroid (glossary #7) computed from $P$'s reference set $R_m$ via

$$
c_m = \mathrm{normalize}\Bigl(\frac{1}{|R_m|} \sum_{r \in R_m} \mathrm{normalize}(\phi_m(r))\Bigr).
$$

Then

$$
C_P \;=\; \bigl\{(m, c_m) : m \in \mathcal{M}\bigr\}, \qquad |C_P| = |\mathcal{M}|.
$$

$C_P$ is **frozen** after Phase 1 of TCT: no gradient flows into it; no element is updated
during Phase 2 training or Phase 3 inference.

### S1.3 Variables and indicators

| Symbol | Role |
|---|---|
| $C_P$ | Constellation for $P$ |
| $\mathcal{M}$, $\|C_P\|$ | Modality set and its size |
| $c_m$ | Centroid, unit vector in $\mathbb{R}^{d_m}$ |
| $\{d_m\}$ | Per-modality embedding dimensions |

For Case 3: $\mathcal{M} = \{\text{speaker}\}$; $|C_P| = 1$; $c_{\text{speaker}} \in
\mathbb{R}^{d_{\text{WavLM}}}$. For Case 2: $\mathcal{M} \supseteq \{\text{visual},
\text{speaker}\}$. For Case 1: $\mathcal{M} = \{\text{text-style}\}$ (the Shakespearean
corpus constellation).

### S1.4 What a constellation is NOT (glossary #8, expanded)

- **NOT a single embedding** (glossary rule: "constellation embedding" is forbidden — wrong
  cardinality).
- **NOT a reference set**. A reference set is the raw clips; the constellation is the set
  of centroids computed from the reference sets.
- **NOT a prototype bank**. "Prototype" is forbidden (V1); use "centroid".
- **NOT a codebook**. Codebooks (as in VQ-VAE / RVQ) have many entries per modality and are
  learned; a constellation has one entry per modality and is frozen.

### S1.5 Usage rule

- Always "constellation" (glossary #8). Plural "constellations" is correct when discussing
  multiple subjects or behavioural-state variants.
- The cardinality is $|\mathcal{M}|$, never "the constellation's dimension".
- In equations use $C_P$ with subject subscript; in prose use "the constellation of subject
  $P$" or "subject $P$'s constellation".

---

## S2 — Frozen embedder / measurement instrument (glossary #5 and #10)

### S2.1 Conceptual definition

A **frozen embedder** is a model $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$ (or into a
sparse / variable-length output space) whose parameters are **fixed throughout the DDA
pipeline, the TCT training loop, and the runtime constellation guard** — not updated by
any loss signal at any stage of the pipeline described in this manuscript. We treat a
frozen embedder as a **measurement instrument**: it maps inputs to a fixed-dimensional
representation by a fixed rule, analogous to how a weighing scale maps a mass to a number
by a fixed rule.

"Frozen" is a **behaviour claim about this pipeline**, not a history claim about how the
embedder was created. A model pretrained at some point in the past and held fixed during
this pipeline is **frozen**; a model that was pretrained and is fine-tuned during this
pipeline is **not frozen** even if its architecture is identical.

### S2.2 Formal definition

Let $\phi_m$ be an embedder with parameters $\theta_m$. $\phi_m$ is **frozen** in this
pipeline iff for every training step $t$ of TCT Phase 2 and every inference step $u$ of
Phase 3, $\theta_m^{(t)} = \theta_m^{(u)} = \theta_m^{(0)}$, where $\theta_m^{(0)}$ is the
pretrained value loaded at the start of the pipeline.

A "frozen-with-LoRA" embedder is a frozen base $\theta_m^{(0)}$ with a trainable low-rank
adapter $A_m$: the composite $\phi_m'(x) = (\theta_m^{(0)} \oplus A_m)(x)$ is trainable via
$A_m$ only. Per V18, do not write "frozen base model" for such a composite; write
"frozen-with-LoRA".

An "inference-only" embedder (Case 3's Qwen3-TTS backbone) has no parameter update at any
stage.

### S2.3 Variables and indicators

| Symbol | Role |
|---|---|
| $\phi_m$ | Frozen embedder |
| $\theta_m$ | Parameters |
| $d_m$ | Output dimension (or sparsity / token count for non-dense embedders) |
| $\mathcal{X}$ | Input space |

**Indicator that an embedder is behaving as a measurement instrument**: $\phi_m$ is applied
to both Phase-1 reference clips and Phase-3 candidate outputs and returns identical vectors
for identical inputs across phases. The pre-flight test: embed one fixed calibration input
at the start of Phase 1 and again at the end of Phase 3; the vectors must match bit-for-bit.

### S2.4 What a frozen embedder is NOT (glossary #5, #10, expanded)

- **NOT an encoder** in this manuscript. "Encoder" is reserved for the speaker-encoder /
  cross-encoder terminology in Case 3 (glossary #20).
- **NOT a backbone**. "Backbone" usually implies the large-model body on which task heads
  are trained; a frozen embedder has no task head trained in this pipeline.
- **NOT a tokenizer**. Tokenizers produce discrete tokens; embedders here produce dense,
  sparse, or variable-length continuous representations.
- **NOT "pretrained"** where "frozen" is meant (V18). Pretrained is a history claim; frozen
  is a behaviour claim in this pipeline.
- **NOT "static"** — too weak. A static target could still be updated out-of-band.

### S2.5 Usage rule

- Use "embedder" for generic references.
- When citing a specific embedder, use its name: "E1", "E5", "E13", "WavLM-SV",
  "SigLIP", "CLIP ViT-L/14", "Qwen3-speaker", etc.
- Context Graph has 13 embedders E1–E13 (mission brief "Key Architecture Facts").
  ClipCannon has 7 named modalities (V25).
- Always "frozen embedder" when the behaviour claim matters (e.g., inside the DDA or TCT
  definition). "Frozen-with-LoRA" for composites.
- Do not use "embedder" and "modality" interchangeably (V17): a modality is the perceptual
  axis; an embedder is the instrument that measures it.

---

## S3 — Same-session verification band (our empirical observation — LABEL, not benchmark)

### S3.1 Conceptual definition

The **WavLM-SECS same-session verification band** is the empirical range of WavLM
speaker-encoder cosine similarities we observed when scoring pairs of the **Case 3
speaker's own real recordings** taken from the same recording session against each other.
It is a measurement of the encoder's discrimination ceiling for that speaker's audio
character under the conditions of Case 3 — **not a universal constant** across speakers,
encoders, or sessions, and **not a published benchmark result**. It is our own measurement
on Case 3's subject, and the manuscript must say so every time it is cited (V12).

### S3.2 Observed band (Case 3)

For the speaker in Case 3 (author-is-subject), pairwise WavLM-SECS cosines between the
speaker's own real recordings from the same session fall in the empirical range
**0.95 – 0.99** (SITREP `tmp/SITREP_2026-04-16_135543.md` §Case 3; white paper
`tasks/newdata/alldata.md`). This is the reference band against which Case 3's clone
outputs are compared. The clone outputs achieved WavLM SECS **mean 0.961 (max 0.975)** on
10 unseen sentences, which falls within the observed band (T1).

### S3.3 Role of the band in TCT

The band supplies the calibration anchor for the per-modality threshold $\tau_{\text{speaker}}$
in Case 3 (glossary #11). We set $\tau_{\text{speaker}}$ at or near the lower end of the
observed band so that accepted clone outputs are indistinguishable, in WavLM-SECS, from
the subject's own same-session recordings. This makes the per-modality threshold a
**subject-specific empirical quantity**, not a universal constant; the band is its source.

### S3.4 Variables and indicators

| Symbol / name | Role |
|---|---|
| $\tau_{\text{speaker}}$ | Per-modality threshold for speaker modality |
| "Same-session band (lower, upper)" | The observed $(\min, \max)$ on pairs of the subject's own same-session recordings |
| Mean / max / std of clone SECS on held-out | The measurement the band is compared against |

### S3.5 What the same-session band is NOT (glossary #17 "NOT" clause, expanded)

- **NOT a theoretical upper bound on any encoder.** WavLM-SECS between two different
  recordings of the same speaker will depend on session, recording conditions, sentence
  content, and speaker fatigue; the band is what we measured, not what is provably
  achievable.
- **NOT a universal constant across speakers.** Speakers with more variable voice
  characteristics, or recorded across multiple sessions with equipment changes, will have
  lower same-session bands. Case 3's band is **the author's band on the specific reference
  session**, not a cross-speaker statistic.
- **NOT a published benchmark result.** No prior work supplied this number; we measured it
  on Case 3's reference audio. Always label it as our empirical observation.
- **NOT an endorsement of a particular threshold.** Setting $\tau_{\text{speaker}}$ inside
  the band is a calibration choice; the band itself is the underlying measurement.

### S3.6 Usage rule (V12)

- First use must read: "the WavLM-SECS same-session verification band
  (**an empirical observation on Case 3's speaker, not a universal value**)" or an
  equivalent phrasing that carries the hedge.
- When comparing clone outputs to the band, always write: "falls within our measured
  same-session band" — **never** "reaches the same-session band" (implies it is a ceiling
  universally defined).
- Bare "SECS" is forbidden (V4). Always "WavLM SECS" / "WavLM-large SV cosine" or another
  named encoder.

---

## S4 — Cross-reference table for support constructs

| Construct | Glossary # | Used in | Tier |
|---|---|---|---|
| Constellation | #8 | §3 (concept), §5 Phase 1, §6 (every case study) | T4 |
| Frozen embedder | #5, #10 | §3 (DDA), §5 (TCT Phase 1 / 2 / 3), §6 | T4 method; T2 instrument list |
| Same-session verification band | #17 | §6.3 Case 3; §5.3 threshold calibration | T1 (measured on Case 3) |

---

## Change log

- 2026-04-16 — Initial lock by agent-04-construct-definer.
