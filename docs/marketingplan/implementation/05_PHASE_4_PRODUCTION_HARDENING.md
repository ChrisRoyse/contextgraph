# Phase 4 — Production Hardening (Weeks 14-16)

> LangSmith full integration; Grafana + Prometheus operator dashboard polished; LLM-as-judge quarterly calibration framework; bench-delta harness operational; weekly Communicator retro polished; Wiki Gardener; vacation/incident CLI modes; self-audit produces full report in <1h; backup+restore tested. After Phase 4, the platform has run 30+ days clean with operator time ≤4 hours/day. The fleet is steady-state; new agent classes, channels, goal types are incremental additions.

**Source spec docs:** `21_BUILD_ROADMAP.md §6 §11`, `05_AGENT_FLEET_DESIGN.md §4 §9`, `15_METRICS_AND_OBSERVABILITY.md §5 §7 §8`, `19_OPERATIONS_AND_CADENCE.md §5 §6 §9 §10 §11`, `20_RISK_AND_FAILURE_MODES.md §10 §12`, `16_TECH_STACK.md §7`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 3 weeks (15 working days) |
| **Prerequisites** | Phase 3 acceptance passed; 14+ days of clean multi-product operation |
| **Headline outcome** | Operator can run `marketing audit --depth=full` and get a comprehensive report in <1h; dashboard polished; bench-delta gate operational for every agent change; vacation/incident modes tested; backup/restore drilled; 30+ days of clean operation |
| **What's NOT in this phase** | Stretch deliverables from `21 §10` (multi-operator coordination, mobile dashboard, public transparency dashboard, A2A protocol) |
| **Acceptance gate to steady-state operation** | All §5 criteria pass; platform is production-grade per `21 §11` definition; Synapse launch ready to begin |
| **Operator time** | 60-90 min/day routine; ≤4 hours/day on full-portfolio supervision; quarterly calibration cadence engaged |

---

## §2. Deliverables — observability

### 2.1 LangSmith full integration

Per `16 §7`:

- Every agent step emits a trace span (LangSmith / OpenTelemetry compatible)
- Spans carry: `goal_id`, `cycle_id`, `node_id`, `agent_class`, tool calls (in/out/cost/latency), LLM call I/O + tokens + cost, approval-queue activity, guardrail decisions with reason, errors with full stack traces
- Replay capability: operator can replay any past cycle from the trace

### 2.2 Grafana + Prometheus operator dashboard

Per `15 §6` + `16 §7`:

- Custom React dashboard reading from RocksDB + LangSmith
- Live per-product views:
  - Active goals with progress bars
  - Pending approvals
  - Last 24h spend by goal + channel
  - Open circuit-breaker alerts
  - Last hourly cycle summary
  - LTV:CAC live per channel
  - K-factor + cycle time per goal's referral mechanic
  - Goodhart Auditor's reality-vs-predicate plot per goal
  - Operator's daily time spent (auto-tracked from approval-queue activity)
- Refresh: every 15 minutes
- Available locally; cloud-exposable on operator approval

### 2.3 Per-agent trace dashboards

LangSmith-native views grouped by agent class, with:
- Token cost per agent over time
- Quality score (Evaluator L4 LLM-as-judge) over time
- Rejection rate (operator approvals)
- Reflexion count + most-cited reflections

---

## §3. Deliverables — bench-delta gate

Per `02 §14` + `15 §5` + `21 §6.1`:

### 3.1 Bench harness

- For each agent class, hold 200 historical experiments / outputs as the eval set
- New agent version scored against the holdout
- Quality (LLM-as-judge calibrated) must not regress from prior version
- Cost per artifact must not regress more than +5%
- Lives at `bench/marketing/<agent_class>/`
- Operator-runnable: `marketing bench delta P1 --new-prompt=path/to/new.md`

### 3.2 Phased rollout per `05 §9`

- **Shadow** — new version runs on real inputs; outputs compared against current; never publishes
- **Canary** — 5% traffic on the lowest-risk action class; outputs reviewed
- **Phased** — 25 → 50 → 100% over a week
- **Auto-rollback** if Evaluator quality drops or any kill switch trips during phased rollout

### 3.3 Bench-delta CI gate

- Any prompt / model / tool change for an agent triggers bench-delta in CI
- Merge blocked if delta regresses
- Operator override possible with documented justification (logged to audit)

