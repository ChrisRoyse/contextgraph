---
namespace: patterns
agent_id: agent-12-context-tier-manager
timestamp: 2026-04-16T22:50:00Z
status: active
locked: false
supersedes: none
tags: writer-cues, per-section, abstract-to-9, citation-mandatory-useful-relevant
depends_on:
  - memory/patterns/agent-12-context-tier-manager--hot-warm-cold.md
  - memory/decisions/agent-05-research-planner--section-ownership.md
  - memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md
  - memory/patterns/agent-11-systematic-reviewer--evidence-grading.md
downstream_consumers:
  - agent-35-introduction-writer
  - agent-36-related-work-writer
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-conclusion-writer
  - agent-41-abstract-writer
  - agent-42-adversarial-reviewer
---

# Writer Cue Sheet — Per-Section Citation Cues

Cue legend per citation:
- **[HOT-M]** Hot-tier, **mandatory** for this section (claim fails without it).
- **[HOT-U]** Hot-tier, useful (paraphrase from findings expected).
- **[WARM-U]** Warm-tier, useful supporting context.
- **[WARM-R]** Warm-tier, potentially relevant if space allows.
- **[COLD-R]** Cold-tier, optional framing / industry-signal / sidebar.

Format for each section: mandatory hot-tier citations first (the section
cannot be adversarially defended without them), warm-tier corroborators next,
cold-tier context last. Numbers in parentheses after each key are the section
word budget recommendations per agent-05.

---

## Abstract — agent-41 (180–250 words) — synthesises Q01, Q04, Q06, Q12, Q15

**Mandatory (≤ 3 cite-by-name allowed in an abstract, so pick the three most load-bearing):**
- **villalobos2024data [HOT-M]** — cite implicitly for the data-wall framing ("the accessible stock of training text is bounded").
- **chen2024valle2 [HOT-M]** — the Case 3 comparator; name when reporting SECS 0.961 vs. human parity.
- **chen2022wavlm [HOT-M]** — the SECS scorer for the 0.961 number.

**Useful (only if word budget permits — do not force):**
- **hu2022lora [HOT-U]** — "one reference per modality + frozen centroids + LoRA".
- **ouyang2022instructgpt [HOT-U]** — contrast with RLHF in the "third path" line.

**Not for abstract:** any Cluster-H industry signal. Any T3 press citation.

---

## §1 Introduction — agent-35 (900–1,200 words) — Q01, Q03, Q04 (teaser), Q06 (teaser)

**Mandatory hot-tier:**
- **villalobos2024data [HOT-M]** — data-wall exhaustion 2026–2032 band; THE anchor.
- **longpre2024consent [HOT-M]** — accessible-fraction collapse (~25% C4 in 12 months).
- **hoffmann2022chinchilla [HOT-M]** — Chinchilla / compute-optimal; why the wall arrives sooner.
- **shumailov2024nature [HOT-M]** — the Nature collapse paper; the "why path 2 (synthesise) is hazardous" anchor.
- **deletang2024compression [HOT-M]** — compression-as-intelligence teaser for §3.

**Useful hot-tier (teasers):**
- **chen2022wavlm [HOT-U]** — DDA substrate teaser (one embedder mention).
- **chen2024valle2 [HOT-U]** — Case 3 teaser (mention "human-parity zero-shot TTS").
- **ouyang2022instructgpt [HOT-U]** + **rafailov2023dpo [HOT-U]** + **bai2022constitutional [HOT-U]** — scalar-reward alignment contrast teaser.

**Warm-tier:**
- **kaplan2020scaling [WARM-U]** — needed only if §1 first-uses "scaling law".

**Cold-tier (framing only; cap at 2 cites in §1):**
- **woo2024information [COLD-R]** or **franzen2024techcrunch [COLD-R]** or **hu2024reuters [COLD-R]** — pick ONE industry-signal footnote.
- **sutskever2024neurips [COLD-R]** — the "pre-training as we know it will end" oral.

