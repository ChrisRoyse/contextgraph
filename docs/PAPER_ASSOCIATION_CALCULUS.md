# Meaning Compression and the Calculus of Association: Frozen Embedders as Designable Measurement Instruments, Derived Data Abundance, and Teleological Constellation Training

**Chris Royse**
Leapable.ai · socialmedia2.com
chrisroyseai@gmail.com

**Version:** 2.0 (association-calculus reframe), 2026-05-31. Supersedes the April 2026 manuscript "Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression," and preserves every measured result and disclosure in it.
**Primary:** cs.LG · **cross-list:** cs.CL, cs.CV, cs.IT
**Keywords:** frozen embedders; multi-embedder decomposition; mutual information; meaning compression; identity-locked generation; model collapse; speaker verification; constellation guard; differentiated information.

---

## Abstract

A trained embedder is usually treated as a black-box vectoriser. This paper takes the opposite view. A frozen embedder is a designable measurement instrument. It crystallises the associational structure of whatever corpus it was trained on, so that projecting a new input through it does not so much "encode" the input as report where that input sits inside the corpus's whole web of associations. The corpus is the lens. That is what makes the instrument manufacturable: you can commission a measuring device for any axis of association you can assemble a corpus for, freeze it, and reuse it indefinitely.

Three constructs follow from this one reframe, and each is formalised and instantiated in a running production system. **Derived Data Abundance (DDA)** is a counting identity. A fixed corpus of $n$ real inputs projected through $N$ frozen, approximately-independent instruments yields up to $n\cdot(N+\binom{N}{2}+1)$ structured supervisory signals; the $\binom{N}{2}$ cross-terms are associations *between* associations, a structured-analogy operator that can relate objects sharing zero surface features. **Meaning compression** is the ratio view, structured signal per raw-data unit, which we propose as a fourth compression category alongside bit-, weight-, and activation-compression. **Teleological Constellation Training (TCT)** reuses the same frozen panel as a geometric alignment target, a constellation of named identity centroids that is never flattened, with a per-output cosine guard $G_\tau$ that gives deterministic, model-free, per-output verifiability.

Three production systems instantiate the substrate at growing panel sizes: **ClipCannon** (video; $N=7$; up to 28 signals/input; ≈67,585-line Python), **Context Graph** (text and code; $N=13$; up to 92 signals/input; ≈370,000-line Rust), and **socialmedia2.com / Polis** (civic matching; $N=21$; up to 231 signals/input). The last enforces an explicit information-theoretic *differentiation contract*: every instrument must add at least 0.05 bits of new information about a real outcome, and no pair may correlate above 0.6, or it is retired as noise. Three case studies then read the framework back. **Case 3 (voice)** is measured at **0.961 mean WavLM speaker-similarity (max 0.975; 9 of 10 at or above 0.95; N=10, one speaker)** under an encoder-matched protocol, with naturalness on par with real recordings (DNSMOS 3.93/3.93; UTMOS 3.012/2.997) and a stated cross-encoder caveat. **Case 1 (style)** answers any topic, holds character under prompt injection, shows an emergent zero-shot transfer to Golden-Age Spanish, and is published (`huggingface.co/cabdru/shakespeare-lora-gemma4`). **Case 2 (talking-head)** is architecturally complete with six forensic fixes but remains end-to-end projected. We keep the boundary honest throughout. This calculus computes the associations present in the training corpora, which are measurable, bounded, and auditable; it does not compute "all of reality," and the $\binom{N}{2}$ abundance is a constructive upper bound that still awaits a pairwise mutual-information audit.

---

# 1. Introduction

## 1.1 The data wall, and the two paths everyone is already on

Frontier pre-training has nearly reached the ceiling of high-quality public text. Villalobos et al. (2022/2024) put the indexed human-text stock at roughly 300 trillion tokens and project exhaustion somewhere between 2026 and 2032, sooner under over-training regimes. Longpre et al. (2024) watched a quarter of the high-quality C4 tokens become restricted in a single year. The most-discussed response is to train on generator-synthesised data, and that road is bounded: Shumailov et al. (2024) show in *Nature* that recursive self-generation drives an irreversible distributional drift they call model collapse. The commercial consequence is a data-licensing race running to the hundreds of millions per partnership. So the standing responses divide cleanly. Path 1 buys more real data. Path 2 makes more synthetic data. Both count Shannon-compressed tokens, and both stay inside the collapse-prone recursion the moment they generate.

## 1.2 The reframe: a frozen embedder is a designable lens of association

This paper takes a third path, and it starts by asking what an embedder actually *is*. Training an embedder on a corpus and then freezing its weights does not store the corpus. It stores the geometry of how everything in that corpus related to everything else. The frozen weights are the associational structure of the training data. So when a new input $x$ is projected through a frozen $\phi$, the output $\phi(x)$ is not an encoding of $x$ at all. It is a measurement of $x$ against that whole web of associations, returning $x$'s coordinates inside it.

Three moves stack on this observation, and they organise the paper:

1. **One frozen embedder is one designable lens of association.** The corpus you train on is the lens you get. So the instrument is manufacturable. A "legal-risk" lens is a model frozen on legal text; an "affective-resonance" lens is one frozen on annotated affect. You are not stuck with off-the-shelf semantics. You can commission an instrument for any axis of association you can assemble a corpus for. The lens is designed by choosing what to remember.
2. **$N$ lenses are $N$ independent association-spaces, and the $\binom{N}{2}$ cross-terms are associations between associations.** The pairwise term does not measure the object twice. It measures how the object's position in lens A relates to its position in lens B. That is a structured-analogy operator, and it can relate two things that share no surface features at all, because the relation lives in latent, transitive association.
3. **A centroid plus a threshold is a label.** Name a region of an association-space (a constellation centroid), gate membership with $G_\tau$, and continuous geometry becomes a discrete, countable, auditable label. That is meaning compression in one line: association-geometry turned into a machine-usable supervisory signal.

Section 2 develops the reframe and its consequences in full; the rest of the paper formalises and measures them.

## 1.3 The three constructs and the contribution