---

## §4. Deliverables — quarterly calibration framework

Per `15 §7 §7.1` + `19 §6`:

### 4.1 LLM-as-judge rubric calibration

For each rubric:
- Operator labels 50-200 random outputs blind, quarterly
- Cohen's kappa computed against judge labels
- Below kappa floor → rubric rewritten or replaced with deterministic classifier
- Tracked in `marketing-fleet/calibration/<rubric>/quarterly/<date>.md`

Rubric calibration targets per `15 §7.1`:

| Rubric | Kappa target |
|---|---|
| Brand voice (per product) | ≥ 0.65 |
| Hallucination check | ≥ 0.80 |
| Compliance check | ≥ 0.85 |
| Outreach quality | ≥ 0.60 |
| Reply substance | ≥ 0.55 |
| Tier classification | ≥ 0.70 |

### 4.2 Brand voice recalibration

- Operator's writing evolves quarterly; Scorer must keep up
- 100 new labels per quarter feed re-tune
- Old labels weighted lower over time

### 4.3 Quarterly audit script

`marketing audit --quarterly` produces:
- Compliance attestations for last quarter
- Brand-voice score histogram with sample low-scoring outputs
- Goodhart divergence plots per active predicate
- All approval-queue decisions
- Token + dollar spend by goal + channel
- Reflection-memory dump

This is the report operator hands to legal counsel or a journalist asking "how does this work."

---

## §5. Deliverables — Wiki Gardener (M10)

Per `05 §4`:

- Runs weekly
- Identifies pages not referenced by any agent in 90 days (candidate for archival)
- Surfaces pages with conflicting claims for operator triage
- Flags skill pages that have absorbed too many reflections (refactor candidate)
- Pings relevant Executor to refresh channel cards with last-update >30d

Output: Gardener Digest sent to operator with proposed prune / merge / refactor actions; operator approves; Gardener executes.

---

## §6. Deliverables — vacation / incident modes

Per `19 §9 §10`:

### 6.1 Vacation mode

```bash
marketing vacation-mode --until=2026-06-15
```

Effect:
- Approval queue items shift to `pause_until_operator_returns`
- Tier-3 communications fully paused
- Outbound `REQUIRE_APPROVAL` actions pause
- `AUTO` and `LOG` actions continue at reduced velocity (50% of normal)
- Operator's founder-thread channels pause (X, LinkedIn, etc.)
- Goal predicates continue evaluating; metric trend doesn't pause
- Kill switches remain armed

On return: `marketing resume`.

### 6.2 Incident mode

```bash
marketing incident --severity=high --reason="LinkedIn account suspended pending appeal"
```

Effect:
- Affected platform: full kill (no actions, not even reads)
- Cross-platform: posting paused on operator's other accounts for 24h
- Reflector files comprehensive post-mortem with full audit log of preceding 7 days
- Operator + counsel get incident report within 1 hour
- Approval queue locked until incident clears

---

## §7. Deliverables — self-audit + backup/restore

### 7.1 Self-audit

Per `20 §10`:

```bash
marketing audit --depth=full
```

Generates within 1 hour:
- Last 30d of every external action with payload + outcome
- Compliance attestations for every outbound campaign
- Brand-voice score histogram with sample low-scoring outputs
- Goodhart-divergence plots for every active predicate
- All approval-queue decisions with operator + timestamp
- Token + dollar spend by goal + channel
- Reflection-memory dump

### 7.2 Backup / restore

- Daily RocksDB snapshots to local + S3-compatible archive
- Restore-from-snapshot tested monthly
- Includes: all CFs, configuration files, agent cards, channel cards, brand voice specs, calibration corpora

---

## §8. Week-by-week breakdown

### Week 14 — Observability + dashboard polish

**Day 66-67 (Mon-Tue) — LangSmith integration**
- Every agent span instrumented
- Replay UI verified end-to-end
- Operator can replay a past cycle

**Day 68-69 (Wed-Thu) — Dashboard polish**
- React dashboard reading live data
- Per-product views; per-channel cards; per-agent traces
- Operator-localhost served; cloud-deploy on approval

**Day 70 (Fri) — Per-agent dashboards + alerting**
- LangSmith views per agent class
- Slack/email alerts on circuit-breaker trips, brand-voice violations, compliance failures

