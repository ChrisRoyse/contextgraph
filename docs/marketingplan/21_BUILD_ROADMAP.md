# 21 — Build Roadmap

> How to construct the marketing platform itself, in order. Five milestones (M0-M4), each shippable and operator-usable, each unlocking the next. The roadmap is opinionated: build the boring foundations first; the agent fleet is the last thing built, not the first; the operator should be able to use the system end-to-end at M1 even if it covers only one product. Designed for a 12-16 week initial build.

---

## §1. The five milestones

| Milestone | Week | Outcome |
|---|---|---|
| **M0 — Foundations** | 1-2 | Goal-spec format works; RocksDB CFs created; basic CLI; leapable vault wiring; Anthropic API integrated |
| **M1 — Single-product end-to-end** | 3-6 | Leapable goal lifecycle: parse `/goal` → cron loop → planner → 2 executors → approval queue → 1 channel publishes |
| **M2 — Multi-channel + Hormozi frameworks** | 7-10 | Full agent fleet (~26 classes); 8 channels live; brand voice evaluator calibrated; Hormozi vaults integrated; viral loop mechanic |
| **M3 — Multi-product** | 11-13 | Synapse + Polis goal classes; cross-goal coordination; Goodhart Auditor; full risk + compliance enforcement |
| **M4 — Production hardening** | 14-16 | Full observability; LangSmith integration; quarterly recalibration cadence; bench-delta gate; operator dashboard polished |

Each milestone is a working system. The operator can use M1 to ship Leapable marketing today even if M2 hasn't started.

---

## §2. M0 — Foundations (weeks 1-2)

### 2.1 Deliverables

- [ ] `marketingplan/22_GOAL_SPEC_FORMAT.md` (already shipped — this doc set)
- [ ] Goal spec parser (Pydantic schema) — validates `/goal` markdown; rejects unsafe predicates
- [ ] RocksDB column families: `CF_GOALS`, `CF_GOAL_CYCLES`, `CF_EXPERIMENT_NODES`, `CF_APPROVAL_QUEUE`, `CF_REFLECTIONS`, `CF_AUDIT_LOG`, `CF_CONTACTS`, `CF_CHANNEL_STATE`, `CF_KILL_SWITCHES`
- [ ] Basic CLI: `/marketing goal create|kill|list|status`, `/marketing approve <id>`, `/marketing audit`
- [ ] Anthropic SDK wrapper with model-routing (Opus / Sonnet / Haiku)
- [ ] Leapable MCP integration: `wiki_search` tool wired to leapable, vault selection per agent
- [ ] LangGraph baseline: a StateGraph with the OuterLoop nodes (`assess`, `plan`, `atomize`, `dispatch`, `aggregate`, `evaluate`, `communicate`)
- [ ] Single test goal that runs end-to-end (no real external actions; mock channel)

### 2.2 Done criteria

- Operator can run `marketing goal create --spec=test.md` and see a cycle log appear in `CF_GOAL_CYCLES`.
- Operator can query a leapable vault from the CLI: `marketing wiki query "hormozi referral mechanics"` returns results.
- Operator can hit Ctrl+C; system durably-shuts-down; restart resumes from checkpoint.

---

## §3. M1 — Single-product end-to-end (weeks 3-6)

### 3.1 Deliverables

- [ ] `02_FIRST_PRINCIPLES.md` enforcement: the worthlessness test in the Planner; the autonomy slider in every action class
- [ ] Two executors live: P1 Content Writer + D1 Multi-Platform Publisher (for one channel — choose X or LinkedIn)
- [ ] The Brand Voice Scorer: fine-tuned Gemma 4B on operator's content; integrated into evaluator
- [ ] Approval queue UI: Slack-based notifications + simple web dashboard (operator approves / rejects / edits via Slack)
- [ ] Brand voice doc structure (`brandvoices/leapable.md`) with forbidden-phrases regex
- [ ] Lead magnet creation tool (the "Leapable Vault Integration Patterns" generation pipeline)
- [ ] Reflexion memory: when a Writer's draft is rejected by operator, the rejection-reason becomes a Reflection
- [ ] First operator-usable Leapable campaign

