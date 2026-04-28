# Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall

**Chris Royse**
*Teleox.ai*
chrisroyseai@gmail.com

*Draft v0.1 — April 16, 2026. APA 7th-edition references; all URLs verified by direct retrieval at the time of writing.*

---

## Abstract

The prevailing narrative in artificial-intelligence research is that the scaling paradigm — larger models trained on more data with more compute — is approaching a structural wall. Villalobos et al. (2024) project that the effective stock of public human-generated text (~300 trillion tokens) will be exhausted between 2026 and 2032, with overtraining regimes pulling that date forward to 2025–2027. Reporting throughout late 2024 and early 2025 (Woo & Efrati, 2024; Franzen, 2024) confirms that frontier labs have observed diminishing returns from pre-training scale alone. The industry's two dominant responses — licensing ever-scarcer human content at hundreds of millions of dollars per deal, and generating synthetic data that provably suffers *model collapse* (Shumailov et al., 2024) — both treat the problem as a volume problem.

We argue the problem is misframed. The bottleneck is not raw token volume but **meaning density**: how much independently-labeled, multi-dimensional training signal can be extracted per unit of raw data. We introduce **Derived Data Abundance (DDA)**, the principle that running a fixed corpus through *N* frozen, independently-trained embedding models produces *N* orthogonal labeled representations grounded in real observations — not generated outputs — and therefore exempt from the collapse dynamics that haunt synthetic-data pipelines. We call this operation **meaning compression**, in structural contrast to the bit compression, weight compression, and activation compression that dominate existing AI-efficiency research.

We formalize and validate the principle across two production systems we have built and one training method that emerges from them. **Context Graph** is a 370,000-line Rust semantic-memory substrate that encodes every stored text through 13 independent embedders (E1–E13) across ~11,000 dense dimensions plus two sparse 30,522-term spaces and one 128-per-token late-interaction space, persisting all vectors plus 78 pairwise synergy cross-correlations and six hierarchical group alignments into a 59-column-family RocksDB store. Every text input therefore yields ≈91 independent meaningful signals (13 per-embedder presence scores + 78 pairwise interactions) before any task-specific processing. **ClipCannon** is a 67,585-line Python video-understanding pipeline that decomposes video through a 23-stage analysis DAG into seven perceptual modalities (visual, semantic, emotional, speaker, prosodic, sentiment-level, voice-verification) spanning 4,044 embedding dimensions. From a 16-minute source interview we derive 12,000+ individually labeled training samples without any human annotation.

On top of these substrates we propose **Teleological Constellation Training (TCT)**: a three-phase method in which (i) the multi-modal decomposition of a reference corpus produces a *constellation* of frozen per-modality centroids; (ii) a generative model is fine-tuned with losses that pull toward the constellation; (iii) at inference, every output is re-embedded through the same frozen models and rejected if it falls outside a per-modality cosine threshold. Alignment is redefined as **geometric proximity to a multi-dimensional identity manifold**, replacing scalar-reward maximisation with a verifiable, per-output constraint that requires no ongoing human feedback after centroid construction.

