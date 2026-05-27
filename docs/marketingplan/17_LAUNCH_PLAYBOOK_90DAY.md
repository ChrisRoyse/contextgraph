# 17 — 90-Day Launch Playbook

> The cold-start sequence. Day 0 to Day 90, week by week, for each product. Borrowed from LaunchList's pre-launch waitlist 90-day playbook + the FORKOFF AI DevRel 90-day plan + Hormozi's posted-and-ghosted + mega-launch dual pattern + BeReal's single-city seed. The playbook produces the **first goal worth evaluating** — the 90-day spec that establishes the trajectory toward the longer-horizon predicates.

---

## §1. The Hormozi dual model

Hormozi shipped `$100M Offers` "posted and ghosted" — no launch fanfare. Four years later it has 26,000 5-star reviews; it grew on word of mouth. He shipped `$100M Leads` and `$100M Money Models` with mega-launches — $4M in ads, $7M in book sales pre-event.

Both patterns work for different goals. The fleet uses both:
- **Posted-and-ghosted** for the early validation phase (Day 0-30): no big announcement; ship the product to a small audience; let word of mouth verify the offer.
- **Mega-launch** for the visibility phase (Day 60-90): coordinated multi-channel push, Product Hunt + HN + X + newsletter + podcasts within a 7-day window.

The 90-day playbook walks through both.

---

## §2. The playbook structure (per product)

Each product has its own 90-day sub-playbook. The structure is universal:

| Phase | Days | Purpose |
|---|---|---|
| **Phase A — Validation** | 0-30 | Posted-and-ghosted. Earn first 100 activated users. Validate the offer + the channel mix. |
| **Phase B — Compounding** | 30-60 | Add 2-3 channels. Build the referral loop. Earn first 500 activated users. |
| **Phase C — Mega-launch** | 60-90 | Coordinated launch: PH + HN + newsletter + podcast tour. Press the 1,000+ activated users line. |

---

## §3. ContextGraph 90-day playbook

### 3.1 Phase A — Validation (days 0-30)

**Goal predicate (90-day):** `weekly_active_integrations >= 2,000` evaluated over the **last 28 days** of the period. Pure-organic.

**Week 1 — Foundations.**
- Goal spec drafted by operator; parsed by Goal Parser (M2); state initialized.
- Brand voice doc finalized (`brandvoices/contextgraph.md`); evaluator calibrated on 100 historical operator-authored samples (target Kappa ≥ 0.65).
- Three notebooks shipped: "Verify your first Cursor patch in 60 seconds", "Comparing ME-JEPA verdicts to oracle on 100 SWE-bench tasks", "Building a custom rule library for Django."
- GitHub repo polished (README, quickstart, SLA badges, contributing guide).
- HN account warmed (operator has 4,000+ karma; preserve).
- Operator's X bio updated; pinned tweet refreshed.
- Channel cards filled out for: github, hackernews, x_organic, dev_to, hashnode, discord (Anthropic-builders, Cursor, LangChain, MCP-builders, OpenAI-dev).
- leapable vault `AlexHormoziTranscripts` confirmed and accessible to agents. Quick test: Brief Generator pulls a Hormozi quote relevant to the day's anchor.

**Week 2 — Content factory boot.**
- 1 anchor / day × 7 days = 7 anchors. Hormozi-volume on owned surfaces.
- 8 derivatives per anchor × 7 days = 56 derivatives published to operator's X, Dev.to, Hashnode, GitHub gists, LinkedIn personal.
- Operator personally posts 4-5 substantive X/Twitter founder-thread posts per week (not delegated).

**Week 3 — Community engagement.**
- D2 Community Operator activates in 5 named Discords. Pure engagement: answer questions, no product promotion until week 6.
- D3 GitHub Maintainer responds to every issue within 24h.
- D6 Comment Triage handles replies on operator's content (auto-publish below 0.85 brand-voice score → operator approval queue).
- Operator: 1 podcast pitch sent per day (5/week) to tier-2 podcasts (relevance score ≥0.6).

**Week 4 — Notebook scaling + first HN submission.**
- 4 more notebooks shipped (week 4 total: 7).
- First Show HN attempt: a high-quality deep-dive that has been validated organically already (no upvote pre-orchestration). Operator hand-comments within 5 minutes of submission.
- Mid-month review: predicate evaluation. Target: 100+ weekly_active_integrations by Day 30.