### 3.2 What ships next

Two specific Hormozi frameworks integrated:
- **Value equation in every brief** (`07_CONTENT_FACTORY.md §4`) — every brief auto-generates the value-equation block; drafts that don't express it are rejected by Evaluator.
- **Posted-and-ghosted launch mode** (`17_LAUNCH_PLAYBOOK_90DAY.md §1`) — the first Leapable campaign runs in posted-and-ghosted mode (no fanfare; ship to a small audience; let word-of-mouth verify).

### 3.3 Done criteria

- Operator types `/goal grow Leapable to 2,000 weekly active subscribers by 2026-08-25` in a markdown spec.
- The fleet's OuterLoop runs hourly.
- The fleet drafts 1 anchor + 4 derivatives per day.
- The fleet publishes them to operator's X.com via Synapse.
- The Brand Voice Scorer flags drafts below 0.85.
- The operator approves / edits 3-5 items per day via Slack.
- Hourly cycle log appears in the dashboard.
- The predicate is being computed against Leapable's SoT (RocksDB readback) and tracked over 28-day windows.

---

## §4. M2 — Multi-channel + Hormozi frameworks (weeks 7-10)

### 4.1 Deliverables

- [ ] Full agent fleet: 26 classes per `05_AGENT_FLEET_DESIGN.md`
- [ ] 8 channels live (per `08_DISTRIBUTION_CHANNELS.md`): X, LinkedIn, GitHub, HN, Reddit, Discord (operator's MCP-builder Discord first), Dev.to, Email outbound (cold + warm)
- [ ] Markifact MCP integration (for paid ad platforms, when operator chooses to add paid)
- [ ] Hyper MCP integration (for CRM + extended scrape coverage)
- [ ] Synapse computer-use deeply integrated for X / LinkedIn / Discord / HN ops (where APIs aren't economical)
- [ ] Audience Sorter (S1): tier classification + LinkedIn / Crunchbase / public-graph signals via Hyper MCP
- [ ] The 6 layers of content production: research → brief → draft → distribute → GEO → attribution
- [ ] Referral Engineer (O3): two-sided incentive design tools
- [ ] First viral loop shipped (badge mechanic for Leapable)
- [ ] Hormozi vault deeply integrated: Brief Generator (P4), Referral Engineer (O3), Outreach Drafter (D4) all query `AlexHormoziTranscripts` before producing
- [ ] Cohort Health Auditor (O7): tracks per-channel cohort retention; can pause referral programs when retention drops below floor

### 4.2 Hormozi integrations specifically

- Core Four channel-quadrant classification in the channel cards (`08_DISTRIBUTION_CHANNELS.md §1`)
- Value Equation block mandatory on every brief (`07_CONTENT_FACTORY.md §4`)
- Volume rule with channel-respect (`07_CONTENT_FACTORY.md §10`)
- Lead Magnet template generator
- Two-sided referral incentive structure (`09_VIRAL_LOOPS_AND_REFERRALS.md §5`)
- Hormozi-style "If you don't X, I will Y" guarantee structure in every outreach + lead-magnet (`09_VIRAL_LOOPS_AND_REFERRALS.md §5.3`)
- Client-financed acquisition 30-day check in the Cost Pacer (`15_METRICS_AND_OBSERVABILITY.md §3.5`)

### 4.3 Done criteria

- Operator can write a new goal spec; fleet routes to ~8 channels in proportional allocation per Planner; first cycle delivers artifacts across all 8 channels within 24h.
- Hormozi vault is queryable; Brief Generator includes Hormozi-grounded references in every brief.
- A Leapable experiment runs with measurable LTV:CAC; LTV:CAC dashboard live.
- Approval queue handles ~30-50 items per day; operator-clearable in ~30 min daily.

---

## §5. M3 — Multi-product (weeks 11-13)

### 5.1 Deliverables

- [ ] Synapse goal class wired: same OuterLoop, same fleet, Synapse-specific brand voice + channel mix + SoT readback
- [ ] Polis goal class wired: consumer launch playbook (TikTok-first, campus ambassador integration, BeReal-pattern); Polis-specific evaluator with NPS and friend-cluster-density signals
- [ ] Cross-goal scheduler: prevents channel collision; coordinates operator's bandwidth
- [ ] Goodhart Auditor (S4): daily; predicate-vs-reality plot dashboard element
- [ ] Influence Network Mapper (S3): full LinkedIn / Crunchbase / public-graph integration; warm-intro pathfinding; per-target ranking
- [ ] Warm-Intro Drafter (D5): forwardable-paragraph generator in bridge's voice
- [ ] Pre-Call Briefer: one-page brief generator for Tier-3 conversations
- [ ] Follow-Up Tracker: 90-day cadence reminders
- [ ] Persona Carrier (P5): multi-persona handling with non-leakage gates
- [ ] Full kill-switch suite: per-goal, per-channel, per-account, global, operator panic hotkey
- [ ] Compliance enforcement at the Guardrail Engine: CAN-SPAM / GDPR / CCPA / CASL / EU AI Act / platform ToS wrappers

### 5.2 Done criteria

- Three goals can run concurrently: Leapable + Synapse + Polis.
- The fleet's daily output: 3 anchors (one per product) + 24+ derivatives.
- The Goodhart Auditor is live; produces daily reports; tripped flag triggers fleet halt.
- The Influence Mapper has produced ≥10 warm-intro paths the operator has either approved or rejected.
- Compliance audit: 30 days of clean operation across all three products.

---

## §6. M4 — Production hardening (weeks 14-16)

### 6.1 Deliverables

- [ ] LangSmith full integration: every agent span traced; replay capability
- [ ] Grafana + Prometheus operator dashboard polished
- [ ] LLM-as-judge calibration framework: G-Eval + AdaRubric harness; 50-200 calibration examples per rubric; quarterly recalibration scripts
- [ ] Bench-delta harness: agent-class-version comparison against held-out test sets
- [ ] Weekly retro Communicator output: rich digest with Reflexion summaries, channel performance, recommended allocation shifts
- [ ] Wiki Gardener (M10): weekly memory cleanup, dedup, staleness flagging
- [ ] "Vacation mode" + "incident mode" CLI commands
- [ ] Self-audit (`marketing audit --depth=full`) produces the full report in <1 hour
- [ ] Operator panic hotkey integration tested end-to-end (50ms halt)
- [ ] Documentation: complete plan in `docs/marketingplan/` is shipped (this is the marketing-platform's CLAUDE.md)
- [ ] Backup + restore: daily RocksDB snapshots; tested restore from backup

### 6.2 Done criteria

- Self-audit runs end-to-end in <1 hour.
- The operator can demonstrate to a regulator (or a journalist) every action the fleet took in the last 30 days with full payload + decision trail.
- The operator's daily marketing-supervision time has stabilized at 3-5 hours.
- The fleet has run for 30+ days without unscheduled downtime.
- Bench-delta harness passes for one agent-class upgrade (prove the gate works).

---

## §7. The cumulative timeline

```
Wk 1  ─ M0 ───────────┐
Wk 2  ───────────────┘
Wk 3  ─ M1 ───────────┐
Wk 4  │                │
Wk 5  │                │
Wk 6  ─────────────────┘  ← first usable Leapable campaign
Wk 7  ─ M2 ───────────┐
Wk 8  │                │
Wk 9  │                │
Wk 10 ─────────────────┘  ← full fleet operational
Wk 11 ─ M3 ───────────┐
Wk 12 │                │
Wk 13 ─────────────────┘  ← three products concurrently
Wk 14 ─ M4 ───────────┐
Wk 15 │                │
Wk 16 ─────────────────┘  ← production-grade
```

After M4, the platform is in steady-state; new agent classes, new channels, new goal types are incremental additions.

---

## §8. Parallel tracks (after M0)

Within each milestone, two parallel tracks:

| Track | Owner |
|---|---|
| **Fleet construction** — agent prompts, evaluators, model routing | Engineering |
| **Operator usability** — CLI, dashboard, approval queue UX | Engineering + operator-feedback-driven |

Both tracks gate the milestone. A milestone isn't done until the operator can use the new capability without engineer-mediated workarounds.

---

## §9. Parallel operations during the build

The build doesn't pause the operator's marketing work. While M0-M4 ship, the operator continues with:

- Manual posting on operator's X / LinkedIn (the founder voice)
- Manual Tier-2 / Tier-3 conversations
- Manual lead-magnet creation (the "Leapable Vault Integration Patterns")
- Manual product work (the highest-leverage hours)

As each milestone ships, more of these activities transfer to the fleet — the fleet starts publishing M1's first channels (e.g. X via Synapse) before M2 has come online for other channels.

The build pattern is **incremental enablement**, not big-bang launch.

---

## §10. Stretch deliverables (post-M4)

Things that aren't on the critical path but become valuable later:

- **Vault marketplace publishing**: turn operator's curated leapable vaults into products of their own (paywall-able knowledge artifacts)
- **Cross-operator coordination**: multiple operators (a team) sharing the fleet under role-based access controls
- **Multi-region operator hosts**: operator-host failover across regions
- **A2A protocol integration**: standardize agent-to-agent communication with the Google A2A protocol if it matures
- **Dedicated mobile dashboard**: iPhone/Android app for the operator's approval queue (currently Slack-only)
- **Public transparency dashboard**: operator-curated subset of the audit log made public as proof of ethical conduct
- **Vault auto-ingestion**: agents flag operator-meaningful sources (podcasts, articles) and propose them as additions to leapable vaults

These are post-M4 enhancements when the steady-state operates well.

---

## §11. Acceptance criteria at M4 (full launch)

The platform is fully launched when:

- 3 active goals across 3 products
- Hourly cycles operating without intervention
- Operator's daily marketing time ≤ 4 hours
- All channels rate-respected (no platform warnings in 30 days)
- All compliance gates green (no compliance trips in 30 days)
- LTV:CAC for at least one product proven (≥3.0 floor)
- Brand voice scorer Kappa ≥0.65 stable
- Goodhart Auditor produces clean reports
- Reflexion memory dense (100+ structured reflections across 4 weeks)
- The 90-day predicate trajectory is on track for ≥1 of the three products

When all of the above hold for 4 consecutive 28-day windows, the platform itself has "shipped" — it's running, it's improving, it's earning back its build cost in predicate progress.

---

## §12. The non-build deliverable

**The most valuable deliverable of this 16-week build is not the code.** It's the **operator's confidence that the fleet operates within doctrine without operator presence**. That confidence is earned through:

- 100+ approvals reviewed and the operator agreeing with the fleet's framing 80%+ of the time
- 30+ days of clean operation
- The Reflexion memory containing the operator's accumulated corrections
- The brand voice scorer being calibrated and stable
- The Goodhart Auditor having tripped at least once and the operator having found it valuable (a real positive)
- The kill switches having been used at least once and the operator having found them snappy

Without this confidence, the operator stays the bottleneck. With it, the autonomy slider can begin to move right over months and years.

---

## §13. Cross-references

- M0 deliverables grounding: `22_GOAL_SPEC_FORMAT.md`
- M1-M4 agent classes: `05_AGENT_FLEET_DESIGN.md`
- Tech stack details per milestone: `16_TECH_STACK.md`
- Channel rollout sequence: `08_DISTRIBUTION_CHANNELS.md §6` (Hormozi's "pick one, max it")
- Risk + observability rollout: `20_RISK_AND_FAILURE_MODES.md`, `15_METRICS_AND_OBSERVABILITY.md`
- Operator workflow at each milestone: `19_OPERATIONS_AND_CADENCE.md`
