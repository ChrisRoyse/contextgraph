# Phase 1 — Single-Product End-to-End (Weeks 3-6)

> Two executor agents live (P1 Content Writer + D1 Multi-Platform Publisher); Brand Voice Scorer fine-tuned + operational; Approval Queue with Slack-based HITL; worthlessness test enforced in Planner; first real Leapable campaign publishing to one operator-owned X account via Synapse. After Phase 1, the operator approves 3-5 daily items from Slack and watches Leapable's `weekly_active_subscribers` SoT readback start moving. This is the first real use of the platform.

**Source spec docs:** `21_BUILD_ROADMAP.md §3`, `05_AGENT_FLEET_DESIGN.md §1 §2 §3 §6.3`, `06_COMPUTER_USE_PLAYBOOK.md §2.1 §4 §8.1`, `07_CONTENT_FACTORY.md §1 §3 §4`, `11_PRODUCT_PLAYBOOKS.md §1`, `14_COMPLIANCE_AND_ETHICS.md §1 §2 §7`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 4 weeks (20 working days) |
| **Prerequisites** | Phase 0 acceptance passed; operator-owned X account registered in `accounts.yaml`; Synapse running on operator desktop |
| **Headline outcome** | Operator types `/goal grow Leapable to 500 weekly_active_subscribers by 2026-07-31`; fleet drafts daily anchor + 4 derivatives; brand-voice-passes auto-publish to X via Synapse `LOG`; brand-voice-fails enqueue for operator approval via Slack |
| **What's NOT in this phase** | More than 1 channel; more than 2 executor agents; the Hormozi vault integration (Phase 2); paid ads (Phase 2); the Goodhart Auditor (Phase 3); multi-product (Phase 3) |
| **Acceptance gate to Phase 2** | All §5 acceptance checks pass; first Leapable campaign has ≥10 days of clean operation with operator-approval friction at acceptable level |
| **Operator time** | 30-45 min/day approving items + sample-audit; ~10-15 hours over 4 weeks |

---

## §2. Deliverables

### 2.1 Agent classes to bring online

Per `05_AGENT_FLEET_DESIGN.md §1.3` and `§1.4`, the two Phase-1 executors:

| ID | Class | Role | Tools allowed |
|---|---|---|---|
| **P1** | Content Writer | Drafts anchor + derivatives from briefs; reads brand voice + reflections | `wiki_search`, `wiki_get`, `brand_voice_score`, `readability_score`, `draft_save` |
| **D1** | Multi-Platform Publisher (single-channel mode) | Cross-formats and schedules drafts to ONE channel (X) per channel card | `draft_get`, `x_post` (Synapse-mediated), `idempotency_check`, `schedule_persist` |

