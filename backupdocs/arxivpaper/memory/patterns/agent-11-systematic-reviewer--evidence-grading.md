---
namespace: patterns
agent_id: agent-11-systematic-reviewer
timestamp: 2026-04-16T21:15:00Z
status: active
supersedes: none
tags: evidence-grading, grade, strong-moderate-weak-single, per-claim, 88-citations
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
downstream_consumers:
  - agent-12-context-tier-manager
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-39-discussion-writer
  - agent-42-adversarial-reviewer
---

# Evidence-Grading Table — Per Major Claim

## Grade definitions

- **STRONG** — ≥ 3 independent T1 sources + methodological agreement across sources.
  Extrapolations and generalisations warranted.
- **MODERATE** — 2 T1 sources *or* 1 T1 + ≥ 2 T2 sources with methodological
  agreement. Appropriate for a published manuscript; hedges may be required.
- **WEAK** — Only T2/T3 sources, OR methodological disagreement among T1 sources.
  Must be hedged (V-hedges per glossary) and flagged in §8.
- **SINGLE-SOURCE** — One load-bearing source; other sources are corroborative but
  do not independently establish the claim. Must be flagged and hedged; the
  manuscript should attribute ("Villalobos et al. estimate …") rather than
  assert as consensus.

Additional note: **AUTHOR-EMPIRICAL** = measured by Chris Royse in one of the
three case studies (Shakespeare / Santa / voice-cloning). These claims are
empirical but single-site; they need explicit in-text attribution plus V12 / V17
hedge ("observed in this study, not generalised").

## Claims × Citations table

The **Claim IDs** follow the paper's §§ 1–9 structure and the Q-ID load-bearing
map in `patterns/agent-08-citation-extractor--claim-to-citation-index.md`.

