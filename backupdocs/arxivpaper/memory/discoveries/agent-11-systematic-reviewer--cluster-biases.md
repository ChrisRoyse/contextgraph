---
namespace: discoveries
agent_id: agent-11-systematic-reviewer
timestamp: 2026-04-16T21:45:00Z
status: active
supersedes: none
tags: bias, per-cluster, publication-bias, geographic-bias, industry-bias, lab-concentration
depends_on:
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/discoveries/agent-09-methodology-scanner--methodology-scan.md
downstream_consumers:
  - agent-12-context-tier-manager
  - agent-36-related-work-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
---

# Cluster-Level Bias Assessment

## Method

For each cluster, we audit along five bias axes:

- **L-Bias-1 Language bias** — English-only pool (shared across all clusters).
- **L-Bias-2 Lab / affiliation concentration** — Does a single lab or small
  consortium dominate the cluster?
- **L-Bias-3 Geographic concentration** — Is the cluster US / China heavy with
  limited geographic spread?
- **L-Bias-4 Venue concentration** — Do 1–2 venues dominate the cluster?
- **L-Bias-5 Temporal concentration** — Are all refs clustered in a narrow time
  window (could signal recency bias or missing foundational work)?
- **L-Bias-6 Publication bias** — Positive-result-only? Are null / replication
  studies present?
- **L-Bias-7 Methodology bias** — Does the cluster's methodological monoculture
  (e.g., all EB + AB) hide alternative framings?

## Cross-cluster: Language bias

**L-Bias-1 GLOBAL.** All 88 citations are English-language sources. Non-English
sources (e.g., Chinese-language or Japanese-language TTS papers) are excluded by
the I4 inclusion criterion. This is a documented limitation of PRISMA-lite scope;
agent-40 limitations-writer must note this.

---

## Cluster A — Scaling laws + data wall

**Bias profile:**

- L-Bias-2 Lab concentration: **MODERATE**. Kaplan (OpenAI), Hoffmann
  (DeepMind), Villalobos (Epoch AI), Longpre (MIT Media Lab). Four distinct
  groups; good spread.
- L-Bias-3 Geographic: **US / UK / continental-EU mix**; no Asian lab
  representation.
- L-Bias-4 Venue concentration: **LOW**. NeurIPS / ICML / arXiv spread.
- L-Bias-5 Temporal: **MODERATE**. 2020 (Kaplan) to 2024 (Villalobos, Longpre);
  tight 4-year window for a fast-moving topic but not narrow.
- L-Bias-6 Publication bias: **LOW for T1; HIGH for T3**. Industry signals
  (woo, metz, hu, franzen, sutskever) all report the same "diminishing returns"
  narrative; publication bias in dated industry reporting is inherent.
  Corroboration does not mean consensus has been independently replicated — the
  journalists cross-reference each other.
- L-Bias-7 Methodology: **MODERATE**. All T1/T2 refs use TD (scaling-law
  extrapolation); no refs use control-experiment AB on data-preparation
  pipelines. This is A-METHOD-GAP-2 per agent-09.

**FLAG:** The industry-signal corroboration loop (5 T3 sources with inter-source
citations) inflates apparent agreement. Treat as *one* narrative signal dated
November 2024, not as five independent confirmations.

---

## Cluster B — Model collapse + synthetic data

**Bias profile:**

- L-Bias-2 Lab concentration: **HIGH**. Dohmatob/Feng/Yang/Charton/Kempe
  (NYU-FAIR) appears twice (dohmatob2024tails, dohmatob2024regression), plus a
  feng2024verification extension. Shumailov lineage (Oxford + Cambridge + Google)
  anchors the *Nature* paper. Two tight author-group clusters dominate.
- L-Bias-3 Geographic: **US / UK / FR**; more geographic spread than A.
- L-Bias-4 Venue concentration: **LOW**. *Nature* + NeurIPS + ICML + ICLR +
  COLM mix.
