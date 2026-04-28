---
namespace: journal
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T15:24:00Z
status: active
tags: day-0, sends, audit-trail, outbound
---

# Day-0 Outbound Send Log

Chronological record of outreach emails Chris sent from `chrisroyseai@gmail.com` on Day 0
(2026-04-19). Each entry: recipient + send-time + draft version + reply-watch trigger +
14-day-breakup-template trigger date.

---

## Send 1 — Dr. Elvis Dohmatob (Concordia + Mila)

- **Sent**: 2026-04-19 (Chris-confirmed) via chrisroyseai@gmail.com headed Gmail session
- **To**: elvis.dohmatob@concordia.ca
- **Subject**: Data-layer primitive vs. Strong Model Collapse — 15 min + EXP-2?
- **Draft version**: Draft 01 v2 (Playwright-verified 2026-04-19)
- **Opening hook**: "Why Less is More (Sometimes): A Theory of Data Curation" (ICLR 2026)
- **Ask**: co-author EXP-2 pairwise-MI audit + 15-min Zoom; PDF on reply
- **Anchors cited**: Shakespeare LoRA MEASURED 2026-04-17 (HF public); Case 3 0.961 within-
  WavLM-family MEASURED 2026-04-16 (EXP-1 pending)
- **Paper URL**: ResearchGate 403916407 + DOI 10.13140/RG.2.2.24370.57288
- **Pre-send check passes (all ✓)**: measured anchor / scope tag / LoRA link / CI disclosure /
  paper section citations / no prohibited phrases / required preserve-scope phrasing /
  single concrete ask / explicit "not a supervision request" framing addressing his Concordia-
  page filter
- **Reply-watch trigger**: 2026-04-26 (7 days). If reply lands → immediate follow-up on
  Zoom scheduling + PDF send.
- **14-day breakup-template trigger**: 2026-05-03 if still silent.

## Send 2 — Prof. Omar Khattab (MIT EECS & CSAIL)

- **Sent**: 2026-04-19 (Chris-confirmed) via chrisroyseai@gmail.com headed Gmail session
- **To**: okhattab@mit.edu
- **Subject**: Late interaction generalized to N=13 — data-layer primitive
- **Draft version**: Draft 02 v2 (Playwright-verified 2026-04-19)
- **Opening hook**: LIR workshop @ ECIR 2026 keynote acknowledgement (he was keynote speaker
  + co-organizer — workshop happened April 2, 2026)
- **Ask**: (a) read 30-page manuscript draft for pool-relative novelty feedback (§3.4);
  (b) if sound, advice on NeurIPS 2026 / SIGIR 2026 workshop venue fit;
  (c) optional EXP-2 pairwise-MI co-authorship for him or one of his students.
- **Anchors cited**: WARP (SIGIR 2025 Best Paper) as retrieval-efficiency adjacency;
  DDA as signal-density adjacency; ColBERT MaxSim = E12 in panel; Shakespeare LoRA MEASURED
  2026-04-17 (HF public); Case 3 0.961 within-WavLM-family MEASURED 2026-04-16 (EXP-1 pending)
- **Paper URL**: ResearchGate 403916407 + DOI 10.13140/RG.2.2.24370.57288
- **Pre-send check passes (all ✓)**: same as Send 1 plus specific-community-acknowledgement
  (LIR keynote) opener
- **Reply-watch trigger**: 2026-04-26 (7 days).
- **14-day breakup-template trigger**: 2026-05-03 if still silent.

---

## Day-0 stock-take (end of day, 2026-04-19)

**Sent**: 2 researcher cold emails (Dohmatob, Khattab) — highest-leverage Day-0 moves complete.

**Queued for Day 1 (2026-04-20)**:
- Draft 03 — X/Twitter thread (8 tweets, tag @dohmatobelvis @EthanJPerez @lateinteraction
  @feeelix_feng @ch402 @jaredkaplan_)
- Draft 04 — LinkedIn post (longer-form variant for VC / corp-dev / technical-exec audience)

**Queued for Day 2 (2026-04-21)**:
- Draft 05 v2 — MATS Autumn 2026 (formerly Summer 2026; pivoted after Playwright verified
  Summer 2026 applications closed Jan 18). EOI form live now at matsprogram.org/apply;
  general application opens "late April" — could be any day this week.

**Queued for Days 3–5 (Apr 22–24)**:
- Draft 06 — Feng + Kempe joint (addresses v2-corrected per Playwright)
- Draft 07 — Longpre (slongpre@media.mit.edu verified)
- Draft 08 — Delétang (route via X-tag; direct email not publicly exposed)
- Draft 10 — Baroni v2 (Playwright-corrected: ICREA Barcelona, not FAIR Paris; mbaroni@gmail.com)
- Draft 11 — Villalobos v2 (route via ops@epoch.ai with "please forward" prefix)

**Outstanding**:
- MATS Autumn 2026 EOI can be submitted NOW (2-min form) to lock in notification when
  application opens — doesn't have to wait until Day 2.
- schedule.teleox.ai still carries Squarespace placeholder content — needs polish or CTA
  re-route before any C-suite outreach uses the booking link.
- Tier-2 tracks A/B/C commercial outreach gated behind Day-14 "≥1 researcher endorsement"
  requirement per `memory/blockers/agent-00-orchestrator--day14-fallback.md`.

---

## Reply-handling SOP

If Dohmatob replies:
1. Reply within 24h with PDF attached (ResearchGate PDF download + cover note).
2. Propose 2–3 Zoom slots in next 2 weeks using schedule.teleox.ai slot (once that page is
   polished — see CEO refactor prompt §5.2) OR a direct Calendly alternative.
3. If positive scope-endorsement signal in reply → unlock Track A/B/C commercial outreach
   per Day-14 gate in `memory/blockers/agent-00-orchestrator--day14-fallback.md` (gate
   requires ≥1 researcher endorsement).
4. If technical pushback on §3.3 → document verbatim in
   `memory/discoveries/agent-NN-dohmatob-feedback--scope-critique.md` and flag for paper v2
   revision. This is the desired outcome even if the answer is critical — real empirical
   correspondence is what the paper needs.