**§1 citation count target: 8–10.**

---

## §2 Related Work — agent-36 (1,200–1,800 words) — Q01, Q02, Q03, Q05, Q06, Q09

### §2.1 Scaling laws + data wall
- **villalobos2024data [HOT-M]**, **longpre2024consent [HOT-M]**, **hoffmann2022chinchilla [HOT-M]**.
- **kaplan2020scaling [WARM-U]** — power-law foundation.
- **cerullo2024cbs [COLD-R]** OR **reuters2024reddit [COLD-R]** OR **openai2024reddit [COLD-R]** — pick 1–2 licensing signals.
- **singhal2023nature [COLD-R]** / **tu2025nature [COLD-R]** / **wu2023bloomberggpt [COLD-R]** — domain-specialised exemplars (pick 1–2).

### §2.2 Model collapse + synthetic data
- **shumailov2024nature [HOT-M]**, **alemohammad2023mad [HOT-M]**.
- **dohmatob2024tails [WARM-U]**, **dohmatob2024regression [WARM-U]**, **gerstgrasser2024colm [WARM-U]**, **feng2024verification [WARM-U]**, **seddik2024collapse [WARM-U]**.

### §2.3 Compression-as-intelligence
- **deletang2024compression [HOT-M]**, **huang2024compression [HOT-M]**.
- **li2025lossless [WARM-U]** (Nature MI), **jiang2023gzip [WARM-U]** (gzip+k-NN).
- **valmeekam2023llmzip [COLD-R]** — one benchmark number only.

### §2.4 Multi-modal embedders + retrieval
- **chen2022wavlm [HOT-M]**, **zhai2023siglip [HOT-M]**, **malkov2018hnsw [HOT-M]**.
- **santhanam2022colbertv2 [WARM-U]**, **khattab2020colbert [WARM-U]**, **formal2021splade [WARM-U]**, **wang2021kepler [WARM-U]**, **bordes2013transe [WARM-U]**, **nussbaum2024nomic [WARM-U]**, **cormack2009rrf [WARM-U]**.

### §2.5 Video (identity-locked) + voice cloning (Case-2/3 comparator lanes)
- **chen2024valle2 [HOT-M]**, **meng2025echomimicv3 [HOT-M]**.
- **wang2023valle [WARM-U]**, **tan2024naturalspeech3 [WARM-U]**, **wang2024maskgct [WARM-U]**, **chen2024f5tts [WARM-U]**.
- **cui2024hallo3 [COLD-R]**, **zheng2025hunyuanvideo [COLD-R]**, **tian2024emo [COLD-R]**, **tian2025emo2 [COLD-R]**, **yang2024cogvideox [COLD-R]**, **le2023voicebox [COLD-R]**, **shen2023naturalspeech2 [COLD-R]**, **jiang2023megatts2 [COLD-R]**, **casanova2024xtts [COLD-R]**.

### §2.6 Alignment (RLHF / DPO / CAI)
- **ouyang2022instructgpt [HOT-M]**, **rafailov2023dpo [HOT-M]**, **bai2022constitutional [HOT-M]**.

**§2 citation count target: 25–32.** Microsoft-balance and Alibaba-balance rules per agent-11 bias file.

---

## §3 Framework (DDA + meaning compression) — agent-37 (1,200–1,600 words) — Q04, Q05

### §3.1 Four-category taxonomy
- **deletang2024compression [HOT-M]**, **li2025lossless [WARM-U]**, **huang2024compression [HOT-M]**, **jiang2023gzip [WARM-U]**.

### §3.2–§3.4 DDA counting (N + C(N,2))
- **chen2022wavlm [HOT-M]**, **zhai2023siglip [HOT-M]**, **malkov2018hnsw [HOT-M]**.
- **nussbaum2024nomic [WARM-U]**, **santhanam2022colbertv2 [WARM-U]**, **formal2021splade [WARM-U]**, **wang2021kepler [WARM-U]**, **bordes2013transe [WARM-U]**, **khattab2020colbert [WARM-U]**, **cormack2009rrf [WARM-U]**.

