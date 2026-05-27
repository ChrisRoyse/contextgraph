# Phase 6 — Synapse 90-Day Operational Launch (Weeks 17-28)

> The second launch. Starts after Leapable's 90-day predicate is achieved. Reuses the engineered platform; runs concurrent with Leapable steady-state operation. Shorter ramp than Leapable because the fleet is mature and the audience overlaps. Goal: `weekly_active_mcp_sessions_with_depth ≥ 2,500` over last 28d. Operator-time-medium; the dev audience overlap with Leapable amortizes content investment.

**Source spec docs:** `11_PRODUCT_PLAYBOOKS.md §2`, `17_LAUNCH_PLAYBOOK_90DAY.md §4`, `12_DEVELOPER_MARKETING.md`, `09_VIRAL_LOOPS_AND_REFERRALS.md §3.2`, `08_DISTRIBUTION_CHANNELS.md §7.2`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 90 days (weeks 17-28) |
| **Engineering concurrency** | None (Phase 4 hardening complete by Week 16); fleet in steady-state |
| **Prerequisites** | Leapable 90-day predicate achieved AND sustained ≥ 28d at threshold |
| **Predicate target** | `weekly_active_mcp_sessions_with_depth ≥ 2,500` over last 28d |
| **Operator time** | 25-35 hr/wk routine; 40-50 hr during Phase C launch week |
| **Acceptance gate** | All §11 criteria pass |
| **Why second** | Same dev audience as Leapable → cheaper to launch; fleet already proven; cross-product content seeds save bandwidth |

---

## §2. The 3-phase pattern (recap from `17 §4`)

Largely parallels Leapable but Synapse-specific deltas:

| Phase | Days | Pattern | Predicate by end |
|---|---|---|---|
| **A — Validation** | 0-30 | Posted-and-ghosted. Demo-heavy (Synapse is visual). | 200 weekly_active_mcp_sessions_with_depth |
| **B — Compounding** | 30-60 | Hackathons + integration partnerships + first viral demo | 800 weekly_active_mcp_sessions_with_depth |
| **C — Mega-launch** | 60-90 | Coordinated with Anthropic MCP ecosystem announcements if timing permits | 2,500+ weekly_active_mcp_sessions_with_depth |

---

## §3. Phase A — Validation (Days 0-30)

### 3.1 Synapse-specific deltas vs Leapable Phase A

| Aspect | Leapable Phase A | Synapse Phase A |
|---|---|---|
| Notebook count target | 8 (text-heavy) | 4 + 4 demo videos (Synapse is demoable) |
| Primary owned content format | Long-form blog | Short demo videos + GitHub repo polish |
| First channel mix | GitHub, HN, X-organic, Dev.to | GitHub, HN, X-organic + YouTube long-form |
| Audience overlap with prior product | (N/A — first launch) | High; Leapable audience also cares about Synapse |
| First hackathon sponsorship | Phase B | Phase A week 3 (AGI House or similar AI agent hackathon) |

### 3.2 Week-by-week

**Week 17 (Days 1-7) — Foundations.**
- Synapse goal spec activated (`marketing-fleet/goals/synapse-200-launch-2026-q3.md` from `04_PHASE_3 §2.1`)
- Brand voice doc finalized (already shipped Phase 3); Scorer Kappa-verified
- 4 launch notebooks per `12 §2.2 Synapse list`:
  - "Drive Notepad with an AI agent in 60 seconds"
  - "Your first MCP computer-use session with Claude"
  - "Synapse vs Browser-Use vs Playwright — when to use which"
  - "Building a custom Synapse automation playbook"
- 2 hero demo videos: Synapse + Claude live game playthrough; Synapse + Excel data-entry workflow
- GitHub README polish (Synapse already at v0.1.0-m3)
- Channel cards: x, github, hackernews, mcp_builder_discords, ai_agent_hackathons, dev_to

**Week 18 (Days 8-14) — Content factory boot (Synapse-specific).**
- 1 anchor/day × 7 = 7 anchors. Many anchors are video-backed.
- Operator personally posts 4-5 substantive X founder threads
- Anchor topics: "Why computer-use needs a Rust MCP server" / "Synapse: the missing piece of AI agent stacks" / "Inside the Synapse perception model" / "Why we built reflex controllers" / "Computer-use safety: how we handle panic hotkeys"

**Week 19 (Days 15-21) — Community + first hackathon sponsorship.**
- MCP-builder Discord engagement (Anthropic, Cursor, LangChain) — operator known from Leapable launch
- First hackathon sponsorship: AGI House or AI Engineer Summit ($2-5k budget)
- Synapse + Claude demo workshop pitched + accepted

