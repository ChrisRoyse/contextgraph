# Phase 5 — Leapable 90-Day Operational Launch (Days 0-90 / Weeks 4-16 concurrent)

> The first real launch. Runs concurrent with engineering phases M1–M4. Phase A (validation, days 0-30) executes posted-and-ghosted alongside Phase 1 engineering. Phase B (compounding, days 30-60) runs on Phase 2 + 3 engineering. Phase C (mega-launch, days 60-90) coordinates with Phase 4 hardening. Goal: `weekly_active_subscribers ≥ 3,000` over last 28d. Operator-time-heavy; the agent fleet scales the work but does not replace the founder voice.

**Source spec docs:** `11_PRODUCT_PLAYBOOKS.md §1`, `17_LAUNCH_PLAYBOOK_90DAY.md §3 §7 §8`, `09_VIRAL_LOOPS_AND_REFERRALS.md §3.1`, `12_DEVELOPER_MARKETING.md`, `08_DISTRIBUTION_CHANNELS.md §3 §7.1`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 90 days (engineering weeks 4-16) |
| **Engineering concurrency** | Day 1-21 uses Phase 1 capability; Day 22-49 uses Phase 2; Day 50-91 uses Phase 3+4 |
| **Predicate target** | `weekly_active_subscribers ≥ 3,000` over last 28d, evaluated as Phase 1 of long-horizon 50k goal |
| **Operator time** | 30-40 hr/wk Phase A; 25-35 hr/wk Phase B; 50-60 hr/wk during Phase C launch week; 25-35 hr/wk surrounding weeks |
| **Acceptance gate** | All §11 criteria pass; predicate sustained ≥ 28d at threshold |
| **Risk profile** | High — first product launch; first test of the fleet against real reality |

---

## §2. The 3-phase Hormozi pattern (recap from `17 §1`)

| Phase | Days | Pattern | Predicate target by end of phase |
|---|---|---|---|
| **A — Validation** | 0-30 | Posted-and-ghosted. No fanfare. Validate offer + channel mix. | 500 weekly_active_subscribers |
| **B — Compounding** | 30-60 | Add 2-3 channels. Build referral loop. | 1,500-2,500 weekly_active_subscribers |
| **C — Mega-launch** | 60-90 | PH + HN + X + newsletter + podcast tour in 7-day window | 3,000+ weekly_active_subscribers |

---

## §3. Phase A — Validation (Days 0-30)

### 3.1 Headline activities

- Foundations: brand voice doc, channel cards, vault pre-seed
- Content factory boot: 1 anchor/day, 8 derivatives/day on owned surfaces
- Operator personal founder-thread cadence (4-5/wk substantive)
- Community engagement (D2) in 5 named Discords; pure engagement, no product mention before week 6 (Joe Karlsson rule)
- D3 GitHub Maintainer responds to issues <24h
- D4 Outreach: 30 cold pitches to named experts with monetizable corpora

### 3.2 Week-by-week breakdown

**Week 1 (Days 1-7) — Foundations.**
- Goal spec drafted (`marketing-fleet/goals/leapable-500-launch-2026-q2.md`) per `02_PHASE_1 §2.6`
- Brand voice doc finalized; Scorer calibrated (Kappa ≥0.65) — synchronized with Phase 1 engineering Day 11-12
- leapable.ai website polish: install page, help page, marketplace page, brand pillars
- Discover marketplace pre-seeded with 3 reference Vaults: `AlexHormoziTranscripts` (live, 1,150 docs), `KarpathyCorpus` (new), `LeapableProductDocs` (self-referential)
- 3 launch notebooks per `12 §2.2`:
  - "Install Leapable + your first Ask in 60 seconds"
  - "Connect Leapable to Claude Code in 5 minutes"
  - "Publish your first Vault for monetization"
- GitHub README polish (signed-installer + 5-command quickstart)
- HN account warmed (operator karma ≥4,000)
- X / LinkedIn bios updated; pinned tweet refreshed
- Channel cards: x, github, hackernews, dev_to, hashnode, mcp_builder_discords, anthropic_dev, claude_subreddit, openai_dev
- 30 cold-outreach pitches drafted for named experts (creator-side acquisition)

