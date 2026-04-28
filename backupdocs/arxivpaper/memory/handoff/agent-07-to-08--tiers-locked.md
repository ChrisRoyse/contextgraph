---
namespace: handoff
agent_id: agent-07-source-tier-classifier
timestamp: 2026-04-16T17:15:00Z
status: active
tags: handoff, agent-08, citation-extractor, tiers-locked, qg1
---

# Handoff: Agent-07 (source-tier-classifier) → Agent-08 (citation-extractor)

## Read these files FIRST, in this order

1. `./memory/decisions/agent-07-source-tier-classifier--tier-assignments.md` — the
   **LOCKED** master tier table. 85 citations across 8 clusters (A–H).
2. `./memory/discoveries/agent-07-source-tier-classifier--tier-3-justifications.md` —
   per-T3-citation justification with KEEP/DROP decisions for body use.
3. `./memory/patterns/agent-07-source-tier-classifier--citation-percent-by-tier.md` —
   summary stats; demonstrates 84–85% T1+T2 in the curated in-body set.
4. `./memory/reports/agent-07-source-tier-classifier--final-report.md` — SMAP report.

## What agent-07 did

1. Ingested all four evidence briefs in full:
   - `_evidence_01_data_wall.md` (data wall, model collapse, compression)
   - `_evidence_02_voice_cloning.md` (zero-shot TTS, WavLM, DNSMOS, UTMOS)
   - `_evidence_03_video_embedders.md` (video embedders, flow matching, retrieval)
   - `_evidence_04_enterprise_market.md` (licensing deals, industry signals, domain LLMs)
2. Enumerated the **full union set of 85 citations** across the four briefs (not just
   agent-06's 55 central citations — because §4 Systems and §6 Case Studies reference
   the auxiliary pipeline components in Cluster G that agent-06 did not central-list).
3. Assigned each citation a tier (T1 / T2 / T3) per the task-brief tier definitions:
   - **T1** = top-tier peer-reviewed venue + DOI.
   - **T2** = reputable peer-reviewed venue OR arXiv-only from T1-publishing group.
   - **T3** = industry/news/blogs/GitHub/specs/books/gov-vocab references.
4. Organised citations into 8 clusters:
   - A Scaling laws + data wall (9 refs)
   - B Model collapse + synthetic data (7 refs)
   - C Compression-as-intelligence (5 refs)
   - D Multi-modal embedders + retrieval (10 refs)
   - E Zero-shot TTS + voice cloning (13 refs)
   - F Identity-locked video generation (12 refs)
   - G Auxiliary pipeline components (9 refs; new cluster, not in agent-06's topology)
   - H Enterprise-market signals (20 refs; the Q19 sidebar pool)
5. For every T3 in the core body set (A–G), wrote a specific justification:
   claim-it-supports + no-Tier-1-equivalent-reason + KEEP/DROP decision.
6. Flagged the 2 ineligible references (Qwen3-TTS, Demucs v1/v2/v3) from brief 03's
   "Unverified / Excluded" section — NOT eligible for citation unless re-verified.
7. Flagged 1 known gap (Q06 RLHF/DPO/CAI triad absent from evidence pool) — agent-11
   and agent-08 to address.

## Headline outcomes

- **Core (A–G) T1+T2 = 85%.** ✅ PASSES the ≥ 80% QG1 G8 gate.
- **Projected in-body (A–G + curated H) T1+T2 = 84%.** ✅ PASSES.
- **Full pool (A–H) T1+T2 = 68%.** ❌ FAILS on its own — this is why Cluster H must be
  curated to ≤ 6 refs per the master-table rule.
- **All 10 T3s in the core set have explicit "no Tier-1 equivalent" justification**
  in the sibling T3-justifications file.

## Quality gate status

- **QG1 (Evidence base complete) G8 (Empirical tier labelling).** Agent-07's
  contribution: **PASS** (subject to Cluster-H curation discipline downstream).
- No citation must be dropped or replaced on pure tier grounds (all are defensible).
- 2 citations are *ineligible* until re-verified (Qwen3-TTS, Demucs v1/v2/v3).

## What agent-08 (citation-extractor) must do

Per agent-05 tasking-table row 08:

1. Read the files listed above.
2. Produce the **master reference database** in APA 7th edition format, with every
   citation carrying full metadata: authors, year, title, venue, DOI/URL, pages (if
   applicable).
3. Deduplicate cross-brief appearances per the agent-07 dedup table:
   - Chen et al. WavLM (2022) → single entry (D03 in the master table)
   - Franzen TechCrunch (2024) → single entry (A08)
   - Woo & Efrati (2024) → single entry (A05)
4. Issue a **citation key** per entry that downstream agents will cite by (e.g.,
   `shumailov2024nature`, `hoffmann2022chinchilla`). These keys become the canonical
   in-text citation handles; writers cite by key, not by free-text URL.
5. Include agent-07's tier label in each entry's metadata (as a comment or a separate
   column), so agent-36 / 38 / 44 can audit tier-density per section.
6. **Decide on the three RLHF/DPO/CAI additions (Q06 gap).** Agent-07 did not
   pre-tier these because they're not in the evidence pool. Agent-08 has two options:
   (a) add them with full APA metadata and assign tiers (expected: Ouyang T1,
   Rafailov T1, Bai T2-or-T3), or (b) leave them unassigned and ping agent-11
   systematic-reviewer to source and tier. **Recommendation: option (a)** — agent-08
   is the single master-bibliography owner; adding three well-known papers is
   in-scope.
7. **Verify the 2 ineligible references.** If Qwen3-TTS has an arXiv identifier by
   the time agent-08 runs, add it; otherwise cite the GitHub model card as T3. For
   Demucs, verify arXiv:1911.13254 resolves; if yes, add as T2.
8. **Reconcile wav2vec 2.0 tier.** Agent-07 tiered G01 wav2vec 2.0 as T2 to match
   brief 03's arXiv-only URL, but the paper was published at NeurIPS 2020. If
   agent-08 confirms the NeurIPS 2020 venue, promote to T1 and update the per-tier
   percentages accordingly (a small increase to the T1 count; T1+T2 percentage
   unchanged).
9. **Reconcile Pile of Law tier.** Agent-07 tiered H05 as T1 based on the NeurIPS
   2022 Datasets & Benchmarks venue. Brief 04 lists only the arXiv URL. Agent-08
   should confirm the venue from the paper's first page / OpenReview record.
10. Write `./memory/decisions/agent-08-citation-extractor--master-bibliography.md`
    (or whatever the locked filename is in the agent-05 tasking table),
    `./memory/handoff/agent-08-to-09--...` etc.

## Critical constraints that carry forward

- **Tier changes require a blocker.** Downstream agents may not silently promote
  or demote any citation's tier. If agent-08 needs to change a tier (e.g., on
  venue re-verification per step 8), file a blocker noting the change and update
  `decisions/agent-07-source-tier-classifier--tier-assignments.md` with a changelog
  entry.
