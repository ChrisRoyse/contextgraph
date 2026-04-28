---
namespace: reports
agent_id: agent-11-outreach-drafts
timestamp: 2026-04-19T03:20:00Z
status: active
tags: final-report, outreach, drafts, day-0-to-7, audit-trail
---

# Final Report — agent-11-outreach-drafts

## Mission

Produce ready-to-send email drafts and social copy for Days 0–7 of the execution plan, preserving paper §8 scope on every touch, per the pre-send checklist in `memory/blockers/agent-00-orchestrator--pre-send-checklist.md`.

## Outcome

All 12 required drafts produced in a single file, each individually passing the pre-send checklist. Full send-day matrix included. Eight operational guardrails appended.

## Read-phase trace (SMAP compliance)

Files read before writing:
- `memory/context/mission-brief.md`
- `memory/context/constraints.md`
- `memory/patterns/agent-00-orchestrator--smap-conventions.md`
- `memory/blockers/agent-00-orchestrator--pre-send-checklist.md`
- `research/00_target_universe/TARGET_UNIVERSE_RANKING.md`
- `memory/decisions/agent-04-warm-channels--top-5-ins.md`
- `memory/handoff/agent-04-to-orchestrator--warm-channel-ranking.md`

## Drafts produced (audit trail)

| # | Recipient | Day | Channel | Body words | Subject chars | Pre-send check |
|---|---|---|---|---|---|---|
| 01 | Dohmatob | 0 | Email | ~195 | 64 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 02 | Khattab | 0 | Email | ~190 | 68 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 03 | X thread (8 tweets) | 1 | X | thread | 280/tweet | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤280/tweet ✓][1ask ✓] |
| 04 | LinkedIn post | 1 | LinkedIn | ~270 | N/A | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][1ask ✓] |
| 05 | MATS application | 2 | Portal | 520 | N/A | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][1ask ✓] |
| 06 | Feng + Kempe | 3 | Email | ~180 | 67 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 07 | Longpre | 4 | Email | ~185 | 66 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 08 | Delétang | 4 | Email | ~195 | 69 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 09 | Olah X reply | 5–7 reactive | X | ≤280 ×2 | N/A | [M ✓][S ✓ (T2)][L ✓][C ✓ (T2)][§ ✓][¬ ✓][= ✓][≤280 ✓] |
| 10 | Baroni | 5 | Email | ~195 | 62 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 11 | Villalobos | 5 | Email | ~195 | 63 | [M ✓][S ✓][L ✓][C ✓][§ ✓][¬ ✓][= ✓][≤200 ✓][≤70 ✓][1ask ✓] |
| 12 | Breakup template | D+14 rolling | Email | ~90 | original + "closing the loop" | [C ✓][¬ ✓][≤200 ✓][≤70 ✓][1ask ✓] |

**Pre-send checklist key**:
- [M] measured anchor present (Case 1 Shakespeare LoRA MEASURED 2026-04-17 or Case 3 voice MEASURED 2026-04-16)
- [S] Case 3 scope tag complete (within-WavLM-family + N=10 + one speaker + English + EXP-1 pending)
- [L] LoRA HF link `huggingface.co/cabdru/shakespeare-lora-gemma4`
- [C] CI disclosure line verbatim
- [§] paper § section citation (§0.2/§3.3/§5.4/§7.3/§8/§9 as applicable)
- [¬] no prohibited phrases (no "state of the art"/"beats"/"refutes"/"cross-encoder verified"/"91 signals" without "up to"/commercial TAM/"industry-leading")
- [=] required preserve-scope phrasing ("up to n·(N+C(N,2))"/"constructive upper bound, tightness pending EXP-2"/"structurally outside the Shumailov-form recursion regime (scope, not refutation)"/"per-output verifiability relative to the chosen centroid panel"/"different property class, not a benchmark contest"/"MEASURED 2026-04-17" or "MEASURED 2026-04-16"/"within-WavLM-family, N=10, one speaker, English"/"encoder-matched scope disclosed")
- [≤200] body ≤ 200 words (emails only)
- [≤70] subject ≤ 70 chars (emails only)
- [1ask] single concrete ask

Every draft passes every applicable item. No exceptions.

## Scope-preservation verification

Each draft was audited against the prohibited-phrase list:
- ❌ "state of the art" applied to Teleox — NOT PRESENT in any draft
- ❌ "beats RLHF" / "outperforms DPO" / "superior to Constitutional AI" — NOT PRESENT
- ❌ "refutes collapse" / "breaks Shumailov" — NOT PRESENT (replaced verbatim with "scope, not refutation")
- ❌ "cross-encoder verified" / "universal identity agreement" — NOT PRESENT (replaced with "within-WavLM-family" + "ECAPA-TDNN triangulation (EXP-1) pending")
- ❌ "91 signals per input" without upper-bound caveat — NOT PRESENT (every mention carries "up to" + "constructive upper bound" + "EXP-2 pending")
- ❌ Commercial TAM claims from Teleox — NOT PRESENT; Delétang draft explicitly states "not claimed superior"
- ❌ "world's most advanced" / "industry-leading" — NOT PRESENT

## Scope-preservation phrase coverage

| Phrase | Drafts where present |
|---|---|
| "up to n·(N+C(N,2)) structured supervisory signals — constructive upper bound, tightness pending EXP-2" | 01, 02, 03 (tweet 1+2), 04, 05, 06, 07, 08, 10, 11 |
| "structurally outside the Shumailov-form recursion regime (scope, not refutation)" | 01, 03 (tweet 3), 04, 05, 06, 07, 08, 10, 11 |
| "per-output verifiability relative to the chosen centroid panel" | 02, 03 (tweet 4), 04, 05, 07, 09, 10 |
| "different property class, not a benchmark contest" | 02, 03 (tweet 4), 04, 05, 07, 09, 10 |
| "MEASURED 2026-04-17" (Case 1) | 01, 02, 03 (tweet 5), 04, 05, 06, 07, 08, 10, 11 |
| "MEASURED 2026-04-16" (Case 3) | 01, 02, 03 (tweet 6), 04, 05, 06, 07, 08, 09, 10, 11 |
| "within-WavLM-family, N=10, one speaker, English" | 01, 02, 03 (tweet 6), 04, 05, 06, 07, 08, 09 (T2), 10, 11 |
| "encoder-matched scope disclosed" | 01, 03 (tweet 6), 04, 05, 06, 07, 08, 09 (T2), 11 |

## Risks and gating items (flagged for orchestrator)

1. **arXiv URL not yet live** — all drafts carry `[arXiv: TBD — placeholder]`. Chris must not send until the real arXiv link is inserted.
2. **Email-address verification pending** — orchestrator or Chris should confirm each address (Dohmatob Concordia, Kempe NYU Courant, Feng NYU, Longpre MIT Media Lab, Delétang GDM author email, Baroni UPF, Villalobos Epoch) before send. Bounce-on-first-touch is reputation damage.
3. **MATS Summer 2026 deadline** — Draft 05 is only useful if the application window is open at Day 2.
4. **ECIR 2026 Late Interaction Workshop CFP** — Draft 02 offers a workshop submission; orchestrator should confirm CFP still open or the offer becomes unsupported.
5. **Breakup template discipline** — send exactly once per silent recipient at Day 14. Do not chain a third follow-up.

## Memory keys written

```
MEMORY KEYS WRITTEN:
- research/07_outreach_artefacts/11_email_drafts.md
- memory/handoff/agent-11-to-orchestrator--outreach-drafts.md
- memory/reports/agent-11-outreach-drafts--final-report.md
```

## Competing Interests Disclosure

> Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
