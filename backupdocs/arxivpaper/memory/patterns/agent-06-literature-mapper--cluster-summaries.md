---
namespace: patterns
agent_id: agent-06-literature-mapper
timestamp: 2026-04-16T16:40:00Z
status: active
supersedes: none
tags: related-work, cluster-summaries, paste-ready, section-2, literature
depends_on:
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/discoveries/agent-06-literature-mapper--structural-holes.md
  - memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md
  - memory/patterns/agent-04-construct-definer--definition-blocks-for-writers.md
  - memory/decisions/agent-05-research-planner--section-ownership.md
consumers:
  - agent-36 (literature-review-writer, §2)
  - agent-35 (introduction-writer, §1, background sentences)
  - agent-37 (methodology-writer, §3 / §5 background paragraphs)
---

# Cluster Summaries — Paste-Ready for §2 Related Work

These summaries are **usable verbatim** in §2. Each is 2–3 sentences per the agent-06
tasking-table row. Vocabulary is canonical per the locked glossary; citations refer to
keys that agent-08 will canonicalise into APA 7th in the reference database.

**Downstream writers MUST NOT paraphrase.** If a writer disagrees with a summary, file
a blocker at `./memory/blockers/agent-06-summary-conflict-with-{agent-id}.md`, do not
silently edit. Vocabulary-drift (FM-09) is the most common failure mode this file
defuses.

---

## Cluster A — Scaling Laws + The Data Wall (§2.1)

Prior work on scaling laws (Kaplan et al., 2020; Hoffmann et al., 2022) establishes that
LLM loss follows a power law in parameters, tokens, and compute, with compute-optimal
allocation requiring roughly 20 tokens per parameter. Villalobos et al. (2024) extend
this to a supply-side forecast: the stock of accessible high-quality public human text
is approximately 300 trillion tokens, projected to be fully consumed by frontier training
between 2026 and 2032, with overtraining accelerating exhaustion by up to 5×. Longpre
et al. (2024) add that the *accessible* fraction of the web corpus is itself shrinking
— roughly 25% of high-quality C4 tokens were restricted by robots.txt and ToS changes
in 2023–2024 — and industry signals in late 2024 (Woo & Efrati, 2024; Franzen, 2024;
Reuters, 2024) report that pre-training on more of the same corpus is yielding
diminishing returns.

---

## Cluster B — Model Collapse + Synthetic Data (§2.2)

Shumailov et al. (2024) show in *Nature* that generative models trained recursively on
their own outputs undergo irreversible distribution drift: early collapse erases the
tails of the original distribution, late collapse reduces variance toward a point mass,
and this dynamic is universal across Gaussian mixtures, VAEs, and LLMs. Subsequent work
proposes two families of mitigations: **accumulation** (Gerstgrasser et al., 2024;
Alemohammad et al., 2023), in which each generation's synthetic data is added to — not
substituted for — the real corpus, and **verification** (Feng et al., 2024), in which
synthetic samples are filtered through an oracle or learned reward model before
training. Both families operate strictly inside the generator-feedback loop; both
concede that the loop itself is the hazard (Dohmatob, Feng, Yang et al., 2024; Seddik
et al., 2024).

---

## Cluster C — Compression-as-Intelligence (§2.3 and §3.1 background)

A recent line of work formalises the long-observed equivalence between prediction and
compression: Delétang et al. (2024) show that a pretrained Chinchilla-70B compresses
ImageNet to 43% and LibriSpeech to 16.4% of raw size via arithmetic coding, beating
PNG and FLAC; Huang et al. (2024) report a near-linear correlation (r ≈ 0.95) between
compression ratio and downstream benchmark performance across 30+ LLMs; and Li et al.
(2025) report ~2× better compression than gzip/bzip2 with LLM arithmetic coding across
text, image, audio, and video. These results position information density — not token
count — as the operative quantity, but they read compression *from a trained model*
and do not prescribe compression *of the training corpus itself* through multiple
frozen embedders. The three established compression categories (bit, weight, activation;
Frantar et al., 2022; Frantar & Alistarh, 2023) all measure economy of output relative
to an input of the same modality.

---

## Cluster D — Multi-Modal Embedders + Retrieval (§2.5 first half)

