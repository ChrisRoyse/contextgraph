# Acceptance Criteria Matrix + Bench Harness + Evaluator Calibration

> The pass/fail discipline that gates progression between phases, the bench-delta harness that gates agent-class upgrades, and the LLM-as-judge quarterly calibration cadence that prevents Evaluator drift. Without this discipline, the system silently degrades. With it, every change is measurable.

**Source spec docs:** `02_FIRST_PRINCIPLES.md §9 §14`, `05_AGENT_FLEET_DESIGN.md §3 §9`, `15_METRICS_AND_OBSERVABILITY.md §1 §5 §7 §12`, `21_BUILD_ROADMAP.md §6.1 §11`.

---

## §1. The full acceptance criteria matrix (all 8 phases)

For each phase, the gate to the next is **all** of the listed criteria true. Each criterion has a how-to-verify procedure; evidence files at `marketing-fleet/evidence/phase_N_acceptance.md`.

### 1.1 Phase 0 — Foundations

| # | Criterion |
|---|---|
| 1 | `marketing goal create --spec=goals/test_mock.md` succeeds + `goal list` shows it |
| 2 | All 13 validation errors from `22 §4` trigger correctly |
| 3 | All 11 RocksDB CFs registered + accept reads/writes |
| 4 | Hourly cron fires; CF_GOAL_CYCLES has rows |
| 5 | OuterLoop completes mock cycle without errors |
| 6 | Process killed mid-cycle → restart → resumes from checkpoint → no duplicate audit entries |
| 7 | `marketing wiki query` returns ≥3 results from leapable AlexHormozi vault |
| 8 | `marketing kill` halts cron within ≤2s |
| 9 | Audit-log chain integrity passes on 1000 random writes |
| 10 | Operator signed off on agent-card schema for Phase 1 |

### 1.2 Phase 1 — Single-product E2E

| # | Criterion |
|---|---|
| 1 | Phase 0 acceptance still passes (regression) |
| 2 | Brand Voice Scorer Kappa ≥ 0.65 on holdout + 20 fresh Phase-1 drafts |
| 3 | First Leapable goal active ≥10 consecutive days |
| 4 | Daily output: 1 anchor + 3-5 derivatives; ≥80% pass brand-voice on first attempt |
| 5 | Operator approval-queue clearance: median <2h; max <24h |
| 6 | Worthlessness test rejects ≥1 attempted experiment lacking `expected_metric_impact` |
| 7 | Reflexion: ≥5 reflections written + ≥1 consulted by Planner on subsequent attempt |
| 8 | No duplicate X posts despite mid-publish kill+restart |
| 9 | SoT readback: predicate value matches independent manual query |
| 10 | Audit log: every external action has payload + idempotency + compliance + brand voice + outcome |

### 1.3 Phase 2 — Full fleet + Hormozi

| # | Criterion |
|---|---|
| 1 | Phase 1 acceptance still passes |
| 2 | All 26 agent classes operational (cards + impl + tested) |
| 3 | 8 channels publishing per cards; no rate-limit warnings in 14 days |
| 4 | Hormozi vault queried by ≥6 distinct agent classes in past 14 days |
| 5 | Daily output: 1 anchor + ≥10 derivatives across ≥6 channels |
| 6 | Brand voice gate fires correctly; ≥80% first-attempt pass rate |
| 7 | LTV:CAC dashboard reads real numbers for ≥2 channels |
| 8 | Viral loop K-factor ≥ 0.10 in first 14 days |
| 9 | Cohort Health Auditor: ≥1 channel-cohort with sub-floor retention flagged OR proven all clean |
| 10 | Cost Pacer denied ≥1 over-budget spending request (test case) |
| 11 | Operator's daily marketing time stable at ≤90 min |
| 12 | First Leapable goal predicate trending toward 1,500+ |

### 1.4 Phase 3 — Multi-product

