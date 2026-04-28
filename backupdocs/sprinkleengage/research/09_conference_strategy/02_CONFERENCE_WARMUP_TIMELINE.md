---
namespace: research
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T16:05:00Z
status: active
tags: conference-strategy, warm-up, timeline, playbook, steve-principle
---

# Pre-Conference Warm-Up Timeline Playbook

**Steve-stated principle (2026-04-19)**: *"We need to warm people up before going, send out emails, figure out who is going to be there. Try to connect beforehand with important people we want to meet with and try to schedule to meet up with them at these conferences."*

This playbook operationalises that principle using three Playwright-verified authoritative sources (Forbes Abouelenein 2024 / Forbes Agency Council 2016 / Forbes Brustein 2018 — full citations in `../../memory/journal/agent-00-orchestrator--conference-warmup-research-day0.md`).

**Governing doctrine** (inherited, non-negotiable):
- **Primitive-first** — we are not pitching Teleox products; we're presenting a primitive to be critiqued or absorbed (per `memory/context/mission-brief.md`).
- **"Collaborate, never sell"** — the exact framing Forbes Brustein #11 prescribes for pre-event outreach.
- **Scope-guard tags on every Case 3 / DDA mention** (within-WavLM-family, up-to-91 constructive, EXP-2 pending, structurally-outside-Shumailov — per `memory/context/constraints.md`).

---

## The Four Phases

| Phase | Timing | Primary action | Output |
|---|---|---|---|
| **Discovery** | **8 weeks out** | Map who will be there | Target list, personas |
| **Warming** | **4 weeks out** | First touches | Signals sent, ~30% reply rate expected |
| **Booking** | **2 weeks out** | Concrete meeting asks | 5-12 scheduled 1:1s |
| **Staging** | **1 week + day-of** | Final confirms + logistics | Printed run-sheet, room to breathe |

---

## PHASE 1 — DISCOVERY (8 weeks out)

**Forbes Agency Council directive**: *"Have your team spend several weeks making pre-conference calls, setting meetings, and making travel arrangements."* This is the "several weeks" phase.

### Step 1.1 — Confirm attendance is worth it (self-check)

- Per `01_CONFERENCE_PRIORITY_MATRIX.md`: has the go/no-go trigger fired for this conference?
- Log the answer: YES → continue. NO → continue DISCOVERY anyway for post-conference value, but don't book travel.

### Step 1.2 — Enumerate the attendee universe

**Sources**:
1. **Official attendee / speaker page** — Playwright-navigate directly
   - FII: fii-institute.org/conferences-and-summits/fii/
   - Milken: milkeninstitute.org/events/global-conference-2026/speakers
   - NeurIPS: neurips.cc/Conferences/2026/Schedule
   - ICML: icml.cc/Conferences/2026/Schedule
   - SIGIR: sigir2026.org
2. **LinkedIn search** — `"attending [CONFERENCE NAME] 2026"` filter by connections-2nd-degree
3. **X / Twitter** — `"going to [CONFERENCE]"` recent posts
4. **Speaker archives from previous editions** — 70-80% repeat-attendance rate in sovereign-access circuits

### Step 1.3 — Build the target list (do this in Playwright + spreadsheet)

**Columns** (use a single file — `09_conference_strategy/attendee_targets_[CONFERENCE].csv`):
- Name | Organization | Role | LinkedIn URL (verified) | X handle (verified) | Email (verified — never guess) | Priority (A/B/C) | Notes from Playwright recon | First-touch channel | Touch date

**Priority rubric**:
- **A** = direct decision authority for a 3B+ transaction (e.g., PIF Head of Technology; MGX senior principal; Anthropic Chief Scientist)
- **B** = 1-hop connector to an A (e.g., PIF portfolio-company CEO; Khattab-type technical respected voice)
- **C** = useful-to-know, broader ecosystem (e.g., AI investor generalists)

**Volume target per conference**: 30 A's + 60 B's + 100 C's as a starting universe. Winnow down ruthlessly.

### Step 1.4 — Personal recon per A-tier target

For each A, Playwright-navigate:
- Their LinkedIn profile (recent 6-12 months of posts — what are they thinking about?)
- Their X timeline (last 100 posts — tone, interests, recent interactions)
- Any recent podcast / panel / published paper
- Company's most recent funding / strategy announcement

**Capture** in their row: "one specific recent thing we can authentically reference." That recent thing is the hook for the warming phase.

**Forbes Abouelenein #3**: *"Develop a list of key people to meet. Follow them on social media and keep up with their recent activities."* Now you have 30 A-tier's recent activity in one spreadsheet.

---

## PHASE 2 — WARMING (4 weeks out)

**Forbes Abouelenein #1**: *"Share in advance that you are going to be attending that event: Post about the event on social media and signal to your network that you will be there and find mutual connections."*

**Forbes Abouelenein #4**: *"DM speakers or attendees: Establish connections before the event."*

