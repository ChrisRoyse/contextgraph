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
