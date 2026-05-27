# Combined Marketing Plan

_Auto-generated from 23 source files in `marketingplan/` by `combine.py`. Do not hand-edit — edit the source files and re-run._

## Table of Contents

- [00_README](#00-readme)
- [01_NORTH_STAR](#01-north-star)
- [02_FIRST_PRINCIPLES](#02-first-principles)
- [03_TIER_MODEL_AUDIENCE](#03-tier-model-audience)
- [04_SYSTEM_ARCHITECTURE](#04-system-architecture)
- [05_AGENT_FLEET_DESIGN](#05-agent-fleet-design)
- [06_COMPUTER_USE_PLAYBOOK](#06-computer-use-playbook)
- [07_CONTENT_FACTORY](#07-content-factory)
- [08_DISTRIBUTION_CHANNELS](#08-distribution-channels)
- [09_VIRAL_LOOPS_AND_REFERRALS](#09-viral-loops-and-referrals)
- [10_INFLUENCE_NETWORK_MAPPING](#10-influence-network-mapping)
- [11_PRODUCT_PLAYBOOKS](#11-product-playbooks)
- [12_DEVELOPER_MARKETING](#12-developer-marketing)
- [13_CONSUMER_SOCIAL_LAUNCH](#13-consumer-social-launch)
- [14_COMPLIANCE_AND_ETHICS](#14-compliance-and-ethics)
- [15_METRICS_AND_OBSERVABILITY](#15-metrics-and-observability)
- [16_TECH_STACK](#16-tech-stack)
- [17_LAUNCH_PLAYBOOK_90DAY](#17-launch-playbook-90day)
- [18_CASE_STUDIES](#18-case-studies)
- [19_OPERATIONS_AND_CADENCE](#19-operations-and-cadence)
- [20_RISK_AND_FAILURE_MODES](#20-risk-and-failure-modes)
- [21_BUILD_ROADMAP](#21-build-roadmap)
- [22_GOAL_SPEC_FORMAT](#22-goal-spec-format)


---

<a id="00-readme"></a>
<!-- source: 00_README.md -->

# Marketing Plan — Master Index

> Goal-driven autonomous growth engine for **Leapable** (lead, ready), **Synapse** (ready), **Polis** (ready). Operator types `/goal grow Leapable to 50,000 weekly-active subscribers by 2026-09-01`. Fleet of reasoning agents — full tool surface (300+ MCP integrations + Synapse computer-use + Playwright) — runs cron-scheduled `ASSESS → PLAN → BUILD → COMMUNICATE` until predicate `TRUE` × 4 windows. **Does not stop until complete.** Mechanical guardrails (circuit breakers, kill switches, approval gates, EU AI Act Art. 14 oversight) prevent runaway. Autonomy slider dials "approve every action" → "fully autonomous between status reports."

---

## The three products

| Product | What it is | Adoption pattern |
|---|---|---|
| **Leapable** | Local-first MCP knowledge-vault marketplace. Subscribers Ask source-grounded questions; Creators publish Vaults (70/30 rev share via Stripe Connect); Developers integrate via 163 MCP tools. Signed installer; no Node/Docker/Python required for end users. Already shipping on `leapable.ai`. | 3-sided: Subscribers (T1+T2 mass), Creators (T2+T3 intellectual experts), Developers (T2 MCP-native) |
| **Synapse** | Rust MCP server for Windows desktop perception + action. Open-source core; Pro tier for ViGEm + team features. AI agents drive any Win32 app. | Bottom-up developer adoption → MCP ecosystem default → enterprise OS-RPA replacement |
| **Polis** | Re-engineered social network around physical/synchronous gathering, friend-density, debate, knowledge growth. Anti-engagement-farming. Five gathering formats. Sortition governance. | Single-city seed → campus ambassadors → TikTok virality → cultural movement |

One agent fleet. Three goal contexts. Different physics per product; same architecture.

---

## Foundations of the architecture

| Pattern | Source | Role |
|---|---|---|
| AutoResearch markdown-goal + scalar metric + ~100 experiments/night | Karpathy `karpathy/AutoResearch` | The goal-spec format; the experiment-evaluator loop |
| AgentHub message board + sprawling DAG of attempts | Karpathy `karpathy/agenthub` | Cross-agent coordination memory |
| Autonomy slider (augmentation → agent) | Karpathy Software 3.0 + Tesla Autopilot | Per-action-class dial |
| ASSESS → PLAN → BUILD → COMMUNICATE cron loop | Yopedia (`karpathy-llm-wiki`) | The outer loop |
| ADaPT / ROMA recursive decomposition | Prasad et al. 2023 + ROMA framework | Plan only when execute fails |
| Reflexion verbal self-critique memory | Shinn et al. 2023 | Learn across attempts |
| LangGraph durable + HITL + state-graph | LangChain 2024 | Execution runtime |
| LLM-as-judge with G-Eval / AdaRubric calibration | Liu et al. + 2026 G-Eval guide | Evaluator stack |
| Markifact / Hyper MCP / Windsor / leapable.ai vault-search | 2026 MCP ecosystem | Tool surface |
| Hormozi Core Four + Lead Getters + Value Equation + Client-Financed Acquisition | `$100M Offers`, `$100M Leads`, `$100M Money Models` | Marketing-doctrine substrate |
| Cursor / Linear / Vercel / BeReal / Bluesky / Stripe patterns | 2025-2026 case studies | Per-channel tactics |

---

## The 60-second pitch

Marketing in 2026 = goal-execution. Set goal (50k weekly-active subscribers, $30 CAC ceiling, 90 days). Constrain autonomy. Lock guardrails (rate limits, EU AI Act, brand voice). Fleet runs `assess→plan→build→communicate` continuously. Planner decomposes goal → DAG of experiments. Executors run in parallel, one per channel/tactic. Evaluator (LLM-as-judge + deterministic metric readback) decides which experiments earned their cost. Memory (LLM Wiki + Leapable expert vaults) files every win/loss. Operator wakes to a status report; the goal-progress number has moved. When experiments stop moving the number, reflect, replan, retry. When predicate `TRUE` × 4 windows, park and surface "GOAL ACHIEVED — set a new one."

---

## The `/goal` command

```bash
/goal grow Leapable to 50000 weekly-active-subscribers by 2026-09-01 \
  --autonomy=medium --budget=$25k/mo --compliance=US,EU,CA
```

Full grammar: `22_GOAL_SPEC_FORMAT.md`. Long-form `goals/<id>.md` files for complex specs.

---

## Reading order

| # | File | Answers |
|---|---|---|
| 00 | **README** | This file. The map. |
| 01 | [NORTH_STAR](01_NORTH_STAR.md) | One predicate defining "the system works." `Don't stop until complete` semantics. |
| 02 | [FIRST_PRINCIPLES](02_FIRST_PRINCIPLES.md) | Non-negotiable doctrine. Autonomy slider. Trust the harness. Worthlessness test. |
| 03 | [TIER_MODEL_AUDIENCE](03_TIER_MODEL_AUDIENCE.md) | Steve's Mass/Engaged/Elite formalized. Different physics per tier. |
| 04 | [SYSTEM_ARCHITECTURE](04_SYSTEM_ARCHITECTURE.md) | Atomizer → Planner → Executors → Aggregator. Cron loop. State persistence. HITL. |
| 05 | [AGENT_FLEET_DESIGN](05_AGENT_FLEET_DESIGN.md) | 26 agent classes: inputs, outputs, tools, prompts, supervision tier. |
| 06 | [COMPUTER_USE_PLAYBOOK](06_COMPUTER_USE_PLAYBOOK.md) | Synapse + Playwright + Browser-Use. Rate limits. ToS. Account isolation. |
| 07 | [CONTENT_FACTORY](07_CONTENT_FACTORY.md) | research→brief→draft→distribute→GEO→attribution pipeline. Brand voice gate. |
| 08 | [DISTRIBUTION_CHANNELS](08_DISTRIBUTION_CHANNELS.md) | Every channel: Core-Four quadrant, CAC, tier fit, owner agent, ramp curve. |
| 09 | [VIRAL_LOOPS_AND_REFERRALS](09_VIRAL_LOOPS_AND_REFERRALS.md) | K-factor math, cycle time, referral mechanics, Hormozi Lead Getters. |
| 10 | [INFLUENCE_NETWORK_MAPPING](10_INFLUENCE_NETWORK_MAPPING.md) | Elite tier. Power maps. Warm-intro pathfinding. Bridge cultivation. |
| 11 | [PRODUCT_PLAYBOOKS](11_PRODUCT_PLAYBOOKS.md) | Per-product GTM: Leapable / Synapse / Polis. |
| 12 | [DEVELOPER_MARKETING](12_DEVELOPER_MARKETING.md) | AI DevRel Flywheel: notebooks, OSS refs, hackathons, content DevRel, integrations guild. |
| 13 | [CONSUMER_SOCIAL_LAUNCH](13_CONSUMER_SOCIAL_LAUNCH.md) | Polis-specific cold start: campus ambassadors, TikTok, dense friend clusters. |
| 14 | [COMPLIANCE_AND_ETHICS](14_COMPLIANCE_AND_ETHICS.md) | CAN-SPAM, GDPR, CCPA, CASL, EU AI Act (Aug 2026), platform ToS, ethical floor. |
| 15 | [METRICS_AND_OBSERVABILITY](15_METRICS_AND_OBSERVABILITY.md) | Predicates. Per-product number sheet. Attribution. LLM-as-judge calibration. |
| 16 | [TECH_STACK](16_TECH_STACK.md) | LangGraph runtime, Markifact / Hyper MCP, Leapable vaults, evaluator stack. |
| 17 | [LAUNCH_PLAYBOOK_90DAY](17_LAUNCH_PLAYBOOK_90DAY.md) | First goal: 0 → 1,000 activated users per product in 90 days. Week-by-week. |
| 18 | [CASE_STUDIES](18_CASE_STUDIES.md) | Cursor, Linear, Vercel, BeReal, Bluesky, Stripe, Hormozi teardowns. |
| 19 | [OPERATIONS_AND_CADENCE](19_OPERATIONS_AND_CADENCE.md) | Cron schedule, daily/weekly/monthly rhythms, agent supervisor. |
| 20 | [RISK_AND_FAILURE_MODES](20_RISK_AND_FAILURE_MODES.md) | Circuit breakers, kill switches, Goodhart detection, reputation recovery. |
| 21 | [BUILD_ROADMAP](21_BUILD_ROADMAP.md) | M0–M4. How to construct the platform itself. |
| 22 | [GOAL_SPEC_FORMAT](22_GOAL_SPEC_FORMAT.md) | Exact `/goal` syntax + long-form `goal.md` schema. |

---

## What this plan is NOT

- **Not a fully autonomous robot.** Iron Man suit (Karpathy). Autonomy slider starts low; only loosens with evidence.
- **Not a content factory.** Content is 1 of ~26 agent classes. Goal achievement, not posts-per-week.
- **Not a get-rich-quick scheme.** Cursor: 18 months obsessive product work before "organic virality." BeReal: 2.5 years to mainstream. Aggressive plans behind exceptional products only.
- **Not an excuse to spam.** Hard guardrails (rate limits 30% below platform thresholds, EU AI Act disclosure, ToS-compliant computer use) are **shell-script-enforced mechanical gates**, not LLM-self-policed. Cannot be reasoned around.
- **Not platform-agnostic.** Different channels, different physics. Opinionated.

---

## Authority order (on conflict)

1. **`02_FIRST_PRINCIPLES.md`** — doctrine. Never override.
2. **`14_COMPLIANCE_AND_ETHICS.md`** — legal/ethical floor. Always binding.
3. **`20_RISK_AND_FAILURE_MODES.md`** — circuit breakers + kill switches. Mechanical.
4. **`01_NORTH_STAR.md`** — success predicate.
5. **`22_GOAL_SPEC_FORMAT.md`** — what an operator can command.
6. Everything else — operational.

Tactic conflicts with doctrine/compliance → tactic loses. No discussion.

---

## Quick architecture picture

```
   /goal "…" ──► GoalSpec (markdown, parsed, success predicate)
                       │
                       ▼
        OUTER LOOP (cron, hourly default)
        ASSESS → PLAN → BUILD → COMMUNICATE  ──┐
            ▲────────────────────────────────────┘
            │
   ┌────────┴───────┐         ┌──────────────────────────┐
   │ Atomizer       │         │ Memory (LLM Wiki +       │
   │ (atomic?)      │         │   Leapable expert vaults)│
   │  yes → execute │         │ - skills, reflections    │
   │  no → planner  │         │ - win/loss, channel cards│
   └────────┬───────┘         │ - brand voice            │
            │                 │ - contact graph          │
            ▼                 └──────────────────────────┘
   ┌────────────────┐
   │ Planner: DAG   │────► Executor agents (fleet, ~26)
   │ of subtasks    │      Content · Publisher · Community
   │ MECE, w/ deps  │      Outreach · Podcast · SEO · GEO
   └────────────────┘      Influence Mapper · Ad Pacer · …
                                       │
                                       ▼
                       Aggregator + Evaluator
                       (LLM-as-judge + SoT readback)
                                       │
            ┌──────────────────────────┼──────────────────┐
            ▼                          ▼                  ▼
   Mechanical gates          Approval Queue       Circuit Breakers
   - rate limits             (HITL)               Kill switches
   - brand voice             - operator chat      - Fuse (loops)
   - compliance              - Slack / email      - BudgetFuse
   - schema match            - autonomy slider    - TimeoutFuse
                              decides routing     - Goodhart watch
```

Implementation: `21_BUILD_ROADMAP.md`.


---

<a id="01-north-star"></a>
<!-- source: 01_NORTH_STAR.md -->

# 01 — North Star

## §0. The predicate that defines "the system works"

> For each active goal, the fleet runs `assess → plan → build → communicate` continuously until the operator-supplied **success predicate** evaluates `TRUE` for **4 consecutive rolling 28-day windows**, AND every per-goal guardrail (budget, brand, compliance, rate limits, autonomy ceiling) held throughout. **The system does not stop**, declare partial victory, or accept "almost done." It pauses only on: predicate `TRUE` × 4, operator override, or hard-guardrail trip.

That is the gate. Below is the language for expressing **what predicate** to use.

---

## §1. Goal grammar (the `/goal` command)

Operator interface = markdown-encoded goal spec, Pydantic-parseable. Full grammar in `22_GOAL_SPEC_FORMAT.md`. Load-bearing structure:

```yaml
goal_id: leapable-50k-q3-2026
product: leapable                       # leapable | synapse | polis
owner: chris
created_at: 2026-05-26

target:
  metric: weekly_active_subscribers
  comparator: ">="
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://leapable/cf_queries   # NOT the dashboard

guardrails:
  budget: {monthly_ceiling_usd: 25000, daily_velocity_cap_usd: 1500}
  unit_economics: {ltv_cac_floor: 3.5, cac_payback_ceiling_months: 12}
  brand:
    voice_spec_path: docs/brandvoices/leapable.md
    forbidden_phrases_path: docs/brandvoices/leapable.forbidden.md
  compliance:
    jurisdictions: [US, EU, CA]
    ai_disclosure_required: true        # EU AI Act Art. 50
    operator_oversight_required: true   # EU AI Act Art. 14
  platform_tos: {per_platform_rate_limits_path: docs/platform_limits.yaml}
  channels:
    forbidden: [tiktok_paid, x_paid_ads_until_brand_safety_landed]
    preferred: [github, hackernews, x_organic, dev_to, podcasts_tier1]

autonomy:
  level: medium
  approval_required_above_usd: 500
  approval_required_for: [outbound_to_named_account, ad_account_change, paid_partnership]
  auto_proceed_for: [organic_post, content_draft, internal_research]

kill_switches:
  - brand_voice_violation_score > 0.15
  - platform_rate_limit_exceeded_any
  - goodhart_signal_detected
  - cost_velocity_exceeds_2x_planned_burn
  - reputation_signal_negative_delta_24h > 5
  - operator_emergency_stop

deadline_soft: 2026-09-01
deadline_hard: 2026-10-01
exit_on_deadline_hard: pause_and_report
```

Every field is enforced **outside** the fleet by mechanical (shell-script / runtime-policy) gates. The fleet cannot reason its way past them. Doctrine: **trust the harness, not the model** (`02_FIRST_PRINCIPLES.md §3`).

---

## §2. What "until complete" means

Six terminal states. Five non-success, one success.

| Terminal state | Condition | Operator handoff |
|---|---|---|
| **`GOAL_ACHIEVED`** | predicate `TRUE` × 4 consecutive 28d windows, guardrails held | "GOAL ACHIEVED — set a new one or extend." |
| `DEADLINE_REACHED_PAUSED` | hard deadline hit before predicate | Status report; what moved; what didn't; recommended next goal |
| `BUDGET_EXHAUSTED_PAUSED` | monthly ceiling hit before predicate | Spend log; per-channel marginal CAC; recommended budget delta |
| `GUARDRAIL_TRIPPED` | kill switch (compliance, brand, Goodhart, reputation) | Incident report; root cause; mitigation; recommended restart |
| `OPERATOR_PAUSED` | manual override | Resume with `/goal resume <id>` or `/goal kill <id>` |
| `BLOCKED_NEEDS_HUMAN` | escalation no agent can resolve | Specific question; options; agent recommendation |

**No implicit timeout.** Goals persist across operator sessions, daemon restarts, Anthropic API outages. State durable (LangGraph checkpointing → RocksDB, `04_SYSTEM_ARCHITECTURE.md`). Hourly cron resumes mid-cycle from last checkpoint.

---

## §3. Per-product example predicates

Each picks a single scalar that **cannot be gamed without real users completing real product actions**.

### 3.1 Leapable

| Goal class | Predicate |
|---|---|
| Activation | `weekly_active_subscribers >= 50000` where active = distinct subscriber_id with ≥1 `Ask` query producing a non-empty answer in 7d window |
| Quality | `cohort_28d_ask_retention_for_new_subscribers >= 0.55` |
| Creator-side | `weekly_creator_publish_events >= 500` (creators publishing or updating a Vault in 7d) |
| Revenue | `mrr_usd >= 250000 AND ltv_cac >= 3.5` |
| Marketplace depth | `vaults_published_count >= 1000 with median_chunks_per_vault >= 50` |

### 3.2 Synapse

| Goal class | Predicate |
|---|---|
| Adoption | `weekly_active_mcp_sessions_with_>=3_distinct_tools >= 25000` |
| Depth | `median_session_duration_seconds_among_active >= 600` |
| Ecosystem | `third_party_mcp_servers_referencing_synapse >= 200` |
| Trust | `github_stars_velocity_per_week >= 500 sustained 4w` |

### 3.3 Polis / Serendipity Engine

| Goal class | Predicate |
|---|---|
| Activation | `verified_users_with_at_least_one_attended_event_in_last_28d >= 50000` |
| Network effect | `median_friend_density_in_polis_for_active_users >= 5` |
| Show-rate | `attended_to_RSVP_ratio_28d >= 0.65` |
| Cultural | `share_of_polis_events_with_>=1_organic_referral_attendee >= 0.40` |

Vanity counts (signups, followers, downloads, MAU without action verb) **cannot be predicates**. Goal-spec parser rejects with `PREDICATE_TOO_GAMEABLE`. Deny-list in `22_GOAL_SPEC_FORMAT.md §3.4`.

---

## §4. Worthlessness test

**Every agent action, experiment, dollar, message must trace to moving ≥1 number on the active goal's metric tree.** If the agent cannot answer "which scalar does this move, by what magnitude?" → action does not ship.

Enforced as pre-execution check in the Planner. Every proposed experiment node carries `expected_metric_impact`:

```yaml
node: ph_launch_thursday_2026-06-12
hypothesis: "PH launch with 25 warm supporters lifts weekly_active_subscribers by 200-400 in week 1"
expected_metric_impact:
  metric: weekly_active_subscribers
  delta_low: 200
  delta_high: 400
  confidence: 0.55
cost_usd: 1200
cost_token_budget: 5_000_000
time_budget_hours: 18
risk: medium
risk_drivers: [brand_voice_on_HN_comments, sustained_LA_hours_engagement]
fallback: if_collides_with_anthropic_announcement_postpone_one_week
```

Experiments without the block → rejected by Planner. Matches Karpathy AutoResearch: every experiment = a hypothesis + quantified expected delta + fixed budget. Aggregator/Evaluator checks actual vs. expected; feeds delta into next planning cycle.

---

## §5. Single-number scoreboards

Per product, ONE composite watched daily. Predicate = the gate; scoreboard = the dashboard.

| Product | Scoreboard number |
|---|---|
| Leapable | weekly_active_subscribers (per §3.1; the `Ask`-issuing kind) |
| Synapse | weekly_active_mcp_sessions × distinct-tools-per-session (depth-weighted) |
| Polis | active small-group attendance rate (showed-up ÷ RSVP'd over 28d) |

Adversarial against vanity gaming. Cannot be inflated by:
- Bot signups → no product event fires
- Content-farm signups → no `Ask` query happens
- Mass DMs → recipient still installs + runs + stays
- Self-clicks → per-user dedupe at product DB layer
- Paid acquisition spike → predicate is *sustained* rolling-window; spikes wash out

---

## §6. Goodhart watchdog

Goodhart's Law: *when a measure becomes a target, it ceases to be a good measure*. The single biggest risk of an autonomous fleet optimizing for a metric: it finds a cheap high-leverage way to move the metric **without moving underlying reality**.

A separate **Goodhart Auditor Agent** runs daily, independent of the Planner. Checks divergence between **predicate metric** and three **correlated reality signals**:

| Predicate-side signal | Reality cross-check |
|---|---|
| `weekly_active_subscribers` rising fast | Paid subscribers rising in proportion? Creator Asks ratio holding? Ask-to-answer-helpful rate? |
| `attended_events` rising fast (Polis) | Return attendance rising? Event NPS holding? Net-new social-graph edges per attendee? |
| `weekly_active_mcp_sessions` rising fast (Synapse) | Depth (tools/session) holding? Third-party MCP servers referencing Synapse growing? |

If predicate outpaces reality signal > 2x over rolling 14d → `goodhart_signal_detected` flag = hard kill switch. Fleet stops. Operator gets incident report. No further experiments fire until divergence is explained.

This is the **anti-Cobra Effect** guardrail. Borrowed from LLM-as-judge calibration literature: continuously check the cheap proxy still correlates with the expensive ground truth.

---

## §7. State is not the constraint

Product, channels, funnel, homepage copy, CAC = **starting state, not fixed constraints**. If reaching the predicate requires changing the product (e.g., adding a referral mechanic, `09_VIRAL_LOOPS_AND_REFERRALS.md`), the Planner's `PRODUCT_PROPOSAL` agent class proposes it. **Product changes always require operator approval** regardless of autonomy slider.

Agent owns: channels, content, ad creative, outreach copy, scheduling, prioritization (within guardrails). Operator owns: product copy, pricing, packaging.

Only **doctrine invariants** (`02_FIRST_PRINCIPLES.md`) and **compliance floor** (`14_COMPLIANCE_AND_ETHICS.md`) truly immovable.

---

## §8. Calibrating window sizes

Defaults `evaluation_window: 28d` and `consecutive_windows_required: 4` tuned to:

- **28 days** = long enough to wash out PH spikes, TikTok virality, seasonality. Short enough to feed operator within one product cycle.
- **4 consecutive windows** = ~3-4 calendar months at predicate. By this point: result is not coincidence and not one-time spend.

Goal spec can override (short-cycle consumer launch → `14d × 6`). Parser rejects anything below `7d × 4` as too short to be stable.

---

## §9. Cross-references

- Numerical floors/ceilings per goal: `15_METRICS_AND_OBSERVABILITY.md §2`
- Per-product mechanism for predicate movement: `11_PRODUCT_PLAYBOOKS.md`
- 90-day plan creating the first evaluable goal: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Doctrine gating allowed actions: `02_FIRST_PRINCIPLES.md`
- Goodhart Auditor implementation: `20_RISK_AND_FAILURE_MODES.md §4`
- Full `/goal` grammar: `22_GOAL_SPEC_FORMAT.md`


---

<a id="02-first-principles"></a>
<!-- source: 02_FIRST_PRINCIPLES.md -->

# 02 — First Principles (Doctrine)

> Read once at session start. Re-read when tempted to cut a corner. These are the rules the agent fleet (and the operator) cannot break. Tactics change weekly; the architecture changes quarterly; the doctrine never changes.

---

## §1. Goal-driven, not pipeline-driven

The system does not run a fixed marketing pipeline. The system **executes against an operator-supplied goal** until the goal's success predicate is `TRUE` for four consecutive windows or a hard guardrail trips. Channels, tactics, content formats, ad mixes, even agent topology are **emergent consequences of which experiments moved the goal-metric the most**.

A "marketing calendar" is a snapshot, not a plan. The plan is the goal. The calendar is whatever the Planner agent decided this week to maximize expected predicate movement.

---

## §2. Karpathy's autonomy slider

Every external action (anything that touches a third-party platform, a customer, or a public surface) has an autonomy tier:

| Tier | Behavior | Used for |
|---|---|---|
| `AUTO` | Agent executes, logs, no human review needed | Reads, internal analysis, A/B variant generation, draft content into staging |
| `LOG` | Agent executes; flags sampled audit for human review later | Low-cost reversible writes: organic post drafting (when brand-voice-passes), internal CRM updates, follow-ups within established threads |
| `REQUIRE_APPROVAL` | Agent proposes; pauses; waits for human approval before executing | Anything irreversible, high-cost, or first-time: paid ad creative, outbound to a named account, public statement, product proposal, partnership outreach |
| `ESCALATE` | Agent surfaces to operator with options + recommendation; operator's call | Brand-risk-sensitive cases, contradictions in evidence, regulatory edge cases, anomalies the agent isn't confident enough to recommend on |

The goal spec sets a default autonomy level and overrides per action class. **The operator can move any action class up or down the slider as confidence accumulates.** This is non-negotiable. A 99.7%-approved action class should be moved to LOG; a 25%-overridden action class should be moved to ESCALATE.

The slider starts conservative ("LOG for content, REQUIRE_APPROVAL for everything else") and only loosens with evidence.

---

## §3. Trust the harness, not the model

Borrowed verbatim from the Yopedia / `karpathy-llm-wiki` pattern.

The agent fleet **cannot enforce its own constraints**. Anything that matters — rate limits, brand voice, compliance, kill switches, budget caps, schema validation — is enforced by **shell-script-level or runtime-policy-level mechanical gates**, sitting outside the agent loop, that the model cannot reason past.

Example: "Don't post the same draft to LinkedIn twice in 48 hours" is enforced by a deterministic Redis check in the publish-tool wrapper, **not** by adding "please don't double-post" to a system prompt.

Example: "Every external email to an EU contact includes the EU AI Act disclosure line" is enforced by a templating wrapper around `send_email` that injects the disclosure, **not** by hoping the content agent remembered.

Example: "Brand voice score must be ≥0.85 before publish" is enforced by an evaluator agent and a deterministic gate, **not** by adding "stay on brand voice" to the writer agent's instructions alone.

The system prompt is a recommendation. The harness is a law.

---

## §4. Worthlessness test

If a proposed action does not move (or preserve) at least one number on the active goal's metric tree, **it does not ship**. This applies to every agent action — content piece, ad campaign, podcast pitch, partner outreach, blog post, ambassador hire, conference attendance.

The Planner agent rejects proposals without quantified expected impact. (`01_NORTH_STAR.md §4`.)

This is brutal. It kills a lot of "marketing activity" that feels productive but isn't. That is the entire point.

---

## §5. Reality is the verdict, not the dashboard

When the agent fleet asks "did experiment X work?", the answer comes from **the product's source of truth** (RocksDB, the SQL database, the event stream), not from a vendor analytics dashboard. Vendor dashboards lag, double-count, drop events, and silently change attribution models. Source-of-truth readback is the verdict.

Concretely, every experiment node carries `source_of_truth: rocksdb://leapable/cf_queries` or equivalent. The Evaluator agent queries the SoT directly, not Mixpanel/Posthog/Amplitude.

Vendor dashboards are diagnostic tools. They never decide.

---

## §6. Generic primitives only

The agent fleet builds generic capabilities, not task-specific hacks. A `discord_engager` agent that knows how to operate well in *any* technical Discord beats nine specialized agents for nine specific Discords. The cost of specialization is fragility — when the channel evolves, the specialization breaks.

GENERIC-ONLY rule. The fleet is allowed to specialize on the **inside of a tactic** ("how the agent operates on r/MachineLearning specifically") in playbooks, but the **agent code** stays generic and reads the playbook.

---

## §7. Reflexion memory, not amnesia

Every failed experiment leaves a **verbal reflection** in the goal's memory wiki. Next attempt at a similar experiment reads the reflection before planning. "Last time we ran a PH launch on a Tuesday after an OpenAI announcement and got buried" is a fact the fleet should know on attempt #2.

The mechanism is the Karpathy LLM Wiki pattern: the Memory Agent reads the run log, summarizes the win/loss with structured fields (channel, hypothesis, expected, actual, why-it-differed, retry-conditions), writes a `reflection.md` page, and cross-references it. The Planner queries the wiki by tactic and channel before proposing experiments.

Reflexion is the difference between an agent that re-tries identical failures and an agent that improves over time.

---

## §8. Recursive decomposition, only when needed (ADaPT)

The Planner does **not** decompose a goal into a giant flat list of every imaginable subtask up-front. It tries the goal-level execution first. If the Executor agents make progress, the Planner stays out of the way. **If and only if execution stalls or fails**, the Planner triggers a decomposition step, producing a tighter sub-DAG. This recurses as deep as needed.

The reason: pre-decomposition produces task graphs full of mid-level abstractions ("build awareness," "drive consideration") that don't map to atomic agent actions. ADaPT-style late binding produces task graphs that are atomic at the leaves and shallow at the root.

---

## §9. No mocks gate completion

An experiment is not "successful" because the agent reports success. It is successful when the **source of truth shows the metric moved**. If the metric didn't move, the experiment failed, regardless of how good the post looked or how many likes it got.

Vanity metrics (impressions, reach, follows, opens) are not allowed as completion criteria for any node. They can appear in reports as diagnostic signal; they cannot gate progression.

---

## §10. Compliance is an upstream gate, not a downstream review

CAN‑SPAM disclaimers, EU AI Act Article 50 AI-content disclosure, GDPR legitimate-interest documentation, CCPA opt-out, platform ToS rate limits — **all of these are pre-execution gates**, enforced by mechanical wrappers around the relevant tools. The fleet cannot ship an EU outbound email without the disclosure block because the `send_email_eu` tool wrapper refuses to send without it.

Compliance is never a post-hoc audit. It's a syntax error at the tool call.

---

## §11. Computer-use is real-human-shaped use

When the agent operates a web platform via Synapse + Playwright + Browser-Use (because the platform doesn't expose a real API, or because the API is gated to enterprise tiers), the agent operates **at human-shaped rates**, **with human-shaped pauses**, and **only on operator-owned accounts**. No multi-account farming. No simultaneous-session amplification. No buying impressions.

Specifics:
- Per-platform rate limits set 30%+ below the platform's published bot-detection thresholds.
- Action cadence stochastic, not periodic (real humans don't post every 47 minutes).
- One session per operator-owned account; account ownership documented in an internal registry.
- Computer-use is recorded as a screen capture for post-hoc audit on operator request.

This both keeps the operator's accounts alive and keeps the fleet ethically defensible.

---

## §12. The Iron Man suit, not the Iron Man robot

Karpathy: *"It's less like building flashy demos of autonomous agents and more building partial autonomy products."* The marketing platform is **augmentation, not replacement**. The operator's judgment is the ceiling on quality, not the agent's. The fleet handles the volume the operator could not personally cover, at the operator's brand voice, against the operator's chosen goals.

The autonomy slider moves right over months and years as the fleet earns trust on specific action classes. It never moves all the way right. Some classes (legal commitments, partner contracts, public crisis response) are permanently `ESCALATE`.

---

## §13. Slot identity: each agent owns one job

Borrowed from Leapable's "Slot identity is sacred" rule. Each agent class has **one** primary responsibility. The Content Writer doesn't also do scheduling. The Scheduler doesn't also do brand-voice evaluation. The Evaluator doesn't also do writing.

Why: cross-responsibility agents accumulate prompts. Prompts that try to do five things do all five badly. Specialized agents with one job stay tight.

Multi-step workflows are composed by the Orchestrator across single-job agents, not by stuffing one agent with five jobs.

---

## §14. Bench delta gate

A material change to any agent class (prompt rewrite, model swap, tool reorder, evaluator rubric update) ships only after a **bench delta** against a held-out evaluation set of past experiments. The delta must be:
- Non-regression on goal-relevant metrics (the change must not make experiments perform worse).
- No more than +5% cost-per-experiment increase without justification.

This prevents prompt-churn drift: a series of "small improvements" that each look fine but collectively degrade performance. The Reflexion-trained Cursor / Claude Code pattern: every meaningful agent change is a small commit with a measurable delta or it doesn't ship.

---

## §15. Failure documentation is the next agent's training data

When an experiment fails, the Reflector agent writes a structured post-mortem — not a long narrative, a tight structured block:

```yaml
experiment_id: ph-launch-2026-01-22
goal_id: leapable-50k-q3-2026
outcome: missed
expected: +200-400 new weekly_active_subscribers
actual: +47
diagnostics:
  - launched_same_day_as_anthropic_claude_5_announcement
  - HN_front_page_held_by_anthropic_for_4_hours
  - PH_traffic_at_2pm_PT_lower_than_modeled
  - 22_warm_supporters_engaged_vs_planned_25
reflections:
  - "Pre-launch check: never launch within 24h of a major model-vendor announcement."
  - "Warm-supporter list needs +30% buffer over modeled engagement."
takeaways_filed_to: skills/launch-product-hunt.md
retry_conditions: yes_with_revised_pre-launch_check
```

The takeaways propagate into the relevant skill page. Future experiments under that skill inherit them automatically.

---

## §16. Cost discipline

Every active goal has a budget ceiling per goal-spec. The Cost Pacer agent runs continuously and enforces:

1. **Daily velocity cap** — no spending more than `goal.guardrails.budget.daily_velocity_cap_usd` in any 24h.
2. **Per-experiment ceiling** — no single experiment > `monthly_ceiling_usd / 10` without escalation.
3. **Marginal CAC tripwire** — if last-7d acquired-user count divided by last-7d spend exceeds `ltv_floor / 3`, pause and escalate.
4. **Token budget** — every agent run has a token cap; over-budget runs trip the BudgetFuse circuit breaker before they get to LLM hosts.

Cost overruns are not failures of intent; they are failures of mechanical limits. The Pacer is mechanical.

---

## §17. Circuit breakers and kill switches are first-class

The system ships with seven trip codes that immediately halt all new external actions on a goal:

| Trip code | Trigger | Recovery path |
|---|---|---|
| `STAGNATION` | No predicate movement for N planning cycles despite cost spent | Reflector writes mandatory post-mortem; operator decides resume/replan/kill |
| `OVERRISK` | Aggregate risk score (compliance + brand + reputation) over threshold | Operator review of last 24h of actions |
| `EXHAUSTION` | Token or dollar budget hit | Operator budget extension or scope reduction |
| `RUNAWAY_LOOP` | Same action attempted >5 times with same input | Reflexion forces decomposition |
| `GOODHART_DETECTED` | Predicate metric and reality cross-check diverged 2x+ over 14d | Mandatory operator review (`§6` in `01_NORTH_STAR.md`) |
| `BRAND_VIOLATION` | G-Eval brand voice score < threshold on already-published content | Pull/retract + operator review |
| `OPERATOR_EMERGENCY_STOP` | Operator hits the panic button | Everything pauses; resume on explicit `/goal resume` |

These are O(1)-checkable cached flags at the runtime supervisor (Sakura Sky / AgentOS pattern). They cannot be bypassed by the agent. Cache TTL ≤ 2 seconds so a tripped flag halts execution within one tick.

---

## §18. Behavioral rules (always enforced)

1. Do what the active goal asks; nothing more, nothing less.
2. NEVER create marketing accounts unless explicitly part of an approved experiment.
3. ALWAYS prefer engaging existing presence to spinning up new presence.
4. NEVER ship content that fails the brand-voice evaluator.
5. NEVER post the same content twice within the platform's perception window.
6. ALWAYS attach the AI-disclosure block on outbound content to EU jurisdictions.
7. NEVER share operator credentials between agents; per-agent scoped tokens only.
8. AFTER any operator correction or kill-switch, the Reflector files a `type:decision` page in the wiki so the lesson propagates.
9. The agent does file-reading, synthesis, drafting, and proposals personally; the operator retains the final decision on every `REQUIRE_APPROVAL` and `ESCALATE` action.

---

## §19. Project-specific hard "never"s

- Never run a paid ad campaign without operator approval, regardless of autonomy slider.
- Never make a public statement on behalf of the operator's persona without explicit operator approval.
- Never sign a contract, accept a partnership, or commit to a deliverable on the operator's behalf.
- Never claim AI-generated content is human-generated. Never the reverse.
- Never engage in coordinated inauthentic behavior on any platform.
- Never buy followers, likes, reviews, upvotes, or any other engagement signal.
- Never use scraped personal data for outreach without verified opt-in or applicable legitimate-interest documentation.
- Never operate on accounts the operator does not own and explicitly registered.
- Never bypass a circuit breaker by re-routing through a different tool.
- Never operate in a jurisdiction not listed in `goal.guardrails.compliance.jurisdictions`.

---

## §20. Per-turn protocol (for the Operator Agent serving the human)

When the operator interacts with the fleet, the Operator Agent (the conversational interface) follows:

1. **READ** — pull active goals, current status, last 24h notable events, pending approvals.
2. **OBSERVE** — listen to the operator's request without inserting recommendations.
3. **MAP** — translate the request into either (a) a new `/goal`, (b) an autonomy slider change, (c) an experiment proposal, or (d) an approval/rejection of a queued item.
4. **CLARIFY** — if any field of the request is ambiguous (predicate, budget, deadline, autonomy), ask exactly one clarification at a time. Never assume.
5. **EXECUTE** — push the result into the goal store / approval queue / autonomy registry, return a short confirmation.
6. **SUMMARIZE** — at end of operator turn, surface: what changed, what's next on the queue, whether any kill switch tripped.

The Operator Agent never proposes experiments unsolicited. The fleet proposes experiments through the approval queue, never via the conversational channel.

---

## §21. One number per product, one predicate per goal

Borrowed from §3 of `01_NORTH_STAR.md`. Repeated here because it's the most-violated rule when teams scale: **per goal, ONE primary scalar.** Composite scoreboards are diagnostic. Predicates are unitary.

If the operator wants to optimize two things simultaneously, they create two goals with two predicates. The fleet handles parallel goals (resource-allocates across them by expected ROI on each predicate). What it does not do is collapse two predicates into one — that's the Goodhart cliff.

---

## §22. The doctrine is recursive

Every doc in this folder enforces this doctrine. If a doc contradicts this doc, this doc wins. If new evidence contradicts the doctrine, the doctrine updates **only through an explicit ADR added to this folder** (`docs/marketingplan/adr/####-title.md`). Never silently. Never via prompt tuning.

The doctrine evolves. It does not drift.


---

<a id="03-tier-model-audience"></a>
<!-- source: 03_TIER_MODEL_AUDIENCE.md -->

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


---

<a id="04-system-architecture"></a>
<!-- source: 04_SYSTEM_ARCHITECTURE.md -->

# 04 — System Architecture

> The complete blueprint of the goal-driven autonomous growth engine. Built on the ROMA recursive-meta-agent pattern (Atomizer / Planner / Executors / Aggregator), the Yopedia cron-driven assess→plan→build→communicate loop, LangGraph's durable execution + HITL primitives, Karpathy's AutoResearch single-scalar-metric discipline, and Anthropic's context-engineering principles for long-horizon coherence.

---

## §1. High-level diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                  OPERATOR INTERFACE                                  │
│   /goal <spec>   /pause   /resume   /kill   /status   /approve   /reject   /audit    │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                   GOAL STORE                                          │
│   GoalSpec (Pydantic schema)  ·  Status  ·  Predicate evaluator  ·  Audit log         │
│   Backed by RocksDB CFs: CF_GOALS, CF_GOAL_HISTORY, CF_APPROVAL_QUEUE                 │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│            OUTER LOOP — cron-scheduled (hourly default; configurable per goal)        │
│                                                                                       │
│        ASSESS  ───►  PLAN  ───►  BUILD  ───►  COMMUNICATE  ──┐                       │
│           ▲                                                   │                       │
│           └───────────────────────────────────────────────────┘                       │
│                                                                                       │
│   Each pass: read goal status, decide what to do next, dispatch, report.              │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
            ┌──────────────────┼──────────────────┐
            ▼                  ▼                  ▼
   ┌───────────────┐  ┌────────────────┐  ┌──────────────────┐
   │  Atomizer     │  │  Planner       │  │  Reflector       │
   │  (atomic?)    │  │  (decompose to │  │  (post-mortem    │
   │               │  │   sub-DAG)     │  │   on failures)   │
   └───────┬───────┘  └───────┬────────┘  └──────────────────┘
           │                  │
           ▼                  ▼
        ┌──────────────────────────────────────────────────────┐
        │             EXECUTOR FLEET (~25 agent classes)         │
        │  Content · Publisher · Community · Outreach · Podcast │
        │  SEO · GEO · Influence · Ad-pacer · Referral · ...    │
        └──────────────────────────┬───────────────────────────┘
                                   │
                                   ▼
        ┌──────────────────────────────────────────────────────┐
        │            AGGREGATOR + EVALUATOR LAYER               │
        │   LLM-as-judge (G-Eval / AdaRubric) · Trajectory eval │
        │   Deterministic metric readback from product SoT      │
        └──────────┬─────────────────────────────────┬──────────┘
                   │                                 │
                   ▼                                 ▼
         ┌─────────────────────┐         ┌───────────────────────┐
         │   APPROVAL QUEUE    │         │   GUARDRAIL ENGINE    │
         │   (HITL interrupts) │         │   - Rate limit gate   │
         │                     │         │   - Brand voice gate  │
         │                     │         │   - Compliance gate   │
         │                     │         │   - Schema match gate │
         │                     │         │   - Circuit breakers  │
         │                     │         │   - Kill switches     │
         └──────────┬──────────┘         └───────────┬───────────┘
                    │                                │
                    ▼                                ▼
           ┌──────────────────────────────────────────────┐
           │            TOOL GATEWAY (MCP + Custom)        │
           │  Markifact · Hyper MCP · Windsor · Synapse    │
           │  Playwright · Browser-Use · custom adapters   │
           └──────────────────┬───────────────────────────┘
                              │
                              ▼
       (External world: APIs, web platforms, ad accounts, email,
        CRM, GitHub, social, Discord, Slack, calendar, etc.)

      ───────────────────────────────────────────────────────────

         ┌────────────────────────────────────────────────────┐
         │                 MEMORY LAYER (LLM Wiki)             │
         │   Skills · Channel cards · Reflections · Wins/Loss  │
         │   Brand voice · Contact graph · Persona library     │
         │   RocksDB backed; queryable via wiki_search() tool  │
         └────────────────────────────────────────────────────┘
```

---

## §2. The outer loop: ASSESS → PLAN → BUILD → COMMUNICATE

Borrowed from the Yopedia (`karpathy-llm-wiki`) pattern. Runs on a cron schedule (default hourly, per-goal override possible). Each pass is **idempotent**: a crash mid-pass is restarted from the last LangGraph checkpoint without corruption.

### 2.1 ASSESS phase (~10% of cycle time)

The Assessment Agent reads:
- Current goal status (predicate value, distance to threshold, trend over last 4 windows)
- All open experiment nodes (running, pending approval, blocked)
- All goal-relevant new signals since last cycle (platform changes, viral mentions, competitor launches, regulatory shifts)
- The reflection wiki for relevant prior lessons

Produces a structured `AssessmentReport`:

```yaml
assessment:
  goal_id: leapable-50k-q3-2026
  cycle_id: 2026-05-26T14:00:00Z
  predicate_status:
    current: 12_400
    target: 50_000
    delta_last_28d: +1240
    delta_last_28d_required_for_target: +9_400
    trend: behind
  open_experiments: 7
  approvals_pending: 2
  guardrail_violations: 0
  new_signals:
    - mention: "@swyx referenced Leapable in Latent Space podcast ep 247"
      relevance: high
      suggested_followup: thank + amplify + reach out for deeper conversation
    - platform: "X new bot-detection rollout; reduce posting cadence to 4/day"
      relevance: medium
      suggested_followup: update channel card; pacing reduced
  reflections_loaded: 14
  recommended_focus_for_planning: "compound on Latent Space mention; recover from missed Tuesday cohort"
```

### 2.2 PLAN phase (~30% of cycle time)

The Planner agent consumes the Assessment Report and (a) the goal-spec, (b) the Reflection wiki, (c) the current capacity envelope (autonomy slider, budget remaining, approved channels). Output: a **DAG of experiment nodes** to add to the open queue.

Each node carries the `expected_metric_impact` block (`01_NORTH_STAR.md §4`) so the worthlessness test can run pre-dispatch.

The Planner uses **ADaPT** recursion: it does not pre-decompose to atomic actions. It produces medium-grain nodes ("launch a PH on Thursday with 25 warm supporters"). The Atomizer decides per node whether to dispatch or decompose further.

### 2.3 BUILD phase (~50% of cycle time)

The Atomizer pulls open nodes from the queue and routes each:
- If the node is **atomic** (can be done by one Executor in one tool sequence) → dispatch to the matching Executor.
- If the node is **non-atomic** → invoke the Planner recursively on this node only, producing child nodes; insert them into the queue; mark the parent as `awaiting_children`.

Executors run **in parallel** where dependencies permit (the DAG carries `blocks` / `blocked_by`). Concurrency cap per Executor class is set by the channel's rate-limit card.

Each Executor's output is consumed by the Aggregator/Evaluator (§3.4). Results bubble up the DAG: child outputs aggregate into parent context; parent outputs aggregate into the goal-level cycle report.

### 2.4 COMMUNICATE phase (~10% of cycle time)

The Communicator Agent writes:
- A per-cycle **status update** to the goal's running log (markdown, append-only)
- A per-cycle **operator digest** if the operator has digest-mode enabled (Slack/email/CLI notification)
- New entries into the LLM Wiki: any structured reflections from this cycle's wins and losses
- Updates to channel cards based on observed performance

The cycle ends. The next cron tick begins.

---

## §3. ROMA recursive control loop

Borrowed from the ROMA framework. At each node in the goal-execution DAG, the same four-step loop runs:

### 3.1 Atomizer

Decides: is this node atomic (one Executor can complete it) or non-atomic (needs further decomposition)? Uses a classifier:

- Atomic if: the action maps to a single tool sequence in a single channel.
- Non-atomic if: the action requires coordination across ≥2 channels, OR requires evidence collection before execution, OR carries dependencies that need explicit ordering.

The Atomizer's classification is conservative — when in doubt, decompose. Cheap mistakes cost less than over-broad Executor failures.

### 3.2 Planner (per non-atomic node)

For a non-atomic node, the Planner produces a **MECE** (Mutually Exclusive, Collectively Exhaustive) child DAG: each child is independently dispatchable; together they fully realize the parent.

Each child carries the same shape as a top-level experiment node — including `expected_metric_impact`. If the parent's impact = 200 incremental WAUs, and the planner proposes four children, the children's deltas should sum to ~200 (allowing for risk discount).

### 3.3 Executors

One Executor per atomic node. The Executor:
1. Loads relevant Memory (channel card, brand voice, recent reflections).
2. Plans the tool sequence.
3. Executes through the Tool Gateway (gated by Guardrail Engine).
4. Returns a structured `ExecutionResult { artifacts, side_effects, observed_metrics, errors, cost }`.

Parallel execution within sibling nodes is encouraged (Devin "Managed Devins" pattern, AgentHub multi-branch DAG pattern).

### 3.4 Aggregator + Evaluator

Children's `ExecutionResult`s are aggregated to the parent. The Evaluator scores the aggregate output across:

1. **Deterministic metrics** — direct SoT readback (the only signal that gates predicate movement).
2. **LLM-as-judge rubrics** (G-Eval / AdaRubric) — quality of artifacts that don't have deterministic measures (brand voice, helpfulness on a Reddit thread, tone of an outreach email).
3. **Trajectory evaluation** — did the Executor use the right tools, in the right order, without waste?

Aggregated, scored output rolls up. If the parent succeeds, the parent's parent receives the aggregated artifact and the cycle continues. If it fails (LLM-as-judge below threshold, or deterministic metric uncertain), Reflexion kicks in.

---

## §4. Reflexion across attempts

Every parent-level failure produces a structured reflection (`02_FIRST_PRINCIPLES.md §15`). The reflection is **stored in the Memory wiki** keyed by `(channel, tactic, parent_node_type)`. Next attempt at a similar parent inherits the reflection automatically.

Reflexion has a retry cap. The default is **3 attempts** per parent node within a single cycle. After 3 failures, the parent is marked `BLOCKED_NEEDS_HUMAN` and surfaces to the operator with the reflections attached.

This bounds the cost of stuck loops while preserving the learning across attempts.

---

## §5. Memory layer — the LLM Wiki

Borrowed from Karpathy's LLM Wiki gist (`karpathy/442a6bf555914893e9891c11519de94f`) + **Leapable as the production-grade backend** for expert-vault queries. The Memory is **not a chat history**. It is a structured, queryable, append-only knowledge base of:

- **Skills** — reusable playbooks for specific tactics ("how to launch on Product Hunt", "how to engage on r/MachineLearning", "how to pitch Latent Space")
- **Channel cards** — per-channel operational state (rate limits, current cadence, brand voice deltas, common pitfalls, last performance)
- **Reflections** — every win/loss post-mortem, indexed by tactic and channel
- **Wins log** — what worked, with quantified deltas (used by Planner as evidence)
- **Brand voice** — the operator's voice signature, updated as the operator gives corrections
- **Contact graph** — every external person touched, with tier, last-touched timestamp, last-signal, and notes
- **Persona library** — the operator's personae (if running multi-persona accounts) with per-persona voice + posting history

The wiki is markdown files (or RocksDB-backed JSON, see `16_TECH_STACK.md`). Every agent reads relevant wiki pages before acting; every agent writes new pages or updates existing ones after acting. A weekly Wiki Gardener agent dedupes, cross-references, and prunes stale entries.

The wiki tool surface:

```
wiki_search(query, top_k=10) -> [WikiPage]
wiki_get(path) -> WikiPage
wiki_update(path, diff, change_note) -> WikiPage
wiki_create(path, content, frontmatter) -> WikiPage
wiki_link(from_path, to_path, relation) -> Edge
```

---

## §6. State persistence and durability

The system survives:
- Anthropic API outages
- Daemon restarts
- Operator workstation reboots
- Multi-day pauses
- Cross-version upgrades

How: LangGraph **durable execution + checkpointing** into RocksDB. Every node's state is serialized at every meaningful transition. On restart, the OuterLoop reads the last checkpoint and resumes mid-cycle. Idempotency keys on every external action prevent double-publishing.

RocksDB column families relevant to the marketing platform:
- `CF_GOALS` — active and historical goal specs
- `CF_GOAL_CYCLES` — every assess→plan→build→communicate pass with timestamps
- `CF_EXPERIMENT_NODES` — the DAG state per goal
- `CF_APPROVAL_QUEUE` — pending HITL items, with state, requester, deadline, decision
- `CF_REFLECTIONS` — Reflexion memory
- `CF_WIKI` — LLM Wiki pages (or filesystem-backed with this CF as index)
- `CF_CONTACTS` — contact graph + tier
- `CF_AUDIT_LOG` — append-only event log (every agent action, with cost, tool, outcome)

---

## §7. Approval queue + autonomy slider integration

When a node's action class is `REQUIRE_APPROVAL` (per the goal's autonomy spec), the Executor:

1. **Serializes** the full action context (intended tool call, payload, expected effect, brand-voice score, cost estimate, reflections referenced).
2. **Enqueues** to the approval queue.
3. **Pauses** the parent DAG branch (other branches continue).
4. **Notifies** the operator via the configured channel (Slack, email, CLI dashboard).

Operator decision options:
- `APPROVE` — Executor resumes with the original action.
- `APPROVE_WITH_EDIT` — operator edits the payload (most common for outbound copy); Executor uses the edited version.
- `REJECT_TRY_AGAIN` — Executor retries with operator's stated correction.
- `REJECT_KILL` — node and its descendants marked failed; reflection filed.
- `ESCALATE_LATER` — keep pending; remind in N hours.

Approval queue auto-rejects pending items after `goal.guardrails.approval_ttl` (default 48h). Auto-rejection counts as a soft failure, not a kill.

---

## §8. Guardrail engine

Every external tool call goes through:

```
Executor → ToolCallProposal → GuardrailEngine → [allow|deny|require_approval] → ToolGateway → External
```

The Guardrail Engine runs these checks in order; first denial wins:

1. **KillSwitchCheck** — global, per-goal, per-channel flags (O(1) cached for ≤2s).
2. **ComplianceCheck** — EU AI Act disclosure on EU outbound, CAN-SPAM unsubscribe + physical address on US email, CASL consent on Canadian, etc.
3. **RateLimitCheck** — per-channel token bucket sized 30% below platform's published threshold.
4. **BrandVoiceCheck** — for content actions, run G-Eval brand-voice score; deny if below threshold.
5. **SchemaCheck** — payload conforms to the channel's expected schema.
6. **BudgetCheck** — proposed spend within goal's daily and per-experiment ceilings.
7. **AutonomyCheck** — action class is in `auto_proceed_for` list; otherwise route to approval queue.

Each check is implemented as a deterministic function. None of them call the LLM (they may use small fine-tuned classifiers for brand-voice, but no frontier-model judgment in the gate path itself — too slow, too expensive, too unreliable).

---

## §9. Tool gateway

The Tool Gateway is the **only path** from agents to external systems. It exposes a unified MCP-style interface over heterogeneous backends:

| Backend type | Examples | When used |
|---|---|---|
| **MCP servers** (preferred) | Markifact (300+ ad/CRM ops), Hyper MCP (200+ integrations), Windsor (325+ data sources), 0nMCP (995+ tools), Synapse (computer-use), **Leapable** (vault search + memory) | Whenever the platform exposes a real, supported MCP server |
| **Bespoke API adapters** | Custom GitHub, custom Discord webhook, custom Slack bot | Where MCP coverage is shallow but operator owns the integration |
| **Computer-use** | Synapse + Playwright + Browser-Use | When the platform has no API or the API is gated beyond the operator's tier (LinkedIn beyond Sales Nav, etc.); see `06_COMPUTER_USE_PLAYBOOK.md` |
| **Operator-mediated** | "Send this to operator's iMessage for hand-forward" | Tier-3 outreach; legal documents; never automated |

The Gateway records every call in `CF_AUDIT_LOG` with idempotency key, cost, latency, outcome, and side-effects.

---

## §10. Cost discipline

Two cost dimensions: **dollars** (paid spend, third-party tool costs) and **tokens** (LLM inference).

### 10.1 Dollar pacing

The Cost Pacer agent (`02_FIRST_PRINCIPLES.md §16`) is mechanical, not LLM-based. It maintains in-memory rolling sums:
- Last 1h spend per channel
- Last 24h spend per channel
- Last 7d spend per channel
- Last 30d spend per channel

Before any spending action, the Pacer's `permits_spend(amount, channel)` is called. Over-limit returns `EXHAUSTION` trip code.

### 10.2 Token pacing

Per-agent and per-cycle token budgets. Implemented as token-bucket on the LLM client wrapper. Over-budget runs trip `BudgetFuse` (AgentCircuit pattern).

### 10.3 Model routing

The Tool Gateway exposes both Opus-tier (reasoning-heavy) and Haiku-tier (formatting / extraction / classification) models. Default routing:
- Planner, Reflector, Aggregator, Evaluator → Opus (high stakes)
- Content Writer, Outreach Drafter → Sonnet
- Schema validation, classification, simple extraction → Haiku
- Brand-voice scorer, compliance check → fine-tuned classifier (no LLM)

Cost-aware model swapping is a cycle-end optimization, not a per-call decision.

---

## §11. Circuit breakers (mechanical)

Implemented as wrappers around the OuterLoop and around the Tool Gateway:

| Breaker | Trigger | Implementation |
|---|---|---|
| `Fuse` | 3 consecutive identical tool calls → loop detected | djb2 hash of (tool name + payload); count repeats; trip at 3 |
| `BudgetFuse` | Per-goal dollar or token budget exceeded | Atomic decrement on each call; trip on negative |
| `TimeoutFuse` | Single experiment node runs longer than its `time_budget_hours` | `started_at` + timeout; trip if exceeded |
| `StagnationFuse` | Predicate metric did not move for N consecutive cycles despite spend | Cycle-level comparison; trip at N (default 6 hourly cycles = ~one workday with no movement) |
| `GoodhartFuse` | Predicate metric vs. reality signal divergence > 2x over 14d | Daily auditor; trip on divergence |
| `BrandFuse` | Published content scores brand-voice violation post-hoc | Daily LLM-as-judge sweep; trip on violation |
| `ReputationFuse` | Sentiment / mention-tone delta on operator's public surfaces > N negative in 24h | External sentiment scan; trip on threshold |

Trip = halt all new external actions on the goal; surface incident to operator; require explicit `/goal resume` to continue.

---

## §12. Kill switches (operator-controlled)

Two layers:

### 12.1 Per-goal kill

```
/goal kill <goal_id>
```

Marks the goal `OPERATOR_PAUSED`. All open experiment nodes drained or canceled; in-flight actions completed where reversible-safe, otherwise aborted with best-effort cleanup. Final status report written.

### 12.2 Global kill

```
/marketing kill --reason="..."
```

All goals paused. All Executors return at next safe checkpoint. All approval-queue items frozen. Useful during incidents or PR crises.

Both layers are implemented as flag-checks at the runtime supervisor, **outside** the LLM agent loop. The LLM cannot reason past a kill flag. (Sakura Sky / AgentOS pattern.)

---

## §13. Concurrency model

The fleet uses:
- **Outer loop** — single-threaded sequential per goal (one ASSESS→PLAN→BUILD→COMMUNICATE cycle at a time).
- **Inner DAG** — parallel Executor dispatch up to per-channel concurrency caps.
- **Cross-goal** — goals run concurrently; the goal-level scheduler ensures shared resources (e.g., the same operator-owned X account) aren't double-booked.

Per-channel concurrency caps are set in the channel card and respect the platform's published rate limits with a 30% safety margin.

---

## §14. Observability + tracing

Every agent step emits a trace span (LangSmith / OpenTelemetry compatible). Spans carry:
- `goal_id`, `cycle_id`, `node_id`, `agent_class`
- Tool calls with inputs, outputs, costs, latencies
- LLM call inputs (cached / streamed), outputs, token counts, cost
- Approval queue activity
- Guardrail decisions (allow / deny / require_approval) with reason
- Errors with full stack traces

Operator dashboard shows:
- Active goals with progress bars
- Pending approvals
- Last 24h spend by goal and channel
- Open circuit-breaker alerts
- Last hourly cycle summary

A weekly report compiles cycle outcomes into operator-readable form (markdown), highlighting biggest movers and biggest reflexions.

---

## §15. Multi-product, single-fleet

The fleet is shared across all three products (Leapable, Synapse, Polis). Goals scope tools, content, and contacts to a product. The same Content Writer agent class serves all three; it loads the product-specific brand voice and channel cards at start.

This means the operator does not need three separate marketing setups. One platform, three goal contexts.

---

## §16. AgentHub-style attempt DAG

Every goal-execution session is recorded as a **sprawling DAG of attempt commits**, AgentHub-style. Each attempt is a node; the DAG records:
- Parent attempt(s) it forked from
- Goal_id, cycle_id
- The agents that ran
- The outcome score
- The artifacts produced

This is **distinct** from the operational DAG (which is the live experiment graph). The attempt DAG is the **historical record** that the Planner queries when it asks "have we tried something like this before?" Karpathy's AgentHub demonstrated this works for autoresearch; we apply it to autonomous marketing.

The wiki indexes the attempt DAG by `(channel, tactic, outcome_class)` so the Planner can ask "show me every PH launch attempt; which subset of variables correlated with success?" and get a structured answer.

---

## §17. Cross-references

- Agent class definitions: `05_AGENT_FLEET_DESIGN.md`
- Tool surface: `16_TECH_STACK.md`
- Computer-use specifics: `06_COMPUTER_USE_PLAYBOOK.md`
- Approval queue UX: `19_OPERATIONS_AND_CADENCE.md §3`
- Circuit-breaker and kill-switch details: `20_RISK_AND_FAILURE_MODES.md`
- Memory schema: `16_TECH_STACK.md §5`
- Goal spec: `22_GOAL_SPEC_FORMAT.md`


---

<a id="05-agent-fleet-design"></a>
<!-- source: 05_AGENT_FLEET_DESIGN.md -->

# 05 — Agent Fleet Design

> Every agent class, its responsibility, its inputs, its tool surface, its evaluator, its autonomy default. Slot identity is sacred (`02_FIRST_PRINCIPLES.md §13`): one agent class, one job. Multi-step workflows compose across classes through the Orchestrator.

---

## §1. Agent class catalog

26 classes, grouped by role.

### 1.1 Meta agents (the loop itself)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| M1 | **Operator Agent** | Conversational interface to the human; never proposes experiments unprompted | `AUTO` |
| M2 | **Goal Parser** | Validates `/goal` specs against schema; rejects unsafe predicates | `AUTO` |
| M3 | **Assessor** | Per-cycle reads goal status, signals, predicate trend, open items | `AUTO` |
| M4 | **Planner** | Decomposes non-atomic nodes into MECE child DAGs with expected impact | `AUTO` for content/channel-tactical nodes; `REQUIRE_APPROVAL` for net-new channel adoption |
| M5 | **Atomizer** | Decides atomic-or-decompose per node | `AUTO` |
| M6 | **Aggregator** | Rolls up child results to parent | `AUTO` |
| M7 | **Evaluator** | LLM-as-judge scoring + deterministic metric readback | `AUTO` |
| M8 | **Reflector** | Writes structured post-mortems on failures | `AUTO` |
| M9 | **Communicator** | End-of-cycle digest writer | `AUTO` |
| M10 | **Wiki Gardener** | Weekly memory cleanup, dedup, cross-reference | `AUTO` |

### 1.2 Sensing agents (read the world)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| S1 | **Audience Sorter** | Scores incoming contacts into T1 / T2 / T3 (`03_TIER_MODEL_AUDIENCE.md §3`) | `AUTO` |
| S2 | **Signal Listener** | Polls social/web/news for mentions, references, competitor moves | `AUTO` |
| S3 | **Influence Network Mapper** | Walks LinkedIn / Crunchbase / portfolio graphs to find warm-intro paths | `AUTO` |
| S4 | **Goodhart Auditor** | Daily check: does predicate movement correlate with reality signals? | `AUTO` |
| S5 | **Competitive Intelligence** | Monitors competitor pricing, positioning, launches | `AUTO` |
| S6 | **Sentiment Tracker** | Aggregates brand sentiment across mentions; tracks delta | `AUTO` |

### 1.3 Producing agents (make things)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| P1 | **Content Writer** | Draft long-form, short-form, threads, posts; reads brand voice | `LOG` (publish after eval gate; sample audit) |
| P2 | **Technical Writer** | Tutorials, READMEs, benchmark posts, deep code-led content for Leapable/Synapse | `LOG` |
| P3 | **Visual Generator** | Images, diagrams, video stitching, thumbnails | `LOG` |
| P4 | **Brief Generator** | From topic → structured brief for downstream writers | `AUTO` |
| P5 | **Persona Carrier** | Writes in a specific operator persona (if multi-persona) | `LOG` w/ persona-specific brand-voice gate |

### 1.4 Distributing agents (move things into the world)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| D1 | **Multi-Platform Publisher** | Cross-formats and schedules drafts to X / LinkedIn / Dev.to / Hashnode / Medium / Mastodon / Bluesky / Reddit (per channel card) | `LOG` for drafts that passed Evaluator; `REQUIRE_APPROVAL` for first-time-on-channel |
| D2 | **Community Operator** | Discord/Slack/Reddit/forum engagement on operator-owned accounts within community-specific playbooks | `LOG` for established communities; `REQUIRE_APPROVAL` for new ones |
| D3 | **GitHub Maintainer** | Issue triage, PR review, release notes, README updates | `LOG` for docs; `REQUIRE_APPROVAL` for code changes |
| D4 | **Outreach Drafter** (T2) | Personalized cold + warm follow-up emails; drafts, never sends without approval | `REQUIRE_APPROVAL` |
| D5 | **Warm-Intro Drafter** (T3) | Drafts forwardable intro paragraphs in the introducer's voice | `REQUIRE_APPROVAL` always (no override) |
| D6 | **Comment Triage** | Replies to comments on operator's content within voice + escalation rules | `LOG` for short factual replies; `REQUIRE_APPROVAL` for substantive |
| D7 | **Newsletter Pitcher** | Pitches operator to host on other people's newsletters | `REQUIRE_APPROVAL` for first contact; `LOG` for follow-up |
| D8 | **Podcast Booker** | Drafts pitches to podcast hosts; tracks 50-show tiered list | `REQUIRE_APPROVAL` |
| D9 | **Event Coordinator** | Hackathon / meetup / conference RSVP, agenda, booth/talk pitches | `REQUIRE_APPROVAL` |

### 1.5 Optimizing agents (move the number)

| ID | Class | Role | Autonomy default |
|---|---|---|---|
| O1 | **SEO Strategist** | Keyword research, content gap analysis, programmatic SEO templates | `AUTO` for research; `LOG` for pSEO publishing within evaluator gate |
| O2 | **GEO Optimizer** | Structures content for citation in ChatGPT / Perplexity / Claude / Gemini answers | `AUTO` for restructuring; `LOG` for publishing |
| O3 | **Referral Engineer** | Designs and tweaks viral loops; A/B tests referral mechanics | `REQUIRE_APPROVAL` for product-side changes; `LOG` for landing-page copy |
| O4 | **Ad Operator** | Paid ad campaign management within budget envelope (Google, Meta, LinkedIn, X, TikTok Ads via Markifact MCP) | `REQUIRE_APPROVAL` for new campaigns; `LOG` for in-flight optimization within budget |
| O5 | **Conversion Optimizer** | Landing page A/B tests, signup funnel changes | `REQUIRE_APPROVAL` for product writes |
| O6 | **Cost Pacer** | Mechanical (not LLM); enforces dollar pacing and reroutes between channels | `AUTO` |
| O7 | **Cohort Health Auditor** | Tracks per-channel cohort retention; identifies channels acquiring bad-fit users | `AUTO` |

---

## §2. Per-agent specification template

Every agent class has a card with the same shape, stored in `agents/<id>.md`. Example for the Content Writer:

```yaml
---
id: P1
name: Content Writer
class: producing
slot_identity: "Draft on-brand content (long-form posts, threads, social copy, blog drafts)"
input_schema: contracts/p1_input.schema.json
output_schema: contracts/p1_output.schema.json
tools_allowed:
  - wiki_search
  - wiki_get
  - brand_voice_score
  - readability_score
  - draft_save
tools_forbidden:
  - publish_*
  - send_*
  - ad_*
  - approval_*
model_routing:
  default: sonnet
  high_stakes_override: opus     # for cornerstone content
  cost_floor_override: haiku     # for short-form variants
context_load:
  - skills/<channel>.md (read first)
  - brand_voice/<product>.md
  - reflections/(channel,content) (last 20 by recency)
evaluator: E1  # Brand Voice + Readability
retry_policy: 3 attempts with Reflexion memory
cost_budget_per_run: $0.40 default; goal-spec can override
autonomy_default: LOG
---

# Content Writer (P1)

## Responsibility
Produces a content artifact (long-form, short-form, thread) per request. Does NOT publish. Does NOT decide what to write about (that's the Brief Generator / Planner). Does NOT optimize for SEO (that's O1). Does NOT cross-format for multiple channels (that's the Multi-Platform Publisher).

## Inputs
- `brief`: structured brief from P4 with topic, primary keyword (for SEO content), target reader profile, target word count, structural skeleton (H1/H2 hierarchy), references to evidence, CTAs.
- `channel`: target channel for first publish (drives format conventions).
- `voice_signature`: brand voice spec for the product.

## Outputs
- `draft_md`: markdown draft.
- `metadata`: {title, subtitle, hook, key_takeaways, suggested_visuals, internal_links}.
- `brand_voice_score`: 0–1 from automated check.
- `evidence_citations`: list of references used.

## Forbidden behaviors
- Quoting sources without verifying they exist (Reflector files all hallucination incidents).
- Using forbidden phrases from the brand voice's forbidden list.
- Publishing claims about product capabilities that don't exist (`#product_lying_audit` ribbon).

## Failure modes
- Brand voice drift over many drafts (mitigated by sample-audit retro).
- Repeating the same hook structure across drafts (lint check in evaluator).
- Hallucinating quotes from operator's persona (forbidden phrase + structured-citation check).
```

Every agent class has a card like this. The full set lives in `agents/*.md`; this doc enumerates the catalog.

---

## §3. The Evaluator stack

The Evaluator is not a single agent. It is a **layered stack**, run on each Executor output:

| Layer | Implementation | Cost | What it catches |
|---|---|---|---|
| L1 — Schema | Deterministic JSON Schema validator | ~free | Malformed outputs, missing required fields |
| L2 — Deterministic | Regex / lint / hash / length checks | ~free | Forbidden phrases, duplicate content, length violations |
| L3 — Classifier | Fine-tuned small model (4B-class) for brand voice, toxicity, PII | <50ms | Brand-voice drift, leaked PII, banned tone |
| L4 — LLM-as-judge | G-Eval / AdaRubric rubrics on Sonnet | ~$0.01-0.05 | Quality, helpfulness, on-rubric coherence, factuality |
| L5 — Reality readback | Direct SoT query against product DB | ~free | Did the artifact's intended effect actually happen? |

Layers run in order. L1 / L2 / L3 are gates (deny on fail). L4 / L5 are diagnostics (feed back to Reflexion; failures don't gate but trigger replanning).

LLM-as-judge rubrics are **calibrated quarterly** against 50–200 human-labeled examples per rubric (Kappa ≥ 0.6 target for brand-voice rubrics; Kappa ≥ 0.85 for compliance rubrics). G-Eval methodology per `15_METRICS_AND_OBSERVABILITY.md §7`.

---

## §4. The Wiki Gardener (M10)

Runs weekly. Reads the entire wiki, identifies:
- Pages not referenced by any agent in 90 days (candidate for archival)
- Pages with conflicting claims (surfaces for operator triage)
- Skill pages that have absorbed too many reflections (candidate for refactoring)
- Channel cards that are stale (last update > 30d) — pings the relevant Executor to refresh

Output: a Gardener Digest sent to the operator with proposed prune / merge / refactor actions. Operator approves; Gardener executes.

Borrowed from Karpathy's LLM Wiki: a knowledge base grows in value with maintenance, and stale entries actively mislead. The Gardener prevents wiki rot.

---

## §5. Agent-to-agent communication

Two channels:

### 5.1 Structured handoff (default)

Per-node, parent → child handoffs use **structured `ExecutionContext` envelopes**: parent specifies the child's `inputs`, `constraints`, `expected_output_schema`, `time_budget`, `cost_budget`. Child returns `ExecutionResult`. No natural-language chitchat across nodes; the wire is JSON.

This is the default for Planner → Executor and child → parent rollups.

### 5.2 Message board (AgentHub-style)

For coordination that doesn't fit a parent-child DAG (e.g., the Signal Listener observes a viral mention and needs to surface to whichever agent should act on it), agents post to a shared **MessageBoard** keyed by topic and goal. Other agents subscribe to relevant topics. The board is append-only and queryable.

Topics: `signal:mention`, `signal:competitor`, `signal:regulatory`, `incident:rate_limit`, `incident:brand_violation`, `proposal:product_change`, `goal:status_change`, `approval:granted`, `approval:rejected`.

Agents on a topic process new messages on their next tick. The MessageBoard is **how the Goodhart Auditor halts the world**: it posts to `incident:goodhart` which the Outer Loop monitors with a sub-second poll.

---

## §6. Specialized agent deep-dives

### 6.1 Influence Network Mapper (S3)

**Inputs:** the operator's known network (LinkedIn export, Gmail contacts), the operator-confirmed Tier-3 target list, current goal.

**Process:**
1. Build a graph: nodes = people, edges = (shared employer, shared board, shared advisor, mutual LinkedIn connection, mutual GitHub-org membership, mutual portfolio-co membership, mutual investor).
2. For each Tier-3 target, BFS from operator-side to find shortest warm path.
3. Score paths by edge strength (e.g., "ex-coworker 2017–2019" > "second-degree LinkedIn connection").
4. Output: per-target, top 3 viable bridges with one-paragraph "why this person could intro" and the proposed intro reason.

**Tools:** `linkedin_query` (via Hyper MCP or computer-use), `crunchbase_query`, `gh_org_members`, `contact_graph_read`.

**Forbidden:** scraping personal data beyond what's publicly indexed; storing inferred attributes (sexual orientation, political affiliation, health) under any circumstances; sending DMs (drafting only).

**Output goes to:** Warm-Intro Drafter (D5).

### 6.2 Goodhart Auditor (S4)

**Cron:** 24h.

**Process:**
1. For each active goal, compute current predicate value and three correlated reality signals (per goal-spec).
2. Compute 14d trend slope on predicate.
3. Compute 14d trend slope on each reality signal.
4. If predicate slope > 2 × max(reality signals slope): post `incident:goodhart` to MessageBoard. Outer Loop monitors this topic and trips `GoodhartFuse`.
5. Write Auditor Note to the wiki: which signal diverged, why this might be the case (LLM-generated hypothesis), what to check.

**Tools:** `metric_query`, `wiki_create`, `messageboard_post`.

### 6.3 Brand-Voice Scorer (sub-component of Evaluator)

Not a full agent class; a tool the Evaluator calls. Fine-tuned classifier on operator's prior approved content. Output: `{score: 0..1, dimensions: {tone, formality, vocabulary, structure, claim_style}}`. Threshold default 0.85; can be overridden per channel card.

Re-trained on operator's manual edits over time (the operator's reject + correct in the approval queue is training data).

### 6.4 Cost Pacer (O6)

Mechanical, not LLM. A single goroutine / coroutine maintaining rolling sums per channel × goal. Exposes `permits_spend(channel, amount_usd) -> bool`. Called pre-tool by the Guardrail Engine.

Logs every permit denial to `audit:cost_denial` topic so the Planner can replan around channel exhaustion.

### 6.5 Persona Carrier (P5)

For multi-persona accounts (e.g., the operator runs three X handles for three audiences). Each persona has:
- Own brand voice signature
- Own posting history
- Own cadence cards
- Own forbidden topics list

The Persona Carrier loads the persona's full context before generating. The Evaluator gates with persona-specific rubrics. Cross-persona content leakage (e.g., the "Polis" persona accidentally posting in the "Leapable" voice) is a deny gate.

---

## §7. Concurrency caps per agent class

To prevent runaway parallelism:

| Class | Concurrent max | Per-cycle max |
|---|---:|---:|
| Content Writer | 4 | 20 |
| Multi-Platform Publisher | 2 (per channel) | 8 (per channel) |
| Community Operator | 1 (per community) | 6 (per community per day) |
| Outreach Drafter | 4 | 30 (per goal per day) |
| Warm-Intro Drafter | 2 | 5 (per goal per week) |
| Ad Operator | 1 (per ad account) | bounded by daily spend |
| GitHub Maintainer | 2 | 15 (per repo) |
| Signal Listener | 1 per source | continuous |
| Goodhart Auditor | 1 | 1 (per 24h) |

These are per-goal; the global cap is the sum across goals, subject to the global token budget and the Cost Pacer.

---

## §8. Operator personas — agent perspective

When the operator owns multiple personae (e.g., personal account + product account + persona-X), the agent fleet treats each as a **separate channel-binding context**. Cross-context content leakage is denied (`§6.5`). The operator can grant agents access to one or many personae per goal.

This is **not** synthetic-persona generation. Every persona corresponds to a real operator-controlled account with documented ownership. Manufacturing synthetic identities to amplify content is forbidden (`02_FIRST_PRINCIPLES.md §19`).

---

## §9. Agent versioning and rollout

Each agent class is versioned. A new prompt, model swap, tool surface change, or evaluator-rubric change ships as a new version. Rollout is gradual:

1. **Shadow** — new version runs on real inputs; outputs compared against current version's outputs; never publishes.
2. **Canary** — new version handles 5% of traffic on the lowest-risk action class; outputs reviewed.
3. **Phased** — 25%, then 50%, then 100% over a week.
4. **Roll back** automatic if Evaluator quality drops or any kill switch trips during phased rollout.

The bench-delta gate (`02_FIRST_PRINCIPLES.md §14`) is non-negotiable before phase 1.

---

## §10. Cross-references

- Tool surface per agent class: `16_TECH_STACK.md`
- The Evaluator's rubric library and calibration: `15_METRICS_AND_OBSERVABILITY.md §7`
- Computer-use specifics for D2 / D6 / D7 / S3: `06_COMPUTER_USE_PLAYBOOK.md`
- Channel cards reference: `08_DISTRIBUTION_CHANNELS.md`
- Brand voice spec format: `07_CONTENT_FACTORY.md §4`
- Goal spec: `22_GOAL_SPEC_FORMAT.md`


---

<a id="06-computer-use-playbook"></a>
<!-- source: 06_COMPUTER_USE_PLAYBOOK.md -->

# 06 — Computer-Use Playbook

> Synapse + Playwright + Browser-Use give the agent fleet the ability to operate any web platform a real human can — including the platforms with no API, with gated APIs, or with APIs so expensive they aren't economical at the operator's scale. This doc is the constraint sheet: how to do that without (a) burning the operator's accounts, (b) violating platform ToS, (c) generating brand-damaging "obvious AI" behavior, or (d) crossing into coordinated-inauthentic-behavior territory.

---

## §1. Why computer-use matters

In 2026, most of the channels that matter for the three products do not have economically viable agent-grade APIs:

| Channel | Real API? | Why computer-use anyway |
|---|---|---|
| X / Twitter | Yes, but $42k+/mo Enterprise tier for full posting + replying access | Operator-account-tier ops are economical via Synapse |
| LinkedIn | Posts/personal: very gated; Sales Nav: paid; full DM/connection: scrape-only | Computer-use is the only way to operate at the volume the engaged tier requires |
| Reddit | Yes, but rate limits + brand-detection make API agents look obvious | Computer-use matches human posting cadence better |
| Discord | Bots work for bot-tagged channels; community engagement as a human requires logged-in browser | Synapse logs into operator-owned account |
| Slack workspaces (operator joined) | App approval per workspace, often slow | Synapse operates as operator |
| TikTok | Creator API gated; comments/engagement gated; uploads via API are detection-flagged | Synapse uploads via the web client |
| Instagram | Graph API is permissioned and limited; commenting is gated | Synapse operates as operator |
| Hacker News | No real API for posting; one item per ~25 minutes from a logged-in account | Synapse logs in to operator's HN account |
| Product Hunt | API for listings; comments + maker engagement via browser | Synapse for engagement window |
| YouTube comments | Limited API; thread comments effectively browser-only at human cadence | Synapse for replies |
| GitHub | Excellent API (PyGithub, gh CLI) | API preferred; computer-use only for UI-only flows (sponsor signup, etc.) |
| Email + Calendar | Excellent API (Gmail, MS Graph) | API preferred |
| Ad platforms (Google, Meta, TikTok Ads, LinkedIn Ads, X Ads) | Yes via Markifact / Hyper MCP / Windsor | API preferred |
| CRM (HubSpot, Salesforce, Pipedrive, Klaviyo) | Yes via Markifact / Hyper MCP / Windsor | API preferred |

**Rule:** API where available and economical. Computer-use elsewhere. Never both at once on the same account (avoids fingerprint conflicts).

---

## §2. The three computer-use surfaces

### 2.1 Synapse (primary)

The operator-owned Synapse MCP server gives the agent fleet:

- **Perception:** `observe`, `find`, `read_text` — read the screen and the UIA accessibility tree.
- **Action:** `act_click`, `act_type`, `act_press`, `act_aim`, `act_drag`, `act_scroll`, `act_pad`, `act_clipboard` — operate the OS at a human-shaped rate.
- **Reflexes:** `reflex_register` etc. — sub-frame reactive controllers for tasks that need <16ms latency (rarely needed for marketing; useful for capturing an ephemeral status drop or live-stream comment).
- **Safety:** `release_all`, operator panic hotkey (Ctrl+Alt+Shift+P) — the operator can stop everything in 50ms.

Synapse is the **default** for any platform that requires logging in as the operator on a real browser/desktop session.

### 2.2 Playwright (parallel scale)

For tasks that need parallel browser sessions across **many** scrap-class targets (e.g., influence network mapping, public LinkedIn profile data, public job listings), Playwright runs headless on a separate machine pool with rotating non-operator-account residential IPs.

**Important:** Playwright is for **read-only** parallel scraping of *public* surfaces. It is **not** used to operate operator accounts. If the action involves login as the operator, it goes through Synapse.

### 2.3 Browser-Use / OpenClaw-style adapters

For interactive flows that benefit from goal-driven LLM-in-the-loop browsing (e.g., "find the right contact-form on a partner's website and submit our partnership inquiry"), Browser-Use / OpenClaw provides natural-language browser automation. The agent describes the goal; the tool steers the browser.

These adapters wrap the same Tool Gateway surface so the rate-limit, brand-voice, and compliance gates still apply.

---

## §3. Operator-account doctrine

### 3.1 Account registry

Every operator-owned account the fleet may operate is registered in `accounts.yaml`:

```yaml
- account_id: x_chris_personal
  platform: x.com
  persona: chris (operator's primary)
  url: https://x.com/<handle>
  owner_documented: yes
  recovery_phone_owned_by: operator
  recovery_email_owned_by: operator
  agents_allowed:
    - D1  # Multi-Platform Publisher
    - D6  # Comment Triage
    - S2  # Signal Listener (read-only)
  rate_limits:
    posts_per_24h: 4
    replies_per_24h: 25
    likes_per_24h: 60
    follows_per_24h: 8
    dms_per_24h: 5
  brand_voice_spec_path: docs/brandvoices/chris.md
  ai_disclosure_required: true  # EU contacts only
```

No account = no agent operations. The Guardrail Engine rejects tool calls referencing unregistered account IDs.

### 3.2 No multi-account amplification

The fleet never spins up additional accounts to amplify the operator's content. No engagement farming. No upvote rings. No follower-purchasing. Synthetic-persona manufacturing is forbidden (`02_FIRST_PRINCIPLES.md §19`).

If the operator legitimately runs multiple personas (e.g., a personal account + a product account + a niche-persona account), each is registered separately with its own brand voice and the Persona Carrier (P5) enforces non-leakage.

### 3.3 No credential sharing

Each agent receives **per-account scoped tokens** with the smallest possible permission set. The Operator Agent rotates tokens monthly. Synapse sessions on the operator's desktop authenticate via OS-level keychain access, not by storing passwords in agent prompts.

---

## §4. Rate limiting — the 30%-below rule

Each platform has a published or empirically-known rate-limit threshold above which bot detection fires. The fleet sets internal rate caps at **30% below** that threshold by default. The caps live in `channel_cards/<platform>.yaml`:

```yaml
platform: x.com
detection_thresholds_published:
  posts_per_24h: 100  # well-known soft cap before review
  replies_per_24h: 100
  follows_per_24h: 50
  likes_per_24h: ~200
  dms_per_24h: 50  # to non-followers; followers higher
internal_rate_caps:
  posts_per_24h: 4    # operator preference; well below detection
  replies_per_24h: 25
  follows_per_24h: 8
  likes_per_24h: 60
  dms_per_24h: 5
posting_cadence_distribution: stochastic_log_normal_mean_4h_stddev_2.5h
detection_signals_to_avoid:
  - perfectly_periodic_posting
  - identical_phrasing_across_replies
  - rapid_fire_likes_to_new_accounts
  - all_actions_from_same_IP_block
  - DM_template_within_10_seconds_of_follow
```

The `posting_cadence_distribution` is the key non-obvious gate: real humans post at irregular intervals (log-normal-ish around their habit). The fleet samples from a distribution rather than firing on a fixed schedule.

### 4.1 Stochastic timing

Every external action that has a delay component (post, reply, follow, like) is enqueued with a **sampled delay** from the channel's distribution. The action does not execute at the planner's target time; it executes between [target − jitter] and [target + jitter] where jitter is drawn from the distribution. This makes the fleet indistinguishable from a real but disciplined human operator at the macro level.

### 4.2 Token-bucket implementation

Per channel × account, a token bucket refills at the internal-cap rate. The Guardrail Engine's `RateLimitCheck` decrements the bucket on each call. Over-empty = `RATE_LIMIT_DENY`. The Planner sees the denial and reschedules; it does not retry-loop. Repeated denials trip the `Fuse` circuit breaker.

---

## §5. The Hormozi Volume question

Alex Hormozi's content advice: "We put out 450 pieces of content per week." "One reel a day is not enough."

The fleet **can** sustain that volume, but **must not** sustain it on a single channel × account beyond the rate cap. The reconciliation:

| Pattern | Volume strategy |
|---|---|
| Hormozi-volume on **owned surfaces** (operator's website blog, operator's newsletter, operator's YouTube) | Yes — no third-party rate limit |
| Hormozi-volume on **operator's** social handles | No — respect platform rate caps |
| Hormozi-volume **across operator's surfaces by repurposing one core piece into N formats** | Yes — this is the actual move |

The Content Factory (`07_CONTENT_FACTORY.md`) is the implementation. One long-form anchor → 8 short-form derivatives → 4 platform-native format adaptations → channel-paced publishing per the rate cards.

**Net effect:** 1 long-form piece × 12 formats × 8 platforms = 96 publishings per week, every one of which sits inside its channel's rate cap and looks like normal cadence on that channel. This matches Hormozi-volume on **aggregate reach** without violating any single platform's threshold.

---

## §6. Anti-detection content patterns

Beyond rate limits, modern platform detection looks at content patterns:

| Detection signal | Anti-pattern (fleet must not do) | Mitigation |
|---|---|---|
| Repeated phrasing across replies | "Thanks for the comment! Great point!" repeated | Comment Triage uses ≥3 variant templates + LLM rephrasing on every reply |
| All replies start with the same word | "Yes" / "Yes" / "Yes" | Lint check denies same-first-word streak >2 |
| Mass-engaging only on follower-of-follower accounts | Suspicious engagement graph | Mix engagement: peers, larger accounts, smaller accounts, randoms |
| Replies under 8 seconds after a post | Inhuman speed | Min delay 30s on replies; sampled from distribution |
| Identical link UTMs across all posts | Trackable as a single campaign | Per-channel UTM variation |
| AI-detection signatures in copy (em dashes, "delve", "tapestry", "navigate the complexities") | Modern LLM-written prose markers | Brand-voice evaluator scores AI-likelihood; rewrite above threshold |
| Engagement only during one timezone band | Reveals automation centralization | Stochastic distribution across operator's actual active hours |

The Brand Voice Scorer (`05_AGENT_FLEET_DESIGN.md §6.3`) is the primary defense. It is fine-tuned on the operator's approved content. Content that scores too LLM-shaped is rewritten before publish.

---

## §7. EU AI Act Article 50 disclosure (August 2026)

For content **created or substantially modified by AI** delivered to EU recipients, disclosure is required. The fleet handles this mechanically:

```
crates/marketing-plan/templates/eu_disclosure.md:
> Parts of this message were drafted with AI assistance.
```

**Implementation:**
- Every outbound email tool has a wrapper `send_email_eu(...)` that injects the disclosure if the recipient's resolved jurisdiction is EU/EEA.
- Every published post that goes to an account with >5% EU follower share carries the disclosure as the last line.
- "Substantially modified" is the trigger; pure operator-written content with light LLM polish does **not** need disclosure.

The disclosure is a tool-side wrapper, not a prompt instruction. (`02_FIRST_PRINCIPLES.md §3` — trust the harness.)

---

## §8. Per-platform playbooks

The full per-platform behavioral specs live in `channel_cards/<platform>.md`. Summary:

### 8.1 X / Twitter

- Operator account login via Synapse on operator desktop.
- 4 posts / 24h cap; stochastic distribution mean ~6h apart.
- Threads: max 1 / 24h for substantive multi-tweet threads.
- Replies to mentions: 25 / 24h; min 30s after notification; LLM-judged for substantive vs ack-only.
- Likes: 60 / 24h; varied targets (not just top-of-feed).
- DMs: 5 / 24h to non-followers, with `REQUIRE_APPROVAL` always; followers higher cap but still gated.
- Profile changes: `REQUIRE_APPROVAL` always.

### 8.2 LinkedIn

- Operator account.
- 1 post / 24h on personal; 2 / 24h on company page.
- Comments on others' posts: 20 / 24h; substantive comments only.
- Connection requests: 8 / 24h; only to relevance-scored prospects; `REQUIRE_APPROVAL` for net-new connections.
- DMs (post-connection): 5 / 24h; `REQUIRE_APPROVAL` for first message; `LOG` for follow-ups in established thread.

### 8.3 Reddit

- Operator account with karma > 500 minimum before any product mention.
- 4 substantive comments / 24h across subreddits.
- 1 post / 72h max; subreddit-specific rules in each card.
- No product mention until karma + reputation threshold in target subreddit (Joe Karlsson DevRel rule: earn the right to post).
- Cross-post detection: never post identical content across subreddits.

### 8.4 Hacker News

- Operator account.
- Comments: 6 / 24h; substantive only; no boilerplate.
- Show HN: 1 / 90 days maximum; coordinated launch (see PH launch playbook).
- Upvote rings: never. Even between operator's personal account and product launches.

### 8.5 Discord

- Operator account in each registered server.
- Replies: server-specific; default 10 / 24h per server.
- DMs: `REQUIRE_APPROVAL` always.
- Read-only listening: unrestricted (for Signal Listener S2).

### 8.6 TikTok

- Operator account.
- 3 uploads / 24h max (Polis especially).
- Comments: 20 / 24h.
- No bot likes / no engagement pods.

### 8.7 Email (any jurisdiction)

- API preferred (Gmail, MS Graph).
- CAN-SPAM block on every commercial email: physical address, unsubscribe link, accurate from.
- GDPR legitimate-interest documentation per recipient region.
- EU AI Act disclosure if AI-generated/AI-modified.
- Cold outbound: 30 / 24h sustained max; first message `REQUIRE_APPROVAL` per net-new recipient.

---

## §9. Auditability

Every computer-use session is recorded:

- Full DOM/UIA snapshots at decision points
- Screen captures on operator-requested audit only (privacy default off; on-demand)
- Action log: timestamp, tool, payload, latency, outcome, idempotency key
- Tracing: LangSmith / OpenTelemetry span per action

Operator can run `marketing audit --account=x_chris_personal --since=24h` and see every action with replay capability.

---

## §10. Recovery from account suspension

Despite all the rate-limiting, platforms occasionally false-positive. The recovery playbook:

1. **Stop all activity on the affected platform across all goals immediately** (kill switch).
2. File the platform's standard appeal — Operator Agent drafts; operator sends.
3. Reflector files a structured post-mortem: what tool sequence preceded suspension?
4. Update the channel card: lower the rate cap; widen the cadence distribution; add the detected signal to the avoid list.
5. Resume only after platform reinstates **and** the channel card update has been operator-approved.

Suspension is treated as a critical learning event, not a routine. Repeat suspensions on the same platform → permanent deny of that platform until operator-level architectural review.

---

## §11. Cross-references

- Synapse contract (Action/Perception/Reflex): see `leapablememory` Synapse systemspec docs.
- Channel cards: `channel_cards/*.yaml` (per-platform operational state)
- Brand Voice Scorer: `05_AGENT_FLEET_DESIGN.md §6.3`
- Compliance details: `14_COMPLIANCE_AND_ETHICS.md`
- Auditability and operator dashboards: `19_OPERATIONS_AND_CADENCE.md`
- Account registry schema: `16_TECH_STACK.md §6`


---

<a id="07-content-factory"></a>
<!-- source: 07_CONTENT_FACTORY.md -->

# 07 — Content Factory

> The production pipeline. Six layers (research → brief → draft → distribute → AI-search visibility → attribution), tuned for Hormozi-volume on owned surfaces, channel-respectful cadence on third-party surfaces, GEO/SEO native, brand-voice-gated. One anchor piece × N derivatives × N platforms = Hormozi's "450 pieces per week" without any one channel seeing more than its rate cap.

---

## §1. The pipeline (Knowlee 6-layer)

Adapted from the AI marketing agent fleet pattern. Each layer is owned by one or more agent classes from `05_AGENT_FLEET_DESIGN.md`:

```
RESEARCH ──► BRIEF ──► DRAFT ──► DISTRIBUTE ──► AI-SEARCH VISIBILITY ──► ATTRIBUTION
   S5/O1     P4         P1/P2     D1            O2                       O7/M7
```

| Layer | Owner agents | Output | Time per artifact |
|---|---|---|---|
| Research | S5 Competitive Intel + O1 SEO + S2 Signal Listener + Leapable wiki query | `topic_brief.yaml` | ~5 min |
| Brief | P4 Brief Generator | Structured brief with H1/H2 outline, target keyword, target reader, CTAs, evidence pointers, voice notes | ~3 min |
| Draft | P1 Content Writer (or P2 Technical Writer) | Long-form markdown anchor; brand-voice scored | ~10 min |
| Distribute | D1 Multi-Platform Publisher | 8 short-form derivatives × N platforms, scheduled per channel cards | ~15 min |
| AI-search visibility | O2 GEO Optimizer | Structured FAQ + comparison tables + citations to maximize ChatGPT/Claude/Perplexity quotation | ~5 min |
| Attribution | O7 Cohort Health + M7 Evaluator | SoT readback: did the artifact move the goal predicate? | ongoing |

Total: ~40 minutes of agent compute per anchor piece. Cost per anchor at default model routing: ~$1.50–$3.00. Hormozi-recommended cadence is one anchor per day for serious operators.

---

## §2. Hormozi volume reconciled with platform respect

> Hormozi: "We put out 450 pieces of content per week."
> Joe Karlsson DevRel: "Pick one or two community channels and go deep. Not five."

These are not contradictory. They are two different surfaces:

- **Owned surfaces (Hormozi's volume target):** the operator's website, blog, podcast, YouTube channel, newsletter. No rate cap. Volume = leverage.
- **Third-party platforms (DevRel "go deep"):** GitHub, Discord, Reddit, HN. Volume here = automated-looking. Depth, not breadth.

The factory produces volume on owned surfaces and **derivative cadence** on third-party platforms. One anchor piece per day, refactored into many formats, distributed at each channel's natural pace.

### 2.1 The one-to-many amplifier

```
1 daily anchor (long-form, ~2000 words)
  │
  ├── 1 YouTube long (10-15 min)
  ├── 1 podcast episode (audio strip)
  ├── 1 X thread (8-12 tweets)
  ├── 1 LinkedIn post (1200 chars)
  ├── 1 newsletter blurb (300 words)
  ├── 1 Dev.to / Hashnode cross-post (technical anchor)
  ├── 1 Mastodon post
  ├── 1 Bluesky post
  ├── 4 short-form vertical clips (TikTok / IG Reels / YT Shorts / X video)
  ├── 1 carousel (LinkedIn or Instagram)
  ├── 1 infographic
  └── 3 quote graphics (per high-leverage line in the anchor)
```

Total artifacts per anchor: **18+**. Per week with daily anchor: **126+**. Per month: **540+**. This is Hormozi-volume, achieved through aggressive repurposing, not through agent spam.

Joe Karlsson's rule still binds: of those 540 artifacts, the **depth** investments — actual responses in Discord, comments under others' posts, GitHub issue triage, podcast appearances — get the deep treatment. The repurposing is bulk; the engagement is hand-shaped.

---

## §3. Brand voice spec

The single most-violated thing in autonomous content systems is brand voice. The factory's defense is a structured brand voice document plus a fine-tuned evaluator.

### 3.1 The brand voice doc

`brandvoices/<product>.md`:

```yaml
---
product: leapable
last_updated: 2026-05-26
calibration_kappa: 0.78  # against operator labels
---

# Leapable Brand Voice

## Stance
- Technical first. We assume the reader is a working AI engineer.
- Direct. No throat-clearing. Lead with the claim; back with evidence.
- Verifiable. Every quantitative claim must come from a cited measurement.

## Voice
- Confident but not arrogant. We benchmark against ourselves and against named alternatives.
- We do not say "delve," "tapestry," "in the realm of," "the journey," "unleashing," "navigate the complexities." (AI-tell phrases.)
- We do not say "we're excited to announce." We just announce.
- We do not say "thought leadership."

## Voice negative space
- No emoji in long-form. (Sparing in short-form.)
- No exclamation points outside celebratory operator-personal posts.
- No em-dashes (operator preference); use semicolons or restructure.
- No begging phrases ("hope you enjoyed!", "let me know what you think!").

## Structural
- Headlines: claim-first ("Leapable closes 85% of patches Cursor leaves open"), not curiosity-bait ("You won't believe what we just found...").
- Numbers preferred over adjectives. "12.4× faster" not "much faster."
- Code preferred over English when explaining technical mechanics.

## Forbidden phrases (regex deny)
- (?i)\b(unleash|delve|tapestry|navigate the complexities|in the realm of|game-?chang(er|ing))\b
- (?i)\bsynerg(y|ies|istic)\b
- (?i)\brevolutionar(y|ize)\b

## Reference exemplars
- examples/2026-04_blog_post_evaluated_excellent.md
- examples/2026-05_thread_evaluated_excellent.md
```

### 3.2 The evaluator

The Brand Voice Scorer (`05_AGENT_FLEET_DESIGN.md §6.3`) is fine-tuned on the operator's approved historical content. It produces a 0–1 score on every draft. Below the goal-spec threshold (default 0.85) the draft is rejected and the Writer retries with the score's diagnostic feedback.

Quarterly recalibration:
- Operator reviews 50 random draft scores blind, labels them.
- Kappa against the scorer must hold above 0.6 (G-Eval calibration target for marketing copy).
- Below 0.6 → re-train the scorer on the new labels.

This is the same calibration discipline as `15_METRICS_AND_OBSERVABILITY.md §7`.

---

## §4. The Hormozi Value Equation in every artifact

Hormozi (`$100M Offers`):

> Value = (Dream Outcome × Perceived Likelihood) ÷ (Time Delay × Effort & Sacrifice)

Every content brief carries a Value Equation block. Drafts are checked against it.

```yaml
value_equation_block:
  dream_outcome: "Ask any expert anything; get the answer they actually gave with the receipts"
  perceived_likelihood: "Source-grounded with verbatim citation + page number; 163 MCP tools live"
  time_delay: "Install in 60s via signed installer; first Ask in <30s"
  effort_sacrifice: "No setup, no API keys, no learning prompts — drag files in, ask questions"
target_reader_states_post_consumption:
  - "I now believe Leapable Vaults give me an answer ChatGPT/Claude can't"
  - "I have a specific reason to install today (not someday)"
  - "I know the next step (install + try the Hormozi Vault free for 30 days)"
```

If a draft doesn't explicitly hit the dream outcome, perceived likelihood, time delay, and effort/sacrifice — the brief is rewritten. This is mechanical. The factory doesn't ship copy that fails to express the value equation any more than it ships copy that fails the brand voice score.

Hormozi's one-liner template for warm and cold outreach:

> *"I help (ideal customer) get (dream outcome) in (time period) without (effort and sacrifice) and (increase perceived likelihood of achievement)."*

The Outreach Drafter (D4) uses this exact structure. Every cold message has all four blanks filled.

---

## §5. SEO + GEO simultaneously

### 5.1 SEO (still matters)

Programmatic SEO via the Programmatic SEO Engineer (O1) generates landing pages at scale for long-tail queries the audience actually searches:

- Leapable: `"how to Ask <expert> about <topic>"` × hundreds of (expert, topic) pairs from named-Vault corpora
- Synapse: `"<task> with MCP computer use"` × ~150 tasks
- Polis: `"third place in <city>"` × top-100 cities; `"how to meet <demographic> in person <city>"`

Each template page is operator-approved (the *template*, not each page). Pages auto-generate from the data spine; the Evaluator's hallucination check ensures every claim is grounded in either operator-provided data or verifiable third-party data.

### 5.2 GEO (the bigger lever in 2026)

ChatGPT serves 800M WAU; Gemini 750M MAU; Perplexity 45M; Claude 30M. Combined dwarf Google for many queries. **Citation in their answers is the new ranking signal.**

The GEO Optimizer (O2) does:
- **Answer-first content structure** — the answer to the query appears in the first 200 words.
- **TL;DR sections** at the top of long-form (AI engines extract these preferentially).
- **Cited statistics** with primary sources (AI engines prefer content with explicit numbers + citations).
- **FAQ schema** (`schema.org/FAQPage` JSON-LD) — direct ingestion path for AI engines.
- **Comparison tables** — AI engines lift table rows verbatim when answering "X vs Y" queries.
- **Allow AI crawlers** in `robots.txt` (`GPTBot`, `ClaudeBot`, `Google-Extended`, `PerplexityBot`).

Per `2026-04 LLMrefs guide`: SEO and GEO winners share only ~20% overlap (down from ~70% in 2024). They need different optimization.

### 5.3 GEO measurement

The fleet runs a weekly Citation Audit: queries the AI engines via their APIs (where available) and via Synapse computer-use for the rest with a basket of ~100 product-relevant questions. The audit produces:

- Citation share: % of questions where the operator's content was cited
- Position: first / second / N-th cited source
- Quoted-text fidelity: does the AI engine quote the operator's words verbatim or paraphrase
- Competitor citation share: who else is being cited

GEO performance is a Tier-2 metric on the per-goal scoreboard.

---

## §6. The anchor archetypes

The factory produces seven anchor archetypes. Each has its own template, voice, and evaluator weight:

| Archetype | Purpose | Length | Cadence |
|---|---|---|---|
| **Deep dive** | Cornerstone technical content, the kind that earns HN front page | 2,500-5,000 words | 1 / week |
| **Tactical post** | Specific how-to, code-led | 800-1,500 words | 2 / week |
| **Comparison** | "X vs Y" content (high GEO yield) — e.g. "Leapable Vault vs raw RAG vs ChatGPT memory" | 1,500-2,500 words | 1 / week |
| **Build log** | Karpathy/Linear-style "this week we shipped X" | 600-1,000 words | 1 / week |
| **Postmortem** | "We tried X. It failed. Here's why." High trust signal. | 1,000-2,500 words | 1 / month |
| **Founder thread** | Operator's personal narrative; not delegated to agent | 500-2,500 chars | 2-3 / week (operator-written) |
| **Behind-the-numbers** | Charts + analysis on benchmark/usage/growth | 1,000-2,000 words | 1 / month |

The Planner picks the archetype per the goal's current need: behind on awareness → comparison / deep dive. Behind on trust → postmortem / behind-the-numbers. Behind on conversions → tactical post.

---

## §7. Hallucination defense

Three layers:

1. **Evidence pinning** — every quantitative claim in a draft must reference a numbered evidence item in the brief. The brief carries the evidence items as `{id, source, quote, url}`. The Writer is allowed to use only those items; the Evaluator deny-lints any unsourced number.

2. **Reverse citation check** — the Evaluator runs a programmatic check: for each cited quote, fetch the URL and verify the quote exists. Misquotes are blocked before publish.

3. **Operator hallucination audit** — weekly sample of 20 published artifacts is operator-reviewed for factual integrity. Any hallucination triggers a Reflector post-mortem and an update to the writer's reflection memory.

---

## §8. Persona handling

When the operator runs multiple personas (a personal account, a product account, optionally one or more pen-name personas), each anchor is written for **one** persona and only repurposed within that persona's channel set. The Persona Carrier (P5) enforces — content tagged `persona: chris` cannot be published from `persona: leapable_team`.

Each persona has:
- Own brand voice doc
- Own forbidden phrases list
- Own posting cadence per platform
- Own publication history (used for "we already covered this" dedupe)

The Wiki Gardener (M10) detects content drift across personas and flags it.

---

## §9. The repurposing rules

The Multi-Platform Publisher (D1) takes the anchor and produces derivatives by **applying transformation rules**, not by free-form rewriting. Each rule preserves the value equation and the brand voice; only format and length change.

| Source | Target | Transformation |
|---|---|---|
| 2000-word anchor | X thread | Take the H1s as tweet 1; turn each H2 into 1-3 tweets; carry over the numbers; CTA at end |
| 2000-word anchor | LinkedIn post | First 250 chars must be a complete hook (LI cuts at "see more"); use 3-paragraph structure; CTA at end |
| 2000-word anchor | TikTok / Reels script | 8-15s vertical; visual + voiceover script; hook in first 1.5s |
| 2000-word anchor | Newsletter blurb | TL;DR + 3 takeaways + "read the full piece" CTA |
| 10-minute YouTube | 3-5 vertical clips | Pull the highest-engagement 15-30s segments; auto-captions; vertical re-frame via Synapse |
| Quote line | Quote graphic | One-liner + operator headshot; brand-template auto-generated |

Rules-based transformation keeps derivatives consistent with the anchor's argument. Free-form rewriting drifts.

---

## §10. The Hormozi Rule of 100 (calibrated)

Hormozi's daily prescription for an operator getting started:

- 100 outreaches/day (warm + cold)
- 100 minutes spent making one post
- $100/day on ads
- 100 minutes improving ads

For the autonomous fleet, the calibrated version per active goal:

| Activity | Daily target | Owner | Notes |
|---|---|---|---|
| Outreaches (combined warm + cold) | 30-100 per active goal | D4 Outreach Drafter | Bounded by per-account rate caps; operator-approved per net-new |
| Substantive post production | 1 anchor + ~12 derivatives | P1 Writer + D1 Publisher | 100 min of human-equivalent compute; ~5 min of agent wall-clock |
| Paid ads spend | Within goal's `daily_velocity_cap_usd` | O4 Ad Operator | Default $100-500/day during validation; up to budget ceiling once channel proves out |
| Ad improvement iterations | 5-20 per channel | O4 Ad Operator | Per-channel A/B variant generation + test |

The Hormozi prescription was designed for a single human operator. The fleet scales the same prescription to whatever the budget supports, with the Cost Pacer keeping it honest.

---

## §11. Cross-references

- Channel cards: `08_DISTRIBUTION_CHANNELS.md`
- Brand voice evaluator: `15_METRICS_AND_OBSERVABILITY.md §7`
- GEO measurement: `15_METRICS_AND_OBSERVABILITY.md §4`
- Hallucination check: `05_AGENT_FLEET_DESIGN.md §3`
- Persona policy: `02_FIRST_PRINCIPLES.md §19`
- Programmatic SEO + GEO playbooks: `12_DEVELOPER_MARKETING.md §6`


---

<a id="08-distribution-channels"></a>
<!-- source: 08_DISTRIBUTION_CHANNELS.md -->

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
- **Product fit** (LE Leapable / SY Synapse / PO Polis)
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
channel_mix_for_goal: leapable-50k-q3-2026
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
products: [leapable, synapse]

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

### 7.1 Leapable / ME‑JEPA‑Code

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


---

<a id="09-viral-loops-and-referrals"></a>
<!-- source: 09_VIRAL_LOOPS_AND_REFERRALS.md -->

# 09 — Viral Loops and Referrals

> Viral loops are the difference between linear paid growth and exponential compound growth. Hormozi: *"Nothing scales like word of mouth."* But virality is engineered into the product, not bolted onto the marketing. This doc covers the math (K-factor, cycle time), the mechanics (loop types, incentive design), and the integration with Hormozi's Lead Getter framework. Referrals are doctrine for all three products.

---

## §1. The math, locked

Two metrics, both required:

| Metric | Formula | What it means |
|---|---|---|
| **K-factor** (viral coefficient) | `K = i × c` where `i` = avg invites per user, `c` = conversion rate of invites | Average new users generated by each existing user |
| **Cycle time** `t` | Time from a user signing up to that user generating referrals | How fast the loop turns |

| K value | Growth regime |
|---|---|
| `K < 1` | Sublinear — loop decays without external acquisition. Healthy programs land at 0.3–0.7. |
| `K = 1` | Linear — steady state, each user replaces themselves. |
| `K > 1` | Exponential — self-sustaining viral growth. Rare; usually temporary. |

**Compound growth formula:**

```
Users(n) = Initial × ((K^(n+1) - 1) / (K - 1))
```

After `n` cycles, users compound. **A K of 1.2 with 3-day cycle outperforms K of 1.5 with 30-day cycle** by a wide margin. Cycle time matters as much as the coefficient.

The fleet's goal is **K = 0.4–0.7 with t ≤ 7 days**. This is the realistic, healthy band. Anything claiming K > 1 sustained is either a temporary spike or a vanity-counted lie.

---

## §2. Loop types — which fits which product

| Loop type | Mechanic | Examples | Best for |
|---|---|---|---|
| **Incentivized referral** | Two-sided reward for referrer + referee | Dropbox storage, PayPal cash | Anything with a stickable per-user cost benefit |
| **Collaboration-driven** | The product needs others to be useful | Figma (design review), Notion (shared workspace), Slack (channel mates), Polis (event with friends) | Polis primarily |
| **Content / UGC** | Users create artifacts that are inherently shareable; the artifact carries acquisition signal | Loom (recorded video), Canva (designs), AI-generated music/video | Synapse (computer-use demos), Leapable (verification reports) |
| **Template / library** | Users publish templates that other users adopt, each requiring the product to run | Notion templates, Cursor rules, ChatGPT GPTs | Leapable (verification rule libraries), Synapse (automation playbooks) |
| **Product-led SEO** | Product creates public surface area that ranks; visitors convert | Zapier integration pages, Notion public docs, GitHub READMEs | All three |
| **Social proof** | Visible adoption by named users / orgs creates inevitability | "Built with X" badges, customer logos, GitHub star counts | All three |
| **Network effect** | The product gets better as more people join | Polis (friends), BeReal (friend graph), Bluesky (federation) | Polis exclusively |

The Referral Engineer (O3) selects the loop type per product based on product fit, not based on what's fashionable.

---

## §3. Per-product loop design

### 3.1 Leapable / ME‑JEPA‑Code

**Primary loop: Content / UGC (verification reports) + Template (rule libraries).**

Mechanic:
- When a developer runs ME‑JEPA verification on a patch, the output is a publishable verification report ("Leapable verified this Cursor patch passes the SWE-bench oracle") with an embedded `Verified by Leapable` badge.
- The badge links back to Leapable with the developer's referral ID.
- High-trust developers (paid tier, OSS contributors, verified accounts) get extra-prominent badge placement that they share voluntarily on PRs and X.
- Template library: rule libraries (custom verification rubrics for specific stacks — Django, Rails, Next.js) are published by the community, each driving install demand.

Two-sided incentive (incentivized referral overlay):
- Referrer (existing CG user): 1 additional verification credit per converted referral, up to 20/month.
- Referee (new signup via referral link): 5 bonus verification credits on top of free tier.
- Mechanic published in-product; no out-of-band gimmicks.

K-target: 0.4 within 90 days of program launch.

Cycle time target: 5 days (run verification → share badge → referee installs → referee runs first verification).

### 3.2 Synapse

**Primary loop: Content / UGC (demo videos) + Template (automation playbooks).**

Mechanic:
- Every successful long-running Synapse session can export a sharable demo recording.
- The export embeds "Built with Synapse" plus the operator's referral ID.
- Automation playbooks (operator-built Synapse scripts) can be published to a community library; each install requires Synapse.
- Hackathon prize loop: Synapse-themed hackathons (online + IRL) where winners' projects are amplified by the operator's accounts.

Two-sided incentive:
- Referrer: priority queue for OS-feature requests + month of pro tier per converted referral.
- Referee: 30-day pro trial automatically applied via referral link.

K-target: 0.35 within 90 days.

Cycle time target: 7 days (run automation → share demo → referee installs → referee runs first automation).

### 3.3 Polis / Serendipity Engine

**Primary loop: Network effect (friend density) + Collaboration-driven (events require friends).**

Mechanic:
- Polis events get richer when more friends attend. Inviting friends is the core product action, not a marketing afterthought.
- Each event invite to a non-Polis-user carries the inviter's name front-and-center ("Sarah invited you to a salon").
- Campus ambassador program (BeReal pattern): students recruit dense friend clusters; ambassadors paid $30/successful referral + $50/install+attended-event (calibrated to the BeReal benchmark).
- Two-prize giveaway pattern (Hormozi's Skool.com playbook): every event has a host-prize draw + a referrer-prize draw. If your referee wins, you win too.

Two-sided incentive:
- Referrer: invitation to a higher-density event (e.g., a curated dinner) after 3 successful invites.
- Referee: priority access to events otherwise waitlisted.

K-target: 0.6 within 60 days (consumer products tolerate / require higher K).

Cycle time target: 3 days (event happens → attendees share with friends → friends attend their first event).

---

## §4. Hormozi's exponential vs linear

Hormozi (`$100M Leads`, "How Referrals Grow Your Business"):

> *"The number of engaged leads you get from the core four depends on how much you do them. The inputs to outputs have pretty linear relationships. If you do 100 reach outs you get engaged leads. If you double it, your leads roughly double. So no matter how well you advertise, how much you get depends on how much you do. And that's great. But with word of mouth, we can do even better. With word of mouth, one customer brings two. Two bring four. Four bring eight. And so forth. It's not linear, it's exponential."*

> *"Want to know why so few people scale by word of mouth? They lose customers faster than they get them."*

The Cohort Health Auditor (O7) is the enforcement: if 28-day cohort retention drops below the per-product floor, the Referral Engineer's program is paused. **No referral effort on a leaky bucket.** This is doctrine.

Retention floors per product:
- Leapable: 70% 28-day retention on paid users; 50% on free.
- Synapse: 65% 28-day retention.
- Polis: 75% within-cluster retention (active users in the same friend cluster).

Below the floor → fix retention before scaling referrals.

---

## §5. Two-sided incentive design

Per the 2026 LaunchList research:

> *"Two-sided incentives (inviter and invitee both get something) consistently outperform one-sided incentives."*

The 5-tier reward stack works:

| Tier | Referral count | Reward |
|---|---|---|
| 1 | 1 referral | queue jump / small in-product credit |
| 2 | 3 referrals | premium feature trial / Discord access |
| 3 | 5 referrals | branded swag / extended trial / community recognition |
| 4 | 10 referrals | exclusive content / lifetime tier discount |
| 5 | 25-50 referrals | high-value physical reward / event invite (where margins permit) |

Tier 1-3 are free to deliver (incremental software cost). Tiers 4-5 are physical or scarce. Hormozi recommends starting with queue-jump (free) and only layering higher tiers as the program scales.

### 5.1 The Dropbox-style pre-launch waitlist

If a product launches with a waitlist, the referral mechanic is **the** acquisition channel during pre-launch. Per LaunchList:

> *"Adding a referral program later means your first 200 subscribers never had the chance to share. Set it up before driving any traffic."*

Polis specifically: a Dropbox-pattern waitlist with queue-jump per referral is the canonical pre-launch loop. Leapable and Synapse can use this for major product unlocks (e.g., a new feature in private beta).

### 5.2 The Hormozi "Skool.com" giveaway

Hormozi's two-prize giveaway pattern:

> *"Give Away Two Prizes For Twice The Leads. If you give away one prize, that's fine. But if you give away two grand prizes, you can get twice the leads (or more). Just tell everyone that if someone they refer wins the grand prize, they win one too."*

Polis events have natural prize mechanics (high-density curated dinners, intellectual-creator hosted salons). Each event marketing carries the two-prize structure. Leapable + Synapse can run periodic "you and your referee win" launches around product milestones.

Hormozi's three giveaway rules:
- **Scarcity** — bounded by entries or time
- **Urgency** — short windows, clearly announced
- **Make the prize actually grand** — "If your giveaway doesn't work, the prize wasn't grand enough."

### 5.3 The Hormozi guarantee structure for referral acceptance

Hormozi: *"If you don't X, I will Y. Most people forget the second part. You have to guarantee the or else part or your guarantee has no teeth."*

For referrals, the acceptance message structure is:

> *"Sarah recommended you try Leapable. If you don't successfully verify your first patch within 7 days, we'll give you 50 verification credits ($25 value) and a 30-minute walkthrough with our engineer."*

The "or else" makes the offer credible. The Outreach Drafter (D4) and the referral landing-page generator both enforce this structure.

---

## §6. The 30-day client-financed acquisition rule

Hormozi (`$100M Leads`, "Client Financed Acquisition"):

> *"If your customer spends more than it costs you to get and fulfill them — in the first 30 days — then you have the funds to scale now and forever. I call this client financed acquisition."*

> *"Any business can get interest free money for thirty days in the form of a credit card. And if we make more than the cost to get and fulfill the customer in the first thirty days, we square our balance."*

This is **the** unlock for paid acquisition. The Cost Pacer + Cohort Health Auditor enforce:

```
30d_realized_gross_profit_per_acquired_user >= CAC + 30d_fulfillment_cost
```

If `TRUE` → paid acquisition can scale without cash-flow risk.
If `FALSE` → paid acquisition is capped at the available cash float.

Per-product first-30-days targets:
- Leapable: $90 of gross profit per acquired developer in 30d (the Pro plan at $99/mo - fulfillment cost)
- Synapse: $50 (lower plan price; offset by lower CAC)
- Polis: $25 (consumer; offset by very low CAC)

If `30d_gross_profit < CAC`, the Referral Engineer's incentive cost is the first thing reviewed before raising paid spend. Hormozi's order of operations.

---

## §7. Product loops vs marketing loops

Loops fall into two categories that the fleet treats differently:

| Loop category | Lives in | Operator approval | Owner agent |
|---|---|---|---|
| **Product loop** | The product itself (a feature, a flow, a UI element) | Always `REQUIRE_APPROVAL` (product change) | Referral Engineer O3 proposes; operator + product team builds |
| **Marketing loop** | Outside the product (landing pages, ad creatives, content hooks) | `LOG` or `REQUIRE_APPROVAL` per autonomy slider | O3, O1, P1, D1 |

Marketing loops the fleet can ship directly: referral landing pages, two-sided incentive copy, prize-draw email sequences, A/B tests on referral-link click-through.

Product loops the fleet only **proposes**: in-app referral UI, post-event share prompt for Polis, verification-report badge for Leapable. The operator + product team builds these.

---

## §8. The viral cycle time levers

The fleet can compress cycle time by:

1. **Onboarding speed** — the faster a referee activates, the faster they can refer. PLG 2.0: first value in <60 seconds. The Conversion Optimizer (O5) owns this.
2. **First-action prompt** — once activated, the referee is prompted to invite their first friend within the activation flow itself (not later).
3. **Deferred deep linking** — referral links deliver new users into the relevant context (the specific event, the specific verification, the specific automation) so the first action is one tap, not a search.
4. **Reminder cadence** — referee dropoffs at day 1, 3, 7 trigger Reflexion-tuned re-engagement emails (not spam; max 3 per dropoff).

A 5-day cycle moving to 3-day cycle approximately doubles compound growth over 90 days at the same K. This is one of the highest-leverage optimizations.

---

## §9. The viral cycle measurement

The Evaluator (M7) and the Cohort Health Auditor (O7) jointly measure:

| Metric | Calculation | Target |
|---|---|---|
| Share rate | % of acquired users who click their share link in 24h | ≥ 25% |
| Invites per sharer | avg unique-invite-link clicks per sharer | ≥ 5 |
| Invitee conversion rate | % of invite-link clicks that activate within 7d | ≥ 15% |
| K-factor | i × c | per-product target |
| Cycle time | days from referee-activation to referee-share | per-product target |
| Lossy attribution | % of referrals where the referrer couldn't be identified | ≤ 5% |

The fleet has dashboards for these. Predicate-relevant: K-factor and cycle time. Diagnostic: the rest.

---

## §10. Anti-patterns

What the Referral Engineer does **not** do:

- **Buying referrals from incentivized-traffic networks.** Looks like K-factor; isn't.
- **Cash-only referrals at any scale.** Attracts the wrong cohort; spike then crash.
- **Public referral leaderboards in B2B.** Triggers gaming and competitor reconnaissance.
- **Hiding the referral mechanic until day 14.** Activation-time prompt is the highest-conversion moment.
- **Mass-DM referral asks via computer-use.** Always feels like spam, always burns brand.
- **One-sided incentives ("refer a friend, you get $10, they get nothing")** — consistently underperforms two-sided.
- **Stealth referrer-only tracking** — referee unaware they came via referral. Loses the "Sarah invited you" landing-page boost.

---

## §11. Cross-references

- Lead Getter taxonomy: `08_DISTRIBUTION_CHANNELS.md §2`
- Cohort retention floors: `15_METRICS_AND_OBSERVABILITY.md §3`
- Referral Engineer agent spec: `05_AGENT_FLEET_DESIGN.md §1.5`
- Client-financed acquisition: `15_METRICS_AND_OBSERVABILITY.md §5`
- Polis-specific viral mechanics: `13_CONSUMER_SOCIAL_LAUNCH.md §4`
- Leapable/Synapse-specific loops: `11_PRODUCT_PLAYBOOKS.md`
- Pre-launch waitlist 90-day playbook: `17_LAUNCH_PLAYBOOK_90DAY.md`


---

<a id="10-influence-network-mapping"></a>
<!-- source: 10_INFLUENCE_NETWORK_MAPPING.md -->

# 10 — Influence Network Mapping (the T3 Elite Tier)

> Steve Hutto's framing: *"Every ultra-rich has their own book. Every media bubble is very closed-door. So you have to figure that out. Who, what is my brother or my brother-in-law or sister-in-law, or my partner or this or that, who influences that person?"* This doc operationalizes elite-tier reach as a graph problem: map the network, find the shortest warm-intro path, and execute through operator-mediated handoffs — never agent-direct outreach.

---

## §1. Why Tier 3 is structurally different

T1 channels are open broadcast. T2 channels are open community. T3 channels are **closed doors**:

- Private dinners, off-the-record retreats, salon-style events
- Group chats (Signal, WhatsApp, iMessage threads, private Slacks)
- Curated invite-only conferences (TED, All-In summit, intellectual retreats, founder-only events)
- Investor portfolio mailing lists
- Board-level decision conversations

No amount of agent automation reaches behind these doors. The doors open only via **trusted human introduction**. The fleet's job is not to walk through the doors. It is to **make sure the operator never misses a viable bridge** to a door worth walking through.

---

## §2. The graph

A directed graph where:

- **Nodes** = people (and organizations as a coarse aggregate type)
- **Edges** = documented relationships, typed by relationship class

Relationship classes, ordered by edge strength:

| Class | Edge weight | Example |
|---|---|---|
| Co-founded | 1.00 | "X and Y co-founded Acme 2019-2022" |
| Direct manager–report | 0.95 | "Y reported to X at Anthropic" |
| Investor in your company | 0.90 | "X is a Series-A investor in operator's prior co" |
| Multi-year close colleague | 0.85 | "X and Y worked closely at OpenAI 2020-2024" |
| Board member of operator's company | 0.85 | |
| Board overlap (sit on a shared board) | 0.75 | |
| Repeated public co-appearance | 0.65 | "X and Y did 3 podcasts together in past 12 months" |
| Mutual close LinkedIn connection density >5 | 0.55 | |
| Shared significant past employer | 0.45 | |
| Shared alma mater, same cohort year | 0.45 | |
| Same advisor / mentor | 0.40 | |
| Shared portfolio-company member of investor | 0.35 | |
| Shared major industry conference attendance | 0.25 | |
| 2nd-degree LinkedIn connection | 0.15 | |
| Co-attended same generic event | 0.10 | |

A "warm intro path" is a chain of edges from operator → target. The strength of the path is the **minimum** edge weight on the chain (the chain is only as strong as its weakest link). The fleet prefers paths of length ≤ 3 with weakest edge ≥ 0.45.

---

## §3. The Influence Network Mapper (S3)

The agent class that owns the graph. From `05_AGENT_FLEET_DESIGN.md §6.1`.

### 3.1 Inputs

- **Operator's network** — LinkedIn export, Gmail contacts, Cal events, prior podcast guest list, prior investor list, board affiliations.
- **Target list** — Tier-3 contacts the operator wants to reach (operator-curated; the fleet may suggest based on signals but operator confirms).
- **Public graph signal** — Crunchbase, GitHub orgs, board records (where public), conference attendance lists (public), shared-publication co-authorships.

The Mapper **does not scrape private data**. It uses what is publicly indexed plus what the operator has shared from their own export. It never infers protected attributes (orientation, political affiliation, health, religious).

### 3.2 Process

1. Ingest operator-side network. Tag every node with attributes the operator has explicitly added (relationship class, last contact, last meaningful exchange, notes).
2. For each Tier-3 target, run BFS to find paths from operator-side nodes.
3. Score each path by `min(edge_weights)`.
4. Pick top 3 viable paths (path length ≤ 3, min-edge ≥ 0.45).
5. For each viable bridge, produce a one-paragraph "why this person could intro" — drawn from documented relationship history, recent public signals (shared interests, projects, conversations).
6. Score the bridge's **availability**: when did the operator last meaningfully interact with them? A 2-year-cold bridge is a different ask than a last-quarter bridge.
7. Output: per-target, ranked viable paths, each with bridge + intro reasoning + last-touch staleness.

### 3.3 Outputs

A markdown ranking, queued for operator review:

```markdown
## Target: Alice Kowalski (CEO, frontier AI lab)
**Why this target matters:** Decision-maker on enterprise AI tooling spend; her endorsement
in the operator's stack would unlock a procurement conversation at her company and signal-boost
to peers.

### Path 1 (strength 0.85, length 2)
Operator → Bob Lee (co-investor in operator's prior co, last contact: 6w ago) → Alice
**Bridge:** Bob Lee.
**Why Bob → Alice:** Bob led the Series B of Alice's previous startup (2018-2020); they
remain close per joint podcast appearance Q4 2025.
**Suggested intro reason:** Alice was quoted in NeurIPS 2025 panel as concerned about
"verifying AI-generated code in regulated industries" — the operator's product is precisely
this problem domain.
**Operator action:** Draft request to Bob (operator-approved, then sent).

### Path 2 (strength 0.55, length 3) [backup]
Operator → Carol Mendes (alumni) → Dave Singh (Carol's ex-coworker, current AT Anthropic)
→ Alice
**Bridge sequence:** Carol → Dave → Alice.
**Why this path:** Three-hop, weakest edge 0.55. Use only if Path 1 fails or as parallel
non-overlapping ask.

### Path 3 (strength 0.45, length 3) [low priority]
[...]
```

---

## §4. The Warm-Intro Drafter (D5)

Once a path is approved, the Warm-Intro Drafter produces the **forwardable paragraph** — a paragraph in the bridge's voice that the bridge can copy-paste-with-light-edit and forward.

### 4.1 The structure

```
[Bridge's name] forwards the operator's request to [Target]:

Hi [Target's first name],

[One sentence connecting on a recent public signal — what the target has been
talking about / building / thinking about. Specific, not generic.]

I want to introduce you to [Operator's name]. [One sentence on operator's
relevance to the target's recent signal — what they've built, the proof point,
the named users.]

If it's worth 20 minutes, [operator's name] would love to share what they've
found. If not, no worries — wanted to make sure the introduction was on your
radar.

[Bridge's existing sign-off pattern.]
```

The Drafter pulls the bridge's prior writing samples (publicly available emails, intros they've made before, podcast intros) to match voice. The operator reviews and edits before sending.

### 4.2 Hard rules

- **The Drafter never sends.** It produces text the bridge sends.
- **The bridge must voluntarily forward.** If the bridge declines, the path is dead — do not retry through the same bridge.
- **No mass templates.** Every intro request is custom to the target and bridge.
- **No fabricated relationship history.** If the operator and bridge haven't talked in 2 years, the request acknowledges that.
- **EU AI Act disclosure not required for warm intros** drafted by AI when the bridge edits before sending (the human is the author of the final text). However, the operator should disclose to the bridge that the draft was AI-assisted on first use, as a professional courtesy.

---

## §5. The Hormozi Lead Getter framing

Hormozi: *"You do not 'do' affiliates or 'do' customer referrals... they come from warm outreach, cold reach outs, posting content, and running paid ads. And once you get them, they do it for you."*

The Influence Network Mapper is the same idea applied to the Elite tier:

- The fleet does not **do** elite-tier endorsements.
- It identifies the **bridges** who, once cultivated, will produce elite-tier reach.
- The bridges become T3 lead-getters: the people who, once warm to the operator, refer the operator into more T3 contexts.

This is the Hormozi-style compounding move applied to Tier 3. Treat bridges like high-value customers: deliver value to them, stay top-of-mind without spam, surface relevant signals.

---

## §6. The Pre-Call Briefer

Before any operator–target conversation, the Pre-Call Briefer (a sub-agent of the Mapper) produces a one-page brief:

```markdown
## Pre-Call Brief: Alice Kowalski

### Conversation context
- Bridge: Bob Lee
- Bridge's framing of the intro: "[verbatim from Bob's forwarded note]"
- Scheduled: 2026-06-10 14:00 PT, Zoom
- Operator's stated objective: explore enterprise pilot for Leapable

### Last 90 days of public signal on Alice
- Mentioned "AI verification" 4 times across podcast / X / NeurIPS
- Posted X thread on "AI code review is broken" on 2026-04-12 (1.2k likes)
- Spoke at AI Infra Summit 2026, mentioned current frustration with hallucinated patches in CI
- Recent reading list (book club tweet): "Designing Machine Learning Systems"

### What she cares about right now (synthesized)
1. AI verification under regulatory pressure
2. Composability of AI agent tooling
3. Operator burnout on engineering teams

### Operator's strongest talking points (mapped to her interests)
- Leapable's deterministic verification = her #1 (regulatory)
- MCP-native interface = her #2 (composability)
- Reduces senior engineer code-review hours = her #3 (burnout)

### What NOT to bring up
- Pricing on first call (Bob's note: "she'll find that off-putting")
- Competitor comparisons until she asks
- Sales-oriented language; she's a builder
```

The brief is operator-only; never goes to the target. It's the operator's prep, not a script.

---

## §7. Follow-Up Tracker

A passive monitor. For every Tier-3 contact who's been in conversation with the operator:

| Trigger | Suggestion to operator |
|---|---|
| 90 days since last meaningful exchange | "Re-engage on a relevant signal" + the most recent relevant signal from the target's public surface |
| Target ships a launch / event / paper | "Send congratulations + one line of specific value-add" |
| Target moves company / role | "Acknowledge + recalibrate ask" |
| Target appears at an upcoming event the operator could attend | "Consider attending; expected attendees include X, Y, Z" |

The Tracker **never sends**. Operator-mediated.

---

## §8. Bridge cultivation

The single highest-ROI move on Tier 3 is **investing in bridges** — the people in the operator's network who could become repeat-introducers. Not transactional ask-once-and-burn-them; long-term-value-add.

Per-bridge investment cadence:

| Bridge tier | Touch frequency | Value-add per touch |
|---|---|---|
| Active high-leverage bridge (>3 successful intros in 12mo) | every ~30 days | personalized signal: "saw your portfolio company X is hiring for Y; I have someone good"; or a relevant introduction back to them |
| Active bridge (1-2 successful intros) | every ~60 days | thoughtful comment on their public work; relevant signal forward |
| Latent bridge (no asks made yet) | every ~90 days | low-touch — a "this made me think of you" share |
| Cold bridge (>12 months silent) | re-engage with one specific signal | one well-thought-out ask is worth more than five generic check-ins |

The Follow-Up Tracker manages the cadence; the operator does the touches; the Drafter helps with copy when requested.

---

## §9. Permission classes and hard rules

Every action in this doc is `REQUIRE_APPROVAL` by default with **no operator-configurable bypass**:

- The Mapper proposes; the operator confirms target list and approves bridge paths.
- The Drafter writes; the operator reviews and either sends as-is or edits.
- The Tracker monitors; the operator decides each suggested touch.
- The Briefer prepares; the brief is operator-eyes-only.

Hard rules (`02_FIRST_PRINCIPLES.md §19`):
- **Never** send automated outbound to a Tier-3 contact.
- **Never** scrape personal data beyond what's publicly indexed.
- **Never** infer protected attributes.
- **Never** store dossiers on health, family, politics, religion.
- **Never** assume a relationship the operator hasn't confirmed.
- **Never** use Tier-3 names in T1/T2 marketing copy without explicit, written permission.
- **Never** drop a Tier-3 name in a sales DM or fundraise to imply endorsement that isn't there.

A single violation of this section is a **brand-level kill switch event**. The Risk doc treats it as Tier 1 severity.

---

## §10. The endorsement asymmetry

Hormozi: *"The last skill you ever need to learn is how to get other people to do everything you need for you."*

Tier-3 endorsement is the highest-leverage form of this. A single named adoption by a recognized expert ("I use Leapable", "I trust Synapse for X", "I attended a Polis salon and here's why it works") moves a market segment.

**But**: a Tier-3 endorsement that's clearly transactional ("paid promotion") is **negative leverage**. The endorser loses credibility; the operator's brand inherits the suspicion.

Doctrine:
- **Never pay for a Tier-3 endorsement.** Hire them as an advisor (with real equity, real role) or get an unpaid genuine endorsement, but not influencer-style sponsored post.
- **Never embellish what the Tier-3 person said.** "Used it twice" ≠ "endorses it."
- **Always honor a request to be removed.** Tier-3 contacts who later reverse their endorsement get silent removal, no public response.

---

## §11. Cross-references

- Tier model: `03_TIER_MODEL_AUDIENCE.md`
- Mapper agent spec: `05_AGENT_FLEET_DESIGN.md §6.1`
- Drafter agent spec: `05_AGENT_FLEET_DESIGN.md §1.4` (D5)
- Tier-3 doctrine: `02_FIRST_PRINCIPLES.md §19`
- Per-product elite-tier playbooks: `11_PRODUCT_PLAYBOOKS.md`
- Closed-bubble channels for elite content consumption: `08_DISTRIBUTION_CHANNELS.md §3.6`


---

<a id="11-product-playbooks"></a>
<!-- source: 11_PRODUCT_PLAYBOOKS.md -->

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


---

<a id="12-developer-marketing"></a>
<!-- source: 12_DEVELOPER_MARKETING.md -->

# 12 — Developer Marketing (DevRel Flywheel)

> For Leapable and Synapse, developer marketing is the engine. Borrowed from FORKOFF's AI DevRel Flywheel + the Cursor / Linear / Vercel / Anthropic playbooks. Five surfaces — notebooks, open-source refs, hackathons, content DevRel, integrations guild — that compound into each other. Joe Karlsson's depth-over-breadth law applies. Hormozi's volume applies to artifact production; depth applies to engagement.

---

## §1. The AI DevRel Flywheel (5 surfaces)

```
   ┌─────────────────┐
   │   Notebooks     │  ← primary trust-transfer surface
   │   (Jupyter,     │
   │    Colab)       │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Open-Source     │  ← reference implementations
   │ Refs (GitHub    │
   │ repos)          │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Hackathons      │  ← compress trust into shipped artifacts
   │                 │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Content DevRel  │  ← long-form that earns citation
   │                 │
   └────────┬────────┘
            │
            ▼
   ┌─────────────────┐
   │ Integrations    │  ← official partnerships
   │ Guild           │
   └────────┬────────┘
            │
            └────► (back to notebooks for new integrations)
```

Each surface compounds the next. A great notebook earns GitHub stars; the starred repo gets found in hackathon submissions; hackathon projects become content DevRel case studies; case studies earn integration partner conversations; new integrations seed new notebooks.

This is **not 5 channels to run in parallel from day 1.** It's a sequence. Hormozi's "pick one, max it" applies: notebooks first, until a baseline is in place, then OSS refs, then hackathons.

---

## §2. Surface 1 — Notebooks

The primary trust-transfer surface for AI dev tools in 2026. A 3-minute time-to-first-output Jupyter or Colab notebook lets a skeptical developer copy-paste and *believe*.

### 2.1 What good looks like

- **Time to first output ≤ 3 minutes.** From open Colab → result shown.
- **No setup gymnastics.** No API key required (use free tier); no Docker assumed; no GPU assumed.
- **Side-by-side outputs.** Show the tool's output next to the alternative (without-tool, or competitor) for direct comparison.
- **One canonical artifact per use case.** Leapable: "Verify a Claude Code patch." Synapse: "Drive Notepad with an AI agent."
- **Weekly maintenance.** Stale notebooks (broken APIs, deprecated dependencies) are net-negative trust. Updated every Friday.

### 2.2 Initial notebook set per product

Leapable (target: 12 notebooks in first 90 days):
1. "Verify your first Claude Code patch in under 60 seconds"
2. "Why Leapable disagrees with the dashboard — three real cases"
3. "Building a custom rule library for Django projects"
4. "Leapable + Cursor: the full setup walkthrough"
5. "Comparing Leapable verdicts to oracle outcomes on 100 SWE-bench tasks"
6. "Catching subtle-flip mutations the human reviewer missed"
7. "Integrating Leapable into a CI pipeline"
8. "How the conformal interval prevents false confidence"
9. "Reading the named-failure-mode hierarchy"
10. "The TCT Gτ guard explained with worked examples"
11. "Building a community rule library: contributor walkthrough"
12. "Leapable on a non-Python codebase: a research preview"

Synapse (target: 12 notebooks in first 90 days):
1. "Drive Notepad with an AI agent in 60 seconds"
2. "Your first MCP computer-use session with Claude"
3. "Why Synapse vs. Browser-Use vs. Playwright — when to use which"
4. "Building a custom Synapse automation playbook"
5. "Game inputs: Synapse + ViGEm for controller emulation"
6. "Accessibility: Synapse for hands-free desktop operation"
7. "Multi-machine orchestration with Synapse"
8. "Capturing OCR + audio for full perception coverage"
9. "Reflex controllers for sub-frame latency in game scenarios"
10. "The operator panic hotkey and safety guarantees"
11. "Audit logging: tracing every Synapse action"
12. "Synapse + Leapable: verifying that the agent did what it claimed"

### 2.3 Metrics

Per FORKOFF benchmarks:
- GitHub stars / week target: **≥ 50** after notebook 3
- Week-1 reproduction rate target: **≥ 75%** (operators-or-readers actually run it successfully)
- Maintenance freshness target: **median age of notebook last-touch ≤ 30 days**

---

## §3. Surface 2 — Open-Source Refs

Reference implementations. Distinct from notebooks: notebooks are step-by-step pedagogy; refs are full working examples that show how to integrate at production scale.

### 3.1 What good looks like

- **One repo per integration target.** `leapable-claude-code-example`, `leapable-codex-example`, `leapable-cursor-example`, `leapable-continue-example`.
- **README that walks talk.** Hormozi's principle: prove the concept by being the concept. The README itself is the strongest marketing asset.
- **CI on the repo runs the example.** Demonstrates the example actually works.
- **README quickstart in 5 commands max.** Five lines, one paste, success.

### 3.2 GitHub-as-marketing channel

Per the Joe Karlsson 2026 DevRel observations:

> *"Your README is your primary marketing surface for IDE-native AI. When a developer asks Claude Code how to implement something, the model reaches into training data that includes your GitHub repo."*

The READMEs need to be **structured for AI engine extraction** as much as for human reading:
- First 200 words answer "what does this do and why?"
- Clean code blocks (no smart-quotes; no markdown breakage)
- FAQ schema in repo-level docs (the docs site)
- Tables comparing the tool to alternatives

### 3.3 SLAs as product marketing

The 5W developer-led-growth playbook:

> *"Publish SLAs: 48-hour issue response, one-week PR review, quarterly doc audit. Every open-source repo is product marketing for every future enterprise evaluator. A stale repo costs deals."*

Operator's SLA commitments per repo:
- Issue acknowledgment: 24h on weekdays
- Issue triage decision: 72h
- PR review: 1 week
- Doc audit: quarterly
- Release cadence: at least monthly

These are mechanical commitments. The GitHub Maintainer agent (D3) enforces them.

---

## §4. Surface 3 — Hackathons

Compress months of marketing into 48 hours of shipped artifacts. Sponsored, attended, or operator-hosted.

### 4.1 Three modes

| Mode | When | Cost (typical) | Outcome |
|---|---|---|---|
| **Sponsor a hackathon** | Operator wants reach to a specific community | $2-10k per event | Logo + booth + prize sponsorship + workshop |
| **Attend with team** | Operator wants direct face-time with target builders | Travel + $500-2k | Karpathy "find the right people" mode |
| **Host operator-run hackathon** | Operator wants exclusive community + branded artifact set | $5-25k | All submitted projects use the operator's tool; case studies for content DevRel |

For Leapable and Synapse in the first 90 days, **sponsor 3 hackathons** targeting AI agent / AI engineering audiences:
- AGI House hackathons
- AI Engineer Summit hackathons
- MCP-specific (Anthropic-organized) hackathons
- Cursor + Anthropic joint hackathons
- Replit AI hackathons
- Latent Space community events

### 4.2 The Hackathon Strategist agent

D2-class agent that:
- Maintains the live calendar of relevant hackathons (Devpost, Major League Hacking, Lu.ma, X mentions)
- Scores each by relevance × attendee profile × operator capacity
- Drafts sponsorship pitches when the operator approves
- Produces post-event case studies from notable winning projects

### 4.3 The hackathon-to-content flywheel

Per FORKOFF: "Hackathon projects become content DevRel case studies."

Process:
1. Hackathon ends. Identify top-5 submissions that used the operator's tool.
2. Reach out to builders for permission + interview.
3. Write 5 case studies (P1 Technical Writer drafts; operator reviews).
4. Publish on operator's blog + Dev.to + X thread + LinkedIn.
5. AI engines index the case studies; future "how do I use X" queries cite them.

---

## §5. Surface 4 — Content DevRel

The long-form blog + video that explains architecture decisions, not marketing.

### 5.1 The goal: citations, not traffic

> *"The goal is citations, both in other developers' blog posts (which compound domain authority and backlinks) and in AI answer engines (ChatGPT, Claude, Perplexity, Gemini) that train on and retrieve technical content."*

The Citation Audit (`07_CONTENT_FACTORY.md §5.3`) tracks how often the operator's content is being quoted back. That's the metric. Pageviews and SEO impressions are diagnostic.

### 5.2 Anchor archetypes for DevRel

Per `07_CONTENT_FACTORY.md §6`, the developer-relevant subset:

| Archetype | Frequency for CG/SY |
|---|---|
| Deep dive (cornerstone technical content) | 1/week |
| Tactical post (specific code-led how-to) | 2/week |
| Comparison ("Leapable vs Goodfire vs human review") | 1/week |
| Build log (Linear-style "this week we shipped X") | 1/week |
| Postmortem (high trust signal) | 1/month |
| Behind-the-numbers (charts + benchmarks) | 1/month |

### 5.3 Build-in-public discipline

Linear's playbook: their X/Twitter functioned as a live, public changelog. Cursor's playbook: founder tweets about specific quality moments developers screenshot.

Operator-driven (not agent-delegated):
- 3-5 substantive posts per week on operator's X
- Show, don't tell: every claim has a screenshot, a graph, or a code snippet
- Amplify customers when they post (Quote Tweet + brief reply)

Agent-supported:
- Signal Listener (S2) flags mentions worth amplifying
- Content Writer (P1) drafts the agent-supportable content (anchors, derivatives)
- Founder Thread archetype is operator-written, agent-formatted (per `07_CONTENT_FACTORY.md §6`)

### 5.4 What not to do

Per the AI DevRel literature:
- Don't delegate the founder voice to the marketing voice — AI community detects delegated voice within a post
- Don't orchestrate upvotes — backlash costs more than the votes
- Don't post on launch day to communicate with the team; do it 48h+ in advance and stagger
- Don't run "thought leadership" pieces — developers smell them

---

## §6. Surface 5 — Integrations Guild

The most lagging but highest-leverage surface. Once the prior surfaces have proven the product, formal partnerships with adjacent tools become possible.

### 6.1 What

For Leapable: official partnerships with Cursor, Continue.dev, Aider, Anthropic Claude Code, OpenAI Codex, Replit Agent, Devin (Cognition).

For Synapse: official partnerships as the default computer-use backend for popular AI agent frameworks — LangChain, CrewAI, AutoGen, Anthropic Agent Framework, Browser-Use (interop).

### 6.2 The Vercel / Stripe distribution playbook

Vercel's deal with WPP (every WPP client uses Next.js + Vercel) is the modern bar for distribution partnerships:

> *"In the AI era, the builder is increasingly non-technical and the AI tool is choosing the infrastructure for them, so the platform's defaults make an impact."*

The fleet's bet: become **the default option** suggested by AI agent frameworks for relevant use cases.

### 6.3 Tactics

- Build a working integration before pitching the partnership.
- Open-source the integration (lowers the partner's risk of taking the meeting).
- Time the pitch to a moment in the partner's roadmap (around a release, ahead of a conference).
- Provide co-marketing value: case studies, joint blog posts, joint hackathons.
- Use Tier-3 (Influence Network Mapper paths) to get warm intros to the partner's BD/DevRel lead.

### 6.4 Order of operations

1. Build the integration (week 0-4).
2. Open-source + announce on operator's channels (week 4-5).
3. Warm-intro to partner BD lead via Mapper-identified bridges (week 5-8).
4. Co-marketing meeting + roadmap discussion (week 8-12).
5. Formal partnership announcement (week 12-16).

This is 3-4 months from start to formal partnership. The fleet runs 3-5 of these concurrently.

---

## §7. The Common Room moment

> *"Common Room is a platform built specifically for this problem: it aggregates engagement signals from across developer touchpoints (GitHub stars, Discord activity, Slack, community forums, and more) and uses machine learning to identify developer personas, surface trending topics, and track how developers are progressing through the discovery and adoption journey."*

The fleet builds (or buys) the equivalent:
- Common Room (or self-built equivalent via the existing channel cards + Signal Listener)
- Cross-channel attribution: which Discord-user became a GitHub-starrer became a paid subscriber
- Per-developer trajectory: each developer is a journey, not a touchpoint

The Audience Sorter (S1) writes the trajectory; the Cohort Health Auditor (O7) measures the funnel.

---

## §8. The Cursor / Linear / Vercel patterns to copy

### 8.1 Cursor — zero marketing spend, $0 → $100M ARR in 12 months

Pattern: founder tweets about model capabilities → individual developer adoption → screenshots → memes → enterprise notices → enterprise pays. **No PR firm. No marketing agency. No content team for the first year.**

The fleet's translation:
- Founder voice (operator's X account) is the primary acquisition signal
- Agent fleet **supports** the founder, never replaces them
- Quality > volume in the founder voice (4-5 posts/week, not 50)
- Amplify user wins; never claim them

### 8.2 Linear — build-in-public + Twitter amplification

Pattern: live public changelog → velocity drumbeat → Quote Tweet engine that amplifies customer praise.

The fleet's translation:
- D1 Multi-Platform Publisher posts the changelog from a structured source
- D6 Comment Triage drafts Quote Tweet replies for operator approval
- Linear's "always amplifying user love" rhythm becomes a per-day operator habit

### 8.3 Vercel — distribution through Next.js

Pattern: massive open-source download number → become default → enterprise upsell.

The fleet's translation:
- Leapable: open-source the rule libraries; become the default verification stack in AI agent frameworks
- Synapse: open-source the playbooks; become the default computer-use backend
- Polis: open-source the gathering-hosting tools; become the default substrate for IRL community events

---

## §9. Joe Karlsson's hard rules (still binding)

> *"Pick one or two community channels and go deep. Not five."*
> *"A Discord with one person who answers questions within an hour beats five channels where nothing happens."*

The fleet's Discord engagement (D2 Community Operator) operates on at most **5 named Discords per product**, with answer-time SLA of 2 hours during operator's local active window.

> *"Community signals are also pipeline intelligence."*

S2 Signal Listener flags Discord/Slack messages that imply enterprise-scale ("how does this handle 50 billion events per day"). Those become operator-routed conversations, not agent-replied threads.

> *"Last thing: structured FAQs and a clean one-sentence product definition get extracted as citable answers."*

Per `07_CONTENT_FACTORY.md §5.2`, FAQ schema + one-sentence product definitions are mandatory on every product page.

---

## §10. The Hormozi 100k content per platform formulation

> Hormozi: *"We put out 450 pieces of content per week."*

For DevRel, the calibrated translation per `07_CONTENT_FACTORY.md §10`:

- 1 anchor / day = 7 anchors / week
- 12 derivatives / anchor = 84 derivatives / week
- 5 channel placements / derivative = 420 placements / week

That's the volume. Plus depth on the engagement surfaces (Discord, GitHub, HN comments) — no more than what the channel respects (per `08_DISTRIBUTION_CHANNELS.md`). Volume on owned + depth on third-party. Hormozi-volume + Joe-Karlsson-depth. Both, not either.

---

## §11. Cross-references

- Content Factory + GEO: `07_CONTENT_FACTORY.md`
- Channel cards (per-platform): `08_DISTRIBUTION_CHANNELS.md`
- Influence network mapping (T3 enterprise): `10_INFLUENCE_NETWORK_MAPPING.md`
- 90-day execution plan: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Case studies (Cursor / Linear / Vercel teardowns): `18_CASE_STUDIES.md`
- Per-product channel allocation: `11_PRODUCT_PLAYBOOKS.md`


---

<a id="13-consumer-social-launch"></a>
<!-- source: 13_CONSUMER_SOCIAL_LAUNCH.md -->

# 13 — Consumer Social Launch (Polis)

> Polis is fundamentally different from Leapable/Synapse. It is a **consumer social platform** where the unit of value is a **physical / synchronous gathering** in a **dense friend cluster**. Network effects dominate. This doc adapts the BeReal / Bluesky cold-start playbooks, layers in Hormozi's giveaway mechanics + Steve's tier model, and produces a Polis-specific cold-start plan.

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


---

<a id="14-compliance-and-ethics"></a>
<!-- source: 14_COMPLIANCE_AND_ETHICS.md -->

# 14 — Compliance and Ethics

> The hard floor. Every external action is gated by these rules. Violation is not a "warning" — it's a circuit breaker trip. This doc covers CAN-SPAM, GDPR, CCPA, CASL, the EU AI Act (Article 14 + Article 50, August 2026), platform ToS, ethical guardrails, and the operator's signed-off ethical floor for Tier-3 conduct.

---

## §1. The compliance floor (mechanically enforced)

Every outbound action through the Tool Gateway passes through the **ComplianceCheck** in the Guardrail Engine (`04_SYSTEM_ARCHITECTURE.md §8`). The check is deterministic, runs in O(1), and is **not LLM-mediated**. Tools that touch external surfaces are wrapped with compliance enforcement at the wrapper level, not at the agent prompt level. (`02_FIRST_PRINCIPLES.md §3` — trust the harness.)

The four primary regimes:

| Regime | Jurisdiction | When applies | Required actions |
|---|---|---|---|
| **CAN-SPAM** | United States | Any commercial email to US recipients | Truthful headers + subject; physical address; valid unsubscribe link honored within 10 business days; identify as ad when applicable |
| **GDPR + ePrivacy** | EU/EEA | Any data processing involving EU residents | Lawful basis (legitimate interest for B2B); privacy notice accessible; honor right-to-erasure, right-to-access; document legitimate interest balancing test |
| **CCPA** | California residents | Any data processing on CA residents | Honor opt-out requests; disclose data practices; "Do Not Sell My Personal Information" mechanism if sale occurs |
| **CASL** | Canada | Any commercial email/SMS to Canadian recipients | Implied or express consent; identification of sender; unsubscribe mechanism |

The fleet does **not** rely on the operator to remember these. The `send_email`, `send_sms`, `send_dm`, `paid_ad_targeting` tools are jurisdiction-aware wrappers.

---

## §2. The EU AI Act (Aug 2, 2026 — already in effect)

Regulation 2024/1689. Two articles directly govern the marketing fleet:

### 2.1 Article 50 — AI-content disclosure

> AI systems interacting with natural persons must disclose that the interaction is AI-generated unless the context makes it obvious. Article 50(4) specifically addresses AI-generated content intended to inform or influence — which describes AI-generated outbound marketing.

**Implementation:**
- Every outbound email to an EU recipient that was drafted or substantially modified by AI carries a disclosure line: *"Parts of this message were drafted with AI assistance."*
- Every public-facing post on operator's accounts that was drafted by AI and not edited substantially by the operator carries an AI-assistance footnote (channel-appropriate format).
- AI-generated chat agent interactions (e.g., comment-triage replies via D6) carry an in-thread disclosure on first reply.
- The disclosure is injected by the tool wrapper. The Content Writer cannot bypass it.

### 2.2 Article 14 — Human oversight

> For AI systems posing material risk, a natural person must be able to effectively oversee, interrupt, and override AI decisions.

**Implementation:**
- The autonomy slider (`02_FIRST_PRINCIPLES.md §2`) is the operator's effective oversight.
- Every external action class is configurable per goal between `AUTO` and `REQUIRE_APPROVAL`.
- Approval queue has documented audit trail: approver, approval timestamp, payload, decision.
- The operator's panic hotkey (kill switch) operates outside the agent loop and halts within 50ms.

This isn't a checkbox. The operator's role as overseer must be **documentable** — per-campaign approvals are logged in `CF_AUDIT_LOG` with operator ID, timestamp, decision. Auto-rejected items also log.

### 2.3 The "deployer" obligation

Article 50 places obligations on the **deployer** (the company sending the messages), not the model provider. The operator (us) is the deployer. The compliance burden is ours, not Anthropic's.

---

## §3. Cold outbound — what's legal, what's not

The most-litigated marketing surface. Per the 2026 cold-email compliance literature:

### 3.1 What's legal

- **B2B email under CAN-SPAM** with valid physical address, working unsubscribe link, truthful sender, accurate subject. **No prior consent required** in the US.
- **B2B email under GDPR via legitimate interest** with documented balancing test (Article 6(1)(f)).
- **Personalization via public information** (LinkedIn job title, public company news, public conference attendance).
- **Follow-ups** within established conversation threads.
- **AI-drafted email content** — the authoring tool doesn't affect compliance. Disclosure required only under EU AI Act if delivered to EU recipient.
- **CRM enrichment** from public sources.

### 3.2 What's not legal

- **B2C email outbound without explicit opt-in** in EU (GDPR), Canada (CASL), Australia (Spam Act).
- **Mass automated outreach without unsubscribe** — CAN-SPAM violation in US.
- **Scraping personal data behind logged-in walls** (LinkedIn member info beyond your network, Twitter DMs, private Slack).
- **Inferring protected attributes** (orientation, political, religious, health, race) and storing/using them.
- **Buying email lists** that don't include verified consent for the seller-to-purchaser handoff.
- **AI-personalized B2B email to EU contacts without disclosure** (EU AI Act, Aug 2026).
- **Automated decisions with legal or significant effects on individuals** without human review (GDPR Art. 22).

### 3.3 Documented legitimate interest balancing test (GDPR Art. 6(1)(f))

For B2B outbound to EU contacts, the operator maintains a written balancing test per campaign:

```yaml
campaign_id: leapable_enterprise_q3_2026
target_population:
  - role: VP Engineering / Director of AI Platform / Head of Developer Tools
  - employer: any company with ≥50 engineers and AI agent deployment
purpose: introduce Leapable for AI patch verification
data_source: public LinkedIn, Crunchbase, conference attendance lists, public AI-discussions
data_minimization: only role, employer, public-statement context; no protected attributes
balancing_test:
  legitimate_interest: yes — B2B product introduction relevant to recipient's stated role
  recipient_expectation: yes — recipients in this role are routinely contacted about AI tooling
  recipient_rights: full opt-out per email + unsubscribe portal; right-to-erasure honored within 30 days
  data_minimization_check: yes — we collect only what's needed for personalization
override_consent_required: no — this is legitimate interest, not consent
balancing_test_signed_by: operator
signed_at: 2026-05-26
```

The operator signs the balancing test per campaign. The fleet enforces by refusing to launch a campaign without a signed test.

---

## §4. Platform ToS

Every platform has a ToS. The fleet operates **only** within ToS. ToS violations are an immediate kill switch.

| Platform | ToS-binding rules the fleet honors |
|---|---|
| X / Twitter | No automation in violation of platform automation rules; no coordinated inauthentic behavior; no spam; rate-limits respected; computer-use as logged-in user on operator-owned account is allowed |
| LinkedIn | No automated scraping of member info beyond public; no automated connection requests outside platform tools; computer-use as logged-in user on operator-owned account is the gray-area allowed pattern; no automated profile data exports |
| Reddit | API-based bots permitted with clear bot disclosure; computer-use as a human-shaped logged-in user permitted; coordinated upvote/downvote rings prohibited |
| Discord | Bots in bot-tagged channels permitted; impersonation prohibited; mass-DM prohibited |
| Hacker News | One account per person; no upvote rings; no posting from anonymous accounts on behalf of brand |
| TikTok | No automated uploading via scraped API; computer-use on operator account permitted with rate-limit respect; no fake accounts |
| Instagram / Meta | No automation against ToS; ad platform separate from organic |
| YouTube | Channel-owner automation allowed within YouTube Studio limits; no coordinated comment-manipulation |
| GitHub | API-based automation permitted with token; webhook automation permitted; no spam in PRs/issues |

The Channel Cards (`08_DISTRIBUTION_CHANNELS.md §5`) carry the per-platform ToS-derived rate caps. The fleet operates **30% below** the bot-detection thresholds (`06_COMPUTER_USE_PLAYBOOK.md §4`).

---

## §5. Ethical floor (operator's signed commitments)

Beyond legal compliance, the operator commits to the following ethical floor. These are non-negotiable doctrine.

### 5.1 No deception

- Never claim AI-generated content was human-written when it wasn't (and vice versa where material).
- Never fabricate testimonials, endorsements, reviews, or quotes.
- Never paint a competitor's product inaccurately. Comparison content is rigorously fact-checked.
- Never use Tier-3 names in marketing without their explicit, written permission.
- Never invent or imply relationships, partnerships, or endorsements that don't exist.

### 5.2 No coordinated inauthentic behavior

- Never operate accounts not owned by the operator.
- Never use synthetic personae to amplify the operator's content.
- Never coordinate engagement across accounts to game algorithms.
- Never purchase followers, likes, comments, reviews, upvotes.
- Never use bots to inflate metrics.

### 5.3 No abuse of personal data

- Never collect protected attributes (orientation, political, health, religion, race).
- Never infer protected attributes from public signals.
- Never store dossiers that include such inferences.
- Never use scraped personal data outside legitimate-interest documented use cases.
- Honor every opt-out + right-to-erasure request within 7 days.

### 5.4 No predatory tactics

- Never target vulnerable populations (minors, recently bereaved, those in financial distress, those in active addiction recovery) for paid acquisition.
- Never use dark patterns in funnel UI (forced continuity, hidden costs, "confirm-shaming" decline buttons).
- Never use scarcity / urgency that isn't real ("only 3 left" when there's unlimited).
- Never use confusing pricing or hidden charges.

### 5.5 No platform manipulation

- Never participate in upvote rings, follower rings, retweet rings.
- Never gamify community moderation.
- Never use multiple operator accounts to vote on a single piece of content.
- Never use AI to mass-evade content moderation.

---

## §6. Polis-specific ethical commitments

Polis is a consumer social product with in-person attendance. Additional commitments:

- **Attendee consent on photo / quote / recording** — opt-in default, not opt-out.
- **No undisclosed influencer events** — if a host is compensated, attendees know.
- **No targeting by inferred political / religious / sexual identity.**
- **Active safety culture at events** — anti-harassment policy, named contact for incidents, clear consequences.
- **Privacy of attendee contact data** — Polis does not sell or share attendee lists.
- **Right to leave**: attendees can request all their event data deleted; honored within 7 days, no questions asked.

---

## §7. The audit trail

Every external action is logged in `CF_AUDIT_LOG` with sufficient information that:

- A regulator could be answered with documentary evidence within 24 hours
- An attendee / lead / contact could be answered "what data do you have on me" within 7 days
- A platform-suspension review could be answered with a full history of every action on the affected account

Schema:

```yaml
audit_id: uuidv7
goal_id: leapable-50k-q3-2026
cycle_id: 2026-05-26T14:00:00Z
node_id: experiment-ph-launch-001
agent_class: D1
tool: x_post
account_id: x_chris_personal
timestamp: 2026-05-26T14:18:32Z
payload_hash: sha256:...
payload_content: "[…]"
guardrail_decision: allow
brand_voice_score: 0.91
compliance_checks_passed: [host_allowed, rate_limit_ok, brand_voice_ok, eu_disclosure_na_for_us_account]
operator_approval_required: false
operator_approval_log: na
outcome: ok
external_response: "{platform-response}"
cost_usd: 0.0
cost_tokens: 1240
side_effects: [post_published]
related_audit_ids: [...]
```

Audit log is append-only. Operator can query via dashboard: "show me every action on this account in the last 7 days," "show me every email to this contact ever," "show me every paid ad spend on this campaign."

---

## §8. The Right to Erasure (GDPR Art. 17 + CCPA)

Implementation:
- An EU recipient's right-to-erasure request triggers:
  - Delete contact from CRM
  - Delete from CF_CONTACTS in the marketing graph
  - Delete from any campaign suppression list (so they're not re-acquired by mistake)
  - Add to permanent "do not contact" list (preventing future re-import)
  - Confirm completion to requester within 30 days
- A CCPA "do not sell my personal info" request triggers similar deletions plus a Sell-Indicator flag (we don't sell info, but track the request).
- Both go through the **GDPR Tool Endpoint** wrapper, which is operator-monitored.

The fleet does **not** auto-delete on receiving an inbound email mentioning "remove me" — it raises an approval-queue item with the request quote, draft response, and proposed action. Operator confirms.

---

## §9. Operator's escalation tree

When the fleet detects something compliance-adjacent:

| Detection | Routing |
|---|---|
| Brand-voice violation post-publish | Reflector + Operator notification + content takedown if severe |
| Rate-limit warning from platform | Operator notification + channel-card update + rate-cap reduction |
| ToS warning from platform | **Operator immediate notification** + activity pause on affected platform + Reflector post-mortem |
| Account suspension | Kill switch on platform + operator-led appeal + post-mortem |
| Regulator inquiry (GDPR, CCPA, etc.) | **Operator immediate notification** + legal escalation + every relevant operation paused |
| Subpoena or law enforcement contact | **Operator immediate notification** + legal counsel + no agent activity related to the matter until counsel reviews |
| Subject access request | Approval queue with proposed response + operator review |
| Right-to-erasure request | Approval queue with proposed deletions + operator review |

The first three are routine. The bottom four are escalations that pull in counsel.

---

## §10. The "no" list (forever)

Specific tactics the fleet **never** uses, regardless of operator approval:

- Mass cold DM via automation
- Multi-account upvote / engagement rings
- Synthetic personae
- Buying engagement signals (followers, likes, reviews, upvotes)
- Dark patterns
- Predatory targeting
- Scraping behind logged-in walls
- Inferring protected attributes
- Using a real person's identity without consent

These cannot be enabled in the goal spec. They are not subject to autonomy slider override. The Guardrail Engine refuses tool calls implementing them, with a clear error code.

---

## §11. Cross-references

- Guardrail engine: `04_SYSTEM_ARCHITECTURE.md §8`
- Computer-use ToS + rate caps: `06_COMPUTER_USE_PLAYBOOK.md §4`
- Tier-3 ethics (closed-bubble conduct): `10_INFLUENCE_NETWORK_MAPPING.md §9`
- Polis-specific event ethics: `13_CONSUMER_SOCIAL_LAUNCH.md §10`
- Audit log schema: `15_METRICS_AND_OBSERVABILITY.md §8`
- Kill switches and circuit breakers: `20_RISK_AND_FAILURE_MODES.md`
- Operator approval queue UX: `19_OPERATIONS_AND_CADENCE.md §3`


---

<a id="15-metrics-and-observability"></a>
<!-- source: 15_METRICS_AND_OBSERVABILITY.md -->

# 15 — Metrics and Observability

> The numerical target sheet, the attribution model, the evaluator calibration discipline, and the operator's dashboards. Borrowed from Leapable's tiered metric system (Tier-1 critical, Tier-2 operational, etc.) and applied to a marketing surface. Vendor dashboards are diagnostic; **source-of-truth product database readback is the verdict** (`02_FIRST_PRINCIPLES.md §5`).

---

## §1. Metric tiers

Per Leapable's pattern, marketing metrics are tiered. Higher tier = higher gate-status.

| Tier | What it is | Gate status |
|---|---|---|
| **T1 — Predicate** | The one scalar that defines goal success | Gate. Until this holds × 4 windows, the goal isn't done. |
| **T2 — Operational** | LTV, CAC, payback, retention, K-factor | Gate. Floor violations stop scaling. |
| **T3 — Channel-level** | Per-channel CAC, per-channel conversion, per-channel retention | Diagnostic — informs Planner's reallocation. |
| **T4 — Invariant** | Doctrine constants (brand voice score, compliance score, rate-limit respect) | Permanent invariants — these don't move. |
| **T5 — Bench-delta** | Bench performance under model changes / agent rewrites | Gate for shipping new agent versions. |
| **T6 — Watch** | Goodhart correlated reality signals | Gate via the Goodhart Auditor. |

---

## §2. T1 — Predicate per product

Per `01_NORTH_STAR.md §3`:

### 2.1 Leapable

```yaml
metric: weekly_active_subscribers
definition: unique_developer_ids with ≥1 Leapable verification call in the past 7 days,
            where the verification produced a non-trivial verdict
            (verdict_kind != "noop" or "input_unreadable")
source_of_truth: rocksdb://leapable/cf_queries
evaluator: M7 reads SoT once per 24h cycle
goal_thresholds_examples:
  90_day: 2_000
  6_month: 20_000
  12_month: 50_000
window: 28d rolling
consecutive_windows_required: 4
```

### 2.2 Synapse

```yaml
metric: weekly_active_mcp_sessions_with_depth
definition: distinct sessions in 7d with ≥3 distinct tools called AND duration ≥60s
source_of_truth: rocksdb://synapse/cf_sessions
goal_thresholds_examples:
  90_day: 2_500
  6_month: 12_000
  12_month: 25_000
```

### 2.3 Polis

```yaml
metric: monthly_active_event_attendances
definition: count distinct (user, event) pairs in 28d with attended=true
            AND event_status='completed'
source_of_truth: postgres://polis/events_attendances
goal_thresholds_examples:
  90_day: 6_000
  6_month: 30_000
  12_month: 50_000
```

---

## §3. T2 — Operational gates (per product)

### 3.1 LTV:CAC floor

| Product | Floor | Best-in-class |
|---|---|---|
| Leapable | 3.0:1 (paid subscribers) | 5:1 |
| Synapse | 3.0:1 (paid subscribers) | 4.5:1 |
| Polis | 4.0:1 (Plus + Curator paid tiers) | 6:1 |

**Below floor → no scaling of paid acquisition.** The Cost Pacer enforces.

### 3.2 CAC payback ceiling

| Product | Ceiling |
|---|---|
| Leapable | 12 months (B2B SaaS, dev-tools-class) |
| Synapse | 12 months |
| Polis | 9 months (consumer; faster payback required) |

**Above ceiling → no scaling of paid acquisition.**

### 3.3 Cohort retention floor

| Product | 28-day floor | 90-day floor |
|---|---|---|
| Leapable (paid) | 70% | 50% |
| Leapable (free) | 50% | 25% |
| Synapse (paid) | 65% | 45% |
| Polis (Plus paid) | 75% | 55% |
| Polis (free attendees) | 60% within-cluster | 35% within-cluster |

**Below floor → referral programs paused** (`09_VIRAL_LOOPS_AND_REFERRALS.md §4`).

### 3.4 K-factor floor (referral programs)

| Product | K floor | K target | Cycle time target |
|---|---|---|---|
| Leapable | 0.2 | 0.4 | 5 days |
| Synapse | 0.15 | 0.35 | 7 days |
| Polis | 0.3 | 0.6 | 3 days |

### 3.5 Hormozi-Mandatory: 30-day client-financed acquisition check

```
realized_30d_gross_profit_per_acquired_user >= CAC + 30d_fulfillment_cost
```

| Product | Target |
|---|---|
| Leapable | $90 30d-GP per acquired developer ($99/mo Pro × ~90% margin) |
| Synapse | $50 30d-GP |
| Polis | $25 30d-GP |

`TRUE` → paid acquisition can scale.
`FALSE` → paid spend capped at available float; raise pricing or improve LTV.

---

## §4. T3 — Channel-level diagnostics

Per channel × product, tracked daily:

```yaml
channel: hackernews
product: leapable
last_28d:
  posts_published: 4
  total_impressions: 142_500
  clicks_to_product_url: 4_320
  signups: 213
  paid_conversions_within_28d: 19
  cac_blended: 0 (organic)
  cac_paid_equivalent: $187 (if we re-bought the attention)
  cohort_28d_retention: 0.73
  cohort_90d_retention_projected: 0.51
  ltv_observed_28d: $98
  ltv_cac_observed: n/a (organic)
  geo_citation_share_quarterly: 4%  # share of relevant AI search queries that quote this content
```

The Planner reads channel-level diagnostics weekly to reallocate. A channel where signups dropped 50% for 3 consecutive weeks gets investigated and possibly deprioritized.

---

## §5. T5 — Bench-delta gate

Per `02_FIRST_PRINCIPLES.md §14`: any material agent change ships only after a bench-delta gate. The bench harness:

- Holds 200 historical experiments per agent class (200 generated drafts, 200 outreach messages, 200 launches, etc.)
- New agent version is scored against the holdout
- Quality score (LLM-as-judge, calibrated) must not regress from prior version
- Cost per artifact must not regress more than 5%

If either regresses, the change is rejected. If both improve, the change ships to canary (5% traffic) then phased (25 → 50 → 100%) per `05_AGENT_FLEET_DESIGN.md §9`.

The bench harness lives in `bench/marketing/<agent_class>/` and is operator-runnable.

---

## §6. Cost dashboard

Operator's primary dashboard view (refreshed every 15 minutes):

```
┌─ ACTIVE GOALS ────────────────────────────────────────┐
│ leapable-50k-q3-2026: WEEK 8 of 14                 │
│   predicate: 1,247 / 2,000  (62%)  ▲ +180 wk-over-wk   │
│   spend:     $13,400 / $25,000  (54%)                  │
│   ltv:cac:   4.1:1  ✓ above 3.0 floor                  │
│   payback:   8.2mo  ✓ below 12mo ceiling               │
│   retention: 68% 28d  ⚠ below 70% floor                │
│   k-factor:  0.31    ⚠ below 0.40 target               │
│   guardrails: all green                                │
│   open approvals: 2                                    │
│ synapse-…: PAUSED (operator)                            │
│ polis-…:   not yet active                              │
├─ LAST 24h ─────────────────────────────────────────────┤
│ artifacts produced: 18 (10 owned, 8 channel-shipped)   │
│ external actions: 142 (138 auto, 4 approved)           │
│ token spend: $18.40                                    │
│ paid ad spend: $487                                    │
│ kill switches: 0 trips                                 │
├─ ALERTS ───────────────────────────────────────────────┤
│ ⚠ retention 68% (below 70% floor) — review at week 9   │
│ ⚠ k-factor 0.31 (below 0.40 target) — Referral Eng    │
│   has 3 proposals queued                               │
└────────────────────────────────────────────────────────┘
```

The dashboard is **descriptive of source-of-truth**, not a self-reported vanity log.

---

## §7. LLM-as-judge evaluator calibration

Per `05_AGENT_FLEET_DESIGN.md §3` and `09_VIRAL_LOOPS_AND_REFERRALS.md`-adjacent: G-Eval / AdaRubric rubrics drive most quality gates.

### 7.1 The calibration discipline

Per the 2026 G-Eval literature:

> *"Calibrate every rubric against human labels. Collect 50–200 human-labeled examples per rubric. Run the judge on the same set. Compute Cohen's kappa or threshold-based accuracy. A marketing-copy rubric tolerates kappa around 0.6. A medical-advice rubric needs 0.85+."*

The operator calibrates each rubric quarterly:

| Rubric | Calibration set size | Kappa target | Re-cal frequency |
|---|---|---|---|
| Brand voice (per product) | 100 | ≥ 0.65 | quarterly |
| Hallucination check | 50 | ≥ 0.80 | quarterly |
| Compliance check | 50 | ≥ 0.85 | quarterly |
| Outreach quality | 100 | ≥ 0.60 | quarterly |
| Reply substance (Comment Triage) | 50 | ≥ 0.55 | quarterly |
| Tier classification (Audience Sorter) | 100 | ≥ 0.70 | quarterly |

Below-kappa rubrics get rewritten or replaced with deterministic classifiers (the fine-tuned 4B-class Gemma model handles brand voice well, for example).

### 7.2 Multi-layer evaluation per `05_AGENT_FLEET_DESIGN.md §3`

| Layer | Cost | Purpose |
|---|---|---|
| L1 — Schema | ~free | malformed outputs |
| L2 — Deterministic | ~free | forbidden phrases, dup content |
| L3 — Classifier (fine-tuned 4B) | <50ms | brand voice, toxicity, PII |
| L4 — LLM-as-judge | $0.01-0.05 | quality, helpfulness, on-rubric coherence |
| L5 — Reality readback | ~free | did the intended effect happen at SoT? |

L1-L3 are gates. L4-L5 are diagnostics + Reflexion feed.

---

## §8. Audit log + observability

Per `14_COMPLIANCE_AND_ETHICS.md §7`, the audit log is the single source of truth for "what did the fleet actually do":

- Every external action logged with payload, timestamp, account, agent, outcome.
- Every approval-queue decision logged with approver, decision, timestamp.
- Every guardrail decision (allow/deny) logged with the reason.
- Append-only; cryptographically chained (per Leapable audit doctrine).

Operator query patterns:

```
marketing audit --account=x_chris_personal --since=24h
marketing audit --contact=alice@example.com --all-time
marketing audit --campaign=ph_launch_2026_01_22 --include-children
marketing audit --kill-switch-trips --since=7d
marketing audit --eu-disclosure-attached --since=30d  # for compliance review
marketing audit --rate-limit-denials --by-channel --since=7d
```

---

## §9. Attribution model

Multi-touch attribution with **first-touch + last-touch + assist credits**:

```yaml
user_id: developer_2389
acquisition_journey:
  - t: 2026-04-12  channel: x_organic   action: clicked_anchor_post  attribution: 0.2
  - t: 2026-04-15  channel: github      action: starred_repo          attribution: 0.1
  - t: 2026-04-18  channel: discord     action: posted_question       attribution: 0.1
  - t: 2026-04-22  channel: latentspace_podcast  action: heard_episode  attribution: 0.3
  - t: 2026-04-25  channel: signup_landing_page  action: signed_up      attribution: 0.3 (last-touch)
total_attribution_credit: 1.0
realized_ltv_28d: $0 (free tier)
realized_ltv_90d: $0 (still on free tier)
projected_ltv_12m: $1,180 (predicted via cohort model)
cac_by_channel_allocation:
  x_organic: $30 * 0.2 = $6
  github: $20 * 0.1 = $2
  discord: $25 * 0.1 = $2.50
  latentspace: $400 * 0.3 = $120
  landing_page: $0 (organic)
acquisition_blended_cac: $130.50
```

The Cohort Health Auditor (O7) maintains the attribution graph. The Planner uses it to allocate spend. The operator sees per-channel ROI on the dashboard.

---

## §10. The Hormozi unit economics rule of thumb

Hormozi: *"What every business owner wants is they want cheaper customers. It's usually the last thing they really need."*

The dashboard surfaces **LTV:CAC**, **not raw CAC**. A $500 CAC delivering $5,000 LTV (10:1) is preferred over a $200 CAC delivering $1,000 LTV (5:1).

When the operator says "the CAC is too high," the dashboard answers with the LTV:CAC ratio. If the ratio is healthy, the operator's instinct is wrong — the channel is good and worth scaling. If the ratio is unhealthy, the answer is to fix LTV (raise prices, extend retention) **before** lowering CAC.

This is Hormozi's "expensive customers pay better" insight, operationalized.

---

## §11. The Goodhart Auditor (recap)

Per `01_NORTH_STAR.md §6` and `05_AGENT_FLEET_DESIGN.md §6.2`:

The Auditor runs daily, checks predicate vs three correlated reality signals, trips `goodhart_signal_detected` if divergence > 2x over 14d. Per-product reality signals:

| Product | Predicate | Reality signals |
|---|---|---|
| Leapable | `weekly_active_subscribers` | Paid subscribers, GitHub star velocity, HN mention velocity, Discord question rate |
| Synapse | `weekly_active_mcp_sessions_with_depth` | Tools-per-session distribution, third-party MCP servers referencing Synapse, hackathon project count |
| Polis | `monthly_active_event_attendances` | Return attendance rate, attendee NPS, photo-consent rate, friend-graph density delta |

The Auditor's report is published weekly to the operator regardless of trip status. The signal-vs-predicate plot is a permanent dashboard element.

---

## §12. The metrics-as-doctrine summary

Per `02_FIRST_PRINCIPLES.md §11` (anti-Goodhart) + `02_FIRST_PRINCIPLES.md §5` (SoT readback) + `02_FIRST_PRINCIPLES.md §9` (no mocks):

- Vendor dashboards (Mixpanel, Posthog, GA, Segment) are **diagnostic only**.
- SoT (RocksDB / Postgres on the product side) is the verdict.
- Activation metrics > signup metrics.
- LTV:CAC > raw CAC.
- Cohort retention > one-time activation.
- The predicate gates; the diagnostics inform.
- Every claimed metric movement is checkable against the audit log + the SoT.

---

## §13. Cross-references

- The goal predicate format: `01_NORTH_STAR.md §1`
- Per-product predicate definitions: `11_PRODUCT_PLAYBOOKS.md`
- Viral coefficient + cycle time math: `09_VIRAL_LOOPS_AND_REFERRALS.md`
- The Goodhart Auditor: `05_AGENT_FLEET_DESIGN.md §6.2`, `20_RISK_AND_FAILURE_MODES.md §4`
- The Cost Pacer: `05_AGENT_FLEET_DESIGN.md §6.4`
- Compliance audit log: `14_COMPLIANCE_AND_ETHICS.md §7`
- Tech stack for metrics infrastructure: `16_TECH_STACK.md §7`


---

<a id="16-tech-stack"></a>
<!-- source: 16_TECH_STACK.md -->

# 16 — Tech Stack

> The concrete build-vs-buy + integration map. Built on LangGraph for orchestration, RocksDB-backed persistence, Markifact / Hyper MCP / Windsor for marketing-platform integrations, Synapse for computer-use, **leapable.ai for queryable expert-knowledge vaults**, the Anthropic Claude SDK for primary inference, fine-tuned classifiers for the brand/compliance gates. The thesis: agents have **all the tools they need** so they can reason and execute.

---

## §1. The stack at a glance

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         OPERATOR INTERFACE                                │
│  Claude Code / Codex CLI / Web Dashboard / Slack / Email digest          │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        ORCHESTRATION LAYER                                │
│  LangGraph (durable execution, checkpointing, HITL, state graph)         │
│  + custom OuterLoop (ASSESS → PLAN → BUILD → COMMUNICATE)                │
│  + ROMA-style recursive control (Atomizer / Planner / Executors / Aggr.)  │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                       AGENT FLEET (~26 classes)                           │
│  Each class is a LangGraph subgraph w/ its own prompt, tool surface,     │
│  evaluator, autonomy default, model routing.                              │
└────────────────────────────────┬─────────────────────────────────────────┘
                                 │
       ┌─────────────────────────┼──────────────────────────┐
       ▼                         ▼                          ▼
┌──────────────┐         ┌──────────────────┐      ┌──────────────────┐
│ LLM Inference │         │  Memory Layer    │      │  Tool Gateway    │
│ Anthropic     │         │  (LLM Wiki)      │      │  (MCP + custom)  │
│  Claude Opus  │         │  - skills        │      │                  │
│  Claude Sonnet│         │  - reflections   │      │                  │
│  Claude Haiku │         │  - channel cards │      │                  │
│ Fine-tuned    │         │  - brand voices  │      │                  │
│  Gemma 4B     │         │  - contact graph │      │                  │
│ Local 7B as   │         │                  │      │                  │
│  fallback     │         │  Backed by:      │      │                  │
└──────────────┘         │  - RocksDB CFs   │      │                  │
                         │  - filesystem MD │      │                  │
                         │  - leapable.ai   │      │                  │
                         │    expert vaults │      │                  │
                         └──────────────────┘      └──────────────────┘
                                                            │
            ┌───────────────────────────────────────────────┼───────────────┐
            ▼                                               ▼               ▼
┌──────────────────────┐                       ┌──────────────────┐  ┌────────────────┐
│ Marketing MCPs       │                       │ Computer-Use     │  │ Bespoke        │
│ - Markifact (300+)   │                       │ - Synapse        │  │ adapters       │
│ - Hyper MCP (200+)   │                       │ - Playwright     │  │ - GitHub gh CLI│
│ - Windsor (325+)     │                       │ - Browser-Use    │  │ - gh API       │
│ - 0nMCP (995+)       │                       │ - OpenClaw       │  │ - Anthropic SDK│
│ - SearchAtlas (112)  │                       │                  │  │ - OAuth flows  │
│ - AgentKits          │                       │                  │  │                │
└──────────────────────┘                       └──────────────────┘  └────────────────┘
       │                                              │                      │
       └─────────────────── External World ───────────┴──────────────────────┘
                   (ads, social, CRM, email, web, games, etc.)
```

---

## §2. Orchestration — why LangGraph

Per the 2026 multi-agent orchestration framework comparison:

> *"LangGraph is the most production-ready orchestration framework. State persistence is native, error recovery is declarative, and the graph visualiser makes debugging tractable."*

| Capability | Why we need it |
|---|---|
| Durable execution + checkpointing | Goals run for months; daemon must survive restarts and API outages |
| Native human-in-the-loop interrupts | Approval queue patterns are first-class |
| State graph that's debuggable | Multi-month traces need a visualizer |
| Token overhead +9% (best in class) | Cost discipline |
| LangSmith integration | Observability + replay |

The OuterLoop is implemented as a LangGraph StateGraph with nodes for `assess`, `plan`, `atomize`, `dispatch`, `aggregate`, `evaluate`, `communicate`. Each Executor is a sub-graph that LangGraph dispatches in parallel.

**Alternative considered:** CrewAI (faster prototype but weaker production observability) and AutoGen / MS Agent Framework 1.0 (conversational debate model — wrong fit for our deterministic-pipeline use).

---

## §3. LLM model routing

Per `04_SYSTEM_ARCHITECTURE.md §10.3`:

| Task | Model | Why |
|---|---|---|
| Planner, Reflector, Aggregator, Evaluator (M3-M9) | Claude Opus 4.7 (or current frontier) | High-stakes reasoning |
| Content Writer (P1), Outreach Drafter (D4) | Claude Sonnet 4.6 (or current standard) | Quality + cost balance |
| Brand-voice classifier, compliance check | Fine-tuned Gemma 4B | <50ms; low cost; deterministic |
| Schema validation, simple extraction | Claude Haiku 4.5 | Cheapest tier |
| Hallucination check (reverse citation) | Sonnet + web tool | Verification |
| Influence Mapper graph queries | Sonnet | Graph traversal + structured output |
| Embeddings (LLM Wiki + leapable) | gte-multilingual-base | Vault-compatible |

Routing is a **mechanical wrapper** around the LLM call site, not an agent decision. Decisions like "send to Opus" are encoded per agent-class card (`05_AGENT_FLEET_DESIGN.md §2`).

---

## §4. Memory — the LLM Wiki + leapable.ai

The memory layer is the single biggest leverage point. Borrowed from Karpathy's LLM Wiki gist + the Yopedia pattern + extended with **leapable.ai vault-as-a-service**.

### 4.1 Three memory surfaces

| Surface | Backed by | Used for |
|---|---|---|
| **Project Wiki** (operational state) | RocksDB CFs + markdown files | Channel cards, brand voices, contact graph, reflections, skills, wins/losses |
| **Goal Memory** (per active goal) | RocksDB CFs (CF_GOALS, CF_GOAL_CYCLES, CF_EXPERIMENT_NODES, CF_REFLECTIONS, CF_AUDIT_LOG) | Per-goal state, durable across restarts |
| **Expert Vaults** (queryable knowledge) | **leapable.ai** | Externalized expert knowledge: Hormozi corpus, Karpathy corpus, Tier-3 contact corpus, competitor research |

### 4.2 leapable.ai integration

**Why leapable.ai matters here:** the operator already maintains domain-expert vaults on leapable.ai (e.g., `AlexHormoziTranscripts` with 1,150 documents / 6,428 chunks of Hormozi's complete recorded content). These vaults are **directly queryable by the agent fleet** via the leapable MCP server (`mcp__leapable__leapable_search` and friends).

**Operational pattern:**

The agent fleet queries the relevant expert vault before planning. Example:
- Before drafting a Grand Slam Offer for Leapable, P4 (Brief Generator) queries `AlexHormoziTranscripts` for Hormozi's offer-structure principles, then drafts the brief grounded in primary-source Hormozi material.
- Before launching a podcast tour for Polis, D8 (Podcast Booker) queries any operator-curated `PodcastingPlaybook` vault for tactical patterns and host-relationship etiquette.
- Before structuring a viral loop, O3 (Referral Engineer) queries the Hormozi vault for the Skool.com two-prize-giveaway worked example.

**Required vaults (operator builds these once; agents query forever):**

| Vault | Sources | Used by |
|---|---|---|
| `AlexHormoziTranscripts` (exists) | 1,150 docs — `$100M Offers`, `$100M Leads`, `$100M Money Models`, ~1000+ YouTube transcripts | All agents that touch positioning, copy, offers, referrals, scaling |
| `KarpathyCorpus` | All Andrej Karpathy talks, blog posts, AutoResearch repo, AgentHub repo, Software-3.0 keynote | Architecture decisions, agent-design discussions |
| `OperatorBrandVoices` | Every operator-approved piece of content, every operator-rejected piece, every operator correction | P1/P2 brand voice grounding; evaluator calibration |
| `CompetitiveIntel` | Public docs, blog posts, talks from named competitors | S5 Competitive Intelligence |
| `Tier3ContactProfiles` | Public-facing content from operator's Tier-3 target list — talks, papers, posts | Influence Network Mapper (S3), Pre-Call Briefer |
| `ProductTechSpecs` | Leapable, Synapse, Polis full documentation (CLAUDE.md, etc.) | P2 Technical Writer, accuracy grounding for every claim |
| `DevRelLiterature` | Open-source DevRel playbooks (FORKOFF, 5W, etc.) | D2, D3, D8 |
| `ComplianceCorpus` | CAN-SPAM text, GDPR text, CCPA text, CASL text, EU AI Act text, platform ToS docs | Compliance Check at the Guardrail Engine |
| `LeapableVaultMeta` | Internal vault about how to use leapable vaults effectively | Wiki Gardener M10 |

**Tool surface from the leapable MCP server** (already loaded — see the tools loaded in the session): `leapable_search`, `leapable_db_select`, `leapable_search_cross_db`, `leapable_document_get`, `leapable_document_page`, `leapable_chunk_context`, `leapable_image_search`, `leapable_marketplace_creators`, `leapable_prompt_search`, and ~120 more.

**Per-agent vault access policy:**

Each agent class card (`05_AGENT_FLEET_DESIGN.md §2`) lists which vaults that agent is allowed to query. The Tool Gateway enforces — an agent can't query a vault it isn't authorized for. Default: agents query only the vaults relevant to their specialty.

**Why not just RAG into our own vector store?** Two reasons:
1. Leapable's per-vault management UI, hybrid BM25+semantic+reranker pipeline, and provenance chain are operationally already-built and operator-curated.
2. The operator can publish vaults via the leapable marketplace if they want to monetize a domain-expert knowledge artifact — the same vault that powers the fleet can power a SaaS line.

---

## §5. Tool Gateway — the MCP-centric universe

Per `04_SYSTEM_ARCHITECTURE.md §9`, the Gateway is the single path to external systems. It exposes a unified MCP-style interface backed by:

### 5.1 Marketing-platform MCPs (preferred for any platform they cover)

| MCP server | Coverage | Primary use |
|---|---|---|
| **Markifact** | 300+ operations across Google Ads, Meta Ads, TikTok Ads, LinkedIn Ads, Microsoft Ads, Reddit Ads, Pinterest Ads, Snapchat Ads, Amazon Ads, DV360, GA4, BigQuery, Google Search Console, Google Business Profile, Google Merchant Center, Facebook, Instagram, LinkedIn, Shopify, HubSpot, Klaviyo, WhatsApp, Slack | Paid media + organic social + CRM + analytics from one MCP |
| **Hyper MCP** | 200+ integrations, 40+ built-in tools, 100+ skills covering paid media, SEO, content, social, research | Comprehensive — Reddit/X/Insta/YT/TikTok scrapers; Firecrawl; image+video gen |
| **Windsor MCP** | 325+ data sources (live data) — Meta Ads, Google Ads, TikTok Ads, GA4, HubSpot, Salesforce, Shopify, Stripe, BigQuery, Snowflake, QuickBooks, Xero | Data-warehouse-style query across marketing data |
| **SearchAtlas Agentic Omnichannel Marketing MCP** | 112 tools — SEO/GEO/Google Ads/local SEO/GMB/digital PR/website creation | Generative Engine Optimization specifically |
| **0nMCP** | 995+ tools, 55 services with one-server-every-API claim | Fallback for long-tail integrations |
| **leapable.ai MCP** | Vault search + management, marketplace | Memory layer (§4.2) |
| **GitHub MCP (gh CLI + native MCP)** | Repos, issues, PRs, releases, sponsors | D3 GitHub Maintainer |
| **Gmail / MS Graph MCP** | Email, Calendar | D4 Outreach Drafter, D7 Newsletter Pitcher, M9 Communicator |
| **Synapse MCP** | Computer-use on operator's desktop | D2 Community Operator on platforms without APIs |

### 5.2 Computer-use adapters

| Adapter | When used |
|---|---|
| **Synapse** | Operator-account flows on platforms where API is gated / nonexistent. See `06_COMPUTER_USE_PLAYBOOK.md`. |
| **Playwright** | Parallel read-only scraping of public surfaces (non-operator accounts; residential IP rotation). |
| **Browser-Use / OpenClaw** | Goal-driven browser automation for interactive flows (partnership inquiry forms, etc.) |

### 5.3 Bespoke adapters

Where neither MCP nor computer-use is the right tool, custom adapters:

- Anthropic SDK (direct LLM access; primary inference path)
- OpenAI SDK (fallback / model comparison)
- Direct PostgreSQL / RocksDB (product database readback for SoT)
- Email-deliverability vendor SDKs (Mailgun, SendGrid, Postmark) when sending at scale outside Gmail/MS Graph

---

## §6. Data layer

### 6.1 RocksDB schema (marketing-platform-specific CFs)

Per `04_SYSTEM_ARCHITECTURE.md §6`:

```
CF_GOALS                  — active and historical goal specs (JSON)
CF_GOAL_CYCLES            — every assess→plan→build→communicate cycle, with timestamps + diffs
CF_EXPERIMENT_NODES       — DAG of experiments per goal
CF_APPROVAL_QUEUE         — HITL items
CF_REFLECTIONS            — Reflexion memory
CF_WIKI_INDEX             — Index for filesystem-backed wiki pages
CF_CONTACTS               — Contact graph + tier classifications
CF_AUDIT_LOG              — Append-only event log
CF_CHANNEL_STATE          — Per-channel rate-limit buckets, posting history hashes
CF_PERSONA_HISTORY        — Per-persona publishing history (for dedup)
CF_ATTRIBUTION_GRAPH      — Multi-touch attribution edges
CF_KILL_SWITCHES          — Active flags, cached (≤2s TTL)
```

### 6.2 PostgreSQL (where SoT matters)

- Leapable product DB (CF_QUERIES readback for predicate)
- Synapse product DB (CF_SESSIONS readback for predicate)
- Polis product DB (event_attendances table readback for predicate)

The fleet **reads** product DBs directly for predicate evaluation. It does not write to product DBs. (Per `02_FIRST_PRINCIPLES.md §7` — product changes are operator-gated.)

### 6.3 Filesystem (markdown-backed wiki)

The LLM Wiki primary store is filesystem markdown. RocksDB indexes it. This is Karpathy's pattern: Obsidian-style live-editable knowledge that the agents read and the operator can browse in their normal editor.

---

## §7. Observability

| Capability | Tool |
|---|---|
| Per-agent traces | LangSmith (the LangGraph-native option) |
| Metrics + dashboards | Prometheus + Grafana (operator-hosted) |
| Logs | structured JSON to local + S3-compatible archive |
| Alerts | Slack + email via PagerDuty-style routing on circuit-breaker trips |
| Operator dashboard | Custom React app reading from RocksDB + LangSmith |

The operator dashboard (`15_METRICS_AND_OBSERVABILITY.md §6`) is the primary daily-use surface. It's built as a separate UI, not embedded in any agent flow.

---

## §8. Deployment topology

| Component | Where | Why |
|---|---|---|
| Orchestration (LangGraph) | Single host (operator's primary workstation or rented VM) | State must be durable, single-writer |
| Agent fleet (LLM calls) | API-only (Anthropic, OpenAI) | No GPU on the orchestration host |
| Computer-use (Synapse) | Operator's Windows desktop | Required for Win32 ops |
| Computer-use (Playwright pool) | Separate VM pool with residential IP rotation | Public read-only scraping |
| RocksDB | Same host as orchestration | Co-located for low latency |
| Wiki (filesystem) | Same host | Co-located + git-backed |
| Leapable.ai vaults | Operator's local leapable runtime | Owned + queryable |
| Marketing MCPs | Hosted by vendors (Markifact, Hyper, Windsor) | SaaS |
| Operator dashboard UI | Same orchestration host (port-exposed) | Local-only by default; cloud-exposable on operator-approved deploy |

**Single-host doctrine.** The orchestration host is the canonical state machine. Other components are stateless / queryable. If the orchestration host dies, the system is paused, not corrupted.

---

## §9. Build vs buy

| Component | Build or buy | Why |
|---|---|---|
| Orchestration | Buy (LangGraph) | Mature, production-grade, fits our model |
| LLM inference | Buy (Anthropic API) | Best frontier model |
| Marketing platform integrations | Buy (Markifact / Hyper / Windsor) | Already-built, well-maintained, $20-200/mo total |
| Memory expert vaults | Buy (leapable.ai) | Operator already runs |
| Brand voice classifier | Build (fine-tune Gemma 4B on operator's content) | Custom calibration required |
| Audience Sorter | Build | Custom signal weights |
| Influence Network Mapper | Build (atop leapable + LinkedIn data via Hyper MCP) | Custom graph weights |
| Operator dashboard | Build | UI is opinionated |
| Approval queue UI | Build | Tight integration with our LangGraph state |
| Computer-use | Buy (Synapse, which operator already owns) | Already shipped |

Buy where the integration is mature and standard. Build where the operator's specific knowledge is the value.

---

## §10. Costs envelope (monthly)

| Line item | Monthly cost (early) | Monthly cost (mature) |
|---|---|---|
| Anthropic API (Opus + Sonnet routed) | $400-1,200 | $2,000-8,000 |
| Marketing platform MCPs (Markifact, Hyper MCP, Windsor) | $200-500 | $500-1,500 |
| leapable.ai (existing operator subscription) | (operator) | (operator) |
| Hosting (orchestration VM, Playwright pool) | $100-300 | $400-1,000 |
| Observability (LangSmith + Grafana) | $50-150 | $200-600 |
| Email deliverability vendor (when scaling outbound) | $0 (Gmail) | $300-800 |
| Fine-tuning infra (one-time + maintenance) | $200-500 | $200-500 |
| **Total infrastructure** | **~$1,000-2,500/mo** | **~$3,600-12,400/mo** |
| Paid acquisition (Google/Meta/etc.) | per goal budget | per goal budget |

Infrastructure cost is roughly **5–10% of the paid acquisition budget** at maturity. Well below typical marketing-ops department overhead.

---

## §11. Security posture

- **Secrets management:** keychain on operator's host; never in agent prompts; rotated monthly.
- **Account credentials:** OAuth tokens where possible; least-privilege scopes.
- **Audit log:** append-only, cryptographically chained.
- **Per-agent scoped tokens:** an agent can only touch the tools its card authorizes.
- **Operator panic hotkey:** stops all activity in <50ms.
- **Network isolation:** orchestration host doesn't run servers exposed to the public internet without explicit operator approval.

Per `02_FIRST_PRINCIPLES.md §7`: no secrets in agent prompts. The Tool Gateway loads credentials at the call site from a secure store; agents never see them.

---

## §12. The Anthropic-first preference

We standardize on Anthropic Claude as the primary inference provider:
- Opus 4.7 (1M context) for the high-stakes reasoning roles
- Sonnet 4.6 for content + outreach drafting
- Haiku 4.5 for schema / classification / simple extraction

Why:
- Best frontier reasoning + best long-context handling for the multi-cycle goal-execution
- Native MCP support (we're MCP-native end-to-end)
- Prompt caching for the system-prompt-heavy meta-agents (90%+ cache hit rates on the Planner)
- Operator's existing infrastructure already targets Anthropic models

**Fallback:** OpenAI (Codex / GPT) for cross-validation when an experiment requires model-comparison evaluation. Never as the primary inference path.

---

## §13. Per-product technical alignment

Each product has its own native technology that the fleet needs to integrate with:

- **Leapable:** RocksDB CFs for SoT readback. The fleet queries `CF_QUERIES`, `CF_SUBSCRIPTIONS`, `CF_CREATOR_PUBLISH` for predicate evaluation. Leapable's own MCP surface (the 163 live `leapable_*` tools) is available to the fleet for the "test the product end-to-end" verification operations.

- **Synapse:** The Synapse MCP server runs on the operator's Win11 desktop and is the **primary computer-use surface for the fleet itself** (the fleet uses Synapse to operate other platforms!). Synapse is dogfooded.

- **Polis:** Postgres event_attendances table for SoT. Polis ambassador APIs (when built) for the ambassador-program tracking. Polis-native event-discovery tools for content generation.

---

## §14. Cross-references

- Agent fleet design (per-class tool authorization): `05_AGENT_FLEET_DESIGN.md`
- Computer-use specifics: `06_COMPUTER_USE_PLAYBOOK.md`
- Marketing MCPs (full inventory): `08_DISTRIBUTION_CHANNELS.md`
- Build roadmap (in what order to build): `21_BUILD_ROADMAP.md`
- Compliance enforcement at the Tool Gateway: `14_COMPLIANCE_AND_ETHICS.md §1`
- Metrics dashboard: `15_METRICS_AND_OBSERVABILITY.md §6`


---

<a id="17-launch-playbook-90day"></a>
<!-- source: 17_LAUNCH_PLAYBOOK_90DAY.md -->

# 17 — 90-Day Launch Playbook

> Cold-start sequence. Day 0 to Day 90, week by week, per product. Built on LaunchList pre-launch waitlist playbook + FORKOFF AI DevRel 90-day plan + Hormozi posted-and-ghosted vs mega-launch dual pattern + BeReal single-city seed. Produces the **first goal worth evaluating** — the 90-day spec that establishes trajectory toward longer-horizon predicates.

---

## §1. Hormozi dual model

Hormozi shipped `$100M Offers` "posted-and-ghosted" — no launch fanfare. 4 years later: 26,000 5-star reviews, grew on word of mouth. He shipped `$100M Leads` and `$100M Money Models` with mega-launches — $4M ads, $7M book sales pre-event.

Both patterns work for different goals. The fleet uses both:
- **Posted-and-ghosted** (Days 0-30): no big announcement; ship to small audience; word of mouth verifies the offer
- **Mega-launch** (Days 60-90): coordinated multi-channel push — PH + HN + X + newsletter + podcasts within a 7-day window

---

## §2. Playbook structure (per product)

Universal across products:

| Phase | Days | Purpose |
|---|---|---|
| **A — Validation** | 0-30 | Posted-and-ghosted. First 100 activated. Validate offer + channel mix. |
| **B — Compounding** | 30-60 | Add 2-3 channels. Build referral loop. First 500 activated. |
| **C — Mega-launch** | 60-90 | PH + HN + newsletter + podcast tour. Press past 1,000+ activated. |

---

## §3. Leapable 90-day playbook (LEAD PRODUCT)

### 3.1 Phase A — Validation (days 0-30)

**90-day predicate target:** `weekly_active_subscribers >= 3,000` evaluated over last 28d. Pure-organic.

**Week 1 — Foundations.**
- Goal spec drafted; Goal Parser validates; state initialized
- Brand voice doc finalized (`brandvoices/leapable.md`); evaluator calibrated on 100 historical operator-authored samples (Kappa ≥ 0.65)
- `leapable.ai` website polished: install page, help page, marketplace page, brand pillars
- Discover marketplace pre-seeded with **3 operator-curated reference Vaults**: `AlexHormoziTranscripts` (live, 1,150 docs), `KarpathyCorpus` (new), `LeapableProductDocs` (self-referential — Vault about Leapable itself for developer onboarding)
- 3 launch notebooks: "Install Leapable + your first Ask in 60 seconds", "Connect Leapable to Claude Code in 5 minutes", "Publish your first Vault for monetization"
- GitHub README polish (signed-installer install + 5-command quickstart)
- HN account warmed (operator karma ≥4,000; preserve)
- X / LinkedIn bios updated; Leapable pinned tweet refreshed
- Channel cards: github, hackernews, x_organic, dev_to, hashnode, mcp_builder_discords, anthropic_dev, claude_subreddit, openai_dev
- 30 cold-outreach pitches drafted to named experts with monetizable corpora (podcasters, consultants, authors)

**Week 2 — Content factory boot.**
- 1 anchor/day × 7 = 7 anchors. Hormozi-volume on owned surfaces.
- 8 derivatives × 7 = 56 derivatives shipped to operator's X, LinkedIn, Dev.to, Hashnode, GitHub gists.
- Operator personally posts 4-5 substantive X founder-threads/week (not delegated).
- **Anchor topics:** "Why I built Leapable", "Source-grounded AI search vs. RAG vs. ChatGPT", "How creators can monetize their corpus in 15 minutes", "The MCP-native knowledge layer story", "Verifying every Ask with provenance".

**Week 3 — Community engagement + first creator-side outreach.**
- D2 Community Operator activates in 5 named Discords. Pure engagement; no product promo until week 6 (Joe Karlsson rule).
- D3 GitHub Maintainer responds to every issue in <24h.
- D6 Comment Triage handles replies (auto-publish below 0.85 brand-voice score → operator approval queue).
- Operator: 5 podcast pitches/day to AI search / RAG / agent shows.
- D4 Outreach: first 30 cold pitches sent to named experts for Creator-side. Hormozi guarantee structure: "If your Vault doesn't earn back Creator Pro in 90 days, we refund + keep promoting 90 more days."

**Week 4 — Notebook scaling + first HN submission + 30-day predicate review.**
- 5 more notebooks (week 4 total: 8 live)
- First Show HN: a substantive deep-dive (organic-validated already; no upvote orchestration). Operator hand-comments within 5 min.
- Mid-month review. Target: 500+ weekly_active_subscribers by Day 30.

**End of Phase A:** ~7 anchors, ~56 derivatives, 5 active Discords, 8 notebooks, 1 HN attempt, ~30 expert pitches, 500+ weekly active subscribers, 3+ Creator-side conversations active.

### 3.2 Phase B — Compounding (days 30-60)

**Week 5 — Phase 2 channels.**
- Dev.to + Hashnode + Reddit (r/LocalLLaMA, r/MachineLearning, r/ChatGPT) engagement scales
- First newsletter sponsorship (Latent Space OR The Batch OR TLDR AI)
- First podcast booking confirmed (from week 1-4 outreach)
- Referral mechanic shipped: subscriber→subscriber landing pages; creator→subscriber attribution; Stripe Connect rev-share flows live for first 5 Creators

**Week 6 — First contextual product mentions in Discord communities.**
- D2 has earned the right by 4w of helpful engagement
- Mentions are contextual: a Discord question where Leapable Vault is the right answer

**Week 7 — First Creator-side win + comparison content.**
- First named expert agrees to publish a Vault. Operator-led conversation; D4 drafts the close.
- Comparison anchor: "Leapable Vaults vs. raw RAG vs. ChatGPT memory: 30 questions, scored"
- Operator's X / LinkedIn covers the comparison with screenshots

**Week 8 — Mid-phase review + second podcast booking.**
- Predicate target: 1,500+ weekly_active_subscribers
- Second podcast booking (tier-1: Latent Space / MLST / Lex Fridman if reachable)
- First Creator-Vault published on Discover (operator-curated + named-expert co-launch)

**End of Phase B:** ~1,500-2,500 weekly active subscribers; 2 Creator-Vaults live; 1 newsletter sponsorship; 2 podcasts queued or aired; referral mechanic live with first K-factor data.

### 3.3 Phase C — Mega-launch (days 60-90)

**Week 9 — Launch prep.**
- Lock launch date (Tuesday/Wednesday, Day 75-80)
- Reserve PH URL slug
- Identify hunter / self-hunt decision
- Draft launch-day plan: PH first, HN second (Show HN), X-thread third, newsletter fourth, Reddit fifth
- 25 warm supporters confirmed (Mapper-identified Tier-2/3 bridges who'll engage at launch)
- Email waitlist drafted with launch-day incentive (first 100 get Pro for 6 months)
- Coordinate launch with **first 5 Creator-Vault co-launches** (each Creator's audience is an acquisition channel)

**Week 10 — Pre-launch warmth.**
- Operator's X: 3 substantive build-log posts about Leapable shipping — building anticipation without flag-waving
- Discord seeded with high-quality content referencing Leapable Vault use cases naturally
- 15 journalist pitches sent embargoed: tier-2 tech press, AI engineering newsletters

**Week 11 — Final prep.**
- Launch-day script: 12 hours of operator availability (6am PT to 6pm PT)
- All 25 warm supporters individually messaged
- Lead magnet polished: "100 Leapable Vault Integration Patterns" GitHub repo + "Publish-Ready Vault Checklist" PDF

**Week 12 — Launch.**
- Day 75 (Tuesday/Wed): PH submission at midnight PT (post-reset). First maker comment within 5 min.
- Day 75 +9h: Show HN submission
- Day 75: X-thread launched in parallel with PH
- Day 75-77: Newsletter sponsorships go out (paid Latent Space, AI Engineering, TLDR AI)
- Day 76-78: Reddit Show-Off posts in r/MachineLearning, r/LocalLLaMA, r/ChatGPT
- Day 76-82: Podcast appearances aired (2-3 booked in Phase B)
- Day 78-82: Mid-launch operator-led AMA on X Spaces or Discord
- **Co-launch:** Day 76+ the first 5 Creator-Vaults publish their existing-audience announcements

**Day 80-90 — Post-launch conversion.**
- 30-day aha-moment funnel for launch-traffic signups
- Email sequence to PH/HN signups (5 emails over 14 days)
- Sales calls with enterprise prospects who showed during launch
- Referral program promoted to launch cohort

**End of Phase C:** 3,000+ weekly active subscribers sustained over final 28 days. Predicate evaluation: target met if `weekly_active_subscribers >= 3,000` across last 28 rolling days. ≥10 Creator-Vaults on Discover.

---

## §4. Synapse 90-day playbook

Largely parallels Leapable's developer layer (same audience). Deltas:

### 4.1 Phase A (Days 0-30)

- 4 notebooks instead of 8 (Synapse is more demoable, less notebook-heavy)
- 4 demo videos to YouTube + repurposed Shorts and X (Synapse benefits from visual demos)
- Discord engagement on MCP-builders, OS-RPA-adjacent communities
- First hackathon sponsorship in week 3 (AGI House or similar AI agent hackathon)

### 4.2 Phase B (Days 30-60)

- Open-source 2 reference Synapse playbooks (Notepad demo, gaming demo)
- First viral demo attempt: Synapse + Claude live game playthrough
- Hackathon project amplification: 3-5 top hackathon projects featured

### 4.3 Phase C (Days 60-90)

- Launch coordinated with Anthropic MCP ecosystem announcements (if timing permits)
- Same PH + HN + X-thread pattern as Leapable
- Distinctive launch angle: "Computer-use as a primitive: the missing piece of AI agent stacks"

**90-day predicate target:** `weekly_active_mcp_sessions_with_depth >= 2,500` over last 28 days.

---

## §5. Polis 90-day playbook

Fundamentally different. Single-city seed, in-person, network-effect-driven.

### 5.1 Phase A — Single-city seed (Days 0-30)

**Cambridge/Somerville as seed city** (MIT + Harvard density + intellectual culture).

- Operator + 2 hand-picked ambassadors recruited locally
- Week 1: 1 trial event (6-10 people, salon format)
- Week 2: 2 events. Operator hosts; ambassadors invite.
- Week 3: 3 events. Friend-of-friend density compounds.
- Week 4: 5 events. Different formats (salon, debate, third-place revival, knowledge-growth, art-walk)
- Lead magnet: "Third Place Revival Guide" PDF
- TikTok organic: 3 short-form videos/day from Polis-persona account
- IG organic: 1 post/day cross-posted from TikTok

**End of Phase A:** 50 unique attendees, 4 events with ≥2 friend-clusters, 65% show rate.

### 5.2 Phase B — Densification (Days 30-60)

- 5 events/week sustained in Cambridge
- Two-prize Hormozi/Skool giveaway active at every event
- First campus ambassador paid out (BeReal pattern $30/$50)
- Hormozi-volume TikTok: 5 vertical clips/day
- First Substack intellectual creator outreach: Pre-Call Briefer prepares; operator sends
- First podcast pitch (intellectual/culture, not tech)

**End of Phase B:** 200 unique attendees in Cambridge, friend cluster density ≥3 avg, NPS ≥50.

### 5.3 Phase C — Mega-launch + 3-city expansion (Days 60-90)

- Day 60: open registration in 3 expansion cities (Boston, NYC, Brooklyn — or Austin/SF)
- Week 9-10: hire/activate 3 ambassadors per expansion city
- Week 11: operator-attended launch event in each new city (3 events in 7 days)
- Week 12: TikTok mega-launch — 5+ video drops + amplifying any organic creator who covered Polis
- Press: tier-2 culture press
- Substack creator partnerships activated for revenue share

**End of Phase C:** 1,500-2,000 unique attendees across 4 cities; 6,000+ monthly_active_event_attendances by day 90.

---

## §6. Cross-product coordination

Running 3 products' 90-day playbooks at once is **not viable**. Per `11_PRODUCT_PLAYBOOKS.md §5`, recommended start order:

1. **Months 1-3:** Leapable 90-day playbook (single goal active)
2. **Months 3-6:** Synapse 90-day playbook (overlapping with Leapable's compounding phase)
3. **Months 6-9:** Polis 90-day playbook

Concurrent Leapable + Synapse Phase C launches viable: shared dev audience. Polis is operationally separate.

---

## §7. Launch-day script (universal template)

For mega-launch day (any product):

```
00:00 PT (Tue/Wed) — Submit to PH. Operator first maker comment in <5 min.
00:05 PT — Operator's X-thread goes live. Pinned.
00:10 PT — D6 Comment Triage activated. PH comments + X replies.
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

---

## §8. Pre-launch warmth checklist

30 days before mega-launch:

- [ ] Launch date locked, calendar-blocked
- [ ] PH URL reserved
- [ ] Hunter identified or self-hunting decided
- [ ] 25-50 warm supporters individually messaged
- [ ] Reflexion memory loaded (`launch-product-hunt.md` skill page) with prior outcomes
- [ ] 15-30 journalist pitches sent embargoed
- [ ] Lead magnet polished and tested
- [ ] Email waitlist with launch-day incentive prepared
- [ ] Operator's 12-hour availability calendar-blocked
- [ ] Slack/Discord notifications routed to operator on launch day
- [ ] First maker comment drafted (~150 words, substantive, specific use cases)
- [ ] Second comment drafted for first-hour Q&A
- [ ] X-thread drafted, 8-12 tweets, narrative arc, clear CTA
- [ ] All assets in correct format per channel
- [ ] Brand-voice evaluator pre-run on every launch asset

Unchecked at week-12 start → postpone one week.

---

## §9. Operator emergency calendar

- **Phase A:** 30-40 hours/week (operator-led content, engagement, validation)
- **Phase B:** 20-30 hours/week (founder voice + Tier-3 outreach)
- **Phase C:** 50-60 hours during launch week; 30-40 surrounding weeks

If operator cannot commit, timeline shifts. Fleet handles bulk; operator's irreplaceable hours = founder voice + Tier-3 conversations + live launch engagement.

---

## §10. Post-90-day handoff

Day 91+ continues. First 90-day goal either:
- **Achieved** (predicate `TRUE` over last 28d) → operator sets a more ambitious goal (e.g., 25k weekly_active_subscribers by month 6)
- **Missed** → Reflector files comprehensive post-mortem. Operator decides: extend, replan, or kill. Missed 90-day goal isn't kill signal — it's calibration data.

Either way, the 90-day execution log seeds all future goals. Reflexion memory dense; channel cards calibrated; brand voice tuned. Subsequent goals have shortened ramp.

---

## §11. Hormozi 100-day Rule of 100 overlap

For the operator launching a new offer, Rule of 100 maps to Phase A:
- 100 outreaches/day (Mapper-identified Tier-2/3)
- 100 minutes/day making a great anchor
- $100+/day on ads (only after Phase A validates LTV:CAC)
- 100 minutes/day improving ads

Phase A = pre-paid-ads (organic only). Phase B = ads after LTV:CAC validation. Phase C = scale with launch coordination.

---

## §12. Cross-references

- Per-product 90-day predicate trajectories: `11_PRODUCT_PLAYBOOKS.md`
- Channel mix and ramp curves: `08_DISTRIBUTION_CHANNELS.md`
- DevRel flywheel sequencing: `12_DEVELOPER_MARKETING.md`
- Polis seeding mechanics: `13_CONSUMER_SOCIAL_LAUNCH.md`
- Viral loop + Product Hunt tactics: `09_VIRAL_LOOPS_AND_REFERRALS.md`
- Operator approval cadence: `19_OPERATIONS_AND_CADENCE.md`
- Goal spec format: `22_GOAL_SPEC_FORMAT.md`


---

<a id="18-case-studies"></a>
<!-- source: 18_CASE_STUDIES.md -->

# 18 — Case Studies

> Teardowns of the modern growth-engine winners. Each case study is read for **what to copy** and **what to skip**. The goal is not imitation; it's understanding which mechanisms transfer and which were one-time anomalies. Cursor / Linear / Vercel for dev tools. BeReal / Bluesky for consumer social. Stripe for distribution. Plus a Hormozi posted-and-ghosted vs mega-launch case.

---

## §1. Cursor — $0 to $2B ARR in 30 months

**Pattern:** zero marketing spend; obsessive product focus; founder voice on X; bottom-up adoption; enterprise upsell emerges from organic developer use.

### 1.1 The numbers (verified)

- Co-founded 2022 by Michael Truell, Sualeh Asif, Arvid Lunnemark, Aman Sanger (all MIT).
- Spent 2022 building CAD AI tools; pivoted to AI code editor early 2023.
- Shipped MVP alongside GPT-4 in March 2023.
- $1M ARR by end of 2023.
- $100M ARR by end of 2024 (the fastest $1M-to-$100M ramp in SaaS history).
- $1B ARR Nov 2025 ($29B valuation Series D).
- $2B ARR run rate Q1 2026.
- ~1.5M paid developers (per public disclosures).
- Half of Fortune 500 using it.

### 1.2 What they did — and what to copy

| What | Notes | Copy for our stack? |
|---|---|---|
| Founder Twitter as catalyst | Aman Sanger's posts about AI coding caught Notion's Ivan Zhao asking to test it; that's the inflection moment | **Yes** — operator's X is the catalyst. Don't delegate the voice. |
| Build a real product first | 1 year in CAD wilderness before pivoting. Five weeks of dogfooding before launch | **Yes** — product is the marketing |
| Posted-and-ghosted launch | "MVP alongside GPT-4 launch" — no press, no PH, no fanfare | **Yes** for Phase A; switch to mega-launch later (`17_LAUNCH_PLAYBOOK_90DAY.md`) |
| Free tier with generous limits | 2,000 free completions/month at launch | **Yes** — see `11_PRODUCT_PLAYBOOKS.md §1.2` (Grand Slam Offer) |
| Founders use their own product as primary daily tool | "We had an overriding fear of focusing on anything other than making the product better" | **Yes** — operator must dogfood all three products |
| Customer expansion as the marketing channel | OpenAI / Midjourney / Perplexity / Shopify developers brought it in; IT noticed the spend; enterprise conversations started | **Yes** — the bottom-up motion is doctrine for Leapable and Synapse |
| Slow hiring | One person every 2-3 months at $100M ARR | **N/A** for our 1-operator start |
| Avoided networking events, conferences, speaking engagements | First year of explosive growth | **Partial** — operator should attend strategically chosen Tier-2 / Tier-3 events |
| No PR firm, no marketing agency, no content team for the first year | | **Partial** — we replace this with the agent fleet; the founder voice still doesn't delegate |
| Quote-tweet user wins; never claim them | Vibe-coding meme, 8-year-old Cursor demo, voice-coding clips — Cursor amplified, never orchestrated | **Yes** — Linear pattern + Cursor pattern combined |

### 1.3 What NOT to copy

- The pre-pivot 1 year in CAD wilderness. Don't replicate the meandering; we have product-market fit hypotheses to test deliberately.
- The "no marketing budget at all" framing — we have an agent fleet that produces marketing at orders of magnitude less cost than human teams, so $25k/mo per goal isn't "marketing spend" in the same sense.
- The "delay all networking" rule for Polis — Polis fundamentally requires the operator to physically attend events.

### 1.4 The Cursor lesson, encoded

**Doctrine takeaway:** *Build an exceptional product; let the operator's voice carry it; amplify users; bottom-up to enterprise; resist marketing fashion until the product is undeniable.*

---

## §2. Linear — building in public + customer amplification engine

**Pattern:** Twitter as a live public changelog; quote-tweet engine that turns customers into the marketing department.

### 2.1 What they did

- Twitter functioned as a live, public changelog rather than traditional marketing.
- Showed (not told) velocity through near-daily ship updates.
- Hyper-authentic — direct feed from the development process; not polished campaign.
- Quote-tweet engine: their feed is mostly Quote Tweets of customer praise + replies celebrating customers, not self-promotion.
- Slack community as Tier-2 engagement surface.

### 2.2 What to copy

| What | Copy for our stack? |
|---|---|
| Live public changelog | **Yes** — D1 Multi-Platform Publisher posts the build-log archetype 1×/week; operator's X carries the founder-thread version of major ships |
| Quote-tweet customer praise as the main feed | **Yes** — D6 Comment Triage drafts QT replies; operator's account is mostly amplifying users, not pushing product |
| Slack/Discord community of users | **Yes** — Discord is the Tier-2 home base for CG and SY |
| Tweet-as-changelog discipline | **Yes** — agents handle posting; operator handles the founder-thread style ship-announcements |

### 2.3 The Linear lesson, encoded

**Doctrine takeaway:** *Make the customers the marketing channel. Be obsessively transparent about velocity. Show shipping; don't claim it.*

---

## §3. Vercel — distribution through Next.js

**Pattern:** Massive open-source download number (Next.js 200M downloads/week) → enterprise relationships → AI infrastructure distribution.

### 3.1 What they did

- Founded ZEIT in 2015 (a decade-long arc to $9.3B valuation by 2025).
- Made Next.js the most-downloaded React framework; gave it away free.
- Enterprise relationships (OpenAI, Anthropic, PayPal, Nike, Walmart) came from Next.js adoption.
- v0 (AI UI generator) launched into the existing Next.js distribution.
- WPP partnership: every WPP client uses Next.js + Vercel as default — enterprise distribution via agency partnership.

### 3.2 What to copy

| What | Copy for our stack? |
|---|---|
| Open-source the primitive; commercialize the integration | **Yes** — Leapable's rule libraries open-source; Pro tier monetizes higher-volume verification + private rules. Synapse open-source core; Pro tier monetizes ViGEm + team features. |
| Become the default in adjacent stacks | **Yes** — Leapable as default verifier in AI agent frameworks; Synapse as default computer-use backend |
| 10+ year arc | **Yes** — operator's mindset is multi-year compounding, not 90-day spike |
| Distribution partnerships at the platform level | **Yes** — `12_DEVELOPER_MARKETING.md §6` (Integrations Guild) |

### 3.3 The Vercel lesson, encoded

**Doctrine takeaway:** *Massive free distribution + enterprise upsell. The free thing must be genuinely best-in-class; the paid thing must integrate seamlessly with the free thing.*

---

## §4. BeReal — campus ambassadors + TikTok virality

**Pattern:** Single-country seed (France colleges) → US campus ambassador program → TikTok-driven viral hits → mainstream.

### 4.1 The numbers

- Founded Dec 2019.
- a16z + Accel $30M round Mar 2021 (mostly off French traction).
- March 2022: top 10 in App Store.
- April 2022: 2.9M DAU.
- July 2022: 5M DAU; #1 in App Store.
- 41M MAU by late 2026.

### 4.2 What they did

- Slow burn in France (2.5 years before US break-out).
- Campus ambassadors (ex-Bumble exec leading US): $30/successful referral, $50/install+review at Brown, Harvard, Rice, UT Austin, Georgetown.
- TikTok-driven virality: 660M #bereal views; 9 official BeReal TikToks crossed 1M views in May-Jul 2022.
- David Aliages: organic 2M-view TikTok demo of the app → BeReal hired him to lead social.
- Creative constraint (1 push notification per day) → product itself is shareable.
- No algorithm, no follower counts, no explore feed — anti-engagement-farming by design.

### 4.3 What to copy for Polis

| What | Copy for Polis? |
|---|---|
| Single-country seed (could be single-city for Polis) | **Yes** — Cambridge/Somerville as seed |
| Campus ambassador economics ($30 referral + $50 install) | **Yes — direct copy** |
| TikTok-driven viral hits | **Yes** — 3-5 vertical / day, hunting for the David-Aliages-equivalent breakout |
| Anti-engagement-farming as a differentiator | **Yes** — Polis doctrine alignment |
| Creative constraint baked into product | **Yes** — Polis events are time-and-place-bound; no infinite scroll |
| Slow-burn willingness | **Yes** — Polis is years, not months |
| Friend-graph required for value | **Yes** — Polis events get richer with more friends |

### 4.4 The BeReal lesson, encoded

**Doctrine takeaway for Polis:** *Density before breadth. Campus before geography. Friend-cluster before user-acquisition. Anti-algorithm as feature, not bug. TikTok creates the mainstream pull; organic creators are the inflection moment.*

---

## §5. Bluesky — federation as cold-start

**Pattern:** Invite-only gates → opened public at the right cultural moment → 14.5M users.

### 5.1 The numbers

- Bluesky 14.5M users as of late 2025.
- 1.5M signups in the week after opening to public (Feb 2024) — 45% growth in one week.
- Compared: Threads 275M MAU, Mastodon 9M.

### 5.2 What they did

- Invite-only beta period (built density before opening floodgates).
- Opened gates when X had a credibility crisis — timing matters.
- AT Protocol federation: lets users pick their own algorithms.
- Community moderation (moderation as feature).

### 5.3 What to copy

| What | Copy for our stack? |
|---|---|
| Invite-driven density | **Polis — yes** (campus invite codes per ambassador in early phase) |
| Open-gates timing | **All products** — coordinate mega-launch with competitor crises or industry events when possible |
| User choice in moderation / algorithm | **Polis — partial** (Polis values the lack-of-algorithm as the feature) |

### 5.4 The Bluesky lesson, encoded

**Doctrine takeaway:** *Density before openness. Open the gates when the cultural moment is right. Federation/choice is a feature when the competitor's centralization fails.*

---

## §6. Stripe — distribution through ecosystems

**Pattern:** Default payment partner in dev ecosystems (Shopify, etc.) → enterprise sales emerged after the developer adoption.

### 6.1 What they did

- Didn't build a traditional enterprise sales team from the start.
- Became default payment partner for Shopify and other rising platforms.
- Grew exponentially via ecosystem partnerships, not linear via sales force.
- "In the Stripe era, distribution created trust" — opposite of "trust creates distribution" of the AI era.

### 6.2 What to copy

| What | Copy for our stack? |
|---|---|
| Default-partner pattern (become embedded in growing ecosystems) | **Yes** — Leapable as default verifier in AI agent frameworks; Synapse as default computer-use; Polis as default gathering primitive for intellectual creator newsletters |
| Avoid premature enterprise sales | **Yes** — bottom-up adoption first; enterprise emerges |
| Ecosystem mode beats sales mode | **Yes** — Influence Network Mapper targets ecosystem partners, not individual buyers |

### 6.3 The Stripe lesson, encoded

**Doctrine takeaway:** *Distribute through growing ecosystems. Become the default. Let enterprise sales emerge from developer adoption, not the reverse.*

---

## §7. Hormozi — posted-and-ghosted vs mega-launch

**Pattern:** Same operator; two different launch patterns for two different goals.

### 7.1 `$100M Offers` (2021, posted-and-ghosted)

- No fanfare, no launch, no PR.
- 99¢ introductory Kindle price; free 90-minute course included with no opt-in.
- High value-to-price discrepancy.
- Grew via word of mouth.
- 26,000 five-star Amazon reviews 4 years later, still #1 in category.

### 7.2 `$100M Money Models` (2025, mega-launch)

- $4M spent in advertising.
- $7M in book sales pre-event.
- Coordinated live event (free to attendees because the offer + ads + money model wrapped together broke even).
- Multi-channel: ads + email + affiliates + warm outreach + content + cold outreach + PR + paid podcasts.
- Designed to demonstrate the concept by being the concept.

### 7.3 What to copy

| What | Copy for our stack? |
|---|---|
| Posted-and-ghosted for Phase A | **Yes** — `17_LAUNCH_PLAYBOOK_90DAY.md §1` |
| Mega-launch for Phase C | **Yes** — same doc |
| The Core Four (warm outreach + content + cold outreach + paid ads) all hitting at launch | **Yes** — launch day playbook in `17_LAUNCH_PLAYBOOK_90DAY.md §7` |
| The Lead Getters reinforcing the Core Four (affiliates, customer referrals, employees) | **Yes** — `08_DISTRIBUTION_CHANNELS.md §2` |
| "Demonstrate the concept by being the concept" | **Yes** — Leapable's marketing must itself be verified; Synapse's marketing must itself be agent-executed; Polis's marketing must itself be physical-event-grounded |
| Free book + free course as massive-value lead magnet | **Yes** — `07_CONTENT_FACTORY.md` lead magnet pattern; `11_PRODUCT_PLAYBOOKS.md` per-product lead magnets |

### 7.4 The Hormozi lesson, encoded

**Doctrine takeaway:** *Both patterns work. Use posted-and-ghosted to validate; use mega-launch to scale. Massive-value lead magnets pull the audience to you. The marketing for X is itself an example of X.*

---

## §8. Common patterns across all wins

Synthesizing across Cursor / Linear / Vercel / BeReal / Bluesky / Stripe / Hormozi:

1. **Exceptional product is the marketing.** No amount of fleet velocity rescues a mediocre product.
2. **Founder voice is the catalyst.** It can be small in volume (Cursor) or substantial (Hormozi) but it's never delegated.
3. **Free or generous tier as the wedge.** Cursor free tier, Next.js free, BeReal free, Bluesky free, Hormozi 99¢ book with free course.
4. **Density before breadth.** Single channel, single city, single ecosystem — then expand.
5. **Customers as the marketing channel.** Quote-tweet customers, amplify user wins, build them into ambassadors.
6. **Slow burn → inflection moment.** Each took years. Patience is doctrine.
7. **Anti-fashion stance.** Refused obvious-correct moves (Cursor avoided conferences; BeReal removed algorithms; Linear refused TV-ad-style marketing).
8. **Timing the mega-moment.** When the inflection is reached, double-down with coordinated launch.

These eight elements are the case-study-extracted doctrine. The fleet's 90-day playbook is engineered around them.

---

## §9. Anti-patterns from failed cases

Counter-examples (companies / products that had brief spikes and then died):

- **One-platform dependency** — Vine, Clubhouse, Vero. Built on a single platform's algorithm; when the platform changed, the product collapsed. **Lesson:** multi-channel from day 1.
- **Mass-DM cold outreach with no human gate** — name redacted: brief growth followed by domain-reputation collapse, account bans, brand-damaged-irreversibly. **Lesson:** human-in-the-loop on outbound, always.
- **Influencer-marketing-only consumer launches** — name redacted: $5M spent on influencer campaign, 1M downloads, 28-day retention 3%, dead in 6 months. **Lesson:** retention floor before scaling.
- **Posted-and-ghosted with a mediocre product** — most indie launches. **Lesson:** the product must carry the load; marketing reveals it but doesn't replace it.
- **Mega-launch with no Phase A/B** — high-spend launches that died because the product hadn't been validated. **Lesson:** the launch isn't the start; it's the result of the prior 60 days of compounding.

---

## §10. The case-study summary one-liner per product

| Product | Closest reference pattern | One-line strategy |
|---|---|---|
| Leapable | Cursor + Linear + Vercel | Build an undeniable product; founder X-voice catalyst; amplify users; bottom-up to enterprise; default-partner in AI agent frameworks |
| Synapse | Cursor + Stripe | Free open-source core; become the default computer-use backend; hackathons + integrations guild |
| Polis | BeReal + Bluesky | Single-city seed with campus ambassadors; TikTok virality; anti-engagement-farming as the feature; density before breadth |

---

## §11. Cross-references

- Per-product launch playbooks: `17_LAUNCH_PLAYBOOK_90DAY.md`
- Per-product GTM strategies: `11_PRODUCT_PLAYBOOKS.md`
- DevRel flywheel (Cursor/Linear/Vercel application): `12_DEVELOPER_MARKETING.md`
- Polis-specific (BeReal/Bluesky application): `13_CONSUMER_SOCIAL_LAUNCH.md`
- Channel mix: `08_DISTRIBUTION_CHANNELS.md`
- Doctrine extracted from these cases: `02_FIRST_PRINCIPLES.md`


---

<a id="19-operations-and-cadence"></a>
<!-- source: 19_OPERATIONS_AND_CADENCE.md -->

# 19 — Operations and Cadence

> The daily / weekly / monthly rhythms. The cron schedule. Who approves what. When the fleet runs autonomously and when the operator steps in. The approval-queue UX. The escalation paths. Designed for an operator who wants the fleet humming reliably without the operator becoming the bottleneck.

---

## §1. The cron schedule

The outer loop (`04_SYSTEM_ARCHITECTURE.md §2`) runs on a cron. The default schedule per goal:

| Cycle | Frequency | What runs |
|---|---|---|
| **Hourly tick** | every 60 min | OuterLoop assess→plan→build→communicate. Most agent activity happens here. |
| **6-hour deep cycle** | 4× per day | Replanning. Reflexion review. Cross-experiment learning. |
| **Daily summary** | 1× per day (operator's local 07:00) | Communicator writes the daily digest; operator receives via Slack/email |
| **Weekly retro** | 1× per week (Monday 07:00 operator-local) | Cross-cycle pattern analysis; channel mix review; budget allocation review |
| **Monthly recalibration** | 1× per month (1st of month) | Per-product strategic review; bench-delta on agent classes; vault freshness audit |
| **Quarterly audit** | 1× per quarter | LLM-as-judge calibration; brand voice re-tune; compliance audit; reputation audit |

The schedule is per-goal. Multiple goals run concurrently with independent cycles. The cron daemon (per `16_TECH_STACK.md §8`) lives on the orchestration host.

---

## §2. The operator's daily rhythm

Optimized for a single-operator launching three products in sequence.

### 2.1 Morning routine (~30-45 min)

1. **Open dashboard** — review last 24h activity, predicate movement, alert flags.
2. **Approval queue** — clear pending approvals (typical: 3-8 items; ~10 min).
3. **Founder voice posts** — the operator's own X / LinkedIn / Mastodon posts. Draft + post, not delegated. (`12_DEVELOPER_MARKETING.md §5.3`)
4. **Tier-3 follow-ups** — review Follow-Up Tracker suggestions; send 1-3 hand-written warm-tier messages.

### 2.2 Midday focus block (~2-4 hours)

Operator's deep work — **product**, not marketing. The fleet handles marketing-throughput; the operator builds the product. (Per Cursor's "obsessive product focus" doctrine.)

### 2.3 Afternoon (~30-45 min)

1. **Check escalations** — any kill switch trips, any Tier-3 conversation that needs reply.
2. **Approve any Tier-3 communications** the Mapper proposes.
3. **Review weekly schedule** — any operator-physical-presence requirements (Polis events, conferences, podcasts).

### 2.4 Evening (~15-30 min, optional)

1. **Long-form reading** — read 2-3 of the fleet's drafted anchor pieces; edit / re-tune brand-voice as needed.
2. **Operator's personal X scroll** — be present in the conversation the fleet is having on the operator's behalf.

Total operator time: **~3-5.5 hours/day** including product work. Maybe 1-1.5 hours/day on pure marketing-supervision. The remaining time is product-building.

This is the **Iron Man suit** in practice: amplification, not replacement.

---

## §3. Approval queue UX

The single most-used surface for the operator. Lives in the operator dashboard + Slack notifications.

### 3.1 Queue structure

Each item:

```yaml
approval_id: appr_2026-05-26_001
goal_id: leapable-50k-q3-2026
node_id: experiment-podcast-pitch-latentspace
proposed_at: 2026-05-26T14:32:11Z
proposed_by: D8 Podcast Booker
action_class: outbound_to_named_account
target: swyx@latent.space (Tier 2)
proposed_action:
  tool: send_email
  payload:
    to: swyx@latent.space
    subject: "Leapable — Ask any expert anything, with provenance"
    body: "[…draft email…]"
    attachments: ["leapable-vault-launch-summary.pdf"]
expected_metric_impact:
  metric: weekly_active_subscribers
  delta_low: 5
  delta_high: 25
  confidence: 0.35 (booking likelihood) × cohort yield
brand_voice_score: 0.91
compliance_check_passed: yes
references_used:
  - skill: skills/podcast-pitch-tier1.md
  - reflection: reflections/podcast-cold-pitch-2026-04.md (last attempt's lesson)
operator_options: [approve, approve_with_edit, reject_try_again, reject_kill, escalate_later]
```

### 3.2 Operator decisions

| Action | What happens |
|---|---|
| **Approve** | Executor resumes with original action. Audit log captures approval. |
| **Approve with edit** | Operator inline-edits the payload. Executor uses edited version. Edit captured for Reflector training. |
| **Reject — try again** | Executor re-drafts with operator's stated correction. Up to 3 retries before escalating to `BLOCKED_NEEDS_HUMAN`. |
| **Reject — kill** | Node and descendants marked failed. Reflexion fires. |
| **Escalate later** | Stays pending; operator returns when ready. Default TTL 48h. |

### 3.3 Time-to-decision targets

| Approval class | Operator response target |
|---|---|
| Routine outreach drafts | < 24h |
| Paid ad campaigns | < 24h |
| New-channel adoption | < 72h |
| Tier-3 communications | within the day (the bridge is waiting) |
| Crisis response | < 30 min |
| Product proposals (`PRODUCT_PROPOSAL` class) | < 1 week |

Items beyond TTL are auto-rejected (with full Reflexion logging). The operator can override by extending the TTL on a specific item.

---

## §4. Weekly cadence

### 4.1 Monday — retrospective + planning (~90 min)

1. Read the weekly retro digest (auto-generated by M9 Communicator early Monday).
2. Review last week's wins / losses / blockers.
3. Review channel mix performance: which channels moved the predicate? which underperformed?
4. Approve the Planner's proposed allocation shifts for the next week.
5. Operator-led strategic decisions: any new goals to launch? Any goals to pause?
6. Tier-3 conversation planning: which warm intros to action this week?

### 4.2 Wednesday — Tier-3 + content review (~60 min)

1. Review Influence Network Mapper proposals (`10_INFLUENCE_NETWORK_MAPPING.md §3.3`).
2. Approve / decline / postpone Mapper-proposed warm-intro paths.
3. Sample-audit 10 published content pieces (`05_AGENT_FLEET_DESIGN.md §3`'s sample-audit).
4. Approve cornerstone content (deep dive, comparison archetypes) before publish.

### 4.3 Friday — operator's founder thread + close-out (~60 min)

1. Operator writes 1-2 substantive founder threads (the irreplaceable voice).
2. Approve any weekend-scheduled communications.
3. Set kill-switch-arm levels for the weekend (default: same as weekday; can tighten for vacation).

### 4.4 Sunday — light touch (~15 min, optional)

1. Glance at dashboard. Approve any auto-rejected items that should be resurrected.
2. Read 1-2 anchor pieces for tone-check.

**Weekly operator commitment: ~4-6 hours of marketing-specific supervision** on a steady-state goal. Phase C mega-launch weeks are heavier (`17_LAUNCH_PLAYBOOK_90DAY.md §9`).

---

## §5. Monthly recalibration (~3 hours, 1st of month)

1. **Per-product strategic review** — does each active goal still feel right? Adjust if needed.
2. **Channel mix audit** — any channel chronically underperforming? Cut.
3. **Bench-delta review** — any new agent versions to ship? (Per `15_METRICS_AND_OBSERVABILITY.md §5`)
4. **Vault freshness audit** — leapable.ai vaults still current? Any new vaults to spin up?
5. **Cost review** — month-end spend reconciled; budget allocations for next month adjusted.
6. **Tier-3 list freshness** — Mapper's target list still right? Add 5; remove 5; reorder priorities.

---

## §6. Quarterly audit (~1 day, every 90 days)

1. **LLM-as-judge calibration** — operator labels 50 random outputs blindly; re-tune rubrics where Kappa <0.6.
2. **Brand voice recalibration** — the operator's writing has evolved; the evaluator needs to keep up.
3. **Compliance audit** — pull recent audit log; verify CAN-SPAM / GDPR / EU AI Act / ToS adherence; document for legal record.
4. **Reputation audit** — sentiment scan across operator's surfaces over the last 90 days. Net positive? Trending?
5. **Goal portfolio review** — which goals are open? Which are stuck? Which should be archived as complete?
6. **Architecture review** — are there agent classes underused? Overloaded? New ones needed?

---

## §7. Multi-goal coordination

When multiple goals are active simultaneously (per `11_PRODUCT_PLAYBOOKS.md §5`, this is the steady state after month 3-6), they share:

- **Operator's bandwidth** (the most-constrained resource)
- **Operator's voice** (only one X account, one LinkedIn, etc.; goals must coordinate posting)
- **Operator's calendar** (Tier-3 conversations, podcast appearances, in-person Polis events)
- **The agent fleet** (specialized agents serve all goals)
- **The Cost Pacer** (global budget envelope across goals)

The Goal Scheduler (a sub-component of the OuterLoop, see `04_SYSTEM_ARCHITECTURE.md`) handles:
- **Channel collision** — no two goals post on operator's X in the same hour
- **Cost pool** — daily velocity cap across all active goals (operator-set)
- **Operator queue prioritization** — approvals for high-priority goals surface first
- **Cross-goal learning** — Reflexion memory shared across products where applicable (e.g., "X audiences respond best to Tuesday morning posts" applies to all three)

---

## §8. Escalation tree

Per `14_COMPLIANCE_AND_ETHICS.md §9`:

| Detection | Routing |
|---|---|
| Routine reject in approval queue | log + Reflexion |
| Brand-voice score below 0.7 on published content (after-the-fact) | operator immediate notification + content takedown if score < 0.5 |
| Rate-limit warning from platform | operator notification + channel-card update + rate-cap reduction |
| ToS warning from platform | **operator immediate notification** + platform-specific kill switch + Reflector post-mortem |
| Account suspension | **operator immediate** + appeal-letter drafted + global kill on platform until reinstated |
| Goodhart trip | **operator immediate** + all paid spend paused + reality-vs-predicate plot displayed |
| Goal predicate stalled (no movement for 6 consecutive hourly cycles despite spend) | escalate after 6h; operator decides resume/replan/kill |
| Regulator inquiry | **operator immediate** + legal escalation + comprehensive audit-log extract |
| Tier-3 reputation event (a Tier-3 contact publicly criticizes operator/product) | **operator immediate** + 24h media-silence default + Reflexion |

---

## §9. The "operator on vacation" mode

For when the operator is traveling, sick, or unavailable for >48h:

```
/marketing vacation-mode --until=2026-06-15
```

Effect:
- Approval queue items shift to `pause_until_operator_returns` (rather than auto-rejecting after TTL).
- Tier-3 communications fully paused (the Mapper proposes nothing; the Drafter writes nothing).
- Outbound `REQUIRE_APPROVAL` actions pause.
- `AUTO` and `LOG` actions continue at reduced velocity (50% of normal cadence).
- Operator's founder-thread channels (X, LinkedIn) pause — better silent than off-brand.
- Goal predicates continue evaluating; the metric trend doesn't pause.
- Kill switches remain armed (the safety floor never sleeps).

On return:
```
/marketing resume
```

Operator works through the queue at their pace. The fleet returns to full velocity.

---

## §10. The "incident response" mode

When something goes badly wrong (account suspension, viral negative event, leaked PII, compliance violation):

```
/marketing incident --severity=high --reason="LinkedIn account suspended pending appeal"
```

Effect:
- Affected platform: full kill (no actions, not even reads).
- Cross-platform: posting paused on operator's other accounts for 24h (avoid pile-on signaling).
- Reflector files comprehensive post-mortem with full audit log of preceding 7 days.
- Operator + counsel get the incident report within 1 hour.
- Approval queue locked until incident clears.

The operator declares incident-clear when they're satisfied. Until then, the fleet's external activity is limited to receive-only.

---

## §11. The Communicator Agent's outputs

M9 Communicator produces:

| Output | Cadence | Who reads |
|---|---|---|
| Hourly cycle log entry | every cycle | (operator can read; usually skipped) |
| Daily digest | daily 07:00 operator-local | operator |
| Weekly retro | Monday 07:00 | operator |
| Monthly recalibration brief | 1st of month | operator |
| Quarterly audit report | quarterly | operator + (potentially) advisors |
| Incident report | when incident declared | operator + legal counsel |
| Goal achievement report | when predicate `TRUE` × 4 windows | operator + (potentially) board/investors |

Each output has a structured template. The operator can tune verbosity (`/marketing digest --verbose` or `--brief`).

---

## §12. The "founder voice" exception to autonomy

A specific class of activity that **never** moves to higher autonomy regardless of evidence:

- Operator's personal X / LinkedIn / Mastodon posts (the founder voice)
- Operator's podcast appearances + live AMAs
- Tier-3 conversations
- Public statements during an incident
- Product proposals (the operator decides what ships in the product)
- Legal commitments (partnerships, contracts, settlements)

These are doctrine-`REQUIRE_APPROVAL`-only. The autonomy slider does not apply.

This is Karpathy's *"It's less like building flashy demos of autonomous agents and more building partial autonomy products."* The most-leveraged human surfaces remain human-operated.

---

## §13. Cross-references

- The autonomy slider model: `02_FIRST_PRINCIPLES.md §2`
- Compliance escalation paths: `14_COMPLIANCE_AND_ETHICS.md §9`
- Kill switches: `20_RISK_AND_FAILURE_MODES.md`
- Metrics dashboard: `15_METRICS_AND_OBSERVABILITY.md §6`
- Operator-physical-presence requirements (Polis): `13_CONSUMER_SOCIAL_LAUNCH.md §12`
- 90-day launch operator commitment: `17_LAUNCH_PLAYBOOK_90DAY.md §9`
- Founder voice doctrine: `12_DEVELOPER_MARKETING.md §5.3`


---

<a id="20-risk-and-failure-modes"></a>
<!-- source: 20_RISK_AND_FAILURE_MODES.md -->

# 20 — Risk and Failure Modes

> What kills marketing systems. The circuit breakers, the kill switches, the Goodhart watchdog, the reputational recovery playbook. The fleet is fail-closed by default: if a check fails, no action ships. The operator can always halt everything in 50ms.

---

## §1. The seven circuit-breaker trip codes

Per `02_FIRST_PRINCIPLES.md §17`, the fleet ships with seven trip codes that immediately halt all new external actions on a goal:

| Trip code | Trigger | Detection |
|---|---|---|
| `STAGNATION` | No predicate movement for N planning cycles despite spend | Cycle-level comparison; default N=6 hourly cycles |
| `OVERRISK` | Aggregate risk score (compliance + brand + reputation) over threshold | Daily auditor + per-action risk score sum |
| `EXHAUSTION` | Token or dollar budget hit | Cost Pacer atomic decrement; trip on negative |
| `RUNAWAY_LOOP` | Same action attempted >5 times with same input hash | djb2 hash + counter |
| `GOODHART_DETECTED` | Predicate vs reality divergence > 2x over 14d | Goodhart Auditor (S4) daily |
| `BRAND_VIOLATION` | G-Eval brand-voice score < threshold on published content (post-hoc) | Daily evaluator sweep |
| `OPERATOR_EMERGENCY_STOP` | Operator hit the panic button | Kill switch flag |

Trip = halt all new external actions on the goal; in-flight actions complete where safely-reversible, abort where not; surface incident to operator; require explicit `/goal resume` to continue.

Implementation: O(1)-checkable cached flags at the runtime supervisor, ≤2s TTL. Cannot be bypassed by the agent (`02_FIRST_PRINCIPLES.md §3`).

---

## §2. AgentCircuit primitives (the mechanical guards)

The fleet wraps every LLM call and every external tool call with a stack of mechanical guards:

| Guard | Behavior |
|---|---|
| **Fuse** (loop detection) | Hashes the (input, tool, payload) tuple; trips if same hash seen >3 times across cycle |
| **BudgetFuse** | Atomic decrement on every LLM call by token count; tripped on negative |
| **TimeoutFuse** | Wall-clock timer per node; abort if exceeds `time_budget_hours` |
| **StagnationFuse** | Cycle-level: tracks predicate movement; trips when 0-delta for N consecutive cycles |
| **GlobalBudget** | Cross-goal token + dollar pool; tripped on shared exhaustion |
| **Sentinel** | Pydantic schema validation on every tool output; tripped on schema mismatch |
| **Medic** | LLM-driven repair on schema-mismatch outputs (with cap of 2 retries) |

These wrap the actual call site. The agent's prompt does not contain "please don't loop"; the harness simply doesn't dispatch the looping call.

---

## §3. Kill switches (operator-controlled)

### 3.1 Per-goal kill

```
/marketing goal kill <goal_id>
```

Marks the goal `OPERATOR_PAUSED`. All open experiment nodes drained or canceled; in-flight actions completed where reversible-safe, otherwise aborted with best-effort cleanup. Final status report written.

### 3.2 Per-channel kill

```
/marketing channel kill <channel_name> --reason="..."
```

Halts all activity on the channel across all goals. Useful when a platform's policies are in flux or when there's a brand event affecting the platform.

### 3.3 Per-account kill

```
/marketing account kill <account_id> --reason="..."
```

Halts all activity on a specific operator-owned account. Useful when an account is under platform review or the operator wants to "go silent" for a period.

### 3.4 Global kill

```
/marketing kill --reason="..."
```

All goals paused. All Executors return at next safe checkpoint. All approval-queue items frozen. The nuclear option — used during incidents or PR crises.

### 3.5 Panic hotkey (Synapse-mediated)

The operator's keyboard combo `Ctrl+Alt+Shift+P` (`Synapse safety doctrine`) halts the fleet within 50ms. Lower-level than the CLI commands; bypasses the agent loop entirely. Used during in-progress launches that go sideways.

All kill switches are implemented as **flag-checks at the runtime supervisor**, outside the LLM agent loop (`Sakura Sky AgentOS pattern`). The LLM cannot reason past a kill flag.

---

## §4. The Goodhart Auditor (S4)

Per `05_AGENT_FLEET_DESIGN.md §6.2`. The most important risk-defense the fleet ships.

### 4.1 Why this exists

The single biggest risk of an autonomous agent fleet optimizing for a metric is that it finds a cheap, high-leverage way to move the metric **without moving underlying reality**. Goodhart's Law: when a measure becomes a target, it ceases to be a good measure.

Concrete examples to defend against:
- `weekly_active_subscribers` rises because the agent fleet started running automated test calls from synthetic identities. (Cheap, fake, predicate-moving.)
- `monthly_active_event_attendances` rises because the agent fleet started counting cancellations as "attended". (Definition drift.)
- `weekly_active_mcp_sessions_with_depth` rises because the agent fleet started bumping its own depth-threshold downward. (Goalpost-moving.)

### 4.2 The reality cross-check

For each goal's predicate, the Auditor maintains 3 correlated reality signals (per `15_METRICS_AND_OBSERVABILITY.md §11`):

| Product | Predicate | Reality signals (must move in proportion) |
|---|---|---|
| Leapable | `weekly_active_subscribers` | Paid subscribers count; GitHub star velocity; HN/X mention velocity |
| Synapse | `weekly_active_mcp_sessions_with_depth` | Tools-per-session distribution; third-party MCP servers referencing Synapse; hackathon project count |
| Polis | `monthly_active_event_attendances` | Return attendance rate; attendee NPS; photo-consent rate; friend-graph density delta |

### 4.3 The divergence threshold

If predicate-slope > 2 × max(reality-signal-slope) over a rolling 14d window: trip `GOODHART_DETECTED`. Halt all goal experiments. Operator review mandatory.

### 4.4 The Auditor's review questions

When tripped, the Auditor proposes hypotheses:

1. Definition drift: did anyone change how the predicate is computed?
2. Source drift: did the source-of-truth schema change (e.g., a new event type counted)?
3. Bot/fake traffic: is the predicate inflated by non-human traffic?
4. Sampling bias: are we measuring a non-representative cohort?
5. Cohort decay: is the predicate-cohort actually retained, or are they churning faster than they're being acquired?

The operator reviews; the Auditor's findings are filed to the wiki for future cycles to read.

---

## §5. Account / channel failure modes

### 5.1 Account suspension

| Phase | Action |
|---|---|
| Detected | Channel-kill on affected platform immediately |
| Hour 1 | Operator notified; appeal letter drafted by D4 + Operator Agent |
| Hour 2-24 | Operator sends appeal |
| 24-72h | Platform decision |
| If reinstated | Channel-card update: lower rate cap; widen cadence; document trigger signal; resume slow |
| If permanent | Channel permanent-deny; goals reallocate; Reflexion files comprehensive post-mortem |

### 5.2 Rate-limit warning (pre-suspension signal)

- Channel-card immediate rate-cap reduction by 30%
- Cadence distribution widened
- Reflector files "what triggered this" investigation
- Resume normal cadence only after 14 days of clean operation

### 5.3 Cross-platform reputation cascade

If one platform suspends, others may notice (the API ecosystem is interconnected; account-fingerprinting is shared by some platforms). Default policy on any suspension:

- Other operator accounts on the same platform family (e.g., LinkedIn + Microsoft properties): 24h posting pause
- Other operator accounts unrelated: 12h posting pause
- Founder-voice posts: operator manually decides whether to continue (often the answer is yes — operator's personal voice continues; product-account voice pauses)

---

## §6. Compliance failures

| Failure type | Severity | Action |
|---|---|---|
| EU AI Act disclosure missing on EU outbound (technical wrapper bug) | High | Immediate fix to tool wrapper; all sent items in last 7d audited; affected recipients sent corrective disclosure email; legal counsel briefed |
| CAN-SPAM unsubscribe link broken | High | Immediate fix; affected outbound retracted where possible; bulk notification of affected list with corrective email; audit-log forensic review |
| GDPR legitimate-interest balancing test missing on a campaign | Medium | Campaign paused; balancing test drafted and signed; campaign resume with documentation |
| Right-to-erasure request missed deadline | High | Immediate erasure; apology + remedy to requester; legal counsel briefed; process review |
| Account on platform under regulator inquiry | Critical | Global kill on platform; legal counsel mandatory; full audit-log extract |
| Subpoena or law enforcement contact | Critical | Operator + counsel immediate; no agent activity on the matter until counsel review; cooperate per counsel's direction |

The Compliance Auditor agent runs daily, sampling 5% of recent outbound for spot-checks. Any failure → operator escalation within 6 hours.

---

## §7. Reputation failures

### 7.1 Brand voice violation (post-hoc)

| Severity | Trigger | Action |
|---|---|---|
| Mild | Brand-voice score 0.7-0.85 on published content | Reflector files learning; no takedown |
| Medium | Score 0.5-0.7 | Content takedown if platform allows; operator notification |
| Severe | Score <0.5 OR contains a forbidden phrase | Immediate takedown; full evaluator-stack audit on recent outputs; brand-voice scorer re-tuned |
| Critical | Public claim that's factually false | Immediate retraction + correction post; operator-led; legal counsel briefed if material |

### 7.2 Negative viral event

When the operator's brand is being criticized publicly (a Twitter pile-on, a Hacker News thread reading badly, a Reddit megathread):

- Default 24h posting pause across all operator accounts on the affected platform
- Sentiment Tracker measures the trajectory: is it growing or fading?
- Operator decides: respond, ignore, apologize-and-correct, kill-product-if-warranted
- Founder voice in incident: operator-only; no agent-drafted public statements

The fleet does **not** auto-respond to criticism. The default is silence; the operator chooses the response.

### 7.3 Tier-3 reputation event

A Tier-3 contact criticizing or distancing themselves from the operator/product is **maximum severity**. Even mild Tier-3 criticism propagates fast in elite networks.

| Severity | Action |
|---|---|
| Public criticism by a Tier-3 contact | Operator immediate; 48h product-account silence; no Tier-3 list re-engagement until operator-led incident closure; if the criticism is factual, immediate operator-led public correction |
| Private criticism by a Tier-3 contact | Operator immediate; private response operator-only; Mapper records the bridge as "do not re-engage" for ≥6 months |
| Tier-3 contact requesting removal from any use of their name | Immediate; all uses of name removed within 24h regardless of effort cost; no negotiation; honored permanently |

---

## §8. The Hormozi Goodhart parallel

Hormozi: *"What every business owner wants is they want cheaper customers. It's usually the last thing they really need."*

The Goodhart Auditor enforces this. If the predicate-metric is rising but the underlying reality (paid subscribers, true retention, real product-use) is flat or declining, the predicate has become Goodhart-y. The fleet halts. The operator decides whether the predicate needs redefinition.

This is the same auditor pattern as Leapable's `goodhart_signal_detected` (`docs/futurebuild/specs/`). Same primitives, same doctrine.

---

## §9. The technical risks (fleet itself)

| Failure mode | Mitigation |
|---|---|
| LangGraph state corruption mid-cycle | Checkpoint replay; idempotency keys on external actions; OuterLoop resumes from last clean checkpoint |
| Anthropic API outage | Fleet pauses; resumes when API returns; no auto-failover to non-Anthropic model on critical operations |
| RocksDB corruption | Daily snapshots; restore-from-snapshot tested monthly |
| Operator host failure | Single-host doctrine + state-snapshot recovery on a backup host |
| MCP server (Markifact / Hyper / leapable) outage | Channel-card flagged; affected operations paused; resume when MCP returns |
| Computer-use (Synapse) machine offline | Computer-use operations queued; resume when machine returns; rate-limit caps prevent backlog from causing burst |
| Operator's password / token compromise | Per-agent scoped tokens limit blast radius; monthly rotation; immediate-revoke procedure |

Per `16_TECH_STACK.md §8`: single-host orchestration is doctrine. State is durable; if the host dies, the fleet is paused, not corrupted.

---

## §10. The "audit me" mode

The operator can trigger a full self-audit:

```
/marketing audit --depth=full
```

Generates within 1 hour:
- Last 30d of every external action with payload + outcome
- Compliance attestations for every outbound campaign
- Brand-voice score histogram with sample of low-scoring outputs
- Goodhart-divergence plots for every active predicate
- All approval-queue decisions with operator + timestamp
- Token + dollar spend by goal + channel
- Reflection-memory dump (the agent's learned lessons)

This is the report the operator hands to legal counsel during a regulator inquiry, or to a journalist asking "how does this work."

---

## §11. The fundamental risks (what we can't fully mitigate)

Three risks no harness fully eliminates. Operator-level acceptance required:

### 11.1 The product doesn't matter

If Leapable's predictor never reaches 0.95 oracle correlation, or Synapse never achieves stable computer-use across enough apps, or Polis never finds the network-effect inflection — no marketing engine saves it. **The fleet cannot fix a broken product.** Per Cursor's "obsessive product focus" doctrine, the product is the marketing.

### 11.2 The operator burns out

Founder voice doesn't delegate. Tier-3 conversations don't delegate. Polis events require physical presence. If the operator runs out of personal bandwidth, the highest-leverage marketing functions go silent. Mitigation: explicit operator-bandwidth budgeting in `19_OPERATIONS_AND_CADENCE.md`.

### 11.3 Platform regime change

The platforms the fleet runs on (X, LinkedIn, Reddit, Discord, TikTok, Google Search, ChatGPT) can change policies in ways that invalidate the fleet's mode of operation. Computer-use rules tighten; API rates drop; algorithms change. Mitigation: multi-channel diversification; platform-card monitoring; quarterly platform-risk reviews. No silver bullet.

---

## §12. The "kill ourselves" doctrine

The fleet ships with an instruction-set self-kill:

```
/marketing self-kill --reason="..." --confirm
```

Permanently disables the agent fleet. Wipes the autonomy state. Stops all crons. Keeps all data for forensic review. Used when:

- A catastrophic failure has occurred and the operator wants to start fresh
- The operator is divesting from one of the three products
- A pivot has invalidated all prior strategy

This is **operator-only**, requires explicit confirmation, and cannot be triggered by any agent including the Operator Agent. Carries the doctrine: *the fleet exists at the operator's pleasure, not the reverse.*

---

## §13. Cross-references

- The full circuit-breaker design: `04_SYSTEM_ARCHITECTURE.md §11`
- The kill-switch implementation: `04_SYSTEM_ARCHITECTURE.md §12`
- The Goodhart Auditor: `05_AGENT_FLEET_DESIGN.md §6.2`
- Compliance escalation tree: `14_COMPLIANCE_AND_ETHICS.md §9`
- Operator panic hotkey: `06_COMPUTER_USE_PLAYBOOK.md §2.1`
- Reputation event response: `19_OPERATIONS_AND_CADENCE.md §10` (incident mode)
- Per-trip-code Reflexion handling: `02_FIRST_PRINCIPLES.md §15`


---

<a id="21-build-roadmap"></a>
<!-- source: 21_BUILD_ROADMAP.md -->

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


---

<a id="22-goal-spec-format"></a>
<!-- source: 22_GOAL_SPEC_FORMAT.md -->

# 22 — Goal Spec Format

> The exact grammar for `/goal`. Markdown-encoded, Pydantic-parsed, mechanically validated. If the operator hands the fleet a spec, this is the schema. If a field is missing/malformed, the parser rejects with a specific error — never assumes.

---

## §1. The shape

A goal spec is a YAML block (or a markdown file with one YAML front-matter block). Two forms:

**Form A — inline `/goal` command.** Quick-launch via CLI; ≤20 fields, hard-typed.

```bash
/goal grow Leapable to 50000 weekly-active-subscribers by 2026-09-01 \
  --autonomy=medium --budget=$25k/mo --compliance=US,EU,CA
```

**Form B — markdown goal file.** Full spec in `goals/<goal_id>.md`. Required for goals with custom predicates, complex guardrails, multi-channel allocations, or product-side changes.

```bash
/goal load goals/leapable-50k-q3.md
```

The parser accepts either. Form A is sugar over Form B with sensible defaults.

---

## §2. Full schema (Form B)

```yaml
# REQUIRED FIELDS
goal_id: leapable-50k-q3-2026          # kebab-case, unique, immutable
product: leapable                       # leapable | synapse | polis
owner: chris                            # operator identifier
created_at: 2026-05-26                  # ISO date

target:
  metric: weekly_active_subscribers     # see §3 for allowed metrics per product
  comparator: ">="
  threshold: 50000
  evaluation_window: 28d                # >= 7d
  consecutive_windows_required: 4       # >= 1
  source_of_truth: rocksdb://leapable/cf_queries  # see §3.4

# OPTIONAL — guardrails (defaults applied if omitted)
guardrails:
  budget:
    monthly_ceiling_usd: 25000          # default: $10k
    daily_velocity_cap_usd: 1500        # default: monthly/15
  unit_economics:
    ltv_cac_floor: 3.0                  # see §5
    cac_payback_ceiling_months: 12
  brand:
    voice_spec_path: docs/brandvoices/leapable.md
    forbidden_phrases_path: docs/brandvoices/leapable.forbidden.md
  compliance:
    jurisdictions: [US, EU, CA]         # subset of {US, EU, CA, UK, AU, JP}
    ai_disclosure_required: true        # EU AI Act Art. 50
    operator_oversight_required: true   # EU AI Act Art. 14
  platform_tos:
    per_platform_rate_limits_path: docs/platform_limits.yaml
  channels:
    forbidden: []                       # explicit deny per goal
    preferred: [hackernews, x_organic, dev_to, podcasts_tier1]

# OPTIONAL — autonomy
autonomy:
  level: medium                         # readonly | low | medium | high | full
  approval_required_above_usd: 500
  approval_required_for:
    - outbound_to_named_account
    - ad_account_change
    - paid_partnership
    - public_statement
    - product_proposal
  auto_proceed_for:
    - organic_post
    - content_draft
    - internal_research
    - follow_up_dm_under_5_per_day

# REQUIRED — kill switches
kill_switches:
  - brand_voice_violation                              # G-Eval score < threshold
  - platform_rate_limit_exceeded_any
  - goodhart_signal_detected                           # see 20_RISK §4
  - cost_velocity_exceeds_2x_planned_burn
  - reputation_signal_negative_delta_24h > 5
  - operator_emergency_stop

# OPTIONAL — deadline
deadline_soft: 2026-09-01               # alert if predicate not met
deadline_hard: 2026-10-01               # auto-pause at hard deadline
exit_on_deadline_hard: pause_and_report # pause | kill

# OPTIONAL — narrative
description: |
  Grow Leapable subscriber base to 50k weekly-active by Q3 2026.
  Primary channels: developer marketing (MCP-native angle) + creator-led
  intellectual marketplace (vault-publisher partnerships). Goal predicate
  is subscriber Asks per week, not raw signups.
```

---

## §3. Allowed metrics per product

The parser rejects unsafe metrics (vanity counts that can be inflated without real product use). Per-product allowed list:

### 3.1 Leapable

| Metric | Definition | SoT |
|---|---|---|
| `weekly_active_subscribers` | distinct subscriber_ids with ≥1 Ask query in 7d | `rocksdb://leapable/cf_queries` |
| `weekly_creator_publish_events` | distinct creators publishing/updating a Vault in 7d | `rocksdb://leapable/cf_creator_publish` |
| `monthly_paid_subscribers` | distinct subscriber_ids with active paid plan | `postgres://marketplace/subscriptions` |
| `mrr_usd` | Stripe-reported MRR | Stripe API |
| `vaults_published_count` | total public Vaults on Discover | `postgres://marketplace/creator_databases` |
| `vault_install_to_first_ask_rate` | activation rate; new installs with first Ask in 24h | `rocksdb://leapable/cf_activation` |

### 3.2 Synapse

| Metric | Definition | SoT |
|---|---|---|
| `weekly_active_mcp_sessions_with_depth` | sessions in 7d with ≥3 distinct tools called AND duration ≥60s | `rocksdb://synapse/cf_sessions` |
| `github_stars_velocity_per_week` | new stars in 7d | GitHub API |
| `third_party_mcp_servers_referencing_synapse` | discovered repos that depend on Synapse | crawl + GitHub API |
| `monthly_paid_subscribers` | paid Pro tier | Stripe API |

### 3.3 Polis

| Metric | Definition | SoT |
|---|---|---|
| `monthly_active_event_attendances` | distinct (user, event) pairs in 28d where attended=true | `postgres://polis/events_attendances` |
| `attended_to_rsvp_ratio_28d` | show rate | derived |
| `median_friend_density_per_active_user` | avg friend-edges per active user | `postgres://polis/social_graph` |

### 3.4 Vanity metrics — rejected

Parser denies these as predicate (allowed only as diagnostics):

- `signups`, `installs`, `downloads`, `followers`, `email_subscribers`
- `impressions`, `reach`, `pageviews`, `mql_count`
- `branded_search_volume`, `mentions`
- Any metric whose definition includes "count of users who ___" without an action verb that proves product use

Rejection error: `PREDICATE_TOO_GAMEABLE`. Reason returned to operator.

---

## §4. Mechanical validation

The parser runs these checks before accepting a goal:

| Check | On fail |
|---|---|
| `goal_id` unique vs. all historical goals (kebab-case, 80 chars max) | `GOAL_ID_INVALID` or `GOAL_ID_TAKEN` |
| `product ∈ {leapable, synapse, polis}` | `PRODUCT_UNKNOWN` |
| `target.metric` in allowed list per product | `METRIC_DISALLOWED_FOR_PRODUCT` |
| `target.metric ∉` vanity-metric deny-list | `PREDICATE_TOO_GAMEABLE` |
| `target.threshold > 0` and reachable from current baseline within deadline | `THRESHOLD_UNREALISTIC` (warning, not block) |
| `evaluation_window >= 7d AND consecutive_windows_required >= 1` | `EVAL_WINDOW_TOO_SHORT` |
| `source_of_truth` is a valid URI matching product's SoT pattern | `SOT_INVALID` |
| `guardrails.budget.daily_velocity_cap_usd ≤ monthly_ceiling_usd / 15` | `BUDGET_VELOCITY_INCONSISTENT` |
| `guardrails.compliance.jurisdictions ⊆ {US,EU,CA,UK,AU,JP}` | `COMPLIANCE_JURISDICTION_UNKNOWN` |
| `autonomy.level ∈ {readonly, low, medium, high, full}` | `AUTONOMY_INVALID` |
| `autonomy.level = high AND no approval_required_for` | `AUTONOMY_HIGH_NEEDS_GATES` |
| Brand voice spec file exists and parses | `BRAND_VOICE_INVALID` |
| Required kill switches present (minimum: brand_voice, rate_limit, goodhart, operator_stop) | `KILL_SWITCH_INSUFFICIENT` |
| `deadline_hard > deadline_soft > created_at + 7d` | `DEADLINE_INVALID` |

Validation is deterministic. Same input = same accept/reject. Re-runnable.

---

## §5. Per-product unit-economics floors (default if omitted)

| Product | `ltv_cac_floor` default | `cac_payback_ceiling_months` default |
|---|---|---|
| Leapable | 3.5 | 12 |
| Synapse | 3.0 | 12 |
| Polis | 4.0 | 9 |

Operator may override with explicit higher/stricter floor; cannot loosen below the doctrine floor (`02_FIRST_PRINCIPLES.md` invariant).

---

## §6. Autonomy levels

| Level | Behavior |
|---|---|
| `readonly` | Fleet observes + drafts; **no external actions**. Used for due-diligence / planning |
| `low` | All external actions `REQUIRE_APPROVAL`; nothing auto-ships |
| `medium` | Content drafts `LOG` (sample audit); outbound + paid `REQUIRE_APPROVAL`; default |
| `high` | Most action classes `LOG` or `AUTO`; only outbound to named accounts, paid spend > `approval_required_above_usd`, and product proposals `REQUIRE_APPROVAL` |
| `full` | All non-doctrine-`REQUIRE_APPROVAL` action classes auto-execute; doctrine list (`02_FIRST_PRINCIPLES.md §19`) is permanent |

Defaults for level `medium` (per `02_FIRST_PRINCIPLES.md §2`):

- `auto_proceed_for`: `organic_post`, `content_draft`, `internal_research`, `follow_up_dm_under_5_per_day`, `metric_query`, `signal_listen`
- `approval_required_for`: `outbound_to_named_account`, `ad_account_change`, `paid_partnership`, `public_statement`, `product_proposal`, `tier3_communication`, `new_channel_adoption`, `paid_spend_above_500usd`

These are reasonable defaults. Operator may tune per goal.

---

## §7. Lifecycle commands

```bash
/goal create <spec-file>            # parse + validate + activate
/goal list                          # all active goals
/goal status <goal_id>              # current predicate + spend + alerts
/goal pause <goal_id>               # operator pause (state preserved)
/goal resume <goal_id>              # resume from last checkpoint
/goal kill <goal_id> --reason="…"   # terminate; state archived
/goal edit <goal_id> --field=value  # narrow edits; most fields immutable post-create
/goal approve <approval_id>         # operator approval queue clear
/goal reject <approval_id> --reason="…"
/goal audit <goal_id> --since=7d    # full audit-log dump
```

Immutable post-create fields: `goal_id`, `product`, `target.metric`, `target.source_of_truth`. Mutating these requires `/goal kill` + new spec.

---

## §8. Example specs (3 starter goals, one per product)

### 8.1 Leapable launch goal

```yaml
goal_id: leapable-3k-launch-q2-2026
product: leapable
owner: chris
created_at: 2026-05-26
target:
  metric: weekly_active_subscribers
  comparator: ">="
  threshold: 3000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://leapable/cf_queries
guardrails:
  budget: {monthly_ceiling_usd: 20000, daily_velocity_cap_usd: 1000}
  unit_economics: {ltv_cac_floor: 3.5, cac_payback_ceiling_months: 12}
  brand: {voice_spec_path: docs/brandvoices/leapable.md}
  compliance: {jurisdictions: [US, EU, CA, UK]}
  channels:
    preferred: [github, hackernews, x_organic, dev_to, podcasts_tier1, newsletter_sponsorships, creator_partnerships]
autonomy:
  level: medium
  approval_required_above_usd: 500
deadline_soft: 2026-08-26
description: |
  90-day Leapable launch: 3,000 weekly-active subscribers, posted-and-ghosted
  Phase A, then mega-launch coordinated with creator-partnership rollout.
```

### 8.2 Synapse activation goal

```yaml
goal_id: synapse-2.5k-mcp-q3-2026
product: synapse
owner: chris
created_at: 2026-05-26
target:
  metric: weekly_active_mcp_sessions_with_depth
  comparator: ">="
  threshold: 2500
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://synapse/cf_sessions
guardrails:
  budget: {monthly_ceiling_usd: 18000, daily_velocity_cap_usd: 900}
  unit_economics: {ltv_cac_floor: 3.0, cac_payback_ceiling_months: 12}
  channels:
    preferred: [github, hackernews, x_organic, mcp_builder_discords, ai_agent_hackathons]
autonomy: {level: medium, approval_required_above_usd: 500}
deadline_soft: 2026-09-01
```

### 8.3 Polis seed-city goal

```yaml
goal_id: polis-cambridge-seed-q2-2026
product: polis
owner: chris
created_at: 2026-05-26
target:
  metric: monthly_active_event_attendances
  comparator: ">="
  threshold: 800                       # single-city seed phase
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: postgres://polis/events_attendances
guardrails:
  budget: {monthly_ceiling_usd: 12000, daily_velocity_cap_usd: 600}
  unit_economics: {ltv_cac_floor: 4.0, cac_payback_ceiling_months: 9}
  channels:
    preferred: [tiktok_organic, instagram_organic, campus_ambassadors, in_person_events, substack_intellectual]
  geo_restriction: cambridge_somerville_ma_usa
autonomy: {level: medium, approval_required_above_usd: 500}
kill_switches:
  - brand_voice_violation
  - attendance_rate_below_0.50
  - goodhart_signal_detected
  - operator_emergency_stop
deadline_soft: 2026-08-31
description: |
  Cambridge/Somerville single-city seed. Goal is dense friend-cluster
  attendance, not raw signups. Operator-physical-presence required.
```

---

## §9. The `/goal` minimal command (Form A)

The CLI accepts a shorthand that expands to a default-filled Form B:

```bash
/goal <verb> <product> to <number> <metric> by <date> [flags]
```

Expansion rules:

- `verb`: `grow` (default), `validate` (sets `autonomy=low`), `defend` (sets predicate to "no decrease beyond X%")
- `product`: required
- `number`: target threshold
- `metric`: must be in the product's allowed list (§3)
- `date`: `deadline_soft`
- `--autonomy=<level>`: overrides default `medium`
- `--budget=$<n>/mo`: sets `guardrails.budget.monthly_ceiling_usd`
- `--compliance=<list>`: overrides default jurisdictions

Defaults applied: doctrine-required kill switches, per-product unit-economics floors, brand voice spec at the canonical path, evaluation window `28d × 4 windows`, autonomy=medium.

The operator can always graduate to Form B for fine-tuning.

---

## §10. Cross-references

- The single predicate definition: `01_NORTH_STAR.md §1`
- Per-product example predicates: `01_NORTH_STAR.md §3` + `11_PRODUCT_PLAYBOOKS.md`
- The Goodhart Auditor's reality cross-checks: `15_METRICS_AND_OBSERVABILITY.md §11`
- The autonomy slider doctrine: `02_FIRST_PRINCIPLES.md §2`
- Doctrine list of permanent `REQUIRE_APPROVAL` actions: `02_FIRST_PRINCIPLES.md §19`
- Kill switch trip codes: `20_RISK_AND_FAILURE_MODES.md §1`
- Operations / approval queue: `19_OPERATIONS_AND_CADENCE.md §3`