- L-Bias-5 Temporal: **VERY HIGH**. All 7 papers are 2023–2024. The topic is
  ≤ 2 years old in published form; foundational pre-2023 work (e.g., Caballero
  et al. 2023 "Broken Neural Scaling Laws" adjacent) is not cited. This reflects
  the field's recency rather than agent selection bias.
- L-Bias-6 Publication bias: **LOW within the cluster**. Mitigation papers
  (gerstgrasser, feng) are published alongside collapse papers (shumailov,
  dohmatob×2), giving balance.
- L-Bias-7 Methodology: **LOW**. TD + EB across generator-in-loop; simulation
  + closed-form both present.

**FLAG:** Author-group concentration (NYU-FAIR × 3 + Oxford/Cambridge × 1)
means the cluster's core theoretical lineage is held by ~2 groups. This is a
lab-concentration bias that limits independent replication; flag in §8.

---

## Cluster C — Compression-as-intelligence

**Bias profile:**

- L-Bias-2 Lab concentration: **MODERATE**. Google DeepMind (Delétang),
  NJUST-affiliates (Huang), Nankai (Li 2025), Texas A&M (Valmeekam),
  Hunan/Yunnan-affiliates (Jiang). Five groups; good spread.
- L-Bias-3 Geographic: **US / China mix**; most of the compression-intelligence
  work after Delétang comes from Chinese institutions (huang2024compression,
  li2025lossless, jiang2023gzip). This is the one cluster in the pool with clear
  Asian lab representation.
- L-Bias-4 Venue concentration: **MODERATE**. ICLR / COLM / ACL Findings /
  Nature MI + arXiv. Spread across top-tier venues.
- L-Bias-5 Temporal: **MODERATE**. 2023–2025 concentration; foundational
  compression theory (Shannon, Rissanen) is not separately cited (treated as
  textbook).
- L-Bias-6 Publication bias: **LOW**. Jiang 2023's gzip-k-NN result is a
  *counter-intuitive* finding that got through peer review; the cluster's
  publication ecosystem accepts null / negative results reasonably.
- L-Bias-7 Methodology: **MODERATE**. All EB + MA; no TD work in the cluster
  beyond the Shannon framing. The cross-compressor interaction gap
  (C-FG-1 per agent-10) is a *methodology-bias* artefact — the cluster's
  questions do not naturally produce the construct we need.

**FLAG:** Methodology monoculture (EB on held-out compression ratio) makes the
cluster's framing of the problem *within-model*; our meaning-compression CAT-EXT
move requires that framing to be extended, not inherited from this cluster's
methods.

---

## Cluster D — Multi-modal embedders + retrieval

**Bias profile:**

- L-Bias-2 Lab concentration: **LOW**. Diverse: Google (Zhai), Nomic (Nussbaum),
  Microsoft (Chen WavLM), Stanford-ish (Khattab / Santhanam / Zaharia),
  Naver (Formal), Tsinghua / MIT (Wang KEPLER), FAIR (Bordes), Yandex / HSE
  (Malkov), Microsoft / Waterloo (Cormack).
- L-Bias-3 Geographic: **US / CA / EU / CN spread**. Most diverse cluster in
  the pool.
- L-Bias-4 Venue concentration: **LOW**. SIGIR / NAACL / ICLR / IEEE JSTSP /
  IEEE TPAMI / TACL / arXiv.
- L-Bias-5 Temporal: **LOW**. 2009 (Cormack RRF) to 2024 (Nomic). 15-year
  window, properly historical.
- L-Bias-6 Publication bias: **LOW**. Mix of positive / null / algorithm papers.
- L-Bias-7 Methodology: **LOW**. TD (HNSW) + EB + AB + SA mix.

**FLAG:** None significant. This is the cleanest cluster in the pool.

---

## Cluster E — Zero-shot TTS + voice cloning

**Bias profile:**