- **Cluster H is body-quota-limited.** ≤ 6 Cluster-H citations in body. Agent-08
  assigns keys for all 20 Cluster-H entries (so they're retrievable), but flags
  which 6 are body-eligible in the bibliography.
- **2 ineligible references (Qwen3, Demucs v1/v2/v3) do NOT enter the bibliography
  without re-verification.** Agent-08 can decide to wait, or cite the GitHub artefact
  as T3 for Qwen3.
- **RLHF/DPO/CAI triad is Q06's anchor.** If agent-08 doesn't add it, agent-11 must
  — otherwise agent-36 related-work-writer has no Tier-1 anchor for §2.4 alignment
  sub-line.

## Per-cluster summary (for agent-08 planning)

| Cluster | Core refs | Ineligible | Notes for agent-08 |
|---|---|---|---|
| A Scaling laws + data wall | 9 | 0 | A05/A06/A07 are industry-signal T3s; keep cluster together |
| B Model collapse + synthetic | 7 | 0 | All T1/T2 with clean venues |
| C Compression-as-intelligence | 5 | 0 | 3 Nature / Nature MI / ICLR / ACL T1s |
| D Multi-modal embedders + retrieval | 10 | 0 | Mix of T1 (Nature-tier journals + NeurIPS + NAACL) and T2 (SIGIR + TACL + arXiv-by-T1-groups) |
| E Zero-shot TTS + voice cloning | 13 | 0 | Most arXiv-only from T1-publishing groups → T2. ICASSP + Interspeech = T2 |
| F Identity-locked video generation | 12 | 0 | Mostly arXiv T2; F11 FLAME is ACM TOG T1; F12 FACS 1978 is T3 |
| G Auxiliary pipeline components | 9 | 2 (Qwen3, Demucs v1–3 if agent-08 adds them) | Wait: Qwen3-TTS & Demucs v1–3 are in brief 03 "Unverified / Excluded" — not central to the core pool. Agent-08 decides. |
| H Enterprise-market signals | 20 | 0 | Body-quota-limited to ≤ 6 |

## Confidence

**HIGH.** All 85 citations have an assigned tier; every tier assignment carries a
rationale; every T3 has a justification; the 80% G8 gate passes for both the core
and the projected in-body set; known gaps and ineligible refs are explicitly flagged
for agent-08 action. Agent-08 has clear, bounded work remaining (dedupe,
key-assignment, Q06 triad decision, venue re-verification for 2 refs).

---

## Change log

- 2026-04-16 — Initial handoff by agent-07-source-tier-classifier.