The embedders this paper composes are drawn from a well-developed literature: vision-
language (SigLIP, Zhai et al., 2023), open text (Nomic Embed, Nussbaum et al., 2024),
speech and speaker identity (WavLM, Chen et al., 2022), late-interaction text retrieval
(ColBERT, Khattab & Zaharia, 2020; ColBERTv2, Santhanam et al., 2022), sparse lexical
expansion (SPLADE, Formal et al., 2021), and knowledge / entity embedding (KEPLER,
Wang et al., 2021; TransE, Bordes et al., 2013). Retrieval over multiple such embedders
is typically organised as ranking fusion (reciprocal rank fusion, Cormack et al., 2009)
over approximate-nearest-neighbour indexes (HNSW, Malkov & Yashunin, 2018), where the
embedders' outputs are the endpoint of the pipeline and their role is *similarity
scoring at query time*.

---

## Cluster E — Zero-Shot TTS + Voice Cloning (§2.6 and §6.3 background)

Zero-shot voice cloning is conventionally framed as in-context learning from a single
speaker prompt: VALL-E (Wang et al., 2023) introduced the neural-codec language-model
formulation; VALL-E 2 (Chen et al., 2024) declared "human parity" on LibriSpeech
continuation via SIM / WER / DNSMOS parity with reported WavLM-large SV cosine around
0.881, and successor systems cluster in the 0.86–0.89 WavLM band (NaturalSpeech 3,
Tan et al., 2024; MaskGCT, Wang et al., 2024; F5-TTS, Chen et al., 2024; Voicebox,
Le et al., 2023; Mega-TTS 2, Jiang et al., 2023; XTTS, Casanova et al., 2024). These
systems are evaluated *post-generation* by speaker-encoder cosine similarity (WavLM,
Chen et al., 2022; ECAPA-TDNN, Desplanques et al., 2020) and by naturalness predictors
(DNSMOS P.808 / P.835, Reddy et al., 2022; UTMOS, Saeki et al., 2022), and none of the
cited systems aggregates a multi-clip reference set into a frozen centroid or applies
a per-utterance runtime cosine gate against one.

---

## Cluster F — Identity-Locked Video Generation (§2.5 second half and §6.2 background)

Audio-driven identity-locked portrait animation is an active area: EchoMimicV3 (Meng
et al., 2025) — the base model for our Case 2 — uses a 1.3B-parameter flow-matching
diffusion transformer with the convention $\text{target} = \text{noise} - \text{latent}$;
Hallo3 (Cui et al., 2024), LatentSync (Li et al., 2024), HunyuanVideo-Avatar (Zheng
et al., 2025), and EMO / EMO2 (Tian et al., 2024, 2025) occupy adjacent design space;
text-to-video foundation models (CogVideoX, Yang et al., 2024; Wan, Wan Team, 2025)
supply the broader lineage. Subject-specific adaptation is typically handled by
low-rank adaptation (LoRA, Hu et al., 2022) on top of a flow-matching loss (Lipman
et al., 2023), conditioned on one or a few reference images plus audio; facial-shape
and facial-action modelling is grounded in FLAME (Li et al., 2017) and FACS (Ekman &
Friesen, 1978). None of the cited systems holds a multi-modal centroid set frozen
through training and inference or applies a runtime per-modality cosine gate against
it at generation time.

---

## Vocabulary-compliance checklist (for agent-36's §2 QG7 pass)

These cluster summaries use canonical vocabulary only:

- "the data wall" (lower-case; glossary #26) — Cluster A.
- "synthetic data" (glossary #28 adjacent) — Cluster B.
- "bit, weight, activation" compression categories (glossary #2 NOT-clause) — Cluster C.
- "embedders" (not "encoders"; glossary #5) — Cluster D.
- "centroid" / "reference set" (not "prototype"/"anchor"/"training set"; glossary #7/#9)
  — Clusters E, F.
- "zero-shot" and "post-generation" (not "after training"; precise TTS vocabulary) —
  Cluster E.
- "identity-locked" used only where the cited systems *do* identity-lock; we do not
  claim they use the TCT construct (glossary #4) — Cluster F.

No forbidden substitutions. No "semantic compression", no "prototype bank", no
"personalised generation" for the cited comparators' methods.

---

## Change log

- 2026-04-16 — Initial issue by agent-06-literature-mapper.