Plus the meta agents from Phase 0 are upgraded:
- **M3 Assessor** — now reads real SoT (RocksDB CF_QUERIES on Leapable's database) for predicate evaluation
- **M4 Planner** — now enforces the worthlessness test (`02 §4`); rejects nodes without `expected_metric_impact` block
- **M7 Evaluator** — first real implementation (L1 Schema + L2 Deterministic only in Phase 1; L3 Brand Voice Scorer lights up §2.3 below; L4 LLM-as-judge deferred to Phase 2)
- **M8 Reflector** — first real implementation: structured post-mortems on Phase-1 failures
- **M1 Operator Agent** — Slack interface for approval queue

### 2.2 Agent class cards to author (per `05 §2` template)

Create `marketing-fleet/agents/P1_content_writer.md` and `marketing-fleet/agents/D1_publisher.md` with the full YAML frontmatter from `05 §2`. Use the Phase-0 schema sign-off (`01 §7`).

For each agent:
- `slot_identity` — one sentence
- `input_schema` / `output_schema` — JSON Schema files referenced
- `tools_allowed` / `tools_forbidden` — explicit lists; Guardrail Engine enforces
- `model_routing` — default tier + overrides
- `context_load` — wiki pages auto-loaded on start
- `evaluator` — which Evaluator pipeline applies
- `retry_policy` — count + Reflexion integration
- `cost_budget_per_run` — per-call cap
- `autonomy_default` — `LOG` for P1 with brand-voice gate; `LOG` for D1 with same gate + first-time-on-channel `REQUIRE_APPROVAL`

### 2.3 Brand Voice Scorer (the central Phase-1 deliverable)

Per `05 §6.3` and `07 §3.2`:

**Build steps:**
1. Operator provides 100-200 historical operator-authored Leapable content samples (X posts, blog drafts, README sections). Label each `accept` / `reject` (~30 min operator work).
2. Fine-tune a Gemma 4B classifier on the labeled set. Train/val split 80/20.
3. Target Kappa ≥ 0.65 against operator labels on holdout (per `15 §7.1`).
4. Ship as a deterministic tool `brand_voice_score(draft_text) → {score: 0..1, dimensions: {...}}`.
5. Wire into Evaluator L3 (`05 §3`).
6. Set threshold = 0.85 in `marketing-fleet/config/evaluators.yaml`.

**Operator validation:** during week 4, operator labels 20 fresh drafts blind. Kappa must hold ≥ 0.6 on this independent set; below → re-train before Phase 1 acceptance.

**Brand voice spec doc:** create `marketing-fleet/brandvoices/leapable.md` per the template in `07 §3.1`, including:
- Stance + voice + structural rules
- Forbidden phrases regex (`07 §3.1` provides starter set)
- Reference exemplars from operator's approved content

### 2.4 Approval Queue (Slack-based HITL)

Per `04 §7` + `19 §3`:

**Slack app installation:**
- Operator creates a Slack workspace (or uses existing) + installs a marketing-fleet Slack app
- App posts approval items to a dedicated `#marketing-approvals` channel
- Operator response via button click (`Approve`, `Approve with edit`, `Reject - try again`, `Reject - kill`, `Escalate later`)
- Round-trip latency target: <5s from operator click to Executor resumption

**Approval item rendering:**
```
[goal: leapable-500-launch | node: anchor-2026-06-12 | priority: routine]
Action: post to X (chris_personal)
Brand voice score: 0.89 ✓
Compliance: US-only, no EU disclosure needed
Expected impact: +5 to +25 weekly_active_subscribers (confidence 0.4)
Cost: $0
Draft:
> [first 500 chars of the proposed post]
[Approve] [Edit & Approve] [Reject - retry] [Reject - kill] [Snooze 4h]
```

**Approval queue persistence:** every approval item written to `CF_APPROVAL_QUEUE` with TTL = 48h (auto-rejection log per `04 §7`). Decisions logged to `CF_AUDIT_LOG` with operator ID + decision + edit-diff if applicable.

### 2.5 Synapse integration for X.com

Per `06 §2.1` + `08.3.1 channel card`:

**Steps:**
1. Operator registers `x_chris_personal` (or chosen handle) in `marketing-fleet/accounts.yaml` per `06 §3.1`.
2. Channel card `marketing-fleet/channel_cards/x.yaml` written per `06 §4` + `08 §5`.
3. Rate caps set 30% below platform thresholds: 4 posts/24h, 25 replies/24h, etc.
4. Stochastic cadence: log-normal mean 4h stddev 2.5h.
5. Synapse session test: operator authenticates X on desktop; `act_*` calls publish a known-test post; idempotency confirmed.
6. EU disclosure wrapper: Phase 1 only operates on operator's primary US-located account → disclosure not required for outbound on this account; the wrapper exists but doesn't fire.

### 2.6 First Leapable goal spec

File `marketing-fleet/goals/leapable-500-launch-2026-q2.md`:

```yaml
goal_id: leapable-500-launch-2026-q2
product: leapable
owner: chris
created_at: 2026-06-10
target:
  metric: weekly_active_subscribers
  comparator: ">="
  threshold: 500
  evaluation_window: 28d
  consecutive_windows_required: 4
  source_of_truth: rocksdb://leapable/cf_queries
guardrails:
  budget: {monthly_ceiling_usd: 2000, daily_velocity_cap_usd: 100}  # Phase 1: low; no paid ads
  unit_economics: {ltv_cac_floor: 3.5, cac_payback_ceiling_months: 12}
  brand: {voice_spec_path: brandvoices/leapable.md}
  compliance: {jurisdictions: [US]}
  channels:
    preferred: [x_organic]  # Phase 1: single channel
    forbidden: [paid_ads_any]
autonomy:
  level: medium
  approval_required_above_usd: 50
  auto_proceed_for: [content_draft, organic_post_under_brand_voice_threshold]
  approval_required_for: [outbound_to_named_account, public_statement]
kill_switches:
  - brand_voice_violation
  - platform_rate_limit_exceeded_any
  - cost_velocity_exceeds_2x_planned_burn
  - operator_emergency_stop
deadline_soft: 2026-07-31
description: |
  Phase 1 single-product validation goal. One channel (X organic). Two agents (P1+D1).
  Worth-test enforced. Brand voice gated. First real use of the platform.
```

### 2.7 Compliance enforcement (Phase 1 subset)

Per `14_COMPLIANCE_AND_ETHICS.md §1 §2`:

- Implement `send_email`, `send_dm`, `x_post` tool wrappers as jurisdiction-aware.
- For Phase 1's US-only goal, the disclosure block is suppressed correctly.
- CAN-SPAM disclosure block exists for any email tool (not used in Phase 1 but skeleton ready).
- Audit log captures every guardrail decision per `14 §7` schema.

### 2.8 Reflexion memory shipping

Per `02 §7 §15` + `04 §4`:

- On every operator rejection from approval queue, M8 Reflector writes a `Reflection` to `CF_REFLECTIONS` keyed by `(channel, tactic, ts_ns)` with the structure from `02 §15`.
- Planner reads relevant Reflections by `(channel, tactic)` before proposing new experiments.
- Retry cap of 3 attempts per parent node within a cycle (per `04 §4`).

---

## §3. Week-by-week breakdown

### Week 3 — Brand Voice Scorer + agent cards + approval-queue skeleton

**Day 11-12 (Mon-Tue) — Brand Voice Scorer fine-tune**
- Operator labels 100-200 historical samples (single 60-90 min session).
- Fine-tune Gemma 4B on the labeled set; train ~2h on operator GPU.
- Validate Kappa ≥ 0.65 on holdout.
- Ship `brand_voice_score` tool.

**Day 13-14 (Wed-Thu) — Agent cards for P1, D1**
- Author `agents/P1_content_writer.md` and `agents/D1_publisher.md` per the schema.
- Implement input/output JSON Schemas; wire to LangGraph subgraph stubs.
- Write per-agent prompt templates; ground in Leapable brand voice doc.

**Day 15 (Fri) — Approval queue Slack integration**
- Slack app installed; channel `#marketing-approvals` set up.
- App can post items; operator can respond via buttons.
- Round-trip latency tested.

### Week 4 — P1 + D1 + Worth Test + First End-to-End Cycle

**Day 16-17 (Mon-Tue) — P1 Content Writer implementation**
- P1 reads brief from Planner; loads brand voice + relevant reflections; drafts via Sonnet.
- Outputs `draft_md` + `metadata` + `brand_voice_score` per `05 §2 P1 card output schema`.
- L1/L2/L3 Evaluator gates wired.

**Day 18 (Wed) — D1 Multi-Platform Publisher (X-only mode)**
- D1 reads draft from P1; cross-formats to X (250-char hook + thread template); idempotency-checked; rate-limit-checked; Synapse-mediated publish.
- First end-to-end dry run on a staging X account (operator-controlled).

**Day 19 (Thu) — Worthlessness test in Planner**
- Planner requires `expected_metric_impact` block on every experiment node per `01 §4` + `02 §4`.
- Rejection error: `EXPERIMENT_LACKS_QUANTIFIED_IMPACT`.
- Test with a goal that has a no-impact experiment proposed; verify rejection.

**Day 20 (Fri) — First Leapable goal live**
- Operator creates `leapable-500-launch-2026-q2`.
- First hourly cycle dispatched: Planner proposes 1 anchor + 4 derivatives.
- P1 drafts → Evaluator gates → D1 publishes (after operator approval for first-time-on-channel).
- Operator approves first item from Slack within 30 min.
- Audit log shows clean trail.

### Week 5 — Compounding + Reflexion + Operator UX tuning

**Day 21-22 (Mon-Tue) — Reflexion memory active**
- First operator rejection → Reflector writes structured Reflection.
- Planner reads on next similar experiment → demonstrates lesson application.
- Operator sample-audits 5 reflections for accuracy.

**Day 23-24 (Wed-Thu) — Daily-cadence stabilization**
- Cycle cadence smoothed: 1 anchor draft per day (operator-approved-or-auto-published); 4 derivatives.
- Approval-queue latency: operator clears in <30 min daily.
- Brand-voice rejection rate measured: target <15%; if higher, re-tune.

**Day 25 (Fri) — Mid-phase review**
- Operator reviews 5 days of operation; collects friction points.
- Engineering pass on top-3 friction items.

### Week 6 — Hardening + Acceptance evidence

**Day 26-27 (Mon-Tue) — Idempotency hardening**
- Stress-test: kill mid-publish; restart; verify no duplicate X posts.
- Verify cross-channel idempotency keys (will matter Phase 2).

**Day 28-29 (Wed-Thu) — 10-day clean-operation log**
- Phase 1 acceptance requires 10 consecutive days of operation without unscheduled breakdown.
- Document the 10 days: cycle count, posts published, rejections, reflections written, approval-queue latency, brand-voice score histogram.

**Day 30 (Fri) — Phase 1 acceptance file**
- Write `marketing-fleet/evidence/phase_1_acceptance.md` per §5.
- Operator sign-off.

---

## §4. Phase 1 architecture decisions to lock

| Decision | Recommended default | When operator can override |
|---|---|---|
| Brand Voice Scorer base model | **Gemma 4B fine-tuned** | Swap for similar-class open model if license issues |
| Brand voice threshold | **0.85** | Per goal in spec; never below 0.7 |
| Approval queue UI primary | **Slack** | Add web dashboard in Phase 4; Slack always available |
| Approval TTL | **48h** | Per goal in spec |
| First channel for D1 | **X organic** | Could be LinkedIn if operator prefers; X has higher dev audience reach |
| Synapse posting cadence distribution | **Log-normal mean 4h stddev 2.5h** | Per channel card; never tighter than 30%-below-detection |
| Worthlessness test enforcement | **Hard (Planner refuses node)** | Never disable; per doctrine §4 |
| Reflection retention | **Forever in CF_REFLECTIONS; Wiki Gardener prunes in Phase 4** | Never delete until M10 ships |

---

## §5. Acceptance criteria (gate to Phase 2)

Phase 1 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | Phase 0 acceptance still passes (regression) | Re-run Phase 0 tests |
| 2 | Brand Voice Scorer Kappa ≥ 0.65 on Phase-0-handoff holdout + 20 fresh Phase-1 drafts | Operator labeling exercise |
| 3 | First Leapable goal active for ≥10 consecutive days | `marketing goal status` cycle count + audit-log timestamps |
| 4 | Daily output: 1 anchor + 3-5 derivatives drafted; ≥80% pass brand-voice gate on first attempt | Cycle log aggregation |
| 5 | Operator approval-queue clearance: median <2h; max <24h | Approval-queue timestamp analysis |
| 6 | Worthlessness test rejects ≥1 attempted experiment lacking `expected_metric_impact` | Test case + audit-log |
| 7 | Reflexion memory: ≥5 reflections written and ≥1 successfully consulted by Planner on subsequent attempt | CF_REFLECTIONS dump + Planner trace |
| 8 | No duplicate X posts despite mid-publish kill+restart tests | Forensic of CF_AUDIT_LOG against X account history |
| 9 | SoT readback: predicate value (`weekly_active_subscribers`) computed correctly from Leapable's RocksDB; matches independent manual query | Manual cross-check |
| 10 | Audit log: every external action has tool-call payload + idempotency key + compliance check + brand voice score + outcome | `marketing audit --since=10d` review |

**Evidence file:** `marketing-fleet/evidence/phase_1_acceptance.md` with matrix filled.

---

## §6. Risks specific to Phase 1

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Brand Voice Scorer Kappa below 0.65 | Medium | High | Re-label edge cases; expand training set; if still failing, fall back to deterministic forbidden-phrase regex + operator-approval-on-everything (slower Phase 1) |
| X rate-limit warning during shakedown | Medium | Medium | Channel-card immediate cap reduction by 30%; widen cadence distribution; investigate trigger |
| Operator approval-queue fatigue | High | Medium | Tune brand-voice threshold up; promote more action classes to `LOG`; reduce volume to 1 anchor + 2 derivatives if needed |
| Synapse session loss mid-publish (browser crash, OS update) | Medium | Low | Idempotency keys + retry policy; operator notified if Synapse offline >1h |
| Planner generates spam-velocity proposals despite worth test | Low | High | Cost Pacer wraps Planner output; per-cycle node-count cap; manual review of Planner's first-week proposals |
| Operator's labeled training set has its own biases | Medium | Medium | Operator labels iteratively; re-tune scorer quarterly per `15 §7.1` |
| First Leapable predicate moves due to non-Phase-1 cause (e.g., a unrelated PR mention) | Low | Low | Attribution: Phase 1 is single-channel; movement clearly traceable to X organic |
| Reflexion memory accumulates noise | Low | Medium | Wiki Gardener (Phase 4) handles cleanup; Phase 1 acceptable to over-collect |

---

## §7. Operator decisions required during Phase 1

| Decision | Day deadline | Default if no response |
|---|---|---|
| Label 100-200 historical content samples for Brand Voice Scorer | Day 11 | Cannot proceed; Phase 1 blocked |
| Sign off on `brandvoices/leapable.md` content | Day 14 | Cannot proceed; defaults too generic |
| Authenticate X account on Synapse desktop | Day 18 | Phase 1 blocked at D1 |
| Approve first-time-on-channel publish for X | Day 20 | First publish blocked until approved |
| 20 fresh labels for scorer re-validation in week 4 | Day 24 | Use Phase 0 holdout only; less robust |
| Phase 1 acceptance sign-off | Day 30 | Phase 2 blocked |

---

## §8. What Phase 1 explicitly does NOT do

- More than 1 channel (X only) — Phase 2 brings 7 more
- Paid ads of any kind — Phase 2 brings the Ad Operator with operator approval
- Hormozi vault deep integration — Phase 1 has leapable wired but Hormozi-specific querying lights up Phase 2
- Outreach (cold or warm) — Phase 2 brings D4
- Tier classification beyond default T1 — Phase 2-3 brings S1
- Goodhart Auditor — Phase 3
- Influence Network Mapper — Phase 3
- Multi-product — Phase 3
- LLM-as-judge (L4 Evaluator) — Phase 2 adds for quality dimensions deterministic gates can't catch
- Programmatic SEO + GEO — Phase 2 brings O1/O2
- Referral mechanic — Phase 2 brings O3 Referral Engineer
- Cohort Health Auditor — Phase 2 brings O7

---

## §9. Operator's daily routine during Phase 1

Per `19 §2`:

1. Morning (15-20 min): clear approval queue, scan dashboard, post 1-2 operator founder-thread items personally.
2. Midday: deep work (product, not marketing).
3. Afternoon (10-15 min): review any escalations; sample-audit 2-3 published items.
4. Evening (optional): read 1 P1 anchor; provide tuning feedback if any drift.

Total operator marketing time in Phase 1: ~45-60 min/day. Significantly less than typical solo-founder marketing supervision because the fleet handles the throughput.

---

## §10. Cross-references

- Agent class catalog: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md`
- Agent card schema template: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §2`
- Brand voice spec template: `docs/marketingplan/07_CONTENT_FACTORY.md §3.1`
- Brand Voice Scorer design: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §6.3`
- Approval queue UX: `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md §3`
- Compliance gates: `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md`
- Computer-use playbook for X: `docs/marketingplan/06_COMPUTER_USE_PLAYBOOK.md §8.1`
- Worthlessness test: `docs/marketingplan/01_NORTH_STAR.md §4` + `02_FIRST_PRINCIPLES.md §4`
- Prior phase: `01_PHASE_0_FOUNDATIONS.md`
- Next phase: `03_PHASE_2_FULL_FLEET_HORMOZI.md`
- First operational launch (concurrent with this phase end): `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md`