Derived Data Abundance (DDA) is the principle that a fixed corpus of $n$ inputs, projected through a panel of $N$ approximately-independent frozen instruments, yields $n\cdot(N+\binom{N}{2}+1)$ structured supervisory signals at the dataset level for a code substrate, which works out to as many as 92 per input at $N=13$ once a chunk-foundationality scalar is included. Meaning compression is the ratio view of that same fact, and we propose it as a fourth taxonomic entry alongside bit-, weight-, and activation-compression. Teleological Constellation Training (TCT) is the three-phase method that builds a frozen multi-modal centroid set $C_P$, trains a generator against it with a geometric-proximity loss, and enforces a per-output cosine guard $G_\tau$ at inference. The same frozen panel plays three roles in TCT (retriever, labeller, runtime guard) with no parameter updates.

So the contribution is one architectural move, three consequences, and one reframe. The move is to use a single frozen embedder panel as retriever, labeller, and runtime guard at once, over identical parameter instances. The consequences are a counting identity ($|D'| = n\cdot(N+\binom{N}{2}+1)$, up to 92 at $N=13$), a constructive sample-complexity bound ($\mathcal{T}_{\mathrm{TCT}}(P)=O(M\cdot k)$), and a per-output verifiability property that scalar-reward alignment does not provide. The reframe is that all three are best read as a calculus of association over manufacturable measurement instruments. That reading is what makes the method general across video, code, and civic matching, and what ties it to the information-physics tradition in which information is a physical, countable substrate.

We make no claim to "calculate all of reality." The claim is narrower and, we think, defensible: a frozen embedder panel used as retriever, labeller, and runtime guard admits a counting identity with a constructive upper bound, a sample-complexity theorem with measured operating points, and a per-output verifiability property. All of it is bounded by the associations present in the chosen corpora, and all of it is auditable.

---

# 2. The Reframe: Frozen Embedders as a Calculus of Association

This section is the conceptual heart of the paper. It states what the later sections formalise.

## 2.1 An embedder is a frozen crystallisation of its corpus's associations

Let $\phi$ be trained on corpus $D_\phi$ and then frozen ($\partial\phi/\partial\theta = 0$). The learned geometry of $\phi$ encodes the relational statistics of $D_\phi$: which things co-occur, entail, resemble, precede, or contrast with which, at whatever level of abstraction the training objective rewarded. Freezing converts a learner into an instrument, a fixed function whose output coordinates report a new input's position relative to the associational structure $\phi$ memorised. Because the instrument is deterministic and frozen, every later measurement is reproducible and auditable. That is the precondition for treating $\phi(x)$ as a supervisory signal rather than a transient activation.

## 2.2 The lens is manufacturable

Because $D_\phi$ determines the lens, the lens is a design choice. This is the most under-exploited fact in representation learning. Practitioners treat embedders as fixed commodities, but they are commissionable measuring devices. Want an instrument that measures "narrative shape"? Train a classifier-embedding on story-structure annotations and freeze it (Polis slot S5). Want to separate "causal want" from "causal give"? Train an asymmetric causal embedder on contribution statements and freeze the cause and effect halves (Polis S3/S4). Want "code foundationality"? Compute a PageRank over a frozen dependency graph (Context Graph $\psi_{\mathrm{found}}$). Assembled this way, the panel is not a model. It is a library of instruments, built per problem.

## 2.3 $N$ lenses and the $\binom{N}{2}$ cross-terms: associations between associations

Project an object through $N$ instruments and you get $N$ coordinate vectors, one per association-space. The pairwise interaction features $\rho_{jk}(\phi_j(x),\phi_k(x))$ (a normalised cosine, a pairwise mutual-information estimate, or a concatenation through a frozen adapter) measure how an object's position in space $j$ relates to its position in space $k$. That is the structured-analogy operator, and it surfaces relations no single lens can see. The same operator underlies code-chunk and failure-mode, patch and nearest-precedent, person and gathering-format, voice-sample and identity: one mechanism across every domain. Its cardinality is $\binom{N}{2}$, which is why adding one instrument multiplies value against every existing instrument, and also why redundancy has to be policed (§2.6, §5.3).

## 2.4 Centroid plus threshold is a label (meaning compression)

A frozen centroid $c_m = \overline{\phi_m(R_m)}/\lVert\cdot\rVert$ names a region of association-space $m$, the region occupied by a reference set $R_m$ (a subject, a style, an outcome class). The predicate $\mathbb{1}[\cos(\phi_m(x),c_m)\ge\tau_m]$ turns continuous geometry into a discrete, countable, auditable label: does $x$ belong to the association-region of $R_m$? Stack such labels across the panel and a fixed, unlabelled corpus becomes a dense, structured supervisory dataset; that is the static side of meaning compression. Gate generation against the same labels and alignment becomes checkable geometry; that is the dynamic side.

## 2.5 What this makes possible

- A designable measurement basis for meaning. You build the exact association-axes a problem needs by choosing corpora and freezing, and each new axis earns its place only by adding *differentiated* information (§5.3).
- Structured supervision from fixed, unlabelled data at $N(N-1)/2$ density, without generating anything, which places it outside the model-collapse recursion (§4.4).
- Cross-domain bridging and analogy at scale, through the transitive cross-terms (§2.3).
- Per-output, model-free verification. Frozen and deterministic means membership is reproducible and auditable, so alignment becomes geometry you can check rather than a scalar you trust (§6).
- Identity-locked, controllable generation. Pin a generator to a named region and it cannot drift out without failing the gate, whether the target is a voice, a style, a face, or a persona.
- Field mapping. Project a domain's corpus through the panel and you get a labelled association-graph of that field (Context Graph's chunk–skill–ability graph; Polis's 21-axis citizen manifold).
- Composability and transfer. Train once, reuse forever. The marginal cost of a new axis is one frozen forward pass and its marginal value is its new bits, which is a different economics from per-task retraining.

## 2.6 The honest boundary (what keeps the claim defensible)

Stated carefully, the claim survives a metrologist; overstated, it collapses into "I calculate all of reality." Three guardrails keep it on the right side.

- It computes associations that are present in the training corpora, and those are measurable, bounded, and auditable. A garbage corpus gives a garbage lens. Spurious associations are a real failure mode, which is why Polis runs a quarterly redundancy and retirement audit (§5.3) and keeps a confounder and proxy-risk register.
- The $\binom{N}{2}$ abundance is a constructive upper bound, not a floor. Two lenses that secretly encode the same associations add almost no bits. The whole differentiation contract exists because the honest version of "associations between anything" is "associations between anything, minus the redundant ones, measured in bits against a real outcome."
- Cross-lens comparison stays structured. We use scalar relationships, mutual information, and agreement, never dim-wise flattening. The associations live between the spaces, and the panel is never collapsed into a single vector.

---

# 3. Related Work (condensed)

**Data wall and collapse.** Villalobos et al. (2022/2024) and Longpre et al. (2024) frame the stock-and-accessibility problem. Shumailov et al. (2024, *Nature*), Dohmatob et al. (2024), and Alemohammad et al. (2023) establish irreversible drift under the recursion $D_{t+1}\ni\tilde x\sim p_{\theta_t}$. The two cited mitigations, Gerstgrasser et al. (2024) (accumulation) and Feng et al. (2024) (verification), both stay inside that recursion. DDA's signals are frozen measurements of real inputs rather than generator samples, so the recursion precondition simply does not hold (§4.4).

**Compression as intelligence.** Delétang et al. (2024) run LLMs as general compressors, and Huang et al. (2024) report a correlation of about 0.95 between compression ratio and capability across more than thirty LLMs. The standard taxonomy of bit-, weight-, and activation-compression measures economy of output relative to a same-modality input. None of it measures labelled-signal yield per input. We propose meaning compression as a fourth axis (§4.2), an extension of this line rather than a contradiction of it.

**Multi-embedder retrieval.** SigLIP (Zhai et al., 2023), Nomic Embed (Nussbaum et al., 2024), WavLM (Chen et al., 2022), ColBERTv2 (Santhanam et al., 2022), SPLADE (Formal et al., 2021), and KEPLER (Wang et al., 2021), with HNSW (Malkov & Yashunin, 2018) and RRF (Cormack et al., 2009), are the standard parts. That literature uses $N$ embedders as retrieval instruments, scored by NDCG, Recall@k, or EER. None uses the same $N$ as a labeller over a fixed corpus and a per-output runtime guard at the same time. The role-flip over identical frozen instances is the novelty relative to this pool.

**Identity-locked generation and alignment.** Zero-shot voice cloning (VALL-E 2 at about 0.881 WavLM-SV; NaturalSpeech 3 at about 0.891; the field clusters around 0.86 to 0.89) uses single-prompt conditioning, not a frozen multi-clip centroid with a per-output cosine gate. Scalar-reward alignment (InstructGPT, Ouyang et al., 2022; DPO, Rafailov et al., 2023; Constitutional AI, Bai et al., 2022) optimises an expectation under a learned scalar and ships no per-output frozen-threshold predicate. The contrast with TCT is structural rather than a benchmark contest (§8.3).

---

# 4. DDA and Meaning Compression

## 4.1 The DDA counting identity

Let $D=\{x_i\}_{i=1}^n$ be a fixed corpus and $\Phi=\{\phi_m\}_{m=1}^N$ a panel of **frozen**, **approximately-independent** instruments. For pairs $j<k$ let $\rho_{jk}$ be a cross-embedder interaction feature, and for code chunks let $\psi_{\mathrm{found}}(x)\in[0,1]$ be the PageRank-derived foundationality scalar from the frozen dependency graph. The derived dataset materialises every per-embedder projection, every pairwise interaction, and the foundationality scalar:

$$|D'| = n\cdot\Bigl(N + \tbinom{N}{2} + 1\Bigr),\qquad(1)$$

which decomposes into $nN$ per-embedder vectors, $n\binom{N}{2}$ pairwise interaction features, and $n$ graph-foundationality scalars. At $N=13$ the per-input yield is $13+78+1 = \mathbf{92}$; at $N=7$ (no code graph) it is $7+21 = \mathbf{28}$; at $N=21$ (no code graph) it is $21+210 = \mathbf{231}$.

Equation (1) is a structural upper bound, not an information-theoretic floor. The $nN$ term is tight as long as the instruments produce non-constant outputs. The $\binom{N}{2}$ term carries real information only under approximate orthogonality. Two instruments that collapse to a linear reparameterisation of each other add almost no bits, and (1) becomes strict. That is why we write "**up to** $n\cdot(N+\binom{N}{2}+1)$," and why the pairwise tightness waits on a mutual-information audit (§9, EXP-2). A *signal* here is a scalar or low-dimensional feature: a deterministic function of a real input or fixed-corpus graph under frozen instruments, attached to a specific index, usable as input or target without collecting new data. DDA is therefore **not** synthetic generation, **not** augmentation, and **not** distillation.

## 4.2 Meaning compression as the fourth category

The meaning-compression ratio is $\mathrm{MC}(\Phi,D) = |D'|/\mathrm{raw}(D) = N+\binom{N}{2}(+1)$ per input. Conventional compression asks how few bits, parameters, or megabytes a fixed content can be squeezed into. Meaning compression flips the question: for a fixed amount of raw data, how many structured supervisory signals can you extract? The four categories work on genuinely different unit pairs:

| Category | Input unit | Output unit | Canonical techniques |
|---|---|---|---|
| Bit | bits of raw data | bits of compressed data | gzip, FLAC, PNG, arithmetic coding |
| Weight | trainable parameters | accuracy at those parameters | GPTQ, SparseGPT, distillation |
| Activation | peak runtime memory | quality at that memory | TurboQuant, KV-cache quantisation |
| **Meaning** | **one raw input** | **structured labelled signals** | **DDA (this paper)** |

The four are complementary axes, and a single pipeline can realise all of them. We do not claim meaning compression supersedes the others. Whether reviewers accept the taxonomic distinction is a scholarly judgement we leave open.

## 4.3 Foundationality-graded compression and compression progress

Two refinements carry over from the production code substrate. The first is foundationality-graded compression: aggressiveness scales inversely with a chunk's PageRank foundationality $s_i$, with $\alpha_{\mathrm{compress}}(x_i)=\mathrm{clip}(1-0.75\,s_i,0.25,1.0)$. Foundational chunks are sampled $(1+\lambda_{\mathrm{found}}s_i)$ and Fisher-protected $(1+\lambda_{\mathrm{fisher}}s_i)$ more aggressively, yet compressed less. The second is compression progress, $CP_\Phi(t) = H_\Phi(Y\mid X,a;\theta_{t-1}) - H_\Phi(Y\mid X,a;\theta_t)$, the per-step reduction in bits of the conditional description length of a frozen target panel under a predictor. It is the Schmidhuberian dynamic dual of meaning compression's static substrate-density claim. This metric belongs to a code-verification deployment that is still earlier-stage, so we record it for completeness, not as a measured result.

## 4.4 Scope of the collapse-silent claim

Collapse needs a generator-in-loop recursion $D_{t+1}\ni\tilde x\sim p_{\theta_t}$. DDA never instantiates one. Each $\phi_m(x)\in D'$ depends only on a real $x\in D$ and on frozen parameters, so $D'$ is a decomposition of $D$ rather than a sample from a generator. The precondition fails, and the cited theorems say nothing about this configuration. This is a scope argument, not an immunity theorem. We do not claim a new independence-from-collapse result, and we hold two boundary cases explicitly out of scope: inference-time best-of-$K$ selection, which updates no parameter, and embedder-mediated data curation, which has a different feedback structure.

## 4.5 The three-role composition

The same frozen $\Phi$ plays three roles at once over identical parameter instances, with no parameter update between or during them. As retriever it ranks stored content by cosine. As labeller it materialises the signals of (1). As runtime guard it evaluates a candidate against a frozen centroid (§6). Relative to the locked reference set, no cited paper composes all three roles together over identical frozen instances.

---

# 5. Three Production Systems

Three antecedent systems instantiate the labeller role at increasing panel size. All counts are system-at-date figures taken from the working tree, so the relationship is a constructive witness, not a controlled experimental platform.

## 5.1 ClipCannon: video, $N=7$, up to 28 signals/input

ClipCannon is a ≈67,585-line Python package that runs a 23-stage analysis DAG over source video. It extracts seven modalities: visual (SigLIP-SO400M, 1,152-D), semantic (nomic-embed-text-v1.5, 768-D), emotion (wav2vec2-large, 1,024-D), speaker (WavLM-large, 512-D), prosody (custom 12-D), sentiment (MiniLM-L6-v2, 384-D), and voice identity (ECAPA-TDNN, 192-D). That is **4,044 dimensions across seven modalities**, exposed as 58 MCP tools, with a SHA-256 tamper-evident provenance chain. At $N=7$ it yields **up to 28** signals/input. It is the labeller for Cases 2 and 3, and the curation substrate that turns one 16-minute interview into 2,362 training clips.

## 5.2 Context Graph: text and code, $N=13$, up to 92 signals/input

Context Graph is a ≈370,000-line Rust workspace that embeds every stored memory through **13 frozen embedders (E1 to E13)**. Together they span 11,008 dense dimensions, two 30,522-term sparse vocabularies (E6 lexical, E13 SPLADE), and a 128-per-token late-interaction space (E12 ColBERTv2). The remaining slots are e5-large-v2 semantic (E1), sinusoidal and Fourier temporal (E2 to E4), asymmetric Nomic causal (E5), Qodo 1,536-D code (E7), structural e5 for graph edges (E8), 10,000-bit HDC typo-robust (E9), paraphrase-asymmetric e5-base (E10), and KEPLER entity geometry (E11). E5, E8, and E10 store dual vectors (cause/effect; source/target; doc/query). Persistence is 59 RocksDB column families, and retrieval is weighted RRF over the per-embedder rankings, exposed as 75 MCP tools. At $N=13$ it yields **up to 92** signals/input ($13+78+1$). Context Graph is the backbone of Leapable.ai's knowledge-vault product and the labeller for Case 1.

## 5.3 socialmedia2.com / Polis: civic matching, $N=21$, and the differentiation contract

The most information-theoretically explicit instantiation is the civic-matching system behind socialmedia2.com (codename Polis), which matches citizens into durable real-world gatherings instead of optimising engagement. Its constellation is a **21-slot slate across 11 orthogonal axes** plus a hard-constraint vector, roughly 35,800 dimensions per citizen, and it is **never flattened**. The axes and a few representative slots: semantic content (S1 `sem-self`, S2 `lex-vocab` SPLADE), causal want and give (S3/S4 asymmetric Nomic-causal), narrative form (S5 `story-shape`, S6 `hero-arc`), cognitive mode (S7 `reason-mode`, S8 `abstract-concrete`), moral foundations (S9 `mfq-vec`), ideological bridging (S10 `bridge-vec`), somatic and prosodic (S11 to S13 voice tempo, affect, energy), temporal rhythm (S14/S15 Fourier day and week), aesthetic field (S16 SigLIP-2 on curated images, S17 stylometry), network position (S18 TransE-class, S19 Burt structural-hole), and relational dynamics (S20 conflict-style, S21 care-mode). At $N=21$ the DDA identity gives **up to 231** signals/input ($21+210$).

Polis is where the honest boundary of §2.6 hardens into an enforced engineering contract. A slate $S$ is *differentiated* when:
- **(D1) Distinctness.** $\forall i\neq j:\ \mathrm{corr}_+(E_i(X),E_j(X))\le 0.6$ on a calibration set $|X|\ge 500$.
- **(D2) Marginal information.** $\forall i:\ H(y\mid E_1..E_{i-1}) - H(y\mid E_1..E_i)\ \ge\ 0.05$ bits, where $y$ is the binary 30-day in-person tie-formation outcome.
- **(D3) Frozen.** $\forall i,\ \forall\text{ phase}\in\{\text{match, train, eval, serve}\}:\ \partial E_i/\partial\theta_i=0$.

The principle behind the three conditions is that a panel's value is $\sum_i\Delta H_i = \sum_i I(y;E_i\mid E_1..E_{i-1})$, not $\sum_i H(E_i)$. A redundant slot, with $\mathrm{corr}\approx1$, contributes $\Delta H_i\approx 0$. It is noise per token, and it is retired at the quarterly audit that runs over all $\binom{21}{2}=210$ pairs. This is the calculus of association turned into a build gate: instruments are admitted in bits against a real outcome, not by raw dimensionality. On status, we stay honest. The slate registry, the 21-slot constellation assembly, slot-shape validation, the semantic probe suite, and the pairwise-redundancy report are implemented and operational; a five-answer intake, for instance, produces a complete 21-slot constellation, and `assemble_constellation` succeeds only when all 21 slots are present. What is still pending is the real ≥500-citizen calibration corpus and the 30-day outcome validation that would turn D2 from a contract into a measured pass. The system is pre-launch, and its mechanical `BUILD_DONE` predicate is deliberately false until that real evidence exists.

## 5.4 The three systems on one curve

Plot signals-per-input, $N+\binom{N}{2}(+1)$, against $N$ and the three systems land on a single quadratic: ClipCannon at $N=7\to28$, Context Graph at $N=13\to92$, Polis at $N=21\to231$. Each added instrument multiplies associations against all the existing ones. That is the engine of meaning compression, and the differentiation contract is what keeps the growth signal rather than noise.

---

# 6. Teleological Constellation Training

## 6.1 The constellation $C_P$

For a subject $P$ and a modality set $\mathcal{M}=\{m_1,..,m_M\}$, with a reference set $R_m$ and a frozen embedder $\phi_m$ per modality, the constellation is $C_P=\{(m,c_m):m\in\mathcal{M}\}$. Each $c_m$ is a frozen unit-norm centroid, double-normalised: per-sample L2, then the mean, then L2 again, with samples of norm below $10^{-6}$ excluded and a leave-one-out cosine below $0.85$ flagged as degenerate. The constellation is the single object the method is defined against. DDA produces the projections it is built from, Phase 2 targets it, and Phase 3 verifies it.

## 6.2 Phase 2: constellation-conditioned training

The training objective is $\mathcal{L}_{\mathrm{TCT}}(\theta) = \mathcal{L}_{\mathrm{gen}} + \sum_m\lambda_m(1-\cos(\phi_m(\hat y),c_m))$. Gradients flow through $\hat y$ into $\theta$, but never into $\phi_m$ or $c_m$: the embedders stay frozen, LoRA sits on the generator only, and centroid hashes are checked before and after. Cosine is computed in fp32 against a bf16 generator forward, which removes threshold-proximity nondeterminism. An over-high $\lambda_m$ collapses the generator onto the constellation independent of conditioning, with $\cos\to1.0$ while $\mathcal{L}_{\mathrm{gen}}$ stalls; the mitigation is to halve $\lambda_m$. When the backbone is inference-only, as in Case 3, Phase 2 reduces to **best-of-$K$ selection**: draw $K$ candidates, score $s_m^{(k)}=\cos(\phi_m(\hat y^{(k)}),c_m)$, and return $\arg\max_k\sum_m w_m s_m^{(k)}$.

## 6.3 Phase 3: the runtime guard $G_\tau$

$$G_\tau(\hat y;C_P) = \mathbb{1}\bigl[\forall m\in\mathcal{M}:\ \cos(\phi_m(\hat y),c_m)\ge\tau_m\bigr].\qquad(5)$$

The guard is per-output, costs $O(M\cdot d_{\max})$, uses no auxiliary learned model, and is deterministic given the tensors. Each $\tau_m$ is calibrated before deployment to a same-session-band floor: the 10th percentile of authentic-sample cosines on a disjoint calibration set. Tuning $\tau$ on the evaluation set would invalidate the guard. Every returned output carries a cosine trace, an embedder weight hash, a centroid hash, the threshold set, the decision, and the attempt count, which together form the audit record that makes the guard verifiable after the fact. What is verified is per-output verifiability relative to the chosen centroid panel: that $\hat y$ satisfies the cosine predicate against frozen $C_P$ under $\Phi$. It is not a claim that $\hat y$ is correct under some external criterion the panel cannot see.

## 6.4 Two theorems

**Theorem 1 (constructive sample complexity).** Under the Phase 1/2/3 structure and Axioms 1 to 3, the subject-specific sample footprint is $\mathcal{T}_{\mathrm{TCT}}(P)=O(\sum_m n_m)=O(M\cdot k)$, independent of $|D|$. The bound is constructive: any pipeline with exactly those phases satisfies it by definition. Its empirical content, that $n_m$ in the range $[10,10^2]$ suffices for a stable centroid, rests on the measured operating points of §7.

**Theorem 2 (per-output verifiability).** $G_\tau$ is a deterministic $O(M\cdot d_{\max})$ geometric test that admits no auxiliary learned model, a property scalar-reward alignment does not furnish in the same technical sense (§8.3).

---

# 7. Case Studies

Every number below is tagged MEASURED or PROJECTED. Measured numbers carry their date and pipeline version; projected numbers are flagged clause by clause.

## 7.1 Case 1: Shakespeare stylistic LoRA (MEASURED end-to-end 2026-04-17)

The target was an open-weight LLM that answers any modern question in Shakespearean style with two hard properties: it never refuses a topic, and it never drops the style under prompt injection. The base was Gemma-4-E4B-it (NF4 at inference), with a LoRA of **rank 256, $\alpha=512$ across 258 modules**. The corpus was the Shakespeare canon projected through Context Graph's 13-embedder panel as DDA labels: 2,698 memories furnishing roughly 249,000 supervisory signals (13 topic-profile scores plus 78 pairwise cross-correlations per memory, 22,152 typed edges, and constellation conditioning), distilled down to a forensically-cleaned **7,925 SFT examples** and **11,724 DPO pairs** scored against a 9-embedder STYLE_CENTROID (E2, E3, E5, and E11 were left out as orthogonal to style).

The measured results: SFT loss fell to **1.12** (from about 12) at **98.9%** token accuracy over 765 steps (164.7 min); DPO loss fell to **0.47** (from 1.75) at **98.0%** rewards-accuracy with `rewards/margins` of **43.62** (282.4 min); about 7.5 hours total on one RTX 5090. On held-out behavioural probes the model wrote love poetry, explained quantum entanglement with reference to Bell's theorem in verse, refused an admin-override prompt-injection attempt while staying in character, produced a Go binary search with Shakespearean code comments, and, when challenged in Spanish, answered in an archaic register (more on that below). The weights are published at `huggingface.co/cabdru/shakespeare-lora-gemma4`.

There was also a forensic finding, and it is where multi-embedder guarding earned its keep. An earlier run passed its integrity checks yet echoed each prompt in modern English before turning Shakespearean. Single-embedder E1 similarity missed the problem completely. The 13-embedder audit caught it: E4 (position-ordering) and E12 (token-level MaxSim) flagged an opening-position style anomaly that was invisible to overall-document similarity. The root cause was mundane. **67% (5,954/8,857)** of the original SFT examples began with inherited play-script headers ("SCENE III. … Enter Cassius …"). The fix was data preparation. The lesson is that single-embedder evaluation misses opening-position pathologies that are highly salient to a human reader, which is the standing argument for multi-embedder guarding.

The emergent finding is measured but single-instance. A Spanish prompt demanding a Spanish answer produced **Siglo de Oro** Spanish, in the Cervantes, Lope, and Calderón register of roughly 1580 to 1630, rather than modern Spanish. It used archaic forms (`vos`, `vuestro`, `ojalás`, `obsequie`), archaic vocabulary (`ocaso`, `reino celestial`, `sacerdotes y sabios`, `perdición`), and Shakespearean rhetoric (cascading polysyndeton, apostrophe to celestial bodies, prophetic portents). The base model without the LoRA does not do this. Read through §2, the explanation is that the LoRA learned "archaic literary register" as a cross-signal fingerprint across language-agnostic association components, namely E9 (rhythm and cadence), E7 (periodic structure), E10 (surface-form underdetermines meaning), and E1 (cosmic-order, fate, and honour semantic field), and forcing Spanish retrieved the nearest manifestation of that fingerprint. It is single-language, single-register, and small-sample. A systematic cross-language matrix is named future work, and we do not rule out alternative accounts such as latent base-model competence.

## 7.2 Case 2: identity-locked talking-head "Santa" (PROJECTED end-to-end; forensic integrity complete)

The target was an audio-driven talking-head that reproduces one subject visually and behaviourally, with an eight-state emotional constellation and per-frame identity verification, built from a 16-minute 15-second 2560×1440/60fps interview. Over five days we evaluated **15 renderer architectures**; EchoMimicV3 LoRA gave the best identity lock. The constellation is a seven-modality ClipCannon panel taken after diarisation: visual 1,725×1,152-D, semantic 200×768-D, emotion 343×1,024-D, speaker 300×512-D, prosody 188×12-D, sentiment 202×384-D, and voice 188×192-D, with projected thresholds of 0.70 visual, 0.60 emotion, 0.80 speaker, and 0.85 voice.

What is measured or complete: the meeting-integration pipeline (Chrome-extension detector, Xvfb bot, Playwright Google-Meet client, Insertable-Streams injection); the audio stack (Whisper plus Gemma reasoning at about 509 ms, with same-subject voice-clone SECS above 0.95); a constellation expression system (40 expression groups to 34 skills to 8 constellations) whose `ConstellationGuard` re-embeds each frame through SigLIP and validates cosine 1.0 against the reference while rejecting stranger faces below 0.5; a YOLOv5-Face pipeline over 24,397 frames at about 90% detection; and a sub-5-ms PhysicsFaceEngine. The forensic work is load-bearing. LoRA versions 1 through 5 trained with clean loss yet produced dead lip-sync, because the training target convention was inverted (`target = latent − noise` where the scheduler expects `target = noise − latent`). Loss convergence did not verify trajectory correctness; only the constellation-side audit surfaced the bug. Five companion fixes followed: a frame-rate mismatch, a mouth-box-on-nose error, a name-shadowing `q_audio` dead-gradient typo, a 22 GB feature RAM leak, and a uniform-instead-of-weighted viseme curriculum. What is still pending is the corrected-sign end-to-end identity-cosine table and the lip-sync LSE-D. Every quantitative row in this subsection is projected, and a later version will re-verify it.

## 7.3 Case 3: voice cloning, 50-clip centroid (MEASURED 2026-04-16)

This is the quantitative anchor. The protocol was Qwen3-TTS (inference-only) with Full In-Context Learning, a 50-clip WavLM-base-plus-sv centroid (the top 50 of a 489-candidate pool), and WavLM-scored best-of-12 at $T=0.3$, with $\tau=0.95$ recorded for audit. It was measured on $N=10$ held-out English sentences, one speaker, encoder-matched (the guard and the evaluator are both WavLM-base-plus-sv), under pipeline `qwen3-tts-foricl-v1`:
- Mean WavLM SECS **0.961**; max **0.975**; min **0.914**; **9 of 10 at or above 0.95**.
- DNSMOS P.808: clone **3.93** versus real **3.93** ($N=14$); UTMOS: clone **3.012** versus real **2.997**.
- A within-WavLM-family 95% bootstrap CI on the mean of **[0.951, 0.971]** (10k resamples, seed 20260416; retrospective, not power-based).
- $T=0.3$ was the sweep optimum (sweep mean 0.932, max 0.963; the headline run was 0.961 / 0.975).
- For field reference: VALL-E 2 at about 0.881 and NaturalSpeech 3 at about 0.891, with the field clustering around 0.86 to 0.89.

One scope disclosure is mandatory. The 0.961 is **encoder-matched**: the selector and the scorer are the same WavLM family, so the figure demonstrates that the guard behaves as specified, but it is **not** cross-encoder identity verification. A survivor of selection under $\phi$ scores high on $\phi$ by construction. The cross-encoder triangulation, an ECAPA-TDNN re-score of the ten argmax candidates, is instrument-ready and pending (§9, EXP-1). Read through §2.4, the 0.961 is the measured endpoint of generation collapsing into the named association-region of the speaker centroid, a tight, low-entropy cluster around identity.

## 7.4 Summary

| Case | Mode | $M$ | Status | Headline |
|---|---|---|---|---|
| 1 Shakespeare | frozen-with-LoRA (SFT+DPO, rank 256) | 1 (text-style, 9-embedder) | **MEASURED 2026-04-17** | **SFT 1.12/98.9%; DPO 0.47/98.0%/43.62; prompt-injection-robust; emergent Golden-Age Spanish; LoRA public** |
| 2 Santa | frozen-with-LoRA (mouth-weighted flow-matching) | up to 7 | *PROJECTED; forensic complete* | *guard cosine 1.0 ref / <0.5 stranger; six fixes in code; identity table pending* |
| 3 Voice | inference-only (best-of-12) | 1 (WavLM speaker) | **MEASURED 2026-04-16** | **mean SECS 0.961, max 0.975; DNSMOS 3.93/3.93; CI [0.951,0.971]; encoder-matched** |

---

# 8. Discussion

## 8.1 Interpreting the measured anchors
The Case 3 anchor supports a narrow reading. Under the fixed pipeline, post-selection mean WavLM SECS is 0.961, and 9 of 10 English sentences clear 0.95 at $M=1$ for one speaker, with naturalness within evaluator noise of the reference. It does **not** support cross-encoder triangulation, multi-speaker or multi-language generalisation, or the $M>1$ conjunction, which is the territory Case 2 would land and Case 2 is projected. Case 1 is a second measured anchor at $M=1$ on text style (public weights, prompt-injection-robust) with an emergent out-of-distribution generalisation observation.

## 8.2 The DDA bound is an upper bound, not a floor
The $\binom{N}{2}$ term is the vulnerable part. Heavy redundancy between any pair, say E1 semantic against E10 paraphrase, both dense-text, contracts the effective count below 92. Qualitative blind-spot evidence, where sparse E6 and E13 recall things E1 misses, E9 HDC handles typos, and E7 ranks code, shows the effective count strictly exceeds $N$. The tight pairwise value still waits on the mutual-information audit (EXP-2). The Polis D1/D2 contract is the same insight made operational, and it is the reason Polis is the cleanest demonstration that "associations between anything" has to be read as "the differentiated associations, measured in bits."

## 8.3 TCT versus scalar-reward alignment: a different property
TCT aims at a frozen multi-modal constellation rather than a learned scalar. Its instrument is a frozen panel rather than a learned reward model, its metric is per-modality cosine, and its runtime posture is a per-output guard. We do **not** claim TCT beats RLHF, DPO, or Constitutional AI on preference benchmarks; the comparison is structural. Per-output verifiability relative to a chosen panel is something scalar-reward alignment cannot furnish even in principle, because it optimises an expectation and ships no frozen threshold against which each output is accepted or rejected at inference without extra learned machinery. Two scope qualifications apply: the guard is panel-relative, so cross-panel transport is open, and it is predicate-level, not identity-level under any external criterion.

## 8.4 Honest failure modes are part of the method
Both load-bearing bugs, Case 2's inverted flow-matching sign and Case 1's 67% header echo, produced pipelines that passed superficial health checks and would have shipped under single-embedder evaluation. The multi-embedder, multi-role composition is load-bearing at diagnosis time, not only at training time. Forensic audit with the frozen panel is a deployment requirement, and the three-role composition is where that audit happens.

## 8.5 Implications
For identity-locked generation, a per-output verifiable predicate against a frozen centroid panel is instrument-ready and measurable, with encoder-matched scope on Case 3 and multi-modal results pending on Case 2. For scaling-law policy, there is a genuine third path: decompose a fixed corpus through $N$ frozen instruments for up to $n\cdot(N+\binom{N}{2})$ signals, with no generation and no recursion, which extends rather than contradicts the compression-as-intelligence frame. For data strategy, the fixed corpus an organisation already holds admits an upper-bounded multiplication of structured signals per input, with the tight count pending the MI audit.

---

# 9. Limitations and Open Problems

**L1, encoder-matched Case 3.** 0.961 is within-WavLM-family agreement, not cross-encoder verification, and the CI is retrospective. Remediation is EXP-1, the ECAPA-TDNN re-score.
**L2, one speaker, one language.** No multi-speaker or multi-language generalisation is supported; the scope tag is the boundary.
**L3, Case 2 projected, Case 1 per-embedder held-out pending.** The constructive theorems and the DDA identity do not depend on either.
**L4, embedder independence is empirical, not proven.** Axiom 2 is an operational hedge, and the $\binom{N}{2}$ term needs EXP-2.
**L5, $\tau$ calibration is per-panel and per-application.** These are observed values, not universal constants, and cross-subject transport is open.
**L6, constellation degeneration under non-diverse references.** Narrow reference pools collapse the centroid; characterisation is pending.
**L7, inference cost.** Full ICL plus best-of-12 is inference-expensive, and no distilled guard is proposed here.
**L8, competing interests.** The author operates Leapable.ai and socialmedia2.com, and Context Graph, ClipCannon, and Polis are production systems from that practice. No vendor-funded benchmark, paid comparison, or commercial-performance claim is reported. Every projected number is flagged and every softening disclosed.

Three experiments would most directly tighten or kill the claims. EXP-1 is an ECAPA-TDNN cross-encoder re-score of the Case 3 argmax candidates; it promotes the headline to cross-encoder identity ranking if the mean is at least 0.85 with Pearson at least 0.6, and otherwise the within-family scope holds. EXP-2 is a pairwise mutual-information audit across the 13-embedder panel and the 21-slot Polis slate on 100 to 1,000 inputs; it would replace the pairwise part of "up to 92 / up to 231" with a measured $N_{\mathrm{eff}}$, and would falsify the counting argument if the pairwise term contracts heavily. EXP-3 is a four-point sweep of $n_m\in\{10,25,50,100\}$ on Case 3, which would promote the one-row sample-complexity anchor to a partial curve. Beyond those, Polis's real ≥500-citizen D2 calibration against 30-day tie outcomes would turn its differentiation contract from enforced-by-construction into measured-in-bits.

---

# 10. Conclusion

A frozen embedder is a reusable instrument that has memorised the association-structure of its training data, and the lens is designed by choosing what to remember. Stack a differentiated panel of such instruments and you can measure any object's position across many association-spaces at once, quantify the relations between those positions (the $\binom{N}{2}$ cross-terms), and freeze named regions of the geometry into auditable labels. Fixed, unlabelled data becomes a machine-legible map of how things relate. That is what we mean by meaning compression, the calculus of association, proposed as a fourth compression category and instantiated across video ($N=7$), code ($N=13$), and civic matching ($N=21$). We measured it where it ran: a voice reproduced at 0.961 speaker-similarity (encoder-matched), a style model with public weights that holds character under prompt injection, and a per-output geometric guard with deterministic provenance. The claims stay bounded by the associations present in the chosen corpora, not "all of reality," and the abundance is a constructive upper bound pending a mutual-information audit. Three named experiments would tighten or kill each load-bearing claim. The contribution is not a new primitive but a composition of existing ones read in a new light: information made into a physical, reusable, designable measurement basis for meaning.

---

**Competing interests.** The author is founder of Leapable.ai (a local-first MCP knowledge-vault marketplace built on Context Graph) and socialmedia2.com (a civic-gathering platform built on the Polis constellation). No vendor-funded benchmark or commercial-performance claim appears in this manuscript.

**Reproducibility.** Case 1's trained LoRA and model card are public at `huggingface.co/cabdru/shakespeare-lora-gemma4`. The Case 2 and Case 3 subject-specific artefacts are withheld on consent and dual-use grounds.

**Companion paper.** "The Symmetry of Knowing: A Theory of Epistemic Transformation in the Age of Artificial Intelligence" (accepted, 2026 AEJMC conference) develops the epistemic-symmetry framing referenced in §2.

---

## References

1. P. Villalobos, A. Ho, J. Sevilla, T. Besiroglu, L. Heim, M. Hobbhahn. Will we run out of data? Limits of LLM scaling based on human-generated data. arXiv:2211.04325, 2022 (rev. 2024; ICML 2024).
2. S. Longpre, R. Mahari, A. Lee, C. Lund, H. Oderinwale, W. Brannon, et al. Consent in Crisis: The Rapid Decline of the AI Data Commons. arXiv:2407.14933, 2024.
3. I. Shumailov, Z. Shumaylov, Y. Zhao, N. Papernot, R. Anderson, Y. Gal. AI models collapse when trained on recursively generated data. *Nature* 631:755–759, 2024. doi:10.1038/s41586-024-07566-y.
4. E. Dohmatob, Y. Feng, P. Yang, F. Charton, J. Kempe. A Tale of Tails: Model Collapse as a Change of Scaling Laws. arXiv:2402.07043, 2024 (ICML 2024).
5. S. Alemohammad, J. Casco-Rodriguez, L. Luzi, A. I. Humayun, H. Babaei, D. LeJeune, A. Siahkoohi, R. G. Baraniuk. Self-Consuming Generative Models Go MAD. arXiv:2307.01850, 2023 (ICLR 2024).
6. M. Gerstgrasser, R. Schaeffer, A. Dey, R. Rafailov, H. Sleight, J. Hughes, et al. Is Model Collapse Inevitable? Breaking the Curse of Recursion by Accumulating Real and Synthetic Data. arXiv:2404.01413, 2024.
7. Y. Feng, E. Dohmatob, P. Yang, F. Charton, J. Kempe. Beyond Model Collapse: Scaling Up with Synthesized Data Requires Verification. arXiv:2406.07515, 2024.
8. G. Delétang, A. Ruoss, P.-A. Duquenne, E. Catt, T. Genewein, et al. Language Modeling Is Compression. arXiv:2309.10668, 2023 (ICLR 2024).
9. Y. Huang, J. Zhang, Z. Shan, J. He. Compression Represents Intelligence Linearly. arXiv:2404.09937, 2024 (COLM 2024).
10. X. Zhai, B. Mustafa, A. Kolesnikov, L. Beyer. Sigmoid Loss for Language Image Pre-Training (SigLIP). ICCV 2023. arXiv:2303.15343.
11. Z. Nussbaum, J. X. Morris, B. Duderstadt, A. Mulyar. Nomic Embed: Training a Reproducible Long Context Text Embedder. arXiv:2402.01613, 2024.
12. S. Chen, C. Wang, Z. Chen, Y. Wu, et al. WavLM: Large-Scale Self-Supervised Pre-Training for Full Stack Speech Processing. *IEEE JSTSP* 16(6):1505–1518, 2022. arXiv:2110.13900.
13. K. Santhanam, O. Khattab, J. Saad-Falcon, C. Potts, M. Zaharia. ColBERTv2: Effective and Efficient Retrieval via Lightweight Late Interaction. NAACL 2022. arXiv:2112.01488.
14. T. Formal, B. Piwowarski, S. Clinchant. SPLADE: Sparse Lexical and Expansion Model for First Stage Ranking. SIGIR 2021. arXiv:2107.05720.
15. X. Wang, T. Gao, Z. Zhu, Z. Zhang, Z. Liu, J. Li, J. Tang. KEPLER: A Unified Model for Knowledge Embedding and Pre-trained Language Representation. *TACL* 9:176–194, 2021. arXiv:1911.06136.
16. Yu. A. Malkov, D. A. Yashunin. Efficient and Robust Approximate Nearest Neighbor Search Using Hierarchical Navigable Small World Graphs (HNSW). *IEEE TPAMI* 42(4):824–836, 2018. arXiv:1603.09320.
17. G. V. Cormack, C. L. A. Clarke, S. Buettcher. Reciprocal Rank Fusion Outperforms Condorcet and Individual Rank Learning Methods. SIGIR 2009. doi:10.1145/1571941.1572114.
18. L. Ouyang, J. Wu, X. Jiang, D. Almeida, C. L. Wainwright, et al. Training Language Models to Follow Instructions with Human Feedback (InstructGPT). NeurIPS 2022. arXiv:2203.02155.
19. R. Rafailov, A. Sharma, E. Mitchell, S. Ermon, C. D. Manning, C. Finn. Direct Preference Optimization: Your Language Model is Secretly a Reward Model (DPO). NeurIPS 2023. arXiv:2305.18290.
20. Y. Bai, S. Kadavath, S. Kundu, A. Askell, J. Kernion, et al. Constitutional AI: Harmlessness from AI Feedback. arXiv:2212.08073, 2022.
21. Y. Lipman, R. T. Q. Chen, H. Ben-Hamu, M. Nickel, M. Le. Flow Matching for Generative Modeling. ICLR 2023. arXiv:2210.02747.
22. E. J. Hu, Y. Shen, P. Wallis, Z. Allen-Zhu, Y. Li, S. Wang, L. Wang, W. Chen. LoRA: Low-Rank Adaptation of Large Language Models. ICLR 2022. arXiv:2106.09685.
23. B. Desplanques, J. Thienpondt, K. Demuynck. ECAPA-TDNN: Emphasized Channel Attention, Propagation and Aggregation in TDNN Based Speaker Verification. Interspeech 2020. arXiv:2005.07143.
24. C. K. A. Reddy, V. Gopal, R. Cutler. DNSMOS P.835: A Non-Intrusive Perceptual Objective Speech Quality Metric to Evaluate Noise Suppressors. ICASSP 2022. arXiv:2110.01763.
25. T. Saeki, D. Xin, W. Nakata, T. Koriyama, S. Takamichi, H. Saruwatari. UTMOS: UTokyo-SaruLab System for VoiceMOS Challenge 2022. Interspeech 2022. arXiv:2204.02152.
26. S. Chen, S. Liu, L. Zhou, Y. Liu, X. Tan, J. Li, S. Zhao, Y. Qian, F. Wei. VALL-E 2: Neural Codec Language Models are Human Parity Zero-Shot Text to Speech Synthesizers. arXiv:2406.05370, 2024.
27. Z. Ju, Y. Wang, K. Shen, X. Tan, et al. NaturalSpeech 3: Zero-Shot Speech Synthesis with Factorized Codec and Diffusion Models. ICML 2024. arXiv:2403.03100.