| # | Criterion |
|---|---|
| 1 | Phase 2 acceptance still passes |
| 2 | Three goals can run concurrently |
| 3 | Cross-goal scheduler prevents channel collision in 100% tested conflicts |
| 4 | Goodhart Auditor produces daily report; tripped flag halts goal within 1 cycle |
| 5 | Influence Network Mapper produced ≥10 warm-intro paths operator-reviewed |
| 6 | At least 1 warm-intro path executed |
| 7 | Pre-Call Briefer used on at least 1 real Tier-3 conversation |
| 8 | Compliance Auditor: 14 days clean; no trips |
| 9 | Right-to-erasure tested end-to-end (synthetic request, deletion verified ≤7 days) |
| 10 | All 5 kill switches tested |
| 11 | Operator personally signed Polis ethical commitments |
| 12 | Leapable Phase B operational: WAS at 1,500+ |
| 13 | Operator daily time ≤90 min routine + ≤30 min Tier-3 average |

### 1.5 Phase 4 — Production hardening

| # | Criterion |
|---|---|
| 1 | Phase 3 acceptance still passes |
| 2 | LangSmith trace coverage: every external action traced + replayable |
| 3 | Dashboard refreshes ≤15 min; live data; operator finds useful |
| 4 | Bench-delta gate operational: ≥1 agent upgrade passed through |
| 5 | Bench-delta gate operational: ≥1 attempted upgrade blocked OR overridden with justification |
| 6 | Quarterly calibration: ≥1 rubric calibrated; kappa documented |
| 7 | Wiki Gardener weekly digest delivered; operator approves actions |
| 8 | Vacation mode test passes per `19 §9` behaviors |
| 9 | Incident mode test passes; report within 1h; cross-platform pause works |
| 10 | `marketing audit --depth=full` completes in <1h |
| 11 | Backup/restore drill: full restore on separate host in <30 min |
| 12 | 30 consecutive days of clean operation (no unscheduled downtime, no compliance trips) |
| 13 | LTV:CAC stable above floor on ≥1 product for 30 days |
| 14 | Operator daily marketing time stable at ≤4h |
| 15 | All 26 agents operational; all 8 channels publishing; ≥1 viral loop measurable K |
| 16 | At least 1 product hit 90-day predicate (Leapable Phase C: 3,000 WAS) |

### 1.6 Phase 5 — Leapable 90-day launch

| # | Criterion |
|---|---|
| 1 | `weekly_active_subscribers ≥ 3,000` sustained over last 28d |
| 2 | ≥ 10 Creator-Vaults live on Discover |
| 3 | Referral K-factor ≥ 0.20 sustained 4 weeks |
| 4 | At least 1 paid acquisition channel proves LTV:CAC ≥ 3.5 |
| 5 | 90-day cohort retention ≥ 50% on paid Subscribers |
| 6 | Goodhart Auditor clean for last 28d |
| 7 | Zero unscheduled brand voice + compliance + rate-limit trips in last 28d |
| 8 | At least 2 T3 endorsement-quality conversations completed |
| 9 | At least 2 podcast appearances aired |
| 10 | At least 1 newsletter sponsorship with measurable acquisition |
| 11 | Reflexion memory: ≥ 30 structured post-mortems from launch |

### 1.7 Phase 6 — Synapse 90-day launch

| # | Criterion |
|---|---|
| 1 | `weekly_active_mcp_sessions_with_depth ≥ 2,500` over last 28d |
| 2 | Third-party MCP servers referencing Synapse ≥ 50 |
| 3 | GitHub stars velocity ≥ 100/week sustained 4 weeks |
| 4 | ≥ 2 integration partnerships announced |
| 5 | ≥ 3 hackathon sponsorships completed |
| 6 | At least 1 demo video ≥ 500k views |
| 7 | Referral K-factor ≥ 0.20 sustained 4 weeks |
| 8 | At least 2 paid acquisition channels at LTV:CAC ≥ 3.0 |
| 9 | At least 5 podcast appearances aired |
| 10 | Goodhart Auditor clean |
| 11 | Zero unscheduled trips in last 28d |
| 12 | Leapable goal still on track (no regression) |

