# 11 — Per-Product GTM Playbooks

> Same fleet, three motions. Each playbook applies Hormozi Grand Slam Offer + Value Equation + Core Four to a specific product. Each carries a starting goal-spec template, an initial channel mix, an initial referral mechanic, and a first-90-days execution outline. 90-day detail in `17_LAUNCH_PLAYBOOK_90DAY.md`.

**Order:** Leapable first (already shipping, lead product). Synapse second (also shipping, dev-tool). Polis third (consumer, longer cold-start).

---

## §1. Leapable

### 1.1 What it is (recap from `leapablememory/README.md`)

Local-first MCP knowledge-vault marketplace. Three audiences:

- **Subscribers** — query expert Vaults via `Ask` and get source-grounded answers with citations + provenance.
- **Creators** — publish Vaults (documents, transcripts, expert corpora) on Discover; 70/30 rev share via Stripe Connect.
- **Developers** — integrate via 163 MCP tools (search, document, chunk, image, vault management, marketplace).

Already shipping on `leapable.ai`. Signed installer bundles `leapable-mcp` sidecar. End users do **not** install Node/Docker/Python/model weights. Windows + Linux use Bun sidecar; macOS uses bare-metal supervised by launchd.

Existing assets the marketing engine inherits:
- 163 MCP tools live
- 18 supported file types (PDF, DOCX, PPTX, XLSX, images, text, markdown, CSV)
- Hybrid BM25 + semantic + cross-encoder reranker search pipeline
- Provenance chain (cryptographically links chunks back to source files)
- VLM (vision-language) image analysis built-in
- Per-Vault embeddings (gte-multilingual-base default; pluggable)
- One Vault already operator-curated: `AlexHormoziTranscripts` (1,150 docs, 6,428 chunks)

### 1.2 The Grand Slam Offer (3-sided)

Three offers, one per audience:

**Subscribers** — *"Ask any expert anything. Get the answer they actually gave, with the receipts."*
- Dream outcome: instant access to a domain expert's lifetime of recorded knowledge
- Perceived likelihood: every answer is source-grounded with verbatim citation + page number
- Time delay: install in 60 seconds via signed installer; first Ask in <30 seconds
- Effort/sacrifice: no setup, no API keys, no copying files, no learning prompts; just type a question
- **Free** to install + sample 5 free Asks per Vault per month
- **$9/mo Subscriber** — unlimited Asks across all subscribed Vaults
- **Per-Vault paid plans** — set by Creator; 70% to Creator, 30% to Leapable; subscribers pay one bundled bill
- **Guarantee:** *"If you can't find the answer you needed in any subscribed Vault in 30 days, full refund AND we'll point you to a Vault that does answer it."*
- **Lead magnet:** the `AlexHormoziTranscripts` Vault — free sample-Ask access for first 30 days (already-curated, mass-known expert)

**Creators** — *"Turn your corpus into a $10k/month asset by Tuesday."*
- Dream outcome: monetize your existing recordings/PDFs/transcripts without building a SaaS
- Perceived likelihood: 70/30 rev share, Stripe Connect, billed-to-subscriber-then-paid-to-you; same infra as Substack but for AI-Asked knowledge
- Time delay: drag files into a Vault → publish to Discover in <15 minutes
- Effort/sacrifice: no platform-fee, no audience-build-from-zero, no production cost; we supply traffic
- **Free Creator account** with 1 Vault, up to 100 docs
- **$29/mo Creator Pro** — unlimited Vaults, unlimited docs, marketplace promotion, analytics
- **$99/mo Creator Plus** — featured placement, custom pricing tiers, AI-personalized landing pages
- **Guarantee:** *"If your Vault doesn't earn back the Creator Pro subscription in your first 90 days, we refund the subscription AND keep promoting your Vault free for 90 more days."*
- **Lead magnet:** "Publish-Ready Vault Checklist" — free 20-page guide on structuring a sellable knowledge corpus, with audit of operator's own AlexHormoziTranscripts as worked example

