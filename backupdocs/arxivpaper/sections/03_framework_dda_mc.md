# 3. Framework: DDA and Meaning Compression

Section 3.1 states the Derived Data Abundance (DDA) counting identity with its constructive upper bound. Section 3.2 develops meaning compression as the ratio view of that identity and positions it as a fourth taxonomic entry alongside bit-, weight-, and activation-compression. Section 3.3 argues that DDA pipelines are structurally outside the Shumailov-form recursion regime — a scope argument, not a collapse-immunity claim. Section 3.4 states the role-composition move that unifies DDA with the Teleological Constellation Training (TCT) method of Section 5: the same frozen embedder panel serves three roles simultaneously — retriever, labeller, and runtime guard — without parameter updates.

## 3.1 The DDA counting identity

Let $D = \{x_i\}_{i=1}^{n}$ be a fixed raw corpus of $n$ inputs, and let $\Phi = \{\phi_m\}_{m=1}^{N}$ be a panel of $N$ **frozen** (V18), **approximately independently trained** embedders $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$. Frozen means parameters are held fixed throughout the DDA pipeline and through any downstream TCT training; approximately independent (V11) carries the hedge that strict statistical independence between embedders is an open measurement question, not a derived property. For pairs $(j, k)$ with $j < k$, let $\rho_{jk}(\phi_j(x_i), \phi_k(x_i))$ be a cross-embedder interaction feature — a non-constant function of its two arguments, typically normalised cosine, a pairwise mutual-information estimate, or a concatenation followed by a frozen adapter.

The **derived dataset** $D'$ materialises every per-embedder projection and every pairwise interaction on every raw input:

$$
D' \;=\; \bigl\{\bigl(x_i,\; \phi_1(x_i), \ldots, \phi_N(x_i),\; \bigl(\rho_{jk}(\phi_j(x_i), \phi_k(x_i))\bigr)_{1 \le j < k \le N}\bigr)\bigr\}_{i=1}^{n}.
$$

Counting structured supervisory signals yields the **DDA counting identity**:

$$
|D'| \;=\; n \cdot \Bigl(N + \binom{N}{2}\Bigr) \;=\; n \cdot \frac{N(N+1)}{2}, \qquad (1)
$$

decomposing into $n \cdot N$ per-embedder labelled vectors and $n \cdot \binom{N}{2}$ pairwise cross-embedder interaction features. The per-input yield is $N + \binom{N}{2}$. For the Context Graph production panel at $N = 13$, the per-input yield is $13 + 78 = 91$ structured signals.

**Constructive upper bound.** Identity (1) is a **structural upper bound**, not an information-theoretic lower bound. Under Axiom 1 (grounded measurement; Section 5) each $\phi_m(x_i)$ is a deterministic function of the raw input, so the $n \cdot N$ term is tight so long as the embedders produce non-constant outputs. Under Axiom 2 (approximate orthogonality) the pairwise mutual-information terms $I(\phi_j(X); \phi_k(X))$ are small relative to the marginal entropies, so the $\binom{N}{2}$ term carries non-trivial information. When two embedders collapse to a linear reparameterisation of each other, their pair contributes near-zero additional information and (1) becomes a strict upper bound. We therefore state the headline "**up to** $n \cdot (N + \binom{N}{2})$ signals" and, for Context Graph, "**up to** 91 signals per input at $N = 13$": empirical embedder overlap shrinks the effective count, and formal mutual-information measurement of redundancy is named future work.

A signal here is a scalar or low-dimensional feature that is a deterministic function of a raw input under frozen embedders and frozen interaction rules, is attached to a specific (input, embedder) or (input, embedder-pair) index, and can be used as input or target in a downstream loss without new data collection. DDA is therefore **not** synthetic-data generation (it decomposes $D$, not synthesises new inputs), **not** data augmentation (the input is unchanged), and **not** distillation or self-training (no teacher–student relationship; no pseudo-labels).

## 3.2 Meaning compression as the ratio view

Meaning compression is the **ratio view** of Identity (1). Let $\mathrm{raw}(D)$ denote the chosen unit of raw data — conventionally the number of inputs $n$, or a modality-specific unit such as "one sixteen-minute interview" for Section 4.2's ClipCannon instance. The **meaning-compression ratio** is

