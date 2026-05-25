# First Principles

This document encodes the philosophical bedrock the entire system rests on. Every later design decision is a deduction from these axioms. If you disagree with these, you will disagree with the design — and that is the right place to argue, not in feature meetings.

## 1. The core thesis (Jiang × Dante × Granovetter × Schmidhuber × Oldenburg)

**Humans are fundamentally religious-imaginative-social beings whose flourishing requires three things mass-society has systematically destroyed: in-person community, multi-perspective debate, and love-as-cosmic-orientation. The internet, as currently built, accelerates the destruction. A different internet — engineered around those three needs as architectural constraints — could reverse it.**

Five thinkers, one synthesis:

### 1.1 Jiang on what makes a society great

From the Greek-civilization lectures (especially `4ehovUNrSrw` and `WoTOxE4DTFA`), Jiang argues Greek civilization — the most creative in human history — arose because three forces combined after the Bronze Age collapse:

- **The polis** — small, decentralized city-states where every citizen was required to speak in war councils, which forced every citizen to develop rhetoric, which forced literacy and education for all.
- **The alphabet** — a simple writing system that anyone could learn, which broke the elite's monopoly on knowledge.
- **Homer** — an *oral* poetic tradition that planted empathy, imagination, and a theory of the human heart into every Greek child's mind.

The polis was a face-to-face community. The alphabet was an open knowledge substrate. Homer was the shared myth. All three preconditions are recreatable digitally — but only if you architect for them deliberately. (See `19_GREEK_TRANSLATION_TABLE.md` for the explicit mapping.)

### 1.2 Dante on love as the cosmic force

From the *Divine Comedy* lectures (especially `6M1Z_V3WgOk` and `aS-NfPSPMu8`), Jiang argues:

> **Love is the unifying force of the universe. The imagination is the animating force.**

Dante explicitly inverts Virgil's *Aeneid* — which preached piety, duty, and empire over love — and reasserts the *Iliad*'s ending: it is Priam's love for Hector, willing to kneel and kiss the hand of his son's murderer, that breaks Achilles's rage and changes the destiny of the war. **Love is engineering**, not sentiment: it is the only force capable of dissolving the OCD-driven, status-driven, anxiety-driven machinery of late capitalism that Weber (*Protestant Ethic*) and Marx and Durkheim all diagnosed.

Therefore: love is not a feature of this system. **It is the optimization target.** (Full treatment: `10_THE_LOVE_LAYER.md`.)

### 1.3 Granovetter on weak ties

The classic 1973 paper: novel information, opportunity, and serendipitous discovery all flow disproportionately through **weak ties** — the acquaintance, the friend-of-friend, the person you met once at a dinner. Strong ties carry redundant information (your close friends know what you know); weak ties bridge the structural holes in your network. Most jobs, most marriages, most ideas you'll ever have come through weak ties.

