# 08 — Distribution Channels

> Every channel, scored by Hormozi's Core Four × Lead Getters lens, mapped to product, tier, and agent class. Per-channel cards live in `channel_cards/<platform>.yaml`. This doc is the index, the prioritization framework, and the channel-mix policy.

---

## §1. Hormozi's Core Four — the only ways to let people know

Hormozi (`$100M Leads`):

> *"Combining warm and cold audiences with 1-to-1 and 1-to-many leads us to the only four ways we can let anyone know about anything: the core four."*

|  | 1-to-1 | 1-to-many |
|---|---|---|
| **Warm audience** (people who already know you) | **Warm Outreach** | **Posting Content** |
| **Cold audience** (strangers) | **Cold Outreach** | **Paid Ads** |

Every channel the fleet operates maps to one of these quadrants. There is nothing else.

> *"Pick one. Then max it out. Do more. Do better. Do new. And all the advertising methods compound together."*

The fleet runs all four simultaneously across goals, with the **Cost Pacer** rebalancing weekly based on which quadrant's experiments are moving the predicate.

---

## §2. Hormozi's Lead Getters — the multipliers

Once a customer enters, they can become a **lead getter**:

| Lead getter | Who | Why |
|---|---|---|
| **Customer Referrals** | happy customers refer their friends | Cheapest CAC, exponential, highest LTV |
| **Employees** | the team gets leads on your behalf | Direct influence, on-payroll |
| **Agencies** | businesses with skill at a specific channel | Borrow their playbook; eventually replace them |
| **Affiliates** | businesses who tell their audience about your stuff | Once set up, self-running |

> *"The last skill you ever need to learn is how to get other people to do everything you need for you."* (Hormozi)

Lead getters compound the Core Four. The fleet runs Core Four directly **and** invests in turning customers/employees/agencies/affiliates into lead getters. Both layers.

The Referral Engineer (O3) owns customer referrals. Employees aren't agent-controlled (they're operator-managed). Agencies are operator-managed but the agent fleet documents the agency's playbook for eventual in-house replication. Affiliates are partner-network — Influence Network Mapper (S3) + Outreach Drafter (D4) handle the discovery + pitch.

---

## §3. Channel inventory

Each channel is scored on:
- **Quadrant** (Core Four classification)
- **Tier fit** (T1 mass / T2 engaged / T3 elite — from `03_TIER_MODEL_AUDIENCE.md`)
- **Product fit** (CG ContextGraph / SY Synapse / PO Polis)
- **CAC range** (typical 2026 benchmarks)
- **Ramp** (weeks to first material traction)
- **Owner agent class** (`05_AGENT_FLEET_DESIGN.md`)

### 3.1 Owned surfaces (no platform rate limits)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| Operator blog (operator.com) | Posting Content | T1, T2 | CG, SY, PO | $50-200 amortized | 12-24 weeks | P1, O1 |
| Operator newsletter (Substack/Beehiiv) | Posting Content | T2 | CG, SY, PO | $30-100 | 8-16 weeks | P1 |
| Operator YouTube long-form | Posting Content | T1, T2 | CG, SY, PO | $40-150 | 16-32 weeks | P1, P3 |
| Operator podcast | Posting Content | T2 | CG, SY, PO | $80-300 (high effort) | 24-48 weeks | P1 + operator |
| Documentation / docs site | Posting Content | T2 | CG, SY | n/a — retention | 4-8 weeks | P2, D3 |
| Hackathon / event landing pages | Posting Content | T2 | CG, SY | n/a | 1-2 weeks | P1 |

### 3.2 Developer-tier channels (T2-dominant)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| GitHub (repo + issues + sponsors) | Posting Content | T2 | CG, SY | n/a — trust signal | 4-12 weeks | D3 |
| Hacker News | Posting Content / 1-on-1 | T2 | CG, SY | $50-200 per submission win | per submission | D2, P1 (Show HN), D6 (comments) |
| Reddit (r/MachineLearning, r/LocalLLaMA, r/programming, r/devops) | Posting Content | T2 | CG, SY, PO (different subs) | $20-80 | 6-16 weeks | D2 |
| Dev.to | Posting Content | T2 | CG, SY | $30-100 | 4-12 weeks | P1, D1 |
| Hashnode | Posting Content | T2 | CG, SY | $30-100 | 8-16 weeks | P1, D1 |
| Discord (technical communities — operator-joined) | Posting Content / 1-on-1 | T2 | CG, SY | $20-60 (community-built) | 12-24 weeks | D2 |
| Slack workspaces (e.g., MLOps, MCP-builders) | Posting Content / 1-on-1 | T2, T3 | CG, SY | n/a — relationship | 12-32 weeks | D2 |
| Stack Overflow | Posting Content | T2 | CG, SY | $40-120 (per answer) | 16-32 weeks | D6 (sparingly) |
| Product Hunt | Paid Ads / Posting Content | T1, T2 | CG, SY, PO (consumer subset) | $200-1000 per launch | per launch | P1, D1 |

