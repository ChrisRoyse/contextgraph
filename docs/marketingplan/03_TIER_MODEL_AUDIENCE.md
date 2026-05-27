# 03 — Audience Tier Model

> Steve Hutto's framing from the 2026-05-26 conversation, formalized. Marketing is a tiered influence problem, not a broadcast problem. Each tier has its own physics, its own success criteria, its own agent class, and its own tools.

---

## §1. The three tiers

| Tier | Steve's name | Population | Reachability | Conversion cost per win | Primary channels |
|---|---|---|---|---|---|
| **T1 — Mass** | "Low-hanging fruit" / "people in the sh*t tank" | Millions, unsorted | Cheap, scalable, low signal | Low ($1–$50) | Open social, paid ads, SEO, AI search, content-led discovery |
| **T2 — Engaged** | "People who got out of the tank and got smarter" | Tens of thousands | Medium, requires authentic engagement | Medium ($50–$500) | Communities (Discord/Reddit), newsletters, podcasts, in-person events, GitHub, HN |
| **T3 — Elite** | "Political layer / inner circles" | Hundreds | Closed-door, requires warm introduction | High ($500–$50k) | Personal networks, board overlaps, dinners, trust-only DM channels, podcasts the elite themselves consume |

Each tier feeds the next. Movement across tiers is rare without a deliberate funnel that respects the etiquette of each.

---

## §2. Why this matters for an agent fleet

**The agent fleet's tactics must change per tier.** A computer-use agent that posts on r/MachineLearning at high frequency is correctly calibrated for Tier 1; the same agent applied to Tier 3 ("a Sequoia partner you want to know") is brand-destroying within hours. The fleet has three distinct sub-fleets, one per tier:

| Sub-fleet | Throughput | Autonomy default | Approval model | Brand sensitivity |
|---|---|---|---|---|
| Mass Agents (T1) | High (hundreds of actions per day per channel) | `LOG` to `AUTO` | Sampled audit | Low — generic copy is fine |
| Engaged Agents (T2) | Medium (tens of contextual actions per day) | `LOG` with brand-voice gate | Approve thread responses; auto-publish drafts that pass evaluator | Medium — must read room |
| Elite Agents (T3) | Low (one or two warm intros per week) | `REQUIRE_APPROVAL` always | Operator approves every external action | Maximum — every word reviewed |

Mass tactics applied to Elite contacts are how the brand dies. Elite tactics applied to Mass channels are how the budget dies. Knowing where a target sits **before** acting is the single most under-built capability in 2026's autonomous marketing stack.

---

## §3. The Sorter Agent

A specialized agent class — the **Audience Sorter** — runs whenever a contact enters the system (form fill, referral, mention, manual operator-added). It scores the contact across signals to assign a tier:

