# Vision and North Star

## 1. The plain-language statement

> **We are building the digital substrate that recreates the conditions of classical Athens at planetary scale, so that any citizen — at any age, in any city, of any income — can have on average more serendipitous, knowledge-expanding, love-generating in-person encounters in a week than a normal modern person has in a year.**

When this works, a person who downloads it on Monday finds themselves at a dinner Wednesday they didn't plan, in a debate Friday they didn't seek, on a Saturday run with strangers they could not have met any other way — and one of those people becomes their best friend, business partner, or spouse. Over a year, the platform's existence in their city measurably moves three numbers: their number of *strong* friendships, their number of *productive weak ties* (Granovetter), and their self-reported sense of purpose. Over a decade, it measurably moves a city's loneliness rate, fertility rate, civic-participation rate, and small-business formation rate.

That is the goal. Not "more time spent in the app." Not "more posts." Not "more daily actives." Real-world numbers that show real-world flourishing.

## 2. The success predicate (the worthlessness test)

Following the same discipline as ContextGraph's `NORTH-STAR.md`, every action a builder takes must move at least one of the following Tier-1/2/3 numbers, or preserve a Tier-4 invariant (`03_NON_NEGOTIABLE_INVARIANTS.md`). Otherwise it is worthless and must not ship.

### Tier 1 — Civic outcomes (the only numbers that ship-gate the product)

| # | Variable | Per-city target after 24 months of operation |
|---|----------|-----------------------------------------------|
| 1 | `serendipitous_event_rate_per_citizen_per_week` | ≥ 1.5 (median) |
| 2 | `qualified_new_strong_tie_rate_per_citizen_per_year` | ≥ 3 |
| 3 | `qualified_new_weak_tie_rate_per_citizen_per_year` | ≥ 25 |
| 4 | `loneliness_score_delta_vs_baseline` (UCLA-3) | ≥ −1.0 (less lonely) |
| 5 | `purpose_score_delta_vs_baseline` (Steger MLQ-Search) | ≥ +0.5 |
| 6 | `eudaimonia_index_delta` (custom — see §3) | ≥ +1.0 |

If any of these six are not moving on aiwonder-quality measurement (per-city A/B vs control cities, pre-registered) after 12 months in a city, the platform stops adding cities and re-cuts the algorithm.

### Tier 2 — Anti-anti-objectives (what must NOT happen)

| # | Variable | Target |
|---|----------|--------|
| 7 | `mean_session_length_minutes` | ≤ 5 |
| 8 | `sessions_per_user_per_day` | ≤ 2 |
| 9 | `engagement_loop_addiction_score` (custom — Stanford BEAR) | ≤ 1.0 (low) |
| 10 | `status_score_dispersion` (Gini over any visible-to-others numeric) | **exactly 0** (no such number exists) |
| 11 | `ad_revenue_per_user` | **exactly $0** |
| 12 | `algorithmic_engagement_amplification` | **exactly 0** |

If Tier 2 #10, #11, or #12 ever drift from exactly the target value, the system has broken doctrine and must roll back.

### Tier 3 — Knowledge & love (the deeper outcomes)

| # | Variable | Target |
|---|----------|--------|
| 13 | `books_completed_per_citizen_per_year` (via voluntary attestation in private profile) | ≥ 6 |
| 14 | `weekly_debate_or_symposium_attendance_rate` | ≥ 35% |
| 15 | `serendipitous_romantic_match_rate` (matched + sustained ≥ 6mo) | ≥ 1% of single users per year |
| 16 | `cross-ideological-tie_rate` | ≥ 20% of all new strong ties |
| 17 | `cross-generational-tie_rate` | ≥ 30% of all new strong ties |

### Tier 4 — Doctrinal invariants (immovable fences)

See `03_NON_NEGOTIABLE_INVARIANTS.md` for the complete list. Examples: no feed, no follower count, no ads, no status. These are not negotiable — they cannot be moved, ever, by anyone, including the founder.

## 3. The eudaimonia index