### 3.3 Long-form general channels (T1 + T2)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| X / Twitter | Posting Content | T1, T2 | CG, SY, PO | $30-100 (organic) | 8-24 weeks | D1, D6, operator |
| LinkedIn personal | Posting Content | T2 | CG, SY | $80-300 | 12-32 weeks | D1, operator |
| LinkedIn company | Posting Content | T1, T2 | CG, SY | $100-400 | 16-32 weeks | D1 |
| Mastodon | Posting Content | T2 | CG, SY (open-source-aligned) | $20-60 | 16-32 weeks | D1 |
| Bluesky | Posting Content | T2 | CG, SY, PO | $30-80 | 16-32 weeks | D1 |
| Threads | Posting Content | T1 | PO | $20-60 | 16-32 weeks | D1 |
| Medium | Posting Content | T1, T2 | CG, SY | $40-120 | 8-16 weeks | P1, D1 |

### 3.4 Consumer-tier channels (T1-dominant, PO-heavy)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| TikTok (organic) | Posting Content | T1 | PO, maybe SY | $5-30 per acquired user | 12-32 weeks | P1, P3, D1 |
| Instagram (organic) | Posting Content | T1 | PO | $20-80 | 16-32 weeks | D1 |
| YouTube Shorts | Posting Content | T1 | PO, maybe CG | $15-60 | 16-32 weeks | P3, D1 |
| Pinterest | Posting Content | T1 | PO (lower priority) | $20-80 | 24-48 weeks | (not prioritized) |
| Snapchat | Posting Content | T1 | PO | $20-60 | n/a | (not prioritized) |

### 3.5 Paid Ads (Cold 1-to-many)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| Google Ads (search + display) | Paid Ads | T1 | CG, SY (intent), PO | $200-1500 (B2B SaaS) / $5-25 (consumer) | 2-6 weeks | O4 |
| Meta Ads (FB/IG) | Paid Ads | T1 | PO primarily | $50-200 (consumer) | 2-6 weeks | O4 |
| LinkedIn Ads | Paid Ads | T1, T2 | CG, SY (B2B) | $400-2000 | 4-8 weeks | O4 |
| TikTok Ads | Paid Ads | T1 | PO | $20-80 | 2-6 weeks | O4 |
| X Ads | Paid Ads | T1, T2 | CG, SY (developer-targeting) | $200-800 | 4-8 weeks | O4 |
| Reddit Ads | Paid Ads | T1, T2 | CG, SY (subreddit targeting) | $80-300 | 4-8 weeks | O4 |
| Newsletter sponsorships | Paid Ads | T2 | CG, SY (developer newsletters) | $100-400 | 2-4 weeks | O4 |
| Podcast ad reads | Paid Ads | T1, T2 | CG, SY, PO | $80-400 | per ad | O4, D8 |

### 3.6 Outreach (Warm + Cold 1-to-1)

| Channel | Quadrant | Tiers | Products | CAC range | Ramp | Owner |
|---|---|---|---|---|---|---|
| Email outbound (cold) | Cold Outreach | T2 | CG, SY (enterprise) | $300-1500 (full SDR replaced) | 4-12 weeks | D4 |
| Email outbound (warm follow-up) | Warm Outreach | T2 | CG, SY, PO | $100-400 | 1-4 weeks | D4 |
| LinkedIn DM (post-connection) | Warm Outreach | T2, T3 | CG, SY | $200-800 | 4-12 weeks | D4 |
| X DM (post-mutual-follow) | Warm Outreach | T2, T3 | CG, SY | $200-800 | 4-12 weeks | D4 |
| Conference + meetup follow-up | Warm Outreach | T2, T3 | CG, SY, PO | $400-2000 | per event | D9 + operator |
| Personal-network warm intros | Warm Outreach | T3 | CG, SY (enterprise), PO | $200-1500 | weeks-months per target | D5 + operator |
| Podcast pitches | Cold Outreach | T2 (host) → T2/T3 (audience) | CG, SY, PO | $200-600 per booking | 4-12 weeks | D8 |

### 3.7 Lead Getter channels (the multipliers)

