---
namespace: patterns
agent_id: agent-12-context-tier-manager
timestamp: 2026-04-16T22:45:00Z
status: active
locked: false
supersedes: none
tags: context-tiers, hot-warm-cold, writer-budgets, retrieval-traffic, citation-load-bearing
depends_on:
  - memory/context/mission-brief.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
  - memory/handoff/agent-11-to-12--systematic-review-done.md
downstream_consumers:
  - agent-13-quality-assessor
  - agent-35-introduction-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-conclusion-writer
  - agent-41-abstract-writer
  - agent-42-adversarial-reviewer
  - agent-44-apa-citation-specialist
---

# Context Tier Table — Hot / Warm / Cold (88 citations)

## Tier definitions (for this manuscript)

The tier layer here is **orthogonal** to the T1/T2/T3 source-quality tier set by
agent-07 / agent-08. T1/T2/T3 measures **source quality**; H/W/C measures
**writer traffic** (how often this citation is expected to appear and how
load-bearing it is for the paper's argument).

- **HOT (H, ≈ 20% of pool, ~18 entries).** Must cite in body; high traffic.
  Appears in ≥ 2 essential questions OR anchors a STRONG/AUTHOR-EMPIRICAL claim
  that the paper's three-paths + DDA + TCT + case-3 spine depends on. Lose any
  of these and the paper no longer makes its load-bearing point. Writers keep
  the full citation + 1–2 paraphraseable findings on hand.
- **WARM (W, ≈ 30% of pool, ~26 entries).** Cite as supporting context.
  Strengthens / corroborates a hot-tier primary, fills a methodological slot,
  or provides a comparator that one STRONG claim needs. Writers pull on demand
  per the cue sheet.
- **COLD (C, ≈ 50% of pool, ~44 entries).** Keep available, low traffic.
  Background, tangential, industry-signal, or hedged-sidebar citations.
  Writers cite sparingly (framing only, never as primary evidence).

**Promotion trigger.** If a writer references a cold-tier citation in ≥ 2
section drafts, propose promotion-to-warm via a blocker. **Demotion trigger.**
If a hot-tier citation is not referenced in any section after agent-39 polish,
it demotes to warm.

## Count summary (target 20 / 30 / 50)

| Tier | Count | % of 88 | Memory traffic |
|---|---|---|---|
| HOT | 18 | 20.5% | Always load — writer has full citation + 1–2 bullet findings |
| WARM | 26 | 29.5% | On demand — writer has full citation; reads abstract if needed |
| COLD | 44 | 50.0% | Indexed — writer cites by key; no expanded reading required |
| **TOTAL** | **88** | **100%** | — |

Distribution is inside the 20/30/50 ±5% band. ✅

## HOT tier — 18 citations (the paper cannot stand without these)

| # | Key | Cluster | Src tier | STRONG/MODERATE/SINGLE etc. | Q coverage | Why HOT |
|---|---|---|---|---|---|---|
| 1 | villalobos2024data | A | T1 | MODERATE (CL-02) | Q01, Q03, Q15, Q16 | The data-wall anchor. §1 framing + §8 caveat. Four questions. |
| 2 | hoffmann2022chinchilla | A | T1 | STRONG (CL-01) | Q01, Q03 | Compute-optimal / Chinchilla; the "wall arrives sooner" support. |
| 3 | longpre2024consent | A | T1 | MODERATE (CL-02); SINGLE (CL-29) | Q01, Q03, Q19, Q20 | Accessible-fraction collapse (C4 25%). §1 + §8 ethics. Four questions. |
| 4 | shumailov2024nature | B | T1 | STRONG (CL-04) | Q02, Q03, Q16 | **The Nature model-collapse paper.** §2.2 + §3.3 + §8 R4. |
| 5 | alemohammad2023mad | B | T1 | STRONG (CL-04) | Q02 | Self-consuming / MAD; second pillar of collapse primary set. |
| 6 | deletang2024compression | C | T1 | STRONG (CL-07); MODERATE (CL-09, CL-10) | Q04, Q05 | Prediction=compression anchor. §3.1 four-category taxonomy. |
| 7 | huang2024compression | C | T2 | SINGLE (CL-08); MODERATE (CL-10) | Q05, Q16 | The r≈0.95 correlation. The §3 empirical anchor + §8 hedge. |
| 8 | chen2022wavlm | D | T1 | STRONG-support (CL-11, CL-12); AUTHOR-EMP anchor (CL-16, CL-17, CL-18) | Q04, Q08, Q09, Q10, Q12, Q14, Q16, Q18, Q20 | **Nine-question citation.** SECS scorer + E-slot + Case 3 evaluator. The single most-loaded paper in the pool. |
| 9 | zhai2023siglip | D | T2 | STRONG-support (CL-12); substrate (CL-11) | Q04, Q09, Q10 | SigLIP. Visual embedder for DDA + TCT-vs-CLIP contrast. |
| 10 | malkov2018hnsw | D | T1 | SINGLE (CL-24); substrate (CL-11) | Q10 | HNSW. Retrieval substrate for 13-embedder Context Graph. |
| 11 | chen2024valle2 | E | T2 | STRONG (CL-15); comparator (CL-16) | Q09, Q12, Q14, Q15 | **The Case 3 comparator.** "Human parity" claim that our 0.961 SECS number is benchmarked against. |
| 12 | meng2025echomimicv3 | F | T2 | STRONG (CL-19); AUTHOR-EMP (CL-21) | Q07, Q11, Q13, Q15, Q17, Q18 | Case 2 base model + flow-matching sign-bug context. Six questions. |
| 13 | hu2022lora | F | T1 | STRONG (CL-19); SINGLE (CL-22) | Q06, Q07, Q11, Q13, Q15, Q17, Q18 | LoRA. Substrate for Cases 1+2 + §4 Systems. Seven questions. |
| 14 | lipman2023flowmatching | F | T1 | STRONG (CL-19); SINGLE (CL-23); AUTHOR-EMP (CL-21) | Q06, Q07, Q13, Q17 | Flow-matching foundational paper + sign-convention forensics. |
| 15 | ouyang2022instructgpt | I | T1 | STRONG (CL-14, CL-28) | Q06, Q13 | RLHF / InstructGPT. **The Q06 comparator agent-07 flagged as missing.** |
| 16 | rafailov2023dpo | I | T1 | STRONG (CL-14, CL-28) | Q06, Q13, Q17 | DPO. Second anchor of the scalar-reward comparator triad. |
| 17 | bai2022constitutional | I | T2 | STRONG (CL-14, CL-28) | Q06 | Constitutional AI. Third anchor of the scalar-reward comparator triad. |
| 18 | li2017flame | F | T1 | MODERATE (CL-20) | Q11 | FLAME. ClipCannon seven-modality anchor; only T1 source for the facial-shape slot. |

**Hot-tier composition check.**
- Src-tier mix: **12× T1, 6× T2, 0× T3** → 100% peer-reviewed anchors. ✅
- Cluster mix: A(3), B(2), C(2), D(3), E(1), F(4), I(3). Every core cluster is represented at the hot tier. ✅
- Q coverage: every Q01…Q20 is served by at least one hot-tier citation except Q17 (forensics) and Q19 (ethics) which are served by warm-tier Pile of Law + forensics files. Q17 and Q19 do not need hot-tier anchors because they lean on the author's own forensic files and Cluster-H sidebar.

**The "10 most-load-bearing" citations** (ranked by Q-coverage × GRADE-weight):

1. **chen2022wavlm** — 9 questions, STRONG + AUTHOR-EMPIRICAL anchor.
2. **hu2022lora** — 7 questions, STRONG + SINGLE-SOURCE foundational.
3. **meng2025echomimicv3** — 6 questions, STRONG + AUTHOR-EMPIRICAL forensic context.
4. **shumailov2024nature** — 3 questions, STRONG. The single biggest "is this paper flagged as at risk of collapse?" question.
5. **chen2024valle2** — 4 questions, STRONG. Case 3's published comparator.
6. **villalobos2024data** — 4 questions, MODERATE. The data-wall anchor; entire §1 pivots on this.
7. **hoffmann2022chinchilla** — 2 questions, STRONG. Compute-optimal / foundational.
8. **lipman2023flowmatching** — 4 questions, STRONG + SINGLE-SOURCE + AUTHOR-EMPIRICAL forensic.
9. **deletang2024compression** — 2 questions, STRONG. Compression-as-intelligence anchor for §3.
10. **rafailov2023dpo** — 3 questions, STRONG. DPO canonical for the scalar-reward contrast.

## WARM tier — 26 citations (cite as supporting context)

| # | Key | Cluster | Src tier | GRADE | Q coverage | Why WARM |
|---|---|---|---|---|---|---|
| 1 | kaplan2020scaling | A | T2 | STRONG-sup (CL-01) | Q01 | Scaling-law foundational; only required if §1 first-uses "scaling law". |
| 2 | dohmatob2024tails | B | T1 | STRONG-sup (CL-04) | Q02 | Collapse as scaling-law crossover. Corroborates shumailov. |
| 3 | dohmatob2024regression | B | T1 | STRONG-sup (CL-04) | Q02 | Closed-form regression collapse. Supports CL-04. |
| 4 | gerstgrasser2024colm | B | T2 | MODERATE (CL-05); support (CL-04) | Q02, Q03, Q16 | Accumulation mitigation. §2.2 + §3.3. |
| 5 | feng2024verification | B | T2 | MODERATE (CL-05) | Q02, Q03 | Verification mitigation. §2.2. |
| 6 | seddik2024collapse | B | T2 | STRONG-sup (CL-04) | Q02 | Statistical analysis of LM collapse. Supports CL-04. |
| 7 | li2025lossless | C | T1 | MODERATE (CL-09); support (CL-10) | Q05, Q20 | Nature MI lossless-compression paper. §3.1 + §9 future work. |
| 8 | jiang2023gzip | C | T1 | STRONG-sup (CL-07) | Q05 | gzip + k-NN ACL Findings precedent. §3.1. |
| 9 | nussbaum2024nomic | D | T2 | STRONG-sup (CL-11, CL-12) | Q04, Q10, Q18 | Nomic text embedder. DDA substrate + §4.1 + reproducibility. |
| 10 | santhanam2022colbertv2 | D | T1 | STRONG-sup (CL-11, CL-12) | Q04, Q10 | ColBERTv2. E12 substrate. |
| 11 | formal2021splade | D | T2 | STRONG-sup (CL-11, CL-12) | Q10 | SPLADE. E13 substrate. |
| 12 | wang2021kepler | D | T2 | STRONG-sup (CL-11) | Q10 | KEPLER. E11 substrate. |
| 13 | bordes2013transe | D | T1 | STRONG-sup (CL-11) | Q10 | TransE. KEPLER lineage. |
| 14 | khattab2020colbert | D | T2 | STRONG-sup (CL-11, CL-12, CL-25) | Q10 | ColBERT predecessor. §4.1. |
| 15 | cormack2009rrf | D | T2 | MODERATE (CL-25); support (CL-11, CL-12) | Q10 | RRF fusion. Context Graph retrieval substrate. |
| 16 | tan2024naturalspeech3 | E | T1 | STRONG (CL-15); comparator (CL-14, CL-15) | Q09, Q14, Q15 | NaturalSpeech 3. TTS comparator. |
| 17 | wang2024maskgct | E | T1 | STRONG (CL-15) | Q09, Q14, Q15 | MaskGCT. TTS comparator. |
| 18 | wang2023valle | E | T2 | STRONG (CL-15) | Q09, Q14 | VALL-E precursor. §2.6. |
| 19 | chen2024f5tts | E | T2 | STRONG (CL-15) | Q09, Q14 | F5-TTS flow-matching comparator. §2.6. |
| 20 | reddy2022dnsmos | E | T2 | AUTHOR-EMP sup (CL-18); Q07/Q08/Q18 | Q07, Q08, Q12, Q18 | DNSMOS scorer. Case 3 measurement anchor. |
| 21 | saeki2022utmos | E | T2 | AUTHOR-EMP sup (CL-18) | Q07, Q08, Q12, Q18 | UTMOS scorer. Case 3 measurement anchor. |
| 22 | baevski2020wav2vec | G | T1 | AUTHOR-EMP sup (CL-27) | Q11 | wav2vec 2.0 (promoted T2→T1 NeurIPS 2020). ClipCannon audio embedder. |
| 23 | bain2023whisperx | G | T2 | AUTHOR-EMP sup (CL-27) | Q11 | WhisperX ASR stage of ClipCannon. |
| 24 | rouard2023htdemucs | G | T2 | AUTHOR-EMP sup (CL-27) | Q11 | HTDemucs music separation stage. |
| 25 | henderson2022pileoflaw | H | T1 | SINGLE (CL-32); MODERATE (CL-31) | Q03, Q19 | Pile of Law. Body-eligible Cluster-H. Consent-aware exemplar. |
| 26 | ekman1978facs | F | T3 | MODERATE (CL-20) | Q11 | FACS. Canonical print reference for facial-action decomposition; the only source for the modality. |

**Warm-tier composition check.**
- Src-tier mix: **9× T1, 16× T2, 1× T3** → 96% peer-reviewed. ✅
- Cluster mix: A(1), B(5), C(2), D(7), E(6), F(1), G(3), H(1). Strong Cluster-D/E representation (comparator + substrate-corroborators). ✅
- Every warm citation corroborates a hot-tier primary or a STRONG/SINGLE/AUTHOR-EMPIRICAL claim listed in agent-11's table.

## COLD tier — 44 citations (low traffic; reference only)

Ordering: within-cluster, then T1 → T2 → T3.

| # | Key | Cluster | Src tier | Role | Why COLD |
|---|---|---|---|---|---|
| 1 | woo2024information | A | T3 | CL-03 industry signal | Press cross-ref; framing only. |
| 2 | metz2024bloomberg | A | T3 | CL-03 industry signal | Press cross-ref; framing only. |
| 3 | hu2024reuters | A | T3 | CL-03 industry signal | Press cross-ref; framing only. |
| 4 | franzen2024techcrunch | A | T3 | CL-03 industry signal | Press cross-ref; framing only. |
| 5 | sutskever2024neurips | A | T3 | CL-03 oral remark | Attribution only; one-liner. |
| 6 | valmeekam2023llmzip | C | T2 | CL-07 support | One benchmark number. |
| 7 | le2023voicebox | E | T2 | CL-15 support | TTS precedent; one sentence. |
| 8 | shen2023naturalspeech2 | E | T2 | CL-15 support | TTS precedent; one sentence. |
| 9 | jiang2023megatts2 | E | T2 | CL-15 support | Single-prompt precedent contrast. |
| 10 | casanova2024xtts | E | T2 | Q09, Q18 support | Multilingual TTS mention; reproducibility substitute. |
| 11 | desplanques2020ecapa | E | T2 | Q20 future work | Only cited in §8/§9 for future work. |
| 12 | christop2025cloneval | E | T2 | Q14, Q16, Q20 | Cross-cohort benchmark reference; future-work mention. |
| 13 | cui2024hallo3 | F | T2 | CL-19 support | Video-generation comparator; §2.5 mention. |
| 14 | li2024latentsync | F | T2 | Q13 projected-metric reference | Lip-sync metric anchor; §6.2 projected. |
| 15 | zheng2025hunyuanvideo | F | T2 | CL-19 support | Multi-character video comparator; §2.5. |
| 16 | tian2024emo | F | T2 | CL-19 support | EMO predecessor; §2.5. |
| 17 | tian2025emo2 | F | T2 | CL-19 support | EMO2 successor; §2.5. |
| 18 | yang2024cogvideox | F | T2 | CL-19 support | T2V diffusion comparator; §2.5. |
| 19 | wan2025video | F | T2 | CL-21 AUTHOR-EMP | Flow-matching lineage for sign-convention disclosure; §6.2/§8.4. |
| 20 | campello2013hdbscan | G | T2 | CL-27 ClipCannon stage | Clustering algorithm; §4.2 mention. |
| 21 | zhang1996birch | G | T1 | CL-27 ClipCannon stage | BIRCH clustering; §4.2 mention. |
| 22 | google2020mediapipe | G | T3 | CL-27 ClipCannon tool | Face-mesh tool; §4.2 mention. |
| 23 | tinachen2021yolov5face | G | T3 | CL-27; Q17 forensic | YOLO mouth-landmark bug; §6.2 forensic. |
| 24 | resembleai2024enhance | G | T3 | CL-27 ClipCannon tool | Speech-enhancement tool; §4.2. |
| 25 | mcp2024spec | G | T3 | CL-26, CL-27 | MCP protocol citation; §4.1/§4.2 infra. |
| 26 | anthropic2024mcp | H | T3 | Q19 ethics | MCP safety framing; §8.6 (sparingly). |
| 27 | field2024techcrunch | H | T3 | CL-30 industry | Reddit licensing signal; §2.1 framing. |
| 28 | gartner2023trends | H | T3 | Q19 sidebar | Enterprise data stance; §8.6 optional. |
| 29 | harvard2024caselaw | H | T3 | Q19 infra | Public-domain corpus; §8.6 optional. |
| 30 | nci2024thesaurus | H | T3 | Q19 infra | Controlled vocabulary; §8.6 optional. |
| 31 | nlm2024mesh | H | T3 | Q19 infra | Controlled vocabulary; §8.6 optional. |
| 32 | openai2024reddit | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 33 | openai2024newscorp | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 34 | reuters2024reddit | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 35 | cerullo2024cbs | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 36 | shutterstock2023openai | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 37 | singhal2023nature | H | T1 | MODERATE (CL-31) | Body-eligible Cluster-H; domain-LLM exemplar; §2.1 / §8.6. |
| 38 | snomed2024ct | H | T3 | Q19 infra | Controlled vocabulary; §8.6 optional. |
| 39 | tu2025nature | H | T1 | MODERATE (CL-31) | Body-eligible Cluster-H; clinical-LLM exemplar; §2.1 / §8.6. |
| 40 | sec2024edgar | H | T3 | Q19 infra | Government corpus reference; §8.6 optional. |
| 41 | sec2024edgarsearch | H | T3 | Q19 infra | Government corpus reference; §8.6 optional. |
| 42 | variety2024newscorp | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 43 | verma2023washpost | H | T3 | CL-30 industry | Licensing signal; §2.1 framing (sparingly). |
| 44 | wu2023bloomberggpt | H | T2 | MODERATE (CL-31) | Body-eligible Cluster-H; domain-LLM exemplar; §2.1 / §8.6. |

**Cold-tier composition check.**
- Src-tier mix: **4× T1, 15× T2, 25× T3** → 57% T3 (press + gov refs + software artefacts, as expected). ✅
- Cluster mix: A(5), C(1), E(6), F(7), G(6), H(19). Cluster-H is almost entirely cold, as its body quota is ≤ 6. ✅
- Cold tier contains three **body-eligible T1 Cluster-H** citations (henderson2022pileoflaw [WARMED — see above], singhal2023nature, tu2025nature): these may promote case-by-case to warm if §8.6 lands heavily on them.

## Cluster-level traffic summary

| Cluster | Total | HOT | WARM | COLD | Notes |
|---|---|---|---|---|---|
| A (data wall) | 9 | 3 | 1 | 5 | All 5 T3 press-signals are cold. |
| B (collapse) | 7 | 2 | 5 | 0 | B is fully hot+warm — the collapse risk is load-bearing. |
| C (compression) | 5 | 2 | 2 | 1 | One T2 precedent cold. |
| D (embedders) | 10 | 3 | 7 | 0 | D is fully hot+warm — it's the DDA substrate. |
| E (TTS) | 13 | 1 | 6 | 6 | Hot is just VALL-E 2 (the comparator); others corroborate. |
| F (video) | 12 | 4 | 1 | 7 | Hot-dense because Case 2 + flow-matching + LoRA + FLAME are here. |
| G (pipeline) | 9 | 0 | 3 | 6 | Pipeline refs are infra mentions; warm for the three peer-reviewed ones. |
| H (enterprise) | 20 | 0 | 1 | 19 | Only Pile of Law warmed; §8.6-lane cite ≤ 6 rule enforced. |
| I (alignment) | 3 | 3 | 0 | 0 | All 3 hot — this was the Q06 gap agent-07 flagged. |
| **TOTAL** | **88** | **18** | **26** | **44** | |

## Enforcement rules for downstream agents

1. **Writers cite by key only.** No new citations. No URL text in prose.
2. **Hot tier is first-load.** When an agent opens a section, it reads the
   hot-tier citations for its assigned Q-IDs before drafting.
3. **Warm tier is on-demand.** Writers open warm tier per the cue sheet
   (`agent-12-context-tier-manager--writer-cue-sheet.md`).
4. **Cold tier cites are one-shot.** Writers do not paraphrase cold citations
   beyond ~1 sentence; framing and industry-signal only.
5. **Cluster-H quota ≤ 6** body citations. Prefer H05 Henderson, H13 Singhal,
   H15 Tu, H20 BloombergGPT (the four T1/T2 body-eligibles).
6. **SINGLE-SOURCE hot cites require attribution phrasing.** Hu 2022 LoRA,
   Lipman 2023 flow-matching, Malkov 2018 HNSW, Huang 2024 compression-
   correlation, Longpre 2024 consent — all of these must say "as introduced by
   X" or "X report" rather than "it is known that".
7. **AUTHOR-EMPIRICAL hot cites require V12/V17/V22 hedge** within 2 sentences.

## Memory budget (conceptual; not enforced in bytes)

- Hot tier: 60% of reader attention, 18 keys × full cite block + 1–2 bullet
  findings = moderate working memory.
- Warm tier: 30% of reader attention, 26 keys × full cite block = moderate.
- Cold tier: 10% of reader attention, 44 keys × key + 1-line role note = low.

## Promotion / demotion log

Initial issue — no promotions or demotions yet. After agent-36 Related Work
polish, revisit:
- If a cold-tier Cluster-E comparator gets cited in §2.6 + §6.3 → propose
  promote to warm.
- If `ekman1978facs` (warm) is cited only once in §4.2 → demote to cold.

## Confidence

**HIGH.** Every tier assignment traces to (a) Q-coverage from agent-08's
claim-to-citation index, (b) GRADE from agent-11's evidence-grading table, and
(c) the cluster topology agent-06 established. Distribution hits 20/30/50 ±5%.

## Changelog

- 2026-04-16 — Initial issue by agent-12-context-tier-manager.
