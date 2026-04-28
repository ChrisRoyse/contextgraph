---
namespace: research
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T16:20:00Z
status: active
tags: conference-strategy, index, readme
---

# 09 — Conference Strategy

**Origin request (Chris, 2026-04-19)**: *"Me and Steve want to go to conferences. Use Playwright and get actual real information on all the conferences you recommended and strategies to use. Steve just said we need to warm people up before going, send out emails, figure out who is going to be there. Try to connect beforehand with important people we want to meet with and try to schedule to meet up with them at these conferences. He says this is important. Do research on this and best practices as well and then give me all new documentation on conferences and how to approach them and strategy."*

**Evidence discipline**: Every factual claim in this directory is Playwright-verified from a primary source — the conference official page OR a named-author Forbes / HBR article. No built-in web research, no LLM-invented facts.

---

## Files in this directory (read in this order)

| # | File | Purpose | When to read |
|---|---|---|---|
| 00 | `00_README.md` | This index | First — orients you to the package |
| 01 | `01_CONFERENCE_PRIORITY_MATRIX.md` | Go/no-go per conference + budget envelope | When deciding which conferences to attend |
| 02 | `02_CONFERENCE_WARMUP_TIMELINE.md` | 8/4/2/1/day-of playbook | Once a conference is selected — run this timeline |
| 03 | `03_CONFERENCE_EMAIL_TEMPLATES.md` | 8 pre-conference DM/email templates | When writing outreach to specific people |
| 04 | `04_CONFERENCE_EXECUTION_SOP.md` | On-site behavior + post-conference SOP | Day of travel + ongoing |
| — | `RAW_conference_data.md` | Underlying Playwright data on 10 conferences | Reference / re-verify dates |

---

## The single highest-leverage decision right now

**NeurIPS 2026 Position Paper — submission closes May 4 abstract / May 6 full. 15-17 days out.**

Whether Chris attends NeurIPS in December or not, the Position Papers track is the cleanest peer-review venue for TCT's fourth-taxonomic-entry argument (§3.2). The manuscript is already written. The lift is NeurIPS-format compliance, not content. **This decision cannot be deferred.**

If the answer is "yes submit," the reformatting starts this week. If the answer is "no skip," that skip is deliberate, not an oversight.

---

## Tier-1 conference summary (detail in `01_CONFERENCE_PRIORITY_MATRIX.md`)

| Conference | Dates | Location | Window | Trigger |
|---|---|---|---|---|
| **Milken Global LA** | May 3-6, 2026 | Beverly Hills | **14 days — compressed** | ≥1 confirmed attendee reply |
| **FII PRIORITY Europe Rome** | Jun 17-19, 2026 | Rome | ~2 months | ≥1 European sovereign-capital reply + ticket <$10K |
| **NeurIPS 2026** | Dec 6-12, 2026 | Atlanta | ~8 months | Paper accepted OR ≥1 researcher confirms attendance |
| **FII 10 Riyadh** | Oct 26-29, 2026 | Riyadh | ~6 months | ≥1 PIF / MGX / ADIA officer books pre-conference Zoom |

---

## Pre-conference warm-up in one table (from `02_CONFERENCE_WARMUP_TIMELINE.md`)

| Phase | Timing | Primary action | Output |
|---|---|---|---|
| Discovery | 8 wks out | Map who will be there | 30 A + 60 B + 100 C target list |
| Warming | 4 wks out | First DMs via verified channels | 20-30% reply rate |
| Booking | 2 wks out | Convert positive replies to 1:1s | 5-12 scheduled meetings |
| Staging | 1 wk + day-of | Confirm + logistics + pitch practice | Printed run-sheet |

For <4-week windows (Milken LA right now), use the Compressed 14-Day plan in `02_CONFERENCE_WARMUP_TIMELINE.md` §Compressed Timeline — expected output is 2-4 pre-booked 1:1s instead of 5-12. Still worth it.