**End of Phase A:** ~7 anchors, ~56 derivatives, 5 active Discords, 7 notebooks, 1 HN attempt, 20+ podcast pitches sent, ~100 weekly active integrations.

### 3.2 Phase B — Compounding (days 30-60)

**Week 5 — Add Phase 2 channels.**
- Dev.to + Hashnode + Reddit engagement scales.
- First newsletter sponsorship (Latent Space tier-1 OR The Batch — operator picks based on cost / fit).
- First podcast booking confirmed (from week 1-4 outreach).
- Referral mechanic shipped (`09_VIRAL_LOOPS_AND_REFERRALS.md §3.1`): badge + two-sided credits.

**Week 6 — First substantive product mention in Discord communities.**
- D2 has earned the right by 4 weeks of helpful engagement.
- Product mention is contextual: a Discord question for which ME-JEPA-Code is the right answer.

**Week 7 — Continued cadence + comparison content.**
- Comparison anchor: "ContextGraph vs. Goodfire vs. human review on 50 PRs."
- Operator's X / LinkedIn covers the comparison with screenshots.

**Week 8 — Mid-phase review + second podcast booking.**
- Predicate evaluation: target 500+ weekly_active_integrations.
- Second podcast booking; this time tier-1 (Latent Space / MLST / No Priors).

**End of Phase B:** ~500-700 weekly active integrations; first newsletter sponsorship live; 2 podcasts in queue or aired; 5+ HN/PH discussions about ContextGraph mention; referral mechanic active with first K-factor data.

### 3.3 Phase C — Mega-launch (days 60-90)

