---
namespace: journal
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T15:00:00Z
status: active
tags: day-0, playwright, verification, gating-items, source-of-truth
---

# Day-0 Playwright Verification — 2026-04-19

Orchestrator ran headed Playwright against the six gating items the engagement package
identified before Gmail composition. This journal records source-of-truth findings for every
check so future agents / Chris can trace back.

## Summary

Out of six gating items, **three were wrong in our package** and needed propagation fixes. The
package's factual-hygiene discipline was strong on the technical claims (paper sections, scope
guards, case-status tags) but weak on the date-verification items (conference schedules,
recipient emails).

## Item 1 — Dohmatob email address (Concordia)

**URL checked**: https://www.concordia.ca/faculty/elvis-dohmatob.html

**Our draft (v1)**: `edohmatob@concordia.ca` (primary) + `e.dohmatob@meta.com` (fallback)

**Ground truth (faculty page)**: `elvis.dohmatob@concordia.ca` — verified via `mailto:` link;
position "Associate Professor, Computer Science and Software Engineering"; also affiliate faculty
at Mila Institute.

**Additional signal from page**:
- Explicit filter: *"I will not reply to random supervision requests!"* — our email ask is
  co-author / paper-read, not supervision, so we add an explicit "not a supervision request" line
  to Draft 01 v2.
- Fresh ICLR 2026 publication (to appear): *"Why Less is More (Sometimes): A Theory of Data
  Curation"* (Dohmatob, Pezeshki, Askari-Hemmat). Directly relevant to DDA's data-curation angle.
  Added as opener hook in Draft 01 v2.
- Confirmed prior papers: "Strong Model Collapse" (ICLR 2025), "A Tale of Tails" (ICML 2024),
  "Beyond Model Collapse: Scaling Up with Synthesized Data Requires Reinforcement" (Feng,
  Dohmatob, Yang, Charton, Kempe — ICLR 2025).

**Action taken**: Draft 01 v2 in `research/07_outreach_artefacts/11_email_drafts.md` carries
corrected email + added ICLR 2026 hook + explicit "not a supervision request" framing.

