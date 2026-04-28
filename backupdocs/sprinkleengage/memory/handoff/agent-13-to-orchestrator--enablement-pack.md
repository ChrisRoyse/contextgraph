---
namespace: handoff
agent_id: agent-13-conversation-enablement
to_agent: agent-00-orchestrator
timestamp: 2026-04-19T09:00:00Z
status: active
tags: handoff, enablement-pack, cheat-sheet, pitches, discovery, objections
---

# Handoff: Agent 13 → Orchestrator — Live-Conversation Enablement Pack

## Deliverables shipped (all four)

| # | Document | Path | Notes |
|---|---|---|---|
| 1 | Cheat sheet | `research/04_sales_enablement/14_cheat_sheet.md` | 1,127 words; fits ≤4 printed pages; phone-readable |
| 2 | Pitches | `research/03_conversation_engineering/12_pitches.md` | 10s × 3 variants; 30s × 2; 2-min with pause-points; 5 audience variants |
| 3 | Discovery questions | `research/03_conversation_engineering/13_discovery_questions.md` | 18 strategic questions across 5 audiences + 3 cross-audience; listen-fors + follow-ups on every one |
| 4 | Objection handling | `research/04_sales_enablement/17_objection_handling.md` | All 12 pre-bank objections; Acknowledge → Reframe → Evidence → Advance + alternate for every one |

## Word-count target met
- Cheat sheet: 1,127 words (target ≤1,200). Fits 4 printed pages at phone-readable type size.

## Scope-guard compliance (applied verbatim across all four docs)
- ✅ Case 3 always carries: within-WavLM-family + N=10 + one speaker + English + EXP-1 pending
- ✅ "Up to 91" always carries: "constructive upper bound" + "EXP-2 pending"
- ✅ Collapse framing always: "structurally outside Shumailov regime" / "scope, not refutation"
- ✅ TCT vs. RLHF/DPO always: "different property class, not a benchmark contest"
- ✅ Case 2 always tagged PROJECTED
- ✅ CI disclosure footer on all four docs
- ✅ No prohibited phrases detected: "SOTA", "beats RLHF", "refutes collapse", "cross-encoder verified", Teleox TAM

## Audience coverage
- **Researcher** (Dohmatob, Perez, Khattab, Kaplan, Kempe, Feng, Shumailov, Olah): cheat-sheet + pitch + 4 discovery Qs + objections 1/2/3/4/5/6/8/10
- **Sovereign** (MGX, Humain, QIA, GIC): cheat-sheet + pitch + 3 discovery Qs + objections 4/7/11/12
- **Banker** (Qatalyst, LionTree, Centerview, Allen): cheat-sheet + pitch + 3 discovery Qs + objections 11/12
- **Hyperscaler** (Alphabet, MSFT, Meta, Amazon, Anthropic, OpenAI, Nvidia): cheat-sheet + pitch + 3 discovery Qs + objections 1/2/4/9/10/11
- **VC** (Casado, Gil, Guo, Midha, Grady, Khosla): cheat-sheet + pitch + 3 discovery Qs + objections 1/8/9/11

## Dependencies for downstream agents
- **Follow-up playbook (16)**: five ready next-steps in cheat sheet page 4 can seed follow-up email templates.
- **Executive brief (18) / START_HERE (00)**: success metrics (min/strong/exceptional) in cheat sheet page 4 can be reused verbatim.
- **Value propositions (08)**: 48h PoC framing in objection #12 + starter #4 + pitch §4B aligns with the `07_48h_poc_offer` convention.
- **Conversation scripts (11)**: 2-min pitch flow (pitches §3) includes pause-points and passes read-aloud.

## Open considerations for orchestrator
1. **Paper status dependency**: every artefact assumes paper PDF + HF LoRA + CI disclosure + scope tags are arXiv-ready. If paper ships with different scope language, pitches/cheat-sheet/objections must be regenerated.
2. **Dohmatob Day-0 move**: cheat sheet puts Dohmatob at P0; this matches orchestrator's TARGET_UNIVERSE_RANKING §3 "highest-leverage single first move" finding.
3. **Humain stated-goal alignment**: the national-data-multiplier framing in pitch §4B and objection #11 aligns with agent-02's finding that Humain's stated goal *is* the literal DDA pitch.
4. **Qatalyst $2B+ floor**: discovery Q-B3 explicitly probes Qatalyst's engagement floor to de-risk agent-03's flagged unknown.

## Files written (SMAP keys)
```
research/04_sales_enablement/14_cheat_sheet.md
research/03_conversation_engineering/12_pitches.md
research/03_conversation_engineering/13_discovery_questions.md
research/04_sales_enablement/17_objection_handling.md
memory/handoff/agent-13-to-orchestrator--enablement-pack.md  (this file)
memory/reports/agent-13-conversation-enablement--final-report.md
```

## Competing Interests Disclosure
Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
