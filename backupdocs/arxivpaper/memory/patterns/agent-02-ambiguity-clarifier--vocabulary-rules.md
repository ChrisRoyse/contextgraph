---
namespace: patterns
agent_id: agent-02-ambiguity-clarifier
timestamp: 2026-04-16T15:30:00Z
status: active
tags: vocabulary, rules, writer-checklist, arxiv
---

# Vocabulary Rules — Downstream Writer Checklist

Short, enforceable rules. Every writer (agents 35–41) runs this list against their section
before handing off. Agent-42 (adversarial-reviewer) regex-greps for violations.

The canonical definitions live in
`./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md`.
This file is the **rules extract**, not the authority.

---

## Absolute prohibitions (regex-enforced)

**Rule V1.** The word **"prototype"** is FORBIDDEN in the manuscript body. Use "centroid".
Exception: when discussing prototypical networks (Snell et al., 2017) in §2 related work, use
"prototypical network" quoted as the method name.

**Rule V2.** The word **"anchor"** as a noun referring to a centroid is FORBIDDEN. Use
"centroid". Exception: "anchor point" is allowed only in pure geometry discussion if no
centroid is meant.

**Rule V3.** **"Identity-preserving"** is FORBIDDEN. Use "identity-locked" (always hyphenated).

**Rule V4.** Bare **"SECS"** without an encoder qualifier is FORBIDDEN. Always one of:
"WavLM SECS", "WavLM-large SV cosine", "SpeechBrain SECS", "SpeechBrain ECAPA-TDNN SECS",
"Qwen3 SECS", "Qwen3-speaker SECS". First use in a section: fully qualified; later uses in
the same section: shortest unambiguous form.

**Rule V5.** **"Semantic compression"** is FORBIDDEN. Use "meaning compression".

**Rule V6.** **"Data augmentation"**, **"synthetic data"**, **"self-training"**, **"distillation"**
are FORBIDDEN as substitutes for DDA. Use "Derived Data Abundance (DDA)".

**Rule V7.** **Superlatives** (revolutionary, breakthrough, game-changing, best-in-class,
unprecedented, world-class, industry-leading, paradigm shift, the future of, solves AGI) are
FORBIDDEN in the body per P09 / G4.

**Rule V8.** **Teleox.ai / Leapable.ai** are FORBIDDEN in the body. Acknowledgements only.

---

## First-use discipline

**Rule V9.** The four load-bearing constructs spell out full name + acronym on first use:
- "Derived Data Abundance (DDA)" — first use in §1, again in §3.1.
- "Teleological Constellation Training (TCT)" — first use in §1, again in §5 / §6.
- "meaning compression" — lower-case, first use in §1, again in §3.1.
- "identity-locked generation" — hyphenated, first use in §1.

**Rule V10.** First use of **"alignment"** in §1 and again in §2.6 must include the
disambiguation clause: "in this paper, alignment means geometric proximity of outputs to a
constellation centroid, not RLHF-style scalar-reward alignment". Thereafter "alignment" is
usable unqualified within the same section.

**Rule V11.** First use of **"orthogonal"** (applied to signals) in §3.4 must include the
hedge: "approximately independent; strict formal orthogonality is an open question (§10)".
Thereafter "orthogonal signals" is acceptable shorthand.

**Rule V12.** First use of **"same-session band"** in §9 must be qualified as "an empirical
observation on Case 3's speaker, not a universal value".

**Rule V13.** First use of **"cross-encoder"** in §9 must disambiguate from the IR sense.

**Rule V14.** First use of **"the data wall"** in §1 cites Villalobos et al. (2024).

**Rule V15.** First use of **"model collapse"** in §3.3 cites Shumailov et al. (2024) AND
states "DDA has no generative feedback loop; the collapse theorem is silent on DDA".

---

## Construct consistency

