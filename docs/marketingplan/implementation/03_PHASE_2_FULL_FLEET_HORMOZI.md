# Phase 2 — Full Fleet + Hormozi Frameworks (Weeks 7-10)

> All 26 agent classes online; 8 channels live; full Content Factory pipeline (research → brief → draft → distribute → GEO → attribution); Hormozi vault deeply integrated (every Brief Generator, Referral Engineer, Outreach Drafter queries it before producing); first viral loop shipped (Leapable subscriber→subscriber + creator→subscriber); Cohort Health Auditor + LTV:CAC dashboard live. After Phase 2, the operator's daily output is 1 anchor + 12 derivatives across 8 channels with measurable LTV:CAC.

**Source spec docs:** `21_BUILD_ROADMAP.md §4`, `05_AGENT_FLEET_DESIGN.md` (all sections), `07_CONTENT_FACTORY.md`, `08_DISTRIBUTION_CHANNELS.md`, `09_VIRAL_LOOPS_AND_REFERRALS.md`, `12_DEVELOPER_MARKETING.md`, `15_METRICS_AND_OBSERVABILITY.md §3 §4 §7 §9`, `16_TECH_STACK.md §4.2 §5.1`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 4 weeks (20 working days) |
| **Prerequisites** | Phase 1 acceptance passed; first Leapable goal stable; brand voice scorer Kappa-validated |
| **Headline outcome** | 26 agents operational; 8 channels publishing; Hormozi-grounded copy; first referral loop shipped; LTV:CAC dashboard reading real numbers; Cohort Health Auditor flagging cohort decay if it happens |
| **What's NOT in this phase** | Synapse + Polis goal classes (Phase 3); Goodhart Auditor (Phase 3); Influence Network Mapper (Phase 3); production hardening (Phase 4) |
| **Acceptance gate to Phase 3** | All §5 criteria pass; Leapable Phase B (operational) traction visible — `weekly_active_subscribers` trending toward 1,500+ |
| **Operator time** | ~60-90 min/day; 30 min approvals + 30 min Tier-2 conversation + 30 min sample audits |

---

## §2. Deliverables — agents

### 2.1 Producing agents (P-tier)

| ID | Class | What ships in Phase 2 |
|---|---|---|
| **P1** | Content Writer | Already shipped; extended with anchor-archetype awareness per `07 §6` |
| **P2** | Technical Writer | New: deep-technical tutorials, READMEs, comparison content; same brand-voice gate; routes Opus on cornerstone pieces |
| **P3** | Visual Generator | New: images / diagrams / video stitching via Hyper MCP or DALL-E equivalent; brand-template enforcement |
| **P4** | Brief Generator | New: topic → structured brief per `07 §1 §4`; auto-includes Value Equation block per `07 §4`; queries Hormozi vault for offer-structure principles |
| **P5** | Persona Carrier | New: multi-persona handling per `05 §6.5`; non-leakage gates |

### 2.2 Distributing agents (D-tier; expanded from Phase 1)

| ID | Class | What ships in Phase 2 |
|---|---|---|
| **D1** | Multi-Platform Publisher | Extended from X-only to 8 channels: X, LinkedIn, GitHub, HN, Reddit, Dev.to, Hashnode, MCP-builder Discord |
| **D2** | Community Operator | New: Discord/Reddit engagement on operator-owned accounts within community playbooks |
| **D3** | GitHub Maintainer | New: issue triage, PR review, README maintenance for Leapable + Synapse repos; SLAs per `12 §3.3` |
| **D4** | Outreach Drafter | New: personalized cold + warm follow-up emails; drafts only, never sends without approval |
| **D6** | Comment Triage | New: replies to comments on operator's content within voice + escalation rules |
| **D7** | Newsletter Pitcher | New: pitches operator to host on other people's newsletters |
| **D8** | Podcast Booker | New: drafts pitches to podcast hosts; tracks 50-show tiered list |
| **D9** | Event Coordinator | New: hackathon / meetup RSVP + booth/talk pitch coordination |

