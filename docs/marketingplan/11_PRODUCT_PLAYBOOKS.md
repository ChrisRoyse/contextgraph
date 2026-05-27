# 11 — Per-Product GTM Playbooks

> Same fleet, three different motions. Each playbook applies Hormozi's Grand Slam Offer + Value Equation + Core Four lens to a specific product. Each carries a starting goal spec template, an initial channel mix, an initial referral mechanic, and a first-90-days execution outline. The 90-day timeline lives in detail in `17_LAUNCH_PLAYBOOK_90DAY.md`; this doc establishes the per-product theses.

---

## §1. ContextGraph / ME‑JEPA‑Code

### 1.1 The product (recap from CLAUDE.md)

A **binary reality predictor**. For any AI-generated code change, returns `Pass | Fail` against the Docker oracle, plus three additional grounded surfaces (Q1 claim existence, Q3 named failure mode + exemplars, Q5 reality-shift impact). MCP server interface; AI coding agents (Claude Code, Cursor, Codex) call it via JSON-RPC stdio.

Product threshold: `prediction_oracle_correlation ≥ 0.95` stable × 4 windows on the 300×8 Python SWE-bench Lite corpus.

### 1.2 The Grand Slam Offer (Hormozi frame)

> *"Offer so good people feel stupid saying no."*

**Dream outcome:** "I can ship AI-generated patches and trust that what gets to CI actually works."

**Perceived likelihood:** "Backed by a 0.95-correlation oracle on the SWE-bench benchmark a regulator will recognize."

**Time delay:** "Per-patch verification runs in seconds. Install + first verification in under 10 minutes."

**Effort & sacrifice:** "Drop-in MCP server. No retraining your agents. No new build system. Works with Claude Code, Cursor, Codex out of the box."

**The offer:**
- Free tier: 1,000 verification calls / month forever.
- $99/mo Pro: 50,000 calls/month + private rule libraries + Discord access.
- Enterprise: custom pricing, on-prem option, SLA, SOC 2.
- **Guarantee:** "If ME‑JEPA-Code's verdict on your patch disagrees with the Docker oracle's verdict more than 5% of the time across 100 patches in your codebase, we refund and work with you free for 60 days until our predictor calibrates to your stack." (Hormozi-style "if you don't X, I will Y" with teeth.)
- **Lead magnet:** the "AI Coding Agent Verification Cookbook" — a free 80-page open-source guide (no opt-in) with Anthropic Claude Code + Cursor + Codex examples.

### 1.3 Goal spec template

```yaml
goal_id: contextgraph-50k-q3-2026
product: contextgraph
target:
  metric: weekly_active_integrations
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
guardrails:
  budget: { monthly_ceiling_usd: 25000, daily_velocity_cap_usd: 1500 }
  unit_economics: { ltv_cac_floor: 4.0, cac_payback_ceiling_months: 12 }
  brand: { voice_spec_path: docs/brandvoices/contextgraph.md }
  compliance: { jurisdictions: [US, EU, CA], ai_disclosure_required: true }
  channels:
    preferred: [github, hackernews, x_organic, dev_to, hashnode, discord_ai, podcasts_tier1, newsletter_sponsorships]
    forbidden: [tiktok_paid, instagram_organic]
autonomy: { level: medium, approval_required_above_usd: 500 }
kill_switches: [brand_voice_violation, platform_rate_limit_exceeded, goodhart_signal_detected, cost_velocity_exceeds_2x]
deadline_soft: 2026-09-01
```

### 1.4 The channel-mix opening allocation

Phase 1 (weeks 0-4): GitHub + HN + X-organic. Hormozi's "pick one, max it" applied to one quadrant of the Core Four: posting content.

Phase 2 (weeks 4-8): Add Discord engagement in 5 specifically-named AI engineer communities (operator-joined): Anthropic-builders, Cursor, LangChain, OpenAI-dev, MCP-builders. Add Dev.to + Hashnode cross-posting of GitHub-anchored deep dives.

Phase 3 (weeks 8-12): Add podcast appearances (Latent Space, MLST, No Priors, Last Week in AI as tier-1 targets; AI Engineering, MLOps Community as tier-2). Add 3 newsletter sponsorships (Latent Space, AI Engineering, The Batch).

Phase 4 (weeks 12+): Cold enterprise outreach to AI Platform / DevOps leads at AI-using Fortune 1000. Paid X Ads on developer-targeted keywords. LinkedIn Ads on AI/ML Platform Lead titles.