**Week 2 (Days 8-14) — Content factory boot.**
- 1 anchor/day × 7 = 7 anchors (Hormozi-volume on owned surfaces)
- 8 derivatives × 7 = 56 derivatives to operator's X, LinkedIn, Dev.to, Hashnode, GitHub gists
- Operator personally posts 4-5 substantive X founder threads (not delegated per `12 §5.3`)
- Anchor topics: "Why I built Leapable" / "Source-grounded AI search vs RAG vs ChatGPT" / "How creators can monetize their corpus in 15 minutes" / "MCP-native knowledge layer story" / "Verifying every Ask with provenance"

**Week 3 (Days 15-21) — Community engagement + first creator outreach.**
- D2 activates in 5 named Discords; pure engagement; no promo before week 6
- D3 GitHub Maintainer: <24h response to every issue
- D6 Comment Triage: auto-publish below 0.85 brand-voice → operator approval queue
- Operator: 5 podcast pitches/day to AI search / RAG / agent shows
- D4 Outreach: first 30 cold pitches sent to named experts (Hormozi guarantee structure per `09 §5.3`)

**Week 4 (Days 22-30) — Notebook scaling + first HN submission + Phase A predicate review.**
- 5 more notebooks (week 4 total: 8 live)
- First Show HN: substantive deep-dive (organic-validated; no upvote orchestration); operator hand-comments within 5 min
- Mid-phase review at Day 30: target 500+ `weekly_active_subscribers`

### 3.3 Phase A end-state

| Metric | Target |
|---|---|
| `weekly_active_subscribers` | ≥ 500 over last 7d |
| Active Discords engaged | ≥ 5 |
| Notebooks live | ≥ 8 |
| Anchors published | ~30 |
| Derivatives published | ~240 |
| Expert pitches sent | ≥ 30 |
| Creator-side conversations active | ≥ 3 |

### 3.4 Engineering dependencies satisfied by Day 30

Phase A requires engineering Phase 1 to be done by Day 21 at the latest:
- Brand Voice Scorer operational
- P1 + D1 publishing to X (and ideally GitHub, Discord by Day 14)
- Approval Queue functional via Slack
- Worthlessness test enforcing
- Reflexion memory writing

If Phase 1 engineering slips → Phase A operational launch slips. Plan accordingly.

---

## §4. Phase B — Compounding (Days 30-60)

### 4.1 Headline activities

- Phase 2 channels light up: Dev.to + Hashnode + Reddit + newsletter sponsorships + first podcast booking
- Referral mechanic ships (subscriber → subscriber, creator → subscriber)
- First contextual product mentions in Discord (after 4 weeks of helpful engagement)
- First Creator-side win (named expert publishes Vault)
- Comparison content begins ("Leapable Vaults vs raw RAG vs ChatGPT memory")
- Mid-phase predicate target: 1,500+ `weekly_active_subscribers`

### 4.2 Week-by-week breakdown