**Developers** — *"Drop-in MCP knowledge layer. Works with Claude Code / Cursor / Codex out of the box."*
- Dream outcome: agents that can query expert knowledge with full provenance, locally
- Perceived likelihood: 163 MCP tools, used in production for Leapable + agent fleet ourselves
- Time delay: install in 60s; first agent query in <5 min
- Effort/sacrifice: no RAG pipeline to build, no vector DB to manage, no embedding model to host; signed installer ships everything
- **Free for personal / open-source / non-commercial use**
- Pro tier for commercial deployments
- **Guarantee:** *"If your agent can't query a Leapable Vault successfully in 5 minutes, we'll do the integration with you on screen-share."*
- **Lead magnet:** "100 Leapable Vault Integration Patterns" — open-source GitHub repo with worked examples for Claude Code, Cursor, Codex, Continue, Aider, LangChain, CrewAI

### 1.3 Goal spec template

```yaml
goal_id: leapable-50k-q3-2026
product: leapable
target:
  metric: weekly_active_subscribers
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://leapable/cf_queries
guardrails:
  budget: {monthly_ceiling_usd: 25000, daily_velocity_cap_usd: 1500}
  unit_economics: {ltv_cac_floor: 3.5, cac_payback_ceiling_months: 12}
  brand: {voice_spec_path: docs/brandvoices/leapable.md}
  compliance: {jurisdictions: [US, EU, CA, UK], ai_disclosure_required: true}
  channels:
    preferred:
      - github
      - hackernews
      - x_organic
      - dev_to
      - hashnode
      - mcp_builder_discords
      - podcasts_tier1
      - newsletter_sponsorships
      - creator_partnerships
      - substack_expert_creators
autonomy: {level: medium, approval_required_above_usd: 500}
deadline_soft: 2026-09-01
```

### 1.4 Channel-mix opening allocation

The 3-audience structure means **three parallel channel layers**, each owned by different agent clusters but sharing the operator's voice.

**Layer 1 — Subscribers (T1 + T2 mass).** SEO + GEO + creator-led discovery.