D5 (Warm-Intro Drafter) deferred to Phase 3 (depends on Influence Network Mapper).

### 2.3 Sensing agents (S-tier; Phase 2 subset)

| ID | Class | What ships in Phase 2 |
|---|---|---|
| **S1** | Audience Sorter | New: scores incoming contacts T1/T2/T3 per `03 §3` weights; default T1 if no signal |
| **S2** | Signal Listener | New: polls social/web for mentions, references, competitor moves |
| **S5** | Competitive Intelligence | New: monitors competitor pricing, positioning, launches via Hyper MCP scrapes |
| **S6** | Sentiment Tracker | New: aggregates brand sentiment across mentions; tracks delta |

S3 (Influence Network Mapper) deferred to Phase 3. S4 (Goodhart Auditor) deferred to Phase 3.

### 2.4 Optimizing agents (O-tier; Phase 2 subset)

| ID | Class | What ships in Phase 2 |
|---|---|---|
| **O1** | SEO Strategist | New: keyword research, content-gap analysis, programmatic SEO templates per `12 §6` |
| **O2** | GEO Optimizer | New: structures content for ChatGPT/Claude/Perplexity citation per `07 §5.2` |
| **O3** | Referral Engineer | New: designs/tweaks viral loops; A/B tests referral mechanics; two-sided incentives per `09 §5` |
| **O4** | Ad Operator | New (skeleton; gated on operator approval): paid ad campaign management within budget envelope |
| **O6** | Cost Pacer | New (mechanical, not LLM): enforces dollar + token pacing per `02 §16` |
| **O7** | Cohort Health Auditor | New: tracks per-channel cohort retention; pauses referral programs if retention drops below floor per `09 §4` |

O5 (Conversion Optimizer) deferred to Phase 3 (depends on product team for landing-page A/B testing).

### 2.5 Meta agents (M-tier; upgrades from Phase 1)

| ID | Class | What changes in Phase 2 |
|---|---|---|
| **M7** | Evaluator | L4 LLM-as-judge rubrics added: quality, helpfulness, on-rubric coherence, factuality per `05 §3` |
| **M9** | Communicator | Daily digest enhanced with channel-by-channel breakdown + Hormozi-vault citations used this cycle |

M10 (Wiki Gardener) deferred to Phase 4.

---

## §3. Deliverables — channels

### 3.1 Channel cards (per `08 §5`)

Author channel cards for each of the 8 channels:

| Channel | Card path | Quadrant | Tier |
|---|---|---|---|
| X organic | `channel_cards/x.yaml` (extended from Phase 1) | Posting Content | T1, T2 |
| LinkedIn personal | `channel_cards/linkedin.yaml` | Posting Content | T2 |
| GitHub | `channel_cards/github.yaml` | Posting Content | T2 |
| Hacker News | `channel_cards/hackernews.yaml` | Posting Content | T2 |
| Reddit | `channel_cards/reddit.yaml` (per subreddit sub-cards) | Posting Content | T2 |
| Dev.to | `channel_cards/devto.yaml` | Posting Content | T2 |
| Hashnode | `channel_cards/hashnode.yaml` | Posting Content | T2 |
| MCP-builder Discord | `channel_cards/discord_mcp_builders.yaml` | Posting Content / 1-on-1 | T2 |

