# User Typology and Citizen Journey

## 1. There are no "users." There are citizens.

Language matters. "Users" implies consumption; "citizens" implies participation. The platform refers to every account-holder as a *citizen* of their *polis* (city). This is not branding — it shapes the entire UX. A citizen has rights (to be matched, to attend, to debate, to leave); responsibilities (to show up, to receive newcomers, to do post-event feedback); and a position in a community (their neighborhood, their interest-cluster, their generational cohort).

## 2. Citizen segments — descriptive only, never displayed

The matching engine internally distinguishes ~12 latent typologies. These are **never shown to citizens**, never used for marketing, and have no influence on individual treatment beyond making matches more useful. Examples:

- **The lonely newcomer** (just moved to the city, knows no one)
- **The local veteran** (lived in the city 10+ years, dense network already)
- **The seeker** (mid-life pivot, hungry for ideas)
- **The convener** (loves hosting, naturally a node)
- **The introvert with depth** (one-on-one preference, drained by groups)
- **The extrovert with energy** (large-group preference)
- **The cross-generational connector** (likes mixed-age gatherings)
- **The single, eros-curious** (open to romantic match if it arises)
- **The partnered, philia-curious** (looking for friends, not romance)
- **The intellectual** (debate-first)
- **The somatic** (movement / sport / food-first)
- **The civic** (volunteering / community-organizing-first)

Latent allocation is **soft, multi-label, and updates** based on stated preferences (primary signal) and post-event feedback (secondary signal). No citizen sees their own labels or anyone else's.

## 3. The onboarding (the "becoming a citizen" ritual)

Onboarding is deliberately substantial. Lunchclub failed in part because onboarding was 30 seconds and matches were therefore mediocre. We err the other way.

### 3.1 Step 1 — Proof of personhood (5 min)

One of: in-person attestation by an existing citizen (preferred — see §3.6), Worldcoin Orb (where available), or government-ID + selfie+video liveness (fallback). Stored as a zero-knowledge unique-personhood credential per `2026/333` (Choudhuri, zkBricks). Never sold. Never used to deanonymize.

