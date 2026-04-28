---
namespace: research
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T16:15:00Z
status: active
tags: conference-strategy, execution, on-site, post-conference, sop
---

# On-Site + Post-Conference SOP

**Scope of this doc**: what to do from the moment you walk into the venue through the 30-day post-conference follow-through. Pre-conference warming is in `02_CONFERENCE_WARMUP_TIMELINE.md`; this doc picks up at the venue door.

**Inherited principles**:
- Forbes Agency Council Thirds Rule: 1/3 sessions, 1/3 meetings, 1/3 exhibitors
- Forbes Brustein relational-not-transactional (Roizen anchor)
- Forbes Abouelenein "bring value first"
- Scope-guard doctrine enforced verbatim in every in-person utterance (Case 3 never uttered without within-WavLM-family; "up to 91" never uttered without "constructive upper bound")
- Primitive-first framing — if asked "what's Teleox?" answer with the primitive, not the products

---

## DAY 0 — Travel day / Check-in

- [ ] Land with a full phone, full laptop, hydrated
- [ ] Check into hotel within walking distance of venue (15-min walk max)
- [ ] Check into conference registration → pick up badge → download app
- [ ] Walk the venue for 30 minutes — know where the coffee bar / men's room / main-hall entrance / quiet lobby corners are. This saves 20 minutes/day of being lost during the conference
- [ ] Locate + confirm your first meeting's meeting place (morning-of timing)
- [ ] Do NOT attend anything evening Day 0 unless there's a specific A-tier at that reception — jet lag + tired pitch = bad first impression
- [ ] Early dinner, early sleep
- [ ] Review tomorrow's run-sheet one last time before bed

---

## MORNING-OF RITUAL (every conference day)

Per `02_CONFERENCE_WARMUP_TIMELINE.md` §4.2, expanded:

1. **06:30–07:00** — Wake, coffee, NO email/Slack yet
2. **07:00–07:30** — 20-min solo pitch practice out loud. **What you're practicing**:
   - 30-second opener: "I found a primitive in the AI data layer. A fixed corpus of n inputs projected through N frozen embedders yields up to n·(N+C(N,2)) supervisory signals — counting identity, constructive upper bound, pairwise-MI tightness pending."
   - 2-minute expansion: add §3.3 scope (not a Shumailov refutation) + §5.4 verifiability predicate class
   - 30-second "what's Teleox / your company": "A primitive published from commercial engineering practice. Teleox is the operating company; the paper is ResearchGate 403916407; the Shakespeare LoRA is the MEASURED reproducibility anchor."
3. **07:30–08:00** — Review today's scheduled 1:1s + 2 walk-up targets per open slot. Pull each person's LinkedIn / X for anything they said in the last 24 hours
4. **08:00–08:30** — Send confirmation text to every scheduled meeting that day: "Coffee at [place] at [time] still good?"
5. **08:30** — Leave for venue

---

## THE THIRDS RULE IN PRACTICE

### Session-attendance third (1/3)

- Pick sessions where A-tier targets are **speaking** — not sessions you'd pick purely on intellectual interest
- Sit front-third of room (visible + first to Q&A mic)
- Ask one question per day during a Q&A session. ONE. Short. Phrased as a question, not a platform. **Forbes Brustein #4**: making yourself visible draws approaches afterward
- Take note of who asked other good questions — they're approachable post-session per Forbes Brustein #3

### Meetings third (1/3)

- Start meetings ON TIME. Be at the coffee 5 min early
- **15-minute meeting script**:
  - 0:00–0:30 — shake hands, sit, light acknowledgement ("thanks for making time")
  - 0:30–2:00 — 90-second primitive framing (what you practiced in morning ritual)
  - 2:00–5:00 — single specific question tied to their work ("Given your recent paper on X, does §3.3 scope hold under Y?" — scope-guarded and concrete)
  - 5:00–12:00 — THEIR response. You listen. You take notes (pen + notepad — not phone; phone reads as disengaged)
  - 12:00–13:00 — ONE follow-up question, not three
  - 13:00–14:00 — "what would be useful from me?" (Forbes Abouelenein "bring value first" principle) — explicit offer to send additional material, make an intro, share the LoRA weights
  - 14:00–15:00 — exchange cards / QR / LinkedIn. Confirm follow-up timeline
