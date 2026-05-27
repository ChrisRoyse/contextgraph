# 13 — Consumer Social Launch (Polis)

> Polis is fundamentally different from ContextGraph/Synapse. It is a **consumer social platform** where the unit of value is a **physical / synchronous gathering** in a **dense friend cluster**. Network effects dominate. This doc adapts the BeReal / Bluesky cold-start playbooks, layers in Hormozi's giveaway mechanics + Steve's tier model, and produces a Polis-specific cold-start plan.

---

## §1. The Polis problem statement

Polis only works if you go. Going is hard if your friends aren't there. So the question is: how do we get **dense friend clusters** to attend a Polis event together, within the same neighborhood / city / campus, often enough for the habit to form, before anyone churns?

This is the **cold-start network effect problem**. The classic solutions: campus, geography, ambassador, ritual.

---

## §2. BeReal pattern — the canonical reference

BeReal: 0 to 41M MAU in 2.5 years. Pattern reconstructed:

| Phase | What they did | What we copy |
|---|---|---|
| Pre-launch (Dec 2019 - Jun 2021) | Slow burn, French college students, organic | Single-city seed phase |
| Featured on App Store (Mar 2021) | 30M raise from a16z + Accel based on traction | We do not need this — we plan for organic |
| Campus ambassador program (US, ex-Bumble lead) | $30/successful referral, $50/install+attendance | Direct copy: `13.4` |
| Organic TikTok (Jun 2021 — Jul 2022) | 660M views on #bereal hashtag; David Aliages hired after his viral demo | Direct copy: `13.5` |
| App goes mainstream (Apr 2022 → Jul 2022) | 2.9M → 5M DAU; #1 in App Store | The natural curve once the lower stages compound |

Why BeReal worked (per Sacra / Platformer analysis):
- **Creative constraint** — single push notification per day; binary decision to participate.
- **Friend graph required for value** — posting to zero followers is pointless.
- **No algorithmic feed** — no engagement-farming pressure.
- **Limited daily action** — no "post 10 times a day" optimization.
- **Show-the-real moment** — anti-curation.

Polis carries similar structural advantages (anti-engagement-farming by design, value requires friends, gathering is the action), so the BeReal playbook is the natural template.

---

## §3. The Bluesky pattern — federation as network effect

Bluesky: 0 → 14.5M users in ~2 years. 1.5M new signups in the week after opening to public. Key pattern:

- **Open the gates at the right moment** — Bluesky's invite gates were lifted exactly when X had a credibility crisis.
- **Algorithmic plurality, not absence** — users pick their own algorithms; not a hard "no algorithm" rule.
- **Community moderation** — moderation as a feature, not a centralized policy.

