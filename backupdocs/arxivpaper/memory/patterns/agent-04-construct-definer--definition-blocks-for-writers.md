---
namespace: patterns
agent_id: agent-04-construct-definer
timestamp: 2026-04-16T16:00:00Z
status: active
tags: definition-blocks, writer-handoff, dda, meaning-compression, tct, identity-locked, support-constructs
---

# Consolidated Definition Blocks — Single-Page Writer Handoff

**Audience.** The intro-writer (agent-35), methodology-writer (agent-37), results-writer
(agent-38), and any other section writer (agents 36, 39, 40, 41) who must quote a
construct definition in the manuscript.

**Rule.** Pull from **this page** first. If more formal depth is needed, follow the link at
the bottom of each block to the full decision file. Never re-invent a definition; never
paraphrase away the hedges (V11, V12). Never substitute a forbidden term (V1–V8).

All vocabulary is in lock-step with the canonical glossary. If a writer finds a
disagreement between this file and the glossary, the glossary wins — file a blocker and
this file is re-issued.

---

## Block 1 — Derived Data Abundance (DDA) — Tier A #1 — Q04, T4/T2

**First-use form.** "Derived Data Abundance (DDA)".

**One-paragraph definition (paste-ready for §1 / §3.1 / §5 first-use).**

> **Derived Data Abundance (DDA)** is a data-enrichment principle stating that a fixed
> corpus of $n$ raw inputs projected through $N$ frozen, independently-trained embedders
> yields $n(N + \binom{N}{2})$ structured supervisory signals — $n$ per-embedder labeled
> vectors plus $n\binom{N}{2}$ pairwise cross-embedder interaction features — **without
> generating any new inputs**. DDA is **not synthetic data**, **not data augmentation**,
> **not distillation**, and **not self-training**; it decomposes existing inputs along $N$
> frozen measurement axes. The word "orthogonal" applied to these signals means
> approximately independent; strict formal orthogonality is an open question (§10).

**Formal count.**

$$
|D'| = n \cdot \Bigl(N + \binom{N}{2}\Bigr) = n \cdot \frac{N(N+1)}{2}.
$$

**Worked instantiation.** Context Graph at $N=13$: $13 + \binom{13}{2} = 13 + 78 = 91$
signals per input. ClipCannon at $N=7$: $7 + 21 = 28$ signals per input.

**Usage reminders.**
- V6: never "synthetic data" / "data augmentation" / "distillation" / "self-training" as
  substitutes for DDA.
- V11: first use of "orthogonal" carries the hedge above.
- V18: embedders are "frozen" (or "frozen-with-LoRA"), not "pretrained" where "frozen" is
  meant.

**Authority.** `./memory/decisions/agent-04-construct-definer--dda-definition.md`;
glossary #1, #14, #15.

---

## Block 2 — Meaning Compression — Tier A #2 — Q05, T4

**First-use form.** "meaning compression" (lower-case, two words, not hyphenated). No
acronym in prose; the ratio $\mathrm{MC}$ in equations is the single exception.

**One-paragraph definition (paste-ready for §1 / §3.1 first-use).**

> **Meaning compression** is a compression category that increases the number of
> information-theoretically-structured labeled supervisory signals **per unit of raw
> data**, by projecting the raw data through $N$ independently-trained frozen embedders.
> It is a fourth category alongside three pre-existing ones: **bit compression** (gzip /
> PNG / FLAC — bits / bits), **weight compression** (GPTQ / SparseGPT / distillation —
> accuracy / parameters), and **activation compression** (TurboQuant / KV-cache
> quantization — memory / output quality). Meaning compression operates on a
> dimensionally-distinct unit pair: **signals per unit of raw data** at fixed corpus.
> It is **not** "semantic compression" (forbidden phrasing, V5), not weight compression,
> not activation compression, not bit compression, not summarisation.

**Four-category taxonomy table (for §3.1 / §3.5).**

| Category | Input unit | Output unit | Typical techniques |
|---|---|---|---|
| Bit | bits | bits | gzip, zstd, PNG, FLAC |
| Weight | parameters / accuracy | reduced-param accuracy | GPTQ, SparseGPT, distillation |
| Activation | peak memory / quality | reduced-memory quality | TurboQuant, KV-cache quantization |
| **Meaning (this paper)** | unit of raw data | labeled signals | DDA |

**Compression ratio.**

