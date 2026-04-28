---
namespace: handoff
agent_id: agent-11-outreach-drafts
timestamp: 2026-04-19T03:15:00Z
status: active
tags: handoff, orchestrator, outreach-drafts, day-0-to-7
---

# Handoff: agent-11 → orchestrator (outreach drafts)

## What the orchestrator receives from this agent

A complete, ready-to-send Day-0–7 outreach package covering:

1. Two Day-0 cold emails (Dohmatob, Khattab) — the load-bearing first touches identified by agents 01 and 04 as highest-leverage.
2. Day-1 social amplification pair (X thread, LinkedIn post).
3. Day-2 MATS Summer 2026 mentor-application narrative (Perez stream) — the published outsider channel into Anthropic.
4. Day-3–4 fan-out emails (Feng+Kempe joint, Longpre, Delétang).
5. Day-5–7 tier-2 fallback warm-ups (Olah X reply template, Baroni cold, Villalobos cold).
6. Day-14 breakup/re-engage template for any silent recipient from drafts 01/02/06/07/08.

All 12 drafts pass the pre-send checklist in `memory/blockers/agent-00-orchestrator--pre-send-checklist.md`.

Primary deliverable: `research/07_outreach_artefacts/11_email_drafts.md` (~14KB, 12 drafts + send-day matrix + 8 operational guardrails).

## Decisions locked by this agent (that downstream agents should not re-litigate)

1. **PDF-on-reply pattern**: every cold draft promises "PDF on your reply" rather than attaching on first touch. This is a micro-commitment forcing function; do not change to attach-on-send.
2. **One concrete ask per email**: enforced across all 12 drafts. Do not stack asks.
3. **arXiv TBD placeholder** is live in all drafts — Chris must replace with real URL after arXiv post; orchestrator should flag this as the gating step for any send.
4. **Breakup template sends once only**: Day-28 silence archives that contact until a new measured artefact exists (EXP-1, EXP-2, v2 paper, workshop acceptance).
5. **Reactive drafts (Olah)** only fire on topically-relevant posts. Do not schedule reactive drafts; they are opportunistic by design.

## What the orchestrator should track (gating dependencies)

- **Paper arXiv URL**: all 11 outreach drafts carry `[arXiv: TBD — placeholder]`. Must be resolved before any send.
- **Email-address verification**: Dohmatob Concordia primary + Meta fallback; Kempe NYU Courant; Feng NYU primary; Longpre MIT Media Lab; Delétang Google DeepMind (author email); Baroni UPF primary + Meta fallback; Villalobos Epoch AI staff. Orchestrator should confirm each address before Chris sends.
- **MATS Summer 2026 deadline**: Draft 05 is only useful if the application window is open at send time. Orchestrator or Chris must verify `matsprogram.org` deadline before Day 2.
- **ECIR 2026 Late Interaction Workshop deadline**: Draft 02 offers a workshop submission. Orchestrator should confirm ECIR 2026 CFP is still open, or the offer becomes unsupported.
- **Reply tracking**: Day-14 breakup template should be prepped for each Day-0–4 recipient as a scheduled follow-up if silent.

## What's NOT in this package (orchestrator should request follow-up if needed)

1. **Tier-A/B/C decision-maker drafts** (Kaplan, McCandlish, Amodei at Anthropic; Ahmed Yahia Al Idrissi at MGX; Tareq Amin at Humain; Brad Lightcap at OpenAI; Alexandr Wang at Meta MSL; Rohit Prasad at Amazon). These require either (a) a researcher co-sign landed first, or (b) Track A/B/C conference access. Per TARGET_UNIVERSE_RANKING §4, hold these until Day 7+ stock-take.
2. **Banker drafts** (Qatalyst, Allen & Co, Centerview). TARGET_UNIVERSE_RANKING §4 schedules the Qatalyst 1-pager for Day 10; this agent intentionally did not preempt that.
3. **Conference-attendance travel plans** (FII Miami Mar 25-27 2026; Milken MEA Abu Dhabi Dec 2026; LEAP Riyadh Mar 2026; Web Summit Qatar Feb 2027). Out of scope for a drafts-only package.
4. **VC-channel drafts** (Guo, Gil, Casado, Midha). Per agent-04 top-5, these are Tier-2 post-researcher-cosign targets — deliberately skipped.

## Files written this wave

```
research/07_outreach_artefacts/11_email_drafts.md
memory/handoff/agent-11-to-orchestrator--outreach-drafts.md
memory/reports/agent-11-outreach-drafts--final-report.md
```

## Competing Interests Disclosure

> Chris Royse is founder/operator of Teleox.ai (primary) and Leapable.ai (companion MCP marketplace). This package emerges from commercial engineering practice. Readers should weight content as a methodology contribution with acknowledged commercial affiliation.
