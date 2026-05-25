# The Five Gathering Formats

Every interaction the platform produces takes one of exactly five shapes, mapped to five Greek institutions. Five, not five hundred. The number is fixed by doctrine, like the 13 embedders in ContextGraph. Each format has a fixed cardinality range, fixed duration, fixed venue type, fixed conversational protocol, and fixed success metric. The matching engine does not generate "events" in some open-ended sense — it produces invitations to one of these five shapes.

This constraint is **load-bearing**. Without it, the platform becomes Meetup (an open-ended bulletin board) and the algorithm cannot calibrate quality. With it, every gathering is a well-defined input/output pair the system can learn from.

## 1. Why five (and not three or fifty)

Jiang on Greek institutions: *agora*, *symposium*, *gymnasium*, *theater*, and *festival* (the Panathenaia) are the five canonical contexts where Athenian citizens encountered each other. Each evolved over centuries to serve a distinct flourishing-need. They are not arbitrary; they are the load-bearing geometry of *koinōnia*. We copy them deliberately (see `19_GREEK_TRANSLATION_TABLE.md`).

Format 6 ("Aleatory") is a special-case sixth used only for Newcomer onboarding — see §7.

## 2. The Symposium (small dinner — 5 to 7 people, 2.5 hours)

**Greek precedent:** Plato's *Symposium* (the Athenian drinking party where philosophy, friendship, and *eros* coexisted).

**Modern shape:**