$$
\mathrm{MC}(\Phi, D) = N + \binom{N}{2} = \frac{N(N+1)}{2} \text{ signals / input at fixed } D.
$$

**Usage reminders.**
- V5: "semantic compression" is forbidden — always "meaning compression".
- First use in §3.1 must state the four-category orthogonality.

**Authority.** `./memory/decisions/agent-04-construct-definer--meaning-compression-definition.md`;
glossary #2.

---

## Block 3 — Teleological Constellation Training (TCT) — Tier A #3 — Q06/Q07, T4/T2

**First-use form.** "Teleological Constellation Training (TCT)".

**One-paragraph definition (paste-ready for §1 / §5 / §6 first-use).**

> **Teleological Constellation Training (TCT)** is a three-phase training method. In Phase
> 1 (constellation construction) we compute a frozen multi-modal **constellation** $C_P$
> from per-modality reference sets for subject $P$: each centroid $c_m$ is the
> L2-normalised mean of L2-normalised embeddings $\phi_m(r)$ over the reference set $R_m$.
> In Phase 2 (constellation-conditioned fine-tuning) we train the generator with the
> composite loss
> $\mathcal{L}(\theta) = \mathcal{L}_\text{gen}(\theta) + \sum_m \lambda_m \bigl(1 - \cos(\phi_m(\hat{y}), c_m)\bigr)$,
> freezing every $\phi_m$ and every $c_m$. In Phase 3 (runtime **constellation guard**)
> every generated output $\hat{y}$ is re-embedded through the same frozen $\phi_m$ and
> accepted iff $\cos(\phi_m(\hat{y}), c_m) \geq \tau_m$ for every modality. TCT is
> **teleological** because the aim is a frozen geometric target, not a learned reward;
> it is **not** RLHF, DPO, Constitutional AI, contrastive learning, metric learning,
> prototypical-network classification, or vanilla LoRA fine-tuning.

**Training objective (equation block).**

$$
\mathcal{L}(\theta) = \mathcal{L}_{\mathrm{gen}}(\theta) + \sum_{m \in \mathcal{M}} \lambda_m \bigl(1 - \cos(\phi_m(\hat{y}), c_m)\bigr).
$$

**Acceptance predicate (equation block).**

$$
\mathrm{Accept}(\hat{y}) \iff \forall m \in \mathcal{M}.\; \cos(\phi_m(\hat{y}), c_m) \geq \tau_m.
$$

**Contrast row (RLHF / DPO / CAI).** Their aim is a *learned scalar reward* against
preference pairs; there is no per-output geometric check at inference. TCT's aim is a
*frozen geometric target*; every output is verified against the same constellation at
inference. V10 disambiguation must appear on first use of "alignment" in §2.6.

**Usage reminders.**
- V9: spell out "Teleological Constellation Training (TCT)" on first use.
- Never "TCTraining", "TC-Training", or "constellation training" alone.
- V10: first "alignment" = geometric alignment to constellation.
- V18: "frozen-with-LoRA" for LoRA composites; "inference-only" for Case 3.

**Authority.** `./memory/decisions/agent-04-construct-definer--tct-definition.md`;
glossary #3, #7, #8, #10, #11, #12, #13.

---

## Block 4 — Identity-Locked Generation — Tier A #4 — Q15, T4/T1/T3

**First-use form.** "identity-locked generation" (hyphenated, two words).

**One-paragraph definition (paste-ready for §1 first-use).**

> **Identity-locked generation** is a generation regime in which every output is verified
> at inference to fall within per-modality cosine thresholds of a specific subject's
> frozen **constellation**. Formally, $\mathrm{IdentityLocked}(\hat{y}; C_P, \tau) \iff
> \forall m \in \mathcal{M}.\; \cos(\phi_m(\hat{y}), c_m) \geq \tau_m$. It is the
> inference-time property that **Teleological Constellation Training (TCT)** delivers
> through its Phase-3 constellation guard. Identity-locked generation is **not**
> personalisation (no per-output predicate), **not** style transfer (no identity claim),
> **not** subject-specific fine-tuning alone (no runtime validator), **not**
> LoRA-personalised generation (same: no per-output check), and **not** zero-shot TTS
> (Case 3 uses a 50-clip reference set, not a single prompt). Never "identity-preserving"
> (V3).

**Acceptance predicate.**