### §3.3 DDA vs. synthetic data (re-cite collapse)
- **shumailov2024nature [HOT-M]** (re-cite), **alemohammad2023mad [HOT-M]** (re-cite).
- **gerstgrasser2024colm [WARM-U]**, **feng2024verification [WARM-U]**.

### §3.5 Four-category table
- **deletang2024compression [HOT-M]**, **huang2024compression [HOT-M]**, **li2025lossless [WARM-U]**.

**§3 citation count target: 10–14.**

---

## §4 Systems (Context Graph + ClipCannon) — agent-37 (700–900 words) — Q10, Q11

### §4.1 Context Graph (13 embedders)
- **chen2022wavlm [HOT-M]**, **zhai2023siglip [HOT-M]**, **malkov2018hnsw [HOT-M]**.
- **santhanam2022colbertv2 [WARM-U]**, **formal2021splade [WARM-U]**, **wang2021kepler [WARM-U]**, **bordes2013transe [WARM-U]**, **khattab2020colbert [WARM-U]**, **nussbaum2024nomic [WARM-U]**, **cormack2009rrf [WARM-U]**.
- **mcp2024spec [COLD-R]** — MCP protocol citation.

### §4.2 ClipCannon (7 modalities × 4,044D, 23-stage DAG)
- **hu2022lora [HOT-M]** (LoRA stage), **li2017flame [HOT-M]** (FLAME modality).
- **ekman1978facs [WARM-U]** (FACS modality), **baevski2020wav2vec [WARM-U]**, **bain2023whisperx [WARM-U]**, **rouard2023htdemucs [WARM-U]**.
- **campello2013hdbscan [COLD-R]**, **zhang1996birch [COLD-R]**, **google2020mediapipe [COLD-R]**, **tinachen2021yolov5face [COLD-R]**, **resembleai2024enhance [COLD-R]**, **mcp2024spec [COLD-R]**.

**§4 citation count target: 12–16.**

---

## §5 TCT method — agent-37 (1,100–1,400 words) — Q06, Q07, Q08

### §5.1 Frozen centroid construction
- **chen2022wavlm [HOT-M]** (audio-speaker centroid), **zhai2023siglip [HOT-M]** (visual centroid), **li2017flame [HOT-M]** (FLAME/expression centroid).

### §5.2 Training via flow-matching + LoRA
- **lipman2023flowmatching [HOT-M]** — flow-matching foundational (SINGLE-SOURCE; attribute).
- **hu2022lora [HOT-M]** — LoRA foundational (SINGLE-SOURCE; attribute).
- **meng2025echomimicv3 [HOT-M]** — Case 2 instantiation.

### §5.3 Runtime per-output guard (cosine gate)
- **chen2022wavlm [HOT-M]** — WavLM SECS gate.
- **reddy2022dnsmos [WARM-U]**, **saeki2022utmos [WARM-U]** — supplementary evaluators.

### §5.4 Contrast vs. scalar-reward alignment
- **ouyang2022instructgpt [HOT-M]**, **rafailov2023dpo [HOT-M]**, **bai2022constitutional [HOT-M]**.

**§5 citation count target: 8–10.**

---

## §6 Case Studies — agent-38 (1,500–2,000 words) — Q12, Q13, Q14, Q15, Q17

Presentation ordering per agent-05 LOCKED: §6.3 → §6.2 → §6.1 (T1 first).

### §6.3 Case 3 Voice cloning (T1 anchor) — strongest evidence
- **chen2022wavlm [HOT-M]** — SECS scorer. Mandatory.
- **chen2024valle2 [HOT-M]** — the comparator for 0.961 SECS. Mandatory.
- **reddy2022dnsmos [WARM-U]**, **saeki2022utmos [WARM-U]** — parity scorers.
- **christop2025cloneval [COLD-R]** — future-cohort benchmark mention.

