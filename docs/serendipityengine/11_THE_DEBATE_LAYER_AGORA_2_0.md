# The Debate Layer — Agora 2.0

The Athenian agora is the institution most aggressively *destroyed* by social media. Twitter, Reddit, and YouTube comments offer the form (anyone can speak about anything) without any of the substance (equal speech-time, in-person presence, slow consideration, no recording, no audience-as-mob, the *eknomōs* convention of off-the-record). The result is rage-engagement, polarization, and the death of the public sphere.

This layer rebuilds the agora. Carefully. Slowly. With the discipline of someone who has studied why the original worked.

## 1. The thesis

Modern democracies can only function if their citizens can think *together* across deep disagreement without violence and without rage. We have lost this capacity. The cause is not "polarization" as a passive cultural drift; it is the *replacement of in-person deliberation with broadcast-and-clap online performance*. The cure is not "better content moderation"; it is the architectural restoration of the conditions under which deliberation worked: small panels, in-person, equal speech-time, no recording, viewpoint-diverse, conducted in a posture of mutual inquiry.

Jiang's lectures `WoTOxE4DTFA` (Aristotle), `_3c3FjS57x4` (Kant/Hegel), `5HwZx5kQm9o` (Reason), and especially `iTuRB48Of9Y` (the original Greek democratic protocol) supply the historical case. Aviv Ovadya's Bridging-Based Ranking (2022) supplies the modern algorithmic complement.

## 2. The Agora format (detailed)

The condensed spec is in `06` §3; this expands.

### 2.1 Proposition selection

- **Source:** citizens submit proposition candidates via a simple form. Submissions accumulate in a queue.
- **Drafting:** a rotating 3-Elder proposition-drafting panel (sortitioned, 3-month terms) selects 1-2 propositions per city per week from the queue. Selection criteria: (a) the proposition is *genuinely contested* (existence of a plausible steelman on both sides); (b) it matters to the citizens of this polis right now; (c) it is not in the "third-rail / harm" zone (see §6).
- **Steelmanning:** for each proposition, the drafting panel commissions two 5-page primers — one from a citizen-Elder who genuinely holds the For position and one who holds the Against. Neither is a lawyer-style brief; both are sincere best-efforts. (Modeled on Jonathan Rauch's *Constitution of Knowledge*, and the long Talmudic tradition of writing your opponent's argument better than they would.)
- **Posting:** the proposition + both primers are sent to all interested citizens ≥ 48h before the Agora. Citizens may register interest, indicate prior position, and add their proposition for a future Agora.

### 2.2 Attendee composition (the matching algorithm)

The matcher (`07`) assembles a 12-25 person panel deliberately balanced:

- ~ 40% prior For, ~ 40% prior Against, ~ 20% prior Unsure (citizens may self-identify on the registration form; we trust their attestation).
- M12 ideological-diversity boosted in this format.
- M9 topic-portfolio diversity rewarded (we want a builder, a healthcare worker, a teacher, a retired contractor in the same room — not just twelve people who already think about this issue).
- M5 vocabulary diversity rewarded (range from poetic to plain).
- M13 hard constraints honored (accessibility, language).
- Sortitioned shuffle: ~25% of slots randomly selected from any willing citizen with no compatibility filtering.

### 2.3 Physical protocol (the in-room shape)

- Chairs in a single circle. No podium. No front-row. No screen.
- A neutral **moderator** (sortitioned Elder, rotates each session) — does not advocate, only times turns and enforces equal speech.
- A wooden **speaking token** (this is not a metaphor — a literal hand-carved token; many cities have inherited a local one and use it ritually). Only the holder may speak. Max 90 seconds per turn.
- **Nobody speaks twice until everyone has spoken once.** (This is the load-bearing rule. It is the most-violated rule of modern discourse.)
- ~ 45 minutes of structured turns.
- 5 minutes of **silent reflection**.
- ~ 35 minutes of free open exchange (token still circulates, time-limits relaxed but moderator may invoke).
- 5 minutes private re-registration of position. Aggregate (not per-person) mind-change rate is calculated.

