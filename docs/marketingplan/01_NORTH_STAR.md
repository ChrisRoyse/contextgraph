# 01 — North Star

## §0. The single predicate that defines "the system works"

> **For each active goal**, the agent fleet runs the assess→plan→build→communicate loop continuously until the operator-supplied **success predicate** evaluates `TRUE` for **four consecutive rolling 28-day windows**, **and** every per-goal guardrail (budget, brand, compliance, rate limits, autonomy ceiling) was respected the entire time. The system does not stop. It does not declare partial victory. It pauses only when the predicate is `TRUE` for four windows, or the operator overrides, or a hard guardrail trips.

That is the gate. Everything below is the language for expressing what predicate to use.

---

## §1. Goal grammar (the `/goal` command)

The operator's interface is a markdown-encoded goal spec, parseable as a single Pydantic schema. The grammar lives in `22_GOAL_SPEC_FORMAT.md` in full; the load-bearing structure is here.

```yaml
goal_id: contextgraph-50k-q3-2026
product: contextgraph        # contextgraph | synapse | polis
owner: chris
created_at: 2026-05-26

target:
  metric: weekly_active_integrations
  comparator: ">="
  threshold: 50000
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://contextgraph/cf_integrations  # NOT the dashboard

guardrails:
  budget:
    monthly_ceiling_usd: 25000
    daily_velocity_cap_usd: 1500
  unit_economics:
    ltv_cac_floor: 3.0
    cac_payback_ceiling_months: 12
  brand:
    voice_spec_path: docs/brandvoices/contextgraph.md
    forbidden_phrases_path: docs/brandvoices/contextgraph.forbidden.md
  compliance:
    jurisdictions: [US, EU, CA]
    ai_disclosure_required: true     # EU AI Act Article 50
    operator_oversight_required: true # EU AI Act Article 14
  platform_tos:
    per_platform_rate_limits_path: docs/platform_limits.yaml
  channels:
    forbidden: [tiktok_paid, x_paid_ads_until_brand_safety_landed]
    preferred: [github, hackernews, x_organic, dev_to, discord_ai, podcasts_tier1]

autonomy:
  level: medium                       # readonly | low | medium | high | full
  approval_required_above_usd: 500    # any single experiment >$500 needs HITL
  approval_required_for: [outbound_to_named_account, ad_account_change, paid_partnership]
  auto_proceed_for: [organic_post, content_draft, internal_research, follow_up_dm_under_5_per_day]

kill_switches:
  - brand_voice_violation_score > 0.15        # G-Eval rubric on outputs
  - platform_rate_limit_exceeded_any
  - goodhart_signal_detected                  # see §6
  - cost_velocity_exceeds_2x_planned_burn
  - reputation_signal_negative_delta_24h > N  # see §6
  - operator_emergency_stop

deadline_soft: 2026-09-01
deadline_hard: 2026-10-01
exit_on_deadline_hard: pause_and_report
```

Every field is enforced **outside** the agent fleet, by mechanical (shell-script / runtime-policy) gates. The fleet cannot reason its way past them. Karpathy's doctrine: **trust the harness, not the model.**

---

## §2. What "until complete" actually means

The system has six terminal states. Five are non-success, one is success.

| Terminal state | Condition | Operator handoff |
|---|---|---|
| **`GOAL_ACHIEVED`** | success predicate `TRUE` × 4 consecutive 28-d windows, all guardrails held | "GOAL ACHIEVED — set a new one or extend." |
| `DEADLINE_REACHED_PAUSED` | hard deadline hit before predicate | Full status report: what moved, what didn't, recommended next goal |
| `BUDGET_EXHAUSTED_PAUSED` | monthly ceiling hit and not yet at predicate | Spend log, marginal cost per acquired user by channel, recommended budget delta |
| `GUARDRAIL_TRIPPED` | any kill-switch (compliance, brand, Goodhart, reputation) | Incident report, root cause, mitigation, recommended restart |
| `OPERATOR_PAUSED` | operator manual override | Resume with `/goal resume <goal_id>` or `/goal kill <goal_id>` |
| `BLOCKED_NEEDS_HUMAN` | escalation that no agent class can resolve (e.g., legal review of new ad creative format) | Specific question, options, agent's recommendation |

