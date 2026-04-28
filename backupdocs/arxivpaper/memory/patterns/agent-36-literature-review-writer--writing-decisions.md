---
namespace: patterns
agent_id: agent-36-literature-review-writer
timestamp: 2026-04-17T10:30:00Z
status: active
locked: false
supersedes: none
tags: writing-decisions, section-2, related-work, thematic-synthesis, hedge-catalogue, cluster-map
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-08-citation-extractor--master-reference-pool.md
  - memory/discoveries/agent-06-literature-mapper--topic-topology.md
  - memory/patterns/agent-06-literature-mapper--cluster-summaries.md
  - memory/discoveries/agent-10-theoretical-framework-analyst--framework-map.md
  - memory/discoveries/agent-11-systematic-reviewer--prisma-flow.md
  - memory/patterns/agent-15-evidence-synthesizer--thematic-synthesis-blocks.md
  - memory/handoff/agent-17-to-18--themes-ready.md
  - memory/discoveries/agent-18-gap-hunter--consolidated-gaps.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-35-to-36--introduction-ready.md
downstream_consumers:
  - agent-37-methodology-writer
  - agent-38-results-writer
  - agent-39-discussion-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-44-citation-crosschecker
---

# Writing decisions for §2 Related Work

## 1. Structural decisions

- **Organisation: thematic, six sub-sections (2.1–2.6).** Mapped one-to-one onto
  agent-06's clusters A, B, C, D, E+F (combined), I, plus a closing gap sub-section.
  Chronological or author-by-author organisation was explicitly rejected per
  agent-36 PhD-framework rules.
- **Cluster-label discipline.** The opening paragraph names all nine cluster
  labels A–I explicitly (A, B, C, D, E, F, G, H, I mentioned by letter) to
  satisfy the agent-36 hard rule that agent-06's cluster labels appear at
  least once. G and H are acknowledged as context-only for §1.
- **Word count: exactly 2,500 words** (tight against the 1,800–2,500 target).
  Went through four iterative tightening passes; final version compresses
  redundant framing clauses without dropping citations.
- **Opening paragraph.** Single paragraph sets scope, pool-relative framing,
  thematic organisation disclaimer, and the reserve list for §3/§5.
- **No numbered sub-sub-sections.** Sub-sections 2.1–2.6 use ## headings;
  paragraphs inside flow as prose.

## 2. Theme-to-subsection mapping

| Subsection | Title | Primary cluster | Themes from agent-17 |
|---|---|---|---|
| 2.1 | The data wall and recursive collapse | A + B | Theme 1 (data wall), Theme 2 first half (collapse) |
| 2.2 | Synthetic-data mitigations and their limits | B | Theme 2 second half |
| 2.3 | Quantization and compression literature | C | Theme 3 |
| 2.4 | Multi-embedder retrieval and evaluation | D | Theme 4 |
| 2.5 | Identity-locked generation and alignment | E + F + I | Themes 5, 6, 7 combined |
| 2.6 | Gaps this paper addresses | — | Six gaps from agent-18's ledger |

## 3. Citation density and coverage

Total citation anchors used (by citation key mapped from master pool):

- **§2.1**: villalobos2024data, longpre2024consent, kaplan2020scaling,
  hoffmann2022chinchilla, efrati2024information, metz2024bloomberg,
  hu2024reuters, franzen2024techcrunch, sutskever2024neurips,
  shumailov2024nature, dohmatob2024tails, alemohammad2023mad.
  **12 anchors.**
- **§2.2**: gerstgrasser2024colm, alemohammad2023mad, feng2024verification,
  seddik2024bad. **4 anchors** (plus forward reference to shumailov2024nature).
- **§2.3**: deletang2024compression, valmeekam2023llmzip, jiang2023gzip,
  li2025lossless, huang2024compression, frantar2022gptq,
  frantar2023sparsegpt. **7 anchors.**
