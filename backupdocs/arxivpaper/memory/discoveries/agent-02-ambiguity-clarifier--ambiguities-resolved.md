---
namespace: discoveries
agent_id: agent-02-ambiguity-clarifier
timestamp: 2026-04-16T15:30:00Z
status: active
tags: vocabulary, ambiguities, resolutions, rationale, arxiv
---

# Ambiguities Resolved — Rationale Log

This file documents every live ambiguity I found in the mission brief and prior manuscript,
the candidate meanings, the resolution chosen, and the failure mode(s) each resolution defuses.

For each resolution, the format is:
- **Term** — the ambiguous surface form.
- **Live in** — where I found it (mission brief / prior manuscript section / SITREP).
- **Candidate meanings** — the 2–4 interpretations that a reader could plausibly pick.
- **Resolution** — what I locked in (cross-reference to canonical-glossary entry #).
- **Rationale** — why this interpretation, not the others.
- **Defuses** — which failure mode (FM-*) or gate (G-*) this resolution protects.

---

## R01 — DDA vs. Data Augmentation vs. Synthetic Data Generation
- **Live in.** Mission brief core claim 1; prior manuscript §3 title; §10 discussion.
- **Candidate meanings.**
  - (A) DDA ≈ multi-task labeling through embedders.
  - (B) DDA ≈ self-supervised augmentation (crop / mask / noise).
  - (C) DDA ≈ synthetic data generation via an external generator.
  - (D) DDA = decomposition of existing inputs along $N$ frozen embedder axes, no new inputs created.
- **Resolution.** (D). See glossary #1.
- **Rationale.** (A) is close but mislabels the object: the multi-task signals are embedding **measurements**, not task labels chosen by a human. (B) is wrong because augmentation produces new *inputs*; DDA produces new *signals* over unchanged inputs. (C) is the most dangerous confusion — the model-collapse literature applies to (C) but is silent on (D); this is exactly the distinction §3.3 makes. (D) is what the $N + \binom{N}{2}$ math requires and what every downstream system actually implements.
- **Defuses.** FM-02 (DDA-as-synthetic-data confusion); G2 Definition-Before-Use.

## R02 — Meaning Compression vs. Bit/Weight/Activation Compression
- **Live in.** Mission brief core claim 2; prior §3.5 table.
- **Candidate meanings.**
  - (A) A colloquial label for "less data, more meaning" that overlaps semantic compression in IR.
  - (B) A distinct compression category orthogonal to bit/weight/activation.
  - (C) A marketing label for DDA.
- **Resolution.** (B). See glossary #2.
- **Rationale.** The compression-taxonomy table in §3.5 is the load-bearing claim: meaning compression's unit is "meaningful training signals / unit raw data", which is dimensionally distinct from "bits / bits", "accuracy / parameters", and "memory / output quality". Collapsing it to (C) marketing wording would make the novelty claim indefensible.
- **Defuses.** FM-01 (marketing creep); FM-02; G3 novelty delta.

## R03 — TCT vs. Contrastive / Metric / Prototypical Learning
- **Live in.** Mission brief core claim 3; prior §2.5, §6.
- **Candidate meanings.**
  - (A) TCT is a flavour of supervised contrastive learning.
  - (B) TCT is metric learning with a learned metric.
  - (C) TCT is prototypical networks applied to generation.
  - (D) TCT is geometric-proximity training to a frozen multi-modal centroid set with per-output runtime validation.
- **Resolution.** (D). See glossary #3.
- **Rationale.** Contrastive learning samples negatives from the batch and learns a representation; TCT's representation (embedders) is already fixed. Metric learning learns the metric; TCT's metric (cosine in each frozen embedder's output space) is fixed. Prototypical networks classify; TCT generates. The runtime guard (per-output cosine check at inference) is a property none of (A)/(B)/(C) have and is the biggest differentiator.
- **Defuses.** FM-09 vocabulary drift; G3 novelty delta.

## R04 — Constellation vs. Embedding vs. Prototype vs. Centroid vs. Reference Set
- **Live in.** Mission brief; prior §6.2; §6.4; SITREP (Case 3).
- **Candidate meanings.** Each of those five terms has been used by the prior draft with overlapping senses.
- **Resolution.** Four-level discipline:
  - **Embedding** = the output of one embedder on one input, $\phi_m(x) \in \mathbb{R}^{d_m}$.
  - **Centroid** = normalised-mean-of-normalised embeddings for one (subject, modality). One vector.
  - **Constellation** = the set of centroids across modalities for one subject.
  - **Reference set** = the raw clips used to compute a centroid.
  - "Prototype" and "anchor" (as a noun for centroid) are **FORBIDDEN**.
- **Rationale.** This is exactly FM-09's named failure mode. Preventing drift requires strict one-word-one-meaning. "Prototype" carries baggage from prototypical networks (a learned prototype that can move); "anchor" carries baggage from triplet loss (one of a triplet). Neither matches TCT's frozen-centroid semantics.
- **Defuses.** FM-09; G2.

## R05 — Modality vs. Embedder vs. Channel vs. Axis
- **Live in.** Prior §6.2; throughout case studies.
- **Candidate meanings.**
  - (A) Modality = embedder (one-to-one).
  - (B) Modality = perceptual channel (e.g., visual, audio), embedder = the instrument measuring it.
  - (C) Modality = measurement axis (purely mathematical).
- **Resolution.** (B). Glossary #5 (embedder), #6 (modality).
- **Rationale.** (A) is false in Context Graph — multiple embedders (E5 cause, E5 effect, E10 doc, E10 query) can share a modality. (C) is too abstract for reader grasp. (B) matches how ClipCannon organises its seven modalities and how Case 3 distinguishes speaker-modality (WavLM) from voice-modality (Qwen3-speaker).
- **Defuses.** FM-09; G2.

## R06 — Alignment (three-way disambiguation)
- **Live in.** Mission brief; prior §2.6; §6.9.
- **Candidate meanings.**
  - (A) RLHF/DPO scalar-reward alignment.
  - (B) Embedding-space alignment (cross-lingual / CLIP-style).
  - (C) Geometric alignment to a constellation centroid (TCT's sense).
- **Resolution.** In this manuscript, **"alignment" means (C)** and first uses must disambiguate from (A)/(B). When we discuss the broader literature, always qualify: "RLHF alignment", "scalar-reward alignment". Glossary #13.
- **Rationale.** Bare "alignment" is the most-loaded word in ML-safety vocabulary right now. Readers' default parse is (A). If we don't preempt that read in §1 and §2.6, §6.9's comparison table is confusing.
- **Defuses.** FM-01 (marketing); FM-09; G2.

## R07 — Identity-Locked Generation vs. Personalised vs. Subject-Specific
- **Live in.** Working title; mission brief; §1; §6.
- **Candidate meanings.**
  - (A) Personalisation — soft preference nudge.
  - (B) Subject-specific fine-tuning (LoRA on one person's data) — no runtime check.
  - (C) Identity-locked generation — per-output cosine-to-centroid verification at inference.
- **Resolution.** (C). Glossary #4.
- **Rationale.** Our core differentiator from e.g. personalised diffusion, DreamBooth-style identity fine-tuning, and voice-conversion is the **runtime guard**. Without the guard, we are just doing LoRA personalisation. The glossary rule "first use in §1 must distinguish from personalisation" makes this visible.
- **Defuses.** FM-01; G3 novelty delta.

## R08 — Frozen Embedder vs. Pretrained Embedder vs. Static / Immutable
- **Live in.** Throughout prior manuscript.
- **Candidate meanings.**
  - (A) Pretrained (pretrained-on-something-somewhere; may be fine-tuned during our pipeline).
  - (B) Frozen during TCT (explicitly NOT updated by our losses).
  - (C) Immutable (legal/philosophical — never changeable).
- **Resolution.** (B). Glossary #10.
- **Rationale.** Pretrained is a history claim ("before we got it"). Frozen is a behaviour claim ("we do not update it during TCT"). The distinction matters because Case 2 uses a *frozen-with-LoRA* generator — the base is frozen, LoRA is trained. Using "pretrained" there would be ambiguous; "frozen" makes the constraint visible.
- **Defuses.** G6 reproducibility completeness; FM-09.

## R09 — SECS (encoder-qualification required)
- **Live in.** SITREP; prior §9.
- **Candidate meanings.** SECS without an encoder name could mean WavLM-SV, SpeechBrain ECAPA-TDNN, Qwen3-speaker, or some other speaker encoder — and their numbers are not comparable.
- **Resolution.** Bare "SECS" is **FORBIDDEN**; always qualify: "WavLM SECS", "WavLM-large SV cosine", "SpeechBrain SECS", "Qwen3 SECS". Glossary #16.
- **Rationale.** Case 3 reports 0.961 WavLM SECS, but the same clip scores 0.975 on Qwen3 and 0.624 on SpeechBrain (§9.6 ablation). Stripping the encoder would let a reviewer misread them as commensurable.
- **Defuses.** FM-04 measured/projected conflation (via G1); G7 citation validity; G8 tier labeling.

## R10 — WavLM-SECS Same-Session Band (our empirical claim, not a universal constant)
- **Live in.** SITREP; prior §9.2, §9.7.
- **Candidate meanings.**
  - (A) The 0.95–0.99 band is a published universal reference.
  - (B) The band is this manuscript's own empirical observation on Case 3's speaker.
- **Resolution.** (B). Glossary #17.
- **Rationale.** The 0.95–0.99 range we use is computed from OUR target speaker's own recordings vs. themselves (prior §9.7 table). It is NOT a published benchmark; treating it as universal would be a silent overclaim. First use must carry the hedge "an empirical observation on Case 3's speaker, not a universal value".
- **Defuses.** FM-04; G1 provenance; G9 limitations.

## R11 — Evidence Tiers (T1–T4)
- **Live in.** P04 in agent-01's principles.
- **Candidate meanings.** Tier ordering varies across fields (T1 could mean strongest or weakest depending on convention).
- **Resolution.** T1 Measured / T2 Constructive / T3 Projected / T4 Theoretical, with T1 being the strongest empirical claim. Glossary #21.
- **Rationale.** P04 defines the tier levels; this glossary entry makes the ordering and meaning unambiguous. Every §6 subsection header carries its tier.
- **Defuses.** G1, G8.

## R12 — "Orthogonal" (approximate independence, not formal)
- **Live in.** Mission brief; prior §3.4; §3.1.
- **Candidate meanings.**
  - (A) Strict mathematical orthogonality in a shared inner-product space.
  - (B) Approximate independence / non-redundancy of information content.
- **Resolution.** (B) with an explicit hedge on first use. Glossary #14.
- **Rationale.** Embedders live in different spaces, so (A) is a category mistake. Reviewers will catch this instantly if we claim (A); hedging to (B) preempts the objection. The hedge "approximately independent; strict orthogonality is an open question (§10)" appears in §3.4 per the glossary rule.
- **Defuses.** FM-10 citation padding (forces us to cite the independence argument properly); G3 novelty delta specificity.

## R13 — Cross-Encoder (two senses)
- **Live in.** Prior §9.6.
- **Candidate meanings.**
  - (A) Cross-encoder in the IR sense (a BERT-based pair classifier that jointly encodes a query-document pair).
  - (B) In SECS evaluation: scoring with an encoder distinct from any conditioning signal during generation.
- **Resolution.** (B), explicitly distinguished from (A) on first use in §9. Glossary #20.
- **Rationale.** The IR sense is common enough that a cs.CL reviewer will default to it; without the disambiguation §9.6's "cross-encoder ablation" reads as a different experiment.
- **Defuses.** G2; G3.

## R14 — "Third Path" (around the data wall)
- **Live in.** Working title; mission brief.
- **Candidate meanings.**
  - (A) A post-scaling-law regime claim ("AGI route").
  - (B) A reframing move: path 1 = more data, path 2 = synthetic generation, path 3 = DDA decomposition of existing data.
- **Resolution.** (B). Glossary #27.
- **Rationale.** (A) is an FM-03 (AGI overclaim) trap. (B) is exactly what DDA does and is defensible with the $N + \binom{N}{2}$ count. First use in §1 names all three paths explicitly to prevent drift into (A).
- **Defuses.** FM-03; P09.

## R15 — Constellation Guard vs. Safety Classifier vs. Reward Model
- **Live in.** Prior §6.4, §6.8; Phoenix `constellation_guard.py`.
- **Candidate meanings.**
  - (A) A safety classifier (labels output as safe/unsafe).
  - (B) A reward model producing scalar preference.
  - (C) A geometric validator: re-embed → cosine → threshold → accept/reject.
- **Resolution.** (C). Glossary #12.
- **Rationale.** The guard has no learned scalar head, no label taxonomy. It only measures distance to a fixed centroid and compares to a fixed threshold. Calling it (A) invites safety-benchmark reviewer expectations we cannot meet and don't claim; calling it (B) invites RLHF comparisons we address separately in §6.9.
- **Defuses.** FM-01; FM-09; G3.

## R16 — Reference Set Size Notation
- **Live in.** Prior §9.8 ("50-clip centroid"); Case 2 "16-minute interview".
- **Candidate meanings.** Does "50-clip centroid" mean "the centroid is 50-dimensional", "the centroid is computed from 50 clips", or something else?
- **Resolution.** "n-clip reference set" describes the size of the raw input pool; "centroid computed from an n-clip reference set" describes the derivation. Never "50-clip centroid" alone. Glossary #9.
- **Rationale.** The dimensional ambiguity above has bitten at least one reader of the prior draft. Using the "reference set" framing fixes it.
- **Defuses.** G6; G2.

## R17 — Cross-Embedder Interaction Feature (the $\binom{N}{2}$ term)
- **Live in.** Prior §3.2.
- **Candidate meanings.**
  - (A) A literal cross-attention layer (transformer sense).
  - (B) An information-theoretic mutual-information estimate.
  - (C) A generic pairwise functional $\rho_{jk}$, specified per deployment (cosine, MI estimate, or learned).
- **Resolution.** (C). Glossary #15.
- **Rationale.** The DDA count doesn't depend on which $\rho_{jk}$ is used; the construction merely requires that some pairwise functional exists. (A) is wrong (we aren't computing transformer cross-attention). (B) is too specific. (C) matches §3.2 as written.
- **Defuses.** FM-02 (prevents readers from conflating with neural mechanisms); G3.

## R18 — Model Collapse (named exclusion)
- **Live in.** Prior §3.3; §10.
- **Candidate meanings.** Readers may assume DDA falls under model-collapse risks.
- **Resolution.** Model collapse is explicitly defined (Shumailov et al., 2024 sense) and explicitly excluded from DDA — §3.3 states the exclusion argument. Glossary #28.
- **Rationale.** This is the most likely adversarial-reviewer objection. Pre-empting it with the named exclusion is cleaner than reacting.
- **Defuses.** FM-02; G3.

## R19 — Flow-Matching Sign Convention
- **Live in.** Prior §6.4, §8.4; `docs/inspect/08_training_audit.md` (Santa project).
- **Candidate meanings.** In flow matching, the velocity target can be $\text{latent} - \text{noise}$ or $\text{noise} - \text{latent}$. Different libraries use different conventions; mixing them is the bug.
- **Resolution.** In EchoMimicV3 / Wan2.1-Fun: $\text{target} = \text{noise} - \text{latent}$. Glossary #30.
- **Rationale.** This specific convention is load-bearing for Case 2's forensic disclosure; generic "sign bug" is insufficient.
- **Defuses.** FM-08 (silent-failure hiding) and the P10 forensics requirement.

## R20 — "Frozen-with-LoRA" (frozen base + trainable LoRA)
- **Live in.** Case 2 (EchoMimicV3 + LoRA); Case 3 (Qwen3-TTS without fine-tune).
- **Candidate meanings.** "Frozen generator" could mean (A) the whole generator is untrained, (B) the base weights are held, with LoRA on top, (C) we only use the generator at inference.
- **Resolution.** Use the qualified forms: "frozen-with-LoRA" for Case 2's generator; "inference-only, no fine-tune" for Case 3's generator.
- **Rationale.** Bare "frozen generator" in Case 2 would be wrong (LoRA is trainable). Bare "fine-tuned generator" in Case 3 would also be wrong (Case 3 does no fine-tune).
- **Defuses.** G6 reproducibility; FM-04.

---

## Off-menu ambiguities I decided NOT to add to the glossary

- **"MOS" (Mean Opinion Score)** — Standard acoustics term; trusting readers to parse it from context in §9. No drift risk observed.
- **"LoRA"** — Standard; cite Hu et al. (2021) on first use; no glossary entry needed.
- **"DPO" / "SFT" / "RLHF"** — Standard acronyms, cited at first use.
- **"FLAME" / "FACS AU"** — Standard computer-graphics / psychology terms; first use cites Li et al. 2017, Ekman & Friesen 1978.
- **"HDC" / "BERT" / "SigLIP" / "SPLADE" / "ColBERT"** — Standard architecture names; first-use citations suffice.
- **"RocksDB" / "MCP" / "CF (column family)"** — System-engineering terms; not load-bearing for novelty claims.

These choices deliberately shrink the glossary so downstream writers don't have to memorise 60
terms. Glossary entries are reserved for terms that (a) are novel to this work OR (b) have a
known drift risk detected in the prior manuscript.

---

## Summary counts

- **Ambiguities identified:** 20 live + 6 off-menu watchlisted = 26.
- **Glossary entries locked:** 30 (some entries cover more than one resolution, e.g., constellation/centroid/reference-set are three glossary rows covering R04's four-way disambiguation).
- **Failure modes defused:** FM-01, FM-02, FM-03, FM-04, FM-08, FM-09, FM-10 (7 of 10).
- **Gates supported:** G1, G2, G3, G6, G7, G8, G9.