### 1.5 The viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md §3.1`:
- **Content / UGC loop:** every successful verification produces a sharable badge. Badge embeds referral ID. Developers share on PRs and X organically.
- **Template loop:** community rule libraries (Django-specific, Rails-specific, Next.js-specific verification rubrics) each require ContextGraph to run.
- **Two-sided incentive overlay:** 1 verification credit per converted referral up to 20/mo; 5 bonus credits for new signups via referral.

### 1.6 The DevRel flywheel

Per `12_DEVELOPER_MARKETING.md`:
1. **Notebooks** — open-source Jupyter/Colab notebooks demonstrating ME-JEPA verification on real GitHub patches.
2. **Open-source refs** — example MCP server integrations with Cursor/Codex/Claude Code as standalone repos.
3. **Hackathons** — sponsor 3 hackathons in 90 days targeting AI engineering audiences; provide $500 prizes for best ME-JEPA-Code usage.
4. **Content DevRel** — long-form blog content explaining the predictor architecture, the SWE-bench results, the deployment patterns. Goal is citation in other developers' content + AI search citations.
5. **Integrations guild** — formal partnership program with Cursor, Anthropic, OpenAI, Continue.dev, Aider.

### 1.7 The Elite tier (T3) plan

Mapper-identified Tier-3 targets:
- AI lab leadership (Anthropic, OpenAI, Meta AI, Google DeepMind verification-adjacent leads)
- AI agent platform leaders (Cursor founders, Replit, Devin/Cognition)
- AI infrastructure VC partners (Sequoia, a16z, Index, NEA AI-focused partners)
- Recognized AI safety / verification researchers (named individuals: Andrew Ng, Yejin Choi, Jacob Andreas, etc.)
- Frontier AI evaluation researchers (METR, Apollo, AISI)

Goal: 1 T3 conversation per week minimum (operator-mediated; warm-intro pathed). Path to enterprise pilots runs through T3 endorsement.

### 1.8 90-day predicate target

Realistic predicate trajectory:

| Week | `weekly_active_integrations` target |
|---|---|
| 4 | 100 |
| 8 | 500 |
| 12 | 2,000 |
| 16 | 5,000 |

The 50,000 target is a 9-12 month goal, not a 90-day goal. The 90-day plan establishes the *trajectory*. A goal spec covering 90 days would set threshold at 2,000, with the 50,000 as a follow-on goal.

---

## §2. Synapse

### 2.1 The product (recap from CLAUDE.md / synapse systemspec)

A Rust MCP server that exposes a Windows 11/10 desktop's perception (capture, UIA, OCR, audio) and accepts high-level action intents (click, type, aim, press, drag, combo), and runs sub-frame reflex controllers. The shipping binary is `synapse-mcp`. Operator's brain-meets-body: AI agents call Synapse for computer-use.

### 2.2 The Grand Slam Offer

**Dream outcome:** "My AI agent can actually use my computer — see the screen, click the button, type the message, drive the game — as well as I can."

**Perceived likelihood:** "Open-source Rust core, MCP-native, used by Anthropic / Cursor / Codex sessions in production. Benchmarked against the operator's own hand operating the same flow."

**Time delay:** "Install + first agent computer-use action in under 5 minutes. Cross-platform agent automation works on day one."

**Effort & sacrifice:** "MCP. No new SDK. No new framework. Drop into your existing agent loop. Free for personal use."

**The offer:**
- Free forever for personal / open-source / non-commercial use.
- $29/mo individual Pro: ViGEm controller, advanced hotkey support, priority support, encrypted session recording.
- $499/mo team: shared automation playbooks, multi-machine fleet, audit trails.
- Enterprise: on-prem deployment + SLA + custom integrations.
- **Guarantee:** "If your agent can't complete a basic Notepad workflow in under 60 seconds on day 1, we'll personally walk through setup with you on a screen-share and get it working. Don't waste your evening fighting Win32." (the time-to-first-value guarantee.)
- **Lead magnet:** a curated "100 MCP Automation Playbooks" Github repo, free, no opt-in. Includes Polis, ContextGraph examples and competitor (Browser-Use) interop.

### 2.3 Goal spec template

```yaml
goal_id: synapse-25k-q3-2026
product: synapse
target:
  metric: weekly_active_mcp_sessions_with_depth
  formula: "count distinct sessions in 7d with >= 3 distinct tools called and duration >= 60s"
  threshold: 25000
  evaluation_window: 28d
  consecutive_windows_required: 4
guardrails:
  budget: { monthly_ceiling_usd: 20000, daily_velocity_cap_usd: 1000 }
  unit_economics: { ltv_cac_floor: 3.5, cac_payback_ceiling_months: 12 }
  brand: { voice_spec_path: docs/brandvoices/synapse.md }
  compliance: { jurisdictions: [US, EU, CA], ai_disclosure_required: true }
  channels:
    preferred: [github, hackernews, x_organic, mcp_builder_discords, ai_agent_hackathons]
autonomy: { level: medium, approval_required_above_usd: 500 }
deadline_soft: 2026-09-01
```