Each card carries:
- `operator_accounts` (the operator's registered account for that channel)
- `rate_caps` (30% below platform thresholds per `06 §4`)
- `posting_cadence_distribution` (stochastic; log-normal where applicable)
- `content_format_rules` (per-channel format)
- `detection_signals_to_avoid` (anti-detection list)
- `successful_skills_path` (link to a `skills/<channel>.md` playbook)
- `last_audit_date` (initially Phase 2 ship date)

### 3.2 Channel-specific tool wrappers

For each channel, wrap the publish tool with:
- Idempotency key
- Rate-limit check via Cost Pacer
- Brand voice gate
- Compliance check (jurisdiction-aware; EU disclosure where applicable)
- Schema match (per-channel payload schema)
- Audit log emission

Implementations:
- X, LinkedIn, Discord, HN — Synapse-mediated (login required)
- GitHub — gh CLI via direct API
- Reddit, Dev.to, Hashnode — Hyper MCP or direct API where available
- Email (for D4 outreach) — Gmail / MS Graph via direct API

---

## §4. Deliverables — Hormozi vault integration

This is the central Phase 2 leverage point. Per `16 §4.2`:

### 4.1 Vault setup confirmation

The `AlexHormoziTranscripts` vault already exists on operator's leapable instance (1,150 docs, 6,428 chunks). Phase 2 wires it into:

| Consumer agent | What it queries | When |
|---|---|---|
| **P4 Brief Generator** | Offer-structure principles, Value Equation patterns, Grand Slam Offer templates | Before every brief |
| **P1 Content Writer** | Hormozi tone exemplars, copy-structure templates | When archetype = comparison / postmortem / behind-the-numbers |
| **D4 Outreach Drafter** | "If you don't X, I will Y" guarantee structure; Hormozi cold-outreach one-liner | Before every outreach draft |
| **D8 Podcast Booker** | Hormozi-style pitch structure for podcast hosts | Before every pitch |
| **O3 Referral Engineer** | Two-prize giveaway pattern (Skool.com); two-sided incentive design; client-financed acquisition rule | Before every referral mechanic design |
| **O4 Ad Operator** | Hormozi paid-ad principles; ad-creative structure | Before every ad-creative variant |
| **M7 Evaluator (L4)** | Hormozi quality rubric for value-equation expression | Per draft scoring |

### 4.2 Vault access policy

In each agent card, declare:
```yaml
vaults_allowed:
  - AlexHormoziTranscripts
  - LeapableProductDocs  # for P2 grounding
  - DevRelLiterature      # for D2/D3 grounding
```

Tool Gateway enforces: agent cannot query a vault not on its allowlist.

### 4.3 Vault-query caching

Common queries (e.g., "value equation structure") cached at the per-cycle level to avoid re-querying. Cache invalidated on vault update.

### 4.4 New vaults to build during Phase 2

Operator-time investment to curate:

| Vault | Purpose | Operator-time to build |
|---|---|---|
| `OperatorBrandVoices` | All operator-approved + operator-rejected content; auto-populated by Brand Voice Scorer + Approval Queue | Auto |
| `ProductTechSpecs` | Leapable, Synapse CLAUDE.md + relevant docs; for P2 Technical Writer factual grounding | ~2h to assemble + publish |
| `DevRelLiterature` | FORKOFF, 5W, Joe Karlsson, public DevRel playbooks | ~3h to assemble + publish |
| `KarpathyCorpus` | Karpathy talks, blog, AutoResearch, AgentHub, Software-3.0 keynote | ~4h to assemble + publish |
| `ComplianceCorpus` | CAN-SPAM, GDPR, CCPA, CASL, EU AI Act, platform ToS texts | ~3h to assemble + publish |
| `CompetitiveIntel` | Public docs from named competitors (Goodfire, etc.) | ~2h to assemble |

Phase 2 ships with first 3 of 6 vaults; remaining 3 over Phase 3.

---

## §5. Deliverables — first viral loop (Leapable)

Per `09 §3.1` + `11 §1.5`:

### 5.1 Mechanic shipping

**Subscriber → Subscriber + Creator → Subscriber loops (the Leapable-side referral substrate):**
- Per-subscriber referral ID embedded in every Ask answer share-link
- Landing page on `leapable.ai/r/<referral_id>` with "Sarah asked Hormozi this; you can too — 5 bonus credits for new subscribers"
- Two-sided incentive:
  - Referrer: +1 verification credit per converted referral, capped at 20/month
  - Referee: 5 bonus credits on top of free tier
- Stripe Connect rev-share already exists for Creator side; no product change needed

### 5.2 Product-side coordination

Some elements require product team work (not agent-shippable per `02 §7`):
- Per-Ask share-link UI
- Per-subscriber referral ID provisioning
- Bonus credit accounting

**Phase 2 ship-condition:** product team agrees on the mechanic + builds the UI; the agent fleet ships the landing-page A/B copy, referral-link tracking + attribution, and incentive-redemption email sequence.

### 5.3 Measurement

K-factor + cycle-time tracked from week 1 of mechanic live. Targets per `15 §3.4`:
- K-factor ≥ 0.2 (floor) → working
- K-factor target 0.4 within 90 days
- Cycle time target 5 days

Below floor → Cohort Health Auditor pauses program (per `09 §4`).

---

## §6. Deliverables — LTV:CAC dashboard

Per `15 §3 §6 §9`:

### 6.1 Cohort tracking

Cohort Health Auditor (O7) computes for every channel × goal × 28-day cohort:
- Acquisition cost (paid CAC + amortized organic CAC)
- 28-day retention rate
- 28-day realized LTV
- 90-day projected LTV (via cohort decay model)
- LTV:CAC ratio

### 6.2 Dashboard UI

Operator dashboard shows live per-channel LTV:CAC per `15 §6`:
- Active goals + predicate progress
- Per-channel CAC, LTV, ratio
- LTV:CAC floor violations highlighted
- Recommended channel reallocation if any channel breaches floor for 4 weeks

### 6.3 Client-financed acquisition check

Per `09 §6` + `15 §3.5`: `realized_30d_gross_profit_per_acquired_user >= CAC + 30d_fulfillment_cost`

- `TRUE` → paid acquisition can scale (Phase 2-end Ad Operator can light up)
- `FALSE` → paid acquisition capped at cash float

---

## §7. Week-by-week breakdown

### Week 7 — Producing + Brief agents + Hormozi vault wiring

**Day 31-32 (Mon-Tue) — P2/P3/P4/P5 agent cards + implementations**
- Author cards for each.
- Wire P4 Brief Generator to query AlexHormoziTranscripts.
- Test: P4 produces a brief; vault-query result is included in brief's `evidence_pointers`.

**Day 33-34 (Wed-Thu) — D1 multi-channel extension**
- D1 extended from X-only to 8 channels.
- Channel cards authored for each.
- Per-channel transformation rules per `07 §9`.

**Day 35 (Fri) — Discord + GitHub + HN computer-use validation**
- Synapse-mediated login + post test on each platform.
- Rate caps verified.
- Audit log captures each.

### Week 8 — Sensing + Optimizing skeleton + first multi-channel cycle

**Day 36-37 (Mon-Tue) — S1/S2/S5/S6 sensing agents**
- S1 Audience Sorter wired to LinkedIn/Crunchbase via Hyper MCP.
- S2 Signal Listener polls for operator-name + product-name mentions.
- S5 + S6 ship with daily cadence.

**Day 38 (Wed) — O1/O2/O6/O7 optimizing agents (excl. O3 referral, O4 ads)**
- O1 SEO Strategist + programmatic SEO template generator.
- O2 GEO Optimizer ships citation-friendly structure transforms.
- O6 Cost Pacer goes mechanical (token-bucket + dollar-bucket per channel).
- O7 Cohort Health Auditor starts tracking; first per-channel cohort data after week.

**Day 39-40 (Thu-Fri) — Full 8-channel cycle dispatch**
- Cycle generates 1 anchor + 12 derivatives.
- Brand-voice gates all 13 items; 8 publish (per channel rules); 5 enter approval queue.
- Audit log clean; operator approves the 5 within an hour.

### Week 9 — Outreach + Community + Hormozi-grounded copy

**Day 41-42 (Mon-Tue) — D2 + D4 + D6 outreach/community agents**
- D2 Community Operator ships per-Discord playbooks.
- D4 Outreach Drafter ships per `09 §5.3` Hormozi guarantee structure.
- D6 Comment Triage replies on operator's content within voice rules.

**Day 43 (Wed) — D7/D8 newsletter + podcast pitchers**
- D7 pitches operator to other newsletters.
- D8 maintains tiered podcast list; drafts pitches.
- First 5 pitches to Latent Space, MLST, AI Engineering podcast, Practical AI, Lex Fridman.

**Day 44-45 (Thu-Fri) — Hormozi vault deep-integration testing**
- For each of P1, P4, D4, D8, O3, O4: verify vault query happens; verify cited principles appear in output; operator sample-audits 5 outputs per agent for Hormozi-fidelity.

### Week 10 — Viral loop + Cohort Auditor + Acceptance

**Day 46-47 (Mon-Tue) — O3 Referral Engineer + first viral loop**
- Operator + product team ship the in-app share-link UI for Leapable.
- O3 designs the two-sided incentive copy + landing page.
- Referral-link attribution + cycle-time tracking live.

**Day 48 (Wed) — LTV:CAC dashboard go-live**
- Dashboard reads real per-channel cohort data.
- First operator review of dashboard.
- Cost Pacer enforces budgets per channel.

**Day 49-50 (Thu-Fri) — Phase 2 acceptance**
- 10 days of clean 8-channel operation.
- Daily output: 1 anchor + 12 derivatives.
- K-factor measurable (≥ 0.05 in first 14 days of mechanic; ≥ 0.20 floor expected by Phase 2 end).
- LTV:CAC visible for at least 2 channels.
- Hormozi vault cited in ≥80% of qualifying outputs (briefs, outreach, referral copy).
- Phase 2 acceptance file written; operator sign-off.

---

## §8. Phase 2 architecture decisions to lock

| Decision | Recommended default | When operator can override |
|---|---|---|
| Vault caching TTL | **1 cycle (hourly default)** | Increase to 24h for stable vaults like Hormozi |
| K-factor floor | **0.20 (Leapable); pause referrals below** | Per goal; never disable the pause behavior |
| Cohort retention floor (Leapable paid) | **70% 28d / 50% 90d** | Cannot loosen below `15 §3.3` doctrine |
| First viral-loop tracking attribution | **30-day cookie + signed referral ID** | Defer to product team |
| LTV projection horizon | **12 months** | Per goal |
| LLM-as-judge rubric set (initial) | **Brand voice + factuality + helpfulness** | Add more per phase 4 calibration |
| Channel transformation rules | **Per `07 §9` table** | Per channel card; never deviate from rate caps |

---

## §9. Acceptance criteria (gate to Phase 3)

Phase 2 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | Phase 1 acceptance still passes (regression) | Re-run Phase 1 tests |
| 2 | All 26 agent classes operational (cards + impl + tested) | Per-agent integration test pass |
| 3 | 8 channels publishing per their cards; no rate-limit warnings in 14 days | Channel-card audit + platform-feedback log |
| 4 | Hormozi vault queried by ≥6 distinct agent classes in past 14 days | CF_AUDIT_LOG aggregate |
| 5 | Daily output: 1 anchor + ≥10 derivatives across ≥6 channels | Cycle log |
| 6 | Brand voice gate fires correctly; ≥80% first-attempt pass rate | Evaluator log |
| 7 | LTV:CAC dashboard reads real numbers for ≥2 channels; ≥1 channel has 28-day cohort retention measurement | Dashboard screenshot |
| 8 | Viral loop K-factor ≥ 0.10 in first 14 days of mechanic going live | Cohort Auditor report |
| 9 | Cohort Health Auditor flagged ≥1 channel-cohort with sub-floor retention (or proves all clean); pause behavior demonstrated | Auditor log + operator review |
| 10 | Cost Pacer denied ≥1 over-budget spending request (test case) | Audit log |
| 11 | Operator's daily marketing-supervision time stabilized at ≤90 min | Operator self-report |
| 12 | First Leapable goal predicate trending toward 1,500+ `weekly_active_subscribers` (Phase B target per `17 §3.2`) | SoT readback |

**Evidence file:** `marketing-fleet/evidence/phase_2_acceptance.md`.

---

## §10. Risks specific to Phase 2

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Rate-limit warning on a new channel (Discord, Reddit, etc.) | High | Medium | 30%-below-detection caps; immediate cap reduction on warning; widen cadence; investigate before resuming |
| Hormozi vault citations feel generic / non-applicable | Medium | Medium | Operator sample-audits 5/week; tune vault-query templates; expand vault if gaps found |
| Viral loop K-factor falls below 0.1 in first 14 days | Medium | High | Don't ship more channels; iterate on referral mechanic copy + landing page first |
| LTV:CAC unfavorable on a paid channel | Medium | High | Cost Pacer pauses that channel; investigate cohort quality; raise prices before lowering CAC per `15 §10` |
| Cohort retention drops below floor on a referred cohort | Medium | High | Auditor pauses referral program automatically; root cause: re-tune mechanic |
| Operator-approval queue grows beyond 30/day | Medium | Medium | Promote action classes to LOG with sampled audit; reduce volume |
| Cross-channel content drift (the same anchor reads differently across channels) | Medium | Low | D1 transformation rules are deterministic; lint-check anchor-derivative consistency |
| Phase 2 timeline slip (engineering of 26 agents is substantial) | Medium | Medium | Ship sub-batches of agents (P-tier week 7, D-tier week 8-9, S/O-tier week 9-10); operator-facing capability appears progressively |

---

## §11. Operator decisions required during Phase 2

| Decision | Day deadline | Default |
|---|---|---|
| Authenticate each new channel on Synapse / API | Day 35 (X, LI, Discord, HN by then; rest by day 40) | Phase 2 partial-blocked per channel |
| Approve first viral-loop incentive economics | Day 46 | Use `09 §5` defaults |
| Approve in-product share-link UI (product team work) | Day 46 | Block referral mechanic ship |
| Approve channel mix for goal (operator commits weekly approval rhythm) | Day 50 | Use `08 §3.7` Leapable defaults |
| Sign off on 3 new vaults (`ProductTechSpecs`, `DevRelLiterature`, `OperatorBrandVoices`) | Day 50 | Phase 2 ships without; effectiveness lower |
| Phase 2 acceptance sign-off | Day 50 | Phase 3 blocked |

---

## §12. What Phase 2 explicitly does NOT do

- Synapse + Polis goal classes (Phase 3)
- Influence Network Mapper / Warm-Intro Drafter / Pre-Call Briefer / Follow-Up Tracker (Phase 3 — Tier-3 work)
- Goodhart Auditor (Phase 3)
- Multi-product cross-goal scheduler (Phase 3)
- Bench-delta gate harness (Phase 4)
- LangSmith full integration (Phase 4)
- Wiki Gardener pruning (Phase 4)
- LLM-as-judge quarterly recalibration cadence (Phase 4)
- Self-audit reports (Phase 4)
- Vacation / Incident modes (Phase 4)
- Per-product analytics export (Phase 4)
- Conversion Optimizer landing-page A/B (Phase 3 with product team)

---

## §13. Cross-references

- Full agent catalog: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §1`
- Channel inventory + cards: `docs/marketingplan/08_DISTRIBUTION_CHANNELS.md`
- Viral loop math + Leapable mechanic: `docs/marketingplan/09_VIRAL_LOOPS_AND_REFERRALS.md`
- DevRel flywheel (P2/D3 grounding): `docs/marketingplan/12_DEVELOPER_MARKETING.md`
- Content Factory pipeline: `docs/marketingplan/07_CONTENT_FACTORY.md`
- Computer-use rules per channel: `docs/marketingplan/06_COMPUTER_USE_PLAYBOOK.md §8`
- LTV:CAC + cohort + Hormozi unit economics: `docs/marketingplan/15_METRICS_AND_OBSERVABILITY.md §3 §9 §10`
- Vault integration: `docs/marketingplan/16_TECH_STACK.md §4.2`
- Prior phase: `02_PHASE_1_SINGLE_PRODUCT_E2E.md`
- Next phase: `04_PHASE_3_MULTI_PRODUCT.md`
- Concurrent operational launch: `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md` Phase A → B transition