### Week 15 — Bench harness + quarterly cadence + Wiki Gardener

**Day 71-72 (Mon-Tue) — Bench-delta harness**
- 200-example holdout per agent class
- `marketing bench delta` CLI
- CI gate that blocks merges with regressions
- Phased-rollout machinery

**Day 73 (Wed) — Quarterly calibration framework**
- LLM-as-judge rubric calibration scripts
- Brand voice recalibration cadence
- `marketing audit --quarterly` script

**Day 74-75 (Thu-Fri) — Wiki Gardener (M10)**
- Weekly cron
- Stale-page detection
- Conflicting-claim surfacing
- Gardener Digest operator email

### Week 16 — Vacation/incident modes + self-audit + backup/restore + acceptance

**Day 76 (Mon) — Vacation + incident CLI modes**
- Both implemented end-to-end
- Tested with synthetic scenarios

**Day 77 (Tue) — Self-audit (full)**
- `marketing audit --depth=full` produces in <1h
- Output is operator-readable + legal-counsel-ready

**Day 78 (Wed) — Backup + restore drill**
- Daily snapshots verified
- Full restore from snapshot tested on a separate host
- Recovery time: <30 min for full restore

**Day 79 (Thu) — 30-day clean-operation verification**
- Audit log of last 30 days
- Zero unscheduled downtime
- Zero compliance trips
- LTV:CAC stable on ≥1 product

**Day 80 (Fri) — Phase 4 acceptance + steady-state declaration**
- Acceptance file written
- Operator sign-off
- Synapse launch can begin (concurrent with Leapable steady-state operation)

---

## §9. Phase 4 architecture decisions to lock

| Decision | Recommended default | When operator can override |
|---|---|---|
| LangSmith deployment | **Operator-hosted on orchestration host** | Cloud-deploy if needed for multi-operator (post-Phase-4 stretch) |
| Bench-delta retention | **Forever** | Never delete |
| Bench-delta sample size | **200 per agent class** | Increase for high-stakes agents (e.g., Outreach Drafter) |
| Quarterly calibration cadence | **Strict 90-day intervals** | Tighten if rapid agent evolution; never loosen |
| Wiki Gardener weekly cadence | **Sunday 03:00 local** | Per operator preference |
| Vacation mode reduced-velocity factor | **50%** of normal AUTO/LOG velocity | Per operator preference (default 50%) |
| Backup snapshot retention | **30 daily + 12 weekly + 12 monthly** | Per operator storage budget |
| Self-audit production cost | **<1h wall-clock; <$50 LLM cost** | If higher, optimize |

---

## §10. Acceptance criteria (gate to steady-state operation)

Phase 4 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | Phase 3 acceptance still passes (regression) | Re-run Phase 3 tests |
| 2 | LangSmith trace coverage: every external action traced; replay possible | Verify with sample cycle replay |
| 3 | Dashboard refreshes ≤15 min; shows live data; operator finds it useful | Operator UX sign-off |
| 4 | Bench-delta gate operational: at least 1 agent upgrade passed through it successfully | Test case |
| 5 | Bench-delta gate operational: at least 1 attempted upgrade was blocked for regression (or rejected with operator override + justification) | Test case or real scenario |
| 6 | Quarterly LLM-as-judge calibration: at least 1 rubric calibrated; kappa documented | Calibration log |
| 7 | Wiki Gardener weekly digest delivered; operator approves prune/merge actions | Gardener output + operator approval |
| 8 | Vacation mode test: operator runs `marketing vacation-mode --until=...`; verify behaviors per `19 §9` | Test scenario |
| 9 | Incident mode test: operator triggers synthetic incident; verify report within 1h; cross-platform pause functions | Test scenario |
| 10 | `marketing audit --depth=full` completes in <1h; output reviewed and approved | Test run |
| 11 | Backup/restore drill: full restore from snapshot succeeds on separate host in <30 min | Drill |
| 12 | 30 consecutive days of clean operation: no unscheduled downtime, no compliance trips, no kill-switch trips beyond normal operation | Audit-log review |
| 13 | LTV:CAC stable above floor on ≥1 product for 30 days | SoT readback |
| 14 | Operator daily marketing time stable at ≤4h | Operator self-report |
| 15 | All 26 agents operational; all 8 channels publishing; ≥1 viral loop with measurable K | Status dashboard |
| 16 | At least 1 product has hit its 90-day predicate (Leapable Phase C target: 3,000 WAS) | SoT readback |