| Phase | Channels |
|---|---|
| 0-4w | GitHub README polish; HN Show HN; X-organic founder thread; Dev.to cross-posts |
| 4-8w | Creator partnerships (each Creator's audience is a Leapable acquisition channel); podcast appearances on AI search / RAG / agent shows |
| 8-12w | Programmatic SEO landing pages for "How to Ask [Expert] [Topic]" patterns; GEO-optimized content for AI search engines (citation in ChatGPT/Claude/Perplexity answers) |
| 12w+ | Paid Google Ads on intent keywords; paid X/Reddit Ads to dev audiences |

**Layer 2 — Creators (T2 + T3 intellectual experts).** Direct outreach + creator partnerships.

| Phase | Channels |
|---|---|
| 0-4w | Operator-curated demo Vaults (Hormozi already exists; add Karpathy, 1-2 named-expert Vaults) as proof |
| 4-8w | Cold outreach (`D4`) to 50 named experts with monetizable corpora (podcasters, authors, consultants); each pitch carries the 90-day guarantee |
| 8-12w | Substack-creator interview swaps; podcast appearances on creator-economy shows (Creator Economy Report, Means of Creation) |
| 12w+ | Affiliate program for Creators (you refer another Creator, you earn % of their first-year revenue) |

**Layer 3 — Developers (T2 MCP-native).** Same as Synapse — dev marketing flywheel.

| Phase | Channels |
|---|---|
| 0-4w | MCP-builder Discord engagement; GitHub repo polish; first OSS reference integration |
| 4-8w | Hackathon sponsorships (AGI House, AI Engineer Summit, MCP-specific events) |
| 8-12w | Integration partnerships (Claude Code, Cursor, Continue.dev, LangChain — published integration guides) |
| 12w+ | Conference talks; integration revenue-share with adjacent dev tools |

### 1.5 Viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md`, Leapable's loops compound across 3 sides:

- **Subscriber → Subscriber:** "Ask shared with you by Sarah" landing pages; first Ask is free; share-rate target 25%
- **Subscriber → Creator:** "Found this answer in [Creator]'s Vault. Subscribe to ask more" — every shared answer is also a Creator-acquisition signal
- **Creator → Subscriber:** Creators promote their Vaults to their existing audience; Leapable provides per-Creator referral codes with 90-day attribution
- **Creator → Creator:** affiliate program; refer another Creator, earn 10% of their year-1 revenue
- **Developer → Subscriber:** every Leapable-integrated agent that ships exposes Vault content to its end-users; "Powered by Leapable" optional credit

Compound K-target across all 4 loops: **0.5–0.7** within 90 days. Cycle time: **5 days** subscriber-side, **14 days** creator-side.

### 1.6 The Elite-tier plan

Tier-3 targets via Influence Network Mapper (`10_INFLUENCE_NETWORK_MAPPING.md`):

- **Named intellectual experts whose corpora become Leapable Vaults** (T3 partners): Tyler Cowen, Sam Harris, Russ Roberts, Paul Graham (essays), Patrick Collison, Marc Andreessen, Tim Ferriss, Lex Fridman (podcast transcripts), Andrew Huberman (research)
- **AI lab leadership** (T3 endorsers): Anthropic + OpenAI + Google DeepMind leads who care about provenance, citation, AI-grounded answers
- **VC partners** in AI infrastructure (Sequoia, a16z, Index, NEA, GV)
- **Creator-economy investors** (a16z Crypto/Games, Atelier Ventures) — for the marketplace angle

Goal: 1 T3 endorsement-quality conversation per week. Bridge-cultivation cadence per `10_INFLUENCE_NETWORK_MAPPING.md §8`.

### 1.7 90-day predicate target

```
Week 4 :     500 weekly_active_subscribers
Week 8 :   2,500
Week 12:  10,000
Week 16:  25,000
```

50k is a 6-9 month goal. The 90-day spec sets `threshold: 10000` with the 50k as a follow-on goal.

---

## §2. Synapse

### 2.1 Product (recap from `synapse-mcp` systemspec)

Rust MCP server exposing Win11 desktop perception (capture, UIA, OCR, audio) + action (click, type, aim, press, drag, combo) + sub-frame reflex controllers. Binary `synapse-mcp` ships via signed installer. AI agents call Synapse for computer-use across any Windows app. Currently at `v0.1.0-m3` with M4 (RP2040 hardware HID + first game profile + 3 new tools) active.

### 2.2 Grand Slam Offer

**Dream outcome:** "My AI agent can use my computer — see, click, type, drive a game — as well as I can."
**Perceived likelihood:** open-source Rust core, MCP-native, dogfooded by operator's own marketing fleet (Synapse drives Synapse's own marketing).
**Time delay:** install + first agent action in <5 min.
**Effort/sacrifice:** MCP — no new SDK, no new framework; works with existing AI agent stack.

- **Free forever** for personal / open-source / non-commercial use
- **$29/mo Individual Pro** — ViGEm controller, advanced hotkey support, encrypted session recording
- **$499/mo Team** — shared playbooks, multi-machine fleet, audit trails
- **Enterprise** — on-prem + SLA + custom integrations
- **Guarantee:** *"If your agent can't complete a basic Notepad workflow in <60 seconds on day 1, we'll personally walk through setup on screen-share."*
- **Lead magnet:** "100 MCP Automation Playbooks" GitHub repo — free, no opt-in

### 2.3 Goal spec template

```yaml
goal_id: synapse-25k-q3-2026
product: synapse
target:
  metric: weekly_active_mcp_sessions_with_depth
  threshold: 25000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://synapse/cf_sessions
guardrails:
  budget: {monthly_ceiling_usd: 20000, daily_velocity_cap_usd: 1000}
  unit_economics: {ltv_cac_floor: 3.0, cac_payback_ceiling_months: 12}
  channels:
    preferred: [github, hackernews, x_organic, mcp_builder_discords, ai_agent_hackathons, podcasts_tier2]
autonomy: {level: medium, approval_required_above_usd: 500}
deadline_soft: 2026-09-01
```