- L-Bias-2 Lab concentration: **HIGH**. **Microsoft Research dominates**:
  wang2023valle, chen2024valle2, tan2024naturalspeech3, shen2023naturalspeech2
  are all Microsoft. Meta FAIR (le2023voicebox); Alibaba (mentioned in
  cluster F); ByteDance/Zhejiang (jiang2023megatts2); SeedTTS / Shanghai Jiao
  Tong (chen2024f5tts); Coqui (casanova2024xtts). **Microsoft or Alibaba account
  for ~50% of cluster E papers**.
- L-Bias-3 Geographic: **US / China heavy**; Korea / Japan represented via
  scorers (saeki2022utmos UTokyo-SaruLab); no African / LATAM representation.
- L-Bias-4 Venue concentration: **MODERATE**. ICML / ICLR / ICASSP / Interspeech
  + arXiv; 4 out of 13 are arXiv-only (industry labs publishing preprints is
  the norm in TTS). Noted explicitly in tier classifications.
- L-Bias-5 Temporal: **VERY HIGH**. 9 of 13 refs are 2023–2025; the field is
  moving fast. This is not selection bias per se but does bias the
  "frontier" framing toward the most recent architectures.
- L-Bias-6 Publication bias: **HIGH**. TTS papers almost universally report
  positive results on comparator benchmarks; failure analyses are rare in
  published venues. VALL-E 2's "human parity" claim is a strong positive
  framing that has not been independently replicated at the method level outside
  the original lab.
- L-Bias-7 Methodology: **HIGH**. Near-uniform EB + AB methodology with SIM-o
  headline + WER + DNSMOS + UTMOS; the cluster's single-prompt paradigm is a
  methodological monoculture. Our Case 3's centroid + runtime gate +
  cross-encoder SECS deviations from this monoculture are partly *why* they
  appear novel.

**FLAG: This cluster is Microsoft / Alibaba dominated.** If we cite
wang2023valle + chen2024valle2 + tan2024naturalspeech3 + shen2023naturalspeech2
as "the TTS frontier", we are citing four Microsoft papers.
chen2024f5tts and casanova2024xtts offset this somewhat.
**Recommendation for §2.5 and §6.3:** use the four Microsoft citations together
with ≥ 2 non-Microsoft citations (wang2024maskgct, chen2024f5tts,
casanova2024xtts, le2023voicebox) to balance the cluster; explicitly note the
lab-concentration in §8 Limitations.

---

## Cluster F — Identity-locked video generation

**Bias profile:**

- L-Bias-2 Lab concentration: **VERY HIGH**. **Alibaba (Ant Group / Tongyi Lab /
  DAMO) dominates**: meng2025echomimicv3, tian2024emo, tian2025emo2,
  wan2025video are all Alibaba. Fudan-Baidu (cui2024hallo3); Tencent Hunyuan
  (zheng2025hunyuanvideo); ByteDance (li2024latentsync); Zhipu-Tsinghua
  (yang2024cogvideox). **Chinese labs account for ~85% of cluster F papers.**
  Western anchors are the foundational theory (hu2022lora Microsoft,
  lipman2023flowmatching Meta, li2017flame MPI) and the print reference
  (ekman1978facs US).
- L-Bias-3 Geographic: **CN-heavy, US/DE foundational**.
- L-Bias-4 Venue concentration: **VERY HIGH**. 8 of 12 are arXiv-only (no
  peer-reviewed venue at the time of briefing); LoRA + Flow Matching + FLAME
  are ICLR / ACM TOG. The arXiv-only fraction is a feature of the
  fast-moving video-diffusion field but biases the evidence base toward
  non-peer-reviewed reporting.
- L-Bias-5 Temporal: **VERY HIGH**. 10 of 12 refs are 2024–2025; only
  ekman1978facs (1978) and li2017flame (2017) are historical.
- L-Bias-6 Publication bias: **HIGH**. Same as Cluster E — positive results
  dominate; failure / sign-convention bugs (our F-METHOD-GAP-3) are undocumented.