**Week 5 (Days 31-37) — Phase 2 channels light up.**
- Dev.to + Hashnode + Reddit (r/LocalLLaMA, r/MachineLearning, r/ChatGPT) engagement scales (D2 now also engages, not just listens)
- First newsletter sponsorship (Latent Space OR The Batch OR TLDR AI — per O4 Ad Operator's allocation; operator-approved budget)
- First podcast booking confirmed (from Week 3-4 outreach)
- Referral mechanic ships per `09 §3.1` + `03_PHASE_2 §5`:
  - subscriber→subscriber landing pages
  - creator→subscriber attribution via Stripe Connect rev-share
  - Two-sided incentive: referrer +1 credit, referee +5 credits

**Week 6 (Days 38-44) — First contextual product mentions in Discord.**
- D2 has earned the right by 4w of helpful engagement
- Mentions contextual: Discord question where Leapable Vault is the right answer
- Operator monitors: any pushback from community moderators? If yes → pull back

**Week 7 (Days 45-51) — First Creator-side win + comparison content.**
- First named expert agrees to publish Vault. Operator-led conversation; D4 drafts the close.
- Comparison anchor: "Leapable Vaults vs raw RAG vs ChatGPT memory: 30 questions, scored"
- Operator's X / LinkedIn covers comparison with screenshots
- Per `07 §6` archetype mix tuning: more "comparison" + "behind-the-numbers" in Phase B

**Week 8 (Days 52-60) — Mid-phase review + second podcast booking.**
- Predicate target: 1,500+ `weekly_active_subscribers`
- Second podcast booking (tier-1: Latent Space / MLST / Lex Fridman if reachable)
- First Creator-Vault published on Discover (operator-curated + named-expert co-launch)

### 4.3 Phase B end-state

| Metric | Target |
|---|---|
| `weekly_active_subscribers` | ≥ 1,500-2,500 over last 28d |
| Creator-Vaults live | ≥ 2 |
| Newsletter sponsorships completed | ≥ 1 |
| Podcasts aired or queued | ≥ 2 |
| Referral mechanic K-factor | ≥ 0.20 (floor) |
| Channels actively publishing | ≥ 8 |

### 4.4 Engineering dependencies satisfied by Day 60

Phase B requires Phase 2 engineering complete by Day 50:
- All 26 agents operational
- 8 channels live
- Hormozi vault deeply integrated
- Referral Engineer (O3) shipped
- Cohort Health Auditor (O7) tracking
- LTV:CAC dashboard live

If Phase 2 engineering slips → Phase B suffers; the referral mechanic + cohort auditor are the most valuable Phase B levers.

---

## §5. Phase C — Mega-launch (Days 60-90)

### 5.1 Headline activities

- Coordinated multi-channel launch in a 7-day window (typically days 75-82)
- PH (Product Hunt) + HN (Hacker News) + X-thread + newsletter sponsorships + Reddit + podcast appearances aired
- 25-50 warm supporters pre-engaged (Mapper-identified Tier-2/3 bridges)
- 15-30 journalist pitches sent embargoed
- Coordinate with first 5 Creator-Vault co-launches
- Operator's 12-hour availability blocked on launch day

### 5.2 Week-by-week breakdown

**Week 9 (Days 61-67) — Launch prep.**
- Lock launch date (Tuesday/Wednesday, Day 75-80)
- Reserve PH URL slug
- Identify hunter / self-hunt decision
- Draft launch-day plan per `17 §7` (PH first, HN second, X thread third, newsletter fourth, Reddit fifth)
- 25 warm supporters confirmed (Mapper-identified)
- Email waitlist drafted with launch-day incentive (first 100 get Pro for 6 months)
- Coordinate launch with first 5 Creator-Vault co-launches (each Creator's audience is acquisition channel)

**Week 10 (Days 68-74) — Pre-launch warmth.**
- Operator's X: 3 substantive build-log posts (per `12 §5.3` build-in-public)
- Discord seeded with high-quality content referencing Leapable Vault use cases (natural; not promotional)
- 15 journalist pitches sent embargoed: tier-2 tech press, AI engineering newsletters

**Week 11 (Days 75-81) — Launch week.**
- Day 75 (Tue/Wed): PH submission at midnight PT; first maker comment within 5 min
- Day 75 +9h: Show HN submission
- Day 75: X-thread launched parallel with PH
- Days 75-77: Newsletter sponsorships go out (Latent Space, AI Engineering, TLDR AI)
- Days 76-78: Reddit Show-Off posts in r/MachineLearning, r/LocalLLaMA, r/ChatGPT
- Days 76-82: Podcast appearances aired (booked in Phase B)
- Days 78-82: Mid-launch operator-led AMA on X Spaces or Discord
- Day 76+: First 5 Creator-Vaults publish their existing-audience announcements (co-launch)

**Week 12 (Days 82-90) — Post-launch conversion.**
- 30-day aha-moment funnel for launch-traffic signups
- Email sequence to PH/HN signups (5 emails over 14 days)
- Sales calls with enterprise prospects shown during launch
- Referral program promoted to launch cohort
- Phase C predicate target: 3,000+ `weekly_active_subscribers` sustained over final 28d

### 5.3 Launch-day universal script (per `17 §7`)

```
00:00 PT (Tue/Wed) — Submit to PH. Operator first maker comment in <5 min.
00:05 PT — Operator's X-thread goes live. Pinned.
00:10 PT — D6 Comment Triage activated for PH comments + X replies.
00:15 PT — Discord communities notified.
03:00 PT — Mid-morning operator X update; engaging top PH commenters.
06:00 PT — Show HN submission. Hand-comment within 5 min.
09:00 PT — Newsletter sponsorships go out.
10:00 PT — First podcast appearance airs (pre-recorded).
12:00 PT — Reddit posts in 3 named subreddits (different content per sub).
13:00 PT — Operator engages every substantive comment for 4 hours.
15:00 PT — Mid-launch Twitter Spaces / Discord AMA (45 min).
18:00 PT — End-of-day wrap-up X post.
21:00 PT — Operator off. D6 + M9 handle overnight community.
```

D+1 to D+7: continued engagement, conversion-focused follow-up, no new launch surface.

### 5.4 Pre-launch warmth checklist (per `17 §8`)

30 days before mega-launch (must be checked by Day 60):

- [ ] Launch date locked, calendar-blocked
- [ ] PH URL reserved
- [ ] Hunter identified or self-hunting decided
- [ ] 25-50 warm supporters individually messaged
- [ ] Reflexion memory loaded (`launch-product-hunt.md` skill page) with prior outcomes
- [ ] 15-30 journalist pitches sent embargoed
- [ ] Lead magnet polished and tested ("100 Leapable Vault Integration Patterns" GitHub repo)
- [ ] Email waitlist with launch-day incentive prepared
- [ ] Operator's 12-hour availability calendar-blocked
- [ ] Slack/Discord notifications routed to operator on launch day
- [ ] First maker comment drafted (~150 words, substantive)
- [ ] Second comment drafted for first-hour Q&A
- [ ] X-thread drafted, 8-12 tweets, narrative arc, clear CTA
- [ ] All assets in correct format per channel
- [ ] Brand-voice evaluator pre-run on every launch asset

Any unchecked at Day 60 → postpone launch by one week.

### 5.5 Engineering dependencies satisfied by Day 90

Phase C requires Phase 3 + early Phase 4 engineering complete by Day 75:
- Goodhart Auditor (to validate predicate isn't being inflated by launch noise)
- Influence Network Mapper (to identify the 25-50 warm supporters)
- Warm-Intro Drafter (to draft the supporter outreach)
- Full kill-switch suite (in case launch goes sideways)
- Compliance enforcement (for launch-day email sequence)

---

## §6. The Grand Slam Offer details (recap from `11 §1.2`)

Three offers, one per audience:

### 6.1 Subscribers — "Ask any expert anything"

- **Free** install + 5 free Asks per Vault per month
- **$9/mo Subscriber** — unlimited Asks across all subscribed Vaults
- **Per-Vault paid plans** — set by Creator; 70/30 rev-share; one bundled bill
- **Guarantee:** "If you can't find the answer in any subscribed Vault in 30 days, full refund AND we point you to a Vault that does answer it."
- **Lead magnet:** AlexHormoziTranscripts free sample-Ask for 30 days

### 6.2 Creators — "Turn your corpus into a $10k/mo asset by Tuesday"

- **Free** Creator account, 1 Vault, 100 docs
- **$29/mo Creator Pro** — unlimited Vaults, marketplace promotion, analytics
- **$99/mo Creator Plus** — featured placement, custom pricing, AI landing pages
- **Guarantee:** "If your Vault doesn't earn back Creator Pro in 90 days, full refund + 90 more days promotion free."
- **Lead magnet:** "Publish-Ready Vault Checklist" PDF (20 pages, free, includes operator's AlexHormozi audit as worked example)

### 6.3 Developers — "Drop-in MCP knowledge layer"

- **Free** for personal / open-source / non-commercial
- Pro tier for commercial deployments
- **Guarantee:** "If your agent can't query a Leapable Vault in 5 min, we'll do integration on screen-share."
- **Lead magnet:** "100 Leapable Vault Integration Patterns" GitHub repo

---

## §7. Channel mix opening allocation (recap from `11 §1.4`)

### Layer 1 — Subscribers (T1+T2 mass)

| Phase | Channels |
|---|---|
| 0-4w | GitHub README, HN Show HN, X-organic founder thread, Dev.to cross-posts |
| 4-8w | Creator partnerships, podcast appearances on AI search/RAG/agent shows |
| 8-12w | Programmatic SEO landing pages, GEO-optimized content |
| 12w+ | Paid Google Ads on intent keywords, X/Reddit Ads |

### Layer 2 — Creators (T2+T3 intellectual experts)

| Phase | Channels |
|---|---|
| 0-4w | Operator-curated demo Vaults (Hormozi + Karpathy + 1-2 named experts) |
| 4-8w | Cold outreach to 50 named experts; pitches carry 90-day guarantee |
| 8-12w | Substack-creator interview swaps; creator-economy podcasts |
| 12w+ | Affiliate program for Creators (10% year-1 referred revenue) |

### Layer 3 — Developers (T2 MCP-native)

| Phase | Channels |
|---|---|
| 0-4w | MCP-builder Discord engagement, GitHub repo polish, first OSS reference |
| 4-8w | Hackathon sponsorships (AGI House, AI Engineer Summit, MCP-specific) |
| 8-12w | Integration partnerships (Claude Code, Cursor, Continue.dev, LangChain) |
| 12w+ | Conference talks; integration revenue-share |

---

## §8. Viral mechanic (recap from `11 §1.5`)

Per `09_VIRAL_LOOPS_AND_REFERRALS.md`:

| Loop | Mechanic | K target | Cycle time |
|---|---|---|---|
| Subscriber → Subscriber | "Ask shared with you by Sarah" landing pages | 0.4 | 5 days |
| Subscriber → Creator | "Found this in [Creator]'s Vault. Subscribe to ask more" | 0.2 | 14 days |
| Creator → Subscriber | Creators promote Vaults to existing audience; per-Creator referral codes | 0.3 | 14 days |
| Creator → Creator | Affiliate (10% year-1) | 0.1 | 30 days |
| Developer → Subscriber | "Powered by Leapable" credit on integrated agents | 0.05 | 30 days |

**Compound K across all 5 loops:** **0.5-0.7** within 90 days. Cycle time weighted average: **~7 days**.

---

## §9. Elite-tier plan (recap from `11 §1.6`)

Tier-3 targets via Influence Network Mapper:

- Named intellectual experts whose corpora become Vaults: Tyler Cowen, Sam Harris, Russ Roberts, Paul Graham (essays), Patrick Collison, Marc Andreessen, Tim Ferriss, Lex Fridman, Andrew Huberman
- AI lab leadership: Anthropic + OpenAI + Google DeepMind leads who care about provenance + citation
- VC partners: Sequoia, a16z, Index, NEA, GV
- Creator-economy investors: a16z Crypto/Games, Atelier Ventures

Goal: 1 T3 endorsement-quality conversation per week. Bridge cultivation per `10 §8`.

---

## §10. Operator commitment schedule

| Phase | Operator weekly hours |
|---|---|
| Phase A (Days 0-30) | 30-40 hours/week |
| Phase B (Days 30-60) | 25-35 hours/week |
| Phase C (Days 60-90) | 50-60 hours during launch week; 35-45 surrounding weeks |

Founder voice + Tier-3 conversations + live launch engagement are non-delegable per `19 §12`.

---

## §11. Acceptance criteria (gate to long-horizon Leapable goal)

Phase 5 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | `weekly_active_subscribers ≥ 3,000` sustained over last 28d | SoT readback from leapable RocksDB CF_QUERIES |
| 2 | ≥ 10 Creator-Vaults live on Discover | Marketplace query |
| 3 | Referral mechanic K-factor ≥ 0.20 (floor) sustained 4 weeks | Cohort Auditor report |
| 4 | At least 1 paid acquisition channel proves LTV:CAC ≥ 3.5 | Dashboard |
| 5 | 90-day cohort retention ≥ 50% on paid Subscribers | Cohort report |
| 6 | Goodhart Auditor reports clean for last 28d (no >2x divergence) | Auditor logs |
| 7 | Brand voice + compliance + rate-limit guardrails: zero unscheduled trips in last 28d | Audit log |
| 8 | At least 2 T3 endorsement-quality conversations completed | Operator log |
| 9 | At least 2 podcast appearances aired | Podcast list |
| 10 | At least 1 newsletter sponsorship completed with measurable acquisition | Attribution graph |
| 11 | Reflexion memory has ≥ 30 structured post-mortems from this launch | CF_REFLECTIONS aggregate |

**Evidence file:** `marketing-fleet/evidence/phase_5_leapable_launch_acceptance.md`.

---

## §12. Risks specific to Phase 5

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Engineering Phase 1 slips, Phase A operational launch delayed | Medium | Medium | Phase A can do operator-manual posting if P1/D1 not ready; less efficient but launch continues |
| First Show HN buried by competing announcement | High | Low | Reflexion: "never launch within 24h of major model-vendor announcement"; check before launch day |
| Referral mechanic K-factor below floor by Phase B end | Medium | High | Cohort Auditor pauses referrals; iterate landing-page copy + incentive; re-test |
| Creator-side acquisition slower than projected | Medium | Medium | Operator double-down on cold outreach; expand pitch pool from 50 to 100 |
| Discord moderator pushback on first product mention | Low | Medium | Pull back; longer pure-engagement runway; D2 reads room better next time |
| Launch-day operator availability collision (sickness, family event) | Low | High | Lock calendar 30 days out; second-best launch dates pre-identified |
| Goodhart trip during launch noise (acquisitions exceed retention by 2x) | Medium | High | Auditor pauses scaling; operator reviews; conversion funnel investigated |
| Brand-voice violation during launch high-volume | Medium | High | Brand-voice gate at 0.85 threshold; first launch all-hands operator review of every published item |
| Cold-outreach reply rate < 5% | Medium | Medium | Re-tune Hormozi guarantee structure; sample top 5 successful pitches; iterate |
| Engineering Phase 2 delivers Hormozi vault integration late | Medium | Medium | Phase B still ships with manual Hormozi-grounded copy; less leverage but launch continues |

---

## §13. Cross-references

- Per-product GTM details: `docs/marketingplan/11_PRODUCT_PLAYBOOKS.md §1`
- 90-day launch playbook (canonical): `docs/marketingplan/17_LAUNCH_PLAYBOOK_90DAY.md §3`
- Viral mechanic design: `docs/marketingplan/09_VIRAL_LOOPS_AND_REFERRALS.md §3.1`
- DevRel flywheel: `docs/marketingplan/12_DEVELOPER_MARKETING.md`
- Channel mix + ramp curves: `docs/marketingplan/08_DISTRIBUTION_CHANNELS.md §3 §7.1`
- T3 plan: `docs/marketingplan/10_INFLUENCE_NETWORK_MAPPING.md` + `11_PRODUCT_PLAYBOOKS.md §1.6`
- Operations cadence: `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md`
- Engineering phase that ships P1/D1: `02_PHASE_1_SINGLE_PRODUCT_E2E.md`
- Engineering phase that ships 8 channels + Hormozi vault + referral: `03_PHASE_2_FULL_FLEET_HORMOZI.md`
- Engineering phase that ships Goodhart Auditor + Mapper: `04_PHASE_3_MULTI_PRODUCT.md`
- Next operational launch: `07_PHASE_6_SYNAPSE_90DAY_LAUNCH.md`