| Signal | Tier-1 weight | Tier-2 weight | Tier-3 weight |
|---|---|---|---|
| Public follower count > 100k | — | — | +2 |
| Employer is a frontier AI lab (named list) | — | +1 | +3 |
| Author of widely-cited paper/post (in agent's domain) | — | +1 | +2 |
| Member of an invite-only Slack/Discord (named list) | — | +2 | +2 |
| Recent appearance on a Tier-1 podcast (named list) | — | +1 | +3 |
| Board membership at an exit-stage portfolio (LinkedIn) | — | — | +3 |
| Verified email at custom domain matching exec roster | — | +1 | +2 |
| Has personal blog with > 5 deep technical posts | — | +2 | — |
| Repeated GitHub commits to relevant projects | — | +2 | — |
| Mention of operator's product in their content | — | +3 | +1 |
| Default contact | +1 | — | — |

Score ≥ 5 → Tier 3. Score ≥ 2 → Tier 2. Else Tier 1.

Tier assignment is reviewable; the operator can override (e.g., "this person is Tier 3 but their public profile is sparse"). Overrides update the scoring rules over time.

---

## §4. Tier 1 — Mass

### 4.1 Physics

- Volume × cost-efficiency. Each individual conversion is small; the system wins by aggregate.
- The unit of work is a **content piece** or **ad creative**, not a relationship.
- Decision lag is short (seconds to minutes from impression to install/signup).
- Brand voice consistency matters more than personal touch.

### 4.2 Channels (full table in `08_DISTRIBUTION_CHANNELS.md`)

- Organic social (X, LinkedIn, Reddit, Dev.to, Hashnode, Mastodon, Bluesky)
- TikTok organic + Shorts (Polis especially)
- Paid: Google Ads, Meta Ads (when approved by operator), TikTok Ads
- SEO + Programmatic SEO (long-tail keyword landing pages)
- GEO (citation in ChatGPT, Claude, Perplexity, Gemini answers)
- Webinars, free tools, lead-magnet downloads
- App store listings (Polis)

### 4.3 Agent classes

- Content Writer
- Visual Generator (image / video)
- Programmatic SEO Engineer
- GEO Optimizer (the structure that gets cited by AI engines)
- Multi-Platform Publisher
- Paid Ad Operator (under operator-approved budget envelope)
- Comment Triage (responds to comments within brand voice + escalation rules)

### 4.4 Failure modes

- Going off-brand under volume pressure.
- Generating content that reads as AI-generated (loses Tier-2/3 promotability).
- Getting platform-flagged for posting cadence.
- Over-indexing on engagement metrics (likes) that don't predict downstream conversion.
- Paying for fake clicks/impressions in poorly-monitored ad placements.

---

## §5. Tier 2 — Engaged

### 5.1 Physics

- Authentic, contextual interaction in a small number of high-signal communities.
- The unit of work is a **thread**, a **response**, a **DM**, or a **conversation**, not a piece of content.
- Decision lag is medium (days to weeks: first see → research → ask peers → try → activate).
- Long-form is acceptable. Lazy is fatal.

### 5.2 Channels

- GitHub (for Leapable, Synapse): issues, discussions, code review on adjacent projects
- Hacker News (Show HN, comments, niche threads)
- Reddit (technical subreddits: r/MachineLearning, r/LocalLLaMA, r/programming, r/devops)
- Discord (named tech Discords: AI engineering communities, Anthropic-builder, OpenAI-dev, Cursor, LangChain, MCP, individual VC dev communities)
- Slack (named workspaces operator has access to)
- Specialized newsletters (Substack, Beehiiv-based publications targeted at the right reader)
- Podcasts targeting practitioners (Latent Space, Lex Fridman for AI, Bankless for crypto-adjacent, etc.)
- In-person: AI engineering meetups, hackathons (small ones the agent can suggest the operator attends)

### 5.3 Agent classes

- Community Operator (per-community playbook; reads the room; doesn't promote until invited)
- Long-Form Writer (deep technical content; the kind that earns HN front page on merit)
- Podcast Booker (pitches operator to relevant shows; not the operator's voice in interviews)
- Newsletter Pitcher (gets the operator into other people's newsletters)
- Conversation Continuator (replies to mentions of the product without sounding scripted)
- Hackathon Strategist (which to attend, who to talk to, what to bring)
- GitHub Maintainer (issues triage, PR review, release notes, README maintenance)

### 5.4 Failure modes

- Posting before earning the right to post.
- Generic responses that read as AI-generated under community scrutiny.
- Triggering moderation by misreading subreddit rules.
- "Performative engagement" — long comments designed to be screen-shotted, not to help. Community detects this in days.
- Cross-posting the same content across multiple T2 channels (overlap detection kills credibility).

### 5.5 Key principle (Joe Karlsson, 2026 DevRel)

> *"Pick one or two community channels and go deep. Not five. A Discord with one person who answers questions within an hour beats five channels where nothing happens."*

The Engaged tier rewards depth, not breadth.

---

## §6. Tier 3 — Elite

### 6.1 Physics

- A handful of people whose endorsement, hire, or simply *visible adoption* moves an entire market segment.
- The unit of work is a **warm introduction**, a **dinner**, a **private DM**, an **investor email**, a **board meeting reference**.
- Decision lag is months (relationship → trust → context → use → endorse).
- One excellent interaction is worth a thousand decent ones.

### 6.2 Channels

- Personal network of operator and operator's investors / advisors / customers
- Founder dinners, off-the-record retreats, salon-style events
- Curated podcasts that Tier-3 people themselves listen to
- Trusted private Slack/Signal/iMessage threads (never automated; never agent-touched)
- Conferences at the right invitation tier (TED, All-In summit, RSA, Bilderberg-adjacent — not the general session)
- LinkedIn DM but only after a warm intro has been documented

### 6.3 Agent classes

- **Influence Network Mapper** (sees `10_INFLUENCE_NETWORK_MAPPING.md`): scans LinkedIn + Crunchbase + investor portfolios + board memberships + alumni networks to identify the shortest warm-intro path from operator to a Tier-3 target
- **Warm-Intro Drafter**: produces a forwardable paragraph the introducer can send, written in the introducer's voice (with explicit operator approval before sending)
- **Pre-Call Briefer**: before any Tier-3 conversation, produces a one-page brief on the contact (recent posts, recent investments, recent talks, what they care about right now)
- **Follow-Up Tracker**: never sends. Tracks every Tier-3 conversation, surfaces "this person hasn't been touched in 90 days, here's a relevant signal that might be worth surfacing"

### 6.4 Hard rules for Tier 3

These agents are **`REQUIRE_APPROVAL` by default, with no operator-configurable bypass.**

- **Never** send automated outbound to a Tier-3 contact. The operator hand-sends every word.
- **Never** scrape Tier-3 personal data beyond what's publicly indexed.
- **Never** assume a relationship. The Influence Mapper produces *paths*, not *contacts*.
- **Never** use Tier-3 names in T1 / T2 marketing copy without explicit, written permission.
- **Always** treat a Tier-3 endorsement as a privilege, not a transaction. If the endorser later reverses, the right move is silence and gratitude, not pushback.

### 6.5 The closed media bubble problem (Steve's insight)

> Steve: *"Every ultra-rich has their own book… every media bubble is very closed-door… So you have to figure that out. Who, what is my brother or my brother-in-law or sister-in-law, or my partner or this or that, who influences that person?"*

The Tier-3 game is to map the **influencers of influencers**. The Influence Network Mapper is built for exactly this: it walks the graph from operator-known nodes outward, identifies bridge nodes (people whose endorsement would be heard by the target Tier-3 contact), and surfaces the cheapest warm-intro path.

The closed-door nature means **no amount of agent automation reaches Tier-3 directly**. The path is always: operator → known bridge → warm intro → Tier-3. The agent's job is to make sure the operator never misses a viable bridge they could have used.

---

## §7. Tier movement

Tiers are not static. People graduate, demote, become irrelevant, become indispensable. The Sorter Agent re-evaluates contacts on a 90-day cycle. Three movement patterns matter:

| Movement | Trigger | Action |
|---|---|---|
| T1 → T2 | Repeat product engagement; deep comment on operator content; community mod activity | Move to engaged tier; agent starts engaging by name |
| T2 → T3 | Recently founded a unicorn; named to a board of a major adopter; viral content in operator's domain | Operator review required; if confirmed, move to gated tier |
| T3 → T2 (rare demotion) | Sustained inactivity; departed the field; reputational event | Tag and pause outreach; operator may keep open or close |

Movement events propagate to the goal-progress log so the system understands when a relationship's expected impact has changed.

---

## §8. Tier-aware budgeting

Within each goal's monthly budget, the operator can set a tier mix:

```yaml
budget:
  monthly_ceiling_usd: 25000
  tier_allocation:
    t1_mass: 0.50    # $12.5k — paid + content + tooling
    t2_engaged: 0.35 # $8.75k — community ops, podcasts, conferences
    t3_elite: 0.15   # $3.75k — operator dinners, private events, travel
```

The Cost Pacer enforces tier ceilings. T1 spend cannot poach from T3 budget without operator approval. This prevents the common autonomous-system failure of "the agent kept buying ads because that's what it could measure" while starving the relationship work it couldn't measure as cleanly.

---

## §9. Tier × product matrix

Different products lean on different tiers for primary momentum:

| Product | T1 weight | T2 weight | T3 weight | Rationale |
|---|---:|---:|---:|---|
| Leapable | 30% | 50% | 20% | Subscribers acquired via T1 mass (SEO/GEO/creator-led discovery); developers in T2 (HN/GitHub/Discord); T3 endorsement from named experts publishing Vaults gives marketplace legitimacy |
| Synapse | 25% | 55% | 20% | Developer trust earned in T2 (HN/GitHub/Discord); T3 endorsement from MCP / Anthropic ecosystem heavyweights is high-leverage |
| Polis | 50% | 35% | 15% | Consumer; T1 TikTok virality is the catalyst; T2 campus ambassadors create density; T3 intellectual creator endorsements give cultural legitimacy |

These weights are the **starting** allocation. The fleet shifts them based on which tier's experiments are moving the predicate fastest within a given goal.

---

## §10. Cross-references

- The Sorter Agent implementation: `05_AGENT_FLEET_DESIGN.md §6.3`.
- T3 Influence Network Mapper: `10_INFLUENCE_NETWORK_MAPPING.md`.
- T1 content factory: `07_CONTENT_FACTORY.md`.
- T2 community ops: `12_DEVELOPER_MARKETING.md`.
- Polis-specific T1 + T2 launch: `13_CONSUMER_SOCIAL_LAUNCH.md`.
- Per-tier autonomy defaults: `02_FIRST_PRINCIPLES.md §2`.