### Step 2.1 — Signal your attendance publicly (ONE time, early)

Single post on X + LinkedIn, 4 weeks out, saying you're going. Do NOT repeat weekly.

Template:
> *"Heading to [CONFERENCE] [dates] in [city]. If you're working on [data-layer sovereignty / alignment-as-verification / late-interaction / data-wall strategy], I'd love to meet. My recent paper on a data-layer primitive (ResearchGate 403916407) might be a useful prompt for a hallway conversation."*

Tone note: invites specific-topic overlap, not a sales-funnel "let's connect." Leaves the scope-guarded reference so technically-alert readers can self-qualify.

### Step 2.2 — DM A-tier targets (first touch)

Use the channel each target prefers (map from recon in 1.4):
- Most frontier-lab researchers → X DM (Olah, Perez, Delétang prefer X)
- Sovereign-wealth / VC principals → LinkedIn InMail or warm-referred email
- Academic researchers → University email (Playwright-verified — never guess)

**Template skeleton** (expanded versions in `03_CONFERENCE_EMAIL_TEMPLATES.md`):

```
Subject: [Specific hook from recon] — [CONFERENCE] coffee?

[Recent-activity hook from recon Step 1.4 — 1 sentence showing you actually read them]

I'm attending [CONFERENCE] [dates]. My recent paper argues [one-line primitive framing] —
[scope tag inline]. If the overlap with your work on [specific recent activity] makes it
worth 15 minutes of your time over coffee at the venue, I'd value the exchange.

Manuscript: researchgate.net/publication/403916407 (DOI 10.13140/RG.2.2.24370.57288).

No expectation either way — if your schedule's locked, I'll look for you on the floor.

— Chris Royse / Teleox.ai
```

