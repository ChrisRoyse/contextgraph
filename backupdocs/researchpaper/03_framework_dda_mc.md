# 3. The Framework: Derived Data Abundance and Meaning Compression

## 3.1 Informal statement

Given a fixed corpus of raw data, the quantity of independently-labeled, information-theoretically-orthogonal training signal that can be extracted from it grows multiplicatively with the number of frozen, independently-trained embedding models applied to it. Each embedder, by virtue of having been trained on a different objective or dataset, measures a different dimension of the content. Each dimension of measurement is a new labeled training signal. The data itself does not change; what changes is the labeled representation available for downstream training.

We name this principle **Derived Data Abundance (DDA)**. We name the underlying operation **meaning compression**: a transformation that increases the meaningful-signal-per-unit-of-raw-data ratio of a dataset, in direct parallel to the way that bit-compression reduces the byte count required to represent a fixed payload, weight-compression reduces the parameter count required to represent fixed capability, and activation-compression reduces the memory required to represent fixed working state.

## 3.2 Setup and notation

Let $\mathcal{D} = \{x_1, x_2, \dots, x_n\}$ be a corpus of raw data points (text, video, audio, image, or any other medium). Let $\{\phi_1, \phi_2, \dots, \phi_N\}$ be a set of $N$ frozen embedding models, each $\phi_m : \mathcal{X} \to \mathbb{R}^{d_m}$ (possibly with sparse or variable-length output spaces), with no shared parameters between them. Let $\mathcal{L}$ be any downstream learning algorithm that consumes labeled training samples.

The *standard* training-data inventory presented to $\mathcal{L}$ from corpus $\mathcal{D}$ is just $\mathcal{D}$ itself: $n$ raw data points, each treated as a single training example (typically with a single-dimensional target or a self-supervised objective that runs on $x_i$ alone).

The *derived* training-data inventory available to $\mathcal{L}$ from $\mathcal{D}$ and $\{\phi_m\}$ is the set of tuples
$$
\mathcal{D}' = \{(x_i, \phi_1(x_i), \phi_2(x_i), \dots, \phi_N(x_i), \rho_{jk}(\phi_j(x_i), \phi_k(x_i)))\}_{i=1}^{n}
$$
where $\rho_{jk}$ is a cross-correlation or pairwise-interaction functional between the $j$-th and $k$-th embeddings. The derived inventory carries:

- $n$ raw data points, unchanged;
- $nN$ per-embedder labeled representations (each $\phi_m(x_i)$ is a labeled vector in $\mathbb{R}^{d_m}$);
- $n \binom{N}{2}$ pairwise-interaction features (each $\rho_{jk}(\phi_j(x_i), \phi_k(x_i))$ is a scalar or low-dimensional vector describing the cross-embedder relationship).

The total count of independently-labeled signals per data point is therefore $N + \binom{N}{2}$. For $N=13$ (Context Graph), this is $13 + 78 = 91$ signals per input. For $N=7$ (ClipCannon), this is $7 + 21 = 28$ signals per input.

## 3.3 Why this is not synthetic data

The critical distinction between DDA and synthetic-data approaches is the source of the additional labeled samples.

Synthetic-data pipelines sample from a generative model $G_\theta$ trained on real data, and feed the samples $G_\theta(x_i)$ back into training. The samples are *generated approximations* of what data might look like; they compress the tails of the real distribution and amplify the modes of whatever generator produced them. Iterating this process produces Shumailov et al. (2024)'s model collapse: the tails disappear, and the distribution variance collapses toward a point-mass estimator.

DDA pipelines do not generate anything. The embedders $\phi_m$ are frozen measurement instruments. They never see their own outputs fed back as training data. They produce, for each real input $x_i$, a measurement $\phi_m(x_i)$ that is an information-theoretically-structured summary of $x_i$. These measurements are as grounded in reality as $x_i$ itself; they are different projections of the same underlying signal.

The structural analogy: synthetic data is like photocopying a document repeatedly, each copy a little blurrier than the last. DDA is like measuring the physical properties of the original document with many different instruments — each measurement adds a new, grounded attribute to the record without degrading the original.

This has three consequences:

1. **No collapse dynamics apply.** The Shumailov et al. (2024) result requires recursive training on generated data. If nothing is being generated, the theorem is silent on DDA pipelines, and the collapse mechanism simply does not have a foothold.
2. **The tails are preserved.** A rare facial expression that appears once in 16 minutes of video produces one rare visual embedding; that embedding faithfully carries the rarity forward rather than regressing it to the mean.
3. **The labels are automatic and grounded.** Unlike synthetic pipelines which require verifiers to filter for realism, DDA labels are measurements of real observations and are, by construction, as realistic as the data they describe.

## 3.4 Independence and the information content of additional embedders

The multiplicative gain claimed by DDA depends on the $\phi_m$ being genuinely independent in the sense that their outputs do not encode redundant information about $x_i$. If two embedders produce highly correlated representations, the effective signal gain from adding the second is reduced.

For the systems we describe in Sections 4–5, the embedders are drawn from different base architectures (BERT-family, Qwen2, NomicBERT, WavLM, SigLIP, ColBERT, SPLADE, custom temporal/HDC models) and were trained on largely disjoint data for different objectives. We therefore treat their outputs as approximately independent for the purposes of the DDA argument, acknowledging that quantifying this independence formally is an open problem (see Section 10).

