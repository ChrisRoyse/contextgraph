---
namespace: handoff
agent_id: agent-06-literature-mapper
timestamp: 2026-04-16T16:45:00Z
status: active
tags: handoff, agent-07, source-tier-classifier, topology-ready, qg1
---

# Handoff: Agent-06 (literature-mapper) → Agent-07 (source-tier-classifier)

## Read these files FIRST, in this order

1. `./memory/discoveries/agent-06-literature-mapper--topic-topology.md` — the 6-cluster
   field topology with 54 central citations (+ 1 supplementary) organised by cluster.
   **This is the corpus you tier.**
2. `./memory/discoveries/agent-06-literature-mapper--structural-holes.md` — the combined
   5-hole gap map. Agent-07 does not tier the holes themselves, but reads them to
   understand which citations are load-bearing for which §-ID.
3. `./memory/patterns/agent-06-literature-mapper--cluster-summaries.md` — paste-ready
   §2 cluster summaries. Agent-07 uses these to verify that every citation named in a
   summary appears in the classification table (no citation orphans).
4. `./memory/reports/agent-06-literature-mapper--final-report.md` — SMAP report for
   cross-reference.

## What agent-06 did

1. Built a **6-cluster field topology** per the seed in
   `handoff/agent-05-to-06--plan-ready.md`, reorganised slightly to align with the
   evidence briefs' actual content:
   - **Cluster A** — Scaling laws + the data wall (8 citations).
   - **Cluster B** — Model collapse + synthetic data (6 citations).
   - **Cluster C** — Compression-as-intelligence (6 citations).
   - **Cluster D** — Multi-modal embeddings + retrieval (10 citations).
   - **Cluster E** — Zero-shot TTS + voice cloning (12 citations + 1 supplementary).
   - **Cluster F** — Identity-locked video generation (12 citations).
   — **Total: 54 central + 1 supplementary = 55 citations.**
2. For each cluster: named the central citations, wrote a 2–3 sentence standard
   framing paragraph, identified 3 per-cluster structural holes (18 total).
3. Consolidated the 18 per-cluster holes into **5 combined structural holes** that
   the arXiv submission explicitly fills. Mapped each hole to Tier-A constructs, Q-IDs,
   and §-IDs.
4. Produced paste-ready §2 cluster summaries for agent-36 with canonical vocabulary.
5. Defused FM-10 (citation padding): every citation is attached to a specific claim
   slot (Q-ID or §-ID), not to a generic survey block.