**Doctrine embedded**:
- Recent-activity hook = personalized (not template-blasted)
- "15 minutes" = low ask (not "dinner" — that's presumption)
- "Coffee at the venue" = zero-friction logistics
- "No expectation either way" = removes pressure (Brustein principle: relax, everyone is there to connect)
- "I'll look for you on the floor" = leaves room for walk-up even on no-reply

### Step 2.3 — Track replies and mutual-connection introductions

Update spreadsheet columns:
- Touch date
- Response type (no-reply / positive / soft-no / negative)
- If positive: "wants to meet" | "suggests Zoom first" | "asks for PDF"

**Positive-reply SOP (≤24 hours)**:
1. Send PDF immediately if requested
2. If they proposed Zoom pre-conference: confirm 2 slots in next 2 weeks
3. If they said "let's meet at the conference": confirm time + place proactively (don't wait)
4. Log exchange to `memory/journal/agent-00-conference-warmup--[CONFERENCE].md`

### Step 2.4 — Mutual-connection warm referrals (for A-tier non-responders)

For any A-tier who didn't reply within 7 days:
- Scan their LinkedIn 1st-degree connections for people Chris or Steve know
- If a connector exists: ask the connector by DM "Hey, I'm trying to meet [target] at [CONFERENCE]. Would you be willing to make a 2-line intro?"
- If not: let it sit; don't double-tap in Phase 2 (reads as needy per `memory/journal/agent-00-orchestrator--social-posting-discipline.md` principle applied to conferences).

**Expected reply rate across all A-tier DMs**: 20–30%. The warming phase is not about closing; it's about seeding.

---

## PHASE 3 — BOOKING (2 weeks out)

**Forbes Agency Council**: *"Once your action plan is established, have your team spend several weeks making pre-conference calls, setting meetings and making travel arrangements."*

### Step 3.1 — Convert positive replies to confirmed 1:1s

Target: **5–12 pre-booked meetings** per attended conference. Rationale:
- 5 is the floor for "not a wasted trip" at a Tier-1
- 12 is the ceiling because each meeting steals from Thirds Rule floor-networking + session time
- Forbes Agency Council serial-goer targets 12–15 meetings/day, but that's an extreme cadence; for primitive-first technical pitches, quality > quantity (one great 60-min conversation > six surface 15-min chats)

For each confirmed 1:1:
- Propose 2 time slots at venue-adjacent coffee / hotel lobby
- Send calendar invite with exact location + "no agenda, just exchange"
- Add to run-sheet in spreadsheet

### Step 3.2 — Meet the event organizers

**Forbes Abouelenein #5**: *"Meet the event organizers. Connect with those in charge of programming and sponsors."*

For each Tier-1 attended conference, identify and DM:
- Conference general-inquiry / attendee-services email — ask about side-events / private dinners
- Sponsor-relations email — some sponsors host private events where A-tier stickers are denser than main floor

This is not networking for its own sake; organizers control the guest list for the *actual* Tier-1 rooms (private dinners, sponsor suites) that aren't on the public program.

### Step 3.3 — Schedule-pack vs breathing-room (the Thirds Rule)

**Forbes Agency Council Thirds Rule applied**:
- 1/3 sessions (you need to be seen attending select talks to be credible as a "technical person")
- 1/3 networking + pre-booked meetings
- 1/3 exhibitor floor + walk-up conversations

For a 4-day Milken-style conference → ~1.3 days of pre-booked 1:1s → 5–8 meetings is plenty. Overbooking kills the floor-presence that generates the serendipitous Tier-1 intros.

### Step 3.4 — Prepare the "Conference Kit"

Physical + digital:
- QR-code business card (Forbes Abouelenein #6) — single page, ResearchGate URL + schedule.teleox.ai + Chris email + Steve email
- 20 printed paper business cards (backup)
- One-page TCT primitive brief (not the 30-page paper) — for people who say "send me something quick"
- Notebook + two good pens
- Portable phone charger
- Breath mints, hand sanitizer (Forbes Agency Council arsenal)
- Conference app installed on phone ahead of time
- Printed daily run-sheet with 1:1 schedule + 2 walk-up targets per open slot

**Pre-print everything 2 weeks out — don't leave it to the day before.**

---

## PHASE 4 — STAGING (1 week + day-of)

### Step 4.1 — One week out: confirm all 1:1s

For every confirmed meeting:
- Send a 2-line confirmation text / DM the week before ("Still on for [day] at [time] at [place]? Looking forward.")
- If they say "yes" → re-confirm location the morning of
- If they say "circumstances changed" → offer alternative slots
- **Do NOT be pushy on no-replies** — the original booking holds, walk-up if needed

### Step 4.2 — Day-of morning ritual

**Forbes Agency Council**: *"Once checked in, send confirmation texts to each of the people you're scheduled to meet with that day. Before those meetings, set aside time for yourself to practice your pitch."*

Morning-of:
1. Badge check-in + conference-app sync
2. Coffee + review run-sheet
3. Send confirmation text to every meeting scheduled that day (short: "Coffee at [place] at [time] still good?")
4. 10-minute pitch-practice alone — but practice the **primitive framing**, not a product pitch (per `memory/context/mission-brief.md` doctrine)
5. Review LinkedIn + recent posts of today's 3-5 scheduled contacts one more time

### Step 4.3 — During the conference — execution rules

See `04_CONFERENCE_EXECUTION_SOP.md` for on-site behavior (conversation openers, body language, bathroom-line encounters, what-to-do-after-each-meeting, etc.). This playbook ends at the venue door.

---

## SPECIAL CASE — COMPRESSED TIMELINE (< 4 weeks out)

**When it applies**: Milken Global LA 2026 (14 days out as of 2026-04-19) is the archetype. Also any Tier-1 where a go-trigger fires inside the 4-week window.

### Compressed 14-day plan

| Day out | Action |
|---|---|
| **14** | Book travel + registration TODAY; Playwright-verify speaker list |
| **13** | Build target list (skip 8-week recon depth; 15 A's + 30 B's is enough) |
| **12-11** | Send A-tier DMs — all of them — same day |
| **10-8** | Warm referrals where possible; second-touch none yet |
| **7** | Reply collection; book what you have |
| **6-4** | Fill remaining slots with walk-up-preferred contacts |
| **3** | Confirmation SMS to all booked meetings |
| **2** | Print run-sheet, finalize kit |
| **1** | Travel day + last LinkedIn refresh |
| **0** | Day-of morning ritual per Step 4.2 |

Expected outcome at Day 0 in compressed mode: **2–4 pre-booked 1:1s** (instead of 5–12 on normal timeline). That's enough. One great Jensen-Huang-adjacent 1:1 + walk-up floor work can yield a Tier-1 outcome from Milken — don't sabotage it by trying to force a full schedule into 14 days.

---

## What NOT to do (distilled from all three Playwright sources)

**Forbes Brustein #9**: Don't scan everyone's badges looking for your target. It's creepy.
**Forbes Brustein #11**: Don't pitch a product in the pre-conference DM. Frame as "collaborate, never sell." For primitive-first doctrine: frame as "critique welcome, product irrelevant."
**Forbes Emma Johnson 2014**: Don't arrive without a goal. Each conference has one A-tier primary outcome you're there for; everything else is compounding.
**Forbes Agency Council**: Don't skip pre-conference calls/meetings. "Several weeks" of warm-up is the standard, not an upsell.
**Internal doctrine**: Don't use prohibited phrases (SOTA / beats / refutes / cross-encoder verified / commercial TAM) in any conference-adjacent DM, email, or post.

---

## Cross-reference

- Conference-by-conference go/no-go: `01_CONFERENCE_PRIORITY_MATRIX.md`
- Email templates for each phase: `03_CONFERENCE_EMAIL_TEMPLATES.md`
- On-site + post-conference SOP: `04_CONFERENCE_EXECUTION_SOP.md`
- Raw Playwright data (conferences): `RAW_conference_data.md`
- Raw Playwright data (networking best practices): `../../memory/journal/agent-00-orchestrator--conference-warmup-research-day0.md`