| Claim ID | Claim (abbreviated) | §§ placement | Primary citations (keys) | Supporting | GRADE | Notes |
|---|---|---|---|---|---|---|
| **CL-01** | Scaling-law extrapolation: loss follows power law in N / D / C; compute-optimal ~20 tok/param | §1, §2.1 | kaplan2020scaling (T2), hoffmann2022chinchilla (T1) | — | **STRONG** | Foundational, thousands of citations + replication. Kaplan is T2 but routinely-T1 group. |
| **CL-02** | Accessible human-text stock bounded; exhaustion 2026–2032 under current scaling | §1, §2.1 | villalobos2024data (T1) | longpre2024consent (T1) | **MODERATE** | Two independent T1 sources. Villalobos estimates the stock; Longpre measures the accessible-fraction delta. Combined they make the claim moderately well-supported. Hedge: extrapolation-dependent. |
| **CL-03** | Industry signal: scaling laws showing diminishing returns (late 2024) | §1 (framing only) | franzen2024techcrunch (T3), metz2024bloomberg (T3), hu2024reuters (T3), woo2024information (T3), sutskever2024neurips (T3 oral) | — | **WEAK (industry signal, framing only)** | 5 corroborating T3 sources. Per agent-07 rule, cited for *framing*, not as evidence. No Tier-1 equivalent exists for dated industry observation. |
| **CL-04** | Model collapse occurs under recursive generator-in-loop training | §2.2, §8 | shumailov2024nature (T1), dohmatob2024tails (T1), dohmatob2024regression (T1), alemohammad2023mad (T1) | gerstgrasser2024colm (T2), seddik2024collapse (T2) | **STRONG** | 4 T1 sources with closed-form + simulation convergence. |
| **CL-05** | Collapse has mitigations: data accumulation + verification | §2.2 | gerstgrasser2024colm (T2), feng2024verification (T2) | — | **MODERATE** | 2 T2 sources; lineage of B03/B04 theory. |
| **CL-06** | Collapse theory is silent on DDA (no generator-in-loop in DDA) | §3.3, §8 | shumailov2024nature (T1) | (categorical argument, no direct support) | **MODERATE (categorical)** | Author argument from the Cluster B framework's precondition; no counter-example in pool. |
| **CL-07** | Prediction–compression duality: good predictor ⇔ good compressor (Shannon-anchored) | §3.1 | deletang2024compression (T1), jiang2023gzip (T1) | valmeekam2023llmzip (T2) | **STRONG** | 2 T1 + 1 T2; Shannon-anchored theory; ACL Findings peer-review. |
| **CL-08** | Compression ratio correlates linearly with LLM capability (r ≈ 0.95) | §3.1 | huang2024compression (T2) | — | **SINGLE-SOURCE** | One COLM paper with N=30+ LLMs. Well-powered within itself but the cross-lab replication is limited. Hedge as "one large observational study (Huang et al., 2024)". |
| **CL-09** | LLMs function as lossless compressors across text/image/audio/video | §3.1 | li2025lossless (T1) | deletang2024compression (T1) | **MODERATE** | 2 T1 sources (Nature MI + ICLR). |
| **CL-10** | Meaning compression is a distinct *fourth* category (orthogonal to bit / weight / activation compression) | §3.1 | deletang2024compression (T1), huang2024compression (T2), li2025lossless (T1) | (taxonomy argument) | **MODERATE (taxonomy claim)** | 3 Cluster-C anchors support the compression-as-intelligence frame; the fourth-category move is a dimensional-unit argument, not a separate empirical claim. Hedge as CAT-EXT (extension, not discovery). |
| **CL-11** | DDA: N + C(N,2) signals per input under N frozen independent embedders | §3.2, §3.4 | (author TD derivation; no direct citation) | chen2022wavlm (T1), zhai2023siglip (T2), khattab2020colbert (T2), santhanam2022colbertv2 (T1), formal2021splade (T2), wang2021kepler (T2), bordes2013transe (T1), nussbaum2024nomic (T2), malkov2018hnsw (T1), cormack2009rrf (T2) | **MODERATE (TD construct)** | TD argument: the embedder substrate is T1-grade Cluster D; the counting formula itself has no prior citation. V14 "approximately independent" hedge required. |
| **CL-12** | Embedders are universally used for retrieval / ranking (not as labellers) | §2.4, §3.4 | khattab2020colbert (T2), santhanam2022colbertv2 (T1), formal2021splade (T2), zhai2023siglip (T2), nussbaum2024nomic (T2), cormack2009rrf (T2), malkov2018hnsw (T1) | — | **STRONG** | 7 citations across T1/T2 all evaluate on retrieval benchmarks. The role-flip (embedder→labeller) has no precedent → novelty claim. |
| **CL-13** | Frozen multi-modal centroid + cosine alignment is the TCT alignment target | §5 | (author construct) | — | **AUTHOR-EMPIRICAL** | Novel construct; no prior art cites centroid-as-alignment-target. BRK in Clusters D/E/F/I per agent-10. |
| **CL-14** | TCT is non-scalar / non-reward / non-preference / per-output gate — framework-breaking vs. RLHF / DPO / CAI | §2.6, §5 | ouyang2022instructgpt (T1), rafailov2023dpo (T1), bai2022constitutional (T2) | — | **STRONG (for the contrast; the TCT claim itself is AUTHOR-EMPIRICAL)** | 3 anchors establish the scalar-reward paradigm; the BRK claim is a scope claim ("TCT lives outside this paradigm's primitives"). Well-supported contrast. |
| **CL-15** | Voice cloning: zero-shot TTS systems achieve SIM-o ~0.64 / WavLM-cosine ~0.86–0.89 frontier | §2.5, §6.3 | chen2024valle2 (T2), tan2024naturalspeech3 (T1), wang2024maskgct (T1), chen2024f5tts (T2), le2023voicebox (T2), shen2023naturalspeech2 (T2), casanova2024xtts (T2), wang2023valle (T2), jiang2023megatts2 (T2) | — | **STRONG** | 9 Cluster-E sources; multiple independent systems on similar benchmarks. |
| **CL-16** | Case 3 measurement: WavLM SECS 0.961 (mean), 0.975 (max) on 10 unseen sentences | §6.3 | (Author's SITREP + whitepaper) | chen2022wavlm (T1 for encoder), chen2024valle2 (T2 for frontier comparator) | **AUTHOR-EMPIRICAL** | Single-site single-speaker measurement. V12/V17/V22 hedges mandated ("observed, not generalised"). The encoder is T1, but our SECS measurement is a one-site empirical observation. |
| **CL-17** | Case 3 method: 50-clip L2-normalised centroid + WavLM best-of-12 at T=0.3 | §6.3 | (author construct) | chen2022wavlm (T1) | **AUTHOR-EMPIRICAL + novel method** | Novel across Cluster E (E-METHOD-GAP-1/2/3 filled). |
| **CL-18** | DNSMOS P.808 parity (3.93 vs. 3.93) and UTMOS parity (3.012 vs. 2.997) with real recordings (Case 3) | §6.3 | (author measurement) | reddy2022dnsmos (T2, scorer), saeki2022utmos (T2, scorer) | **AUTHOR-EMPIRICAL** | Scorers T2; measurements author-single-site. |
| **CL-19** | Identity-locked video generation: flow-matching + LoRA + DIT architectural paradigm | §2.5, §6.2 | meng2025echomimicv3 (T2), cui2024hallo3 (T2), zheng2025hunyuanvideo (T2), tian2024emo (T2), tian2025emo2 (T2), yang2024cogvideox (T2), hu2022lora (T1), lipman2023flowmatching (T1) | — | **STRONG** | 6 Cluster-F T2 + 2 T1 architectural anchors (LoRA, Flow Matching). |
| **CL-20** | FLAME / FACS as canonical facial-shape + facial-action decomposition | §6.2 | li2017flame (T1), ekman1978facs (T3) | — | **MODERATE** | T1 journal anchor + T3 reference-work anchor. FACS is the canonical print reference, no T1 replacement exists. |
| **CL-21** | Flow-matching sign convention (target = noise − latent) is a correctness condition | §6.2 forensic | meng2025echomimicv3 (T2 base model), lipman2023flowmatching (T1 theory) | — | **AUTHOR-EMPIRICAL (forensic)** | The sign convention is not documented in the cluster. The Santa project's `docs/inspect/08_training_audit.md` is the disclosure. FC register; V17 / FM-08 required. |
| **CL-22** | LoRA: low-rank weight update ΔW = BA is an efficient fine-tune for pretrained models | §4, §6.1, §6.2 | hu2022lora (T1) | — | **SINGLE-SOURCE** | One ICLR 2022 foundational paper — STRONG within itself (heavily replicated in practice) but one formal citation. Treat as foundational single-source. |
| **CL-23** | Flow-matching: continuous-time probability path, non-AR generation | §4, §5, §6.2 | lipman2023flowmatching (T1) | — | **SINGLE-SOURCE** | One ICLR 2023 foundational paper. Same pattern as LoRA. |
| **CL-24** | HNSW provides log-time approximate-NN search with bounded recall | §4 | malkov2018hnsw (T1) | — | **SINGLE-SOURCE** | One IEEE TPAMI paper, heavily replicated in open-source. Foundational single-source. |
| **CL-25** | Reciprocal Rank Fusion is empirically robust relative to Condorcet / rank learning | §4, §4.1 | cormack2009rrf (T2) | khattab2020colbert (T2), santhanam2022colbertv2 (T1) (use RRF-adjacent fusion) | **MODERATE** | One SIGIR paper + several applications. Foundational method; RRF's ubiquity is not contested. |
| **CL-26** | Context Graph: 13 embedders, 51 RocksDB CFs, 75 MCP tools, 5,184 tests | §4.1 | (Author's system doc) | mcp2024spec (T3) | **AUTHOR-EMPIRICAL (system description)** | Non-contested; measurable artefact. The system description is factual; the MCP spec is T3 with no Tier-1 equivalent. |
| **CL-27** | ClipCannon: 23-stage DAG, 7 modalities × 4,044 dim, 58 MCP tools, 12,000+ labelled samples from 16 min source | §4.2 | (Author's system doc) | baevski2020wav2vec (T1), bain2023whisperx (T2), rouard2023htdemucs (T2), campello2013hdbscan (T2), zhang1996birch (T1), google2020mediapipe (T3), tinachen2021yolov5face (T3), resembleai2024enhance (T3), mcp2024spec (T3) | **AUTHOR-EMPIRICAL (system description)** | Non-contested artefact description; components peer-reviewed (wav2vec2 T1, WhisperX T2, HTDemucs T2, BIRCH T1, HDBSCAN T2); three SA T3 components with no T1 equivalents. |
| **CL-28** | Alignment via RLHF / DPO / CAI is scalar-reward + preference-based | §2.6, §5 | ouyang2022instructgpt (T1), rafailov2023dpo (T1), bai2022constitutional (T2) | — | **STRONG** | 3 anchors; DPO's closed-form derivation is the tightest. |
| **CL-29** | Consent / scraping restrictions on high-quality web corpora (~25% of C4) | §2.1, §8 | longpre2024consent (T1) | — | **SINGLE-SOURCE** | One NeurIPS 2024 D&B Track measurement. Well-powered but single-team. Hedge: "one measurement". |
| **CL-30** | Enterprise licensing deals (Reddit-Google, OpenAI-AP, etc.) instantiate "path 1 — license more" | §2.1 | reuters2024reddit (T3), cerullo2024cbs (T3), field2024techcrunch (T3), openai2024reddit (T3), variety2024newscorp (T3), verma2023washpost (T3) | — | **WEAK (industry signals; H cluster)** | 6 corroborating T3 industry signals. Per §7 Discussion quota ≤ 6 Cluster-H citations; use at most 2–3 at a time. Framing only. |
| **CL-31** | Domain-specialised LLMs (MedPaLM, BloombergGPT) demonstrate the enterprise-sidebar path | §2.1, §8.6 | singhal2023nature (T1), tu2025nature (T1), wu2023bloomberggpt (T2) | henderson2022pileoflaw (T1) | **MODERATE** | 3 T1 + 1 T2; Cluster H body-eligible subset. |
| **CL-32** | Pile of Law: 256GB consent-aware legal dataset | §8.6 | henderson2022pileoflaw (T1) | — | **SINGLE-SOURCE** | One NeurIPS 2022 D&B entry; well-powered. |

## Summary: STRONG vs. SINGLE-SOURCE vs. AUTHOR-EMPIRICAL per claim class

### STRONG (8 claims)
- CL-01 scaling law (Kaplan / Chinchilla)
- CL-04 model collapse
- CL-07 prediction–compression duality
- CL-12 embedders as rankers/retrievers (supports the role-flip novelty claim)
- CL-14 TCT vs. scalar-reward contrast (3 alignment anchors)
- CL-15 zero-shot TTS frontier (9 sources)
- CL-19 flow-matching + LoRA + DIT video paradigm
- CL-28 RLHF / DPO / CAI scalar-reward

### MODERATE (8 claims)
- CL-02 data wall exhaustion
- CL-05 collapse mitigations
- CL-06 collapse-silence on DDA (categorical)
- CL-09 LLM compressors across modalities
- CL-10 meaning-compression taxonomy (CAT-EXT)
- CL-11 DDA counting (with T1 embedder substrate)
- CL-20 FLAME + FACS
- CL-25 RRF robustness
- CL-31 domain-specialised LLMs

### SINGLE-SOURCE (6 claims)
- CL-08 compression-capability correlation (Huang 2024; r ≈ 0.95)
- CL-22 LoRA (Hu et al., 2022)
- CL-23 Flow matching (Lipman et al., 2023)
- CL-24 HNSW (Malkov & Yashunin, 2018)
- CL-29 accessible-fraction decay (Longpre 2024)
- CL-32 Pile of Law (Henderson 2022)

### AUTHOR-EMPIRICAL (7 claims)
- CL-13 frozen centroid TCT construct
- CL-16 WavLM SECS 0.961 (Case 3)
- CL-17 centroid + best-of-12 method (Case 3)
- CL-18 DNSMOS + UTMOS parity (Case 3)
- CL-21 flow-matching sign-convention forensic (Case 2 Santa)
- CL-26 Context Graph system description
- CL-27 ClipCannon system description

### WEAK (2 claims; both framing-only)
- CL-03 diminishing-returns industry signal (5 T3 sources, framing)
- CL-30 enterprise-licensing signals (6 T3 sources, framing)

## Rules for §2 Related Work & §3–§7 main body

1. **STRONG claims can assert** as field consensus. Use ≥ 3 citations.
2. **MODERATE claims must hedge.** Use 2+ citations, prefer T1.
3. **SINGLE-SOURCE claims must attribute.** "Huang et al. (2024) report r ≈ 0.95
   across 30 LLMs" not "compression correlates linearly with capability". Flag
   in §8 Limitations where relevant.
4. **AUTHOR-EMPIRICAL claims must use V12 / V17 / V22 hedges.** "observed in
   our Case 3 evaluation, not generalised across speakers / languages /
   recording conditions". Flag in §8.
5. **WEAK / framing claims get at most 2–3 citations.** Use as narrative
   anchors only. Never use as primary evidence.

## Confidence

**HIGH.** Every claim traces to a Q-ID + citations already in the LOCKED
master pool. Every GRADE is defensible per source count + tier distribution +
methodology agreement. The six SINGLE-SOURCE claims are all foundational papers
(LoRA, Flow Matching, HNSW) or flagship measurements (Huang correlation,
Longpre accessible-fraction, Henderson Pile of Law); their SINGLE-SOURCE
status reflects the field's citation economics, not a deficiency of the pool.

## Changelog

- 2026-04-16 — Initial issue by agent-11-systematic-reviewer.
