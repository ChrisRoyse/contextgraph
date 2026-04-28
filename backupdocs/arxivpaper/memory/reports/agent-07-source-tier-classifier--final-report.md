---
namespace: reports
agent_id: agent-07-source-tier-classifier
timestamp: 2026-04-16T17:20:00Z
status: active
tags: final-report, smap, tier-classification, qg1, g8
---

# Agent-07 Source-Tier-Classifier — Final Report (SMAP)

## Situation

The arXiv manuscript's evidence pool had been mapped by agent-06 (literature-mapper)
into a six-cluster field topology of 55 central citations (+ 1 supplementary), drawn
from four verified evidence briefs:

- `_evidence_01_data_wall.md` (21 refs on scaling laws, model collapse, compression)
- `_evidence_02_voice_cloning.md` (14 refs on zero-shot TTS + SECS metrics)
- `_evidence_03_video_embedders.md` (31 refs on embedders + video generation +
  pipeline components)
- `_evidence_04_enterprise_market.md` (22 refs on licensing / industry signals /
  domain LLMs)

The full union across briefs (post-deduplication) is **85 citations**, some of which
were not central to agent-06's topology (notably the 9-citation Cluster G of
auxiliary pipeline components used by ClipCannon §4 and the 20-citation Cluster H
enterprise-market pool for Q19).

Pipeline quality gate **QG1 G8** requires ≥ 80% of references to be Tier 1 or Tier 2
(peer-reviewed top-tier OR peer-reviewed reputable OR arXiv preprint by a routinely-
T1-publishing group). Tier 3 (industry press, company blogs, GitHub software,
official specs, books, government web references) is acceptable only for claims where
no T1/T2 equivalent exists.

## Mission

Classify **every** citation in the evidence pool into T1 / T2 / T3 per the task-brief
tier definitions, ensuring ≥ 80% of *in-body* citations are T1 or T2. Justify every
T3 keeper. Flag unverified references as ineligible. Produce the locked master tier
table that downstream writers (agent-36 related-work, agent-38 case-studies, agent-44
citation specialist) cite by key.

## Action

1. **Read required context** — mission brief, ReWOO plan, quality gates,
   agent-06 topology, agent-06 structural holes, agent-06→07 handoff.
2. **Read all four evidence briefs in full.** Enumerated the complete citation
   union (85 refs, post-dedupe) — noting that this exceeds agent-06's central 55
   because §4 Systems and §6 Case Studies reference auxiliary pipeline components
   (Cluster G) that agent-06 did not central-list.
3. **Tiered each citation** using the task-brief definitions:
   - T1 requires a top-tier peer-reviewed venue (Nature / Science / NeurIPS / ICML /
     ICLR / CVPR / ACL / EMNLP / NAACL / IEEE flagship journals / top-tier ACM SIGs)
     with DOI.
   - T2 is peer-reviewed at a reputable venue (COLM / Interspeech / ICASSP / SIGIR /
     TACL / ACM TOG / domain journals) OR arXiv-only from a research group that
     routinely publishes at T1 venues.
   - T3 is industry/news/blog/GitHub/specs/books/gov-web.
4. **Organised into 8 clusters (A–H)** aligning with agent-06's 6 clusters plus
   new G (aux pipeline) and H (enterprise-market).
5. **Wrote per-T3 justifications** in a companion file: for each of 10 core-set T3s
   and 2 likely body-cited enterprise-market T3s, recorded the specific claim, the
   no-Tier-1-equivalent reason, and a KEEP/DROP recommendation.
6. **Flagged 2 ineligible references** (Qwen3-TTS, Demucs v1/v2/v3) that carry the
   "Unverified / Excluded" label in brief 03 — NOT eligible for citation unless
   agent-08 re-verifies them.
7. **Flagged 1 known gap** (Q06 RLHF/DPO/CAI triad absent from the evidence pool)
   inherited from agent-06 handoff — agent-08 or agent-11 must source and tier.
8. **Computed per-cluster and overall T1+T2 percentages** with a sensitivity
   analysis on Cluster-H curation.
9. **Wrote five files** per the task requirements: master tier table (LOCKED), T3
   justifications, percent-by-tier stats, handoff to agent-08, this final report.

## Produced