---

## Scope-guard doctrine (inherited, applied to conferences)

Every pre-conference email, every in-person utterance, every follow-up must carry these hedges when the relevant fact is mentioned:
- **Case 3 voice**: `within-WavLM-family, N=10 held-out English sentences, one speaker, single session, EXP-1 pending`
- **"Up to 91" DDA signals**: `constructive upper bound, EXP-2 pairwise-MI audit pending`
- **Shumailov reference**: `structurally outside the recursion regime — scope, not refutation`
- **G_τ verifiability predicate**: `different property class than scalar-reward alignment — not a benchmark contest`
- **Case 1 Shakespeare LoRA**: `MEASURED 2026-04-17, HF public, weights + training recipe reproducible`

Primitive-first frame (never product pitch):
- **Opening line in every context**: "I have a primitive in the AI data layer."
- **Not**: "I want to sell you Teleox." "I have a product that does X." "Teleox has a TAM of $1.7T."

---

## Three authoritative networking sources underlying this package

(Full captures in `../../memory/journal/agent-00-orchestrator--conference-warmup-research-day0.md`)

1. **Forbes — Maha Abouelenein (2024-09-27)** — 8-tip pre-event playbook, relational-not-transactional
2. **Forbes Agency Council (2016-08-10)** — serial-conference-goer process, Thirds Rule, 12-15 meetings/day target
3. **Forbes — Darrah Brustein (2018-04-01)** — 15 conversation openers, "collaborate never sell" framing

Supplementary anchor: **HBR IdeaCast Ep. 1002 Heidi Roizen (2024-11-26)** — "make networking relational, not transactional."

---

## Immediate actions by priority

### This week (2026-04-19 to 2026-04-26)

1. **NeurIPS Position Paper go/no-go** (Chris decision)
2. **Milken LA 14-day compressed warm-up** if trigger fires (see `01_CONFERENCE_PRIORITY_MATRIX.md` §1B)
3. Playwright-verify FII PRIORITY Europe Rome registration page (open question from RAW data)
4. Standard Day-0 through Day-5 email fan-out continues (see `07_outreach_artefacts/11_email_drafts.md`)

### Next 2 weeks (2026-04-26 to 2026-05-10)

1. NeurIPS Position Paper submission (if go) by May 6 AoE
2. Milken LA attended (if go)
3. FII PRIORITY Europe Rome 8-week Phase 1 DISCOVERY kicks off (April 22 onwards)

### Next 2 months (2026-05-10 to 2026-07-10)

1. FII PRIORITY Europe Rome Phase 2 WARMING (~May 20) + Phase 3 BOOKING (~June 3)
2. Attend FII Europe Rome June 17-19
3. ICML Seoul Phase 1 DISCOVERY kicks off (~May 11) — only if Tier-2 trigger fires

### Rest of year

- July: ICML Seoul (if Khattab/Perez reply) + SIGIR Melbourne (if Khattab reply specifically)
- Aug-Sep: LEAP Riyadh (if stacking with FII 10)
- Oct: FII 10 Riyadh (primary Track A target) + possible Milken Asia Singapore
- Dec: NeurIPS Atlanta (if paper accepted or reply signal)

---

## Cross-references out of this directory

- Primitive doctrine: `../05_executive_synthesis/THE_PRIMITIVE.md`
- Email drafts (Days 0-5 standing outreach): `../07_outreach_artefacts/11_email_drafts.md`
- Day-1 social posts (X + LinkedIn): `../07_outreach_artefacts/DAY1_social_posts.md`
- Target universe (non-conference): `../00_target_universe/`
- Track briefs (A sovereign / B Anthropic / C Qatalyst): `../08_track_briefs/`
- Standing constraints: `../../memory/context/constraints.md`
- Standing mission brief: `../../memory/context/mission-brief.md`
- Scope-check per email: `../../memory/blockers/agent-00-orchestrator--pre-send-checklist.md`
