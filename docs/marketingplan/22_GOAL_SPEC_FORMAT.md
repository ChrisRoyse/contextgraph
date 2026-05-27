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
