---
namespace: handoff
agent_id: agent-36-literature-review-writer
timestamp: 2026-04-17T10:35:00Z
status: active
tags: handoff, agent-37, methodology-writer, related-work-ready, section-2, reserve-list
---

# Handoff: Agent-36 (literature-review-writer) → Agent-37 (methodology-writer)

## Read these files FIRST, in this order

1. `/home/cabdru/contextgraph/docs/arxivpaper/sections/02_related_work.md` — the
   §2 prose as it stands. Read this BEFORE writing §5 (TCT method) so the
   methodology section's framing does not re-cite the retrieval literature
   already exhausted in §2.4 or the alignment anchors already situated in §2.5.
2. `/home/cabdru/contextgraph/docs/arxivpaper/sections/01_introduction.md` — §1
   for the funnel context.
3. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-36-literature-review-writer--writing-decisions.md`
   — the writing decisions, hedge catalogue, and **reserve list** describing
   which concepts §2 deliberately did NOT develop.
4. `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md`
   — the paste-ready hedges and banned-phrase list (load-bearing for §3 / §5).
5. `/home/cabdru/contextgraph/docs/arxivpaper/memory/decisions/agent-25-method-designer--tct-protocol.md`
   — the locked TCT three-phase protocol specification.

## What §2 accomplishes (summary)

§2 organises the related-work discussion into six thematic sub-sections (2.1–2.6)
covering the data wall and recursive collapse (Clusters A + B), synthetic-data
mitigations (Cluster B), compression (Cluster C), multi-embedder retrieval
(Cluster D), identity-locked generation and alignment (Clusters E + F + I), and
a closing gap sub-section. Final word count: exactly 2,500 words. All nine
cluster labels (A–I) appear. All hedges from agent-34's C-series and
agent-17's R1–R4 register rules apply.

## What §3 (framework) and §5 (method) MUST NOT re-cite

Per agent-35's reserve list and agent-36's own reserve discipline, the
following concepts were set up in §2 but **deliberately not developed**. §3
and §5 should pick them up without re-citing the §2 anchors:

### Items §3 owns

1. **DDA counting identity $n \cdot (N + \binom{N}{2})$** — §2.4 names the
   "signal cardinality" and §2.6 names the "pairwise-interaction count" as an
   upper bound under approximate independence. §3.4 formalises the identity;
   §3 should NOT re-cite cormack2009rrf or malkov2018hnsw (already done in
   §2.4).
2. **Meaning-compression fourth category** — §2.3 ends with the three-category
   taxonomy (bit / weight / activation) and flags §3.1 / §5.2 as owners of
   the fourth-category argument. §3 / §5 should NOT re-cite
   deletang2024compression, huang2024compression, or the GPTQ/SparseGPT/
   TurboQuant family; those are in §2.3. §3's four-category taxonomy table
   may reference them by key but should not narrate them.
3. **Collapse-silence scope argument** — §2.1 closes with "structurally not
   subject to, rather than a refutation of, the Shumailov regime" and
   forward-references §3.3. §3 / §5 should NOT re-narrate the four collapse
   anchors (shumailov2024nature, dohmatob2024tails, alemohammad2023mad,
   gerstgrasser2024colm). §3.3 picks up the scope claim directly.

### Items §5 owns

4. **Constellation-guard predicate $G_\tau(\hat y; C_P) = \mathbb{1}[\forall m:
   \cos(\phi_m(\hat y), c_m) \geq \tau_m]$** — §2.5 ends by naming three
   absences ("constellation set", "constellation guard", "per-modality
   conjunction") and forward-references §5. §5.3 specifies the predicate; §5
   should NOT re-cite ouyang2022instructgpt, rafailov2023dpo, or
   bai2022constitutional for the structural comparison (already done in §2.5
   closing paragraph).
5. **Multi-clip centroid aggregation** — §2.5 closes the voice-cloning
   paragraph noting no cited system aggregates a multi-clip reference set.
   §5.1 specifies the L2-normalised-mean-of-normalised construction; §5
   should NOT re-cite the TTS cluster (wang2023valle, chen2024valle2,
   tan2024naturalspeech3, wang2024maskgct, chen2024f5tts, le2023voicebox,
   shen2023naturalspeech2, jiang2023megatts2, casanova2024xtts) for the
   "no multi-clip centroid" framing; that framing is done.
6. **Runtime per-output geometric guard** — §2.4 and §2.5 both close with
   this framing. §5.3 specifies the guard; §5 should NOT re-cite the
   retrieval and generation sub-fields' post-generation evaluation cluster
   for the "absent from the pool" framing.
7. **TCT three-phase method (Phase 1 centroid; Phase 2 geometric-proximity
   training; Phase 3 guard)** — §2 does not mention the three-phase structure
   at all. §5 owns the entire specification.
8. **Sample-complexity theorem $\mathcal{T}_{\mathrm{TCT}}(P) = O(M \cdot k)$**
   — §2 does not mention the theorem. §5.4 owns Theorem 1. Do NOT re-cite
   anything from §2 for the theorem's setup; the theorem is derived from the
   Axioms stated in §3.

### Items §6 owns

9. **Case 3 measured numbers (0.961 mean SECS, max 0.975, 10 held-out
   sentences, encoder-matched protocol, DNSMOS 3.93 vs 3.93, UTMOS 3.012 vs
   2.997)** — §2 does NOT claim any measured results. §6.3 owns all Case 3
   numbers. The C5 + C6 + C7 + C8 hedges apply.
10. **Case 2 flow-matching sign-convention forensic** — §2.5 names the
    existence of the forensic in one sentence and flags it as engineering
    narrative; §6.2 owns the full forensic paragraph.
11. **Case 1 Shakespeare echo-bug forensic and 15/15 FSV status** — NOT
    mentioned in §2. §6.1 owns.

## Reserved citation keys (used in §2 — do not re-cite in §3 / §5)

Using each citation key only once per section is the discipline. §2 has used
the following keys; §3 and §5 should avoid re-citing these unless absolutely
necessary:

- **Cluster A+B (§2.1, §2.2)**: villalobos2024data, longpre2024consent,
  kaplan2020scaling, hoffmann2022chinchilla, efrati2024information,
  metz2024bloomberg, hu2024reuters, franzen2024techcrunch,
  sutskever2024neurips, shumailov2024nature, dohmatob2024tails,
  alemohammad2023mad, gerstgrasser2024colm, feng2024verification,
  seddik2024bad.
- **Cluster C (§2.3)**: deletang2024compression, valmeekam2023llmzip,
  jiang2023gzip, li2025lossless, huang2024compression, frantar2022gptq,
  frantar2023sparsegpt.
- **Cluster D (§2.4)**: zhai2023siglip, nussbaum2024nomic, chen2022wavlm,
  khattab2020colbert, santhanam2022colbertv2, formal2021splade,
  wang2021kepler, bordes2013transe, malkov2018hnsw, cormack2009rrf.
- **Cluster E (§2.5)**: wang2023valle, chen2024valle2,
  tan2024naturalspeech3, wang2024maskgct, chen2024f5tts, le2023voicebox,
  shen2023naturalspeech2, jiang2023megatts2, casanova2024xtts,
  reddy2022dnsmos, saeki2022utmos, desplanques2020ecapa,
  christop2025cloneval.
- **Cluster F (§2.5)**: meng2025echomimicv3, cui2024hallo3,
  li2024latentsync, zheng2025hunyuanvideo, tian2024emo, tian2025emo2,
  yang2024cogvideox, wan2025video, hu2022lora, lipman2023flowmatching,
  li2017flame, ekman1978facs.
- **Cluster I (§2.5)**: ouyang2022instructgpt, rafailov2023dpo,
  bai2022constitutional.

**Permitted re-cite exceptions**: lipman2023flowmatching (§5 method may
re-cite for Phase-2 loss derivation if load-bearing); chen2022wavlm (§6.3
re-cites for the WavLM-base-plus-sv scorer identity); hu2022lora (§6.1 and
§6.2 re-cite for the LoRA rank specification in Cases 1 and 2). These are
pre-approved by agent-35 / agent-25 protocol specs.

## Hedges that MUST carry into §3 and §5

All 10 hedges below from agent-34 are still load-bearing:

- **C1** — "up to $N + \binom{N}{2}$" / "up to 91 signals" — every DDA count
  mention. §3.4 states the identity *with* this hedge.
- **C2** — "semantically distinct unit" (never "dimensionally distinct") —
  §3.1 / §5.2 meaning-compression.
- **C3** — "per-output verifiable relative to the chosen centroid panel" —
  §5.3 guard spec.
- **C4** — "structurally not subject to the generator-in-loop recursion
  regime of Shumailov et al. (2024)" — §3.3 scope claim.
- **C5** — "encoder-matched protocol" — mandatory companion sentence for
  every 0.961 SECS mention in §6.3.
- **C12a / C12b split** — constructive theorem vs. one T1 empirical anchor.
  §5.4 Theorem 1 is C12a-only; §6.3 + §7 headlines carry C12b.
- **C13** — "pool-relative" + structural for scalar-reward contrast.
  §5.1 / §7.
- **Approximate independence** (V11) — every Axiom-2 mention in §3.
- **Frozen / frozen-with-LoRA** (V18) — every §5 / §6 mention.

## Banned phrases (still banned in §3 / §5)

All 10 banned phrases from agent-34's table remain banned. §2 is clean; §3
and §5 must stay clean.

## What §3 should produce for §5

- A locked Axiom set (1, 2, 3) per agent-21 and agent-25 specs.
- The counting identity $|D'| = n \cdot (N + \binom{N}{2})$ with C1 hedge.
- The four-category compression taxonomy table (bit / weight / activation /
  meaning).
- The collapse-silence scope claim with C4 phrasing.
- The meaning-compression ratio $\mathrm{MC} = N(N+1)/2$ as constructive upper
  bound.

## What §5 should produce for §6

- The TCT three-phase protocol specification (Phase 1 centroid; Phase 2
  training; Phase 3 guard).
- The constellation-guard predicate $G_\tau$ with C3 hedge.
- Theorem 1 (sample-complexity) with C12a constructive framing.
- The pool-relative structural-comparison framing against scalar-reward
  alignment (C13).

## Files agent-36 produced

- `/home/cabdru/contextgraph/docs/arxivpaper/sections/02_related_work.md`
  (the §2 prose; 2,500 words; no frontmatter per the section-file rule)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/patterns/agent-36-literature-review-writer--writing-decisions.md`
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/handoff/agent-36-to-37--related-work-ready.md`
  (this file)
- `/home/cabdru/contextgraph/docs/arxivpaper/memory/reports/agent-36-literature-review-writer--final-report.md`

## Quality-gate status delivered by agent-36

| Gate | Status |
|---|---|
| §2 prose file written at `sections/02_related_work.md` with `# 2. Related Work` header | **PASS** |
| Word count within 1,800–2,500 target (2,500 words) | **PASS** |
| Thematic organisation (six sub-sections, NOT chronological, NOT author-by-author) | **PASS** |
| All nine cluster labels A–I named at least once | **PASS** |
| Six sub-sections map to agent-06 clusters | **PASS** |
| 60+ citation anchors from the LOCKED master pool | **PASS** (~62 unique keys) |
| No banned phrase from agent-34 table | **PASS** |
| All agent-34 C-series hedges applied where relevant | **PASS** |
| R1–R4 narrative-register rules respected | **PASS** |
| Reserve list intact (DDA identity / collapse scope / 4th category / TCT method / $G_\tau$ / measured numbers all deferred) | **PASS** |
| APA 7th in-text format | **PASS** |
| No Python / Rust code blocks in body | **PASS** |
| No frontmatter on the section file | **PASS** |

## Confidence

**HIGH** that §2 will not force §3 or §5 into redundant citation work.

**HIGH** that the reserve list is complete.

**HIGH** that no banned phrase slipped through.

**MEDIUM** on agent-42 adversarial review: the DINOv2 exclusion and the
voice-cloning closed-artefact hedging may draw requests for amplification.

## Changelog

- 2026-04-17 — Initial handoff by agent-36-literature-review-writer.