### 2.4 The channel-mix opening allocation

Phase 1: GitHub + HN + X-organic + MCP-builder Discord. Synapse is a primitive in the MCP ecosystem; the ecosystem itself is the early channel.

Phase 2: Hackathons (AI agent hackathons specifically: AGI House, AI Engineer Summit, MCP hackathons). YouTube demos of Synapse + agent combos solving real workflows (game playthroughs, accessibility flows, automation demos).

Phase 3: Podcast appearances on AI agent–themed shows (Latent Space, MCP-specific, Lex Fridman if reachable). Integrations guild: ensure Synapse is the default computer-use backend for popular AI agent frameworks.

Phase 4: Enterprise via OS-RPA-disruption angle (UIPath/Automation Anywhere replacement at one-tenth the cost). Cold outreach to Operations leaders at AI agent companies.

### 2.5 The viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md §3.2`:
- **Demo / UGC loop:** every successful long-running Synapse session exportable to a sharable video. "Built with Synapse" + referral ID embedded.
- **Playbook loop:** community library of Synapse automation scripts; each install needs Synapse.
- **Hackathon loop:** sponsored hackathons → winning projects amplified → audience installs to replicate.

### 2.6 The Elite tier plan

Mapper-identified Tier-3 targets:
- MCP ecosystem leadership (Anthropic MCP team, MCP early contributors)
- AI agent platform founders (Cognition/Devin, Cursor, Replit Agent team)
- Accessibility advocates (named individuals in the accessibility tech community)
- OS-RPA decision-makers at named Fortune 1000 (Operations / Automation lead titles)

### 2.7 90-day predicate target

| Week | weekly_active_mcp_sessions_with_depth |
|---|---|
| 4 | 200 |
| 8 | 800 |
| 12 | 2,500 |
| 16 | 6,000 |

---

## §3. Polis / Serendipity Engine

### 3.1 The product (recap from docs/serendipityengine/)

A social platform that re-engineers around physical / synchronous gathering, friend-density network effects, debate, knowledge growth, and intellectual love. Anti-engagement-farming by design. Five gathering formats (salon, debate, third-place revival, knowledge-growth sessions, love-layer experiences). Sortition governance.

### 3.2 The Grand Slam Offer

**Dream outcome:** "I find people who actually want to talk about what I care about, in real life, on a regular cadence — not doom-scroll dopamine."

**Perceived likelihood:** "Curated friend-of-friend network in your city. Three intellectual / artistic / debate formats every week within walking distance. Show-up rates 65%+."

**Time delay:** "First event within 7 days of signup in seed cities. Friend-cluster activation in 30 days."

**Effort & sacrifice:** "Free to attend events. No algorithm to fight. No content quotas. Just RSVP and show up."

**The offer:**
- Free for attendees in seed cities.
- $9/mo Polis Plus: early access to high-density curated events, host-prize draws.
- $29/mo Polis Host: tools to host events in your city.
- $99/mo Polis Curator: tools for editorial-grade events; revenue share on premium events.
- **Guarantee:** "If you attend 3 Polis events and don't make a meaningful new connection, we'll cover the cost of a small dinner at a local spot for you and a friend you've already met. We back the network's signal-density." (Hormozi-style "if you don't X, I will Y.")
- **Lead magnet:** the "Third Place Revival Guide" — a free practical guide on how to host intellectual gatherings (download required; converts to Polis signup at 18-25%).

### 3.3 Goal spec template

```yaml
goal_id: polis-50k-attendance-q3-2026
product: polis
target:
  metric: monthly_active_event_attendances
  formula: "count distinct (user, event) pairs in 28d with attended=true"
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
guardrails:
  budget: { monthly_ceiling_usd: 30000, daily_velocity_cap_usd: 1500 }
  unit_economics: { ltv_cac_floor: 4.0, cac_payback_ceiling_months: 9 }
  brand: { voice_spec_path: docs/brandvoices/polis.md }
  compliance: { jurisdictions: [US, EU, CA, UK, AU], ai_disclosure_required: true }
  channels:
    preferred: [tiktok_organic, instagram_organic, substack_intellectual, podcasts_culture, campus_ambassadors, x_organic, in_person_events]
    forbidden: [linkedin_ads, programmatic_seo]
autonomy: { level: medium, approval_required_above_usd: 500 }
kill_switches: [brand_voice_violation, attendance_rate_below_0.50, goodhart_signal_detected, complaint_velocity_exceeds_threshold]
deadline_soft: 2026-09-01
```

