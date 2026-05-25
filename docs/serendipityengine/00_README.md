# Serendipity Engine — PRD Set

**Author:** Chris Royse
**Status:** Living design document v0.1 — 2026-05-24
**Codename:** internal working name `Polis` (alternates: `Agora`, `Koinōnia`). Final name TBD.

> Every doc in this directory exists to answer one operational question:
> **How do we build a digital substrate whose only legitimate output is a real-world event — a meeting, a dinner, a debate, a run, a teach-in, a marriage — that makes a human's life measurably better, that cannot be gamed for status, that has no ads, that is provably impossible to scroll, and that pulls people out of their phones and into each other.**

If a feature does not cause people to meet, learn, grow, or fall in love **in physical reality**, it does not belong here.

---

## 0. Reading order

For someone new, read in this order:

| # | Doc | Read it for |
|---|-----|-------------|
| 01 | `01_VISION_AND_NORTH_STAR.md` | What we're building and the success predicate |
| 02 | `02_FIRST_PRINCIPLES.md` | The philosophical bedrock (Jiang, Dante, Granovetter, Schmidhuber, Oldenburg) |
| 03 | `03_NON_NEGOTIABLE_INVARIANTS.md` | The fences — what may never change |
| 04 | `04_USER_TYPOLOGY_AND_JOURNEY.md` | Who the user is and the lifecycle |
| 05 | `05_THE_CONSTELLATION_DDA_LAYER.md` | Personal fingerprint (the engine's substrate) |
| 06 | `06_THE_FIVE_GATHERING_FORMATS.md` | The five Greek-derived primitives for getting people together |
| 07 | `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` | The matching algorithm itself |
| 08 | `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md` | The mechanism design that prevents status farming |
| 09 | `09_THE_KNOWLEDGE_GROWTH_LAYER.md` | Paideia — continuous learning as a serendipity surface |
| 10 | `10_THE_LOVE_LAYER.md` | Love as the cosmic force the engine engineers around |
| 11 | `11_THE_DEBATE_LAYER_AGORA_2_0.md` | Agora 2.0 — Bridging-ranked dialectic |
| 12 | `12_THE_THIRD_PLACE_REVIVAL.md` | The IRL venue strategy |
| 13 | `13_BUSINESS_MODEL.md` | Revenue without ads, engagement, or status |
| 14 | `14_GOVERNANCE_AND_SORTITION.md` | How the platform itself avoids capture |
| 15 | `15_TECHNICAL_ARCHITECTURE.md` | Tech stack and integration with existing Royse stack |
| 16 | `16_METRICS_AND_FALSIFICATION.md` | What we measure (and explicitly do NOT measure) |
| 17 | `17_FAILURE_MODES_AND_BLINDSPOTS.md` | What could kill it; explicit threat model |
| 18 | `18_ROADMAP_FROM_ZERO_TO_POLIS.md` | Phase plan from single neighborhood to network |
| 19 | `19_GREEK_TRANSLATION_TABLE.md` | Explicit map: every Greek institution → system primitive |
| 20 | `20_JIANG_DOCTRINE_INTEGRATION.md` | How Jiang's framework + DDA + TCT + this system fit |
| 21 | `21_SOURCES_AND_INFLUENCES.md` | Bibliography of every idea that influenced the design |

---

## 1. The 60-second pitch (for anyone)

It's a social platform whose product is a calendar invite to a place in your city, an introduction to a person you would never have met, and a question that will keep you up tonight. There is no feed. There are no likes, follower counts, or status displays of any kind. You cannot become "famous" on it. You cannot game it. You cannot scroll it. It does almost nothing on its own surface — its surface is the city you live in, the people you meet there, and the conversations they spark in you.

The algorithm has one objective: maximize the number of **serendipitous events** in the life of every citizen. A serendipitous event is a moment you did not plan, did not expect, were not seeking — and were, after the fact, glad happened to you. Granovetter's weak-tie research, Oldenburg's third places, Aeschylus's dialectical theatre, and Plato's symposium all point to the same primitive: the high-bandwidth, multi-perspective, in-person encounter between people from different parts of the network. The Greeks built a civilization on it. Five things went wrong since: the polis got too big, status got economized, debate got commercialized, knowledge got siloed, and love got tinderized. This system reverses all five — not as five products, but as one substrate.

## 2. Why these docs exist as a set, not a deck

Per the Royse doctrine in `CLAUDE.md` and `docs/futurebuild/specs/NORTH-STAR.md`, every feature must trace to a number on the target sheet (`16_METRICS_AND_FALSIFICATION.md`) or preserve a doctrinal invariant (`03_NON_NEGOTIABLE_INVARIANTS.md`). The PRD set exists so that **no future maintainer can ship a feature that violates either** without first writing down which doc they are amending and why. This is the same evidence-weighted ordering that governs ContextGraph / ME-JEPA-Code.

## 3. Relationship to the rest of the Royse stack

- **ContextGraph** provides the 13-embedder TeleologicalFingerprint substrate. Reused directly.
- **ME-JEPA-Code** provides the binary-reality-prediction discipline (see `15_TECHNICAL_ARCHITECTURE.md`). Re-used as the architectural style for the matching predictor.
- **ClipCannon / Phoenix** provide multimodal constellation infrastructure. Reused for any video/voice features the platform eventually exposes (e.g., voice-only debate rooms inspired by Greek oral tradition).
- **TCT (Teleological Constellation Training)** is the explicit alignment philosophy: the platform's algorithm is aligned to a frozen centroid panel called *the Citizen Constellation* (see `09_05` in the relevant doc), not to engagement metrics.

This is not "social media powered by AI." It is a **TCT-aligned civic substrate** whose centroids are the operator-locked invariants of `03_NON_NEGOTIABLE_INVARIANTS.md`.

## 4. What's NOT in scope

- An ad network of any kind
- Profile pages browsable by other users (no "scrolling through people")
- Public follower / friend / engagement counts
- A discovery feed of any kind (no infinite scroll)
- Algorithmically-amplified content
- Influencer mechanics
- Crypto tokens, NFTs, points, badges, levels, or any leaderboard
- "Stories", "reels", "live", or any always-on broadcast surface
- Any feature whose KPI is time-on-app

If any of those appear in a future PR, it is a bug.

---

**Last updated:** 2026-05-24. Update this index when adding or reordering docs.
