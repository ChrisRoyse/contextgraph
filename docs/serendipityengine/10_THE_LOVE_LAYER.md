# The Love Layer

> *Amor che move il sole e l'altre stelle.* — Dante, *Paradiso* XXXIII
>
> *Love is the unifying force of the universe. The imagination is the animating force.* — Jiang, Great Books #4, #6, #9 (paraphrased)

The platform's deepest optimization target is **the generation of love at civilizational scale**. Not "matches" in the Tinder sense. Not "engagement." *Love* — in the Greek's four-fold sense: *agape* (selfless concern for another's flourishing), *philia* (friendship), *eros* (romantic / sexual), *storgē* (familial / parental affection). All four.

Dante's *Comedy* is the architectural template. He moves from the cold, frozen, status-obsessed lowest hell (the love of self, the betrayal of trust — `Inferno`) through the purgation of love rightly ordered (`Purgatorio`) to the Empyrean where the Trinity itself is love (`Paradiso` XXXIII). The platform's whole arc — from a Newcomer's first dinner to a city of citizens bound by deep ties — is a translation of this arc into the substrate of contemporary urban life.

## 1. The four loves the platform engineers for

Per C.S. Lewis (*The Four Loves*, 1960) extending the Greek typology:

| Love | Greek | The platform produces it via |
|---|---|---|
| **Agape** | ἀγάπη | Sortitioned welcoming role; Newcomer outreach; Aleatory format; *charitas* in convener compensation (service-credit, not money) |
| **Philia** | φιλία | The Symposium and Chevruta primarily; Theater and Gymnasium secondarily |
| **Eros** | ἔρως | The Symposium long-term (Plato's actual *Symposium*); explicit opt-in single-status flag at intake; never targeted advertising of "single women near you" |
| **Storgē** | στοργή | Year-2+ family formats; cross-generational ties (Tier-3 #17); Festival ritual (a culture's collective parenting of its young) |

The platform is **not a dating app** (`01` §5). It is a *koinōnia* engine that, among other things, sometimes produces romance. Tier-3 metric #15 says: ≥ 1% of single citizens form a sustained (≥ 6 months) romantic match per year. This is modest. It is also true and durable.

## 2. The romantic-match path — what is engineered

### 2.1 Stated single-status as an intake field

Citizens at intake (`04` §3.2) optionally indicate one of: *partnered, not looking; single, not looking; single, open to romantic match if it arises*. This is a private field used only by the matcher; never displayed. A citizen may change it at any time.

### 2.2 The romance-curious Symposium

For citizens flagged "single, open to romantic match if it arises," the matcher periodically composes Symposia where ~30% of attendees share the flag (across compatible M13 constraints — gender preference, age band, geography). The conversation isn't framed as "romance dinner"; it's a regular Symposium. The matching prior is a private nudge, not a public label.

Why this works:

- **No swiping, no profile-browsing.** You meet people across a table, in a real conversation, with no pre-rated photo. (The opposite of every dating app since 2012.)
- **Romance emerges from companionship,** as it has in every healthy human society. The platform creates the conditions for it; it does not "match for romance."
- **Failure is silent.** If two people at a Symposium aren't drawn to each other romantically, they may still leave with a great friendship or a new idea. The dinner was not "for" romance.

This is the digital analogue of how Athenians, Renaissance Italians, 19th-century Parisians, and 1950s Greenwich-Village New Yorkers actually formed romantic ties — through *koinōnia*-level mutual exposure, not through 1-on-1 transactional matching.

### 2.3 The sustained-match definition

A *sustained romantic match* (Tier-3 metric #15) is: two single citizens who, after meeting at a gathering, self-attest 6 months later that they are partnered. The 6-month delay defeats Tinder-style one-night metrics; what we measure is the formation of *durable* love.

We deliberately do not measure marriage, cohabitation, or sexual exclusivity — those are private choices outside our jurisdiction. We measure only the citizens' own attestation that what began on the platform turned into love that lasted.

## 3. The philia path — friendship engineering at scale

Friendship is the most undersupplied form of love in modernity. Putnam's *Bowling Alone* documents the 60% collapse in close friendships from 1975-2010 in the U.S. The Surgeon General's 2023 loneliness advisory cites this directly.

The platform's philia path is the workhorse of the love layer. It runs through:

- **Repeated low-stakes co-attendance** (Symposium + Gymnasium): per Aristotle's *Nicomachean Ethics* VIII, *philia* requires shared activity over time. The platform supplies the time and the activity.
- **Chevruta pairings** (`09` §4.2): the deepest friendship-formation mechanism the platform offers. 8-12 weeks of reading together → typically a multi-year friendship.
- **Convener-attendee ties**: a citizen who has hosted you at a Symposium is statistically twice as likely (calibration-pending; Phase 0 target) to become a sustained friend.
- **Festival co-experiences**: large shared rituals bind weak ties into durable friendships across decades.

Tier-1 metric #2 (≥ 3 new strong ties / citizen / year) is mostly delivered by this path.

## 4. The agape path — civic love for those you don't know

This is the love Dante puts at the top of *Paradiso*: love that flows outward beyond your circle, into the body politic and the world.

The platform's agape path:

- **The Welcoming Citizen role** (`04` §3.2). When you sit on a 15-minute video call with a stranger and ask them what they care about, you are practicing agape. The platform makes this practice a normalized, periodic, lightly-honored act.
- **The Newcomer Symposium**, where Established citizens give an evening to people they will probably never become close friends with, just to receive them.
- **The Civic value coefficient δ** in the matching utility (`07` §2): the algorithm explicitly rewards events that benefit third parties.
- **Festivals** (`06` §6): an entire city giving itself a day of shared joy is collective agape.
- **The Polis Network** (`18` §6): cross-city citizen exchanges, sister-city visits, knowledge sharing. Agape across geographies.

Eudaimonia question 4 ("Did you do something this quarter that helped your neighborhood and benefited no one in your household?") directly measures this.

## 5. The storgē path — Year-2+ scope

Family-form love is the longest-tail and most consequential. The platform's storgē work — bringing children into Symposia, cross-generational chevruta, intergenerational mentoring — is out of Year-1 scope but explicitly in the long-arc roadmap (`18`). When we do build it, we will build it carefully and with extra child-safety review.

We do not build any feature that mediates between parents and children, between spouses, or inside the household. Those are sacred private spheres. The platform serves the polis-level, not the family-level.

## 6. The anti-pattern catalog (what we must never become)

| Anti-pattern | How we prevent it |
|---|---|
| Tinder / Hinge swipe-mediated romance | No profile-browsing, no swiping, no algorithmic 1-on-1 introduction; romance emerges only at in-person gatherings (Invariant I11, I4) |
| Bumble BFF "friendship app" | No "find a friend" UI; friendship emerges as a by-product of gatherings (`06`, `09`) |
| Match.com paid-priority for romance | Invariant I16 — no pay-to-play in matching, no "Premium" tier that biases romantic surfacing |
| AI-ghostwritten dating openers | Invariant I15 — no AI-generated messages on behalf of users |
| Sexual harassment / predatory behavior at gatherings | Strict code of conduct enforced at venue; reporting triggers sortitioned review; 90-day pause for first violation, lifetime ban for second; convener has authority to remove attendee mid-event |
| Gender or race quotas applied paternalistically | Sortitioned review board oversees matching parity reports; if disparity arises, board commissions investigation; no top-down quotas without citizen-assembly approval |
| "Romantic compatibility score" displayed to users | Forbidden surface (`08` §2) |
| Public "couple of the year" advertising | Forbidden ad-style content per Invariants I5 and I17 |
| Therapy-substitute promises | We are not therapy. The intake interview (`04` §3.2) and venue norms make this clear; serious mental-health crises are referred to professional help with no platform mediation |

## 7. The intersectional friction (deliberate)

The platform deliberately makes romance **slower** and **higher-friction** than Tinder. This is the design. The body of evidence (Twenge *iGen*, Haidt *Anxious Generation*, Wallace-Wells on the dating-apps mental-health collapse) is clear: fast-friction app-mediated romance has produced an unprecedented loneliness epidemic, not solved it. Slower romance — meeting at a dinner, then again at a debate, then again at a Theater, *then* getting coffee — replicates what worked for the entire history of human pair-bonding.

A citizen who joins primarily to find love will find some impatient. Most will, after a few weeks, find themselves accidentally with three new friends, a book they're reading with someone they enjoy, and (sometimes) a person they'd like to date. Those who only want fast romantic transaction will leave — and that is correct sorting.

## 8. The data discipline for romantic data

Romantic-status data is the most sensitive on-platform. Specific extra protections:

- Single-status flag is stored encrypted with a per-citizen key derived from PoP (per `05` §6).
- The matcher reads this flag only ephemerally, at match-computation time, in-memory.
- Sustained-match attestations (per §2.3) are stored only as a binary "started match on platform: yes/no" with no partner identity attached unless both citizens explicitly opt-in.
- Romantic status is **never** an input to any aggregate analytic publication.
- Withdrawal of consent purges the flag in < 1 hour.

## 9. The deepest measurement

Beyond Tier-3 #15, the truer measurement is in Eudaimonia question 5: *"Do you love someone, and do they know it?"* — taken in the broad sense (a parent, a friend, a partner, a fellow citizen). Across a polis-population, the median answer rising from 3 to 4.2 on a 5-Likert across 24 months is what success at this layer actually looks like.

When Dante looks back at Beatrice in *Paradiso* and sees that the love that began on a Florentine street has become the architecture of the cosmos, he is describing the long-arc outcome the platform aspires to make ordinary in every city.

## 10. Cross-references

| Topic | Doc |
|---|---|
| Symposium (the primary love-forming format) | `06_THE_FIVE_GATHERING_FORMATS.md` §2 |
| Chevruta (the strongest philia-forming practice) | `09_THE_KNOWLEDGE_GROWTH_LAYER.md` §4.2 |
| Matching algorithm (love is the optimization target) | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §2 |
| Citizen lifecycle | `04_USER_TYPOLOGY_AND_JOURNEY.md` |
| Anti-gaming, including against romantic gaming | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` |
| Eudaimonia index where love is measured | `01_VISION_AND_NORTH_STAR.md` §3 |
| Jiang on Dante and the Great Books on love | `20_JIANG_DOCTRINE_INTEGRATION.md` |

---

**Last updated:** 2026-05-24.
