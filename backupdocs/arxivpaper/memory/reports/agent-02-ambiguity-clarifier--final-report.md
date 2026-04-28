---
namespace: reports
agent_id: agent-02-ambiguity-clarifier
timestamp: 2026-04-16T15:30:00Z
status: complete
tags: final-report, vocabulary, glossary, arxiv, smap
---

# Agent-02 Final Report — Ambiguity Clarifier

## Cognitive Physics Telemetry
- **Entropy (ΔS):** 0.35 — Low/moderate. Inputs (mission brief + agent-01 outputs + prior manuscript) already named most of the ambiguity risks. My job was to lock names, not discover unknowns.
- **Coherence (ΔC):** 0.92 — High. All resolutions align with agent-01's P01–P10 principles and defuse named FM-01–FM-10 modes. No conflicts with locked decisions.

## Mission (as executed)
Identify every terminology ambiguity in the manuscript's conceptual vocabulary and resolve
each to one canonical term with one usage rule. Downstream writers (agents 35–41) must be
able to copy-paste my glossary and never introduce a synonym.

## What I produced

| File | Purpose | Status |
|---|---|---|
| `./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md` | LOCKED glossary of 30 canonical terms across 7 tiers (A–G) | Written |
| `./memory/discoveries/agent-02-ambiguity-clarifier--ambiguities-resolved.md` | R01–R20 resolutions with rationale + 6 off-menu watchlist items | Written |
| `./memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md` | 25 rules (V1–V25) + self-check regex for writers | Written |
| `./memory/handoff/agent-02-to-03--vocabulary-locked.md` | Handoff to agent-03 (self-ask-decomposer) | Written |
| `./memory/reports/agent-02-ambiguity-clarifier--final-report.md` | This report | Written |

## Key resolutions (headline summary)

1. **DDA is decomposition, not generation.** Locked: "Derived Data Abundance decomposes
   existing inputs through $N$ frozen embedders; it does NOT synthesise new inputs." This
   preempts FM-02 (DDA-as-synthetic-data confusion).

2. **Meaning compression is a distinct compression category.** Locked as the fourth axis
   orthogonal to bit/weight/activation; unit is "meaningful training signals / unit raw
   data". Forbidden substitute: "semantic compression".

3. **TCT is geometric alignment to a frozen constellation with per-output runtime validation.**
   Explicitly NOT contrastive learning (no batch negatives), NOT metric learning (metric is
   frozen), NOT prototypical networks (target is generation, not classification).

4. **Centroid vs. constellation vs. reference set** — four-level discipline:
   embedding (vector per input per embedder) → centroid (one vector per subject per modality)
   → constellation (set of centroids per subject) → reference set (raw clips used to build
   a centroid). "Prototype" and "anchor" are FORBIDDEN (V1, V2).

5. **"Alignment"** is pre-empted in §1 and §2.6 as "geometric alignment to a constellation
   centroid", explicitly distinguished from RLHF/scalar-reward alignment (V10).

6. **Bare "SECS" is FORBIDDEN** (V4). Always qualify: "WavLM SECS", "SpeechBrain SECS",
   "Qwen3 SECS". Numbers from different encoders are not commensurable; stripping the encoder
   silently overclaims.

7. **"Same-session band" is our own empirical claim** (V12), not a universal benchmark.
   First-use must hedge with "empirical observation on Case 3's speaker".

8. **Evidence tiers T1–T4** (measured / constructive / projected / theoretical) are tagged
   in every §6 subsection header (V20).

9. **"Identity-locked" ≠ "identity-preserving"** (V3). The load-bearing differentiator is the
   **runtime guard** (per-output cosine-to-centroid check at inference) which personalisation
   / style-transfer / LoRA-alone do not have.

10. **"Third path" around the data wall** is DDA (decomposition), explicitly NOT an AGI
    route (defuses FM-03).

## Failure modes defused