- L-Bias-7 Methodology: **HIGH**. Near-uniform flow-matching / DIT /
  LoRA architectural stack. The identity-locked literature inherits a narrow
  methodological vocabulary.

**FLAG: Cluster F is Alibaba-dominated and arXiv-heavy.** If we cite
meng2025echomimicv3 + tian2024emo + tian2025emo2 + wan2025video together,
all four are Alibaba. Use hu2022lora + lipman2023flowmatching + cui2024hallo3
+ zheng2025hunyuanvideo + yang2024cogvideox for corroboration. **Explicitly
note the CN-lab concentration and arXiv-heavy status in §8 Limitations.**

---

## Cluster G — Auxiliary pipeline components

**Bias profile:**

- L-Bias-2 Lab concentration: **LOW**. Meta FAIR (wav2vec2, HTDemucs), VGG
  Oxford (WhisperX), UAlberta (HDBSCAN), Wisconsin (BIRCH), Google (MediaPipe).
  Good spread.
- L-Bias-3 Geographic: **US / EU spread; no Asian**. Good historical
  representation.
- L-Bias-4 Venue concentration: **LOW**. NeurIPS / Interspeech / ICASSP / SIGMOD
  / PAKDD / GitHub / spec.
- L-Bias-5 Temporal: **LOW**. 1996 (BIRCH) to 2024 (Resemble Enhance). Good
  historical depth.
- L-Bias-6 Publication bias: **LOW**. Mix of positive / algorithm / software
  refs.
- L-Bias-7 Methodology: **LOW**. Heterogeneous by design.

**FLAG:** Software T3 components (MediaPipe, YOLOv5-Face, Resemble Enhance)
have no peer-review equivalent but are standard tools; agent-07 T3-justification
file documents per ref.

---

## Cluster H — Enterprise / market signals

**Bias profile:**

- L-Bias-2 Lab concentration: **HIGH for T3 signals**. OpenAI announcements
  (openai2024reddit, openai2024newscorp) + Anthropic (anthropic2024mcp) dominate.
  T1/T2 subset (Henderson Stanford, Singhal Google, Tu Google, Wu Bloomberg /
  JHU) is more diverse.
- L-Bias-3 Geographic: **US-dominant**.
- L-Bias-4 Venue concentration: **VERY HIGH**. 14 T3 signals are press articles
  / corporate announcements; only 4 are peer-reviewed.
- L-Bias-5 Temporal: **MODERATE**. 2022–2025 concentration.
- L-Bias-6 Publication bias: **VERY HIGH**. Industry deals are announced by
  companies with a strong incentive to frame them favourably; journalists
  cross-reference each other (Reuters → CBS → Variety → Washington Post on
  similar deal). Treat as *one signal per deal*, not N-source consensus.
- L-Bias-7 Methodology: **HIGH**. Non-theoretical (SI); the T1/T2 subset
  (H05/H13/H15/H20) is EB + SA.

**FLAG:** Cluster H exists primarily as narrative framing; body-quota ≤ 6 with
T1/T2 preferred (H05, H13, H15, H20). Multiple T3 corroborations do not
establish independent consensus (L-Bias-6).

---

## Cluster I — Alignment (RLHF / DPO / CAI)

**Bias profile:**

- L-Bias-2 Lab concentration: **VERY HIGH**. OpenAI (Ouyang), Stanford (Rafailov
  et al.), Anthropic (Bai). **Three papers, three labs — but all three are the
  dominant US alignment players**. No representation from EU (DeepMind's work is
  adjacent but not in pool) or Asia.
- L-Bias-3 Geographic: **US-only**.
- L-Bias-4 Venue concentration: **HIGH (intentional)**. NeurIPS 2022 + NeurIPS
  2023 + arXiv. Only 3 refs, so concentration is structural.
- L-Bias-5 Temporal: **MODERATE**. 2022–2023. The field has evolved since (e.g.,
  RLAIF variants, KTO, etc.) but the three foundational refs suffice for our
  Q06 BRK contrast.