**There is no implicit timeout.** A goal stays active across operator sessions, across daemon restarts, across Anthropic API outages. State is durable (LangGraph checkpointing into RocksDB, see `04_SYSTEM_ARCHITECTURE.md`). The hourly cron picks up exactly where it left off.

---

## §3. Per-product example predicates

These are the success predicates you'd put in the goal spec. Each picks a single scalar metric that **cannot be gamed without actually moving real users through real product behavior**.

### 3.1 ContextGraph / ME‑JEPA‑Code

| Goal class | Example predicate |
|---|---|
| Activation | `weekly_active_integrations >= 50000` where "active" = unique developer (by stable identifier) with ≥1 ME‑JEPA verification call returning a non-trivial verdict in the 7-day window |
| Quality | `cohort_28d_retention_for_new_developers_signed_up_in_window >= 0.40` |
| Enterprise | `paid_organizations_with_>=3_seats_active >= 50` |
| Revenue | `mrr_usd >= 250000 AND ltv_cac >= 3.0` |

### 3.2 Synapse

| Goal class | Example predicate |
|---|---|
| Adoption | `weekly_active_mcp_sessions_with_>=3_distinct_tools >= 25000` |
| Depth | `median_session_duration_seconds_among_active >= 600` |
| Ecosystem | `third_party_mcp_servers_referencing_synapse >= 200` |
| Trust | `github_stars_velocity_per_week >= 500 sustained 4w` |

### 3.3 Polis / Serendipity Engine

| Goal class | Example predicate |
|---|---|
| Activation | `verified_users_with_at_least_one_attended_event_in_last_28d >= 50000` |
| Network effect | `median_friend_density_in_polis_for_active_users >= 5` |
| Show-rate | `attended_to_RSVP_ratio_28d >= 0.65` |
| Cultural | `share_of_polis_events_with_>=1_organic_referral_attendee >= 0.40` |

Vanity counts (signups, followers, downloads) **cannot be predicates**. The goal-spec parser rejects them with `PREDICATE_TOO_GAMEABLE`. The parser keeps a deny-list of unsafe metric names.

---

## §4. The worthlessness test

Borrowed from ContextGraph project doctrine. **Every agent action, every experiment, every dollar spent, every external message sent must trace to moving at least one number on the active goal's metric tree.** If the agent cannot answer "which scalar does this move, and what is the expected magnitude?" the action does not ship.

Concretely, this is enforced as a pre-execution check inside the Planner agent. Every proposed experiment node in the DAG carries a required `expected_metric_impact` field:

```yaml
node: ph_launch_thursday_jan_22
hypothesis: "Coordinated PH launch with 25 warm supporters lifts ContextGraph weekly_active_integrations by 200-400 in week 1"
expected_metric_impact:
  metric: weekly_active_integrations
  delta_low: 200
  delta_high: 400
  confidence: 0.55
cost_usd: 1200
cost_token_budget: 5_000_000
time_budget_hours: 18
risk: medium
risk_drivers: [brand_voice_on_HN_comments, sustained_comment_engagement_during_LA_hours]
fallback: if_thursday_collides_with_anthropic_announcement_postpone_one_week
```

Experiments without that block are rejected by the Planner. The pattern matches Karpathy's AutoResearch: **every experiment is a hypothesis with a quantified expected delta and a fixed budget**. The Aggregator/Evaluator then checks actual against expected and feeds the delta back into the next planning cycle.

---

## §5. Single-number scoreboards

Each product has one composite number watched daily. This is **not** the predicate (the predicate is the gate); the scoreboard is the **dashboard everyone sees**.