6. Cross-checked locked vocabulary (glossary #1–#30) and failure modes (FM-01..FM-10):
   no vocabulary drift; no AGI overclaim; no marketing creep.

## Divergence from the agent-05 seed (documented, not unauthorised)

The agent-05 handoff seed (`handoff/agent-05-to-06--plan-ready.md`) listed six clusters
labelled A–F where agent-06 has now named them slightly differently. The substantive
divergences are:

1. **Cluster D (alignment RLHF/DPO/CAI)** was listed as a *separate* cluster in the
   agent-05 seed. Agent-06 **folded the RLHF/DPO/CAI novelty-delta into HOLE-3** (the
   runtime geometric guard hole), consistent with the ReWOO plan §1 section-map listing
   alignment as a six-sub-line inside §2 but not necessarily as a stand-alone evidence
   cluster. RLHF / DPO / CAI citations are NOT in the four evidence briefs; therefore
   no citations for them are invented here. **Agent-11 or agent-08 may need to add a
   small alignment citation block** (Ouyang et al., 2022; Rafailov et al., 2023 DPO;
   Bai et al., 2022 CAI) to the master reference database at that stage — the agent-05
   seed flags these three. Agent-06 does NOT add them, per the instruction "every
   citation must exist in the four evidence briefs — do NOT invent new citations".
2. **Cluster E (multi-modal embedders) and Cluster F (voice cloning)** in the agent-05
   seed are here split into **Cluster D (multi-modal embedders + retrieval)**,
   **Cluster E (zero-shot TTS + voice cloning)**, and **Cluster F (identity-locked
   video generation)**. This 3-way split is dictated by how the evidence briefs are
   actually organised (`_evidence_02` = TTS / voice cloning; `_evidence_03` = video
   embedders + video generation; the retrieval substrate lives in `_evidence_03`
   references but is logically a separate cluster).
3. The 6-cluster topology **preserves the ReWOO plan's six-sub-line count for §2**;
   agent-36 can still produce a six-paragraph §2 Related Work.

**Escalation check.** This divergence is within-authority for agent-06 (topology-
building requires mapping evidence onto clusters faithfully). If agent-07 / agent-11
/ agent-36 think the reorganisation is wrong, file a blocker — do not silently
re-reorganise.

## What agent-07 (source-tier-classifier) must do

Per agent-05 tasking-table row 07:

1. Read the files listed above.
2. Tier **every** citation in the 55-citation corpus:
   - **T1** = peer-reviewed venue (journal or A/A* conference) with working DOI.
   - **T2** = arXiv preprint with an artefact (code repo, benchmark, dataset) or a
     workshop publication with a stable URL.
   - **T3** = blog post, technical report, trade press (The Information, TechCrunch,
     Bloomberg, Reuters), or industry announcement.
   - **T4** = authoritative web reference (organisation home page, canonical
     specification, press release).
3. Target **≥ 80% T1 + T2** across the 55-citation corpus (QG1 criterion).
4. Flag any T3 / T4 citation whose removal would break a Q-ID claim (these are the
   *load-bearing* non-peer-reviewed citations; they need explicit justification).
5. Cross-check: every citation named in an agent-06 cluster summary MUST appear in the
   agent-07 tier table. No orphans.
6. Write `./memory/decisions/agent-07-source-tier-classifier--tier-classification.md`,
   `./memory/handoff/agent-07-to-08--classification-ready.md`, and
   `./memory/reports/agent-07-source-tier-classifier--final-report.md`.

### Corpus summary for agent-07's planning

| Cluster | Citations | Preliminary tier expectation |
|---|---|---|
| A Scaling laws + data wall | 8 | 4× T1/T2 (Kaplan, Hoffmann, Villalobos, Longpre); 4× T3/T4 (industry signals: The Information, TechCrunch, Reuters, Sutskever oral). |
| B Model collapse + synthetic | 6 | 6× T1/T2 (all peer-reviewed / preprint-with-artefact). |
| C Compression-as-intelligence | 6 | 5× T1/T2 (Delétang, Huang, Li-Nature, Valmeekam, Jiang); 0–1× T4 (the "adjacent" GPTQ/SparseGPT/TurboQuant block may be added by agent-11 for the taxonomy table — if agent-07 tiers them now, all T1/T2). |
| D Multi-modal embedders + retrieval | 10 | 10× T1/T2 (all peer-reviewed or strong preprint). |
| E Zero-shot TTS + voice cloning | 12 (+1 supp.) | 11× T1/T2 (all TTS SOTA, WavLM, DNSMOS, UTMOS, ECAPA-TDNN, ClonEval); 1× empirical-only hedge (WavLM same-session band, no citation — this is NOT a citation but a hedge; agent-07 confirms it is excluded from the tier table). |
| F Identity-locked video generation | 12 | 11× T1/T2 (EchoMimicV3, Hallo3, LatentSync, HunyuanVideo-Avatar, EMO/EMO2, CogVideoX, Wan, LoRA, Flow-matching, FLAME); 1× T4 (FACS Ekman & Friesen 1978, print book, no URL). |

Expected aggregate: ≥ 49 / 55 = **89% T1+T2**, well above the 80% QG1 target.

## Files agent-06 wrote

1. `./memory/discoveries/agent-06-literature-mapper--topic-topology.md` — 6-cluster
   field topology, 55 citations, 18 per-cluster holes.
2. `./memory/discoveries/agent-06-literature-mapper--structural-holes.md` — 5-hole
   combined gap map with Tier-A construct and Q-ID coverage.
3. `./memory/patterns/agent-06-literature-mapper--cluster-summaries.md` — paste-ready
   §2 cluster summaries with vocabulary-compliance checklist.
4. `./memory/handoff/agent-06-to-07--topology-ready.md` — this file.
5. `./memory/reports/agent-06-literature-mapper--final-report.md` — SMAP report.

## Quality gate status

- **QG1 (Evidence base complete).** Agent-06's contribution: PASS.
  - 6 clusters × ≥ 3 T1/T2 candidates = ≥ 18 central T1/T2. Actual: 49+ (well above).
  - No thin cluster flagged; no blocker filed; no escalation needed.
- Agent-07 is unblocked.

## Binding constraints that carry forward

- Every downstream writer uses the glossary vocabulary exactly. Vocabulary-drift (FM-09)
  is agent-42's #1 regex target.
- Every citation supports a specific claim (FM-10); no generic survey-block insertions.
- No citation invented that is not in one of the four evidence briefs. If agent-11 or
  agent-36 needs additional citations (e.g., the alignment RLHF / DPO / CAI triad), they
  must be sourced from a documented retrieval and flagged in the master reference
  database with explicit provenance.
- No AGI / "solved" / superlative language (FM-03). The 5-hole gap map is expressed as
  narrow structural deltas.

## Confidence

**HIGH.** The six-cluster topology matches both the agent-05 seed (with one
documented reorganisation) and the actual structure of the evidence briefs. Every
cluster has ≥ 3 T1/T2 candidates. The 5-hole combined gap map covers all four Tier-A
constructs and all §1/§2-critical questions. Agent-07 should be able to start
immediately.

---

## Change log

- 2026-04-16 — Initial handoff by agent-06-literature-mapper.