We validate TCT through three empirical case studies.
**Case 1 (Shakespeare stylistic LoRA).** A Gemma-class base model fine-tuned via LoRA on 7,908 cleaned Shakespeare examples and further trained by DPO on 12,567 contrastive pairs, run behind a three-layer enforcement stack (LoRA-learned weights → deterministic constrained decoder → 13-embedder runtime style-centroid guard), passes 15/15 Full-State-Verification style tests including explicit prompt-injection resistance. We identify and document a root-cause training-data failure mode (prompt-echo from uncleaned stage directions) that single-embedder evaluation cannot detect.
**Case 2 (identity-locked talking-head cloning).** LoRA fine-tuning of EchoMimicV3 (Meng et al., 2025) on 2,362 curated clips from a single 16-minute interview, with a mouth-weighted flow-matching loss (10× mouth, 2× face, 1× background) and a forensically-identified flow-matching sign correction, produces visual-centroid cosine similarity ≥ 0.70 and speaker-centroid cosine similarity ≥ 0.80 across eight controllable emotional states — all derived from a single subject dataset at a size (~16 minutes) one to two orders of magnitude below what zero-shot systems use.
**Case 3 (voice cloning beyond the same-session band).** On the microsoft/wavlm-base-plus-sv cross-encoder used by ClonEval (Christop et al., 2025), our pipeline reaches **0.961 mean SECS (max 0.975)** across ten novel unseen sentences, exceeding VALL-E 2's declared "human parity" threshold (≈ 0.881; Chen et al., 2024) by +0.080 mean and crossing into the empirical same-session range we establish at ≥ 0.95. Two independent automated naturalness predictors (DNSMOS P.808 and UTMOS) score the clone identically to real recordings (3.93 vs 3.93; 3.012 vs 2.997). The result is obtained with an unmodified Qwen3-TTS 1.7B model via pipeline engineering alone: Full in-context-learning references, 50-clip centroid enrollment, best-of-*N* selection scored by the target encoder rather than a mismatched one, and a temperature sweep that identifies T = 0.3 as optimal for WavLM objective.

Across the three cases we observe a unifying pattern: once a constellation is built and used as an alignment target at every level (architecture, loss, runtime guard), small datasets produce outputs that are verifiably indistinguishable from reference on the dimensions the constellation covers. The sample complexity of identity-locked generation collapses from "collect millions of subject-specific examples" to "collect one reference sample in each modality and freeze the centroids." TCT is therefore both a data-efficient training method and a mechanism for transforming any existing corpus into a dataset that is one to two orders of magnitude richer in labeled signal.

We close by arguing that DDA reframes the scaling debate: the data *wall* is a **data door**, because the bottleneck was never raw volume. The solution is to decompose the data we already have through more, better, and more diverse independent embedding models — each new embedder that captures a genuinely orthogonal dimension of meaning multiplies the effective training corpus by another factor, without generating a single synthetic token.

---

## Keywords

training data wall; model collapse; synthetic data; multi-modal embeddings; identity-locked generation; voice cloning; LoRA fine-tuning; flow matching; constellation alignment; meaning compression; Derived Data Abundance; Teleological Constellation Training; scaling laws; retrieval-augmented systems; Model Context Protocol

---

## Contributions

1. A formal principle — **Derived Data Abundance (DDA)** — identifying meaning density (not token volume) as the binding constraint on AI training-data scaling.
2. A positioning of DDA as a new category in AI compression — **meaning compression** — orthogonal to and more economically valuable than bit-, weight-, or activation-compression (Section 3).
3. A production 13-embedder semantic-memory substrate (**Context Graph**) that realises DDA on text and exposes the derived representations through a 75-tool Model Context Protocol server, providing a concrete, open-sourceable reference implementation (Section 4).
4. A production 7-modality video-understanding pipeline (**ClipCannon**) that extends DDA to audiovisual data via a 23-stage DAG and derives 12,000+ labeled training samples from a 16-minute input (Section 5).
5. A training method — **Teleological Constellation Training (TCT)** — that uses multi-modal centroid constellations as the alignment target at every level of a generative pipeline (Section 6).
6. Three empirical case studies with reproducible protocols and forensically-identified failure modes, including a sign-bug in the flow-matching loss formulation of a widely-used audio-driven video model that affected every prior LoRA iteration (Section 7–9).
7. A claim, supported but not proven: that TCT's per-output geometric-proximity guarantee is a **verifiable alternative to scalar reward** for the subclass of alignment problems that admit a reference embedding.

---

## Reading order

The manuscript is split into sixteen sibling Markdown files (`00_frontmatter.md` through `15_appendix_repro.md`) for editing and review. Read in numerical order. A full references list (APA 7th ed., every URL verified) is in `13_references.md`. Appendices with schemas, key formats, and reproducibility notes are in `14_` and `15_`. Section 11 (`11_leapable.md`) describes the Leapable AI-agent marketplace as a companion commercial exemplar.