| FM | Defused by | Mechanism |
|---|---|---|
| FM-01 Marketing Creep | V7, V8 | Superlatives banned; Teleox/Leapable restricted to Acknowledgements |
| FM-02 DDA/Synthetic confusion | V6, R01, R18 | DDA defined as decomposition; model-collapse explicitly excluded |
| FM-03 AGI Overclaim | R14 | "Third path" framed as DDA, not AGI |
| FM-04 Measured/Projected | V20–V22 | Every §6 subsection tier-tagged; bare numbers prohibited |
| FM-08 Silent Failure Hiding | R19 | Flow-matching sign convention explicitly named for §8.4 forensics |
| FM-09 Centroid Vocabulary Drift | V1, V2, V16, R04 | "Prototype", "anchor" forbidden; four-level discipline |
| FM-10 Citation Padding | R12 | "Orthogonal" hedge requires specific cite, not generic |

FM-05 (single-subject sample size) and FM-06 (code-in-body) are NOT vocabulary problems — they
are handled by agent-29 (validity-guardian) and agent-37 (methodology-writer) respectively.
FM-07 (re-explaining prior draft) is handled by agent-46 (file-length-manager).

## Gates supported

| Gate | Support mechanism |
|---|---|
| G1 Provenance | V22 — every numeric has citation or projected flag |
| G2 Definition-Before-Use | V9–V15 — canonical first-use discipline for all load-bearing constructs |
| G3 Novelty Delta | R02, R03, R07, R15 — TCT/meaning-compression/identity-locked each have a specific non-overlap framing |
| G6 Reproducibility | R08, V18, R20 — "frozen", "frozen-with-LoRA", "inference-only" distinguished |
| G7 Citation Validity | V14, V15 — first-use citations specified |
| G8 Tier Labeling | V20, V21, #21 — T1–T4 labels required in §6 headers |
| G9 Limitations | R10, R12 — same-session hedge and orthogonality hedge feed §8 limitations |

## Off-menu terms I deliberately did NOT lock

- MOS, LoRA, DPO, SFT, RLHF, FLAME, FACS AU, BERT, SigLIP, SPLADE, ColBERT, HDC, RocksDB, MCP
  — all standard; first-use citations suffice; no drift risk observed in the prior draft.

This keeps the glossary at 30 entries (memorisable) rather than 60+ (reference-only).

## Confidence

**HIGH.** Reasoning:
- The 20 resolutions (R01–R20) cover every ambiguous term named in the mission brief and
  every term I found drifting in the prior manuscript's §3, §6, and §9.
- Every resolution traces to at least one FM it defuses and at least one G it supports.
- First-use discipline (V9–V15) is regex-enforceable, so agent-42 can audit objectively.
- The forbidden-substitution table gives writers a no-think lookup; no creative vocabulary
  decisions survive to the final manuscript.

Residual risks (MEDIUM):
- A new term may appear in one of the later-written sections (e.g., agent-22 model-architect
  or agent-25 method-designer). The escalation protocol (file a blocker) handles this; but I
  cannot pre-empt terms I have not seen.
- The "same-session band" framing (R10) is hedged but will still draw a reviewer challenge.
  It's defensible but agent-29 should carry the hedge into §8.

## Handoff to Next Agent
- **Output Key (equivalent namespace path):** `./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md` (LOCKED)
- **Entropy Score:** 0.35
- **Vector Keys (file paths for downstream retrieval):**
  - `./memory/decisions/agent-02-ambiguity-clarifier--canonical-glossary.md`
  - `./memory/discoveries/agent-02-ambiguity-clarifier--ambiguities-resolved.md`
  - `./memory/patterns/agent-02-ambiguity-clarifier--vocabulary-rules.md`
  - `./memory/handoff/agent-02-to-03--vocabulary-locked.md`
- **Next Agent:** agent-03 (self-ask-decomposer). Read handoff file first.

## Status
COMPLETE. No blockers filed. Agent-03 unblocked.