| Namespace | File | Purpose |
|---|---|---|
| decisions | `agent-07-source-tier-classifier--tier-assignments.md` | **LOCKED master tier table (85 citations × 8 clusters × T1/T2/T3 + rationale + load-bearing Q-IDs)** |
| discoveries | `agent-07-source-tier-classifier--tier-3-justifications.md` | Per-T3 justification + KEEP/DROP decisions |
| patterns | `agent-07-source-tier-classifier--citation-percent-by-tier.md` | Summary stats, sensitivity analysis, QG1 pass evidence |
| handoff | `agent-07-to-08--tiers-locked.md` | Agent-08's starting instructions + known gaps |
| reports | `agent-07-source-tier-classifier--final-report.md` | This file (SMAP) |

## Key numbers

- **Core (A–G) reference set:** 65 citations — T1=20 (31%), T2=35 (54%), T3=10 (15%).
  **T1+T2 = 85%** ✅ PASS the ≥ 80% QG1 G8 gate.
- **Projected in-body set (A–G + curated Cluster-H ≤ 6):** 70 citations —
  **T1+T2 = 84%** ✅ PASS.
- **Full pool (A–H uncurated):** 85 citations — **T1+T2 = 68%** ❌ FAIL
  (why Cluster H must be curated; the ReWOO plan already mandates this).
- 19 of 20 essential questions have a Tier-1 anchor in the pool; Q06 RLHF/DPO/CAI
  is the known gap (not in briefs — agent-08/11 to add).
- 2 ineligible references held out pending re-verification.

## Load-bearing T3s in body (10 expected)

| ID | Source | Reason T3 is acceptable |
|---|---|---|
| A05 | The Information | Industry-signal anchor; no peer-reviewed equivalent for late-2024 corporate strategy shift |
| A06 | Bloomberg | Corroborator to A05; diversifies the industry observation |
| A07 | Reuters | Third corroborator for the late-2024 diminishing-returns framing |
| A09 | Sutskever NeurIPS 2024 oral | Attribution-only (inherently oral); single-use cap |
| F12 | Ekman & Friesen 1978 FACS | Canonical print reference; task brief cites this as the T3 exemplar |
| G06 | MediaPipe | Software release; cited for reproducibility disclosure |
| G07 | YOLOv5-Face | Software release; cited for reproducibility disclosure |
| G08 | Resemble Enhance | Software release; cited for reproducibility disclosure |
| G09 | MCP spec | Official specification; task brief cites this as a T3 exemplar |
| H10 or H11 + H18 | Reuters / CBS / Variety licensing-deal figures | Needed to concretise the "path 1 — license more" sub-argument in §1 |

**A08 TechCrunch and A09 Sutskever oral** are both **drop-candidates** per the
T3-justifications file. Dropping either (or both) moves T1+T2 to 85–87%.

## Problems encountered & resolved

- **Agent-06 topology enumerates 55 "central" citations, but the evidence pool
  contains more.** Resolution: agent-07 tiered the full 85-citation union. Cluster
  G (9 refs: wav2vec2, WhisperX, HDBSCAN, BIRCH, HTDemucs, MediaPipe, YOLOv5-Face,
  Resemble Enhance, MCP spec) is new relative to agent-06's topology but is needed
  for §4.2 ClipCannon systems description. No conflict with agent-06 because
  Cluster G serves §4, not §2.
- **Cluster H drags the pool-wide percentage to 68%.** Resolution: agent-07 noted
  that Cluster H serves Q19 (Leapable sidebar, deprioritised by mission brief) and
  must be body-quota-limited to ≤ 6 citations. Agent-36 and agent-46 will enforce.
- **2 references ineligible per brief 03.** Resolution: agent-07 listed them
  separately with "required action" notes for agent-08.
- **Q06 RLHF/DPO/CAI triad absent from evidence pool.** Resolution: flagged as a
  known gap to agent-08 with recommendation to add (option a) rather than punt to
  agent-11 (option b). Expected tiers: Ouyang T1, Rafailov T1, Bai T2-or-T3.

## Next

Agent-08 (citation-extractor) starts from
`./memory/handoff/agent-07-to-08--tiers-locked.md`.

## Confidence

**HIGH.** Every citation in the pool is assigned a tier. Every tier has a rationale.
Every T3 has a justification. The 80% G8 gate passes on the projected in-body set
(84–87% depending on drop-candidate decisions). Known gaps and ineligible refs are
flagged. Downstream agents can begin work without waiting for clarifications.

---

## Change log

- 2026-04-16 — Initial issue by agent-07-source-tier-classifier.