- If they extend: roll with it. If they close: close. Never pad for time.

### Exhibitor + floor third (1/3)

- Walk the floor with **deliberate slow-walking** and eye contact availability (Forbes Brustein #13: "post up")
- Visit booths only of companies you'd genuinely want to know about. Avoid swag-collecting — it's a distraction
- Hallway conversations: if someone approaches you or you're 3 feet from a target per Forbes Brustein #2, use Template 6 walk-up opener from `03_CONFERENCE_EMAIL_TEMPLATES.md`

---

## CONVERSATION OPENERS (Forbes Brustein distilled)

**With someone next to you at a session** (before speaker starts):
- *"Are you familiar with this speaker?"*
- *"What made you choose this session?"*

**With someone next to you at a session** (after speaker finishes):
- *"What did you take away?"*
- *"I really loved that question you asked."* (if they asked a good one)

**At food/drink area**:
- *"What's been one of the better talks of the conference so far for you?"*

**Lobby / quiet-corner during breaks**:
- *"Mind if I join you for a minute? Just trying to be horizontal for 5 minutes between sessions."*

**Walk-up to a target** (Template 6 from email templates, short form):
- *"[First name] — that [specific panel/question/moment] earlier — I'm Chris Royse. My recent paper argues [one-sentence primitive]. Is there a time this week I could send you a short read?"*

---

## EVENING RITUAL (every conference day)

- [ ] Back to hotel by 22:00 (or venue after-party if A-tier confirmed there)
- [ ] Spread notebook on desk. Review every meeting of the day — **re-write key quotes in cleaner form while fresh**
- [ ] For each meeting, note: "does this person deserve a follow-up within 24h?" (A-tier yes; B-tier yes; C-tier within 72h)
- [ ] Send Template 7 follow-ups for A-tier met today (do not delay to tomorrow — Forbes Abouelenein "follow up fast")
- [ ] Update attendee_targets_[CONFERENCE].csv with outcomes
- [ ] Check tomorrow's run-sheet once; then stop
- [ ] Sleep by 23:00 — this is the non-negotiable rule that prevents Day 3 degradation

---

## SPECIAL SCENARIOS

### Scenario A — A-tier target you hoped to meet is at a reception you weren't invited to

- Do NOT crash the reception. It's detectable and it burns bridges in the sovereign-capital circuit.
- Instead: find them the next morning at the general-admission coffee bar. Early mornings are the most under-utilized 1:1 window at every conference.

### Scenario B — You're invited to a private dinner / suite event

- Go. This is where the Tier-1 rooms actually happen.
- Do NOT bring a stack of papers or pitch materials. Dinner pitch = status downgrade.
- Primitive-first orally; when someone asks for the paper, text them the ResearchGate URL at the table.

### Scenario C — You get a cold walk-up from a C-tier

- Be gracious. Forbes Brustein #14: be open to everyone. You never know who they're connected to.
- 5 minutes max. Hand over QR card. Offer to connect on LinkedIn.
- Do NOT extend past 5 minutes if you have scheduled A-tier coming. Politely: "I've got a meeting coming up at [time] but let's connect on LinkedIn and find a time."

### Scenario D — A journalist / press finds you

- Forbes Abouelenein #8: volunteer to speak to press = free publicity.
- BUT scope-guard discipline applies to press MORE than to researchers. Press misquotes land on Google forever.
- Primitive-first framing, verbatim. Never claim Case 3 speaker generalization beyond within-WavLM-family. Never claim jailbreak-proof Shakespeare LoRA; always "deterministic".
- Get the reporter's email. Offer to fact-check the direct quotes before publication — most reputable reporters will agree. Always follow up with the quote you want them to use in email within 2 hours.

### Scenario E — Someone offers on-the-spot to invest / partner / license

- Do NOT commit. "I'd love to take this further — let's put time on the calendar next week."
- Add them to the run-sheet priority follow-up.
- A real 3B+ transaction never closes at a conference floor; it opens there. Protect Chris's / Steve's authority to walk it through proper process.

### Scenario F — You're invited onto a panel last-minute

- Forbes Agency Council: "once you attend an event and know how it works, find a way to bring value to the event and offer to speak or moderate a panel" — this is exactly that.
- Say yes if the topic is primitive-adjacent (data-wall, alignment-as-verification, late-interaction, derived supervision).
- Prepare a 90-second primitive intro + 3 scope-guarded talking points. Bring the one-page brief, not the 30-page paper.

---

## POST-CONFERENCE SOP (Day +1 through Day +30)

### Day +1 — recovery (don't disconnect)

- Forbes Agency Council: *"Give yourself a day to decompress but don't disconnect completely."*
- No meetings. No new outbound.
- Finish Template 7 follow-ups for anyone you didn't email the night-of (B/C tier)

### Day +2 — team debrief (Chris + Steve)

- Go through the attendee_targets spreadsheet end-to-end
- Mark outcomes per row:
  - **LOCKED** = 1:1 confirmed for post-conference follow-up
  - **WARM** = met, positive, no specific next step
  - **COLD** = met but no signal
  - **WALK-UP ONLY** = bumped into, card exchanged, no real conversation
  - **NO-SHOW** = scheduled but didn't meet
- For every LOCKED: set calendar reminder for Day +7 follow-up
- For every WARM: set reminder for Day +14
- For every COLD: archive unless specific reason to re-engage
- For every WALK-UP ONLY: send a LinkedIn connection request with 1-line reference to the conference

### Day +3 to Day +7 — compound

- Send summary emails to LOCKED contacts with the specific deliverables promised
- Handwritten thank-you notes to the 3 most valuable conversations (Forbes Agency Council). Ship via hotel concierge or post. This is a low-volume / high-signal gesture — do NOT scale it to more than 3 per conference

### Day +7 — post-conference LinkedIn post

ONE post recapping the conference, no tags, no humblebrag:

> *"Good week at [CONFERENCE]. A few conversations that sharpened §[relevant section] of the manuscript — particularly around [topic]. The [specific technical question] came up repeatedly and is now at the top of our EXP-2 priority queue. Thanks to everyone who made time."*

Posted once. Not repeated.

### Day +14 — evaluate

- Which LOCKED contacts became 2nd-meeting-scheduled?
- Which WARM contacts re-engaged organically?
- Conversion rate: pre-conference target was 5–12 1:1s; how many actually compounded past Day 14?
- Update `memory/journal/agent-00-orchestrator--conference-[NAME]-outcomes.md` with honest rate

### Day +30 — strategic integration

- For every LOCKED contact still engaged: explicit Q&A milestone (e.g., "will review EXP-2 results when they land")
- Add to conference-recurring-attendees tracking — the same person who warmed well at Milken LA May is worth a 30-sec "we met" nod at NeurIPS Dec. The 12-month compounding of 10 Tier-1 relationships is how Chris's network grows from "0 sovereign contacts" to "3-4 warm sovereign contacts who take a call."

---

## THE ASYMMETRY

**A no-result conference is fine.** The warm-up emails happened. The primitive got in front of new people. A conference you attended and got zero 1:1s still beats a conference you didn't warm at all. **The failure mode is showing up cold.**

**A big-result conference compounds for years.** The one Jensen-Huang-level contact at Milken who takes a 45-min Zoom 8 weeks later is worth more than the 11 C-tier handshakes combined. Protect your schedule for the A-tier.

**The pre-conference warm-up creates asymmetric value independent of attendance.** If Chris doesn't go to FII 10, but the LinkedIn DMs warmed 6 PIF principals to the primitive thesis, the next attempt (FII 11 in 2027 or a direct London-Zoom with one of them in Q3 2026) starts from "we've corresponded" rather than "never heard of you."

---

## Cross-reference

- Go/no-go per conference: `01_CONFERENCE_PRIORITY_MATRIX.md`
- Warm-up timeline (pre-conference): `02_CONFERENCE_WARMUP_TIMELINE.md`
- Email templates: `03_CONFERENCE_EMAIL_TEMPLATES.md`
- Raw Playwright conference data: `RAW_conference_data.md`
- Networking-research source snapshots: `../../memory/journal/agent-00-orchestrator--conference-warmup-research-day0.md`
- Day-0 pre-send checklist (reuse for conference DMs): `../../memory/blockers/agent-00-orchestrator--pre-send-checklist.md`
