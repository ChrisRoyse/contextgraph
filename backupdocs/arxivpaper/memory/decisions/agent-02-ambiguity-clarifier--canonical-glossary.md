---
namespace: decisions
agent_id: agent-02-ambiguity-clarifier
timestamp: 2026-04-16T15:30:00Z
status: active
locked: true
supersedes: none
tags: vocabulary, glossary, terminology, arxiv, canonical-definitions
---

# Canonical Glossary — LOCKED

This file is **the authoritative vocabulary** for the arXiv manuscript. Every downstream agent
(03–48) MUST use the canonical term exactly as spelled here and MUST NOT introduce synonyms.
Violations are detected by agent-42 (adversarial-reviewer) via regex and block submission.

**Locking rule.** A term in this file can only be changed by a coordinator-signed decision
file that supersedes this one. If a writer believes a term is wrong, file a blocker at
`./memory/blockers/`, do not invent a replacement.

Format per entry:
- **Canonical spelling** (first mention in manuscript uses full form + acronym; thereafter acronym)
- **One-sentence definition** (what it IS)
- **What it is NOT** (the nearest confusable construct)
- **Usage rule** (the mandate — what downstream writers must do)

---

## Tier A — Load-Bearing Novel Constructs (P03 / G2 enforcement)

### 1. Derived Data Abundance (DDA)
- **Definition.** A data-enrichment principle stating that a fixed corpus of $n$ raw inputs projected through $N$ frozen, independently-trained embedding models yields $n(N + \binom{N}{2})$ information-theoretically-structured **supervisory signals** — $n$ per-embedder labeled vectors plus $n\binom{N}{2}$ pairwise cross-embedder interaction features — without generating any new inputs.
- **NOT.** DDA is NOT synthetic data generation, data augmentation, distillation, or self-training. DDA does NOT produce new inputs; it decomposes existing inputs along $N$ measurement axes.
- **Usage rule.** First use always: "**Derived Data Abundance (DDA)**". Thereafter: "DDA". Every first section-level mention (§1, §3, §5, §6) must be followed within two sentences by the phrase "not synthetic data" OR a pointer to §3.3.

### 2. Meaning Compression
- **Definition.** A compression category that increases the labeled-signal-per-unit-of-raw-data ratio by projecting inputs through $N$ frozen embedders, orthogonal to the three prior categories (bit-, weight-, and activation-compression).
- **NOT.** Meaning compression is NOT weight compression (GPTQ, SparseGPT, distillation), NOT activation compression (KV-cache quantisation, TurboQuant), NOT bit compression (gzip, PNG, FLAC), and NOT semantic compression in the information-retrieval sense.
- **Usage rule.** Always lower-case ("meaning compression"), never hyphenated, never "semantic compression". First use in §3.1 must state the four-way orthogonality explicitly.

### 3. Teleological Constellation Training (TCT)
- **Definition.** A training method in which a generative model is optimised to place its outputs within per-modality cosine thresholds of a frozen multi-modal centroid set (the constellation), using distance-to-centroid losses during training and per-output cosine validation at inference.
- **NOT.** TCT is NOT contrastive learning (no negative pairs are sampled from the batch), NOT metric learning (the metric is fixed by the frozen embedders, not learned), NOT prototypical-network classification (the target is generation, not classification), and NOT vanilla LoRA fine-tuning (the loss includes a geometric-proximity term, not just cross-entropy).
- **Usage rule.** First use: "**Teleological Constellation Training (TCT)**". Thereafter: "TCT". Never "TCTraining", "TC-Training", or "constellation training" alone.

### 4. Identity-Locked Generation
- **Definition.** Generation in which every output is verified at inference to fall within per-modality cosine thresholds of a specific subject's frozen centroid set.
- **NOT.** NOT personalisation (soft preference steering), NOT style transfer (transferring style without identity claim), NOT subject-specific fine-tuning alone (no runtime validator), NOT LoRA-personalised generation (same: no per-output verification).
- **Usage rule.** Always hyphenated: "identity-locked generation". Never "identity locked", "identity-preserving", or "identity-bound". First use in §1 must distinguish from personalisation.