| Product | Scoreboard number |
|---|---|
| ContextGraph | Weekly active developer integrations (defined in §3.1) |
| Synapse | Weekly active MCP sessions × distinct-tools-per-session (depth-weighted) |
| Polis | Active small-group attendance rate (showed-up ÷ RSVP'd over 28d) |

These metrics are designed **adversarial against vanity gaming**. They cannot be inflated by:
- Bot signups (no product event fires)
- Content-farm content (no developer integration happens)
- Mass DMs (the recipient still has to install, run, and stay)
- Self-clicks (per-user dedupe at the product DB layer)
- Paid acquisition spike (the predicate is a *sustained* rolling-window comparison; spikes wash out)

---

## §6. Goodhart watchdog

Goodhart's law: *when a measure becomes a target, it ceases to be a good measure*. The single biggest risk of an autonomous agent fleet optimizing for a metric is that it finds a cheap, high-leverage way to move the metric *without moving the underlying reality*.

A separate **Goodhart Auditor Agent** runs on a 24h cron, independently of the Planner. It looks for divergence between the **predicate metric** and three **correlated reality signals**:

| Predicate-side signal | Reality-side cross-check |
|---|---|
| `weekly_active_integrations` rising fast | Are paid users / paid orgs rising in proportion? Are GitHub-star velocity, HN/X mention velocity, and Discord question rate consistent with the new active count? |
| `attended_events` rising fast (Polis) | Are *return* attendances rising? Is event NPS holding? Are net-new social graph edges per attendee growing? |
| `weekly_active_mcp_sessions` rising fast (Synapse) | Is depth (tools per session) holding? Are integrators publishing new MCP servers that reference Synapse? |

If the predicate metric outpaces the reality signal by more than a threshold (default 2x divergence over a rolling 14d window), the auditor flips the `goodhart_signal_detected` flag, which is a hard kill-switch. The fleet stops, the operator gets an incident report, and no further experiments fire until the divergence is explained.

This is the **anti-Cobra Effect** guardrail. Borrowed from the LLM-as-judge calibration literature: continuously check that the cheap proxy still correlates with the expensive ground truth.

---

## §7. State is not the constraint

The product, the channels, the funnel, the homepage copy, the CAC — these are starting state, not fixed constraints. If reaching the predicate requires changing the product (e.g., adding a referral mechanic, see `09_VIRAL_LOOPS_AND_REFERRALS.md`), that change can be proposed by the Planner (the `PRODUCT_PROPOSAL` agent class), but **product changes always require operator approval regardless of autonomy slider setting**. The agent fleet can *propose* with full reasoning and expected impact; it cannot *ship* product changes.

Channels, content, ad creative, outreach copy, scheduling, prioritization — the fleet owns those within guardrails. Product copy on the website, pricing, packaging — operator-gated.

Only **doctrine invariants in `02_FIRST_PRINCIPLES.md`** and the **compliance floor in `14_COMPLIANCE_AND_ETHICS.md`** are truly immovable.

---

## §8. Calibrating "consecutive windows" and "evaluation window"

The defaults of `evaluation_window: 28d` and `consecutive_windows_required: 4` are tuned to:

- **28 days** → long enough to wash out Product Hunt spikes, viral TikTok hits, and seasonal patterns. Short enough that the operator gets feedback within one product cycle.
- **4 consecutive windows** → ~3-4 calendar months at the predicate. By this point the fleet has demonstrated the result is not a coincidence and not a one-time spend.

The goal spec can override (e.g. a short-cycle consumer launch might use `14d × 6`). The parser rejects anything below `7d × 4` as too short to be stable.

---

## §9. Cross-references

- The numerical floors and ceilings per goal: `15_METRICS_AND_OBSERVABILITY.md §2`.
- The mechanism by which each product moves its predicate: `11_PRODUCT_PLAYBOOKS.md`.
- The 90-day plan that creates the first goal worth evaluating: `17_LAUNCH_PLAYBOOK_90DAY.md`.
- The doctrine gating which actions are allowed: `02_FIRST_PRINCIPLES.md`.
- The Goodhart auditor implementation: `20_RISK_AND_FAILURE_MODES.md §4`.
- The `/goal` command full grammar: `22_GOAL_SPEC_FORMAT.md`.