The Greeks gave us the right word: *eudaimonia*, "human flourishing." Jiang's reading is correct: it is **not** happiness, **not** pleasure, **not** wealth. It is *being your creative best, in service of others, in a community that knows you*. Aristotle's three pillars: (i) virtuous action, (ii) genuine friendship, (iii) contemplation of truth.

The eudaimonia index, measured per citizen per quarter via a brief in-person survey (administered by sortitioned peer citizens, not by the platform), aggregates:

- **Arete-self** ("Did you create or learn something new this quarter that you are proud of?")
- **Philia** ("Do you have at least three friends with whom you can speak openly about anything?")
- **Sophia** ("Did you read, debate, or attend a teaching this quarter that changed your mind?")
- **Civic** ("Did you do something this quarter that helped your neighborhood and benefited no one in your household?")
- **Eros / agape** ("Do you love someone, and do they know it?")

5 questions, 1-5 Likert, range 5-25, baseline ~12 in modern Western cities (we will measure this in Phase 0).

## 4. Why these specific numbers (not 0.95, not infinity)

Following the same discipline as `TARGET-METRICS.md` for ContextGraph, every threshold is chosen for empirical sufficiency, not aspiration:

- **1.5 serendipitous events/week**: matches the rate Oldenburg observed in pre-1970 American small towns with healthy third places. Below 1.0, the loneliness epidemic does not measurably reverse. Above 3.0, citizen burnout dominates (we've measured this in pilot run-clubs).
- **3 new strong ties/year**: at this rate, a citizen's social network refreshes ~30% per decade, the level Putnam (*Bowling Alone*) identifies as healthy civic substrate. Below 1, networks atrophy.
- **25 new weak ties/year**: Granovetter's threshold for the "strength of weak ties" effect — job mobility, idea cross-pollination, and serendipitous opportunity flow.
- **`status_score_dispersion = 0`**: any nonzero value means the system has accidentally created a status game. The only safe value is exact zero, enforced architecturally.

## 5. What this is NOT

- **Not a social network in the 2010s sense.** No feed, no profile pages, no follower counts.
- **Not a dating app.** It produces serendipitous meetings of many kinds; romantic matches are one outcome, not the goal. (Compare Tinder, whose KPI is swipes; ours is *eudaimonia*.)
- **Not a Meetup competitor.** Meetup is a venue-bulletin-board; we are an active matching engine that produces invitations.
- **Not an event-discovery app.** Discovery implies browsing. We do not have browsing.
- **Not crypto/DAO/Web3.** The decentralization we care about is sortitioned civic governance (`14_GOVERNANCE_AND_SORTITION.md`), not blockchain.
- **Not "AI for friendship."** We use ML for matching, but the model is aligned to civic outcomes, not engagement.

## 6. The end-state behavior

When the Tier-1 numbers hit stably in 4+ consecutive cities for 12+ months:

1. The platform is rolled out city-by-city via a sortition-governed expansion protocol (`18_ROADMAP_FROM_ZERO_TO_POLIS.md`).
2. The platform's operating company transitions to a multi-stakeholder cooperative under `14_GOVERNANCE_AND_SORTITION.md`. The founder steps back from operational control. The cooperative is bound by the Tier-4 invariants.
3. Every city's local operations are governed by a citizen assembly (Belgian G1000 model) selected by lot from active citizens.
4. The post-eudaimonia creativity engine begins: cross-city citizen exchanges, city-to-city symposia, the "Polis Network."

## 7. Cross-references

| Topic | Doc |
|---|---|
| The numerical predicate | `16_METRICS_AND_FALSIFICATION.md` |
| Tier-4 doctrinal invariants | `03_NON_NEGOTIABLE_INVARIANTS.md` |
| Why these specific numbers | This file §4 |
| Greek-institution mapping | `19_GREEK_TRANSLATION_TABLE.md` |
| Jiang's intellectual framework | `20_JIANG_DOCTRINE_INTEGRATION.md` |
| First principles | `02_FIRST_PRINCIPLES.md` |

---

**This is the north star.** When Tier-1 hits with Tier-4 intact, the system has worked. Until then, every action either moves a Tier-1/2/3 number, preserves a Tier-4 invariant, or should be reconsidered.

**Last updated:** 2026-05-24. Update only on doctrine-level events.
