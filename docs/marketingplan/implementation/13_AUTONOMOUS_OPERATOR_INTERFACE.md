# Autonomous Operator Interface — Slack-as-Ask + OperatorAsk Agent

> The 27th agent class — `M11 OperatorAsk` — and the Slack-mediated question channel that turns the system from "operator must approve everything proactively" into "operator answers questions when the fleet is genuinely stuck." This is the Karpathy-LLM-Wiki-style HITL pattern that lets the fleet run autonomously for long stretches while still asking when uncertain.

**Source spec docs:** `02_FIRST_PRINCIPLES.md §2 §12 §20`, `04_SYSTEM_ARCHITECTURE.md §7`, `05_AGENT_FLEET_DESIGN.md §1.1 §5.2`, `19_OPERATIONS_AND_CADENCE.md §3 §11 §12`.

---

## §1. Why this exists

The original plan had ONE operator-interactive path:
- **Approval Queue** — agent proposes an action; operator approves/rejects.

This handles "high-stakes execution decisions" but does NOT handle:

| Scenario the original plan handles poorly | Symptom |
|---|---|
| Agent uncertain on tone/framing of a draft (not action-blocking) | Either over-escalates (queue spam) or under-escalates (drift) |
| Agent needs operator's gut on a non-binary creative choice | Forced to guess; Reflexion only fires *after* a wrong guess |
| Agent encounters an artifact the operator personally created (a Tier-3 contact, a private context, an inside-joke vault) and needs ground-truth | No path to ask |
| Agent's reflexion memory has 3 conflicting prior reflections; needs operator to resolve | Hits RUNAWAY_LOOP without resolution |
| Operator notices something the agent missed (e.g., a competitor announcement) and wants to inject it as guidance | No structured way to inject |

The fix: a **Slack-mediated question channel**. Agent posts a structured question, operator replies in thread, agent learns + proceeds.

---

## §2. The Slack workspace

| Field | Value |
|---|---|
| Workspace | leapable.slack.com |
| Workspace ID | `T0B3LAJFYGY` |
| OperatorAsk channel | `C0B2Q0CSJTY` |
| Workspace URL | `https://app.slack.com/client/T0B3LAJFYGY/C0B2Q0CSJTY` |

Stored as env vars (`SLACK_WORKSPACE_ID`, `SLACK_OPERATOR_ASK_CHANNEL`, `SLACK_WORKSPACE_URL`) in Infisical (or `.env.local` for build mode).

---

## §3. M11 OperatorAsk agent class

### 3.1 Slot identity

> "Single point of contact between any other agent and the operator's Slack. Posts structured questions; waits for + parses responses; integrates the answer back into the originating agent's context; writes a Reflexion so the next similar situation doesn't re-ask."

### 3.2 Card

```yaml
---
id: M11
name: OperatorAsk
class: meta
slot_identity: "Slack-mediated operator-question channel"
input_schema: contracts/m11_input.schema.json
output_schema: contracts/m11_output.schema.json
tools_allowed:
  - slack_post_message
  - slack_get_thread_replies
  - wiki_create  # for reflection writing
  - wiki_link
tools_forbidden:
  - send_email_*
  - x_post
  - any external publication tool
vaults_allowed:
  - OperatorBrandVoices
  - all (read-only)  # context-dependent on the originating agent's question
model_routing:
  default: haiku  # cheap; pure formatting + dispatching
  high_stakes_override: sonnet  # for synthesizing operator's free-text answer into structured guidance
context_load:
  - skills/operator-ask-conventions.md
evaluator: E_OperatorAsk  # checks: question is well-formed; not duplicative of recent asks; respects rate cap
retry_policy: 1 attempt; timeout = answer-timeout-config
cost_budget_per_run: $0.01 default
autonomy_default: AUTO  # M11 itself never publishes anything operator-facing; only Slack messages within bound rate
concurrent_max: 3
per_cycle_max: 10 (per goal)
phase_first_active: Phase 1 (basic), Phase 2 (full)
---
```