- L-Bias-6 Publication bias: **MODERATE**. RLHF / DPO papers report
  positive alignment outcomes; failure modes (reward-hacking, scalar-reward
  insufficiency for identity locking — precisely our TCT motivation) are
  under-reported in the same papers.
- L-Bias-7 Methodology: **HIGH**. All three share the scalar-reward /
  preference-model primitive; there is no methodological pluralism within the
  cluster. This is precisely the monoculture TCT departs from.

**FLAG:** **Cluster I is a small, US-only, methodologically homogeneous
3-paper cluster.** This is adequate for the Q06 BRK contrast (we *want* the
scalar-reward paradigm to be tightly defined so TCT can depart from it) but
should be disclosed in §8 Limitations as "alignment literature scope defined
by three canonical references; we do not claim to survey the broader
alignment landscape (e.g., RLAIF, KTO, IPO, ORPO)".

---

## Cross-cluster bias summary

| Cluster | Lab concentration | Geographic | Venue concentration | Temporal | Publication bias | Top flag |
|---|---|---|---|---|---|---|
| A | Moderate | US/UK/EU | Low | Moderate | High (T3 loop) | Industry-signal circularity |
| B | High (NYU-FAIR) | US/UK/FR | Low | Very high | Low | Author-group concentration |
| C | Moderate | US/CN | Moderate | Moderate | Low | Methodology monoculture (within-model) |
| D | Low | US/CA/EU/CN | Low | Low | Low | None — cleanest cluster |
| E | **Very high (Microsoft / Alibaba)** | US/CN | Moderate | High | High | **Microsoft / Alibaba dominance** |
| F | **Very high (Alibaba / Chinese labs ~85%)** | **CN-heavy** | **Very high (arXiv-only)** | Very high | High | **Alibaba dominance + arXiv-heavy** |
| G | Low | US/EU | Low | Low (historical depth) | Low | Software T3 components (documented) |
| H | High (US companies) | US-only | **Very high (press)** | Moderate | **Very high (press cross-ref)** | Industry press circularity |
| I | **Very high (US alignment labs)** | **US-only** | High | Moderate | Moderate | **3-paper, US-only, monoculture cluster** |

## Recommendations for §§ 2, 6, 8

1. **§2.5 (zero-shot TTS)**: balance Microsoft-heavy Cluster E citations with
   chen2024f5tts (Shanghai Jiao Tong), casanova2024xtts (Coqui), wang2024maskgct
   (CUHK-Shenzhen). Note Microsoft dominance in §8.
2. **§2.5 (identity-locked video)**: balance Alibaba-heavy Cluster F with
   cui2024hallo3 (Fudan-Baidu), zheng2025hunyuanvideo (Tencent),
   yang2024cogvideox (Tsinghua-Zhipu), li2024latentsync (ByteDance). Note
   Alibaba + CN-lab dominance + arXiv-heavy in §8.
3. **§2.6 (alignment)**: explicitly state the 3-paper scope. Note absence of
   RLAIF / KTO / IPO / ORPO as a scope boundary in §8.
4. **§2.1 (path 1 — license more)**: at most 3 Cluster-H citations in body;
   prefer T1 (H05, H13, H15) and T2 (H20). Note press-cross-ref inflation in §8.
5. **§8 Limitations** (agent-40) — enumerate:
   - English-language-only sources (L-Bias-1).
   - NYU-FAIR × 3 in Cluster B + Microsoft × 4 in Cluster E + Alibaba × 4 in
     Cluster F.
   - Cluster I is a 3-paper, US-only sub-corpus; does not survey full alignment
     landscape.
   - No non-English / non-Western / non-industry sources for
     identity-locked video or TTS.

## Confidence

**HIGH.** Bias flags derive from inspection of author affiliations, venue
counts, and temporal distributions in the LOCKED master pool. Recommendations
are actionable by agents 36 / 40 / 42 without further review.

## Changelog

- 2026-04-16 — Initial issue by agent-11-systematic-reviewer.