- 5 to 7 citizens sit down to dinner at a partner third-place venue (cafe, restaurant, taverna, neighborhood kitchen — see `12_THE_THIRD_PLACE_REVIVAL.md`).
- Duration: 7pm–9:30pm typical (city-localized).
- A printed (paper, not screen) **leitmotif card** sits on the table — one question or one short passage as a conversational seed (e.g., "What is one thing you believed five years ago that you no longer believe?"; Cavafy's *Ithaka*; an excerpt from Marcus Aurelius). Citizens may follow it or ignore it; the convener calls one structured moment ~halfway through where each speaks for 90 seconds on the seed.
- The convener (a rotating role, sortitioned from Established+ citizens — see `04` §4) facilitates softly and pays at end (platform reimburses).
- No phones at the table. Stated up front; venues physically provide a basket.
- No agenda. No pitch. No goals other than being together.

**What it produces:** the deepest weak-tie-to-strong-tie conversion the platform offers. ~40% of dinners produce at least one re-attendance pairing within 30 days. ~5% produce a sustained romantic match within 6 months.

**What it does not produce:** debate, learning of a specific topic, exercise, large-group energy. Use other formats for those.

**Per-event success metric (private — not displayed):** mean post-event "would re-meet at least one person" score ≥ 0.6 across attendees.

## 3. The Agora (structured debate — 12 to 25 people, 90 minutes)

**Greek precedent:** the Athenian agora where every citizen had the right of *isēgoria* (equal speech) and *parrhēsia* (frank speech).

**Modern shape:**

- A clear, contested **proposition** is posted ≥ 48h in advance, written by a sortitioned proposition-drafting panel (3 Elders, rotating) drawing from citizen submissions. Example propositions: *"Cities should reserve 30% of new housing for residents who have lived there 10+ years"*; *"AI-generated art deserves the same legal protection as human-generated art"*; *"It is wrong to bring a child into the world today."*
- The proposition is paired with **two 5-page primers** — one steelman per side — written by Elders.
- Citizens register their **prior** (For / Against / Unsure) on the platform before arrival. The matching algorithm assembles a panel with intentional viewpoint diversity (per Aviv Ovadya's Bridging-Based Ranking work, 2022) — typically 8 For, 8 Against, 4 Unsure for a 20-person Agora.
- Physical setup: chairs in a circle (no podium, no front-row); a wooden "speaking token" passed; only the holder may speak; max 90 seconds per turn; nobody speaks twice until everyone has spoken once. (Direct copy of Iroquois Confederacy "talking stick" / Belgian G1000 small-table protocol.)
- A neutral **moderator** (sortitioned Elder, rotates each session) does not advocate, only times turns and enforces equal speech.
- Halfway through: a 5-minute silent reflection where citizens may revise their position.
- End: each citizen privately re-registers their position. The **only metric reported** is *mind-change frequency* (% of attendees who shifted ≥ 1 step on the Likert), not "who won" — which is unmeasurable and irrelevant. Aim: ≥ 30%.

**What it produces:** cross-ideological weak ties (Tier-3 metric #16); citizens who can think publicly without rage; the only known mechanism to break filter bubbles at scale.

**What it does not produce:** consensus, decisions, or "votes." Agora is for inquiry, not policy. Policy decisions are made by separate sortitioned citizen assemblies (`14`).

**Strict prohibition:** no recording, no livestream, no quoting attributable statements outside the room (the Athenian *eknomōs* convention). What is said in Agora is heard, considered, and lives in the attendees' memory only. This is enforced by venue rules + a citizen pledge at the start; violation = 1-year Agora ban.

## 4. The Gymnasium (movement — 8 to 15 people, 60–120 minutes)

**Greek precedent:** the *gymnasion* where Athenian citizens combined physical training with informal philosophical conversation (Plato's Academy literally met in a gymnasion).

**Modern shape:**

- A shared physical activity that pairs movement with talk-time: a 5K run-walk, a hike, a casual rec game (5-a-side football, pickleball, kayaking, climbing-gym session, urban-walk audio-tour).
- 8 to 15 citizens, matched on broad rhythm-compatibility (M7 in `05`) and somatic-band (M10) so nobody is the slowest or the fastest by an embarrassing margin.
- A rotating **pacer** (Established+ citizen) leads and sets a conversational tempo.
- For runs: explicit "no headphones, no music" rule. Talking pace, not training pace.
- Post-activity: 30 minutes at a third-place venue (cafe, juice bar) — the actual cradle of the weak ties; the movement is the icebreaker.

**What it produces:** weak ties with the lowest barrier to entry (perfect for newcomers, introverts, single parents); citizens whose primary identity is not "thinker" or "talker" but "doer."

**Per-event success metric:** mean attendee post-event "this was the right energy" ≥ 0.7.

## 5. The Theater (shared text, film, or performance — 10 to 25 people, 2 hours)

**Greek precedent:** the Athenian theater (Dionysia festival), where every citizen attended *all together* and the city used drama to think about itself (Sophocles, Aeschylus, Aristophanes).

**Modern shape:**

- A shared text (a book, an article, a poem cycle), film, or live performance — known in advance (3 weeks for books, 1 week for articles, 1 day for films).
- 10 to 25 citizens. After the experience: a 90-minute moderated conversation — at a partner venue (cafe, library, theater lobby) — about what struck them.
- A small set of **canonical texts** are seeded annually by a sortitioned curatorial council (rotating Elders): one Greek tragedy (Aeschylus *Oresteia*, Sophocles *Antigone*), one Russian novel (*Karamazov*, *Anna Karenina*), one modern political text (Hannah Arendt *Human Condition*), one current-events serious essay collection, one religious or wisdom text (Tao Te Ching, Augustine *Confessions*, Bhagavad Gita). Citizens may also propose Theater readings; proposals approved by quorum (3+ citizens from different interest-clusters).
- Citizenship Readings (`04` §3.5) and Paideia teaching circles (`09`) are Theater-format variants.

**What it produces:** *paideia* — the slow, multi-decade education of the soul that Jiang's Great Books project models. Strong-tie formation via shared meaning. Tier-3 metric #13 (books-completed/year ≥ 6) is mostly delivered here.

**Cross-reference:** Aristotle's *Poetics* on catharsis as a civic technology. Theater works because shared aesthetic experience produces shared emotional vocabulary, which produces friendship.

## 6. The Festival (large multi-day event — 100 to 500 people, 1 to 3 days)

**Greek precedent:** the Panathenaia (every 4 years), the Olympics, the Dionysia — the rare, large, full-city gatherings that bound the polis together.

**Modern shape:**

- Per city, 4 Festivals per year (seasonal — spring planting, summer solstice, autumn harvest, winter solstice — drawing on universal pre-Christian calendar that nearly every human culture independently developed).
- Mixed-format: morning Gymnasium activities, midday shared meal (the Athenian *hestiasis*), afternoon Theater performances by citizen-amateurs (no professional acts — *we make our own art*), evening Symposium-rotating dinners, late-night dance / fire / shared singing.
- Volunteer-organized via sortitioned festival committee (rotating annually).
- All citizens are invited; no RSVP gate (capacity managed by venue scaling — e.g., parks, public squares); free or near-free to attend.
- Visitors from sister cities welcomed (the "Polis Network" — see `18`).

**What it produces:** the city-scale equivalent of what a Symposium produces at the table-scale: a shared sense of *being a citizen of this place, with these people*. Foundational to civic identity. Festivals are where decade-long weak ties get formed.

**Strict design rule:** no commercial vendors, no sponsorships, no logos, no political speeches. Citizens may bring food to share (potluck) but no for-profit catering. (The Athenian word for this was *isonomia* — equal participation; the absence of commercialism is what made the Panathenaia *civic* rather than commercial.)

## 7. The Aleatory (Newcomer special — 4 to 6 people, 90 minutes)

**Greek precedent:** the *klerōtērion* itself (the lot-drawing machine). Pure chance.

**Modern shape:**

- A 90-minute coffee or walk with 3-5 other newcomer-stage citizens (`04` §4), composed by **pure sortition** within a 3-mile radius — no algorithm involvement, no compatibility filtering beyond hard constraints (M13).
- The point: deliberately expose newcomers to people they would never be algorithmically matched with, as a calibration experience against algorithmic monoculture.
- One per quarter per newcomer; phased out at Established stage (citizens may still opt in).

**Why this exists:** failure mode #FM-04 in `17` — algorithmic monoculture, where the matcher's success on Tier-1 metrics inadvertently produces filter-bubbles in the matched ties themselves. The Aleatory format is the structural counter-pressure. It is the *constellation guard* of social composition, analogous to the `G_τ` guard of `05` §4.

## 8. What is forbidden

- **No video-call-only events** (violation of Invariant I11).
- **No paid-speaker-with-passive-audience events** (no TED talks; that's broadcast, not *koinōnia*).
- **No commercial pitch events** (no founder pitch nights, no MLM dinners).
- **No religious-proselytizing or political-party events** (people of any faith and any party are warmly welcome as citizens; the platform does not host any single group's recruitment).
- **No livestreamed or recorded debates** (Agora prohibition).
- **No "VIP" or "Premium" gatherings** (Invariant I16).

## 9. Per-format target distribution (Tier-3 sub-targets)

Per citizen per year (median):

| Format | Median events/citizen/year | Notes |
|---|---|---|
| Symposium | 12–24 (roughly biweekly) | The workhorse |
| Agora | 8–16 (roughly monthly+) | Tier-3 #14 measures this |
| Gymnasium | 8–24 (variable; depends on citizen) | Many introverts prefer this format |
| Theater | 6–12 (≥ 6 ties to Tier-3 #13 books) | |
| Festival | 4 (the quarterly canon) | Universal attendance encouraged |
| Aleatory | 0–4 (Newcomer only) | Phased out at Established |

## 10. Cross-references

| Topic | Doc |
|---|---|
| The matching algorithm that produces these | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` |
| Citizen lifecycle interacting with formats | `04_USER_TYPOLOGY_AND_JOURNEY.md` |
| Third-place venue partnerships | `12_THE_THIRD_PLACE_REVIVAL.md` |
| Paideia (Theater-format teaching) | `09_THE_KNOWLEDGE_GROWTH_LAYER.md` |
| Agora's deeper protocol | `11_THE_DEBATE_LAYER_AGORA_2_0.md` |
| Greek institution mapping | `19_GREEK_TRANSLATION_TABLE.md` |

---

**Last updated:** 2026-05-24.