### 3.3 The Ask protocol

Any agent calling `operator_ask(...)`:

```python
operator_ask(
    requesting_agent: str,           # e.g., "D5 Warm-Intro Drafter"
    goal_id: str,
    question_kind: AskKind,          # one of: gut_check | binary_choice | freeform | injection_request
    question: str,                   # 1-3 sentences max
    context_summary: str,            # 3-5 sentences; what the agent was doing + why this question matters
    options: list[str] | None,       # for binary_choice / multi-select; max 4
    timeout_minutes: int,            # how long to wait; default 240 (4h)
    fallback_action: str,            # what the agent will do if timeout expires
    urgency: Literal["routine", "high"] = "routine",
) -> OperatorAnswer
```

M11 OperatorAsk:
1. Validates the call (rate caps, deduplication, schema)
2. Renders to a Slack message (template per §3.4)
3. Posts to `C0B2Q0CSJTY` (or operator DM for urgency=high)
4. Subscribes to the thread for replies
5. On reply: parses operator's response; if free-text, uses Sonnet to extract structured answer
6. Writes a Reflexion: question + answer + tags
7. Returns `OperatorAnswer { answered: bool, choice?: int, freeform?: str, timed_out: bool }`

If timeout: the originating agent proceeds with `fallback_action`; Reflexion records "timed out, used fallback."

### 3.4 Slack message template

```
:thinking_face: *<requesting_agent> asks* (goal: <goal_id>)

> <question>

*Context:*
<context_summary>

<if options provided:>
*Options:*
1. <option 1>
2. <option 2>
3. <option 3>

<endif>
*Fallback if no answer in <timeout> min:* <fallback_action>

[Reply in thread. Number for option, or free-text for guidance.]

— Ask ID: <ask_id>
```

Operator can:
- Reply `1` / `2` / `3` to pick an option
- Reply free-text to provide guidance
- Reply `skip` to tell the agent to use the fallback
- Reply `kill` to halt the originating node (Reflexion records the kill reason if free-text follows)

---

## §4. Calling conventions for other agents

### 4.1 When to use OperatorAsk

| Situation | Use OperatorAsk? |
|---|---|
| Drafting a cold outreach to a named Tier-2 contact, uncertain on framing tone | Yes — `gut_check` |
| About to send an outbound (any kind) for the first time | No — use Approval Queue (action-gating) |
| About to publish content with brand-voice score 0.78 (below 0.85 threshold) | No — Reflector + retry; if 3 retries fail, surface to Approval Queue |
| Picked an angle for a comparison article but two angles seem viable | Yes — `binary_choice` |
| Tier-3 conversation reveals operator's private context the agent didn't know | Yes — `freeform` |
| Signal Listener spotted a competitor announcement the agent's plan didn't account for | Yes — `injection_request` |
| Daily routine post drafted, brand-voice-passes | No — `LOG` per autonomy slider |

### 4.2 Rate caps per goal

Per `02 §16` (cost discipline) extended to operator-bandwidth discipline:

- **Routine asks:** max 5 per day per goal; OperatorAsk batches if rate-limited
- **High-urgency asks:** max 3 per day per goal; uses operator DM
- **Per agent class:** max 3 asks per cycle per agent (prevents one agent monopolizing)
- **Per cycle (across goals):** max 10 total

Above caps → M11 OperatorAsk refuses the call with `OPERATOR_ASK_RATE_EXCEEDED`; calling agent uses fallback.

### 4.3 Deduplication

OperatorAsk hashes `(question_kind, context_summary[:200], options)`; if the same hash was asked in last 24h, skip the ask and use the prior answer from Reflexion memory.

---

## §5. Operator's experience

### 5.1 Channel pinning

Slack channel `#marketing-fleet-asks` (the operator-named alias for `C0B2Q0CSJTY`) gets:
- Notifications on every routine ask (operator can mute during deep work hours)
- DM notification on every high-urgency ask
- Daily summary at 07:00 of asks answered + asks timed out