### §6.2 Case 2 Santa identity-locked talking head (T2+T3)
- **meng2025echomimicv3 [HOT-M]** — base model. Mandatory.
- **hu2022lora [HOT-M]** — LoRA fine-tune. Mandatory.
- **lipman2023flowmatching [HOT-M]** — flow-matching sign-bug forensic anchor. Mandatory.
- **li2017flame [HOT-M]** — FLAME modality.
- **li2024latentsync [COLD-R]** — lip-sync metric (projected).
- **wan2025video [COLD-R]** — sign-convention lineage.
- **tinachen2021yolov5face [COLD-R]** — YOLO mouth-landmark forensic.
- **cui2024hallo3 [COLD-R]** — portrait-animation baseline.

### §6.1 Case 1 Shakespeare stylistic LoRA (T2+T3)
- **hu2022lora [HOT-M]** — LoRA substrate.
- **rafailov2023dpo [HOT-U]** — DPO context (Case 1 forensic echo-bug).
- **ouyang2022instructgpt [HOT-U]** — SFT context.

**§6 citation count target: 10–14.**

---

## §7 Discussion — agent-39 (800–1,100 words) — synthesises Q01–Q17

**Mandatory (re-cited from hot tier):**
- **villalobos2024data [HOT-M]** — three-paths framing synthesis.
- **shumailov2024nature [HOT-M]** — DDA-vs-collapse distinction.
- **deletang2024compression [HOT-M]** — compression-as-intelligence.
- **chen2022wavlm [HOT-M]** — cross-study synthesis anchor.
- **chen2024valle2 [HOT-M]** — Case 3 comparator synthesis.
- **ouyang2022instructgpt [HOT-M]**, **rafailov2023dpo [HOT-M]**, **bai2022constitutional [HOT-M]** — alignment-paradigm contrast closure.

**Useful:**
- **hoffmann2022chinchilla [HOT-U]** — synthesis on Chinchilla + data-wall.
- **hu2022lora [HOT-U]** — cross-case substrate.

**Cluster-H body quota ≤ 6** enforcement point: §7 may cite **singhal2023nature [COLD-R]** / **tu2025nature [COLD-R]** / **wu2023bloomberggpt [COLD-R]** / **henderson2022pileoflaw [WARM-U]** — max 2 at once.

**§7 citation count target: 10–14** (re-cites allowed).

---

## §8 Limitations — agent-29 + agent-39 (500–700 words) — Q16, Q18 (proprietary items), Q19 (ethics hook)

**Mandatory (must appear to satisfy agent-11 bias + hedge rules):**
- **shumailov2024nature [HOT-M]** — collapse-confusion threat.
- **chen2022wavlm [HOT-M]** — cross-encoder scoring-bias (Q16).
- **villalobos2024data [HOT-M]** — generalisation-scope limit.
- **longpre2024consent [HOT-M]** — consent/accessible-fraction re-cite.

**Useful:**
- **huang2024compression [HOT-U]** — compression-intelligence as proxy caveat.
- **gerstgrasser2024colm [WARM-U]** — mitigation alternative.
- **christop2025cloneval [COLD-R]** — cross-cohort benchmark future-work tie.
- **henderson2022pileoflaw [WARM-U]** — consent-aware exemplar for §8.6 ethics.

**Cold, if ethics §8.6 needs depth:**
- **singhal2023nature [COLD-R]**, **tu2025nature [COLD-R]** — responsible-use exemplars.
- **anthropic2024mcp [COLD-R]** — MCP safety framing (sparingly).

**§8 citation count target: 6–10.**

---

## §9 Conclusion — agent-40 (250–400 words) — Q20

**Mandatory:**
- **villalobos2024data [HOT-M]** — close on the data-wall reframing (not "solve AGI" per mission brief).
- **deletang2024compression [HOT-M]** — compression-as-intelligence as the claim ceiling.