### 1.8 Phase 7 — Polis 90-day launch

| # | Criterion |
|---|---|
| 1 | `monthly_active_event_attendances ≥ 6,000` over last 28d |
| 2 | 4 cities active with sustained event cadence |
| 3 | NPS at events ≥ 60 |
| 4 | 28d attendee return rate ≥ 50% |
| 5 | Goodhart Auditor clean |
| 6 | Photo-consent rate ≥ 70% |
| 7 | Ambassador program: ≥ 11 active; ≥ 30 activations/month per ambassador; ≥ 70% retained |
| 8 | K-factor (friend-graph density delta) ≥ 0.4 |
| 9 | Zero brand/compliance/Polis-ethical violations |
| 10 | ≥ 3 Substack creator partnerships with revenue share |
| 11 | At least 1 viral TikTok hit (≥500k views) |
| 12 | Leapable + Synapse goals still on track |

---

## §2. The bench-delta harness (Phase 4 deliverable; cross-cutting discipline)

Per `02 §14` + `21 §6.1`.

### 2.1 Why bench-delta exists

Without it, a series of "small prompt improvements" can collectively degrade Evaluator quality or balloon cost. The bench-delta is the **objective, regression-resistant gate** that every agent class change must pass.

### 2.2 Per-agent corpus

For each of the 26 agent classes, build and maintain a 200-example holdout corpus:

```
bench/marketing/<agent_class>/
├── corpus.jsonl                       # 200 (input, ground_truth_output) pairs
├── evaluator_config.yaml              # which evaluator + thresholds + cost ceiling
├── runs/
│   ├── 2026-XX-XX_v0.1.0_initial.json
│   ├── 2026-XX-XX_v0.2.0_prompt_refactor.json
│   └── ...
└── decisions.md                       # operator-readable accept/reject log
```

The `corpus.jsonl` is built by sampling 200 real historical executions of that agent class once enough history exists (typically by mid-Phase-2 for high-volume agents like P1; later for low-volume like D8 Podcast Booker).

### 2.3 Run procedure

```bash
marketing bench delta P1 --new-prompt=path/to/new.md
```

The harness:
1. Runs new version against all 200 inputs
2. Scores outputs via Evaluator L4 (LLM-as-judge, calibrated)
3. Computes deltas vs prior version:
   - Quality (calibrated quality score per output, then mean+stddev)
   - Cost (tokens + dollars per output, mean)
   - Cost ratio (new mean / prior mean)
4. Outputs run.json with: pass/fail, deltas, sample diffs

### 2.4 Gate rules

- Quality must not regress (new mean < prior mean - 1 stddev)
- Cost must not increase >5% without justification
- If either fails: merge blocked
- Operator override possible with explicit `--justify="reason"` flag; logged to audit + `decisions.md`

### 2.5 Phased rollout (per `05 §9`)

If bench-delta passes:

| Stage | Traffic share | Duration |
|---|---|---|
| Shadow | 0% (run on real inputs but never publish; compare with current) | 24h |
| Canary | 5% | 48h |
| Phased 25 | 25% | 48h |
| Phased 50 | 50% | 48h |
| Phased 100 | 100% | indefinite |

Auto-rollback on:
- Evaluator quality drops mid-phase
- Any kill switch trips during phased rollout
- Operator manual rollback

---

## §3. LLM-as-judge Evaluator calibration

Per `15 §7.1` + `19 §6`.

### 3.1 Why calibration exists

LLM-as-judge rubrics (G-Eval / AdaRubric) drift as:
- Operator's voice evolves (Brand Voice rubric)
- Compliance regimes change (Compliance rubric)
- Quality bar evolves (Outreach Quality, Reply Substance rubrics)

