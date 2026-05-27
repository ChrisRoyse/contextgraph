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