$$
\mathrm{IdentityLocked}(\hat{y}; C_P, \tau) \iff \forall m \in \mathcal{M}.\; \cos(\phi_m(\hat{y}), c_m) \geq \tau_m.
$$

**Evidence tier for each case study.**
- Case 3 — **T1**: WavLM SECS mean 0.961, max 0.975 on 10 unseen sentences (SITREP
  `tmp/SITREP_2026-04-16_135543.md`).
- Case 1 — **T3**: constellation-guard accept-rate pending; camera-ready will re-verify.
- Case 2 — **T3**: per-output visual-centroid cosine and WavLM SECS pending;
  camera-ready will re-verify.

**Usage reminders.**
- V3: never "identity-preserving"; always "identity-locked".
- V22: every numeric value is T1 (measured) or flagged "(projected; camera-ready will
  re-verify)" (T3). No bare numbers.

**Authority.** `./memory/decisions/agent-04-construct-definer--identity-locked-definition.md`;
glossary #4, #11, #12.

---

## Block 5 — Support constructs (one paragraph each)

### Constellation (glossary #8)

> The **constellation** $C_P = \{(m, c_m) : m \in \mathcal{M}\}$ is the set of
> per-modality centroids for subject $P$. Each $c_m$ is the L2-normalised mean of
> L2-normalised $\phi_m(r)$ over the reference set $R_m$; $c_m$ is a unit vector in
> $\mathbb{R}^{d_m}$. The constellation is **frozen** after construction. A constellation
> is **not** a single embedding, **not** a reference set, **not** a prototype bank
> (V1 forbids "prototype"), and **not** a codebook.

### Frozen embedder / measurement instrument (glossary #5, #10)

> A **frozen embedder** $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$ is a model whose
> parameters are fixed throughout the DDA pipeline, the TCT training loop, and the runtime
> constellation guard. We treat it as a **measurement instrument**: it maps inputs to a
> fixed-dimensional representation by a fixed rule. "Frozen" is a behaviour claim about
> this pipeline, not a history claim about pretraining. A frozen-base model with a
> trainable LoRA adapter is written **"frozen-with-LoRA"** (V18); a model with no parameter
> update at all is written **"inference-only"** (Case 3's Qwen3-TTS backbone).

### Same-session verification band (glossary #17)

> The **WavLM-SECS same-session verification band** is the empirical range (approximately
> **0.95 – 0.99**) observed when scoring pairs of Case 3's speaker's own real recordings
> from the same recording session against each other. It is **our own empirical
> observation on Case 3's speaker, not a published benchmark and not a universal value
> across speakers or encoders** (V12). Case 3's clone outputs achieved WavLM SECS **mean
> 0.961 (max 0.975)** on 10 unseen sentences, which falls within the observed band. The
> band is used to calibrate the per-modality threshold $\tau_{\text{speaker}}$ for Case 3
> only; Cases 1 and 2 thresholds are T3 projected.

**Authority for all three support constructs.**
`./memory/decisions/agent-04-construct-definer--support-constructs.md`.

---

## Quick-check list for writers (before handoff)

- [ ] Is the first use of **DDA** in my section followed within two sentences by the phrase
  "not synthetic data" or a pointer to §3.3? (V6)
- [ ] Is the first use of **orthogonal** applied to signals carrying the hedge
  "approximately independent; strict formal orthogonality is an open question (§10)"? (V11)
- [ ] Is **meaning compression** always lower-case, two words, not hyphenated, and never
  "semantic compression"? (V5)
- [ ] Is the first use of **alignment** in §1 / §2.6 carrying the V10 geometric-vs-reward
  disambiguation clause?
- [ ] Is every numeric value tagged T1 (cited to a specific report) or T3 (flagged
  "(projected; camera-ready will re-verify)")? (V22)
- [ ] Is **SECS** always qualified by encoder — "WavLM SECS", "SpeechBrain SECS", "Qwen3
  SECS"? (V4)
- [ ] Is **same-session band** first-use hedged as "our empirical observation on Case 3's
  speaker, not a universal value"? (V12)
- [ ] Is **identity-locked generation** always hyphenated, never "identity-preserving"? (V3)
- [ ] Is every use of **Teleox.ai / Leapable.ai** confined to Acknowledgements? (V8)

A single failed check blocks handoff; rewrite and re-run before passing to the next agent.

---

## Change log

- 2026-04-16 — Initial issue by agent-04-construct-definer.
