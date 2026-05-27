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