### 3.4 The channel-mix opening allocation

Phase 1: TikTok + Instagram (organic) + campus ambassador pilot in 3 cities (Austin, Boston, NYC). Hormozi-volume on TikTok: 3-5 organic uploads / day per persona.

Phase 2: Substack intellectual creators (interview swaps, host-each-other on respective newsletters), podcast tour on culture / intellectual shows (Lex Fridman, Tyler Cowen, Sam Harris, Russ Roberts if reachable; tier-2: Plain English, The Knowledge Project, EconTalk).

Phase 3: Geographic expansion to 10 cities; ambassador program scales. Paid Meta + TikTok Ads in proven cities only (after organic CAC reaches LTV:CAC floor).

Phase 4: Partnerships with intellectual creators (revenue share on referred subscribers); Polis Curator launches for editorial-grade event hosting.

### 3.5 The viral mechanic

Per `09_VIRAL_LOOPS_AND_REFERRALS.md §3.3`:
- **Network-effect loop:** events get richer with more friends. Friend invites are the core product action.
- **Campus ambassador program:** BeReal-pattern. $30/successful referral, $50/install+attendance. Targets dense friend clusters in seed-city universities.
- **Two-prize giveaway pattern (Hormozi/Skool):** every event has a host-prize draw + referrer-prize draw; "if your referee wins, you win."

### 3.6 The Elite tier plan

Mapper-identified Tier-3 targets:
- Public intellectuals with engaged audiences (Tyler Cowen, Sam Harris, Russ Roberts, Patrick Collison, Marc Andreessen — both for ideas and capital)
- Curatorial figures (museum directors, intellectual conference organizers, Sun Valley / Allen & Co / Davos attendees who care about civic / cultural revival)
- Third-place revival movement leaders (Strong Towns adjacent, urbanism intellectuals)
- Major Substack writers with intellectual / cultural focus

### 3.7 90-day predicate target

| Week | monthly_active_event_attendances |
|---|---|
| 4 | 200 |
| 8 | 1,500 |
| 12 | 6,000 |
| 16 | 18,000 |

The 50,000 target is a 6-9 month goal. Seed-city density first; expansion second.

---

## §4. Cross-product synergies

The three products are not independent. Synergies the fleet exploits:

### 4.1 Content cross-pollination

The operator is one person with one brand. ContextGraph posts about AI verification, Synapse posts about computer-use, Polis posts about IRL community — all under the operator's personal channels. The Persona Carrier ensures voice consistency where personas overlap (the operator's personal channel), and persona-specific voice where they diverge (Polis-specific account voice).

### 4.2 Audience overlap

The AI engineers who care about ContextGraph and Synapse also care about Polis (the intellectual ones; not all). Polis events in Bay Area / Boston / NYC seed cities likely have AI engineer attendees. Cross-mention: when the operator hosts a Polis salon on "AI Safety in Practice" → ContextGraph / Synapse audience attends.

### 4.3 Operator credibility compounding

A successful ContextGraph + Synapse track record makes the operator's Polis pitch more credible ("the operator who builds rigorous AI tools is also rebuilding third places"). The operator's brand benefits from each product's success.

### 4.4 Shared infrastructure

All three use the same agent fleet, same evaluator stack, same memory layer, same Leapable vaults. Cost-amortized.

---

## §5. The Hormozi "max one, then add" applied across products

The fleet does not run three goals at maximum velocity from day 1. Hormozi: *"Just pick one. Then max it out."*

Recommended starting order:
1. **Months 1-3:** ContextGraph only. Validate the engine (the agent fleet, the channel mix, the predicate measurement) against the product with the clearest predicate (developer integrations).
2. **Months 3-6:** Add Synapse. Reuse the same channel playbook (developer audiences overlap heavily); validate cross-product audience handoff.
3. **Months 6-9:** Add Polis. Different audience, different physics; the fleet has matured enough to handle a third concurrent goal.

This is operator preference + Hormozi compounding. Each product's success makes the next one cheaper to launch.

---

## §6. Cross-references

- Goal spec full grammar: `22_GOAL_SPEC_FORMAT.md`
- Channel inventory: `08_DISTRIBUTION_CHANNELS.md`
- Viral mechanics: `09_VIRAL_LOOPS_AND_REFERRALS.md`
- DevRel flywheel (CG + SY): `12_DEVELOPER_MARKETING.md`
- Consumer launch (Polis): `13_CONSUMER_SOCIAL_LAUNCH.md`
- 90-day execution: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Influence network mapping (T3 per product): `10_INFLUENCE_NETWORK_MAPPING.md`
- Metrics + predicates: `15_METRICS_AND_OBSERVABILITY.md`