---

## Tier B — Geometric Primitives (FM-09 enforcement)

### 5. Embedder
- **Definition.** A frozen, pretrained, independently-trained model $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$ (or to a sparse / variable-length output space) that produces a fixed-dimensional representation of an input in a specific perceptual modality.
- **NOT.** NOT "encoder" (we reserve "encoder" for the cross-encoder / speaker-encoder terminology in Case 3), NOT "backbone", NOT "feature extractor", NOT "tokenizer".
- **Usage rule.** Use "embedder" for generic references; when citing a specific one, use its name (E1, E5, SigLIP, WavLM-SV, Qwen3-speaker, etc.). Context Graph has thirteen embedders labeled **E1–E13**; ClipCannon has seven.

### 6. Modality
- **Definition.** A perceptual axis along which a subject is measured (visual, audio-speaker, audio-voice, emotion, text-semantic, text-syntactic, etc.); one modality typically corresponds to one embedder, though a single modality may be measured by multiple embedders.
- **NOT.** NOT "embedder" (a modality is the target of measurement; an embedder is the instrument), NOT "channel" (reserved for audio-stereo-channel usage elsewhere), NOT "axis" (ambiguous in geometry context).
- **Usage rule.** Always "modality" / "modalities". In tables use the named modality (Visual, Speaker, Voice, Emotion) capitalised.

### 7. Centroid
- **Definition.** The L2-normalised mean of a set of L2-normalised embeddings for a single subject in a single modality: $c_m = \mathrm{normalize}(\mathrm{mean}(\{\mathrm{normalize}(\phi_m(x_i))\}))$. One centroid per (subject, modality) pair.
- **NOT.** NOT "prototype" (FORBIDDEN in this manuscript — see vocabulary rules file), NOT "anchor" (FORBIDDEN), NOT "reference vector" (use "reference embedding"), NOT "mean vector" (insufficiently specific; we mean the normalised-mean-of-normalised).
- **Usage rule.** "Centroid" is singular per (subject, modality). Always "centroid", never "prototype"/"anchor".

### 8. Constellation
- **Definition.** The set of per-modality centroids for a single subject: $C_P = \{(m, c_m) : m \in \mathcal{M}\}$. A constellation is the multi-modal alignment target.
- **NOT.** NOT "embedding set", NOT "prototype bank", NOT "reference set" (a reference set is raw clips, not centroids), NOT "constellation embedding" (singular embedding — wrong cardinality).
- **Usage rule.** Use "constellation" for the set of centroids. Plural "constellations" is correct when discussing multiple subjects or behavioural-state variants. Never conflate with "reference set".

### 9. Reference Set
- **Definition.** The raw clips / samples of a subject in a given modality from which a centroid is computed (e.g., the 50 reference audio clips for Case 3).
- **NOT.** NOT "constellation" (a constellation is the set of centroids derived from reference sets), NOT "training set" (a reference set is specifically used for centroid construction, not for model updates), NOT "support set" (few-shot terminology we deliberately avoid).
- **Usage rule.** Always two words: "reference set". Describe size as "n-clip reference set" (e.g., "50-clip reference set").

### 10. Frozen (as modifier)
- **Definition.** A model or centroid whose parameters / values are fixed throughout training and inference and are not updated by any loss signal within the TCT pipeline.
- **NOT.** NOT "static" (too weak — a static target could still be updated out-of-band), NOT "pretrained" (pretrained is a history claim; frozen is a behaviour claim during THIS pipeline), NOT "immutable" (legal / philosophical term).
- **Usage rule.** "Frozen embedder", "frozen centroid", "frozen constellation". A model that was pretrained elsewhere and is held fixed during TCT is "frozen". A model whose backbone is held fixed but which has a trainable LoRA on top is "frozen-with-LoRA" (avoid the ambiguous "frozen base model").

---