**Week 9 — Launch prep.**
- Lock the launch date (Tuesday or Wednesday, Day 75-80 range).
- Reserve PH URL slug.
- Identify hunter or commit to self-hunting.
- Draft launch-day plan: PH first, HN second (Show HN), X-thread third, newsletter fourth, Reddit fifth.
- 25 warm supporters confirmed (operator's bridges: Mapper-identified Tier-2/3 contacts who will engage at launch).
- Email waitlist drafted with launch-day incentive (first 100 get free Pro for 6 months).

**Week 10 — Pre-launch warmth.**
- Operator's X: 3 substantive posts about ContextGraph's "what we shipped this month" — building anticipation without launch-flag-waving.
- Discord communities seeded with high-quality content that references ME-JEPA-Code naturally.
- 15 journalist pitches sent (embargoed): tier-2 tech press, AI engineering newsletters.

**Week 11 — Final prep.**
- Launch-day script written: 12 hours of operator availability planned (6am PT to 6pm PT).
- All 25 warm supporters individually messaged with launch details + ask.
- Lead magnet polished: the "AI Coding Agent Verification Cookbook" PDF + free 80-page guide.

**Week 12 — Launch.**
- Day 75 (or Tuesday in week 12): PH submission at midnight PT (post-reset). Operator's first maker comment within 5 minutes.
- Day 75 +9h: Show HN submission (after PH momentum visible).
- Day 75: X-thread launched in parallel with PH.
- Day 75-77: Newsletter sponsorships go out (paid Latent Space, paid AI Engineering, paid TLDR).
- Day 76-78: Reddit Show-Off posts in r/MachineLearning, r/LocalLLaMA, r/programming.
- Day 76-82: Podcast appearances aired (the 2-3 booked in Phase B).
- Day 78-82: Mid-launch operator-led AMA on X Spaces or Discord.

**Day 80-90 — Post-launch conversion.**
- 30-day "aha moment" funnel for launch-traffic signups.
- Email sequence to PH/HN signups (5 emails over 14 days).
- Sales calls with enterprise prospects who showed during launch.
- Referral program promoted to launch cohort.

**End of Phase C:** 2,000+ weekly active integrations sustained over the final 28 days. Predicate evaluation: target met if `weekly_active_integrations >= 2,000` across the last 28 rolling days.

---

## §4. Synapse 90-day playbook

Largely parallels ContextGraph (same developer audience), with these deltas:

### 4.1 Phase A (Days 0-30)

- 4 notebooks instead of 7 (Synapse is more demoable, less notebook-heavy).
- 4 demo videos uploaded to YouTube + repurposed to Shorts and X (Synapse benefits from visual demonstrations).
- Discord engagement on MCP-builders, OS-RPA-adjacent communities.
- First hackathon sponsorship in week 3 — AGI House or similar AI agent hackathon.

### 4.2 Phase B (Days 30-60)

- Open-source two reference Synapse playbooks (Notepad demo, gaming demo).
- First demo video viral hit attempt: a Synapse + Claude live game playthrough.
- Hackathon project amplification: 3-5 top hackathon projects featured.

### 4.3 Phase C (Days 60-90)

- Launch coordinated with Anthropic MCP ecosystem announcements (if timing permits).
- PH + HN + X-thread same pattern as ContextGraph.
- Distinctive launch angle: "Computer-use as a primitive: the missing piece of AI agent stacks."

**90-day predicate target:** `weekly_active_mcp_sessions_with_depth >= 2,500` over last 28 days.

---

## §5. Polis 90-day playbook

Fundamentally different from CG/SY. Single-city seed, in-person, network-effect-driven.

### 5.1 Phase A — Single-city seed (Days 0-30)

**Cambridge/Somerville as the seed city (MIT + Harvard density + intellectual culture).**

- Operator + 2 hand-picked ambassadors recruited locally.
- Week 1: 1 trial event (small, 6-10 people). Salon format.
- Week 2: 2 events. Operator hosts; ambassadors invite.
- Week 3: 3 events. Friend-of-friend density compounds.
- Week 4: 5 events. Different formats (salon, debate, third-place revival, mini-knowledge-growth, art-walk).
- Lead magnet shipped: "Third Place Revival Guide" PDF.
- TikTok organic: 3 short-form videos / day from Polis-persona account. Operator-narrated.
- IG organic: 1 post / day cross-posted from TikTok.

**End of Phase A:** 50 unique attendees, 4 events with ≥2 friend-clusters, 65% show rate.

### 5.2 Phase B — Densification (Days 30-60)

- 5 events / week sustained in Cambridge.
- Two-prize giveaway pattern active at every event.
- First campus ambassador paid out (BeReal pattern, $30/referral $50/install+attendance).
- Hormozi-volume on TikTok: 5 vertical clips / day. Some attempt at viral content.
- First Substack intellectual creator outreach: operator's Pre-Call Briefer prepares; operator sends.
- First podcast pitch (intellectual / culture shows, not tech).

**End of Phase B:** 200 unique attendees in Cambridge, friend cluster density ≥3 average, NPS ≥50.

### 5.3 Phase C — Mega-launch + 3-city expansion (Days 60-90)

- Day 60: open registration in 3 expansion cities (Boston, NYC, Brooklyn — closely-connected; or Austin/SF if operator preference).
- Week 9-10: hire / activate 3 ambassadors per expansion city.
- Week 11: operator-attended launch event in each new city (operator physically attends 3 events in 7 days).
- Week 12: TikTok mega-launch — coordinated 5+ video drops across operator's accounts + amplifying any organic creator who covered Polis.
- Press: tier-2 culture press (e.g., 1843, Lapham's Quarterly tier of audience).
- Substack intellectual creator partnerships activated for revenue share on referred Polis subscribers.

**End of Phase C:** 1,500-2,000 unique attendees across 4 cities; 6,000+ monthly_active_event_attendances by day 90.

---

## §6. The cross-product coordination problem

Running 3 products' 90-day playbooks at once is **not viable**. Per `11_PRODUCT_PLAYBOOKS.md §5`, the operator's recommended start order is:

1. **Months 1-3:** ContextGraph 90-day playbook (single goal active).
2. **Months 3-6:** Synapse 90-day playbook (overlapping with ContextGraph's compounding phase).
3. **Months 6-9:** Polis 90-day playbook.

If operator wants concurrent launches: ContextGraph + Synapse can co-launch in Phase C because they share audience (same hackathons, same podcasts, same channels). Polis is operationally separate; the fleet treats it as a separate goal with separate budget.

---

## §7. The launch-day script (universal template)

For mega-launch day (any product):

```
00:00 PT (Tue/Wed) — Submit to PH. Operator first maker comment within 5 min.
00:05 PT — Operator's X-thread goes live. Pinned.
00:10 PT — D6 Comment Triage activated to monitor PH comments + X replies.
00:15 PT — Discord communities notified (operator's own posts + ambassadors' posts).
03:00 PT — Mid-morning operator X update; engaging top PH commenters.
06:00 PT — Show HN submission. Operator hand-comments within 5 min.
09:00 PT — Newsletter sponsorships go out (timed to morning).
10:00 PT — First podcast appearance airs (pre-recorded).
12:00 PT — Reddit posts in 3 named subreddits (different content per sub).
13:00 PT — Operator engages every substantive comment for next 4 hours.
15:00 PT — Mid-launch Twitter Spaces / Discord AMA (45 min).
18:00 PT — End-of-day wrap-up X post.
21:00 PT — Operator off the clock; D6 + M9 handle overnight community.
```

D-day +1 through +7: continued engagement, conversion-focused follow-up, no new launch surface.

---

## §8. Pre-launch warmth checklist

Per LaunchList's 2026 SaaS pre-launch playbook, the 30 days before mega-launch:

- [ ] Launch date locked, calendar-blocked
- [ ] PH URL reserved
- [ ] Hunter identified or self-hunting decided
- [ ] 25-50 warm supporters individually messaged (operator-owned, not agent-mass-blasted)
- [ ] Reflexion memory loaded (`launch-product-hunt.md` skill page) with prior outcomes
- [ ] 15-30 journalist pitches sent embargoed
- [ ] Lead magnet polished and tested
- [ ] Email waitlist with launch-day incentive prepared
- [ ] Operator's 12-hour availability calendar-blocked
- [ ] Slack/Discord notifications routed to operator (not fleet) on launch day
- [ ] First maker comment drafted (~150 words, substantive, calls out specific use cases)
- [ ] Second comment drafted for first-hour Q&A
- [ ] X-thread drafted, 8-12 tweets, narrative arc, ends in clear CTA
- [ ] All assets in correct format per channel
- [ ] Brand-voice evaluator pre-run on every launch asset

If any item is unchecked at week-12 start, postpone the launch one week.

---

## §9. Operator emergency calendar

The 90-day playbook assumes operator focus. Specifically:

- **Phase A:** 30-40 hours/week (operator-led content, operator-led engagement, operator-led product validation)
- **Phase B:** 20-30 hours/week (operator-led founder voice + Tier-3 outreach)
- **Phase C:** 50-60 hours during launch week; 30-40 in the surrounding weeks

If operator cannot commit, the timeline shifts. The fleet handles the bulk; the operator's irreplaceable hours are founder voice, Tier-3 conversations, and live launch engagement.

---

## §10. The post-90-day handoff

Day 91+ continues. The first 90-day goal either:
- **Achieved** (predicate `TRUE` over the last 28 days): the operator sets a new, more ambitious goal (e.g., 10,000 weekly_active_integrations by month 6).
- **Missed** (predicate not met): the Reflector files a comprehensive post-mortem. The operator decides: extend, replan, or kill. A missed 90-day goal isn't a kill signal — it's a calibration data point.

Either way, the fleet's 90-day execution log becomes the seed data for all future goals. Reflexion memory is dense; channel cards are calibrated; brand voice is tuned. Subsequent goals have a much-shortened ramp.

---

## §11. The Hormozi 100-day fundamentals overlap

Hormozi's 100-day "Rule of 100" for the operator launching a new offer maps to Phase A:
- 100 outreaches/day (mostly cold from Mapper-identified Tier-2/3 targets)
- 100 minutes/day making a great anchor piece
- $100+/day on ads (only after Phase A validates LTV:CAC)
- 100 minutes/day improving ads

Phase A starts pre-paid-ads (organic only). Phase B introduces ads after LTV:CAC validation. Phase C scales them with launch coordination. Hormozi's prescription, sequenced for the fleet's reality.

---

## §12. Cross-references

- Per-product 90-day predicate trajectories: `11_PRODUCT_PLAYBOOKS.md`
- Channel mix and ramp curves: `08_DISTRIBUTION_CHANNELS.md`
- DevRel flywheel sequencing: `12_DEVELOPER_MARKETING.md`
- Polis-specific seeding mechanics: `13_CONSUMER_SOCIAL_LAUNCH.md`
- Pre-launch waitlist + Product Hunt launch tactics: `09_VIRAL_LOOPS_AND_REFERRALS.md`
- Operator approval cadence: `19_OPERATIONS_AND_CADENCE.md`
- Goal spec format: `22_GOAL_SPEC_FORMAT.md`