Without periodic recalibration against human labels, the judge silently degrades.

### 3.2 Per-rubric calibration targets

| Rubric | Calibration set size | Kappa target | Re-cal frequency |
|---|---|---|---|
| Brand voice (per product) | 100 | ≥ 0.65 | quarterly |
| Hallucination check | 50 | ≥ 0.80 | quarterly |
| Compliance check | 50 | ≥ 0.85 | quarterly |
| Outreach quality | 100 | ≥ 0.60 | quarterly |
| Reply substance | 50 | ≥ 0.55 | quarterly |
| Tier classification | 100 | ≥ 0.70 | quarterly |

### 3.3 Calibration procedure (per rubric, quarterly)

1. Sample 50-200 outputs the judge scored last quarter (random, blind to operator)
2. Operator labels each blind (without seeing judge score)
3. Compute Cohen's kappa between operator + judge
4. If kappa ≥ target → rubric holds; continue
5. If kappa < target:
   - First quarter below: re-tune rubric prompt; re-run; if still below, escalate
   - Second consecutive quarter below: replace LLM-as-judge with deterministic classifier where possible (e.g., brand voice → fine-tuned Gemma); or escalate to operator decision
6. Document calibration in `calibration/rubrics/<rubric>/<date>.md`

### 3.4 Brand Voice Scorer-specific recalibration

The fine-tuned Gemma 4B for brand voice gets re-tuned more often:
- 100 new operator-labeled samples per quarter feed re-train
- Old labels weighted lower (decay weight 0.5 per quarter)
- The operator's writing evolves; the Scorer must keep up

---

## §4. The 5-layer Evaluator stack (recap)

Per `05 §3`:

| Layer | Implementation | Cost | What it catches | Gate? |
|---|---|---|---|---|
| L1 — Schema | Pydantic JSON Schema validator | ~free | Malformed outputs, missing fields | Yes — deny on fail |
| L2 — Deterministic | Regex / lint / hash / length checks | ~free | Forbidden phrases, dup content | Yes — deny on fail |
| L3 — Classifier | Fine-tuned Gemma 4B for brand voice / toxicity / PII | <50ms | Brand drift, leaked PII, banned tone | Yes — deny on fail (configurable threshold) |
| L4 — LLM-as-judge | G-Eval / AdaRubric rubrics on Sonnet | $0.01-0.05 | Quality, helpfulness, factuality | No — diagnostic + Reflexion feed |
| L5 — Reality readback | Direct SoT query against product DB | ~free | Did the artifact's intended effect actually happen? | No — diagnostic (predicate computation) |

L1-L3 are gates (block publish). L4-L5 are diagnostics (feed Reflexion + Planner).

---

## §5. Per-product SoT readback paths

The Evaluator's L5 layer queries these per `15 §2`:

| Product | Predicate metric | SoT |
|---|---|---|
| Leapable | `weekly_active_subscribers` | `rocksdb://leapable/cf_queries` |
| Synapse | `weekly_active_mcp_sessions_with_depth` | `rocksdb://synapse/cf_sessions` |
| Polis | `monthly_active_event_attendances` | `postgres://polis/events_attendances` |

The Evaluator reads these directly. **No vendor dashboards.** Vendor analytics (Mixpanel, GA, Amplitude) are diagnostic-only; never gate.

---

## §6. Goodhart Auditor (cross-cutting acceptance criterion)

Per `01 §6` + `05 §6.2` + `20 §4`:

The Auditor enforces a cross-cutting acceptance criterion for every active goal: predicate cannot rise faster than reality signals. Without it, all other phase acceptance criteria can be Goodhart'd.

Phase 3 ships the Auditor. From Phase 3 onward, every phase acceptance includes "Goodhart Auditor reports clean" as criterion.

If the Auditor trips during any phase's operation, that phase's acceptance is invalidated until the divergence is explained and resolved.

---

## §7. Continuous acceptance — beyond Phase 4 (steady-state)