Empirically, the independence claim is supported by the *blind-spot* phenomenon Context Graph exposes through its `search_cross_embedder_anomalies` tool (Section 4.7): for any query, there exist memories that score highly in one embedder but poorly in another, indicating that the embedders are not redundant. If they were, no blind spot could exist.

## 3.5 Meaning compression as a compression category

Every compression algorithm in modern AI operates along the same axis: reducing the number of bits required to represent a fixed amount of information.

| Category | What it compresses | Representative techniques | Measurement |
|---|---|---|---|
| **Bit compression** | Raw storage of fixed information | Huffman coding, gzip, PNG, FLAC | bits out / bits in |
| **Weight compression** | Parameter cost of learned knowledge | Quantisation (e.g., GPTQ), pruning (e.g., SparseGPT), distillation | accuracy at reduced parameter count |
| **Activation compression** | Memory cost of inference-time working state | KV-cache quantisation (e.g., TurboQuant), prompt compression | memory reduction at fixed output quality |
| **Meaning compression (new)** | Density of labeled signal per unit of raw data | DDA, multi-modal embedding decomposition, constellation construction | meaningful training signals / unit raw data |

The axes are orthogonal. An advance in weight compression does not reduce the demand for bit compression of storage, and an advance in bit compression does not reduce the demand for efficient activation-state representation. Meaning compression is orthogonal to all three: it increases the training-value density of data *before* any of the other three categories enter the picture. A corpus that has been decomposed through 13 embedders is, for training purposes, much richer than the same corpus treated as raw text — and it occupies exactly the same number of bytes on disk for its raw payload (plus the embeddings, which are themselves compressible through bit- and weight-compression techniques).

## 3.6 Why meaning compression is the most economically valuable

A concrete economic argument: each level of the compression hierarchy operates on a progressively more fundamental bottleneck.

- **Bit compression** saves storage and bandwidth — measured in pennies per terabyte.
- **Weight compression** saves inference compute and VRAM — measured in dollars per million tokens served.
- **Activation compression** saves inference memory — measured in dollars per concurrent user.
- **Meaning compression** saves *training*. Frontier model training runs now cost US$1–10 billion. The next generation is projected to cost US$10–100 billion. If these costs are driven primarily by the need for more data, more compute, and more parameters to achieve incremental capability gains, and if the supply of the first factor is structurally constrained (Section 1), then any technology that multiplies the effective training corpus by 10×–100× without generating synthetic data addresses the single most expensive bottleneck in the field.

The TurboQuant announcement (Google Research, 2026; hypothetical placeholder cited in the accompanying evidence brief) represents a best-in-class instance of activation compression and briefly affected memory-chip equities on the strength of its inference-cost implications. Meaning compression, if the argument in this paper is right, addresses a bottleneck that is one to two orders of magnitude more expensive.

(We note that the TurboQuant reference is unverified as of this writing; the paper's argument does not depend on that specific system. The compression-taxonomy argument holds against any activation-compression technique.)

## 3.7 The sample-complexity claim

DDA also has a sample-complexity implication that we exploit in all three empirical case studies.

Conventional identity-locked training typically asks for thousands to millions of subject-specific examples to lock the generative target to a specific person or style. Under TCT, the training target is not "produce outputs that look like subject X" (a distribution to match) but "produce outputs that fall within cosine threshold $\tau_m$ of centroid $c_m$ in modality $m$, for every $m$" (a geometric constraint). The centroid is constructed once, from however much reference data is available, and it does not move during training. The model's job is to stay near it.

The number of training examples needed to learn to stay near a frozen centroid in a structured embedding space is dramatically smaller than the number needed to learn a full distribution. This is what explains our Case 2 result: a 16-minute interview provides enough material to build seven per-modality centroids of sufficient quality (e.g., 1,725 visual samples, 343 emotion samples, 300 speaker samples), and 2,362 LoRA-curated clips are enough to fine-tune a generator to stay within the centroid neighbourhoods.

## 3.8 When DDA does not apply

DDA requires that:

1. **Independent embedders exist for the dimensions of interest.** A domain whose relevant content dimensions are not captured by any frozen embedding model cannot be decomposed through DDA. The framework is limited by the frontier of available measurement instruments.
2. **The training target is expressible as proximity to embeddings.** TCT in particular requires that "correct output" can be operationalised as proximity to a reference centroid. Open-ended generative objectives (e.g., "be helpful and harmless in a novel conversational context") do not naturally factor through this constraint.
3. **The compute budget for decomposition is acceptable.** Running a corpus through 13 embedders costs 13× the compute of running it through one. At internet scale this is substantial. We argue the cost is small relative to the training-run cost savings from multiplicatively larger effective datasets, but the trade-off is not free.

Section 10 returns to these limitations in detail.

## 3.9 Summary

| Claim | Status |
|---|---|
| Embedders extract information-theoretically-different dimensions of meaning | Empirically supported; approximation of orthogonality — open formal question |
| Derived data scales multiplicatively in embedder count | Construction (Section 3.2) |
| Derived data is not synthetic and therefore does not collapse | Structural argument (Section 3.3); no feedback loop exists |
| Meaning compression is a distinct compression category | Definitional (Section 3.5); the axes of bit/weight/activation compression are formally orthogonal to meaning compression |
| Meaning compression is the highest-value category because it addresses training-side data scarcity | Economic argument (Section 3.6); depends on how much of frontier-training cost is driven by data scarcity vs. compute scarcity |
| DDA reduces sample complexity for identity-locked generation | Empirical claim validated in Sections 7–9 |

The remainder of the paper presents the systems that realise DDA at production scale and the empirical validation that supports the sample-complexity claim.