**Evidence file:** `marketing-fleet/evidence/phase_4_acceptance.md`.

---

## §11. Risks specific to Phase 4

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| LangSmith ingestion cost balloons | Medium | Medium | Sampling on low-stakes spans; full coverage on Evaluator + outbound only |
| Dashboard complexity exceeds operator's want-to-look-at threshold | High | Low | Default-collapsed sections; operator-curated favorite views; tight defaults |
| Bench-delta false-negatives (a real improvement looks like regression on holdout) | Medium | Medium | Holdout refresh quarterly; operator override with justification logged |
| Calibration kappa fails to recover after re-tune | Low | Medium | Fall back to deterministic classifier or stricter operator-approval |
| Wiki Gardener prunes a still-useful page | Low | Low | Operator-approval-required for all prunes; recovery from prune log |
| Vacation mode misbehaves on real vacation | Medium | High | Test scenarios + operator dry-run before first real use |
| Incident mode triggers cascade | Low | High | Per-platform isolation; cross-platform pause is 24h, not permanent; operator can clear with explicit `incident clear` |
| Backup restore takes >30 min on operator's hardware | Medium | Low | Daily snapshots are incremental; full restore tested in slack hours |
| Operator approval-queue burnout (still too many items) | Medium | Medium | Re-tune action-class autonomy; promote more to LOG with sample audit |

---

## §12. Operator decisions required during Phase 4

| Decision | Day deadline | Default if no response |
|---|---|---|
| Approve dashboard layout / hosting decision | Day 69 | Localhost; private to operator |
| Set bench-delta override policy for edge cases | Day 72 | Default: bench-delta blocks unless operator explicitly logs justification |
| Quarterly calibration sample size + cadence | Day 73 | Per `15 §7.1` defaults |
| Wiki Gardener prune-approval cadence | Day 75 | Weekly approval batch |
| Vacation mode dry-run | Day 76 | Recommended before real use |
| Incident mode contact list (who gets notified) | Day 78 | Operator + 1 designated counsel email |
| Backup destination + retention policy | Day 78 | Local + S3-compatible at operator-chosen bucket |
| Phase 4 acceptance + steady-state declaration | Day 80 | Phase 4 stays in-progress |

---

## §13. What Phase 4 explicitly does NOT do

- Stretch deliverables from `21 §10` (multi-operator team mode, mobile app, A2A integration, public transparency dashboard, vault auto-ingestion)
- Post-launch optimization rounds (Leapable Phase D-equivalent — these belong in operational launch playbooks)
- New agent classes beyond the 26 (incremental additions are post-Phase-4 enhancements)
- New product launches (Synapse and Polis launches per docs 07/08)
- Cross-operator coordination (team mode is post-Phase-4)

---

## §14. Steady-state operation (post-Phase-4)

Once Phase 4 acceptance passes, the platform enters steady-state. Operations cadence per `19`:

- **Hourly cycles** run autonomously
- **Daily digest** to operator at 07:00
- **Weekly retro** Monday 07:00
- **Monthly recalibration** 1st of month
- **Quarterly audit** every 90 days (calibration + compliance + reputation)
- **Per-90-day operational launches** for new products (Synapse 90-day per doc 07; Polis 90-day per doc 08)

The fleet's role expands when operator launches a new product; contracts when goals achieve. The infrastructure does not need re-building.

---

## §15. Cross-references

- Observability stack: `docs/marketingplan/16_TECH_STACK.md §7`
- Bench-delta discipline: `docs/marketingplan/02_FIRST_PRINCIPLES.md §14` + `15_METRICS_AND_OBSERVABILITY.md §5`
- Quarterly cadence: `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md §6`
- Wiki Gardener: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §4`
- Vacation + incident modes: `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md §9 §10`
- Self-audit: `docs/marketingplan/20_RISK_AND_FAILURE_MODES.md §10`
- Acceptance criteria source: `docs/marketingplan/21_BUILD_ROADMAP.md §6.2 §11`
- Prior phase: `04_PHASE_3_MULTI_PRODUCT.md`
- Concurrent operational launches: `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md`, `07_PHASE_6_SYNAPSE_90DAY_LAUNCH.md`, `08_PHASE_7_POLIS_90DAY_LAUNCH.md`