**Week 20 (Days 22-30) — Demo escalation + Phase A predicate review.**
- Hero demo video drops on YouTube + X + LinkedIn (operator's own social)
- First Show HN attempt for Synapse (operator hand-comments)
- Mid-phase target: 200+ `weekly_active_mcp_sessions_with_depth`

### 3.3 Phase A end-state

| Metric | Target |
|---|---|
| `weekly_active_mcp_sessions_with_depth` | ≥ 200 over last 7d |
| Active MCP Discord engagement | ≥ 3 servers |
| Notebooks live | ≥ 4 |
| Demo videos live | ≥ 4 |
| Hackathon sponsorships completed | ≥ 1 |
| GitHub stars velocity | ≥ 50/week (per FORKOFF benchmarks `12 §2.3`) |

---

## §4. Phase B — Compounding (Days 30-60)

### 4.1 Headline activities

- Open-source 2 reference Synapse playbooks (Notepad demo, gaming demo)
- First viral demo attempt: Synapse + Claude live game playthrough (TikTok / YouTube Shorts / X video)
- Hackathon project amplification: 3-5 top hackathon projects featured
- First integration partnership conversation (LangChain, CrewAI, AutoGen)
- Referral mechanic active: demo UGC + automation playbook library

### 4.2 Week-by-week

**Week 21 (Days 31-37) — Open-source playbooks.**
- 2 reference Synapse playbooks published on GitHub
- Each playbook is a working example: clone, run, see Synapse drive the workflow
- Operator's X covers each launch

**Week 22 (Days 38-44) — Viral demo attempt.**
- 4-5 short vertical clips: Synapse + Claude game / Excel / Notepad
- Operator personally edits hero clip; agent handles derivatives
- Watch for organic viral signal (per `13 §6.3` David Aliages pattern); if a non-affiliated creator picks up Synapse, operator reaches out

**Week 23 (Days 45-51) — Hackathon project amplification.**
- 3-5 top hackathon projects featured per `12 §4.3`
- Case studies published on operator's blog + Dev.to + X thread
- AI engines index the case studies for future "how do I use X" queries

**Week 24 (Days 52-60) — First integration partnership conversation.**
- Mapper-identified bridge to LangChain BD lead → warm intro → first meeting
- Same for CrewAI / AutoGen / Anthropic Agent Framework
- Mid-phase target: 800+ `weekly_active_mcp_sessions_with_depth`

### 4.3 Phase B end-state

| Metric | Target |
|---|---|
| `weekly_active_mcp_sessions_with_depth` | ≥ 800 over last 28d |
| Open-source playbooks | ≥ 2 reference + 3-5 community-contributed |
| Hackathon project case studies | ≥ 5 |
| Integration partnership conversations active | ≥ 3 |
| Demo videos with ≥ 100k views | ≥ 1 |
| Referral K-factor | ≥ 0.20 (floor) |

---

## §5. Phase C — Mega-launch (Days 60-90)

### 5.1 Synapse-specific launch angle

Per `17 §4.3`: "Computer-use as a primitive: the missing piece of AI agent stacks"

Coordinate with Anthropic MCP ecosystem announcements if timing permits (e.g., a major MCP spec update or Anthropic Agent Framework release).

### 5.2 Week-by-week

**Week 25 (Days 61-67) — Launch prep.**
- Lock launch date (Tuesday/Wednesday, Day 75-80)
- Reserve PH URL
- Coordinate with first integration partner (joint blog post + co-launch X amplification)
- 25-50 warm supporters identified (overlap with Leapable launch list)
- 15-30 journalist pitches sent embargoed
- Lead magnet polished: "100 MCP Automation Playbooks" GitHub repo

**Week 26 (Days 68-74) — Pre-launch warmth.**
- Operator's X: 3 substantive build-log posts
- Discord seeded with high-quality content
- 10-15 journalist pitches sent embargoed (tier-2 tech press; AI engineering newsletters)

**Week 27 (Days 75-81) — Launch week.**
- Same launch-day script from `06 §5.3`
- Distinctive: 1-3 integration partner co-announcements within launch week
- Pre-recorded podcast: "Building computer-use into AI agent stacks" with hosted-by partner team

**Week 28 (Days 82-90) — Post-launch conversion.**
- 30-day aha-moment funnel
- Email sequence to PH/HN signups
- Enterprise sales calls (OS-RPA replacement angle)
- Phase C predicate target: 2,500+ `weekly_active_mcp_sessions_with_depth` sustained over final 28d

---

## §6. The Grand Slam Offer (recap from `11 §2.2`)

- **Free forever** for personal / open-source / non-commercial use
- **$29/mo Individual Pro** — ViGEm controller, advanced hotkeys, encrypted session recording
- **$499/mo Team** — shared playbooks, multi-machine fleet, audit trails
- **Enterprise** — on-prem + SLA + custom integrations
- **Guarantee:** "If your agent can't complete a basic Notepad workflow in <60s on day 1, we'll personally walk through setup on screen-share."
- **Lead magnet:** "100 MCP Automation Playbooks" GitHub repo — free, no opt-in

---

## §7. Channel mix opening allocation (recap from `11 §2.4`)

| Phase | Channels |
|---|---|
| 0-4w | GitHub, HN, X-organic, MCP-builder Discords (Anthropic, Cursor, LangChain, OpenAI) |
| 4-8w | YouTube demos, hackathon sponsorships (AGI House, AI Engineer Summit) |
| 8-12w | Podcast tour (Latent Space, MLST, AI Engineering), integrations guild (Claude Code + Cursor + Continue) |
| 12w+ | Enterprise OS-RPA-replacement angle; cold outreach to Ops leaders at AI agent companies; LinkedIn Ads on automation-decision titles |

---

## §8. Viral mechanic (recap from `11 §2.5`)

Per `09 §3.2`:

- **Demo / UGC loop:** every successful Synapse session exportable as sharable video; "Built with Synapse" referral ID embedded
- **Playbook library:** community Synapse scripts; each install requires Synapse
- **Hackathon loop:** sponsored hackathons → winners amplified → audience installs to replicate

K-target: 0.35 within 90 days. Cycle time: 7 days.

---

## §9. Elite-tier plan (recap from `11 §2.6`)

Tier-3 targets:
- MCP ecosystem leadership (Anthropic MCP team)
- AI agent platform founders (Cognition/Devin, Cursor, Replit Agent)
- Accessibility advocates (Synapse is hands-free)
- OS-RPA decision-makers at Fortune 1000 (enterprise angle)

---

## §10. Cross-product synergies with Leapable (recap from `11 §4`)

- Same operator's voice → no extra brand-voice work
- High audience overlap → Leapable's X posts can reference Synapse and vice versa
- Operator credibility from Leapable's success accelerates Synapse trust
- Leapable can verify Synapse outputs (proof-of-concept content)

Cross-promotion budget: 10% of Synapse channel mix can be allocated to cross-product placements on Leapable's organic channels.

---

## §11. Acceptance criteria

Phase 6 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | `weekly_active_mcp_sessions_with_depth ≥ 2,500` sustained over last 28d | SoT readback from synapse CF_SESSIONS |
| 2 | Third-party MCP servers referencing Synapse ≥ 50 | GitHub crawl |
| 3 | GitHub stars velocity ≥ 100/week sustained 4 weeks (per FORKOFF benchmarks) | GitHub API |
| 4 | ≥ 2 integration partnerships announced (Claude Code / Cursor / LangChain / etc.) | Public announcement + audit log |
| 5 | ≥ 3 hackathon sponsorships completed with featured projects | Hackathon followup log |
| 6 | At least 1 demo video with ≥ 500k views | Platform analytics |
| 7 | Referral K-factor ≥ 0.20 (floor) sustained 4 weeks | Cohort Auditor |
| 8 | At least 2 paid acquisition channels at LTV:CAC ≥ 3.0 (Synapse floor) | Dashboard |
| 9 | At least 5 podcast appearances aired | Podcast log |
| 10 | Goodhart Auditor reports clean | Auditor logs |
| 11 | Brand voice + compliance + rate-limit: zero unscheduled trips in last 28d | Audit log |
| 12 | Leapable goal still on track (not regressed during Synapse launch) | Leapable dashboard |

**Evidence file:** `marketing-fleet/evidence/phase_6_synapse_launch_acceptance.md`.

---

## §12. Risks specific to Phase 6

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Leapable regresses during Synapse launch (operator attention split) | Medium | High | Cross-goal scheduler enforces operator-bandwidth-protection; Leapable steady-state cycles run unchanged |
| MCP ecosystem changes mid-launch (Anthropic deprecates a tool API) | Low | High | Quarterly platform-risk review; channel cards updated within 48h of any change |
| Hackathon project amplification fails (no notable winning projects use Synapse) | Medium | Low | Operator-sponsored bounty for "best Synapse use" at each hackathon |
| Viral demo doesn't pop | Medium | Medium | Continue trying; 1 anchor + 5 derivatives daily provides shots-on-goal |
| Integration partnership conversations stall | Medium | Medium | Influence Mapper iterates bridges; operator drives directly if Mapper paths exhaust |
| Enterprise OS-RPA angle drags into long sales cycle (Phase C timeline) | High | Medium | Don't include in 90-day predicate; treat as Phase D+ work |
| Cross-product content over-references Leapable, alienating Synapse-specific audience | Low | Medium | Persona Carrier (P5) enforces persona-specific voice; Synapse posts can mention but not over-reference |

---

## §13. Cross-references

- Per-product GTM details: `docs/marketingplan/11_PRODUCT_PLAYBOOKS.md §2`
- 90-day launch playbook (canonical, Synapse section): `docs/marketingplan/17_LAUNCH_PLAYBOOK_90DAY.md §4`
- DevRel flywheel: `docs/marketingplan/12_DEVELOPER_MARKETING.md`
- Viral mechanic design: `docs/marketingplan/09_VIRAL_LOOPS_AND_REFERRALS.md §3.2`
- Channel mix: `docs/marketingplan/08_DISTRIBUTION_CHANNELS.md §7.2`
- T3 plan: `docs/marketingplan/10_INFLUENCE_NETWORK_MAPPING.md` + `11_PRODUCT_PLAYBOOKS.md §2.6`
- Cross-product synergies: `docs/marketingplan/11_PRODUCT_PLAYBOOKS.md §4`
- Prior launch: `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md`
- Next launch: `08_PHASE_7_POLIS_90DAY_LAUNCH.md`
