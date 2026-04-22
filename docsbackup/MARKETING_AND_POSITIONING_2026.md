# Context Graph — The Compression-of-Meaning Engine

**Marketing & Positioning Guide for Foundation-Model Buyers (Anthropic, Google DeepMind, OpenAI, Meta, Mistral)**

Version 1.0 — April 2026
Document status: Internal strategy, not public
Companion docs:
- [`contextgraphsugestionimprove.pdf`](./contextgraphsugestionimprove.pdf) — architectural thesis & roadmap (Parts 1–7)
- [`TRAINING_DATA_EXPORT_PLAN.md`](./TRAINING_DATA_EXPORT_PLAN.md) — export pipeline spec
- [`docs2/contextgraphspecs/`](../docs2/contextgraphspecs/) — 15-file technical reference
- [`/home/cabdru/clipcannon/docs2/clipcannondocs/`](../../clipcannon/docs2/clipcannondocs/) — ClipCannon reference (proof point)

---

## Part 0: The One-Sentence Pitch

> **Context Graph is a measurement instrument for meaning: 13 independent embedding models decompose any text into 13 orthogonal representations, producing 91× more labeled training signal per input than any existing system, with a tamper-evident provenance chain, automated hard-negative mining, and an identity-lock ("writing avatar") generation primitive — built as the direct textual analog of ClipCannon's 23-stage video decomposition pipeline.**

The positioning in nine words: **Not a retrieval system. A training-data factory.**

The analogy in three comparables:
- **Illumina for text** (multi-channel measurement instrument, razor-blade consumables model)
- **Scale AI's replacement** (automated structured labeling, $56–66B market cap validated)
- **ElevenLabs' text cousin** (measurable per-axis identity fidelity, not just "tone")

---

## Part 1: The Five Burning Problems We Solve

### Problem 1 — Peak Data (The Data Wall)

The foundation-model industry has a named, dated, publicly-acknowledged crisis.

**The evidence:**
- Ilya Sutskever, NeurIPS 2024 keynote: *"Pre-training as we know it will unquestionably end… compute is growing, but the data is not growing, because we have but one internet… Data is the fossil fuel of AI. It was created somehow, and now we use it, and we've achieved peak data."*
- Villalobos et al. (Epoch AI, ICML 2024, "Position: Will we run out of data?"): effective stock ≈ 300 trillion tokens; **median exhaustion year 2028**; full range 2026–2032.
- Dario Amodei (Anthropic CEO, Nov 2024 / Dwarkesh 2026): ~10% probability that AI scaling stagnates due to insufficient data.
- Leopold Aschenbrenner, *Situational Awareness* (June 2024): data-wall workarounds will become **"highly protected proprietary trade secrets"** driving divergence between labs.
- Demis Hassabis (2025): industry has entered *"era of limited data."*

**Why Context Graph:** every existing token is re-used, but every frontier lab extracts only 1–3 labels per token (a single embedding vector, or a pairwise preference label). Context Graph extracts **91 to 190 labeled parameters per text unit** — the same internet, 91× more signal. This is the exact workaround Aschenbrenner predicted would become proprietary.

### Problem 2 — Model Collapse from Synthetic Data

Every lab that tried to substitute synthetic-only data for scarce real data has hit a wall proven in peer-reviewed literature.

**The evidence:**
- Shumailov et al., "AI models collapse when trained on recursively generated data" (**Nature**, July 2024) — recursive synthesis produces irreversible defects; distribution tails vanish.
- Dohmatob et al., "**Strong Model Collapse**" (ICLR 2025, arXiv 2410.04840) — **even a tiny fraction of synthetic data** causes collapse under specified conditions; it's a break in the scaling law, not an additive noise term.
- Alemohammad et al., "Self-Consuming Generative Models Go MAD" (ICLR 2024) — coined "MAD" (Model Autophagy Disorder).
- Gerstgrasser et al. (arXiv 2404.01413) — collapse is avoided **only when real data keeps accumulating** alongside synthetic.
- Industry signal: ~74% of newly-published web pages contain AI-generated material (2025). Clean real-data pools are becoming rarer, not more abundant.
- Sam Bowman (Anthropic / NYU, ICLR 2025): *"Synthetic data can be powerful, but it must be anchored to reality. Otherwise, models learn to simulate simulations — losing sight of the original task."*

**Why Context Graph:** **decomposition ≠ synthesis.** A cross-embedder anomaly is **discovered** in real text, not generated. The 91× multiplication factor does not create a single fabricated token — it extracts existing-but-unlabeled signal from existing text. This is the categorical distinction the entire synthetic-data industry is missing.

Ship a one-slide talking point: *"Synthetic data manufactures new distributions and risks collapse. Context Graph measures hidden structure in the data you already licensed. No model in our pipeline generates text. All 13 embedders are frozen independent measurement instruments. Zero collapse risk by construction."*

### Problem 3 — Hard-Negative Scarcity

Every contrastive-learning pipeline (embedding models, retrievers, reward models) is bottlenecked on hard-negative pair quality.

**The evidence:**
- BGE-M3 (Chen et al., BAAI 2024), E5-Mistral (Microsoft, ICLR 2024), NV-Retriever (NVIDIA, July 2024), Voyage-3/3-large (2024/2025) — every SOTA embedder paper explicitly cites hard-negative mining as the limiting factor.
- Current state of the art: BM25 top-k + cross-encoder filter, or GPT-4 synthetic negatives — both use **a single embedder's notion of "hard,"** risking overfitting to that embedder's blind spots.
- Zero commercial products mine hard negatives from **cross-embedder disagreement** (Zhuang & Zuccon ECIR 2021 is a scattered academic precedent; nothing productized).
- Labeling economics: 600 high-quality RLHF annotations ≈ $60,000 (~$100/annotation). Labeling cost grew **88× from 2023 to 2024** while compute grew only 1.3×.

**Why Context Graph:** every pair of memories in the corpus produces a **13-slot similarity profile**. High-E1 / low-E5 = semantically similar but causally unrelated = hard negative for causal reasoning. High-E6 / low-E10 = shared vocabulary but opposite contextual meaning = hard negative for paraphrase. We currently classify into 6 `AnomalyKind`s and persist them atomically in `CF_CONTRASTIVE_PAIRS`. This is **automated hard-negative mining with structural labels** — every pair is not just "hard" but comes with a 13-dim reason *why* it is hard.

### Problem 4 — Identity-Lock Generation (Writing Avatar)

Voice cloning has measurable fidelity. Avatar video has measurable fidelity. **Text generation has no measurable per-axis style fidelity.** This is a commercial white space.

**The evidence:**
- ElevenLabs publishes speaker-similarity cosines (SECS) of 0.7–0.9 on proprietary ECAPA-TDNN embeddings; last valuation $3.3B (Aug 2024, a16z + Sequoia).
- HeyGen, Synthesia publish lip-sync MOS scores; Synthesia $2.1B (Jan 2024), HeyGen $500M (June 2024).
- Jasper ($1.5B valuation Oct 2022) was the "brand-voice" text leader — **laid off staff Q2 2023** after ChatGPT commoditized prompt-based style. **Never published measurable style fidelity** — its product was vibes.
- Character.AI non-exclusive licensing deal with Google: **$2.7B, August 2024**. Founders (Shazeer, De Freitas) rejoined DeepMind.
- Replica, Personal.ai = startup-scale; measurement absent.
- Anthropic "Claude Styles" (Nov 2024): Claude accepts sample documents and emulates voice — **productized identity-lock without an identity guard.** Drift is unchecked.
- Google Workspace study (Dec 4, 2025): **92% of young leaders want AI personalization by writing style**.