Polis copies the **invite-driven density** part of the playbook (the gates open one cluster at a time) but rejects the **public-firehose** part (Polis is in-person; it's not designed to be a Twitter replacement).

---

## §4. The Polis 90-day cold-start plan

### 4.1 Phase 1 — Single-city seed (weeks 0-4)

Pick **one city** as the seed. Cambridge / Somerville (MIT/Harvard density) is the leading candidate; alternatives: Austin (campus density + intellectual culture), Berkeley (Bay Area but indie), NYC (LES / Williamsburg cluster).

Why one city: BeReal lesson — *campus density* trumps *geographic spread*. A 50-person friend cluster in one city beats a 5,000-person diffuse audience across 100 cities.

Activities:
- Operator + 2 ambassadors host 3 trial events / week (salons, debates, third-place revivals)
- Attendance cap: 12 per event (intimacy > scale; the dinner-party threshold)
- Every attendee invited to bring ≥1 friend to next event
- Light branded swag (a postcard with the Polis logo and "Sarah brought you")

Goal: 50 unique attendees, 4 events with ≥2 friends overlap, 65% show rate.

### 4.2 Phase 2 — Cluster densification (weeks 4-8)

Same single city. Friend density compounds.

Activities:
- 5 events / week (still capped at 12 attendees)
- Two-prize giveaway pattern (Hormozi/Skool): each event has a host-prize draw + a referrer-prize draw
- First campus ambassadors paid out per referral / install / attendance metrics (`§3.5` below)
- Founder thread (operator-written) on Polis seeded city; documented as social proof

Goal: 200 unique attendees, friend cluster density (avg connections per attendee within Polis): ≥3.

### 4.3 Phase 3 — Geographic expansion (weeks 8-16)

Open 3 additional cities based on natural demand signals (signups from those cities, intellectual creator presence, campus density).

Activities:
- Ambassador program scales (3 ambassadors per new city)
- TikTok organic content production scales (3-5 / day from the operator's Polis-persona account)
- Operator-hosted launch event in each new city (operator attends in person)
- First press: targeted to intellectual / culture writers (not tech press)

Goal: 1,500 unique attendees across 4 cities, attendance rate ≥65%, NPS ≥ 50.

### 4.4 Phase 4 — Scale (weeks 16+)

Open 10 cities. Paid Meta + TikTok in proven cities (after organic CAC reaches LTV:CAC floor). Polis Curator launches.

---

## §5. Campus ambassador program (BeReal pattern)

### 5.1 The economics

- **$30 per successful referral** (a friend who signs up and is verified)
- **$50 per install + attended-event** (installed app AND attended ≥1 event)
- **Bonus tiers for ambassadors who cross 25+, 50+, 100+ activations** (e.g., $500 lump sum at 50)
- **Ambassador must be a student** at the seed-city institution
- **Cap of 5 ambassadors per institution** in seed phase (small density beats sprawl)

### 5.2 Ambassador recruitment

- Operator + first ambassadors recruit from their own networks
- Drip-out application through Substack / X-organic / campus newsletter cross-posting
- Vet for: intellectual curiosity (essay question), local friend density (how many people they regularly hang out with in person), genuine interest in the product

### 5.3 Ambassador playbook

A 30-page playbook (Hormozi-style "literally tell them what to do"):

- How to host your first event (templates: salon, debate, third-place revival, mini-knowledge-growth session)
- Three icebreaker scripts
- How to invite friends (the social etiquette of "I'm doing a thing, want to come")
- Post-event ritual (thank-you, photo, ask for one feedback)
- Tracking your referrals and payouts

The playbook lives in the Polis content library; ambassadors get an exclusive Discord with operator + the ambassador team.

### 5.4 Ambassador measurement

| Metric | Target |
|---|---|
| Average activations per ambassador per month | ≥ 5 |
| Average attendance per ambassador's referred attendees | ≥ 3 events in first 90 days |
| Ambassador retention (still active at 6 months) | ≥ 70% |
| Ambassador NPS | ≥ 60 |

Ambassadors who repeatedly fail the activation target are coached; those who consistently miss are gracefully off-boarded.

---

## §6. TikTok-first content strategy

Per the BeReal pattern (660M #bereal views; 100K+ first official TikTok; 9 viral videos over 2 months):

### 6.1 Hormozi-volume here

Hormozi's "post 100 times a day" applies on **TikTok / IG Reels / YT Shorts**: the platforms reward volume, dedupe per-viewer. The Polis-persona account posts **3-5 organic vertical clips per day**. Channel rate cap (`08_DISTRIBUTION_CHANNELS.md` channel cards) keeps this within natural human-shaped cadence.

### 6.2 Content archetypes

For Polis on TikTok / Reels / Shorts:

| Archetype | Example | Why it works |
|---|---|---|
| "POV: you walk into a Polis salon" | Short cinematic of an actual event (consented attendees) | Shows-not-tells; aspirational |
| "I went to my first Polis event — here's what happened" | First-person attendee testimony (real attendees, with explicit consent) | Trust signal; word-of-mouth simulation |
| "Three things people get wrong about meeting smart strangers" | Operator's voice; brand-pillar content | Intellectual signaling |
| "How to host an intellectual dinner" | Tutorial; lead-magnet adjacent | High share-ability; matches lead magnet |
| "Anti-engagement-farming" hot takes | Operator's voice; positioning vs. existing social | Differentiation |
| "What we talked about at the salon last night" | Brief intellectual content drawn from real events | Substantive; intellectual proof |
| Quotes from attendees | Designed for screenshot + share | Social proof |
| "Behind-the-scenes" of event planning | Light, founder-style | Build-in-public |

### 6.3 The David Aliages move

BeReal's David Aliages got 2M views on a TikTok showcasing the app organically; BeReal hired him. The fleet keeps a watchlist: which non-affiliated creators are organically posting about Polis (positively)?

Signal Listener (S2) flags them. The operator (not the agent) reaches out to the most resonant. The pattern: **identify the natural advocates, bring them in**. This is Hormozi's "the customer becomes the lead-getter, sometimes the employee."

---

## §7. The Hormozi two-prize giveaway (Skool pattern) for events

Hormozi: *"Give Away Two Prizes For Twice The Leads. If you give away one prize, that's fine. But if you give away two grand prizes, you can get twice the leads (or more). Just tell everyone that if someone they refer wins the grand prize, they win one too."*

Polis event mechanic:

- Each Polis event has a **host-prize draw** (1 attendee wins a small high-quality item — a hosted dinner, an annotated book, a signed print).
- Each Polis event has a **referrer-prize draw**: if a friend you brought wins the host-prize, you win an equivalent prize too.

Both draws are public, both are real, both are small. The economic cost is low; the referral motivation is high.

Hormozi's three giveaway rules apply:
- **Scarcity** — 1 winner per event (bounded by event)
- **Urgency** — the draw happens at the event itself
- **Grand prizes** — small but desirable, not gimmicky

---

## §8. Substack intellectual creator partnerships

Polis is fundamentally an intellectual product. The natural T2 partners are intellectual creators on Substack:

### 8.1 Tier-1 partners (top of operator's wishlist)

- Tyler Cowen (Marginal Revolution)
- Patrick Collison (his curated lists)
- Sam Harris (his audience overlaps heavily)
- Russ Roberts (EconTalk + writings)
- Marc Andreessen (where ideologically aligned)
- Ben Thompson (Stratechery, for the platform-tech overlap)

### 8.2 Tier-2 partners

- Erik Hoel, Astral Codex Ten, Robin Hanson, Tyler Cowen-adjacent econ-blog cluster
- Cultural critics: a curated set of ~30 writers
- Niche intellectual: book-club founders, philosophy podcasters

### 8.3 Engagement format

Per `08_DISTRIBUTION_CHANNELS.md`, partnership tactics:
- **Interview swap** — operator hosts creator on Polis "podcast"; creator hosts operator
- **Joint event** — operator-funded Polis event where the creator hosts; creator promotes to their list
- **Newsletter cross-promotion** — creator writes about Polis; operator's newsletter writes about creator
- **Referral revenue share** — for paid Polis tiers, creators earn 20-30% revenue share on referred subs

These are operator-driven (T3 / high-T2). The Mapper proposes paths to the creators; operator executes.

---

## §9. The Substack lead magnet pattern

Polis's lead magnet (per `11_PRODUCT_PLAYBOOKS.md §3.2`): **"Third Place Revival Guide"** — a free practical guide on hosting intellectual gatherings.

Distribution:
- Substack post under the operator's personal account, free
- Cross-posted to operator's blog
- Promoted to Tier-2 intellectual creators ("I wrote this; would you read and share?")
- Lead-magnet conversion overlay: download requires email; email flow converts at 18-25% to Polis signup over 21 days

This is Hormozi's lead-magnet pattern (`07_CONTENT_FACTORY.md`): give massive value free, capture the email, convert over time. The guide is genuinely useful even if the reader never joins Polis. That's the point — high price-to-value discrepancy.

---

## §10. Hard rules for Polis specifically

Polis carries reputation risks the dev-tool products don't:

- **No fake attendees.** Synthetic accounts in events are an instant-kill brand event.
- **No paid first-attendee.** Polis attendees come because they want to, not because they're paid.
- **No undisclosed paid event hosts.** If a host is compensated, the post-event communication says so.
- **No demographic targeting beyond legal / ethical norms.** Polis cities target geographically; demographically inclusive.
- **Photo / quote consent at every event.** Default opt-out, explicit opt-in for promotion.
- **No event recording without explicit attendee consent.** Polis Plus / Curator tiers can record; default does not.
- **Anti-engagement-farming enforcement on operator's content.** Operator's Polis-persona account is a strict brand: no engagement-bait, no thirst-bait, no "you won't believe what happened."

---

## §11. The Goodhart watchdog for Polis

The standard Goodhart Auditor (`05_AGENT_FLEET_DESIGN.md §6.2`) is **especially relevant for Polis** because the unit of measurement (attended events) is more gameable than developer integrations.

Reality checks Polis-specific:
- **NPS at events** — anonymous, measured at end of event
- **Attendee return rate** — % who attend ≥2 events in 90 days
- **Friend graph density per attendee** — average new friend-edges per attendee per 30 days
- **Photo-consent rate** — proxy for organic attendee engagement (people don't consent if they didn't enjoy)

If `attended_events` rises but NPS or return-rate falls, the Auditor trips `goodhart_signal_detected` and Polis pauses scaling.

---

## §12. Operator personal cost commitment

Polis cannot be operated by agents alone in the seed phase. The operator must **personally attend** seed-city events:
- 3+ events per week in the seed city for weeks 0-8
- 1+ event per week per new expansion city in weeks 8-16

This is the Vercel / Cursor / Linear "founder shows up" pattern, applied to a consumer social product. The agent fleet handles scheduling, comms, content, follow-up, ambassador ops — but the operator's physical presence is non-delegable in early phases.

If the operator cannot commit to this, Polis launch is paused. (Hard doctrine.)

---

## §13. Cross-references

- Polis goal-spec template: `11_PRODUCT_PLAYBOOKS.md §3.3`
- Audience tier model (Polis-specific weights): `03_TIER_MODEL_AUDIENCE.md §9`
- Viral mechanics for Polis: `09_VIRAL_LOOPS_AND_REFERRALS.md §3.3`
- BeReal, Bluesky teardowns: `18_CASE_STUDIES.md`
- Compliance (event-attendee data): `14_COMPLIANCE_AND_ETHICS.md §6`
- 90-day operational timeline: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Goodhart Auditor: `20_RISK_AND_FAILURE_MODES.md`