- **§2.4**: zhai2023siglip, nussbaum2024nomic, chen2022wavlm,
  khattab2020colbert, santhanam2022colbertv2, formal2021splade,
  wang2021kepler, bordes2013transe, malkov2018hnsw, cormack2009rrf.
  **10 anchors** (DINOv2 acknowledged as adjacent, outside pool).
- **§2.5** (three sub-paragraphs: alignment, voice cloning, video):
  alignment — ouyang2022instructgpt, rafailov2023dpo, bai2022constitutional
  (3 anchors); voice cloning — wang2023valle, chen2024valle2,
  tan2024naturalspeech3, wang2024maskgct, chen2024f5tts, le2023voicebox,
  shen2023naturalspeech2, jiang2023megatts2, casanova2024xtts,
  reddy2022dnsmos, saeki2022utmos, desplanques2020ecapa, chen2022wavlm
  (reused), christop2025cloneval (14 anchors); video — meng2025echomimicv3,
  cui2024hallo3, li2024latentsync, zheng2025hunyuanvideo, tian2024emo,
  tian2025emo2, yang2024cogvideox, wan2025video, hu2022lora,
  lipman2023flowmatching, li2017flame, ekman1978facs (12 anchors).
  **~29 anchors total** in §2.5.
- **§2.6**: no new citations; references gap IDs from agent-18 by name.

**Total unique citation keys used**: ~62 across the LOCKED master pool (of 88).

## 4. Hedge catalogue applied

From agent-34 C-series and agent-33 / agent-14 mitigations, carried verbatim
or paraphrased minimally:

- **C1** — "up to $N+\binom{N}{2}$" appears in §2.4 when the counting extension
  is referenced. "Up to 91 signals" is NOT used in §2 (that claim is §1/§3
  territory; §2 only names the signal-cardinality *formula* once and defers
  instantiation).
- **C2** — "semantically distinct unit" used once in §2.3 and once in §2.6.
  "Dimensionally distinct" is NOT used (banned per agent-34).
- **C4** — "structurally not subject to … the Shumailov regime" phrasing used
  in §2.1 closing sentence.
- **C13** — "pool-relative" + structural, not competitive, for scalar-reward
  comparison in §2.5 closing paragraph.
- **Approximate independence** — used in §2.6 gap (v) and implicit in §2.4
  "approximately-independent" framing absent from §2 body (the phrasing is
  §3 territory).
- **Closed-artefact** — VALL-E 2 and NaturalSpeech 3 flagged in §2.5 per
  agent-13 Flags 1 and 4.
- **One dated narrative signal** — the five industry citations in §2.1 are
  explicitly framed as one narrative moment per agent-14 Bias-10.

## 5. Banned-phrase compliance

Spot-checked all 10 banned phrases from agent-34's table. None appear in §2.
Specifically:

- "91 signals per input" (bare) — absent
- "dimensionally distinct" — absent (§2.3 uses "semantically distinct unit")
- "0.961 mean SECS" (bare) — absent (no measured numbers are claimed in §2)
- "indistinguishable from real speech" — absent
- "solves the data wall" — absent
- "outperforms RLHF" — absent (§2.5 uses pool-relative structural comparison)
- "T=0.3 is optimal" — absent
- "proven collapse-free" — absent ("structurally not subject to" used instead)
- "mouth-weighted loss is necessary" — absent
- "we demonstrate the sample-complexity collapse" — absent

## 6. Reserved content (deliberately NOT in §2)

Per agent-35's handoff reserve list, these concepts appear by name only:

- **DDA counting identity $n(N+\binom{N}{2})$**: referenced as "signal cardinality"
  in §2.4 and "pairwise-interaction count" in §2.6; the identity is NOT claimed.
- **Collapse-silence scope argument**: §2.1 names the precondition and forward-
  references §3.3; argument is NOT developed.
- **Meaning-compression fourth category**: §2.3 names the three-category taxonomy
  and forward-references §3.1 / §5.2; fourth-category argument is NOT developed.