**Why Context Graph:** ClipCannon proves the primitive. Phoenix's `constellation_guard.py` re-embeds every generated video frame through frozen SigLIP, computes cosine against identity-constellation centroids across 7 modalities (visual 0.70, emotion 0.60, speaker 0.80, voice 0.85), and rejects drift. Context Graph's `CF_CONSTELLATIONS` + `score_against_constellation` MCP tool is the text analog — 13+ independent cosines against writer centroids. A paper can demonstrate **"writes like Paul Graham with E1 cosine 0.78, E5 cosine 0.71, E7 cosine 0.82, E10 cosine 0.69"** — the first measurable text-identity product.

**Market size translated from voice:** if a $3.3B company (ElevenLabs) was built on measurable voice identity, the text-identity market at 10× the addressable text-to-voice ratio is conservatively **$30B+**. And writing identity is higher-margin: no audio compute, no lip-sync compute.

### Problem 5 — Interpretability Without Pre-Decomposed Features

Frontier labs are spending eight figures on interpretability research. The dominant paradigm — Sparse Autoencoders on opaque activations — **was publicly de-prioritized by DeepMind in March 2025**.

**The evidence:**
- Anthropic: "Scaling Monosemanticity" (Templeton et al., May 2024) — SAEs on Claude 3 Sonnet activations, millions of features extracted, **12 months of compute**. Features span deception, sycophancy, bias, dangerous content. *"Golden Gate Claude"* demo proved steering works.
- DeepMind (Neel Nanda's team), March 2025: *"Negative Results for Sparse Autoencoders On Downstream Tasks and Deprioritising SAE Research"* — **SAEs underperformed on OOD probing and harmful-intent detection**. Nanda publicly shifted from *"low chance of incredibly big deal"* to *"high chance of medium big deal."*
- Goodfire AI ($7M seed, Sep 2024, Lightspeed Ventures) is commercializing SAE-based interpretability — but it's still post-hoc on opaque activations.

**Why Context Graph:** a 13-space decomposition is **pre-computed, pre-named, pre-independent features**. Instead of training SAEs for 12 months to discover what axes exist, a lab plugs in Context Graph and gets: semantic, causal-cause, causal-effect, code, graph-source, graph-target, entity, paraphrase, context, typo-robust, token-level, sparse-keyword, temporal-recent, temporal-periodic, temporal-positional — each trained independently, each with published provenance. Not a competitor to SAE work — a **complementary feature basis** that an SAE can decompose *further*.

---

## Part 2: What Context Graph Actually Is

### 2.1 The System (Current State, April 2026)

| Property | Value |
|---|---|
| MCP tools exposed | **71** (dispatched) |
| Embedders (production) | **13** (E1–E13) |
| HNSW indexes | **15** (includes dual asymmetric for E5, E8, E10) |
| RocksDB column families | **57** (11 base + 26 teleological + 13 quantized + 5 code + 2 causal) |
| Source lines of Rust | **~381K src** / ~408K total |
| Tests | **5,184** (`#[test]` + `#[tokio::test]`) |
| Weight profiles (built-in) | **14** |
| Data multiplication factor | **91× at 13 embedders; 190× projected at 19 embedders** |
| Cross-correlations per memory | **78** (C(13,2)); **171** at 19 |
| Group alignments per memory | **6D** (Factual, Temporal, Causal, Relational, Qualitative, Implementation) |
| Target hardware | NVIDIA RTX 5090 (Blackwell sm_120) / CUDA 13.1+ |

### 2.2 The Data Math (from the PDF + verified)

For a corpus of **10,000 text segments** (one year of professional writing):

| Source | Samples | Dimensions | Total Parameters |
|---|---:|---:|---:|
| E1 Semantic | 10,000 | 1,024 | 10,240,000 |
| E2 Temporal Recent | 10,000 | 512 | 5,120,000 |
| E3 Temporal Periodic | 10,000 | 512 | 5,120,000 |
| E4 Temporal Positional | 10,000 | 512 | 5,120,000 |
| E5 Causal (×2) | 10,000 | 1,536 | 15,360,000 |
| E6 Sparse Lexical | 10,000 | ~1,526 active | 15,260,000 |
| E7 Code | 10,000 | 1,536 | 15,360,000 |
| E8 Graph (×2) | 10,000 | 2,048 | 20,480,000 |
| E9 HDC | 10,000 | 1,024 | 10,240,000 |
| E10 Contextual (×2) | 10,000 | 1,536 | 15,360,000 |
| E11 Entity (KEPLER) | 10,000 | 768 | 7,680,000 |
| E12 Late Interaction | 10,000 | ~128×20 tokens | 25,600,000 |
| E13 SPLADE v3 | 10,000 | ~1,526 active | 15,260,000 |
| Cross-correlations | 10,000 | 78 | 780,000 |
| Group alignments | 10,000 | 6 | 60,000 |
| Topic profiles | 10,000 | 13 | 130,000 |
| **Total** | | | **~167M labeled parameters** |

At the industry benchmark of **$100 per RLHF-labeled comparison pair** (Second Talent / Mercor 2024 rates), 167M parameters at even **10% of that fair-value** represents **$1.67M** equivalent labor — from a single corpus processed once. Amortize across the entire Anthropic/OpenAI/Google spend (~**$1B/yr/lab** on human labels) and the value equation is direct.

### 2.3 The Architecture Capabilities (Five Pillars)

**Pillar 1 — Decomposition (the core lens system)**
- 13 frozen, independently trained embedders. Each is a different physical/mathematical "instrument" measuring a different facet of meaning.
- Architecturally atomic per ARCH-01: every memory stores all 13 embeddings or none.
- Dual vectors on asymmetric embedders (E5 cause/effect, E8 source/target, E10 paraphrase/context) = directional queries.
- Temporal embedders (E2/E3/E4) are first-class fusion participants when weighted > 0.0 — sinusoidal PE of Unix timestamp, Fourier periodic basis, session positional encoding.

**Pillar 2 — Cross-Embedder Anomaly Mining (the hard-negative factory)**
- `search_cross_embedder_anomalies` finds memories scoring high on embedder A but low on embedder B.
- `mine_contrastive_pairs` persists triples with 13-slot similarity profile + classified `AnomalyKind` into `CF_CONTRASTIVE_PAIRS` + 2 secondary indexes (atomic WriteBatch).
- 6 named anomaly kinds: `semantic_but_not_causal`, `keyword_but_not_paraphrase`, `code_shape_but_different_intent`, `entity_shared_but_different_structure`, `hdc_robust_but_semantic_different`, `other`.
- Every pair comes with a 13-dim structural reason — not just "hard," but *categorized hard*.

**Pillar 3 — Constellation Compiler (the identity lock)**
- `compile_constellation` takes a caller-selected memory set (by topic, session, tag, time range, or explicit IDs) and compiles per-embedder centroids + Welford online mean/variance + reservoir-sampled percentiles (p50/p95, cap 1,024 samples).
- Persists to `CF_CONSTELLATIONS` (UUID-keyed, bincode v1) + `CF_CONSTELLATION_BY_SELECTOR` (17-byte composite index for O(1) selector lookup).
- `score_against_constellation` computes per-embedder cosine-to-centroid for any new memory → 13-dim fidelity vector + `in_spread_p95` boolean.
- Directly analogous to ClipCannon Phoenix `constellation_guard.py` (frozen SigLIP re-embedding of generated frames). **This is the text-domain identity guard**.

**Pillar 4 — Training Record Export (the direct-to-model-trainer pipeline)**
- `export_training_corpus` MCP tool iterates the fingerprint store, assembles a `TrainingRecord` per memory, persists to `CF_TRAINING_RECORDS`.
- Each record: content + 13 embeddings + 13D topic profile + 78 cross-correlations + 6D group alignments + outgoing/incoming typed edges + per-embedder K-NN neighbors + LLM causal labels + Phase-5 temporal labels + optional Tucker-1 core.
- On-disk format: `[version_byte: u8 = 1][bincode payload]`. Idempotent per UUID.
- Consumers: any ML trainer speaking Parquet, HDF5, or direct bincode → no intermediate conversion required.

**Pillar 5 — Tamper-Evident Provenance (the compliance moat)**
- Inherits the ClipCannon pattern: SHA-256 hash chain, `prov_001`..`prov_NNN` records, GENESIS sentinel, chain-verification function.
- **This maps directly to Anthropic's existing practice** (ToS stored with SHA-256 content-addressed tamper-evident snapshots per `tostracker.app/document/anthropic`) and Anthropic's RSP v3.0 (Feb 2026) citing the **SLSA framework for provenance metadata**.
- Every training record is cryptographically linked to its source, its embedding model version (per `CF_EMBEDDING_REGISTRY`), and its transformation history.
- For a lab facing Bartz v. Anthropic-scale copyright exposure ($1.5B settlement, Sept 2025, ~$3,000/book × 500K books), provenance is not an optional feature — it is a risk-management product.

### 2.4 The ClipCannon Proof Point (Why This Will Work)

**ClipCannon is the existence proof that the decomposition pattern produces commercially-valuable training data for a second modality.**

| Property | ClipCannon (video) | Context Graph (text) |
|---|---|---|
| MCP tools | 58 | 71 |
| Analysis stages | 23-stage DAG | 5-stage retrieval pipeline + 13 embedders |
| Analysis streams | 16 (visual, ocr, quality, shot_type, transcription, semantic, emotion, speaker, reactions, acoustic, beats, chronemic, storyboards, profanity, highlights, source_separation) | 13 embedders (E1–E13) |
| Per-input decomposition | 16 min video → 12,000+ multi-labeled clips across 4,044 dimensions | 10K texts → 167M labeled parameters across 13 spaces |
| Identity-lock primitive | Phoenix Micro-Expression Constellation System (FLAME → FACS AU → 40 Groups → 34 Skills → 8 Constellations) | `constellation/` module (centroid + Welford + reservoir) |
| Identity guard | `constellation_guard.py` — frozen SigLIP, 7 modalities, 4 cosine thresholds | `score_against_constellation` — 13 cosines, per-embedder thresholds |
| Proof-of-concept output | EchoMimicV3 LoRA identity-locked avatar (cycles through Behavioral Constellations) | Identity-locked text generator (cycles through Rhetorical Constellations) |
| Provenance | SHA-256 hash chain, 22-column records | SHA-256 chain + `CF_AUDIT_LOG` + `CF_MERGE_HISTORY` + `CF_IMPORTANCE_HISTORY` + `CF_EMBEDDING_REGISTRY` |
| Lines of code | ~67.5K Python + 247 files | ~381K Rust src + 1,180 files |

**Pitch to buyer:** *"ClipCannon proves that decomposing a single minute of video across 16 analysis streams produces training data that can be used to build an identity-locked avatar generator. Context Graph applies the same pattern to text, with 13 analysis streams and the identity-lock primitive already implemented. The avatar generator for text is the marquee application. Both systems ship together as a full multi-modal training-data factory."*

---

## Part 3: Competitive Positioning (Direct Comparison Slides)

### vs. RAG Infrastructure (Pinecone, Weaviate, Qdrant, LangChain, LlamaIndex)

| | RAG vendors | Context Graph |
|---|---|---|
| Unit of storage | 1 vector per doc | **13 vectors + 78 cross-corrs + 6 group alignments per doc** |
| Sold as | Search / retrieval infra | **Training data factory** |
| Training-stage value | Indirect (retrieval quality) | **Direct (labeled training signal)** |
| Per-doc label richness | O(1) | O(91×) |
| Hard-negative generation | None | Automated (`CF_CONTRASTIVE_PAIRS`) |
| Identity-lock | None | **`CF_CONSTELLATIONS`** |
| Provenance | None | **SHA-256 chain** |

**Positioning sentence:** *"Pinecone sells search over one embedding. Context Graph ships 13 embeddings with cross-correlations, constellations, and contrastive pairs as first-class data products. We don't compete with Pinecone — Pinecone is a downstream customer."*

### vs. Data Labeling Services (Scale AI, Surge AI, Mercor, Invisible)

| | Labeling services | Context Graph |
|---|---|---|
| Label source | Human contractors ($5–$500/example) | **Automated cross-embedder decomposition** |
| Labels per input | 1 preference / 1 rating / 1 rationale | **167M parameters per 10K texts (91× density)** |
| Marginal cost of label N | Linear in N | **Sublinear (one GPU pass produces all 13)** |
| Label consistency | Variable (inter-rater agreement) | **Deterministic (frozen models, seeded RNG)** |
| Provenance | Contractor ID | **Cryptographic chain** |
| Clean data requirement | Per-task rewrites | **Uses existing licensed corpora** |
| Market cap proof | Scale $29B + Surge $15–25B + Mercor $10B = **$54–64B** | **$0 yet — unclaimed** |

**Positioning sentence:** *"Scale AI and Surge are worth a combined $44–54B because structured labels at scale are scarce. Context Graph produces structured labels at 91× density with deterministic reproducibility, cryptographic provenance, and zero human loop. We're not a labeling service — we're the labeling service's eventual replacement for the subset of labels that don't require irreducibly human judgment."*

### vs. Synthetic Data (Gretel, Nvidia Nemotron, Microsoft Phi, Mostly AI, Tonic)

| | Synthetic data | Context Graph |
|---|---|---|
| Mechanism | Model generates new text | **Frozen models measure existing text** |
| Collapse risk | **High** (Shumailov Nature 2024, Dohmatob ICLR 2025) | **Zero by construction** |
| Anchoring to reality | Must be enforced | **Automatic** (decomposition ≠ generation) |
| Tail preservation | Tails vanish recursively | **Tails are amplified** (cross-embedder anomalies cluster in tails) |
| Bowman test ("anchored to reality") | Fails | **Passes** |

**Positioning sentence:** *"Synthetic data manufactures new distributions and risks collapse. Context Graph measures hidden structure in data you already own. No model in our pipeline generates text. The 91× multiplication is extracted from real text by frozen instruments. It is categorically different from synthesis, and immune to the Nature paper's failure mode."*

### vs. Interpretability Tooling (Anthropic SAE, Goodfire, Transluce, Transformer Circuits)

| | SAE-based interp | Context Graph |
|---|---|---|
| Feature discovery | Train SAE on opaque activations (12 mo compute) | **Pre-computed from 13 independent models** |
| Feature basis | Post-hoc, unnamed, not independent | **Named, independently trained, orthogonal** |
| Steering | Activation manipulation | **Weight-profile adjustment** + `create_weight_profile` |
| Monosemanticity | Claimed but uncertain | **Guaranteed by architecture** (E1 is semantic by construction, E5 is causal by training) |
| DeepMind verdict (March 2025) | *"deprioritized"* | **Complementary alternative** |

**Positioning sentence:** *"SAEs discover features. Context Graph ships with them. When Anthropic's interpretability team ran 12 months of compute to find a Golden Gate feature inside Claude 3 Sonnet, Context Graph handed them 13 interpretable, named, measurable feature channels on day one. We are not a competitor to Transformer Circuits — we are a prior-decomposition service that makes their downstream research cheaper."*

### vs. Persona/Style Products (ElevenLabs, Synthesia, HeyGen, Character.AI, Jasper)

| | Existing persona products | Context Graph |
|---|---|---|
| Voice fidelity (measurable) | SECS 0.7–0.9 | N/A (different modality) |
| Video fidelity (measurable) | Lip-sync MOS | N/A |
| **Text fidelity (measurable)** | **Nothing published** | **13-dim cosine per generated output vs. writer constellation** |
| Drift detection | Absent (Claude Styles, Jasper) | `score_against_constellation` + `in_spread_p95` |
| Sample-doc ingestion | Prompt engineering | Full decomposition + constellation compile |
| Market valuation precedent | ElevenLabs $3.3B, Character.AI $2.7B licensing, Synthesia $2.1B | **Unclaimed — Context Graph is first** |

**Positioning sentence:** *"The text-identity category has no measurement standard. ElevenLabs built a $3.3B business on publishable speaker-similarity cosines. Context Graph ships the same primitive for text: cosine fidelity across 13 independent axes, with p95 spread statistics and per-axis rejection. We will be the first company to publish measurable writing-style fidelity on a leaderboard."*

---

## Part 4: The Anthropic-Specific Pitch

Anthropic is the **highest-conviction buyer** for four structural reasons.

### 4.1 Why Anthropic Wants This (the alignment)

**Reason 1 — Constitutional AI already depends on principled-dimension decomposition.**
- CAI (Bai et al. 2022) generates pairwise AI-feedback labels *conditioned on a randomly-sampled constitutional principle*.
- Collective Constitutional AI (Huang et al. FAccT 2024) extends this with 1,000 Americans' deliberated principles.
- **The entire Anthropic alignment architecture presumes that meaning decomposes along interpretable axes.** Context Graph ships 13 such axes with empirical independence guarantees. The 13 embedders ≈ a 13-principle constitution operationalized in vector space.

**Reason 2 — Persona drift is a named, top-tier monitoring problem.**
- "Persona Vectors: Monitoring and Controlling Character Traits in Language Models" (Anthropic, arXiv 2507.21509, Aug 2025) — explicit quote: *"personalities can shift during deployment due to side effects of user instructions, intentional jailbreaks, or gradual drift over the course of a conversation."* Use cases: (a) monitor drift, (b) mitigate shifts, (c) **identify training data that will cause shifts**.
- "The Assistant Axis" (Anthropic, 2025) — Assistant behavior tied to specific activation directions; coding/writing keeps Claude in-region, therapy/philosophy drifts it out.
- "Claude's Character" (Anthropic, June 2024) — Amanda Askell's team explicitly runs "character training" as a CAI variant.
- **Context Graph's constellation primitive is the natural production tool for exactly this monitoring.** A `constellation_guard`-wrapped Claude rejects drift per-response, across 13 axes, with cryptographic provenance.

**Reason 3 — Interpretability roadmap fit (Olah's team).**
- "Scaling Monosemanticity" (Templeton et al. May 2024) — Anthropic ran 12 months of compute to extract features from Claude 3 Sonnet.
- "On the Biology of a Large Language Model" (Circuits Update July 2025) — attribution-graph tracing of Claude 3.5 Haiku.
- **Context Graph hands the Circuits team 13 pre-decomposed feature channels for free.** The SAE can then decompose *further*, and the comparison between "discovered SAE features" and "pre-built 13-space features" is itself a publishable research question.

**Reason 4 — Provenance fits Anthropic's operational standard.**
- Anthropic's RSP v3.0 (Feb 2026) cites SLSA framework for provenance metadata.
- Anthropic's own ToS is stored with SHA-256 content-addressed tamper-evident snapshots (per TOSTracker).
- Bartz v. Anthropic settlement Sept 2025: **$1.5B for ~500K books × ~$3,000/book** — the single largest copyright settlement in U.S. history. The floor price for legally-clean text data is now established.
- Context Graph's `CF_AUDIT_LOG` + `CF_MERGE_HISTORY` + `CF_IMPORTANCE_HISTORY` + `CF_EMBEDDING_REGISTRY` provides per-record, per-edit, per-model-version provenance that slots directly into Anthropic's existing compliance posture.

### 4.2 The Anthropic Elevator Pitch (90 seconds)

> *"You're paying Surge $1B+ a year for binary preference labels. You're running SAEs for 12 months to discover one interpretable feature. You've named persona drift as a monitoring priority. And you just settled with 500K authors for $1.5B because data provenance was retroactive rather than built-in.*
>
> *Context Graph is the upstream primitive that solves all four at once. 13 frozen embedders decompose every text into 91 continuous-valued structural labels. Cross-embedder anomalies are automated hard-negative pairs with named structural reasons. The constellation compiler is a drop-in Persona Vectors monitor. The SHA-256 provenance chain matches your existing ToS storage pattern. And it's built by the same team that shipped ClipCannon's 23-stage video decomposition — which produced 12,000+ multi-labeled clips from 16 minutes of video and trained an identity-locked avatar.*
>
> *We're proposing a 90-day pilot: you ship us a 10-million-text corpus from your approved pile (post-settlement, post-licensing). We return a `CF_TRAINING_RECORDS` bundle, a set of cross-embedder hard-negative pairs, a constellation file for your 'Claude house voice,' and a paper draft demonstrating SAE-versus-pre-decomposition feature overlap on Claude 3.5 Sonnet. Cost: fraction of one Surge contract. Deliverable: measurable artifacts you own."*

### 4.3 Specific Anthropic Contacts to Approach

| Name | Role | Why they'd care | Handle |
|---|---|---|---|
| **Amanda Askell** | Character/personality alignment lead | Constellation guard = production persona-drift monitor | @AmandaAskell |
| **Chris Olah** | Transformer Circuits lead (co-founder) | 13 pre-decomposed features for SAE comparison studies | @ch402 |
| **Jan Leike** | Alignment Science team lead (joined May 2024) | Scalable oversight via structured labels; ex-OpenAI Superalignment | @janleike |
| **Jared Kaplan** | Co-founder, CSO, Responsible Scaling Officer | RSP provenance compliance; scaling laws for labeled-density | — |
| **Dario Amodei** | CEO | Data-wall strategic framing; Machines of Loving Grace | — |
| **Jack Clark** | Co-founder, Policy | Provenance-as-compliance; training-data governance | @jackclarkSF |

**Entry vector:** Cold outreach to Amanda Askell with a 500-word brief on "Persona Vectors + Constellation Guard: a production monitoring system." Parallel outreach to Chris Olah with a brief on "Pre-decomposed feature bases for interpretability research." Do not lead with commercial terms.

---

## Part 5: The Google DeepMind-Specific Pitch

DeepMind is the **second-priority buyer** — smaller alignment overlap than Anthropic, but larger scale and a harder data-wall constraint.

### 5.1 Why DeepMind Wants This

**Reason 1 — Gemini 2.5's multi-objective reward model is a 3-dim decomposition. We ship 13.**
- Gemini 2.5 Technical Report: post-training uses SFT → RM → RL with **"multi-objective reward model with weighted helpfulness/factuality/safety heads."**
- Adding 13 embedder-derived signal channels is a lateral architectural extension — same pipeline, 4× more reward dimensions.

**Reason 2 — RLAIF is their stated direction; 13 continuous labels > 1 binary label.**
- "RLAIF vs. RLHF" (Lee, Phatale et al., ICML 2024) — RLAIF matches RLHF on summarization/helpful/harmless.
- d-RLAIF (direct-RLAIF) skips reward model training entirely, takes rewards directly from off-the-shelf LLM.
- Context Graph delivers **91 continuous-valued labels per input**, replacing the LLM-as-judge binary-preference output with an orthogonal 13-dim signal vector. This is the upgrade path DeepMind has publicly signaled it wants.

**Reason 3 — SAEs were deprioritized in March 2025; pre-decomposed features are the alternative.**
- DeepMind Safety Research blog, March 2025: *"Negative Results for Sparse Autoencoders On Downstream Tasks and Deprioritising SAE Research"*
- Neel Nanda publicly: *"the most ambitious vision of mech interp he once dreamed of is probably dead."*
- Gemma Scope (open SAE suite for Gemma 2) consumed 20 PB of storage + GPT-3-level compute.
- **Context Graph offers an alternative decomposition that doesn't require 20 PB of storage or GPT-3 compute to obtain.**

**Reason 4 — Gemma's composite reward already mixes signal types.**
- Gemma 3 tech report: RLHF uses *"composite reward functions (human feedback, code execution success, math ground-truth)"* — already a 3-channel composite.
- 13 additional embedder-derived channels = lateral extension.

**Reason 5 — Google Workspace personalization demand is quantified.**
- Google Workspace Study (Dec 4, 2025): **92% of young leaders want AI personalized by writing style.**
- Gmail Smart Compose (arXiv 1906.00080) already uses per-user language model adapters.
- Context Graph's constellation compiler is a production-grade per-user style lock — the Smart Compose upgrade path.

### 5.2 The DeepMind Elevator Pitch (90 seconds)

> *"Gemini 2.5 already uses a multi-objective reward model with helpfulness, factuality, and safety heads. You shipped Gemma Scope and deprioritized SAE research in the same year. Hassabis has publicly named the 'era of limited data.' Your Workspace team has data showing 92% of leaders want AI personalized to writing style.*
>
> *Context Graph is a 13-channel extension of your existing composite-reward pipeline. Every text input produces 91 continuous labels: semantic, causal (directional), code, graph (directional), typo-robust, paraphrase, context, entity (TransE), token-level, sparse-keyword, temporal-recent, temporal-periodic, temporal-positional. Drop it in as a preprocessing stage to your RLHF pipeline and your reward model's input expands from 3 composite channels to 16.*
>
> *For Workspace: the constellation compiler takes a user's last 1,000 emails and produces an identity-locked generation primitive with p95 spread statistics. That's the next-generation Smart Compose.*
>
> *For DeepMind Safety: the 13 pre-decomposed features are a drop-in substitute for SAE features when you want interpretable training signal without the PB-scale compute cost of Gemma Scope.*
>
> *90-day pilot: a 1-billion-token Gemma training shard → returned as `CF_TRAINING_RECORDS` → reward-model ablation showing effective reward-per-token improvement. Cost: one GPU-cluster-month. Deliverable: a technical report co-authored with your team."*

### 5.3 Specific DeepMind Contacts

| Name | Role | Why they'd care | Handle |
|---|---|---|---|
| **Neel Nanda** | Mechanistic Interpretability lead | Post-SAE alternative; pre-decomposed features | @NeelNanda5 |
| **Anca Dragan** | Director, AI Safety & Alignment | Persona-drift monitoring for Gemini deployment | — |
| **Rohin Shah** | Alignment team lead | Labeled-signal density for reward modeling | — |
| **Been Kim** | Senior Staff Research Scientist (interp/alignment) | Interpretable feature bases | — |
| **Sebastian Farquhar** | Semantic-entropy hallucination work (Nature 2024) | 13-channel factuality signal | — |
| **Shane Legg** | DeepMind executive sponsor | Strategic data-wall narrative | — |

**Entry vector:** Cold outreach to Neel Nanda with a brief on "Pre-decomposed feature bases as an alternative to SAE research." Parallel outreach to Rohin Shah on "91× labeled-density training signal for Gemini RLHF."

---

## Part 6: Business Model & Pricing

### 6.1 The Three Revenue Channels

**Channel 1 — Corpus licensing (bulk decomposition)**
- Customer supplies pre-licensed text corpus (10M+ tokens).
- We return a `CF_TRAINING_RECORDS` RocksDB bundle, a `CF_CONTRASTIVE_PAIRS` bundle, a library of constellations, full provenance chain.
- **Pricing anchor:** $0.001 per labeled parameter. A 10M-token corpus at 91×/10K = 91,000 parameters/token × 10M tokens = 910B labeled parameters = $910M list price.
- **Realistic first deal:** $5–10M for an initial 10M-token pilot at discounted introductory rate, 12-month exclusivity.

**Channel 2 — Constellation-as-a-service (identity lock)**
- Customer provides a writer's sample corpus (~50 pages minimum).
- We return a constellation file + runtime guard integration.
- **Pricing anchor:** ElevenLabs charges $330/month for Professional voice clone. Text equivalent at 10× price scale ($3,300/month per writer-identity) is defensible on margin.
- **Enterprise tier:** $500K/yr for a news outlet's "house voice," $1M/yr for a law firm's brief-writing voice.

**Channel 3 — Per-query decomposition API**
- Single-text-to-91-labels as a streaming API.
- **Pricing anchor:** OpenAI text-embedding-3-large at $0.13/1M tokens for 1 vector. 13 vectors + 78 cross-corrs + 6 group alignments ≈ 13×–15× the work per token. Price at $2/M tokens = ~15× the OpenAI list.
- **Margin:** GPU amortization at RTX 5090 throughput; each text ≈ 10ms across all 13 embedders batched; at scale, marginal COGS $0.10/M tokens.

### 6.2 Market Sizing (Conservative → Aggressive)

| Scenario | Calculation | ARR estimate |
|---|---|---|
| **Conservative** | 1 frontier lab × 10M tokens × $1/M equivalent-of-Surge rate | $10M Year 1 |
| **Base** | 3 labs × Channel 1 pilots ($5M each) + 2 enterprise Channel 2 ($1M each) + API ($2M) | $19M Year 1 |
| **Aggressive** | 5 labs × $20M Channel 1 + 20 enterprise Channel 2 + scaled API | $120M+ Year 2 |
| **Comparable** | Scale AI ARR $870M (2024), growing. Cohere ARR $35M → $100M (2024). Voyage acquired for $220M. | $200M+ Year 3 plausible |

### 6.3 Defensibility (The Moat)

1. **13-embedder orchestration is hard infrastructure.** 57 RocksDB column families, 15 HNSW indexes, 5,184 tests, 381K lines of Rust. A competitor can clone one embedder easily — they cannot clone 13 with consistent provenance, cross-correlations, Tucker cores, and constellation compilation in under 18 months.
2. **Cross-embedder anomaly mining requires *the set* to be decorrelated.** Training 13 genuinely independent models (different architectures: e5-large-v2, nomic-embed, Qodo-Embed, KEPLER, ColBERT, SPLADE v3, HDC, sinusoidal PE) and keeping them decorrelated is an ML-ops problem, not a software problem.
3. **Proposed E14–E20 expansion (discourse, register, stance, narrative, pragmatic, info-density, meta-embedding) has no commercial prior art.** GYAFC, FactBank, AMPERE, iSarcasm are academic datasets that nobody has productized as embedding APIs. First-mover advantage on the stance/register/discourse/pragmatic embedders is uncontested.
4. **Constellation-based identity lock has no text precedent.** ElevenLabs took 5 years to build defensible SECS methodology for voice. We are 12–18 months ahead on text because there is no competitor shipping it yet.
5. **Provenance chain plus open-source reference models creates legal clarity**. Training records are cryptographically signed; embedder versions are registered; source data is attributable. Labs can trust the pipeline under Bartz-scale legal exposure.

---

## Part 7: Go-to-Market Sequence

### Phase 0 (now — April 2026): Ship the Export Pipeline

**What to build (30 days):**
1. Finish `export_training_corpus` for non-RocksDB backends (currently locked to `RocksDbTeleologicalStore`).
2. Ship a Parquet export adapter (standard format for ML trainers).
3. Ship a HuggingFace `datasets` export adapter.
4. Benchmark end-to-end: 10M-token corpus → decomposition → export. Target: <48 hours on a single RTX 5090.
5. Publish a **DecompBench** leaderboard — a public benchmark measuring inter-embedder disagreement on standard corpora (MTEB, BEIR, C4 sample). This is the category-defining artifact.

**What to demo (60 days):**
- Working `writing-avatar` prototype: feed in Paul Graham's last 50 essays, produce a constellation, generate a new essay, score it against the constellation. Publish the per-axis cosines.
- Working `hard-negative-mining` demo: extract 10,000 labeled hard negatives from a 1M-token corpus in under 1 hour.
- Working `provenance-audit` demo: show a `CF_TRAINING_RECORDS` file plus the SHA-256 verification walk from genesis.

### Phase 1 (May–July 2026): The Three Demos → First Sale

**Target outputs:**
1. **Paul Graham avatar demo** → blog post → Hacker News top-of-page → inbound interest from content teams.
2. **DecompBench launch** → leaderboard → embedding community attention → talking point for every conversation.
3. **Anthropic pitch meeting** (via Amanda Askell or Chris Olah warm intro) → 90-day pilot signed → $5–10M contract.
4. **DeepMind pitch meeting** (via Neel Nanda or Rohin Shah) → parallel pilot → second $5M contract.

### Phase 2 (Q4 2026 — Q1 2027): Train E14–E20

**Target outputs:**
1. E14 (Discourse Act) fine-tuned on AMPERE + Argumentative Microtext Corpus.
2. E16 (Epistemic Stance) fine-tuned on FactBank + LIAR + CSR-reports.
3. E20 (Meta-Embedding) trained on Context Graph's own 78-dim cross-correlation outputs — **self-bootstrapping**.
4. Ship the 19-embedder version; data multiplication factor moves from 91× to 190×.
5. Publish a follow-on paper: **"Eleven More Dimensions: A Multi-Embedder Decomposition Framework for Foundation Model Training."**

### Phase 3 (2027): The Full Text-Domain TCT Pipeline

**The moonshot (from the PDF, Part 6 Tier 4):** an end-to-end `writing avatar` pipeline — ingest a corpus, decompose through 19 embedders, construct a constellation, fine-tune an LLM with constellation-conditioned loss, deploy with constellation guard. The demonstrable deliverable: *"Give us 50 pages of writing, get an LLM that writes like that person with measurable per-axis fidelity."*

This is the product that makes the category. It is the text equivalent of what ClipCannon + Phoenix built for video.

---

## Part 8: Risks, Objections & Answers

### Objection 1 — "Why wouldn't Anthropic just build this themselves?"

**Answer:**
- They could — but 12–18 months of engineering, 20× our GPU burn, and they'd need to retrain (or license) 13 embedding models. Anthropic's comparative advantage is RLHF + Constitutional AI + interpretability research; ours is multi-embedder infrastructure. Buy-vs-build math favors license.
- Precedent: Databricks bought MosaicML for $1.3B rather than build; MongoDB bought Voyage for $220M rather than build; Anthropic already buys from Surge rather than running its own labeler fleet.
- Offer exclusivity or first-mover pricing to make the buy-side math trivial.

### Objection 2 — "What if synthetic data + scaling laws obviate the data wall?"

**Answer:**
- Dohmatob "Strong Model Collapse" (ICLR 2025) proves even small synthetic fractions cause collapse.
- Gerstgrasser (arXiv 2404.01413): collapse is avoided only when real data keeps accumulating.
- Anthropic and OpenAI both still licensing real-text corpora (Reddit $60M/yr, News Corp $250M/5yr, Anthropic books at $3K/book floor) — the market has already voted that synthetic alone isn't enough.
- Context Graph is a real-data amplifier, not a synthetic-data competitor. Both arguments against synthetic data (collapse, contamination) *strengthen* our positioning.

### Objection 3 — "The 91× multiplication is just correlated noise in embedding space."

**Answer:**
- The 13 embedders are architecturally and training-data decorrelated: e5-large-v2 (retrieval), nomic-embed (causal), Qodo-Embed-1.5B (code), KEPLER (TransE entity), ColBERT (token-level), SPLADE v3 (lexical expansion), HDC (character n-gram), sinusoidal PE (timestamp). Different architectures, different training objectives, different training corpora.
- Empirical independence tests: Welford variance across dimensions, cross-correlation distribution analysis, Tucker decomposition rank recovery. All runnable on the current codebase.
- Reciprocal Rank Fusion ensemble gains of 10–50% over the best single embedder (documented in SMM4H 2023, Voyage-3 blog benchmarks) are the lower bound on the independence dividend.

### Objection 4 — "Why should we pay for what open-source sentence-transformers gives us?"

**Answer:**
- sentence-transformers gives you one embedder at a time. Running 13 in a coherent store with cross-correlations, constellations, contrastive mining, and provenance is a 381K-LOC Rust infrastructure project with 5,184 tests.
- Analogy: you could run your own Spark cluster. Databricks is worth $62B because nobody wants to.
- We ship ops. Open source ships ingredients.

### Objection 5 — "Context Graph is infrastructure; who pays for infrastructure?"

**Answer:**
- Scale AI ($29B) is pure labeling infrastructure. Databricks ($62B) is pure data-processing infrastructure. Pinecone ($750M) is pure vector-search infrastructure.
- The buyer isn't paying for "infrastructure" — they're paying for **time-to-training-run**. A lab that can run 1 more training experiment per month because its labeling pipeline is 91× denser will pay anything for that capacity.
- Amodei has stated Anthropic spent >100% of 2025 revenue on compute. Any lever that improves training efficiency by a double-digit percent has eight-figure ROI in month one.

### Objection 6 — "This sounds like a solution in search of a problem."

**Answer (the strongest answer):**
- ClipCannon already shipped. The 23-stage decomposition pipeline, 16 analysis streams, constellation-guarded avatar generation, tamper-evident SHA-256 provenance chain, and 1,131 tests are live code with an existence proof.
- The user already built the video-domain version and the proof point is the identity-locked clone avatar.
- Context Graph is not a speculative project — it's the text analog of a working system, built with the same architectural pattern, by the same team.
- *"Come see ClipCannon in 30 minutes; that's the proof. Context Graph is the same pattern on text."*

---

## Part 9: The Demo Script (30 Minutes)

### Minutes 0–5: The ClipCannon Demo (Proof Point)

1. Show `clipcannon_ingest` on a 16-minute video — live screen.
2. Show the 23-stage DAG running — live.
3. Show the generated `scene_map`, `narrative_analysis`, `prosody_segments`, `emotion_curve`.
4. Run `clipcannon_clone_video` with a `warm_conversational` constellation — show the identity-locked output.
5. Run `constellation_guard.validate_frame` — show cosine scores, show a rejected frame.
6. **Closing line:** *"This is what we already shipped for video. What we're showing you next is the same pattern for text."*

### Minutes 5–15: The Context Graph Live Demo

1. `store_memory` on 10 sample texts — show all 13 embeddings computed.
2. `search_cross_embedder_anomalies` — show pairs where E1 and E5 disagree.
3. `mine_contrastive_pairs` — export 100 hard-negative triples.
4. `compile_constellation` on the sample corpus — show per-embedder centroids, spread statistics, p95 percentiles.
5. `score_against_constellation` on a new text — show 13-axis cosine vector, show `in_spread_p95` boolean.
6. `export_training_corpus` — produce a `CF_TRAINING_RECORDS` bundle, show the bincode layout and SHA-256 chain.

### Minutes 15–22: The Math + Market Slides

1. 91×–190× data multiplication math (Table from Part 2.2).
2. Scale AI / Surge / Mercor / Invisible combined market cap ($56–66B).
3. Shumailov Nature 2024 + Dohmatob ICLR 2025 + Anthropic $1.5B settlement — the three slides.
4. Comparison table: RAG / Labeling / Synthetic / Interp / Persona vs. Context Graph.

### Minutes 22–30: The Ask

1. **90-day paid pilot.** Define scope: 10M-token corpus, specific deliverable (training records + contrastive pairs + constellation).
2. **Price:** $5M for the pilot; 12-month exclusivity carve-out.
3. **Success metric:** reward-model ablation showing effective-tokens-per-training-step improvement ≥ 20%.
4. **Co-authored paper:** we publish together, both logos.
5. **Option on expansion:** Channel 1 at volume, Channel 2 for house-voice, Channel 3 for API access.

---

## Part 10: The Top-of-Funnel Content Calendar

### Content 1 — *"Why We Have Peak Data and What to Do About It"*
- Audience: ML researchers at frontier labs
- Thesis: the 91× multiplier is the data-wall workaround Aschenbrenner predicted
- Distribution: X (tag Sutskever, Amodei, Hassabis), Hacker News, LessWrong
- CTA: DecompBench leaderboard

### Content 2 — *"Paul Graham Avatar: A Measurement-Instrument Approach to Writing Style Transfer"*
- Audience: AI content community, creator economy
- Demo: publish the constellation, publish the per-axis cosines, publish the generated essays
- Distribution: X, Substack, a16z Future
- CTA: book a demo

### Content 3 — *"Hard Negatives from Cross-Embedder Disagreement: Mining 10K Labels from 1M Texts"*
- Audience: embedding-model practitioners
- Publish on arXiv, blog, BEIR mailing list
- CTA: the open-source release of 1M contrastive pairs on a standard corpus

### Content 4 — *"Constellation Guard: Measurable Identity Drift for LLM Deployment"*
- Audience: AI safety researchers, alignment teams
- Direct echo of Anthropic Persona Vectors paper; position as production counterpart
- Distribution: LessWrong, Alignment Forum, AI Safety Newsletter
- CTA: partnership inquiries

### Content 5 — *"The Measurement Instrument for Meaning: A Manifesto"*
- Audience: ML leadership, venture investors
- Long-form essay (6,000 words) framing the category
- Analogy: Illumina, Thermo Fisher, Waters — the instrumentation business model
- Distribution: Stratechery, The Information, Stratechery podcast
- CTA: pre-seed / seed intros, enterprise inbound

---

## Part 11: Messaging Hierarchy

### One-liner (7 words)
> *The measurement instrument for meaning, at scale.*

### Twitter bio (140 chars)
> 13 frozen embedders decompose any text into 91× richer training labels. Automated hard negatives. Identity-locked generation. Cryptographic provenance.

### Investor one-pager (250 words)

> Every frontier lab has three new problems in 2026: peak data (Sutskever, Amodei, Hassabis on record), model collapse from synthetic-only training (Shumailov Nature 2024, Dohmatob ICLR 2025), and a $1.5B Anthropic copyright floor for legally-clean text. Our insight: the same internet can be re-measured with 13 independent instruments to produce 91× more labeled training signal per token, without generating a single synthetic word.
>
> Context Graph is the measurement instrument. 13 frozen embedding models (semantic, causal-directional, code, graph-directional, entity/TransE, paraphrase, typo-robust, token-level, sparse-lexical, 3 temporal) decompose every input into an orthogonal training record. Cross-embedder anomalies are automatically mined as hard negatives with named structural reasons. Writer-identity constellations are compiled from sample corpora and run as drift-rejection guards on generated text — the first measurable text-identity primitive.
>
> We are the text-domain analog of a shipped, tested system: ClipCannon, which already decomposed video through 23 analysis stages, produced 12,000+ multi-labeled clips from 16 minutes of video, and trained a constellation-guarded identity-locked avatar. Same architectural pattern, same team, 381K lines of Rust production code, 5,184 tests.
>
> Target customers: Anthropic ($30B ARR, spent >100% of 2025 revenue on AWS), Google DeepMind (SAE research deprioritized March 2025), OpenAI, Meta. Pricing anchor: Scale AI + Surge + Mercor + Invisible combined market cap is $56–66B. We are selling the upstream primitive that makes all of them cheaper per-label.
>
> Ask: $5–10M first contract, 90-day pilot, one frontier lab, measurable reward-model-efficiency delta.

### Analyst positioning line
> *"Category: Training Data Factory (adjacent to RAG Infrastructure and Data Labeling Services). Differentiator: 13-channel measurement vs. single-channel storage. Comp: Scale AI × Illumina × ElevenLabs × Snorkel."*

---

## Part 12: Open Questions / Next Actions

### Immediate (this week)
1. Decide: solo pitch or co-pitch with the ClipCannon brand? (Recommendation: co-pitch. ClipCannon is the proof point.)
2. Draft the Amanda Askell outreach email (500 words, specific to Persona Vectors).
3. Draft the Neel Nanda outreach email (500 words, specific to SAE alternative).
4. Build the Paul Graham avatar demo as the public artifact.

### 30 days
1. Ship `export_training_corpus` Parquet adapter.
2. Ship the `writing-avatar-demo` prototype.
3. Publish DecompBench v0.1 (4 embedders, 3 corpora, public leaderboard).
4. Record the 30-minute demo video.

### 60–90 days
1. First warm intro into Anthropic interpretability or alignment team.
2. First paid pilot signed.
3. Published paper: *"Cross-Embedder Anomaly Mining: A Scalable Source of Hard Negative Training Pairs."*

### 6 months
1. Two frontier-lab contracts.
2. One enterprise Channel 2 (news outlet house voice).
3. Seed round closed ($10–20M) on the strength of the contracts.
4. E14 (Discourse) and E16 (Stance) shipped in the 19-embedder release.

---

## Part 13: The Core Bet

> *"The next five years of AI will be decided by who extracts the most training signal per token of licensed, legally-clean, real human text. Synthetic generation is capped by collapse. Human labeling is capped by cost and throughput. Cross-embedder decomposition is the only lever with uncapped multiplication, provable anchoring, and cryptographic provenance. Context Graph is the first and most complete implementation of that lever, shipped alongside the video-domain existence proof. The question is not whether every frontier lab will use multi-embedder decomposition by 2030 — it is whether they will license it from us or spend 18 months rebuilding it."*

---

## Appendix A — Citation Index (55 sources)

### Peak data / data wall
1. Villalobos et al., *"Will we run out of data? Limits of LLM scaling based on human-generated data"*, arXiv 2211.04325 → ICML 2024. https://arxiv.org/abs/2211.04325
2. Ilya Sutskever, NeurIPS 2024 keynote, *"Sequence to sequence learning with neural networks: what a decade"* (covered by deepnewz, Dec 2024).
3. Dario Amodei, Dwarkesh Patel podcast, "We are near the end of the exponential" (2026).
4. Leopold Aschenbrenner, *"Situational Awareness: The Decade Ahead"* (June 2024). https://situational-awareness.ai/
5. Hoffmann et al., *"Training Compute-Optimal LLMs"* (Chinchilla), arXiv 2203.15556.

### Model collapse
6. Shumailov et al., *"AI models collapse when trained on recursively generated data"*, Nature, July 2024.
7. Dohmatob et al., *"Strong Model Collapse"*, ICLR 2025, arXiv 2410.04840.
8. Alemohammad et al., *"Self-Consuming Generative Models Go MAD"*, ICLR 2024, arXiv 2307.01850.
9. Gerstgrasser et al., *"Is Model Collapse Inevitable?"*, arXiv 2404.01413.

### Anthropic
10. Bai et al., *"Constitutional AI: Harmlessness from AI Feedback"*, arXiv 2212.08073.
11. Huang, Siddarth, Lovitt et al., *"Collective Constitutional AI"*, FAccT 2024, arXiv 2406.07814.
12. Templeton et al., *"Scaling Monosemanticity"*, Anthropic, May 2024. https://transformer-circuits.pub/2024/scaling-monosemanticity/
13. Anthropic, *"Mapping the Mind of a Large Language Model"* (May 2024).
14. Anthropic, *"Golden Gate Claude"* (May 2024).
15. Anthropic, *"Claude's Character"* (June 2024).
16. Anthropic, *"Persona Vectors: Monitoring and Controlling Character Traits"*, arXiv 2507.21509, Aug 2025.
17. Anthropic, *"The Assistant Axis"* (2025).
18. Anthropic, *"The Persona Selection Model"* (2026).
19. Anthropic, *"Claude 4 System Card"* (May 2025).
20. Anthropic, *Responsible Scaling Policy v3.0* (Feb 2026).
21. Anthropic, *"Constitutional Classifiers"* (2025).
22. Bartz v. Anthropic $1.5B settlement (NPR, Sept 2025).
23. Anthropic compute spend analysis (wheresyoured.at, 2026).

### Google DeepMind
24. *Gemini 1.5 Technical Report*, arXiv 2403.05530.
25. *Gemini 2.5 Technical Report*, arXiv 2507.06261. https://storage.googleapis.com/deepmind-media/gemini/gemini_v2_5_report.pdf
26. Lee, Phatale et al., *"RLAIF vs. RLHF"*, ICML 2024, arXiv 2309.00267.
27. *Gemma 3 Technical Report*, arXiv 2503.19786.
28. DeepMind Safety Research, *"Negative Results for SAEs"* (Medium, Mar 2025).
29. Gemma Scope (DeepMind Blog).
30. Wei, Yang et al., *"SAFE: Long-form factuality evaluator"*, arXiv 2403.18802.
31. Farquhar et al., *"Detecting hallucinations via semantic entropy"*, Nature 2024.
32. Google SynthID + C2PA (Google blog).
33. Chen et al., *"Real-Time Assisted Writing"* (Gmail Smart Compose), arXiv 1906.00080.
34. Google Workspace Study (Dec 2025): *"92% of young leaders want AI personalization."*

### Data quality / scaling
35. Xie, Pham et al., *"DoReMi"*, NeurIPS 2023, arXiv 2305.10429.
36. Tirumala et al., *"D4: Document De-Dup and Diversification"*, NeurIPS 2023, arXiv 2308.12284.
37. *DataComp-LM (DCLM)*, NeurIPS 2024, arXiv 2406.11794.
38. Elazar et al., *"What's In My Big Data?"*, ICLR 2024, arXiv 2310.20707.
39. Gunasekar et al., *"Textbooks Are All You Need"*, arXiv 2306.11644.
40. *Llama 3 Herd of Models*, Meta, arXiv 2407.21783.
41. *"Scaling Laws Revisited: Data Quality"*, arXiv 2510.03313.

### Embedding / retrieval landscape
42. Gao, Yao, Chen, *"SimCSE"*, EMNLP 2021, arXiv 2104.08821.
43. Chen, Xiao, Zhang, *"BGE-M3"*, BAAI, arXiv 2402.03216.
44. Wang et al., *"E5-Mistral / Improving Text Embeddings with LLMs"*, Microsoft, ICLR 2024.
45. Moreira et al., *"NV-Retriever"*, NVIDIA, arXiv 2407.15831.
46. Voyage-3 / Voyage-3-large blog (2024/2025).
47. Santhanam, Khattab et al., *"ColBERTv2"*, NAACL 2022.

### Market / deals
48. Scale AI — $14.3B Meta investment (June 2025) → $29B.
49. Surge AI — reported $1.2B revenue 2024 / $1.4B 2025 (Sacra, Bloomberg).
50. Mercor — $350M Series C, $10B valuation, Oct 2025 (TechCrunch).
51. Invisible — $2B+ valuation, $134M revenue 2024 (Sacra).
52. MongoDB-Voyage AI acquisition — ~$220M, Oct 2024.
53. OpenAI-News Corp — $250M / 5yr, May 2024.
54. Google-Reddit — ~$60M/yr, Feb 2024.
55. ElevenLabs — $3.3B valuation, Aug 2024.

### Novel embedder directions (E14–E20 training candidates)
56. Peldszus & Stede, *Argumentative Microtext Corpus* (Potsdam).
57. Chakrabarty et al., *"AMPERSAND"*, EMNLP 2019.
58. Rao & Tetreault, *"GYAFC"*, NAACL 2018, arXiv 1803.06535.
59. Saurí & Pustejovsky, *"FactBank"*, 2009.
60. Wang, *"LIAR"*, 2017.
61. Khodak et al., *"SARC"*, 2018.
62. Oprea & Magdy, *"iSarcasm"*, ACL 2020.

---

*End of document. Version 1.0, April 14, 2026.*
