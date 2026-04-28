---
namespace: handoff
agent_id: agent-08-citation-extractor
timestamp: 2026-04-16T17:55:00Z
status: active
tags: handoff, agent-09, methodology-scanner, citation-pool-ready
---

# Handoff: Agent-08 (citation-extractor) → Agent-09 (methodology-scanner)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-08-citation-extractor--master-reference-pool.md` — the
   **LOCKED** master bibliography. 88 entries across Clusters A–I with complete APA 7th
   metadata, DOIs/arXiv IDs, and citation keys.
2. `./memory/patterns/agent-08-citation-extractor--claim-to-citation-index.md` —
   Q01–Q20 → primary/supporting citation mapping. Body-pool target counts per Q.
3. `./memory/discoveries/agent-08-citation-extractor--unresolved-citations.md` —
   5 unverified + 4 tier-deferred + 6 metadata-watch + 24 URL-stability-risk items.
4. `./memory/reports/agent-08-citation-extractor--final-report.md` — SMAP report.

## What agent-08 did

1. **Consolidated all citations** from the four evidence briefs and the prior
   `docs/researchpaper/13_references.md` into a single LOCKED master pool.
2. **Deduplicated cross-brief appearances** per the agent-07 dedup table (WavLM, Franzen,
   Woo & Efrati).
3. **Added the RLHF/DPO/CAI triad** flagged by agent-07 (Q06 gap) as Cluster I — 3 new
   entries: ouyang2022instructgpt (T1), rafailov2023dpo (T1), bai2022constitutional (T2).
4. **Venue-confirmed two flagged refs** per agent-07 handoff:
   - G01 wav2vec 2.0 — NeurIPS 2020 confirmed, **promoted T2 → T1**.
   - H05 Pile of Law — NeurIPS 2022 D&B confirmed, tier held at T1.
5. **Assigned citation keys** to every entry (e.g., `shumailov2024nature`,
   `rafailov2023dpo`). Writing agents cite by key.
6. **Built the Q-to-citation claim index** with primary/supporting split per Q01–Q20 and
   per-Q body-pool target counts.
7. **Flagged 5 unverified refs** (Qwen3-TTS, Demucs v1–v3, same-session band, TurboQuant
   specifics, enterprise pricing) with resolution paths.
8. **Recorded 4 tier-deferred citations** that could be promoted T2 → T1 when venue is
   re-verified by agent-45.

## Headline outcomes

- **Total entries in the master pool: 88** (65 core A–G + 3 new I + 20 H-cluster).
- **Core (A–G + I) T1+T2 = 85%** (unchanged since agent-07; new I entries maintain the
  ratio). ✅ PASSES the 80% QG1 G8 gate by 5 points.
- **Every Q01–Q20 has ≥ 3 body-pool citations planned** and ≥ 1 T1 or T2 primary.
- **Zero citations lack a DOI / arXiv ID / URL.**
- **Unique body-pool keys planned for body-wide citation: ~52**, inside the agent-05
  "30–60 references" target.

## Quality gate status

- **QG1 (Evidence base complete) G7 (Citation validity).** Agent-08's contribution:
  **PASS pending agent-45 URL re-verification at camera-ready.**
- **QG1 (Evidence base complete) G8 (Empirical tier labelling).** Agent-08's
  contribution: **PASS** at 85% T1+T2 in the Core + I set.
- **Q06 gap (RLHF/DPO/CAI) closed** via Cluster-I additions.

## What agent-09 (methodology-scanner) should do next

Per agent-05 tasking-table row 09:

1. Read the master pool + claim-index + unresolved files (listed above).
2. Scan the master pool for **methodology-revealing citations** — papers whose
   methods are borrowed/adapted in our §3–§5. Specifically:
   - Flow-matching loss mechanics → [F10] lipman2023flowmatching, [F01] meng2025echomimicv3.
   - LoRA low-rank adaptation → [F09] hu2022lora.
   - DPO objective → [I02] rafailov2023dpo.
   - RLHF preference modelling → [I01] ouyang2022instructgpt.
   - Late-interaction retrieval → [D04] khattab2020colbert + [D05] santhanam2022colbertv2.
   - Reciprocal-rank fusion → [D10] cormack2009rrf.
   - HNSW ANN search → [D09] malkov2018hnsw.
   - SECS speaker encoder → [D03] chen2022wavlm + [E12] desplanques2020ecapa.
   - Non-intrusive speech-quality scoring → [E10] reddy2022dnsmos + [E11] saeki2022utmos.
3. Produce a methodology dependency map that downstream writers (agents 37–38) consume
   when drafting §5 Method. Each method claim should map to a citation key from this
   pool.
4. **Do NOT add new citations.** If agent-09 believes a citation is missing, file a
   blocker; do not silently extend the pool.

## What agent-11 (systematic-reviewer) should do later

- Review the unresolved-citations file and triage items.
- If a PRISMA expansion is warranted on a specific cluster, file a blocker first.
  Current cluster density per agent-07: all clusters pass the ≥ 3 Tier-1/2 criterion.
- If any tier promotion is confirmed (four tier-deferred candidates), update the
  master-pool changelog and notify agent-44/45.

## What agents 35–41 (writers) must do

- **Cite by key, not by URL or free-text.** Every in-text citation resolves to a key in
  the master pool.
- **Respect the Cluster-H body quota ≤ 6.** Prefer H05, H13, H15, H20 (T1/T2).
- **Respect the per-Q body-pool targets** in the claim-to-citation index.
- **Disclose forensic files** (agent-01 P10) in-text with file paths; those are NOT
  citations, they are forensic disclosures.
- **If a citation is needed and not in the pool**, file a blocker; do not invent.

## Critical constraints carrying forward

- **Tier changes require a blocker** and a changelog entry in the master pool.
- **No silent additions.** The master pool is LOCKED.
- **Qwen3-TTS citation is still pending** — see unresolved-citations.md U01. §6.3
  Methods will need a citation for the Case 3 base model; proposed T3 key
  `qwen2024qwen3tts` must be entered in the pool before PAPER.md is finalised.

## Confidence

**HIGH.** The evidence pool is consolidated, tier-labelled, key-indexed, Q-mapped, and
handoff-ready. One must-resolve blocker (U01 Qwen3-TTS) is flagged with a clear
remediation path; everything else is either resolved, mitigated by policy, or
annotated for agent-45's camera-ready re-verification pass.

---

## Changelog

- 2026-04-16 — Initial handoff by agent-08-citation-extractor.