### 2.4 Channel-mix opening allocation

Same pattern as Leapable's developer layer, sharper focus:

| Phase | Channels |
|---|---|
| 0-4w | GitHub README polish; HN Show HN; X-organic; MCP-builder Discords (Anthropic, Cursor, LangChain, OpenAI, MCP-specific) |
| 4-8w | YouTube demos (Synapse + Claude live game/Notepad/Excel workflows); hackathon sponsorships (AGI House, AI Engineer Summit) |
| 8-12w | Podcast tour (Latent Space, MLST, AI Engineering); integrations guild (Claude Code + Cursor + Continue) |
| 12w+ | Enterprise OS-RPA-replacement angle; cold outreach to Operations leaders at AI agent companies; LinkedIn Ads on automation-decision titles |

### 2.5 Viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md §3.2`:
- **Demo / UGC loop:** every successful Synapse session exportable as sharable video; "Built with Synapse" referral ID embedded
- **Playbook library:** community Synapse scripts; each install needs Synapse
- **Hackathon loop:** sponsored hackathons → winners amplified → audience installs to replicate

K-target: 0.35 within 90 days. Cycle time: 7 days.

### 2.6 Elite-tier plan

Tier-3 targets: MCP ecosystem leadership (Anthropic MCP team), AI agent platform founders (Cognition/Devin, Cursor, Replit Agent), accessibility advocates, OS-RPA decision-makers at Fortune 1000.

### 2.7 90-day predicate target

```
Week 4 :    200 weekly_active_mcp_sessions_with_depth
Week 8 :    800
Week 12:  2,500
Week 16:  6,000
```

---

## §3. Polis / Serendipity Engine

### 3.1 Product (recap from `docs/serendipityengine/`)

Re-engineered social platform around physical/synchronous gathering, friend-density network effects, debate, knowledge growth, and intellectual love. Anti-engagement-farming by design. Five gathering formats (salon, debate, third-place revival, knowledge-growth sessions, love-layer experiences). Sortition governance.

### 3.2 Grand Slam Offer

**Dream outcome:** "I find people who actually want to talk about what I care about, in real life, on a regular cadence — not doom-scroll dopamine."
**Perceived likelihood:** curated friend-of-friend network in your city; 3 intellectual/artistic/debate formats per week within walking distance; show-up rates ≥65%.
**Time delay:** first event in 7 days; friend-cluster activation in 30 days.
**Effort/sacrifice:** free to attend; no algorithm; no content quotas; RSVP and show up.

- **Free** for attendees in seed cities
- **$9/mo Polis Plus** — early access to high-density curated events, host-prize draws
- **$29/mo Polis Host** — tools to host events in your city
- **$99/mo Polis Curator** — editorial-grade events with revenue share
- **Guarantee:** *"If you attend 3 Polis events and don't make a meaningful new connection, we'll cover a small dinner at a local spot for you and a friend you've already met."*
- **Lead magnet:** "Third Place Revival Guide" — free practical guide on hosting intellectual gatherings

### 3.3 Goal spec template

```yaml
goal_id: polis-50k-attendance-q3-2026
product: polis
target:
  metric: monthly_active_event_attendances
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: postgres://polis/events_attendances
guardrails:
  budget: {monthly_ceiling_usd: 30000, daily_velocity_cap_usd: 1500}
  unit_economics: {ltv_cac_floor: 4.0, cac_payback_ceiling_months: 9}
  channels:
    preferred: [tiktok_organic, instagram_organic, substack_intellectual, podcasts_culture, campus_ambassadors, x_organic, in_person_events]
    forbidden: [linkedin_ads, programmatic_seo]
autonomy: {level: medium, approval_required_above_usd: 500}
kill_switches:
  - brand_voice_violation
  - attendance_rate_below_0.50
  - goodhart_signal_detected
  - operator_emergency_stop
deadline_soft: 2026-09-01
```

### 3.4 Channel-mix opening allocation

