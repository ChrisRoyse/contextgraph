---
namespace: handoff
agent_id: agent-12-track-briefs
timestamp: 2026-04-19T03:25:00Z
status: active
tags: handoff, track-briefs, tracks-abc, ship-ready
---

# Handoff — Track Briefs A / B / C to Orchestrator

## What was produced

Three ship-ready, standalone briefs corresponding to the three parallel tracks in `research/00_target_universe/TARGET_UNIVERSE_RANKING.md §2`. Each brief is a self-contained document Chris can email as PDF or paste as markdown. All three carry full scope guards per `memory/context/constraints.md` and pass the `memory/blockers/agent-00-orchestrator--pre-send-checklist.md` criteria.

| Track | File | Length | Audience |
|---|---|---|---|
| A | `research/08_track_briefs/TRACK_A_MGX_sovereign_brief.md` | ~1,150 words (≈3 pages printed) | Ahmed Yahia Al Idrissi (CEO MGX) + adjacent Mubadala principals |
| B | `research/08_track_briefs/TRACK_B_Anthropic_technical_path.md` | ~1,000 words | Ethan Perez (direct) / Jared Kaplan (via Perez) / Chris Olah (via X) |
| C | `research/08_track_briefs/TRACK_C_Qatalyst_one_pager.md` | ~650 words (≈1 page printed) | Frank Quattrone + team, Qatalyst Partners |

## Per-brief frame and ask

**Track A (MGX — sovereign)**
- Frame: "Sovereign AI Signal Multiplication: the 91-Structured-Signal Constructive Upper Bound Applied to Licensed National Corpora"
- Ask: 30-min exploratory call → conditional 48-hour on-prem PoC on MGX-selected sample corpus
- Key levers: data wall (Villalobos 2022/2024), licensing cost (News Corp $250M, Reddit $60M/yr, Anthropic $1.5B settlement), Shumailov 2024 recursion collapse, air-gapped deployment posture, MGX's published "sovereignty of training signal" framing

**Track B (Anthropic — research-first)**
- Frame: "Per-Output Geometric Verifiability as a Constitutional-AI-Adjacent Alignment Primitive"
- Ask (rank-ordered): MATS Summer 2026 mentor match with Perez → paper review from Kaplan → Olah blog-comment signal
- Key levers: Kaplan 2020 scaling laws (data-wall native reader); Bai et al. 2022 Constitutional AI as nearest alignment-adjacent primitive; Case 1 Shakespeare LoRA as toy adjacency to sleeper-agent robustness under adversarial prompt
- Explicitly research-track, no commercial engagement requested

**Track C (Qatalyst — banker)**
- Frame: "Category-Defining AI Substrate: Meaning-Compression-as-Fourth-Taxonomic-Entry"
- Ask: 20-min exploratory call re: sell-side mandate fit
- Key levers: Figma–Adobe $20B structural precedent (Qatalyst-advised); Scale $14.3B / Character $2.7B / Inflection $650M / Anthropic $1.5B settlement valuation frame; Case 1 + Case 3 with full scope disclosure; 20VC Jan 2026 founder-direct posture

## Ready-state verification (against pre-send checklist)

All three briefs confirmed to contain:
- [x] Measured anchor (Case 1 SFT 1.12 / DPO 0.47 + Case 3 0.961 WavLM SECS)
- [x] Case 3 scope tags: within-WavLM-family, N=10, one speaker, English, ECAPA-TDNN (EXP-1) pending
- [x] LoRA HF link: huggingface.co/cabdru/shakespeare-lora-gemma4
- [x] Competing Interests disclosure footer
- [x] Paper section citations with § notation (§0.2, §2.5, §3.1, §3.2, §3.3, §3.4, §5.4, §6, §8, §9)
- [x] "Up to 91 signals" with constructive-upper-bound + EXP-2 pending hedge
- [x] "Structurally outside Shumailov regime (scope, not refutation)" framing
- [x] No commercial TAM claims, no "state of the art" / "beats RLHF" / "refutes collapse" / "cross-encoder verified"

## Next-step dependencies for orchestrator

1. **Draft-to-send cadence is still governed by §4 of TARGET_UNIVERSE_RANKING.md** — these three briefs are the artefacts needed for Days 6, 5, and 14 of the Week-1 plan. Day 0 remains Dohmatob; Day 2 remains MATS application (Track B brief is NOT yet the application text — it's the preceding research-interest signal).
2. **Track B usage pattern:** send as a paper-review-request email to Perez, **after** MATS Summer 2026 application is in. Do not use as the MATS application itself — the application goes through the public form.
3. **Track A usage pattern:** hold until at least one collapse-cluster co-sign (Dohmatob / Feng / Kempe) is in hand. MGX materially weights researcher endorsement.
4. **Track C usage pattern:** send Day 10 per the Week-1 plan — after collapse-cluster has had 10 days to respond. Quattrone cold is a parallel, not an alternative, to researcher-endorsement path.

## Files written (absolute paths)

- `/home/cabdru/contextgraph/docs/sprinkleengage/research/08_track_briefs/TRACK_A_MGX_sovereign_brief.md`
- `/home/cabdru/contextgraph/docs/sprinkleengage/research/08_track_briefs/TRACK_B_Anthropic_technical_path.md`
- `/home/cabdru/contextgraph/docs/sprinkleengage/research/08_track_briefs/TRACK_C_Qatalyst_one_pager.md`
- `/home/cabdru/contextgraph/docs/sprinkleengage/memory/handoff/agent-12-to-orchestrator--track-briefs.md` (this file)
- `/home/cabdru/contextgraph/docs/sprinkleengage/memory/reports/agent-12-track-briefs--final-report.md`