**Useful (pick 2 at most):**
- **chen2022wavlm [HOT-U]** — embedder-independence future-work.
- **rafailov2023dpo [HOT-U]** — alignment-paradigm contrast closure.
- **li2025lossless [WARM-U]** — compression-direction future-work (Q20).
- **christop2025cloneval [WARM-R]** — benchmark future-work anchor.
- **desplanques2020ecapa [COLD-R]** — cross-encoder ECAPA-TDNN future-work.
- **longpre2024consent [WARM-R]** — data-stewardship future-work.

**§9 citation count target: 3–5.**

---

## Per-section totals check

| Section | Target body cites | Hot-M count | Hot-U + Warm count | Cold count |
|---|---|---|---|---|
| Abstract | 3 | 3 | 0–2 | 0 |
| §1 | 8–10 | 5 | 2–5 | 1–2 |
| §2 | 25–32 | 11 | 13–15 | 6–8 |
| §3 | 10–14 | 8 | 7–9 | 0 |
| §4 | 12–16 | 5 | 5–6 | 2–5 |
| §5 | 8–10 | 7 | 2 | 0 |
| §6 | 10–14 | 7 | 2–3 | 3–5 |
| §7 | 10–14 | 8 | 2 | 1–2 |
| §8 | 6–10 | 4 | 3–5 | 1–3 |
| §9 | 3–5 | 2 | 1–2 | 0–1 |

**Body-unique citation estimate:** 40–55 unique keys; well within the
"30–60 references" mission-brief target.

## Cross-section re-cite bookkeeping

Citations that appear in ≥ 3 sections:
- **chen2022wavlm** (Abstract, §2.4, §3.2, §4.1, §5.1, §5.3, §6.3, §7, §8, §9) — 9+ re-cites. Justified by CL-11, CL-12, CL-16, CL-17, CL-18, CL-27.
- **hu2022lora** (§2.6 teaser, §4.2, §5.2, §6.1, §6.2) — 5 re-cites. Justified by SINGLE-SOURCE.
- **meng2025echomimicv3** (§2.5, §4.2, §5.2, §6.2) — 4 re-cites. Justified by Case 2 base model.
- **villalobos2024data** (Abstract, §1, §2.1, §7, §8, §9) — 6 re-cites. Anchor.
- **shumailov2024nature** (§1, §2.2, §3.3, §7, §8) — 5 re-cites. Nature anchor.
- **deletang2024compression** (§1, §2.3, §3.1, §3.5, §7, §9) — 6 re-cites.
- **chen2024valle2** (Abstract, §2.5, §6.3, §7) — 4 re-cites.
- **ouyang2022instructgpt + rafailov2023dpo + bai2022constitutional** (§1 teaser, §2.6, §5.4, §7) — 4 re-cites each.

Re-citing the same work across multiple sections is expected for foundational
load-bearing references and does not inflate the 30–60 reference target.

## Rules for agent-42 adversarial reviewer

- Any section that **misses a [HOT-M]** flag → P0 flag.
- Any section that uses a cold-tier citation as **primary evidence** (not
  framing) → P1 flag.
- Any AUTHOR-EMPIRICAL claim without V12/V17/V22 hedge within 2 sentences
  of the citation → P1 flag.
- Any SINGLE-SOURCE claim (hu2022lora, lipman2023flowmatching,
  malkov2018hnsw, huang2024compression, longpre2024consent,
  henderson2022pileoflaw) cited as consensus rather than attribution → P1 flag.
- **Cluster-H body-quota > 6** → P0 flag.
- Any citation outside LOCKED master pool → P0 flag.

## Confidence

**HIGH.** Every mandatory/useful flag traces to (a) agent-08 Q-coverage,
(b) agent-11 GRADE, and (c) agent-05 section ownership + word budgets.

## Changelog

- 2026-04-16 — Initial issue by agent-12-context-tier-manager.