| Channel | Type | Tiers | Products | Mechanic | Owner |
|---|---|---|---|---|---|
| Referral program (customer) | Customer Referrals | T1, T2 | CG, SY, PO | Two-sided incentive; in-product mechanic | O3 |
| Hackathon sponsorships | Affiliate / Sponsorship | T2 | CG, SY | Pay community organizers to feature product | D9 |
| Open-source integrations | Affiliate (indirect) | T2 | CG, SY | Build adapters for popular adjacent tools | D3 |
| University ambassador program | Customer Referrals (Gen Z) | T1, T2 | PO | BeReal-pattern campus ambassadors $30/referral $50/install | O3 + operator |
| Influencer affiliate program | Affiliate | T1, T2 | PO, maybe CG/SY | Revenue share on referred subs | D7 + O3 |
| Newsletter affiliate program | Affiliate | T2 | CG, SY | Revenue share on signups from named newsletters | O3 |
| Joint launches with adjacent products | Affiliate | T2 | CG, SY | Cross-promotion with non-competing dev tools | O3 + operator |

---

## §4. Channel prioritization framework

For any goal, the Planner chooses channels based on:

1. **Predicate movement potential** (per `expected_metric_impact` blocks).
2. **CAC vs goal's LTV:CAC floor** — channels with expected CAC over `LTV / 3` are deprioritized.
3. **Ramp time vs deadline** — a 32-week channel doesn't help a 90-day goal.
4. **Channel-mix diversity** — don't put >50% of effort on one channel; Hormozi's compound rule applies, but also platform-risk.
5. **Reflexion memory** — prior outcomes on this channel × product × tactic combo.

The Planner outputs a channel mix per goal:

```yaml
channel_mix_for_goal: contextgraph-50k-q3-2026
total_budget_monthly_usd: 25000
allocation:
  github:                  { share: 0.10, mode: free_owned, owner: D3 }
  x_organic:               { share: 0.10, mode: free_owned, owner: D1+operator }
  hackernews:              { share: 0.05, mode: free_owned, owner: D2 }
  reddit:                  { share: 0.05, mode: free_owned, owner: D2 }
  discord_ai_communities:  { share: 0.10, mode: free_engagement, owner: D2 }
  podcast_appearances:     { share: 0.10, budget_usd: 2500, owner: D8 }
  dev_to_hashnode:         { share: 0.05, mode: free_owned, owner: D1 }
  programmatic_seo:        { share: 0.05, mode: free_owned, owner: O1 }
  geo_optimization:        { share: 0.05, mode: free_owned, owner: O2 }
  cold_outreach_enterprise: { share: 0.10, budget_usd: 2500, owner: D4 }
  paid_google_ads:         { share: 0.10, budget_usd: 2500, owner: O4 }
  paid_x_ads:              { share: 0.05, budget_usd: 1250, owner: O4 }
  newsletter_sponsorships: { share: 0.05, budget_usd: 1250, owner: O4 }
  referral_program:        { share: 0.05, budget_usd: 1250, owner: O3 }
```

This mix is **reviewed weekly** by the Communicator + Operator. Channels that miss expected impact for 4 consecutive weeks are deprioritized. New channels can be proposed by the Planner with operator approval.

---

## §5. Channel cards (per-platform operational state)

Each channel has a YAML card stored in `channel_cards/<platform>.yaml`. Schema:

```yaml
platform: hackernews
quadrant: posting_content
tiers: [t2]
products: [contextgraph, synapse]

operator_accounts:
  - {id: hn_chris, karma: 4290, account_age_years: 8}

rate_caps:
  comments_per_24h: 6
  submissions_per_90d: 1
  upvotes_per_24h: 30

posting_cadence_distribution:
  type: log_normal
  mean_hours: 4
  stddev_hours: 2.5

content_format_rules:
  comments: substantive_only; min_30_chars; no_self_promotion_until_thread_substantive
  submissions: Show_HN_format; show_actual_artifact; first_comment_within_5min_by_maker

detection_signals_to_avoid:
  - upvote_ring_pattern
  - timing_clustering
  - identical_phrasing
  - boilerplate

successful_skills_path: skills/launch-product-hunt-hackernews.md
last_audit_date: 2026-05-20
last_audit_outcome: kapca_0.92
notes:
  - "HN front page lifts last about 18-24 hours; convert hard during that window"
  - "Comment quality > submission quality for sustained karma"
  - "Never post within 24h of a major model-vendor announcement"
```

The Wiki Gardener (M10) keeps channel cards fresh — when a card's `last_audit_date` is >30 days old, the relevant agent class refreshes it.

---

## §6. The Hormozi "Pick one, max it, then add another" rule

