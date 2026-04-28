---
namespace: reports
agent_id: agent-08-citation-extractor
timestamp: 2026-04-16T17:58:00Z
status: active
tags: smap, final-report, citation-extractor
---

# Final Report — Agent-08 Citation Extractor (SMAP)

## S — Situation

- **Role.** Agent-08 of 48. Produce the authoritative citation pool for the arXiv
  manuscript on **Teleological Constellation Training**.
- **Inputs consumed.**
  - `./memory/context/mission-brief.md`
  - `./memory/patterns/agent-05-research-planner--quality-gates.md` (G7 citation validity)
  - `./memory/discoveries/agent-06-literature-mapper--topic-topology.md` (6 clusters A–F)
  - `./memory/decisions/agent-07-source-tier-classifier--tier-assignments.md` (85 citations, LOCKED)
  - `./memory/handoff/agent-07-to-08--tiers-locked.md` (action list for agent-08)
  - `./memory/decisions/agent-03-self-ask-decomposer--essential-questions.md` (Q01–Q20)
  - Four evidence briefs: `docs/researchpaper/_evidence_01_data_wall.md` through `_evidence_04_enterprise_market.md`
  - Prior references file: `docs/researchpaper/13_references.md`
- **Constraints.**
  - APA 7th edition strictly.
  - Every entry must carry DOI / arXiv ID / working URL.
  - No new citations invented; additions must be explicitly justified (Q06 gap only).
  - No silent tier changes — all promotions require a changelog entry.
  - 2 unverified refs (Qwen3-TTS, Demucs v1–v3) must not enter the pool without re-verification.

## M — Method

1. **Read** all six input files plus the four evidence briefs plus the prior references
   file.
2. **Enumerate the full union** of citations across briefs + prior refs. Checked for
   duplicates using the agent-07 dedup table.
3. **Assigned citation keys** (e.g., `shumailov2024nature`) to every entry. Keys are
   `lastname` + `year` + `short-slug`.
4. **Filled in missing metadata** per APA 7th: full author lists (up to 20, then et al.),
   DOI or arXiv ID when available, venue, page ranges where applicable.
5. **Re-verified two flagged venues** per agent-07 handoff steps 8–9:
   - G01 wav2vec 2.0 — NeurIPS 2020 confirmed → **promoted T2 → T1**.
   - H05 Pile of Law — NeurIPS 2022 D&B confirmed → tier held at T1.
6. **Closed the Q06 gap** by adding Cluster I (RLHF / DPO / CAI):
   - I01 ouyang2022instructgpt (NeurIPS 2022) — T1
   - I02 rafailov2023dpo (NeurIPS 2023) — T1
   - I03 bai2022constitutional (arXiv, Anthropic tech report, T1-publishing group) — T2
7. **Built the Q-to-citation claim index** mapping each of Q01–Q20 to primary/supporting
   citation keys with body-pool target counts.
8. **Catalogued unresolved items** (unverified, tier-deferred, metadata-watch,
   URL-stability-risk) for agent-45's camera-ready pass.
9. **Wrote five files** to memory per the spec (master pool, claim index, unresolved,
   handoff, this report).

## A — Actions / Artefacts

Five files produced:

1. `./memory/decisions/agent-08-citation-extractor--master-reference-pool.md` — **LOCKED**
   master bibliography. 88 entries; every entry carries cluster label (A–I), tier,
   citation key, APA 7th metadata, DOI / arXiv ID, working URL, and the Q-IDs it is
   load-bearing for.
2. `./memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md` — Q01–Q20
   → primary/supporting citation map. Per-Q body-pool target counts. Thinnest-support
   watch list (Q08 and Q17).
3. `./memory/discoveries/agent-08-citation-extractor--unresolved-citations.md` —
   5 unverified refs, 4 tier-deferred candidates, 6 metadata-watch items, 24
   URL-stability-risk items. All categorised and scheduled for agent-45.
4. `./memory/handoff/agent-08-to-09--citation-pool-ready.md` — handoff to
   methodology-scanner. Points at the master pool; names the methodology-revealing
   citations.
5. `./memory/reports/agent-08-citation-extractor--final-report.md` — this report.

### Headline numbers

- **Total citations in the pool: 88** (65 core A–G + 3 new I + 20 H-cluster).
- **Tier distribution:** T1 = 25, T2 = 37, T3 = 26.
- **Core (A–G + I) T1+T2 = 85%** — passes QG1 G8 by 5 points.
- **DOI / arXiv / URL coverage: 100%** (every entry has at least one resolver).
- **Q01–Q20 coverage: 100%** (every question has ≥ 3 planned body-pool citations and at
  least one T1 or T2 primary).
- **Unique body-pool keys planned: 52** (inside the agent-05 "30–60 references" target).

### Resolutions

- **Dedupe:** 3 cross-brief duplicates collapsed (chen2022wavlm, franzen2024techcrunch,
  woo2024information).