## Tier C — Method & Training Vocabulary

### 11. Per-Modality Threshold
- **Definition.** A scalar cosine-similarity floor $\tau_m$ used at inference to decide whether a generated output is accepted in modality $m$ (accept iff $\cos(\phi_m(\text{output}), c_m) \geq \tau_m$).
- **NOT.** NOT "reward threshold" (no reward model is involved), NOT "confidence threshold" (not probabilistic).
- **Usage rule.** Always "per-modality threshold" or $\tau_m$ in equations. Report in a table per case study.

### 12. Constellation Guard
- **Definition.** A runtime validator that, for every generated output, re-embeds the output through the same frozen embedders used to build the constellation, computes per-modality cosine similarities to the centroids, and accepts or rejects based on per-modality thresholds.
- **NOT.** NOT "safety classifier" (classifiers produce labels; the guard produces a distance-to-centroid boolean), NOT "reward model" (no scalar preference).
- **Usage rule.** "Constellation guard" when referring to the concept; "ConstellationGuard" (CamelCase) when referring to the Phoenix / Context Graph code artifact. Prefer the concept form in the manuscript body.

### 13. Alignment (disambiguated)
- **Definition (in this manuscript).** Proximity of generated outputs to a frozen centroid in embedding space, as measured by per-modality cosine similarity. "Alignment" in this paper is always **geometric alignment to a constellation**.
- **NOT.** In this manuscript, "alignment" is NOT RLHF/DPO/RLAIF scalar-reward alignment, NOT embedding-space alignment in the cross-lingual sense, NOT CLIP-style image-text contrastive alignment.
- **Usage rule.** First use in §1 and in §2.6 must explicitly distinguish geometric alignment from scalar-reward alignment. Never use bare "alignment" without the preceding disambiguation. When referring to the broader ML literature's alignment, always qualify: "RLHF alignment", "scalar-reward alignment", "preference alignment".

### 14. Orthogonal (as applied to signals)
- **Definition.** In DDA, "orthogonal" is a shorthand for "approximately independent in the information-theoretic sense" — embedders drawn from different base architectures trained on largely disjoint data for different objectives, treated as producing non-redundant representations. Strict formal orthogonality is not claimed.
- **NOT.** NOT "mutually linearly orthogonal in $\mathbb{R}^d$" (embedders live in different spaces; that statement is category-mistake).
- **Usage rule.** First use in §3.4 must include the hedge: "approximately independent; strict orthogonality is an open question (§10)". Thereafter "orthogonal signals" is allowed as shorthand. Never claim "formal orthogonality".

### 15. Cross-Embedder Interaction Feature
- **Definition.** A scalar or low-dimensional vector $\rho_{jk}(\phi_j(x), \phi_k(x))$ describing a pairwise relationship (e.g., normalised cosine, MI estimate, concatenation-followed-by-projection) between two embedders' outputs for the same input.
- **NOT.** NOT "cross-attention" (reserved for transformer-architecture usage).
- **Usage rule.** "Cross-embedder interaction feature" or "pairwise interaction feature". In DDA-count expressions use $\binom{N}{2}$ explicitly.

---

## Tier D — Evaluation Vocabulary (Case 3-specific; P04 tier labels)

### 16. SECS (Speaker Encoder Cosine Similarity)
- **Definition.** Cosine similarity between two audio recordings' embeddings produced by a **specific named speaker encoder**; the encoder must always be named (e.g., "WavLM SECS", "SpeechBrain ECAPA-TDNN SECS", "Qwen3-speaker SECS").
- **NOT.** NOT a single number without encoder context — SECS values from different encoders are not directly comparable.
- **Usage rule.** Bare "SECS" is FORBIDDEN in the manuscript body except inside a fully-qualified phrase. Canonical forms: "WavLM SECS", "WavLM-large SV cosine", "SpeechBrain SECS", "Qwen3 SECS". Every headline SECS number must name its encoder at first use in that section.