> *"Just pick one. Then, max it out. Do more. Do better. Do new."*

The fleet does not start day one on 15 channels. It picks **one or two** within the goal's primary quadrant and maxes them out. Once they reach the LTV:CAC floor and a stable predicate-contribution, a third is added.

The "depth then width" pattern from `07_CONTENT_FACTORY.md §2`:

| Phase | Active channels | Behavior |
|---|---|---|
| 0-30d | 1-2 channels | Find the channel that works for the product. Hormozi: max it. |
| 30-90d | 3-5 channels | Add adjacent channels that compound (e.g., HN → Dev.to → X-organic). |
| 90d+ | 6-10 channels | Width phase. Each new channel inherits skills from the previous. |
| 6mo+ | 10-15 channels | All-on, with operator-level resource allocation. |

The fleet **does not skip ahead.** A goal that's 30 days in and on its second channel does not get a TikTok Ads campaign added because "we should be everywhere." It gets the second channel maxed first.

---

## §7. Product × channel matrix

The starting channel mix per product (`03_TIER_MODEL_AUDIENCE.md §9`):

### 7.1 ContextGraph / ME‑JEPA‑Code

Primary (start here): **GitHub + HN + X-organic**. These are where AI engineers actually live.

Secondary (add at 30d): **Discord (Anthropic, Cursor, LangChain, MCP-builders), Dev.to, podcast appearances (Latent Space, MLST), newsletter sponsorships**.

Tertiary (add at 90d): **Cold enterprise outreach, paid Google ads on intent keywords, LinkedIn Ads to AI infra decision-makers**.

### 7.2 Synapse

Primary: **GitHub + HN + X-organic**.

Secondary: **MCP-builder Discords, AI agent hackathons, dev.to, podcast appearances on MCP/agent-related shows, integrations guild (build adapters into popular AI agent frameworks)**.

Tertiary: **LinkedIn for the accessibility / automation enterprise angle, cold outreach to operations leaders at AI agent companies, paid LinkedIn Ads**.

### 7.3 Polis / Serendipity Engine

Primary: **TikTok + Instagram + X-organic + campus ambassadors**.

Secondary: **Substack newsletter (intellectual creators), podcast appearances on intellectual / culture shows, in-person events in seed cities**.

Tertiary: **Paid Meta + TikTok Ads in proven cities, partnerships with intellectual creators (interview swap, joint content), affiliate program for intellectual creators**.

---

## §8. The Hormozi compound rule

> *"All the advertising methods compound together. The money, systems, and experience you earned from the prior method will help you master the next."*

Per Hormozi: a business doing content + paid ads gets more out of *both* than one doing only one. Specifically:

- Content makes your ads convert higher (cold audience has already heard of you).
- Ads make your content reach further (paid amplification of organic winners).
- Outreach + content: outreach gets warmer when targets have seen your content.
- Affiliates + ads: ads make your affiliates' job easier; affiliates make your ads more efficient via creator UGC.

The Planner exploits this. When a content piece overperforms organically, the Ad Operator (O4) is auto-prompted to consider boosting it with paid amplification. When outreach acceptance rates are high in a vertical, the SEO Strategist (O1) is auto-prompted to publish vertical-specific landing pages.

---

## §9. Channel deprecation

A channel exits the active mix when:

- 4 consecutive weeks of zero predicate contribution
- CAC exceeds 2× the goal's LTV:CAC floor for 4 consecutive weeks
- Platform changes that make the channel structurally hostile (e.g., a major algo change that buries the operator's content type)
- Account suspension or sustained shadowban

Deprecation is not deletion. The channel card is preserved with a "deprecated" tag and a reason. Future goals can re-evaluate. Channels can come back; the fleet remembers what worked.

---

## §10. Cross-references

- Per-platform behavioral specs: `channel_cards/<platform>.yaml`
- Computer-use vs API mode: `06_COMPUTER_USE_PLAYBOOK.md`
- Content production: `07_CONTENT_FACTORY.md`
- Lead Getter mechanics (referral, affiliate): `09_VIRAL_LOOPS_AND_REFERRALS.md`
- Per-product GTM specifics: `11_PRODUCT_PLAYBOOKS.md`
- Polis-specific consumer launch: `13_CONSUMER_SOCIAL_LAUNCH.md`
- DevRel playbook: `12_DEVELOPER_MARKETING.md`
- Influencer / elite-tier: `10_INFLUENCE_NETWORK_MAPPING.md`
- Budget pacing: `02_FIRST_PRINCIPLES.md §16`, `15_METRICS_AND_OBSERVABILITY.md §6`
