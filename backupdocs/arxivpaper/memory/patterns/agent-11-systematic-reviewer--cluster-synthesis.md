---
namespace: patterns
agent_id: agent-11-systematic-reviewer
timestamp: 2026-04-16T21:30:00Z
status: active
supersedes: none
tags: narrative-synthesis, per-cluster, paste-ready, section-2, related-work
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
downstream_consumers:
  - agent-12-context-tier-manager
  - agent-36-related-work-writer
---

# Narrative Synthesis Per Cluster — Paste-Ready Building Blocks for §2

Each synthesis is 2–3 sentences. Citations are by key (agent-08 master pool).
All prose is written to be *assembly material* for agent-36's §2 Related Work;
copy-edit welcome, but the factual and framing content is locked.

---

## Cluster A — Scaling laws + the data wall

Scaling-law theory (kaplan2020scaling; hoffmann2022chinchilla) predicts a power-law
relation between loss and compute / parameters / tokens, with Chinchilla's
compute-optimal ~20 tokens-per-parameter prescription widely replicated. The data
wall — the concern that the stock of high-quality human-generated text is bounded
and will be exhausted between 2026 and 2032 under current training
demand — is quantified by villalobos2024data (stock estimate) and
longpre2024consent (accessible-fraction decay driven by robots.txt / TDM opt-outs).
Dated industry signals (woo2024information; metz2024bloomberg; hu2024reuters;
franzen2024techcrunch; sutskever2024neurips oral) corroborate the framing that
pure-compute-and-tokens scaling is showing diminishing returns, though these
signals are cited here only for dating, not as primary evidence.

---

## Cluster B — Model collapse + synthetic data

Model collapse — the failure mode in which iteratively training a generator on
its own outputs causes tail-mass loss followed by variance contraction — is
established across closed-form and simulation work (shumailov2024nature in
*Nature*; alemohammad2023mad ICLR; dohmatob2024tails ICML and
dohmatob2024regression NeurIPS). Two mitigation lineages exist: real-data
accumulation (gerstgrasser2024colm) bounds the degradation, and oracle
verification of synthetic samples (feng2024verification) restores scaling. All
cited results share a **generator-in-loop** precondition; the theorems are
silent on configurations where supervision is derived from frozen independently
trained embedders rather than samples drawn from a generator.

---

## Cluster C — Compression-as-intelligence

A tight prediction–compression duality makes a good next-token predictor
equivalent to a good arithmetic-coded compressor (deletang2024compression ICLR;
valmeekam2023llmzip); this lineage extends to image / audio / video via
li2025lossless (*Nature Machine Intelligence*) and downward to compressor-based
classification (jiang2023gzip ACL Findings). huang2024compression (COLM) reports
an approximately linear correlation between compression ratio and LLM capability
across 30+ models (r ≈ 0.95, single-source; hedge required). The cluster reads
compression *from a trained model* on held-out sequences; it does not develop a
cross-compressor interaction theory or use compression as a *labeller* at
training-data preparation time.

---

## Cluster D — Multi-modal embedders + retrieval

A wide embedder family — contrastive (zhai2023siglip sigmoid CLIP;
nussbaum2024nomic long-context text), speech self-supervised (chen2022wavlm IEEE
JSTSP), late-interaction (khattab2020colbert SIGIR, santhanam2022colbertv2
NAACL), sparse lexical (formal2021splade SIGIR), and knowledge-graph
(wang2021kepler TACL, bordes2013transe NeurIPS) — is supported by approximate-
nearest-neighbour infrastructure (malkov2018hnsw IEEE TPAMI) and rank-fusion
(cormack2009rrf SIGIR). Across the cluster, embedders are evaluated as
**rankers / retrievers**: top-k accuracy, NDCG, EER. No cited paper proposes a
runtime per-output cosine-gate against a frozen centroid as an acceptance
predicate for generation, and no paper quantifies labelled-signal yield from N
embedders; this is the novelty space for the DDA role-flip and the
constellation guard.

---

## Cluster E — Zero-shot TTS + voice cloning

Zero-shot TTS frontier systems — neural-codec (wang2023valle, chen2024valle2),
factorised-codec diffusion (tan2024naturalspeech3 ICML,
shen2023naturalspeech2), masked generative codec (wang2024maskgct ICLR),
flow-matching (chen2024f5tts, le2023voicebox, lipman2023flowmatching) and
open-artefact systems (casanova2024xtts) — achieve speaker similarity in the
SIM-o ≈ 0.64 / WavLM-cosine ≈ 0.86–0.89 band under single-prompt conditioning.
Evaluation is anchored on fixed scorers (reddy2022dnsmos ICASSP for P.835;
saeki2022utmos Interspeech for UTMOS; desplanques2020ecapa Interspeech
ECAPA-TDNN for speaker verification; chen2022wavlm for WavLM-SECS), plus open
benchmarks (christop2025cloneval). No cited system aggregates a multi-clip
L2-normalised centroid as an alignment target, applies a runtime best-of-N
cosine-gate at inference, or enforces cross-encoder SECS (scoring encoder
distinct from the conditioning encoder); these are the Case-3 methodological
novelties.

---

## Cluster F — Identity-locked video generation