| Phase | Channels |
|---|---|
| 0-4w | Cambridge/Somerville single-city seed; operator + 2 ambassadors host 3 events/week; TikTok 3-5 vertical/day |
| 4-8w | 5 events/week; campus ambassadors paid out (BeReal pattern $30/$50); first Substack creator outreach; first podcast pitches |
| 8-12w | Expand 3 cities (Boston/NYC/Brooklyn); ambassador program scales (3 per city); operator-attended launches |
| 12w+ | Paid Meta + TikTok in proven cities; Polis Curator launches; intellectual creator partnerships |

### 3.5 Viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md §3.3` + `13_CONSUMER_SOCIAL_LAUNCH.md`:
- **Network-effect loop:** events get richer with more friends. Friend invites are the core product action.
- **Campus ambassador program:** $30/successful referral + $50/install+attendance
- **Two-prize Hormozi/Skool giveaway** at every event: host-prize draw + referrer-prize draw

K-target: 0.6 within 60 days. Cycle time: 3 days.

### 3.6 Elite-tier plan

Tier-3 targets: Tyler Cowen, Sam Harris, Russ Roberts, Patrick Collison, Marc Andreessen, Tim Ferriss, intellectual conference organizers, third-place revival movement leaders (Strong Towns adjacent), major Substack writers with intellectual/cultural focus.

### 3.7 90-day predicate target

```
Week 4 :    200 monthly_active_event_attendances
Week 8 :  1,500
Week 12:  6,000
Week 16: 18,000
```

50k is 6-9 months. Seed-city density first; expansion second.

---

## §4. Cross-product synergies

### 4.1 Leapable + Synapse + Polis share the operator's voice

Single operator, single brand. Leapable posts on AI/knowledge-Ask topics; Synapse on computer-use; Polis on IRL community. All under operator's personal channels. Persona Carrier ensures voice consistency where personas overlap (operator's personal channel) and divergence where they don't (Polis-specific account).

### 4.2 Leapable + Synapse audience overlap (high)

AI engineers who care about Leapable also care about Synapse. Polis-attending intellectuals partially overlap. Cross-promotion: a Leapable launch post can mention Synapse; a Synapse podcast appearance can mention Leapable.

### 4.3 Leapable is the natural dogfood substrate

The agent fleet itself uses Leapable Vaults (Hormozi, Karpathy, etc.) as memory layer (`16_TECH_STACK.md §4.2`). Marketing Leapable while running the marketing fleet on Leapable is the strongest possible proof.

### 4.4 Operator credibility compounds

Successful Leapable + Synapse track record makes Polis pitch more credible. The operator who builds rigorous AI infrastructure is also rebuilding third places. Brand compounds.

---

## §5. The Hormozi "max one, then add" applied across products

Hormozi: *"Just pick one. Then max it out."*

Recommended start order:
1. **Months 1-3:** Leapable only. Validate the engine against the ready-to-market product with the clearest predicate (subscriber Asks). Cross-product audience overlap with Synapse means content seeds for Synapse audiences naturally.
2. **Months 3-6:** Add Synapse. Same dev audience, same channel playbook; validate cross-product audience handoff.
3. **Months 6-9:** Add Polis. Different audience, different physics; fleet has matured.

Concurrent launches viable: Leapable + Synapse can co-launch in Phase C because they share dev audience (hackathons, podcasts, channels). Polis is operationally separate.

---

## §6. Cross-references

- Goal spec full grammar: `22_GOAL_SPEC_FORMAT.md`
- Channel inventory: `08_DISTRIBUTION_CHANNELS.md`
- Viral mechanics: `09_VIRAL_LOOPS_AND_REFERRALS.md`
- DevRel flywheel (Leapable + Synapse): `12_DEVELOPER_MARKETING.md`
- Consumer launch (Polis): `13_CONSUMER_SOCIAL_LAUNCH.md`
- 90-day execution: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Influence network mapping (T3 per product): `10_INFLUENCE_NETWORK_MAPPING.md`
- Metrics + predicates: `15_METRICS_AND_OBSERVABILITY.md`