### 17. WavLM-SECS Same-Session Band
- **Definition.** The empirical range 0.95–0.99 observed when scoring one target speaker's own real recordings from the same recording session against each other; this range is the encoder's discrimination ceiling for that speaker's audio character.
- **NOT.** NOT a theoretical upper bound, NOT a universal constant across speakers, NOT a published benchmark result — it is **our own empirical measurement** on the Case 3 speaker's real recordings.
- **Usage rule.** First use must include the hedge: "same-session band (0.95–0.99) — **an empirical observation on Case 3's speaker, not a universal value**". When used as a comparator for clone SECS, say "falls within our measured same-session band" not "reaches the same-session band".

### 18. DNSMOS P.808 / P.835
- **Definition.** DNSMOS is a non-intrusive naturalness / speech-quality predictor (Reddy et al., 2022); P.808 and P.835 are two published protocols (P.808 gives a single naturalness MOS; P.835 gives SIG/BAK/OVRL).
- **NOT.** NOT a subjective listener rating, NOT a speaker-similarity metric.
- **Usage rule.** Always qualify the protocol: "DNSMOS P.808" or "DNSMOS P.835". Never bare "DNSMOS" after first use.

### 19. UTMOS
- **Definition.** A learned speech-naturalness MOS predictor (Saeki et al., 2022), winner of VoiceMOS Challenge 2022.
- **NOT.** NOT a SECS variant; UTMOS measures naturalness not speaker similarity.
- **Usage rule.** "UTMOS" with citation at first use.

### 20. Cross-Encoder (in SECS context)
- **Definition.** An evaluation paradigm in which the encoder used to score the TTS output is **different from** any encoder the TTS model was conditioned on during generation. This eliminates the "scoring with your own training signal" bias.
- **NOT.** NOT "cross-encoder" in the information-retrieval sense (BERT-based pair classifier).
- **Usage rule.** First use in §9 must explicitly distinguish from the IR sense. Preferred phrasing: "cross-encoder SECS evaluation".