The system also stamps the credential with **city anchoring** — the polis you participate in. You can be a citizen of at most 1 city at a time (to prevent gaming). Moving cities is allowed but takes 60 days to take effect (so people don't switch to game match queues).

### 3.2 Step 2 — The intake interview (15 min, conducted by a human)

A live, 15-minute video call with a sortitioned **welcoming citizen** (rotating role; lightly compensated in service-credit, not money — see `13`) who asks five questions:

1. *Tell me three things you've cared about in the last year — anything, no judgment.*
2. *What is one question that has been keeping you up at night?*
3. *Describe a moment in the last year you felt most alive.*
4. *What would you like more of in your life?*
5. *Is there anyone — alive, dead, real, fictional — you wish you could have a long dinner with, and why?*

The welcoming citizen transcribes responses with the citizen's consent (the citizen can edit before saving). Responses become the seed for the citizen's Constellation (`05`). The welcoming citizen records a single private "vibe note" (5-10 words) — never shown to the new citizen, used as soft input to matching for the first 90 days only.

**Why a human, not a form:** the data quality from a 15-minute conversation with a curious human is order-of-magnitude better than a form. Greek symposia worked because they were conversations. Forms produce LinkedIn.

### 3.3 Step 3 — The First Format (within 7 days)

The new citizen is matched to one of the lower-stakes Five Gathering Formats (`06`) within their first week — usually a Symposium (small dinner of 5–7), a Gymnasium (run/walk, 8–15), or a Theater (book or film discussion, 10–25). Not Agora (debate) first — that comes later, once the citizen has formed at least three weak ties.

### 3.4 Step 4 — The follow-up loop (perpetual)

After every event:

- 24h: a short, private reflection prompt (3 questions, 90 seconds): *Was it worth your time? Did you meet someone you'd want to see again? Was there anyone you'd rather not meet again?*
- 7 days: optional re-attendance prompt for "people you'd want to see again."
- 30 days: tie-strength check — did any of these turn into ongoing contact?

The 30-day signal is **the primary training signal for the matching algorithm**. (See `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §6.) Showing up to events is not the metric; producing ongoing real relationships is.

### 3.5 Step 5 — The citizenship test (after 90 days or 5 events, whichever later)

The citizen attends one Theater-format session called the *Citizenship Reading*, where they discuss with 8–12 other recently-arrived citizens a short text on each of: (i) the Iliad's ending, (ii) Granovetter's weak-tie paper, (iii) the platform's 18 invariants, and (iv) one community-elder essay from their polis's local archive. The "test" is not pass/fail; it is a ritual of orientation. The citizen leaves understanding the platform's philosophy in a way that makes them a competent participant and (crucially) a competent welcomer for future newcomers.

This step is borrowed directly from the Athenian *ephēbeia* — the two-year civic apprenticeship every Athenian male went through before becoming a citizen.

### 3.6 The growth loop is invitation-based, not viral

Citizens may invite up to 5 new citizens per quarter. Invitation = personal vouching. The inviter's reputation does **not** affect the invitee's matching, but if the invitee no-shows repeatedly or reports negative experiences, the inviter receives a private notification ("the people you bring in seem to be having a rough time — want to talk to a community advisor?"). Trust without surveillance, accountability without status.

No "growth hack" mechanics. No referral bounties. No "invite 10 friends, unlock X." The platform's growth rate is intentionally bounded; see `18_ROADMAP`.

## 4. The lifecycle stages

| Stage | Roughly when | What changes |
|---|---|---|
| **Newcomer** | months 0–3 | Lighter-stakes formats; matched primarily on broad compatibility; weekly check-ins |
| **Established** | months 3–24 | Full menu of formats; eligible to be a welcoming citizen; can convene Symposia and host Theaters |
| **Elder** | year 2+ | Eligible for sortitioned governance roles; can convene Agora; can teach in Paideia (`09`) |
| **Inactive** | 90 days no event | Soft pause — no penalty, no shaming; one outreach offer; otherwise full silence |
| **Departed** | self-declared or 12 months inactive | Account archived. All personal data deletable on request per GDPR/CCPA. No "memorial pages." |

Note the deliberate absence of "VIP," "Premium," "Verified," or any badge tier. Elder status is **functional** (it enables certain civic actions like convening Agora) and **visible only at events** (the convener wears, for example, a simple wooden token at a Symposium). It is not displayed in any UI list.

## 5. The "what's on my phone" question

A citizen's home screen in the app shows:

1. **The next thing** — a single calendar item ("Dinner with 5, Wednesday 7pm, Cafe Olive, 0.8mi") with a one-line "what you might talk about" briefing and the first names + 1-line bios of attendees.
2. **Optional pre-read** — for Theaters, a 5-page article; for Agora, the proposition under debate.
3. **One reflection prompt** — only if there's an event in the last 7 days needing feedback.
4. **A "set my preferences" link** — pause, adjust radius, change interests, etc.

That's it. No feed. No "you might also like." No notifications other than the next event and direct messages from people you've actually met.

Median screen-time per session: **under 2 minutes**. (Tracked, capped by Tier-2 metric #7.)

## 6. The "what happens after the event" question

Two paths:

- **Path A (most common):** the people you met exchange contact details directly, in person, with no platform mediation. The platform asks the next day if it should retain the connection privately to inform future matching. If yes, kept; if no, forgotten.
- **Path B (when both opt in):** the platform provides a lightweight, end-to-end-encrypted **direct message** channel between people who have met in person at least once. No group chats. No threads. No reactions. Just text + image (no auto-play media). The DM channel exists only after an in-person meeting has happened and at least one party has explicitly enabled it. **There is no "match me with X without meeting them first."**

This is the architectural enforcement of "IRL-first." Online communication exists only as a continuation of in-person, not as a substitute.

## 7. The exit ritual

When a citizen decides to leave, the platform offers (does not require) a 30-minute exit conversation with a sortitioned citizen-advisor. The goal: discover whether the platform failed the citizen, the citizen's life changed, or something else; capture the lesson; thank them; delete what they want deleted; preserve only the anonymized event-outcome data needed for algorithmic learning. The citizen receives a printed (yes, paper-mailed) one-page summary of "your time as a citizen of [polis]" with the events they attended, the people they met (with names redacted by default; opt-in to keep), and any text artifacts they want to keep.

Departure is honored, not punished. The shame-based "if you leave, your friends get notified" mechanic that Instagram has perfected is precisely what we forbid.

## 8. Cross-references

| Topic | Doc |
|---|---|
| Five Gathering Formats | `06_THE_FIVE_GATHERING_FORMATS.md` |
| Matching algorithm | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` |
| The Constellation substrate | `05_THE_CONSTELLATION_DDA_LAYER.md` |
| Proof of personhood | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` §3 |
| Sortitioned roles | `14_GOVERNANCE_AND_SORTITION.md` |

---

**Last updated:** 2026-05-24.
