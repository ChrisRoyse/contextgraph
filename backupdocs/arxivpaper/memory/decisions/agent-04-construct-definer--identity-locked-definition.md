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
  - memory/decisions/agent-03-self-ask-decomposer--essential-questions.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
tags: construct, definition, identity-locked-generation, locked, q15
answers_questions: [Q15]
---

# Construct 4 — Identity-Locked Generation

**Glossary anchor.** Tier A, entry #4 (`canonical-glossary.md`). Glossary wins on
disagreement.

**Evidence tier.** T4 (predicate definition); T1 (Case 3 — voice cloning instantiation with
measured WavLM SECS 0.961); T3 (Cases 1 and 2 projections).

**Binding question.** Q15 from `essential-questions.md`.

**Forbidden substitutes.** "Identity-preserving" (V3), "personalised generation" for our
method (vocabulary-rules table), "identity prototype" (V1), "identity anchor" (V2).

---

## 4.1 Conceptual definition

**Identity-locked generation** is a generation regime in which every output is verified at
inference to fall within per-modality cosine thresholds (glossary #11) of a specific
subject's frozen constellation (glossary #8). The subject's identity is operationalised as
the constellation — a set of per-modality centroids (glossary #7) built from the subject's
reference sets (glossary #9) — and "identity-locked" means that the generator is held to a
per-output, per-modality cosine check against that constellation, not merely to a
distributional preference shaped during training.

Identity-locked generation is the **inference-time outcome** of TCT (glossary #3) when the
subject is a specific person, style, or persona. TCT defines the training + runtime
protocol; identity-locked generation is the runtime **property** that TCT enforces.

---

## 4.2 Formal predicate

Let $C_P = \{(m, c_m) : m \in \mathcal{M}\}$ be the constellation of subject $P$
(`tct-definition.md` §3.2, Phase 1). Let $\{\tau_m\}_{m \in \mathcal{M}}$ be the
per-modality thresholds (glossary #11). Let $\hat{y}$ be a candidate generated output and
$\phi_m$ the frozen embedder for modality $m$.

**Strict (conjunctive) predicate:**

$$
\boxed{\;\mathrm{IdentityLocked}(\hat{y}; C_P, \tau) \iff \forall m \in \mathcal{M}.\; \cos\bigl(\phi_m(\hat{y}), c_m\bigr) \geq \tau_m.\;}
$$

Equivalently:

$$
\mathrm{IdentityLocked}(\hat{y}; C_P, \tau) \iff \min_{m \in \mathcal{M}} \bigl[\cos(\phi_m(\hat{y}), c_m) - \tau_m\bigr] \geq 0.
$$

$\hat{y}$ is **accepted** iff the predicate holds; otherwise it is **rejected** and the
system resamples (up to a finite budget $K$) or returns no output. A generation session is
identity-locked iff every returned output satisfies the predicate.

**Relaxed (soft) variant — weighted-min / best-of-$K$**, as used in Case 3:

$$
\hat{y}^{*} = \arg\max_{k = 1,\ldots,K} \sum_{m \in \mathcal{M}} w_m \cdot \cos(\phi_m(\hat{y}^{(k)}), c_m),
$$

with per-modality weights $w_m \geq 0$, $\sum_m w_m = 1$. The strict predicate is then
applied post-hoc to $\hat{y}^{*}$; Case 3's 10 held-out sentences all satisfied the strict
predicate with $\tau_{\text{WavLM-SECS}}$ set from the same-session band (glossary #17).

---

## 4.3 Variables and indicators

| Symbol / name | Role |
|---|---|
| $C_P$ | Constellation for subject $P$ |
| $c_m$ | Centroid for $(P, m)$ (glossary #7) |
| $\tau_m$ | Per-modality threshold (glossary #11) |
| $\phi_m$ | Frozen embedder for modality $m$ |
| $s_m(\hat{y}) = \cos(\phi_m(\hat{y}), c_m)$ | Per-output, per-modality cosine score |
| $K$ | Best-of-$K$ resampling budget |
| $w_m$ | Per-modality weight in the soft variant |
| **Accept rate** | $\Pr[\mathrm{IdentityLocked}(\hat{y}; C_P, \tau)]$ under the target distribution of prompts |
| **Rejection rate** | $1 - \text{accept rate}$ |
| **Expected budget to first accept** | $\mathbb{E}[\min\{k : \mathrm{IdentityLocked}(\hat{y}^{(k)})\}]$ |

**Primary indicators** (per claim tier):

- **T1** — Case 3: mean per-output $s_{\text{WavLM-SECS}}$, max, distribution over 10
  held-out sentences; observed mean 0.961, max 0.975 (SITREP `tmp/SITREP_2026-04-16_135543.md`
  §Case 3).
- **T3** — Case 1: projected constellation-guard accept-rate on held-out Shakespeare lines
  (pending; camera-ready will re-verify).
- **T3** — Case 2: projected per-output visual-centroid cosine and WavLM SECS on unseen
  prompts (pending; camera-ready will re-verify).

---

## 4.4 Relation to TCT's Phase-3 guard

Identity-locked generation **is** what the TCT Phase-3 constellation guard (glossary #12;
`tct-definition.md` §3.2 Phase 3) enforces. The guard computes $s_m$ for every modality at
every generation step; the predicate above is exactly the guard's accept condition. TCT's
three-phase protocol is the **procedure** that produces identity-locked generation; the
predicate in §4.2 is the **property** TCT delivers.

A system that runs only Phases 1 and 2 of TCT (constellation construction + training with a
geometric-proximity loss) but omits Phase 3 (no per-output cosine check) may push outputs
toward the constellation on average but does **not** produce identity-locked generation in
the sense defined here. The per-output verification is essential; identity-locked generation
is defined at the per-output granularity, not the batch-average granularity.

---

## 4.5 What identity-locked generation is NOT

- **NOT personalisation.** Personalisation softly steers outputs toward user preferences
  (e.g., preference-tuned LoRA). No per-output geometric check; no formal predicate. A
  personalised model may produce any output and still be "personalised on average".
  Identity-locked generation is a per-output predicate.
- **NOT style transfer.** Style transfer maps an input in one style to an output in another
  without making an identity claim about a specific subject. Identity-locked generation is
  tied to a specific subject's constellation.
- **NOT subject-specific fine-tuning alone.** Fine-tuning a LoRA on subject data (as in
  Cases 1 and 2's Phase 2) pushes the generator toward the subject but does not by itself
  validate each output. Without the runtime constellation guard (Phase 3), the regime is
  **TCT-trained but not identity-locked at inference**.
- **NOT LoRA-personalised generation.** Same reasoning: LoRA alone has no per-output
  validator.
- **NOT zero-shot TTS.** Zero-shot TTS (e.g., a single-prompt voice clone) produces outputs
  conditioned on one reference clip without computing a centroid and without per-output
  cosine verification. Case 3 is **not zero-shot** in this sense — it uses a 50-clip
  reference set aggregated into a centroid and a WavLM-scored best-of-12 guard at inference.
  V19 enforces the terminology distinction: "50-clip reference set", never "50-clip
  centroid".
- **NOT "identity-preserving" generation.** V3 forbids the phrase "identity-preserving";
  that literature typically lacks a per-output predicate and threshold bank. Always write
  "identity-locked" (hyphenated, both words).
- **NOT watermarking or cryptographic authentication.** Identity-locked generation is a
  geometric property of the generated content; it is not a cryptographic signature. The
  constellation guard can be repurposed as a **detector** (Q19) because the same cosine
  check can be run on arbitrary content and compared to a subject's constellation, but
  detection is an application of the predicate, not the definition.

---

## 4.6 Usage rule (V3, V9)

- Always "identity-locked generation", hyphenated, two words.
- Never "identity preserving", "identity bound", "identity locked" (unhyphenated), or
  "personalised generation" (for our method).
- First use in §1 must distinguish from personalisation (glossary #4 usage rule).
- Every numeric claim about identity lock is tagged T1 (measured, Case 3) or T3 (projected,
  Cases 1 and 2) per V22. No bare numbers.
- When referring to the subject in Case 3, say "the author" (author-is-subject) or "the
  speaker in Case 3"; do not generalise beyond the protocol (P02, FM-05).

---

## 4.7 Cross-references

- **Glossary #4** (identity-locked generation) — canonical entry.
- **Glossary #12** (constellation guard) — the runtime mechanism.
- **Glossary #11** ($\tau_m$) — the per-modality thresholds.
- **`tct-definition.md`** §3.2 Phase 3 — the guard protocol producing this property.
- **Q15** — sample-complexity collapse claim (one reference set per modality + frozen
  constellation instead of thousands of subject-specific examples).
- **Q12** — Case 3 T1 numerics that instantiate the predicate.
- **Q13** — Cases 1 and 2 T3 projections.
- **Q19** — ethics: the guard can serve as a detector.

---

## Change log

- 2026-04-16 — Initial lock by agent-04-construct-definer.