**Rule V16.** **Embedding** = one embedder's output on one input (a vector).
**Centroid** = normalised-mean-of-normalised embeddings for one (subject, modality) (a vector).
**Constellation** = the set of centroids across modalities for one subject.
**Reference set** = the raw clips from which a centroid is computed.
These are four distinct concepts. Writers NEVER use one term where another is meant.

**Rule V17.** **Embedder** = the frozen instrument; **modality** = the perceptual axis being
measured. One modality may have more than one embedder. Do NOT use "embedder" and "modality"
interchangeably.

**Rule V18.** **Frozen** is always the modifier for "not updated during TCT". When a base
model is frozen but has trainable LoRA on top, write "frozen-with-LoRA". For Case 3's
Qwen3-TTS (no fine-tune at all), write "inference-only".

**Rule V19.** Reference set sizes: write "n-clip reference set" (e.g., "50-clip reference
set"), NEVER "50-clip centroid" (dimensional ambiguity).

---

## Evidence-tier labeling

**Rule V20.** Every §6 (Results) subsection header carries its evidence tier in parentheses,
e.g., "§6.3 Case 3 — Voice Cloning (T1)". Tiers:
- **T1** — Measured (numeric result in a cited forensic report / SITREP).
- **T2** — Constructive (system exists and runs; cite file paths and line counts).
- **T3** — Projected (architecturally complete, measurement pending).
- **T4** — Theoretical (derivation, count, framework).

**Rule V21.** Writers NEVER mix tiers within a single claim sentence. If a paragraph discusses
multiple tiers, each sentence is tagged.

**Rule V22.** Every numeric value in prose has either a citation to a T1 source (measured) OR
the flag "(projected; camera-ready will re-verify)" (T3). No bare numbers. Per P01 / G1.

---

## System and case-study naming

**Rule V23.** "Context Graph" — two words, both capitalised. Never "ContextGraph" / "CG" /
"contextgraph" in prose.

**Rule V24.** "ClipCannon" — one word, internal capital. Never "Clip Cannon".

**Rule V25.** Embedder labels E1–E13 for Context Graph; for ClipCannon, use the named seven
(visual, audio-speaker, audio-voice, emotion, FLAME/expression, transcript, meta).

---

## Forbidden substitutions (quick reference)

| If you are tempted to write… | Write instead |
|---|---|
| prototype | centroid |
| anchor (as centroid) | centroid |
| reference vector | reference embedding |
| prototype bank | constellation |
| identity prototype | constellation centroid |
| identity-preserving | identity-locked |
| personalised generation (our method) | identity-locked generation |
| data augmentation (for DDA) | Derived Data Abundance (DDA) |
| synthetic data (for DDA) | DDA (with explicit exclusion of synthetic data) |
| semantic compression | meaning compression |
| TCTraining / TC-Training / constellation training (alone) | Teleological Constellation Training (TCT) |
| immutable centroid | frozen centroid |
| pretrained embedder (where frozen is meant) | frozen embedder |
| SECS (bare) | WavLM SECS / SpeechBrain SECS / Qwen3 SECS |
| same-session ceiling (universal) | our measured same-session band on Case 3 |
| alignment (bare, first use) | geometric alignment to constellation |
| 50-clip centroid | centroid computed from a 50-clip reference set |
| revolutionary / breakthrough / … | (delete; rewrite without superlative) |

---

## Self-check regex (for writers before handoff)

Before finishing a section, run mentally (or programmatically):

```
# V1–V8 prohibitions:
grep -iE '\b(prototype|anchor|identity[- ]preserving|semantic compression|revolutionary|breakthrough|game[- ]changing|best[- ]in[- ]class|unprecedented|world[- ]class|industry[- ]leading|paradigm shift|solves? AGI)\b' section.md
# bare-SECS check:
grep -iE '\bSECS\b' section.md | grep -vE '(WavLM|SpeechBrain|Qwen3|ECAPA).{0,30}SECS'
# Teleox / Leapable outside Acknowledgements:
grep -iE '\b(Teleox|Leapable)\b' section.md
```

Any hits = section is NOT ready. Fix and re-check.