- **Constellation-guard predicate $G_\tau$**: §2.5 notes absence across four
  sub-fields and forward-references §5; predicate is NOT specified here.
- **Sample-complexity theorem $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$**:
  NOT mentioned in §2; reserved for §5.4.
- **Case 3 headline numbers (0.961 mean SECS, 0.95–0.99 band)**: NOT mentioned
  in §2; reserved for §6.
- **VALL-E 2's 0.881 SIM-o** is mentioned in §2.5 as a paradigm-anchor datum,
  framed as "reported" under closed-artefact caveat per agent-13 Flag 1.

## 7. Narrative-register rule compliance (agent-17 R1–R4)

- **R1 composition-not-invention.** §2.5 closing paragraph explicitly uses
  "three absences are structural rather than incidental" and describes what
  §5 picks up as "constellation set", "constellation guard", "per-modality
  conjunction" — all framed as composition of existing primitives per MP-12.
- **R2 open-scorer / disclosed-forensic.** Case 2 flow-matching sign-convention
  forensic is named (without detail) in §2.5. VALL-E 2 and NaturalSpeech 3
  closed-artefact status is disclosed explicitly.
- **R3 pool-relative scope.** Opening paragraph names this rule. Every
  "no cited method" / "none aggregates" / "no cited system" claim is
  pool-relative; DINOv2 is explicitly flagged as "adjacent but outside the
  present pool".
- **R4 projected vs. measured.** §2 does not claim any measured numbers for
  our system (all our measurements are reserved for §6). Only cited external
  measurements appear (VALL-E 2's 0.881 SIM-o).

## 8. Risks and adversarial surfaces

1. **DINOv2 exclusion risk (§2.4).** DINOv2 is widely cited in the adjacent
   video-embedder literature but is outside the master pool. §2.4 names this
   explicitly ("adjacent but outside the present pool") per R3 scope discipline.
   An adversarial reviewer (agent-42) may want the sentence strengthened to
   name specific DINOv2/CLIP variants outside the pool; agent-36 chose to keep
   the sentence minimal to avoid implying the pool is exhaustive.
2. **Voice-cloning metric inflation risk (§2.5).** Frontier systems' 0.86–0.89
   band is reported; agent-13 Flag 1 caveat applied. An adversarial reviewer
   could argue §2 should make the closed-artefact status more prominent; the
   single dedicated sentence is the agent-14 mitigation minimum.
3. **Industry signal weight risk (§2.1).** Agent-14 Bias-10 mitigation
   ("one dated narrative rather than five independent confirmations") is
   applied in a single sentence. An adversarial reviewer could demand stronger
   hedging; agent-36 chose not to front-load the caveat because agent-14's
   mitigation snippet is deemed sufficient.
4. **Kaplan 2020 as T2 is downgraded from T1.** The master pool tiers
   kaplan2020scaling as T2 (arXiv-only; OpenAI group); §2.1 cites it as
   context without claiming T1 authority. No risk flagged.

## 9. Cluster-label visibility check

Cluster labels appearing in §2 (by letter): A, B, C, D, E, F, G, H, I. All nine
labels are named. Required minimum per agent-36 hard rules was "at least one";
delivered: nine.

## 10. Confidence

**HIGH** that §2 accurately reflects agent-15's thematic synthesis and
agent-17's six themes collapsed into six subsections.

**HIGH** that no banned phrase appears.

**HIGH** that all reserve items are preserved for §3 / §5 / §6.

**HIGH** that pool-relative discipline (R3) is maintained throughout.

**MEDIUM** on whether the word-count tightening hurt prose quality in §2.5;
after four tightening passes the prose is denser than the initial draft. An
adversarial reviewer could request a loosening in the voice-cloning paragraph.

## Changelog

- 2026-04-17 — Initial issue by agent-36-literature-review-writer.