Once Phase 4 is achieved, the acceptance discipline does not stop. Steady-state acceptance:

| Cadence | What's checked |
|---|---|
| **Per cycle (hourly)** | All guardrails passing; OuterLoop completing; SoT readback functioning |
| **Daily** | Goodhart Auditor report clean; brand voice score histogram stable; cost within budget |
| **Weekly** | Channel mix LTV:CAC; cohort retention; reflexion-memory quality |
| **Monthly** | Per-product strategic review; per-channel performance; bench-delta on any agent upgrade attempts |
| **Quarterly** | LLM-as-judge rubric calibration; brand voice recalibration; compliance audit; reputation audit |

These are the heartbeat. If any of them stops, something is wrong.

---

## §8. Evidence file template (universal)

For every phase acceptance, an evidence file at `marketing-fleet/evidence/phase_N_acceptance.md`:

```markdown
# Phase <N> Acceptance Evidence

**Phase:** <name>
**Date completed:** <YYYY-MM-DD>
**Operator sign-off:** <name>
**Engineering sign-off:** <name>

## Criteria

| # | Criterion | Pass/Fail | Evidence (command / log path / screenshot) |
|---|---|---|---|
| 1 | <criterion> | PASS | `<command>` returned `<output>` |
| 2 | ... | PASS | ... |

## Acceptance metrics snapshot

<key dashboard view OR SoT readback values at moment of sign-off>

## Open items deferred to next phase

<bullet list — anything not blocking but noteworthy>

## Risks observed during this phase

<bullet list of any risks that materialized + how mitigated>

## Reflections filed during this phase

<count + most-cited reflection paths>

## Next phase activation

`<phase activation command>` queued for `<date>`.
```

---

## §9. The single-question test (universal, every gate)

Before declaring any acceptance criterion PASS, ask:

> *"Could this criterion be true while the underlying property is false?"*

If yes, the criterion is Goodhart-risk. Rewrite it to reference SoT, not self-report. Examples:

| Goodhart-risk criterion | Better criterion |
|---|---|
| "Daily output: 1 anchor + 12 derivatives" | "Daily output: 1 anchor + 12 derivatives **AND** ≥80% pass Evaluator L1-L3 gates **AND** appear in CF_AUDIT_LOG with successful publish outcomes" |
| "Operator approval-queue cleared" | "Operator approval-queue cleared **AND** approval decisions logged in CF_AUDIT_LOG with operator ID + timestamp + decision rationale where edit applied" |
| "Brand voice scorer Kappa ≥ 0.65" | "Brand voice scorer Kappa ≥ 0.65 against operator-labeled holdout NOT IN training set" |

The acceptance matrix in §1 is written with this discipline.

---

## §10. Cross-references

- Doctrine on acceptance discipline: `docs/marketingplan/02_FIRST_PRINCIPLES.md §9 §14`
- Evaluator stack design: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §3 §9`
- Calibration discipline: `docs/marketingplan/15_METRICS_AND_OBSERVABILITY.md §7`
- SoT readback principle: `docs/marketingplan/02_FIRST_PRINCIPLES.md §5` + `15_METRICS_AND_OBSERVABILITY.md §12`
- Bench-delta and phased rollout: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §9` + `21_BUILD_ROADMAP.md §6.1`
- Goodhart Auditor: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §6.2` + `20_RISK_AND_FAILURE_MODES.md §4`
- Per-phase build sequences: `01_PHASE_0_FOUNDATIONS.md` through `05_PHASE_4_PRODUCTION_HARDENING.md`
- Per-launch acceptance: `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md` `§11`, `07_PHASE_6_SYNAPSE_90DAY_LAUNCH.md` `§11`, `08_PHASE_7_POLIS_90DAY_LAUNCH.md` `§11`
- Repository layout (bench corpus + calibration dirs): `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md §6 §7`
- Guardrail Engine (the gate machinery): `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md`