### 2.4 The post-Agora silence

Per the *eknomōs* prohibition (`06` §3): no recording, no livestream, no quoting attributable statements outside the room. What is said in Agora is *heard, considered, and lives in the attendees' memory*. Violations: 1-year Agora ban + sortitioned-review-board hearing.

This is non-negotiable. Without it, every citizen self-censors to their reputation and the Agora becomes a public theater of safe positions. With it, the room becomes a sanctuary for honest doubt.

### 2.5 The Mind-Change Aggregate

The only public output of an Agora is a single number: "% of attendees who shifted ≥ 1 step on the Likert from pre- to post-Agora." This is reported to the citizenry as an aggregate per-Agora number with no per-attendee data, no demographics, no "who changed which way."

Why this is the metric: it is the *only* measurable outcome that corresponds to inquiry (vs. performance). High mind-change = real deliberation happened. Low mind-change can mean either (a) the proposition was already deeply considered by the panel, or (b) the discussion didn't move anyone. Either is information, neither is a "win."

There is no "winning side" output. Asking "who won" is asking the wrong question.

## 3. The Bridging-Based Ranking layer (when text-debate exists)

For exceptional cases where text-debate is genuinely necessary — e.g., asynchronous deliberation on a citywide proposition that requires a 1,000-citizen response — the platform supports an **Agora Online** mode with **Bridging-Based Ranking** (Aviv Ovadya, "Bridging-Based Ranking," Belfer Center 2022; deployed by Twitter Community Notes 2021-present, by Polis (pol.is, vTaiwan), and by Meta's "Community Forums" 2024 pilots).

### 3.1 The mechanism

Text comments are *not* ranked by engagement (likes, replies). They are ranked by **bridging score** — a comment receives a higher rank if it has received agreement from citizens whose *prior positions* are *opposed*. A comment that elicits "agree" from people who disagreed on the underlying proposition is structurally a bridge.

This produces a dramatically different ranking from any engagement-based feed. The top comments tend to be: (a) reformulations both sides recognize as fair, (b) genuinely novel framings, (c) personal stories that contextualize the abstract debate.

Empirical track record:
- Polis / vTaiwan (Audrey Tang, Taiwan 2014-): produced consensus statements on UberX, online liquor sales, fintech regulation that the legislature directly enacted.
- Twitter Community Notes: independent studies (Wojcik et al. 2022, Drolsbach & Pröllochs 2023) show bridging-ranked notes reduce misinformation spread by ~26% with high cross-ideological trust.

We use this same mechanism for the few async-debate use cases where in-person Agora is impossible.

### 3.2 Why we mostly don't use Agora Online

Because text-debate inherits 70% of the pathologies of social media even with bridging-rerank. The in-person Agora has structural advantages text can never reproduce: facial expression, vocal tone, the calm of physical presence, the impossibility of a pile-on. We use Agora Online sparingly, for genuinely necessary city-scale propositions, never for everyday discourse.

## 4. The deliberation calendar (cadence)

- Each polis hosts ~ 4 Agora sessions per week (one per neighborhood, on rotating evenings).
- Each citizen attends 8-16 Agora per year (Tier-3 metric #14: ≥ 35% weekly attendance rate of Agora-or-Symposium).
- 1 city-scale Agora Online per quarter on a high-stakes proposition (zoning, transit, school policy) — coordinated with the citizen assembly (`14`).
- Special Agora during crisis events (e.g., a contested election, a public health emergency, a major local controversy) on demand.

## 5. The Socratic method as default mode

Per Jiang's Plato lectures: the Socratic method is the operating procedure of Agora. The moderator gently encourages every contribution to take the form of (a) *steelmanning a previous speaker*, (b) *asking a clarifying question*, (c) *offering a counter-example*, or (d) *advancing a new consideration*. Pure assertion is allowed but discouraged in favor of reasoned form.

Newcomer Citizenship Reading (`04` §3.5) introduces this protocol. Every Agora opens with the moderator briefly reciting the Socratic norms (~ 30 seconds).

## 6. The "third-rail" zone — propositions we will not host

Some propositions are not productive for civic deliberation, either because they are settled (e.g., "the Holocaust happened") or because the framing is itself a harm vector (e.g., "is group X less than human"). The drafting panel filters these out via the **harm zone** rule: a proposition is third-rail if (a) one steelman would require dehumanizing a group of people, or (b) the proposition itself violates the cooperative's articles of incorporation (e.g., propositions that would re-introduce Tier-4-invariant violations).

We do not list a permanent set. The drafting panel uses judgment, supervised by the sortitioned algorithmic-review-board. We err on the side of allowing hard conversations — most of what people think is "settled" actually isn't (Dreyfus' point in *Mind over Machine*; Haidt's point about moral foundations across cultures). What we forbid is a narrow margin.

## 7. The citizen-assembly tie-in

Agora is for *inquiry*. The platform does **not** make policy decisions via Agora — that confuses deliberation with governance. Decisions about platform policy, about city-scale civic recommendations, and about cross-city coordination are made by **sortitioned citizen assemblies** (`14`) which may *commission* Agora sessions on the propositions they're considering. The Agora output (mind-change aggregate + post-Agora public summary written by the moderator) feeds into the assembly's deliberation but does not bind it.

## 8. The cross-city Agora (Polis Network, Year-2+)

When two cities are deliberating related propositions, the platform may host **cross-city Agora**: a 24-citizen panel where 12 are from City A and 12 are from City B, sometimes meeting in person at a midway location (modeled on the classical Greek *symmachia* assemblies), sometimes hybrid in-person-each-side. This is the deepest possible exposure to a different polis's thinking. Out of Year-1 scope; in the long-arc roadmap.

## 9. What is forbidden in this layer

- **No livestreaming, no recording, no transcription** (even by attendees on their own devices — phones in the basket per Symposium rule).
- **No paid moderators or paid attendees** — sortitioned and lightly compensated only in service-credit.
- **No proposition by a political party, campaign, or commercial entity** — propositions must originate from citizens.
- **No "celebrity moderator" or "celebrity attendee" model** — Agora has no stars.
- **No "live polling" overlays during debate** — instant feedback distorts deliberation.
- **No public "transcripts" or "highlights" published by the platform**. The only outputs are: aggregate mind-change %, the moderator's brief public summary (which preserves *eknomōs* — no attributable quotations).
- **No bots, no AI summaries of debates, no AI-generated proposition primers** (Invariant I15; primers are citizen-Elder authored).

## 10. Why this is the most consequential layer politically

If the platform's only contribution to civilization were *to restore the practice of multi-perspective in-person reasoned disagreement* it would be one of the most consequential pieces of civic infrastructure of this century. The collapse of this capacity is what Jiang, Putnam, Haidt, Sunstein, Pariser, Ovadya, and the U.S. Surgeon General all converge on. Every great democracy of the past relied on it; every collapsing one has lost it.

We are rebuilding it on purpose.

## 11. Cross-references

| Topic | Doc |
|---|---|
| Agora format spec (short) | `06_THE_FIVE_GATHERING_FORMATS.md` §3 |
| Matching algorithm composing Agora panels | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §5 |
| Sortitioned governance interface | `14_GOVERNANCE_AND_SORTITION.md` |
| Ovadya / Polis / Community Notes background | `21_SOURCES_AND_INFLUENCES.md` |
| Greek institution mapping | `19_GREEK_TRANSLATION_TABLE.md` |
| Anti-gaming protections that apply | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |

---

**Last updated:** 2026-05-24.
