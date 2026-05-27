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