### 5.2 Response pattern

Operator replies in thread. Numbered for option-pick; free-text for guidance:

```
@OperatorAskBot
2

(Or with explanation:)

@OperatorAskBot
2 — option 1 sounds too formal for Tyler's voice; option 2 is more him
```

Free-text explanations are gold for Reflexion memory; M11 weights them more heavily than bare option-picks.

### 5.3 Slash commands

| Command | Effect |
|---|---|
| `/marketing pause-asks` | Pauses all routine asks for 4h (deep work mode); high-urgency still fires |
| `/marketing resume-asks` | Resumes |
| `/marketing ask-history --since=24h` | Shows recent asks + answers |
| `/marketing inject "<text>"` | Operator-initiated guidance; M11 routes to the next-relevant agent's context |

### 5.4 Vacation-mode integration

Per `19 §9`: when operator runs `marketing vacation-mode --until=...`:
- All routine asks pause (timeouts disabled; queued for return)
- High-urgency asks → emergency contact (operator's pre-designated counsel or backup)
- On return, queued asks surface in order

---

## §6. Reflexion integration

Every answered ask becomes a Reflexion row keyed by `(requesting_agent, question_kind, context_hash)`. The Planner reads these before proposing similar work; the originating agent reads before re-asking.

Example Reflexion:

```yaml
ask_id: ask_2026-06-15_T08
requesting_agent: D5 Warm-Intro Drafter
question_kind: gut_check
question: "Tyler Cowen warm intro — formal or casual tone?"
context_summary: "Drafting warm intro forwardable paragraph from Bob Lee. Tyler's recent X posts skew casual, but the topic is enterprise AI verification which is more formal."
options: ["formal", "casual", "casual but technical"]
operator_answer:
  choice: 2
  freeform: "casual is better; he hates corporate-speak. lead with the proof point, not the pitch."
answered_at: 2026-06-15T08:42:11Z
agent_used: D5 (D5 then continued drafting with casual + lead-with-proof)
takeaways_filed_to: skills/warm-intro-tier3.md
retry_conditions: re-ask if target's voice diverges meaningfully from this baseline
```

Next time D5 drafts a Tyler intro (or similar Tier-3 with casual voice signal), it reads the Reflexion and skips the ask.

---

## §7. Anti-patterns OperatorAsk avoids

| Anti-pattern | Why it's bad | OperatorAsk mitigation |
|---|---|---|
| Spamming operator with "are you sure?" | Burnout; operator ignores future asks | Rate cap + dedup |
| Asking the same question twice in a week | Same as above; signals broken Reflexion | Dedup checks (`context_hash`) |
| Asking for permission to do something the autonomy slider allows | Operator wants the agent to just do it | Strict separation: Approval Queue for execution gating; OperatorAsk for *information* gathering only |
| Asking vague questions ("what should I do?") | Operator can't answer | Schema enforces `question_kind` + structured `options` where applicable |
| Asking too late (after the action) | Operator can't influence outcome | M11 only accepts asks pre-action; post-action goes to Reflector |
| Asking via 3 different channels simultaneously | Confusing | M11 is the only path; other agents call M11 and only M11 talks to Slack |

---

## §8. The full HITL surface (after M11 added)

| Surface | Purpose | Operator-time per item |
|---|---|---|
| **Approval Queue** | Gate execution of REQUIRE_APPROVAL action classes | ~30s per item; ~3-8 items/day routine |
| **OperatorAsk (Slack)** | Ask informational questions; gather guidance; allow injection | ~20s per item; ~3-5 items/day routine |
| **Daily digest (M9 Communicator)** | Read-once summary of cycle activity | ~5 min/day |
| **Weekly retro** | Strategic review + channel reallocation approval | ~90 min/wk |
| **Monthly recalibration** | Per-product strategic decisions | ~3 hr/month |
| **Quarterly audit** | Calibration + compliance + reputation review | ~1 day/quarter |

Total routine operator time: **~45-60 min/day** in steady state. Lower than the original plan (60-90 min) because OperatorAsk reduces friction on borderline drafts that previously would have hit the Approval Queue.

---

## §9. Build-out checklist (Phase 1 → Phase 2 → Phase 4)

### Phase 1 (basic OperatorAsk)
- [ ] Slack app created in `T0B3LAJFYGY` workspace
- [ ] Bot user added to `C0B2Q0CSJTY`
- [ ] `SLACK_BOT_TOKEN` + `SLACK_SIGNING_SECRET` in Infisical (or `.env.local`)
- [ ] M11 OperatorAsk agent shipped (basic: post + listen + parse + reply)
- [ ] Approval Queue + M11 differentiated in operator UX
- [ ] Operator tests on 5 synthetic asks; latency + clarity confirmed

### Phase 2 (full OperatorAsk)
- [ ] Reflexion integration (dedup + skill-page writeback)
- [ ] Rate caps + per-agent quotas
- [ ] Slash commands (`/marketing pause-asks`, `/marketing inject`)
- [ ] Daily summary digest
- [ ] M11 calls from all 26 other agent classes wired

### Phase 4 (hardening)
- [ ] Vacation-mode integration
- [ ] High-urgency DM routing
- [ ] LangSmith tracing of every ask
- [ ] Quarterly OperatorAsk performance audit (% answered, median time-to-answer, % timed-out, % followed)

---

## §10. Acceptance criteria (woven into per-phase gates)

### Phase 1 addition

- [ ] M11 OperatorAsk operational; ≥5 successful round-trips on test asks
- [ ] Slack bot installed in `C0B2Q0CSJTY`
- [ ] First real ask from D5 Warm-Intro Drafter answered + Reflexion written

### Phase 2 addition

- [ ] M11 ask volume sustained ≤5/day routine; operator-time per ask ≤30s median
- [ ] Dedup catches ≥1 attempted duplicate ask
- [ ] At least 3 agent classes have called M11 in last 14 days

### Phase 4 addition

- [ ] Vacation-mode test: routine asks pause; high-urgency routes to backup
- [ ] Quarterly OperatorAsk audit: % answered ≥85%; median time-to-answer ≤2h

---

## §11. Agent catalog update

The 26-agent catalog becomes **27 agents** with M11 added under meta-tier:

| ID | Class | Phase first active |
|---|---|---|
| M1 | Operator Agent (conversational) | Phase 0 |
| M2 | Goal Parser | Phase 0 |
| M3 | Assessor | Phase 0 |
| M4 | Planner | Phase 0 |
| M5 | Atomizer | Phase 0 |
| M6 | Aggregator | Phase 0 |
| M7 | Evaluator | Phase 0 (stub) → 1 → 2 |
| M8 | Reflector | Phase 0 |
| M9 | Communicator | Phase 0 |
| M10 | Wiki Gardener | Phase 4 |
| **M11** | **OperatorAsk (NEW)** | **Phase 1 (basic) → 2 (full) → 4 (hardened)** |

`09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md` updated accordingly.

---

## §12. Cross-references

- Approval Queue (the action-gating channel): `docs/marketingplan/04_SYSTEM_ARCHITECTURE.md §7` + `19_OPERATIONS_AND_CADENCE.md §3`
- Reflexion memory pattern (where answers land): `docs/marketingplan/02_FIRST_PRINCIPLES.md §7 §15` + `04_SYSTEM_ARCHITECTURE.md §4`
- Operator's daily rhythm (where OperatorAsk fits): `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md §2`
- Secrets handling for Slack token: `12_SECRETS_AND_INFISICAL.md §5`
- Build-mode + Haiku routing for M11: `14_BUILD_MODE_HAIKU_ONLY.md`
- Agent skeleton template: `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md §2.2`
- Updated agent build order with M11: `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md §2.3`