### 21. Evidence Tier (T1 / T2 / T3 / T4)
- **Definition.** Per P04, every claim is labeled:
  - **T1 Measured** — numeric result in a cited report (Case 3).
  - **T2 Constructive** — system exists and runs; cite file paths / line counts (Context Graph, ClipCannon).
  - **T3 Projected** — architecturally complete, measurement pending (Case 1, Case 2).
  - **T4 Theoretical** — derivation, framework, count (DDA's $N + \binom{N}{2}$).
- **NOT.** NOT a confidence interval, NOT a p-value, NOT a quality rating.
- **Usage rule.** Every §6 subsection header carries its tier in parentheses, e.g., "§6.3 Case 3 — Voice Cloning (T1)". Writers NEVER mix tiers within a single claim sentence.

---

## Tier E — System & Case-Study Vocabulary

### 22. Context Graph
- **Definition.** The ~370K-line Rust memory system with 13 embedders (E1–E13), 59 RocksDB column families, 75 MCP tools, and 5,184 tests.
- **NOT.** NOT "contextgraph" / "ContextGraph" / "CG" in prose (only in code identifiers or file paths).
- **Usage rule.** Capitalise both words, two separate words.

### 23. ClipCannon
- **Definition.** The ~67,585-line Python DAG with 23 stages, 7 modalities × 4,044 dimensions, 58 MCP tools, producing 12,000+ labeled training samples from 16 minutes of source material.
- **NOT.** NOT "Clip Cannon" (two words) / "clipcannon" (lowercase) in prose.
- **Usage rule.** One word, internal capital: "ClipCannon".

### 24. Leapable.ai
- **Definition.** The companion commercial marketplace mentioned only in Acknowledgements and the commercial-disclosure footnote per P09 / FM-01. NOT a core contribution.
- **Usage rule.** Mention ONLY in Acknowledgements. Zero mentions in body sections.

### 25. Teleox.ai
- **Definition.** Author's primary affiliation; mentioned only in Acknowledgements and the commercial-disclosure footnote.
- **Usage rule.** Same as Leapable.ai — Acknowledgements only.

---

## Tier F — Data-Wall Framing Vocabulary

### 26. The Data Wall
- **Definition.** The scaling-law observation that high-quality human-generated training data is a structurally constrained resource (Villalobos et al., 2024; see `_evidence_01_data_wall.md`), exhaustion projected 2026–2032 by current usage rates.
- **NOT.** NOT a wall in a physical or metaphysical sense; use the phrase only in the sense of the cited literature.
- **Usage rule.** Always "the data wall" (lower-case). Reframed, not "solved" — see P09 / FM-03.

### 27. Third Path (around the data wall)
- **Definition.** The DDA route that neither acquires more inputs (first path) nor synthesises new inputs from generators (second path), but decomposes existing inputs through $N$ frozen embedders to extract additional supervisory signals.
- **NOT.** NOT a new discovery of data, NOT a route to AGI, NOT a post-scaling-law regime claim.
- **Usage rule.** First use in §1 must name all three paths explicitly. "Third path" is informal phrasing; in technical sections use "DDA".

### 28. Model Collapse (as a distinguished anti-pattern)
- **Definition.** The recursive-training failure mode of Shumailov et al. (2024): iterated training on generator-produced samples causes tails to disappear and variance to collapse.
- **NOT.** NOT applicable to DDA (DDA has no feedback loop of generator → training data).
- **Usage rule.** First use in §3.3 cites Shumailov et al. and states "DDA has no generative feedback loop; the collapse theorem is silent on DDA".

---

## Tier G — Forensic / Failure Vocabulary (P10)

### 29. Forensic Report
- **Definition.** A reproducible diagnostic document that identifies a specific bug, states the diagnostic method, the fix, and the post-fix verification result. Examples: Shakespeare echo-bug FSV log; Santa flow-matching sign-bug report (`docs/inspect/08_training_audit.md`).
- **NOT.** NOT a bug report in the GitHub-issue sense; a forensic report is complete documentation suitable for inclusion-by-reference in a paper.
- **Usage rule.** Each of the two disclosed bugs gets a compact "Forensics and corrections" paragraph in its case-study subsection with a pointer to the specific forensic report file path.

### 30. Flow-Matching Sign Convention
- **Definition.** In EchoMimicV3 / Wan2.1-Fun family: $\text{target} = \text{noise} - \text{latent}$ (NOT $\text{latent} - \text{noise}$). The sign matters; flipping it causes anti-denoising.
- **NOT.** NOT a generic sign ambiguity; this is the specific convention used by the EchoMimicV3 family.
- **Usage rule.** First use in §8.4 must state the convention explicitly and cite the forensic file.

---

## Quick-reference index of FORBIDDEN / MANDATED substitutions

| Forbidden | Use instead |
|---|---|
| prototype | centroid |
| anchor (as noun for centroid) | centroid |
| reference vector | reference embedding |
| prototype bank | constellation |
| constellation embedding (singular) | centroid |
| identity prototype | constellation centroid (or just centroid if modality clear) |
| identity-preserving | identity-locked |
| personalised generation (for our method) | identity-locked generation |
| data augmentation (for DDA) | Derived Data Abundance (DDA) |
| synthetic data (for DDA) | DDA — and explicitly distinguish from synthetic data |
| semantic compression | meaning compression |
| TCTraining / TC-Training / constellation training (alone) | Teleological Constellation Training (TCT) |
| immutable centroid | frozen centroid |
| pretrained embedder (where "frozen" is meant) | frozen embedder |
| bare SECS | WavLM SECS / SpeechBrain SECS / Qwen3 SECS (always qualify encoder) |
| same-session ceiling (as universal) | our measured same-session band |
| solved / revolutionary / breakthrough / etc. | (see P09 — deleted entirely) |
| alignment (bare) | geometric alignment to constellation (first use); alignment (later, in context) |

---

## Change log

- 2026-04-16 — Initial lock by agent-02-ambiguity-clarifier.