$$
\mathrm{MC}(\Phi, D) \;\triangleq\; \frac{|D'|}{\mathrm{raw}(D)} \;=\; N + \binom{N}{2} \;=\; \frac{N(N+1)}{2} \qquad \text{(per-input units).} \qquad (2)
$$

For Context Graph at $N = 13$, $\mathrm{MC} = 91$ signals per input (up to; Section 3.1); for ClipCannon at $N = 7$, $\mathrm{MC} = 28$ signals per input (up to).

**Positioning as a fourth category.** Conventional compression asks "for fixed semantic content, how few bits / weights / activations can I use?" Meaning compression flips the question: "for fixed raw-data volume, how many structured supervisory signals can I extract?" The pre-existing three categories are well-defined in the ML systems literature (Section 2.3). Bit compression — arithmetic coding, gzip, FLAC, PNG — measures bits of compressed output per bit of raw input (Delétang et al., 2024). Weight compression — post-training quantisation (GPTQ), one-shot pruning (SparseGPT), and distillation — measures task accuracy per trainable parameter. Activation compression — KV-cache quantisation, TurboQuant — measures peak runtime memory per unit of output quality.

The four categories operate on **semantically distinct unit pairs**, as Table 1 summarises.

| Category | Input unit | Output unit | Canonical techniques |
|---|---|---|---|
| Bit compression | bits of raw data | bits of compressed data | gzip, zstd, FLAC, PNG, arithmetic coding |
| Weight compression | trainable parameters | task accuracy at those parameters | GPTQ, SparseGPT, distillation |
| Activation compression | peak runtime memory | output quality at that memory | TurboQuant, KV-cache quantisation |
| **Meaning compression** | units of raw data | structured labelled signals | DDA (this paper) |

*Table 1. Four compression categories and their unit pairs. Meaning compression is proposed as a fourth entry; we make no claim it supersedes the other three, and we note that reviewer acceptance of the taxonomic distinction is a scholarly judgement we do not pre-empt.*

The unit pairs are **semantically distinct**: what is counted in numerator and denominator differs category-to-category. A corpus compressed with gzip loses no information and produces no additional supervisory signals. A model pruned with SparseGPT is a different model on the same training data. TurboQuant changes what must be held in memory during a forward pass but not the signal density driving training. Meaning compression changes the **training-data side** at a raw corpus held constant. The four are therefore **complementary axes** along which a pipeline can be compared, not competing alternatives — a pipeline can realise all four simultaneously. We refrain from "dimensionally distinct"; the claim is the narrower semantic one. The quantitative question — whether DDA-enriched supervision improves sample efficiency in the sense of refitted Kaplan- or Chinchilla-style exponents (Kaplan et al., 2020; Hoffmann et al., 2022) — is named future work; we defend the counting identity, the ratio view, and the taxonomic proposal as an axis-extension of the compression-as-intelligence line (Huang et al., 2024; Delétang et al., 2024), not as a re-fit result.

## 3.3 Scope of the collapse-silent claim

The recursive self-training literature — Shumailov et al. (2024) in *Nature*, Dohmatob et al. (2024) on modified scaling laws, Alemohammad et al. (2023) on model autophagy — establishes an irreversible distributional drift under a **generator-in-loop recursion**. The precondition across every cited theorem and simulation is structural: there exists a sequence of corpora $\{D_t\}_{t \geq 0}$ such that $D_{t+1}$ contains samples drawn from a generator trained on $D_t$, i.e. $D_{t+1} \ni \tilde x \sim p_{\theta_t}$.

**Scope claim.** DDA pipelines do not instantiate this recursion. By Axiom 1, each $\phi_m(x)$ in $D'$ depends only on a real input $x \in D$ and on the frozen parameters of $\phi_m$; it does not depend on any generator trained on $D$ or on a predecessor corpus. The derived dataset is a decomposition of the real corpus, not a sample from a generator. The generator-in-loop precondition of the cited collapse theorems is therefore not satisfied by construction. DDA is **structurally not subject to**, rather than a refutation of, the Shumailov-form recursion regime.

This is a **scope argument, not an immunity theorem**. We claim the preconditions of the cited theorems are unsatisfied for DDA; we do not claim a new independence-from-collapse result. Embedder-mediated data selection — using $\phi$-scored similarity to filter or re-weight $D$ before training — is a different feedback structure that the cited theorems do not cover, and we flag it as out of scope rather than included in the silence. The scope argument does load-bearing work for the three-paths framing of Section 1: accumulation (Gerstgrasser et al., 2024) and verification (Feng et al., 2024) — the two cited mitigations — still operate on samples drawn from a generator trained on a prior round's data, and neither has a theoretical place for the configuration in which a fixed corpus $D$ is held constant and $N$ frozen independently-trained embedders compute deterministic projections $\phi_m(x)$. The cited theorems are silent on this configuration; the third path (decompose a fixed corpus through frozen embedders) is what the silence names.

## 3.4 From DDA to TCT: the three-role composition

The same frozen embedder panel $\Phi$ that produces $D'$ in DDA is the object TCT reuses at the method level. Section 5 specifies the TCT phases formally. Here we state the structural move that connects the two constructs: under Axiom 1, a fixed panel $\Phi$ can serve three distinct semantic roles simultaneously, over **identical parameter instances**, without any parameter update between or during the roles:

1. **Retriever.** In the Context Graph substrate (Section 4.1), each $\phi_m$ ranks stored content by cosine similarity against a query projection. This is the canonical retrieval role of contrastive-geometry literature (Khattab & Zaharia, 2020; Malkov & Yashunin, 2018; Cormack et al., 2009).
2. **Labeller.** In DDA (Sections 3.1, 4.1, 4.2), each $\phi_m$ evaluates the fixed corpus $D$ to produce per-embedder projections and, with companion embedders, the pairwise interaction features — the $n \cdot (N + \binom{N}{2})$ structured supervisory signals of Identity (1). This role populates $D'$.
3. **Runtime guard.** In TCT Phase 3 (Section 5.4), each $\phi_m$ evaluates a generated candidate $\hat y$ and computes $\cos(\phi_m(\hat y), c_m)$ against a frozen centroid $c_m$ built in Phase 1. The acceptance predicate $G_\tau(\hat y; C_P) = \mathbb{1}[\forall m : \cos(\phi_m(\hat y), c_m) \geq \tau_m]$ is computed per generated output.

All three roles invoke the same frozen function and none updates its parameters. Under Axiom 1 none is generator-coupled, so the three compose without mutual interference. DDA uses $\Phi$ as labeller; Context Graph uses $\Phi$ as retriever; TCT uses $\Phi$ as runtime guard. To our knowledge in the locked 88-citation pool that anchors this manuscript, no cited paper uses the same frozen embedder panel in all three roles jointly, over identical parameter instances, without any parameter update between roles. The novelty is pool-relative (Section 2); a pool-external counter-example would qualify the framing but would not invalidate the composition-level claim for the specific systems this manuscript reports. Section 4 describes Context Graph and ClipCannon as constructive evidence that the labeller role is implementable at production scale; Section 5 specifies the TCT three-phase protocol under which the same panel becomes the per-output verifiability mechanism, with the two theorems following from the Axioms stated here rather than from any result in Section 2.