Modern social media nominally creates weak ties (you "follow" 500 people), but the ties are **passive** and **broadcast**, not interactive and dialogic. The serendipity engine has to produce *active, in-person, dialogic weak ties* — the kind that produce a coffee, not a like. (Full treatment: `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §4.)

### 1.4 Schmidhuber on compression progress as the formal definition of "interesting"

Subjective interestingness (curiosity, beauty, surprise, fun) = the *first derivative* of how much the observer can compress an experience. Not Shannon-novelty (random noise is uninteresting). Not pure familiarity (already-known patterns are boring). The sweet spot: experiences that *teach* the observer to compress better, that yield *compression progress*.

This is the formal grounding for what "serendipity" means in our algorithm. A serendipitous event is one that maximizes the user's expected compression progress over a one-week horizon, under the constraint that the event happens in physical reality and involves new people. (Formal definition: `07_THE_SERENDIPITY_ENGINE_ALGORITHM.md` §2.)

### 1.5 Oldenburg on third places

The 1989 *The Great Good Place*: human flourishing requires not just home (1st place) and work (2nd place) but a **third place** — informal, voluntary, repeated-encounter venues where weak ties become strong ties through accumulation of low-stakes interactions. Cafés, bowling alleys, churches, pubs, barber shops.

US Surgeon General's 2023 advisory on loneliness explicitly cites the decline of third places. Run clubs, sober social clubs, and Timeleft dinner formats are bottom-up reactions to the collapse. **Our system is the top-down infrastructure to make every city a third-place machine.** (Full treatment: `12_THE_THIRD_PLACE_REVIVAL.md`.)

## 2. Six secondary axioms (deduced from the core)

### 2.1 Status seeking is the cancer

(Jiang on Achilles vs. Priam; Jiang on meritocracy as Yale's Hunger Games; Bauchaud's anarchist critique of vanguardism.)

The single greatest design failure of every social platform 2007–present is letting status leak in as a side channel. Likes were a bug. Follower counts were a bug. The blue checkmark was a bug. Each makes the platform a status game; status games are zero-sum; zero-sum games produce the rat-utopia collapse Jiang diagnoses in `NpNCQ_gNqdE`. The only safe architectural choice is to make status numerically impossible to display, accumulate, or transfer. (Full treatment: `08_ANTI_GAMING_AND_STATUS_FORBIDDANCE.md`.)

### 2.2 The system must pull people OFF the device, not into it

(Earned-attention reading: Ned Karlovich, Dull, CalmLoop; Jiang on schools as brainwashing through the absence of parents.)

The KPI of attention-economy products is time-on-app. Ours is the **inverse**: time-off-app spent doing something the app caused. The platform's interface should be as silent, terse, and infrequent as a doctor's appointment reminder. One push per few days. One screen per session. Then go.

### 2.3 No advertisers, ever, full stop

(Bouchaud, Pariser, Sunstein, Allcott, Ranking-for-Engagement.)

An ad-funded model forces engagement-maximization which forces filter bubbles which forces status games which destroys eudaimonia. There is no version of "tasteful ads" that does not eventually become this. The business model (`13_BUSINESS_MODEL.md`) is venue-revenue-share, civic subscription, and patronage — never ads.

### 2.4 The algorithm must be auditable and ungameable by design

(Jiang on bureaucracy; Vitalik on PoP; Bridging-based ranking; sortition.)

If the algorithm is a black box, the people who write it accumulate power; if it can be gamed, status-seekers will hollow it. Defense: the matching algorithm publishes its centroid set (TCT-style), its weights, and its bridging-rerank deltas. Per-user predictions are not published. The system is **transparent in mechanism, private in instance** — like a constitution.

### 2.5 Knowledge growth and love generation are not separate features

(Jiang on the Greek symposium; Plato's *Symposium*; Dante on love-as-imagination.)

The Greeks did not have "learning apps" and "dating apps." They had one place — the symposium — where philosophical debate, friendship-formation, and erotic discovery all happened in the same evening. The whole design instinct of unbundling these is a 21st-century mistake. We re-bundle them: the same gathering format can produce a new book read, a new friendship, and a new lover.

### 2.6 Sortition is the only ungameable governance mechanism at scale

(Belgian G1000; Ostbelgien Model; classical Athenian klerōtērion; Jiang on the failure of meritocracy.)

If the platform is governed by election, voting, popularity, or merit, capture is inevitable. The only mechanism that has empirically resisted capture across millennia is *sortition* — selection by lot from the eligible population, with rotation. The platform's governance, its content moderation, its algorithmic-weights review board, and its territorial expansion decisions are all sortitioned. (Full treatment: `14_GOVERNANCE_AND_SORTITION.md`.)

## 3. The Aristotle vs. Plato split — and why we side with Dante

Jiang's lectures on Aristotle (`WoTOxE4DTFA`) and Kant/Hegel (`_3c3FjS57x4`) lay out the deepest fork in Western thought: empiricism vs. rationalism, becoming vs. being, telos vs. form. We do not side with either. We side with **Dante**, who reconciled them in the *Divine Comedy* by making love (Aristotelian *agape* + Platonic *eros*) the unifying force that contains both motion and form, both becoming and being.

Concretely: our matching engine is **empirical** (it learns from feedback) but its objective function is **idealist** (the frozen Citizen Constellation, not a moving engagement metric). It is Aristotle's *telos* applied to Plato's *form of the good*, with Dante's love as the resolution. This is the same TCT philosophy described in `docs/PAPER.md` — the same trick that makes ME-JEPA work.

## 4. What modernism destroyed (and what we are restoring)

Jiang's lecture `ra2v9HkaHz8` ("How Modernism Ruined Everything") makes the case that the modernist cult of the self — Freud's individualism, Joyce's stream-of-consciousness, Picasso's fractured ego — was deliberately spread (with documented CIA support) as a Cold War weapon against communism, and ended up destroying not Soviet collectivism but our own capacity for community. We do not, of course, re-impose collectivism. We restore the **third option** Jiang and Bakunin both point at: a society of free individuals whose freedom is mutually reinforcing because they are continuously in real-world contact with each other.

That third option, in classical Greek, was *koinōnia* — fellowship under shared inquiry. That is the operating word for what this system creates. The codename `Polis` is the political wrapper; the soul of the thing is *koinōnia*.

## 5. The blast radius of these principles

These axioms have ferocious downstream consequences. They forbid features that every founder will want to add ("just a small follower count to drive growth"); they forbid revenue mechanisms most investors will demand; they force a slow city-by-city rollout instead of a viral one; they require sortitioned governance that most founders will not want to cede control to. **Accepting these costs is the price of admission to building a system that actually works.**

The Tier-4 invariants in `03_NON_NEGOTIABLE_INVARIANTS.md` are the contractual expression of this acceptance.

---

**Last updated:** 2026-05-24. Update only when the philosophical foundations themselves change.