- **Venue confirmations:** 2 (G01 wav2vec 2.0 promoted to T1; H05 Pile of Law held at T1).
- **Q06 gap closed:** 3 RLHF/DPO/CAI citations added as Cluster I.
- **Unverified refs excluded from pool:** 2 (Qwen3-TTS proposed as T3 key pending
  re-verification at agent-45; Demucs v1–v3 optional, not needed because [G05]
  rouard2023htdemucs covers v4/HTDemucs).

### What is NOT in the pool (by design)

- Qwen3-TTS (base model for Case 3) — resolution path: T3 GitHub model-card citation at
  proposed key `qwen2024qwen3tts`. Must be entered before PAPER.md is finalised.
- Demucs v1–v3 — optional.
- Weight-compression (GPTQ, SparseGPT) specifics — descriptive only in §3.5; agent-11
  may add if §3.5 needs citation density.
- Activation-compression (TurboQuant specifics) — "6× KV-cache" number cannot be
  sourced; §3.5 stays with generic KV-cache quantisation framing.
- Pricing figures for bank/insurer custom models — no citable source; omitted.

## P — Position & recommendations

### Confidence: **HIGH**

Every action item in the agent-07 handoff is addressed:

| agent-07 handoff step | Resolution |
|---|---|
| 1. Read input files | Done. |
| 2. Produce master reference database in APA 7th | Done (88 entries). |
| 3. Deduplicate cross-brief appearances | Done (3 dedupes). |
| 4. Issue citation key per entry | Done. |
| 5. Include tier label per entry | Done. |
| 6. Decide on RLHF/DPO/CAI triad | Option (a) taken; 3 entries added as Cluster I. |
| 7. Verify Qwen3-TTS / Demucs v1–v3 | Qwen3: T3 GitHub model-card disclosure path documented (U01); Demucs: optional (U02). |
| 8. Reconcile wav2vec 2.0 venue | Done — promoted T2 → T1. |
| 9. Reconcile Pile of Law venue | Done — T1 held. |
| 10. Write files to memory | Done (5 files). |

### Thinnest citation support (3-sentence summary for the coordinator)

**The pool has 88 citations, 85% T1+T2 in the Core (A–G + I), and 100% DOI/URL
coverage.** Every Q01–Q20 has ≥ 3 planned body-pool citations and at least one T1 or
T2 primary; the only remaining **must-resolve blocker** is Qwen3-TTS (Case 3 base
model; proposed T3 GitHub-model-card citation path is documented and scheduled for
camera-ready entry). **The two thinnest-support questions are Q08 (threshold
calibration — only 1 external T1 primary, chen2022wavlm) and Q17 (forensics — bugs are
unique to our pipelines, so external citations are minimal by design); both are
accepted.**

### Risks / watch-list for agent-45

1. **Qwen3-TTS U01** — blocks §6.3 Methods until entered. Medium-effort fix at
   camera-ready.
2. **Four tier-deferred promotions** (zhai2023siglip, le2023voicebox,
   shen2023naturalspeech2, tian2024emo) — promoting any of them to T1 would move the
   Core + I percentage from 85% to ~88–90%. Low-effort.
3. **Paywall / URL-rot on 24 web-source entries** — agent-45 must HEAD-check every URL
   at T-minus-2-days; Internet Archive fallbacks allowed under G7's ≤ 5% transient
   tolerance.

### Observations for downstream agents

- The Cluster-I alignment citations (ouyang2022instructgpt, rafailov2023dpo,
  bai2022constitutional) are **mandatory for §2.6 Related Work** and §5 TCT — without
  them, Q06 has no T1 anchor. Writers must cite all three even if §2.6 is kept tight.
- The **claim-to-citation index** gives writers (agents 35–41) a turnkey reference for
  what to cite in each section. They should consult it before drafting; it saves a
  round of citation rework at QG7.
- agent-09 methodology-scanner's next move: read the master pool and build the
  methodology dependency map (flow-matching, LoRA, DPO, RLHF, HNSW, RRF, SECS,
  DNSMOS/UTMOS). Do not extend the pool.

## Self-critique

- **Strength 1.** The pool strictly respects the LOCKED tier table from agent-07; no
  silent promotions. The one recorded promotion (G01 wav2vec 2.0) carries a changelog
  entry.
- **Strength 2.** The Q-to-citation index is granular (primary/supporting split,
  per-Q body-pool target counts), which removes ambiguity for writers.
- **Strength 3.** Unresolved citations are categorised (unverified / tier-deferred /
  metadata-watch / URL-stability-risk) so agent-45 can triage efficiently.
- **Watch 1.** The bai2022constitutional tier (T2) is a close call. If body T1+T2
  density ever drops below 82% at agent-42 review, demote to T3 and revise Q06 primary
  set to `[I01, I02, S: I03]`.
- **Watch 2.** Four tier-deferred candidates could all be promoted; if agent-44 or
  agent-45 runs the promotion pass, the master pool's changelog must be updated and
  the handoff to agent-46 (length-auditor) amended.

## Confidence: **HIGH**

Inputs fully consumed, outputs complete, every agent-07 handoff item addressed, one
explicit blocker flagged with a documented resolution path, every Q01–Q20 has ≥ 3
body-pool citations.

---

## Changelog

- 2026-04-16 — Initial issue by agent-08-citation-extractor.
