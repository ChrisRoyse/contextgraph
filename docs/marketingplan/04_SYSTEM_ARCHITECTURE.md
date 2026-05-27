# 04 — System Architecture

> The complete blueprint of the goal-driven autonomous growth engine. Built on the ROMA recursive-meta-agent pattern (Atomizer / Planner / Executors / Aggregator), the Yopedia cron-driven assess→plan→build→communicate loop, LangGraph's durable execution + HITL primitives, Karpathy's AutoResearch single-scalar-metric discipline, and Anthropic's context-engineering principles for long-horizon coherence.

---

## §1. High-level diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                  OPERATOR INTERFACE                                  │
│   /goal <spec>   /pause   /resume   /kill   /status   /approve   /reject   /audit    │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                   GOAL STORE                                          │
│   GoalSpec (Pydantic schema)  ·  Status  ·  Predicate evaluator  ·  Audit log         │
│   Backed by RocksDB CFs: CF_GOALS, CF_GOAL_HISTORY, CF_APPROVAL_QUEUE                 │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│            OUTER LOOP — cron-scheduled (hourly default; configurable per goal)        │
│                                                                                       │
│        ASSESS  ───►  PLAN  ───►  BUILD  ───►  COMMUNICATE  ──┐                       │
│           ▲                                                   │                       │
│           └───────────────────────────────────────────────────┘                       │
│                                                                                       │
│   Each pass: read goal status, decide what to do next, dispatch, report.              │
└──────────────────────────────┬──────────────────────────────────────────────────────┘
                               │
            ┌──────────────────┼──────────────────┐
            ▼                  ▼                  ▼
   ┌───────────────┐  ┌────────────────┐  ┌──────────────────┐
   │  Atomizer     │  │  Planner       │  │  Reflector       │
   │  (atomic?)    │  │  (decompose to │  │  (post-mortem    │
   │               │  │   sub-DAG)     │  │   on failures)   │
   └───────┬───────┘  └───────┬────────┘  └──────────────────┘
           │                  │
           ▼                  ▼
        ┌──────────────────────────────────────────────────────┐
        │             EXECUTOR FLEET (~25 agent classes)         │
        │  Content · Publisher · Community · Outreach · Podcast │
        │  SEO · GEO · Influence · Ad-pacer · Referral · ...    │
        └──────────────────────────┬───────────────────────────┘
                                   │
                                   ▼
        ┌──────────────────────────────────────────────────────┐
        │            AGGREGATOR + EVALUATOR LAYER               │
        │   LLM-as-judge (G-Eval / AdaRubric) · Trajectory eval │
        │   Deterministic metric readback from product SoT      │
        └──────────┬─────────────────────────────────┬──────────┘
                   │                                 │
                   ▼                                 ▼
         ┌─────────────────────┐         ┌───────────────────────┐
         │   APPROVAL QUEUE    │         │   GUARDRAIL ENGINE    │
         │   (HITL interrupts) │         │   - Rate limit gate   │
         │                     │         │   - Brand voice gate  │
         │                     │         │   - Compliance gate   │
         │                     │         │   - Schema match gate │
         │                     │         │   - Circuit breakers  │
         │                     │         │   - Kill switches     │
         └──────────┬──────────┘         └───────────┬───────────┘
                    │                                │
                    ▼                                ▼
           ┌──────────────────────────────────────────────┐
           │            TOOL GATEWAY (MCP + Custom)        │
           │  Markifact · Hyper MCP · Windsor · Synapse    │
           │  Playwright · Browser-Use · custom adapters   │
           └──────────────────┬───────────────────────────┘
                              │
                              ▼
       (External world: APIs, web platforms, ad accounts, email,
        CRM, GitHub, social, Discord, Slack, calendar, etc.)

      ───────────────────────────────────────────────────────────

         ┌────────────────────────────────────────────────────┐
         │                 MEMORY LAYER (LLM Wiki)             │
         │   Skills · Channel cards · Reflections · Wins/Loss  │
         │   Brand voice · Contact graph · Persona library     │
         │   RocksDB backed; queryable via wiki_search() tool  │
         └────────────────────────────────────────────────────┘