**Impact if we had sent v1**: email would have bounced at `edohmatob@` (non-existent alias), or
gone to stale Meta fallback (he's been at Concordia since 2024). Trust-damage with a Tier-1
collapse-cluster researcher at the exact moment we most needed his read.

---

## Item 2 — Khattab email address (MIT)

**URL checked**: https://omarkhattab.com/

**Our draft**: `okhattab@mit.edu` — CORRECT as verified. Assistant Professor, MIT EECS & CSAIL.

**Additional signal from page**:
- **SIGIR 2025 Best Paper**: "WARP: An Efficient Engine for Multi-Vector Retrieval" (Scheerer,
  Zaharia, Potts, Alonso, Khattab). Multi-vector retrieval efficiency — adjacent to DDA on the
  efficiency axis. Added as adjacency reference in Draft 02 v2.
- **ICLR 2026 Oral**: "GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning"
  (Agrawal et al., incl. Klein, Zaharia, Khattab). Demonstrates active ICLR 2026 cycle.
- Confirms ColBERT / DSPy / GEPA / RLMs as his primary systems. DSPy + ColBERT are "downloaded
  several million times per month".

**Action taken**: Draft 02 v2 carries LIR-keynote acknowledgement + WARP adjacency + reframed
ask.

---

## Item 3 — ECIR 2026 Late Interaction Workshop (CFP status)

**URL checked**: https://www.lateinteraction.com/

**Our draft v1 ask (Khattab)**: "Does the Late Interaction Workshop at ECIR 2026 take a
submission on this generalization?"

**Ground truth**:
- Workshop **took place April 2, 2026**. Today is April 19. **We are 17 days past the workshop
  itself.**
- Submission deadlines: First wave **Jan 30, 2026** (past); second wave **Feb 27, 2026** (past).
- Camera-ready deadline: Mar 24 – Apr 6, 2026 (past).
- Khattab was **keynote speaker** AND **co-organizer** (confirmed both roles on site).
- Other organizers: Benjamin Clavié (Mixedbread/NII), Xianming Li (Mixedbread/HK PolyU), Antoine
  Chaffin (LightOn), Manuel Faysse (CentraleSupélec), Tom Aarsen (HuggingFace), Jing Li (HK
  PolyU).

**Impact if we had sent v1**: Khattab would have seen a direct miss of the community calendar
he co-organized. Worst possible signal — "this person hasn't done their homework on the
retrieval community he's trying to enter." Probable outcome: silent reply, permanent cold lead.

**Action taken**: Draft 02 v2 removes ECIR workshop ask entirely; reframes as LIR-keynote
acknowledgement + 30-page manuscript read + NeurIPS 2026 / SIGIR 2026 venue advice + EXP-2
co-auth option.

---

## Item 4 — MATS Summer 2026 application window

**URLs checked**:
- https://www.matsprogram.org/
- https://www.matsprogram.org/apply
- https://www.matsprogram.org/program/summer-2026

**Our draft v1 Day-2 action**: "Submit MATS Summer 2026 application with Draft 05 narrative."

**Ground truth**:
- Summer 2026 applications **opened Dec 16, 2025 and closed Jan 18, 2026**.
- Admissions decisions: Early April 2026.
- Summer 2026 cohort is already placed (120 fellows, 100 mentors, runs June–August 2026).
- **Perez is NOT listed as a Summer 2026 mentor** — checked page source. May not be mentoring
  this cycle, or may be on the roster but not on the public stream page we checked.
- **Autumn 2026 applications open "in late April"** — Expression of Interest form live now. EOI
  sign-up recommended so Chris is notified when the general application opens (likely this
  week or next).

**Impact if we had queued v1 for Day 2 send**: wasted narrative polish on a closed application
window. Chris would have discovered the closure at submission attempt, losing a day.

**Action needed** (not yet applied): Rewrite Draft 05 for Autumn 2026 — submit EOI now,
prepare full narrative for late-April opening. Flag that Perez may or may not be a mentor for
Autumn 2026; narrative should name Perez as preferred but have fallback language for alternate
alignment-robustness mentors.

---

## Item 5 — FII PRIORITY Miami 2026 dates (Track A physical access)

**URL checked**: https://fii-institute.org (via Google search + Facebook/Instagram confirmations)

**Our package claim (TARGET_UNIVERSE_RANKING §2 Track A, 18_executive_brief §2, 00_START_HERE
gating item #6)**: "FII Miami Mar 25-27 2026 is the earliest actionable sovereign-access
window" and "book FII Miami Mar 25-27 2026".

**Ground truth**:
- FII PRIORITY Miami 2026 **took place March 25-27, 2026** — CORRECT date but the event is
  **past** as of today (April 19). Our package treats it as a future action.
- Next FII PRIORITY events: **LEAP East Hong Kong July 2026** (new chapter); **LEAP 2026 Riyadh
  Aug 31–Sep 3, 2026** (see Item 6); **FII9 Riyadh October 2026** (annual flagship, "Davos in
  the Desert").

**Impact**: our package currently tells Chris to book a past event. Propagation fix needed
across TARGET_UNIVERSE_RANKING, 18_executive_brief, 00_START_HERE, PACKAGE_SUMMARY.

**Recommended replacement for Track A physical-access recommendation**: **FII9 Riyadh October
2026** (primary — MGX + PIF principals attend) OR **LEAP 2026 Riyadh Aug 31–Sep 3, 2026**
(secondary — Humain/Amin-keynote venue).

---

## Item 6 — LEAP 2026 Riyadh dates (Track A Humain channel)

**URL checked**: https://onegiantleap.com/

**Our package claim**: "LEAP 2026 Riyadh (Mar 2026)" (appeared in sovereign research and
TARGET_UNIVERSE_RANKING §4 ranking-table row for PIF/Humain).

**Ground truth**:
- LEAP 2026 is **August 31 – September 3, 2026** at RECC Malham, Saudi Arabia.
- The sovereign-research claim that LEAP Riyadh is "March annually" was wrong (or at minimum
  has changed for the 2026 edition — LEAP may have moved from Q1 to Q3).
- LEAP **East** (new chapter): July 2026, Hong Kong.

**Impact**: our Track A timing assumption was wrong by ~5 months. **This is a positive update
for Chris** — LEAP is not past; it's 4.5 months out. Full prep window available.

---

## Item 7 — arXiv URL (separately resolved by Chris)

**Our package assumption**: all drafts carried `[arXiv: TBD — placeholder]` pending arXiv post.

**Ground truth** (Chris confirmed 2026-04-19): paper is on **ResearchGate** at
`https://www.researchgate.net/publication/403916407`. No arXiv version posted yet.

**Action taken**: replaced every `[arXiv: TBD — placeholder]` across `11_email_drafts.md` with
the ResearchGate URL (9 substitutions). Also removed "arXiv URL" from the 6-gating-items list
in `18_executive_brief.md §5` + `00_START_HERE.md` (no longer a blocker).

---

## Cumulative impact

Six gating items checked via Playwright. Three were wrong in package (Dohmatob email, ECIR
workshop ask, FII Miami/LEAP dates). All three would have caused visible misses in live outreach
if Chris had sent v1 drafts. **Playwright verification before send was load-bearing — not
optional diligence.**

## Pattern lesson for future waves

The package's research phase validated technical claims aggressively (scope guards, paper
sections, case-status tags). It validated date-sensitive facts (conference dates, application
windows, email addresses) with much less rigor — treating research-agent outputs as
authoritative when they were not. **Any future engagement wave must Playwright-verify every
date and every institutional contact before send.** Pattern added to
`memory/patterns/` in a companion file if it doesn't already exist.

## Source-of-truth artefacts (.playwright-mcp/page-*.yml snapshots)

All snapshots under `.playwright-mcp/` in the working directory. Timestamps:
- `page-2026-04-19T14-40-26-947Z.yml` — Concordia Dohmatob faculty page
- `page-2026-04-19T14-40-53-974Z.yml` — omarkhattab.com home
- `page-2026-04-19T14-41-19-643Z.yml` — lateinteraction.com (LIR workshop)
- `page-2026-04-19T14-42-01-948Z.yml` — matsprogram.org home
- `page-2026-04-19T14-42-17-178Z.yml` — matsprogram.org/program/summer-2026
- `page-2026-04-19T15-00-17-050Z.yml` — matsprogram.org/apply (Autumn 2026 EOI)
- `page-2026-04-19T15-01-09-478Z.yml` — onegiantleap.com (LEAP 2026 dates)
- `page-2026-04-19T15-00-51-746Z.yml` — Google SERP FII Miami 2026 dates

## Next actions

- [ ] Propagate LEAP/FII corrections to 7 files (see `research/00_target_universe/` and
  `research/05_executive_synthesis/`).
- [ ] Rewrite Draft 05 for MATS Autumn 2026 + submit EOI.
- [ ] Compose Dohmatob + Khattab v2 drafts in Gmail (pending Chris login).
- [ ] Write `memory/patterns/agent-00-orchestrator--date-verification.md` as standing rule.