Identity-locked human-animation systems combine flow-matching
(lipman2023flowmatching, meng2025echomimicv3 EchoMimicV3 1.3B, wan2025video) or
diffusion-transformer video generation (yang2024cogvideox, cui2024hallo3,
zheng2025hunyuanvideo, tian2024emo, tian2025emo2), with LoRA
(hu2022lora ICLR) adapting pretrained backbones on subject-specific data and
LatentSync (li2024latentsync) providing lip-sync supervision; statistical facial
geometry (li2017flame ACM TOG) and action-unit decomposition (ekman1978facs)
supply the structural identity substrate. Evaluation is typically LSE-D /
LSE-C / FVD / CSIM reported after generation on a test set. No cited system
constructs a multi-modal frozen-centroid identity target or applies a runtime
per-modality cosine-gate at inference, and the flow-matching sign-convention
ambiguity (target = noise − latent vs. latent − noise) is not a documented
correctness condition in the cluster's published work.

---

## Cluster G — Auxiliary pipeline components

ClipCannon's 23-stage DAG composes well-documented components: wav2vec 2.0
speech pretraining (baevski2020wav2vec NeurIPS 2020), time-accurate ASR with
WhisperX (bain2023whisperx Interspeech 2023), music source separation with
HTDemucs (rouard2023htdemucs ICASSP 2023), density-based and hierarchical
clustering (campello2013hdbscan PAKDD 2013; zhang1996birch ACM SIGMOD 1996), and
open-source face / software tooling (google2020mediapipe, tinachen2021yolov5face,
resembleai2024enhance), orchestrated via the Model Context Protocol
(mcp2024spec). Each component is individually well-evaluated against its own
domain's metric, but no cited work reports end-to-end composition quality on a
single-subject 16-minute corpus producing 12,000+ labelled samples across 7
modalities — this pipeline-composition claim is a system-paper contribution
rather than a novel methodology within Cluster G.

---

## Cluster H — Enterprise / market signals (body quota ≤ 6)

The enterprise-market context for the "license more data" path is covered by
dated industry signals (reuters2024reddit, cerullo2024cbs the Reddit-Google
deal; openai2024reddit, openai2024newscorp, variety2024newscorp, verma2023washpost
OpenAI's publisher deals; shutterstock2023openai image-training partnership) and
vocabulary / data infrastructure (harvard2024caselaw, nci2024thesaurus,
nlm2024mesh, snomed2024ct, sec2024edgar, sec2024edgarsearch). Peer-reviewed
anchors for the sub-claim that domain-specialised LLMs instantiate commercially
viable data strategies include singhal2023nature and tu2025nature
(*Nature* / *Nature Medicine* clinical), wu2023bloomberggpt (financial), and
henderson2022pileoflaw (consent-aware legal corpus, NeurIPS 2022 D&B). This
cluster's role in the manuscript is sidebar-framing only; body quota is ≤ 6
citations, with T1/T2 entries (H05, H13, H15, H20) prioritised to preserve the
80% T1+T2 body-wide floor.

---

## Cluster I — Alignment (RLHF / DPO / Constitutional AI)

Alignment of generative language models proceeds via **scalar-reward**
optimisation (ouyang2022instructgpt NeurIPS 2022 InstructGPT; rafailov2023dpo
NeurIPS 2023 DPO closed-form; bai2022constitutional AI-feedback). The theoretical
primitive is a reward model — learned from human preferences (InstructGPT),
re-expressed as an implicit-reward in closed form (DPO), or generated by an AI
simulator under natural-language rules (Constitutional AI) — optimised under a
KL-to-reference-policy regulariser. No cited alignment method uses a **frozen
geometric target**, applies a **per-output runtime acceptance predicate**, or
**conjoins multiple independent modalities** simultaneously; these three
structural absences are the points on which Teleological Constellation Training
departs from the Cluster I paradigm.

---

## Synthesis Q-roll-up

| Cluster | Q-IDs served | Body-placement | Novelty contribution to our paper |
|---|---|---|---|
| A | Q01, Q03, Q15, Q16 | §1 §2.1 | DDA as EXT of the data axis |
| B | Q02, Q03, Q16 | §2.2 §8 | DDA as ORTH (collapse-silent) |
| C | Q04, Q05 | §3.1 | Meaning compression as CAT-EXT |
| D | Q04, Q08, Q09, Q10 | §2.4 §3.4 §4.1 | Embedder role-flip + constellation-guard |
| E | Q08, Q09, Q12, Q14, Q20 | §2.5 §6.3 | Centroid + runtime guard + cross-encoder SECS |
| F | Q11, Q13, Q17 | §2.5 §6.2 | Multi-modal frozen-centroid + per-output gate + sign-convention forensic |
| G | Q10, Q11, Q18 | §4.2 | Pipeline-composition claim |
| H | Q19 | §2.1 (path 1) §8.6 | Enterprise-sidebar framing only |
| I | Q06 | §2.6 §5 | TCT as BRK relative to scalar-reward |

## Confidence

**HIGH.** Every synthesis names ≥ 3 primary citations from the LOCKED master
pool; the novelty-delta framing is consistent with agent-10's EXT/COMP/BRK/ORTH
grid and agent-09's methodological gaps. Paste-ready prose; agent-36 may copy
in whole or re-structure per §2 G3 novelty-delta table.

## Changelog

- 2026-04-16 — Initial issue by agent-11-systematic-reviewer.