```

---

## §2. The outer loop: ASSESS → PLAN → BUILD → COMMUNICATE

Borrowed from the Yopedia (`karpathy-llm-wiki`) pattern. Runs on a cron schedule (default hourly, per-goal override possible). Each pass is **idempotent**: a crash mid-pass is restarted from the last LangGraph checkpoint without corruption.

### 2.1 ASSESS phase (~10% of cycle time)

The Assessment Agent reads:
- Current goal status (predicate value, distance to threshold, trend over last 4 windows)
- All open experiment nodes (running, pending approval, blocked)
- All goal-relevant new signals since last cycle (platform changes, viral mentions, competitor launches, regulatory shifts)
- The reflection wiki for relevant prior lessons

Produces a structured `AssessmentReport`:

```yaml
assessment:
  goal_id: leapable-50k-q3-2026
  cycle_id: 2026-05-26T14:00:00Z
  predicate_status:
    current: 12_400
    target: 50_000
    delta_last_28d: +1240
    delta_last_28d_required_for_target: +9_400
    trend: behind
  open_experiments: 7
  approvals_pending: 2
  guardrail_violations: 0
  new_signals:
    - mention: "@swyx referenced Leapable in Latent Space podcast ep 247"
      relevance: high
      suggested_followup: thank + amplify + reach out for deeper conversation
    - platform: "X new bot-detection rollout; reduce posting cadence to 4/day"
      relevance: medium
      suggested_followup: update channel card; pacing reduced
  reflections_loaded: 14
  recommended_focus_for_planning: "compound on Latent Space mention; recover from missed Tuesday cohort"
```

### 2.2 PLAN phase (~30% of cycle time)

The Planner agent consumes the Assessment Report and (a) the goal-spec, (b) the Reflection wiki, (c) the current capacity envelope (autonomy slider, budget remaining, approved channels). Output: a **DAG of experiment nodes** to add to the open queue.

Each node carries the `expected_metric_impact` block (`01_NORTH_STAR.md §4`) so the worthlessness test can run pre-dispatch.

The Planner uses **ADaPT** recursion: it does not pre-decompose to atomic actions. It produces medium-grain nodes ("launch a PH on Thursday with 25 warm supporters"). The Atomizer decides per node whether to dispatch or decompose further.

### 2.3 BUILD phase (~50% of cycle time)

The Atomizer pulls open nodes from the queue and routes each:
- If the node is **atomic** (can be done by one Executor in one tool sequence) → dispatch to the matching Executor.
- If the node is **non-atomic** → invoke the Planner recursively on this node only, producing child nodes; insert them into the queue; mark the parent as `awaiting_children`.

Executors run **in parallel** where dependencies permit (the DAG carries `blocks` / `blocked_by`). Concurrency cap per Executor class is set by the channel's rate-limit card.

Each Executor's output is consumed by the Aggregator/Evaluator (§3.4). Results bubble up the DAG: child outputs aggregate into parent context; parent outputs aggregate into the goal-level cycle report.

### 2.4 COMMUNICATE phase (~10% of cycle time)

The Communicator Agent writes:
- A per-cycle **status update** to the goal's running log (markdown, append-only)
- A per-cycle **operator digest** if the operator has digest-mode enabled (Slack/email/CLI notification)
- New entries into the LLM Wiki: any structured reflections from this cycle's wins and losses
- Updates to channel cards based on observed performance

The cycle ends. The next cron tick begins.

---

## §3. ROMA recursive control loop

Borrowed from the ROMA framework. At each node in the goal-execution DAG, the same four-step loop runs:

### 3.1 Atomizer

Decides: is this node atomic (one Executor can complete it) or non-atomic (needs further decomposition)? Uses a classifier:

- Atomic if: the action maps to a single tool sequence in a single channel.
- Non-atomic if: the action requires coordination across ≥2 channels, OR requires evidence collection before execution, OR carries dependencies that need explicit ordering.

The Atomizer's classification is conservative — when in doubt, decompose. Cheap mistakes cost less than over-broad Executor failures.

### 3.2 Planner (per non-atomic node)

For a non-atomic node, the Planner produces a **MECE** (Mutually Exclusive, Collectively Exhaustive) child DAG: each child is independently dispatchable; together they fully realize the parent.

Each child carries the same shape as a top-level experiment node — including `expected_metric_impact`. If the parent's impact = 200 incremental WAUs, and the planner proposes four children, the children's deltas should sum to ~200 (allowing for risk discount).

### 3.3 Executors

One Executor per atomic node. The Executor:
1. Loads relevant Memory (channel card, brand voice, recent reflections).
2. Plans the tool sequence.
3. Executes through the Tool Gateway (gated by Guardrail Engine).
4. Returns a structured `ExecutionResult { artifacts, side_effects, observed_metrics, errors, cost }`.

Parallel execution within sibling nodes is encouraged (Devin "Managed Devins" pattern, AgentHub multi-branch DAG pattern).

### 3.4 Aggregator + Evaluator

Children's `ExecutionResult`s are aggregated to the parent. The Evaluator scores the aggregate output across:

1. **Deterministic metrics** — direct SoT readback (the only signal that gates predicate movement).
2. **LLM-as-judge rubrics** (G-Eval / AdaRubric) — quality of artifacts that don't have deterministic measures (brand voice, helpfulness on a Reddit thread, tone of an outreach email).
3. **Trajectory evaluation** — did the Executor use the right tools, in the right order, without waste?

Aggregated, scored output rolls up. If the parent succeeds, the parent's parent receives the aggregated artifact and the cycle continues. If it fails (LLM-as-judge below threshold, or deterministic metric uncertain), Reflexion kicks in.

---

## §4. Reflexion across attempts

Every parent-level failure produces a structured reflection (`02_FIRST_PRINCIPLES.md §15`). The reflection is **stored in the Memory wiki** keyed by `(channel, tactic, parent_node_type)`. Next attempt at a similar parent inherits the reflection automatically.

Reflexion has a retry cap. The default is **3 attempts** per parent node within a single cycle. After 3 failures, the parent is marked `BLOCKED_NEEDS_HUMAN` and surfaces to the operator with the reflections attached.

This bounds the cost of stuck loops while preserving the learning across attempts.

---

## §5. Memory layer — the LLM Wiki

Borrowed from Karpathy's LLM Wiki gist (`karpathy/442a6bf555914893e9891c11519de94f`) + **Leapable as the production-grade backend** for expert-vault queries. The Memory is **not a chat history**. It is a structured, queryable, append-only knowledge base of:

- **Skills** — reusable playbooks for specific tactics ("how to launch on Product Hunt", "how to engage on r/MachineLearning", "how to pitch Latent Space")
- **Channel cards** — per-channel operational state (rate limits, current cadence, brand voice deltas, common pitfalls, last performance)
- **Reflections** — every win/loss post-mortem, indexed by tactic and channel
- **Wins log** — what worked, with quantified deltas (used by Planner as evidence)
- **Brand voice** — the operator's voice signature, updated as the operator gives corrections
- **Contact graph** — every external person touched, with tier, last-touched timestamp, last-signal, and notes
- **Persona library** — the operator's personae (if running multi-persona accounts) with per-persona voice + posting history

The wiki is markdown files (or RocksDB-backed JSON, see `16_TECH_STACK.md`). Every agent reads relevant wiki pages before acting; every agent writes new pages or updates existing ones after acting. A weekly Wiki Gardener agent dedupes, cross-references, and prunes stale entries.

The wiki tool surface:

```
wiki_search(query, top_k=10) -> [WikiPage]
wiki_get(path) -> WikiPage
wiki_update(path, diff, change_note) -> WikiPage
wiki_create(path, content, frontmatter) -> WikiPage
wiki_link(from_path, to_path, relation) -> Edge
```

---

## §6. State persistence and durability

The system survives:
- Anthropic API outages
- Daemon restarts
- Operator workstation reboots
- Multi-day pauses
- Cross-version upgrades

How: LangGraph **durable execution + checkpointing** into RocksDB. Every node's state is serialized at every meaningful transition. On restart, the OuterLoop reads the last checkpoint and resumes mid-cycle. Idempotency keys on every external action prevent double-publishing.

RocksDB column families relevant to the marketing platform:
- `CF_GOALS` — active and historical goal specs
- `CF_GOAL_CYCLES` — every assess→plan→build→communicate pass with timestamps
- `CF_EXPERIMENT_NODES` — the DAG state per goal
- `CF_APPROVAL_QUEUE` — pending HITL items, with state, requester, deadline, decision
- `CF_REFLECTIONS` — Reflexion memory
- `CF_WIKI` — LLM Wiki pages (or filesystem-backed with this CF as index)
- `CF_CONTACTS` — contact graph + tier
- `CF_AUDIT_LOG` — append-only event log (every agent action, with cost, tool, outcome)

---

## §7. Approval queue + autonomy slider integration

When a node's action class is `REQUIRE_APPROVAL` (per the goal's autonomy spec), the Executor:

1. **Serializes** the full action context (intended tool call, payload, expected effect, brand-voice score, cost estimate, reflections referenced).
2. **Enqueues** to the approval queue.
3. **Pauses** the parent DAG branch (other branches continue).
4. **Notifies** the operator via the configured channel (Slack, email, CLI dashboard).

Operator decision options:
- `APPROVE` — Executor resumes with the original action.
- `APPROVE_WITH_EDIT` — operator edits the payload (most common for outbound copy); Executor uses the edited version.
- `REJECT_TRY_AGAIN` — Executor retries with operator's stated correction.
- `REJECT_KILL` — node and its descendants marked failed; reflection filed.
- `ESCALATE_LATER` — keep pending; remind in N hours.

Approval queue auto-rejects pending items after `goal.guardrails.approval_ttl` (default 48h). Auto-rejection counts as a soft failure, not a kill.

---

## §8. Guardrail engine

Every external tool call goes through:

```
Executor → ToolCallProposal → GuardrailEngine → [allow|deny|require_approval] → ToolGateway → External
```

The Guardrail Engine runs these checks in order; first denial wins:

1. **KillSwitchCheck** — global, per-goal, per-channel flags (O(1) cached for ≤2s).
2. **ComplianceCheck** — EU AI Act disclosure on EU outbound, CAN-SPAM unsubscribe + physical address on US email, CASL consent on Canadian, etc.
3. **RateLimitCheck** — per-channel token bucket sized 30% below platform's published threshold.
4. **BrandVoiceCheck** — for content actions, run G-Eval brand-voice score; deny if below threshold.
5. **SchemaCheck** — payload conforms to the channel's expected schema.
6. **BudgetCheck** — proposed spend within goal's daily and per-experiment ceilings.
7. **AutonomyCheck** — action class is in `auto_proceed_for` list; otherwise route to approval queue.

Each check is implemented as a deterministic function. None of them call the LLM (they may use small fine-tuned classifiers for brand-voice, but no frontier-model judgment in the gate path itself — too slow, too expensive, too unreliable).

---

## §9. Tool gateway

The Tool Gateway is the **only path** from agents to external systems. It exposes a unified MCP-style interface over heterogeneous backends:

| Backend type | Examples | When used |
|---|---|---|
| **MCP servers** (preferred) | Markifact (300+ ad/CRM ops), Hyper MCP (200+ integrations), Windsor (325+ data sources), 0nMCP (995+ tools), Synapse (computer-use), **Leapable** (vault search + memory) | Whenever the platform exposes a real, supported MCP server |
| **Bespoke API adapters** | Custom GitHub, custom Discord webhook, custom Slack bot | Where MCP coverage is shallow but operator owns the integration |
| **Computer-use** | Synapse + Playwright + Browser-Use | When the platform has no API or the API is gated beyond the operator's tier (LinkedIn beyond Sales Nav, etc.); see `06_COMPUTER_USE_PLAYBOOK.md` |
| **Operator-mediated** | "Send this to operator's iMessage for hand-forward" | Tier-3 outreach; legal documents; never automated |

The Gateway records every call in `CF_AUDIT_LOG` with idempotency key, cost, latency, outcome, and side-effects.

---

## §10. Cost discipline

Two cost dimensions: **dollars** (paid spend, third-party tool costs) and **tokens** (LLM inference).

### 10.1 Dollar pacing

The Cost Pacer agent (`02_FIRST_PRINCIPLES.md §16`) is mechanical, not LLM-based. It maintains in-memory rolling sums:
- Last 1h spend per channel
- Last 24h spend per channel
- Last 7d spend per channel
- Last 30d spend per channel

Before any spending action, the Pacer's `permits_spend(amount, channel)` is called. Over-limit returns `EXHAUSTION` trip code.

### 10.2 Token pacing

Per-agent and per-cycle token budgets. Implemented as token-bucket on the LLM client wrapper. Over-budget runs trip `BudgetFuse` (AgentCircuit pattern).

### 10.3 Model routing

The Tool Gateway exposes both Opus-tier (reasoning-heavy) and Haiku-tier (formatting / extraction / classification) models. Default routing:
- Planner, Reflector, Aggregator, Evaluator → Opus (high stakes)
- Content Writer, Outreach Drafter → Sonnet
- Schema validation, classification, simple extraction → Haiku
- Brand-voice scorer, compliance check → fine-tuned classifier (no LLM)

Cost-aware model swapping is a cycle-end optimization, not a per-call decision.

---

## §11. Circuit breakers (mechanical)

Implemented as wrappers around the OuterLoop and around the Tool Gateway:

| Breaker | Trigger | Implementation |
|---|---|---|
| `Fuse` | 3 consecutive identical tool calls → loop detected | djb2 hash of (tool name + payload); count repeats; trip at 3 |
| `BudgetFuse` | Per-goal dollar or token budget exceeded | Atomic decrement on each call; trip on negative |
| `TimeoutFuse` | Single experiment node runs longer than its `time_budget_hours` | `started_at` + timeout; trip if exceeded |
| `StagnationFuse` | Predicate metric did not move for N consecutive cycles despite spend | Cycle-level comparison; trip at N (default 6 hourly cycles = ~one workday with no movement) |
| `GoodhartFuse` | Predicate metric vs. reality signal divergence > 2x over 14d | Daily auditor; trip on divergence |
| `BrandFuse` | Published content scores brand-voice violation post-hoc | Daily LLM-as-judge sweep; trip on violation |
| `ReputationFuse` | Sentiment / mention-tone delta on operator's public surfaces > N negative in 24h | External sentiment scan; trip on threshold |

Trip = halt all new external actions on the goal; surface incident to operator; require explicit `/goal resume` to continue.

---

## §12. Kill switches (operator-controlled)

Two layers:

### 12.1 Per-goal kill

```
/goal kill <goal_id>
```

Marks the goal `OPERATOR_PAUSED`. All open experiment nodes drained or canceled; in-flight actions completed where reversible-safe, otherwise aborted with best-effort cleanup. Final status report written.

### 12.2 Global kill

```
/marketing kill --reason="..."
```

All goals paused. All Executors return at next safe checkpoint. All approval-queue items frozen. Useful during incidents or PR crises.

Both layers are implemented as flag-checks at the runtime supervisor, **outside** the LLM agent loop. The LLM cannot reason past a kill flag. (Sakura Sky / AgentOS pattern.)

---

## §13. Concurrency model

The fleet uses:
- **Outer loop** — single-threaded sequential per goal (one ASSESS→PLAN→BUILD→COMMUNICATE cycle at a time).
- **Inner DAG** — parallel Executor dispatch up to per-channel concurrency caps.
- **Cross-goal** — goals run concurrently; the goal-level scheduler ensures shared resources (e.g., the same operator-owned X account) aren't double-booked.

Per-channel concurrency caps are set in the channel card and respect the platform's published rate limits with a 30% safety margin.

---

## §14. Observability + tracing

Every agent step emits a trace span (LangSmith / OpenTelemetry compatible). Spans carry:
- `goal_id`, `cycle_id`, `node_id`, `agent_class`
- Tool calls with inputs, outputs, costs, latencies
- LLM call inputs (cached / streamed), outputs, token counts, cost
- Approval queue activity
- Guardrail decisions (allow / deny / require_approval) with reason
- Errors with full stack traces

Operator dashboard shows:
- Active goals with progress bars
- Pending approvals
- Last 24h spend by goal and channel
- Open circuit-breaker alerts
- Last hourly cycle summary

A weekly report compiles cycle outcomes into operator-readable form (markdown), highlighting biggest movers and biggest reflexions.

---

## §15. Multi-product, single-fleet

The fleet is shared across all three products (Leapable, Synapse, Polis). Goals scope tools, content, and contacts to a product. The same Content Writer agent class serves all three; it loads the product-specific brand voice and channel cards at start.

This means the operator does not need three separate marketing setups. One platform, three goal contexts.

---

## §16. AgentHub-style attempt DAG

Every goal-execution session is recorded as a **sprawling DAG of attempt commits**, AgentHub-style. Each attempt is a node; the DAG records:
- Parent attempt(s) it forked from
- Goal_id, cycle_id
- The agents that ran
- The outcome score
- The artifacts produced

This is **distinct** from the operational DAG (which is the live experiment graph). The attempt DAG is the **historical record** that the Planner queries when it asks "have we tried something like this before?" Karpathy's AgentHub demonstrated this works for autoresearch; we apply it to autonomous marketing.

The wiki indexes the attempt DAG by `(channel, tactic, outcome_class)` so the Planner can ask "show me every PH launch attempt; which subset of variables correlated with success?" and get a structured answer.

---

## §17. Cross-references

- Agent class definitions: `05_AGENT_FLEET_DESIGN.md`
- Tool surface: `16_TECH_STACK.md`
- Computer-use specifics: `06_COMPUTER_USE_PLAYBOOK.md`
- Approval queue UX: `19_OPERATIONS_AND_CADENCE.md §3`
- Circuit-breaker and kill-switch details: `20_RISK_AND_FAILURE_MODES.md`
- Memory schema: `16_TECH_STACK.md §5`
- Goal spec: `22_GOAL_SPEC_FORMAT.md`
