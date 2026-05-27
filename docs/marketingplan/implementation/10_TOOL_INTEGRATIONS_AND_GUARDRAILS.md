# Tool Integrations + Guardrail Engine Build Order

> The single most leveraged engineering subsystem: the Tool Gateway with the Guardrail Engine. Every external action goes through this layer. If the gates are weak, the whole system fails (per `02 §3` — trust the harness). This doc specifies what integrations land in which phase, how the Guardrail Engine is implemented, and the mechanical-not-LLM principle that makes the system safe.

**Source spec docs:** `04_SYSTEM_ARCHITECTURE.md §8 §9 §10 §11 §12`, `06_COMPUTER_USE_PLAYBOOK.md`, `14_COMPLIANCE_AND_ETHICS.md`, `16_TECH_STACK.md §5 §11`, `20_RISK_AND_FAILURE_MODES.md §1 §2 §3`.

---

## §1. The flow (every external action)

```
Executor agent
   │
   ▼  ToolCallProposal
GuardrailEngine
   │
   ├─► KillSwitchCheck       (O(1) cached flags)
   ├─► ComplianceCheck       (jurisdiction-aware)
   ├─► RateLimitCheck        (token-bucket per channel × account)
   ├─► BrandVoiceCheck       (Gemma 4B classifier; for content actions)
   ├─► SchemaCheck           (payload conforms to channel's expected schema)
   ├─► BudgetCheck           (Cost Pacer enforces dollar + token caps)
   └─► AutonomyCheck         (action class in auto_proceed_for, else approval queue)
   │
   ▼  [allow | deny | require_approval]
ToolGateway
   │
   ├─► MCP server (Markifact, Hyper, Windsor, leapable, etc.)
   ├─► Computer-use (Synapse, Playwright, Browser-Use)
   ├─► Bespoke adapter (Gmail, MS Graph, gh CLI, Anthropic SDK, Stripe)
   └─► Operator-mediated (send to Slack for hand-forward; Tier-3)
   │
   ▼
External world
   │
   ▼  Response
CF_AUDIT_LOG entry (with payload + outcome + idempotency key)
```

Every check is **deterministic, mechanical, sub-100ms**. None call an LLM. The Brand Voice check uses a fine-tuned small classifier (Gemma 4B). All other checks are pure logic.

---

## §2. The Guardrail Engine — implementation order

### 2.1 Phase 0 — skeleton + KillSwitch + Schema + Audit

Phase 0 builds the engine shell with two gates active:
- **KillSwitchCheck** (works even with no other gates)
- **SchemaCheck** (Pydantic-based; refuses malformed tool calls)

Plus the **CF_AUDIT_LOG emission** on every call. Even mock-channel calls log.

### 2.2 Phase 1 — BrandVoice + RateLimit + Autonomy + Compliance (US-only subset)

Phase 1 adds:
- **BrandVoiceCheck** (Gemma 4B fine-tuned per `02_PHASE_1 §2.3`)
- **RateLimitCheck** (token-bucket per channel × account; reads `platform_limits.yaml`)
- **AutonomyCheck** (consults action-class allowlist per goal-spec's `autonomy.auto_proceed_for`)
- **ComplianceCheck** (CAN-SPAM only; Phase 1 single channel is US-only)

### 2.3 Phase 2 — BudgetCheck + multi-channel compliance + full schema coverage

Phase 2 adds:
- **BudgetCheck** (Cost Pacer mechanical; per-goal daily + monthly caps)
- ComplianceCheck extended: 8 channels now active; per-channel jurisdiction logic
- SchemaCheck extended: per-channel payload schemas for all 8 channels

### 2.4 Phase 3 — full jurisdiction coverage + Tier-3 doctrine gates

Phase 3 adds:
- ComplianceCheck extended: full 6-jurisdiction coverage (US, EU, CA, UK, AU, JP) per `14 §1 §2`
- New gate: **Tier3DoctrineCheck** — refuses any tool call that would automate outbound to a Tier-3 contact (per `02 §19` + `10 §9`)
- New gate: **PersonaLeakageCheck** — refuses tool calls that would publish persona-X content from persona-Y's account

### 2.5 Phase 4 — observability + bench-delta gate

Phase 4 adds:
- LangSmith span emission on every guardrail decision (observable in dashboards)
- Optional pre-flight bench-delta check for changes to gate behaviors

---

## §3. The seven kill-switch trip codes

Per `02 §17` + `20 §1`. Implemented as O(1)-checkable cached flags at the runtime supervisor (≤2s TTL). Cannot be bypassed by the agent.

| Trip code | Trigger | Reset path |
|---|---|---|
| `STAGNATION` | No predicate movement for N planning cycles despite spend (default N=6 hourly cycles) | Reflector post-mortem; operator decides resume/replan/kill |
| `OVERRISK` | Aggregate risk score (compliance + brand + reputation) over threshold | Operator review of last 24h |
| `EXHAUSTION` | Token or dollar budget hit | Operator budget extension or scope reduction |
| `RUNAWAY_LOOP` | Same action attempted >3 times with same input hash (djb2 hash + counter) | Reflexion forces decomposition |
| `GOODHART_DETECTED` | Predicate vs reality divergence > 2x over 14d | Mandatory operator review |
| `BRAND_VIOLATION` | G-Eval brand-voice score < threshold on published content (post-hoc daily sweep) | Pull/retract + operator review |
| `OPERATOR_EMERGENCY_STOP` | Operator panic hotkey or `marketing kill` | Operator explicit resume |

Implementation: each trip code is a flag in `CF_KILL_SWITCHES`. The supervisor caches them with 2s TTL. Every Executor checks the relevant flag before dispatching to the Tool Gateway. Trip = halt all new external actions on the scope.

---

## §4. AgentCircuit primitives (per `20 §2`)

The fleet wraps every LLM call and external tool call with a stack of mechanical guards:

| Guard | Behavior | Implementation |
|---|---|---|
| **Fuse** (loop detection) | Hashes (input, tool, payload); trips if same hash seen >3 times across cycle | djb2 hash + counter in CF_GUARD_STATE |
| **BudgetFuse** | Atomic decrement on every LLM call by token count; tripped on negative | Pacer-managed token bucket |
| **TimeoutFuse** | Wall-clock timer per node; abort if exceeds `time_budget_hours` | `asyncio.wait_for` or equivalent |
| **StagnationFuse** | Cycle-level: tracks predicate movement; trips when 0-delta for N consecutive cycles | Compares CF_GOAL_CYCLES record values |
| **GlobalBudget** | Cross-goal token + dollar pool; tripped on shared exhaustion | Atomic counter |
| **Sentinel** | Pydantic schema validation on every tool output; tripped on schema mismatch | Pydantic per-tool output schema |
| **Medic** | LLM-driven repair on schema-mismatch outputs (cap 2 retries) | Sonnet-call with reformat prompt |

---

## §5. Tool Gateway — backend integration order

### 5.1 Phase 0

| Backend | Status | Why first |
|---|---|---|
| Anthropic SDK | Required Day 1 | The LLM is the agent's brain |
| leapable MCP | Required Day 5 | Memory layer for the LLM Wiki |
| Mock channel sink | Required Day 9 | End-to-end testing without external risk |

### 5.2 Phase 1

| Backend | Phase 1 use | Adapter file |
|---|---|---|
| **Synapse MCP** | Operator's X account (single channel) | `tool_gateway/synapse_adapter.py` |
| OS keychain | OAuth token retrieval | inline to other adapters |

### 5.3 Phase 2

| Backend | Phase 2 use | Notes |
|---|---|---|
| **gh CLI** | GitHub channel (D3 GitHub Maintainer) | API-preferred |
| **Gmail / MS Graph** | Email outbound (D4 Outreach Drafter) | API-preferred; CAN-SPAM wrappers active |
| **Hyper MCP** | LinkedIn scraping (S1 Audience Sorter), CRM ops, image-gen for P3 | 200+ integrations |
| **Markifact MCP** | Paid ad platforms (O4 Ad Operator, when operator approves) | 300+ ops |
| **Synapse MCP** extended | LinkedIn / Discord / HN / Reddit / TikTok logged-in actions | Operator-account-tier only |
| **Playwright** | Parallel public-surface scraping (S2 Signal Listener) | Read-only; residential IP rotation; no operator accounts |
| **Stripe SDK** | Per-Creator referral attribution + payout (O3 Referral Engineer) | Direct API |

### 5.4 Phase 3

| Backend | Phase 3 use | Notes |
|---|---|---|
| **Windsor MCP** | 325+ data sources for cross-platform analytics aggregation | Data-warehouse-style query |
| **SearchAtlas MCP** | 112 tools for SEO/GEO/local-SEO (O1 + O2) | Generative Engine Optimization specifically |
| **0nMCP** | 995+ tools — fallback for long-tail integrations | Use only when neither Markifact nor Hyper covers |
| **Browser-Use / OpenClaw** | Goal-driven browser automation (partner inquiry forms, etc.) | Interactive flows |
| **Synapse computer-use** extended | TikTok, IG Polis operations (P3, D1, D2) | Polis launch dependency |

### 5.5 Phase 4

| Backend | Phase 4 use | Notes |
|---|---|---|
| **LangSmith** | Every agent span traced; replay capability | Operator-hosted |
| **Prometheus + Grafana** | Dashboard backend + alerts | Operator-hosted |
| **S3-compatible archive** | Audit log archive + RocksDB snapshots | Per `16 §11` backup strategy |

---

## §6. Per-jurisdiction compliance wrappers (Phase 2 → 3)

Per `14 §1 §2 §3`:

### 6.1 The wrapper pattern

```python
# tool_gateway/email_eu_wrapper.py (sketch)
def send_email_eu(recipient: str, subject: str, body: str, campaign_id: str) -> EmailResult:
    # Phase 3: also called from generic send_email when jurisdiction resolves to EU
    require_balancing_test_signed(campaign_id)
    body_with_disclosure = inject_ai_disclosure(body)
    body_with_unsubscribe = inject_unsubscribe_block(body_with_disclosure, recipient)
    body_with_address = inject_physical_address(body_with_unsubscribe)
    # idempotency + audit
    idempotency_key = uuidv7()
    audit_log.write(...)
    result = _native_send(recipient, subject, body_with_address, idempotency_key)
    return result
```

The Executor never calls `_native_send`. The wrapper is the only path. Per `02 §3`: trust the harness.

### 6.2 Per-jurisdiction matrix

| Jurisdiction | Required injections | Required behavior |
|---|---|---|
| **US (CAN-SPAM)** | Physical address; unsubscribe link; truthful sender | Honor unsubscribe ≤10 business days |
| **EU/EEA (GDPR)** | Lawful basis logged; balancing test ref; privacy notice link | Honor right-to-erasure + right-to-access within 30d |
| **EU/EEA (EU AI Act Art. 50)** | AI-assistance disclosure on AI-drafted content | Disclosure injected by wrapper |
| **CA (CASL)** | Consent verification; identification; unsubscribe | Verify consent before sending |
| **UK (UK GDPR mirror)** | Same as EU | Same as EU |
| **AU (Spam Act)** | Consent; identification; unsubscribe | Same |
| **JP (APPI)** | Disclosure on data use; opt-out for direct marketing | Disclosure on first contact |

---

## §7. Account registry enforcement

Per `06 §3.1` + `09 §4 accounts.yaml`:

- Every Executor tool-call references an `account_id`
- `AccountCheck` (a sub-check inside the gate) verifies:
  - `account_id` is in `accounts.yaml`
  - The calling agent class is in that account's `agents_allowed`
  - The account is `active: true`
- Failure → `ACCOUNT_NOT_REGISTERED` or `AGENT_NOT_AUTHORIZED_FOR_ACCOUNT` error

No exceptions. No bypass. New account must be added to `accounts.yaml` by the operator before use.

---

## §8. The mechanical-not-LLM principle

Per `02 §3` (trust the harness, not the model):

| Wrong approach | Right approach |
|---|---|
| Add "please don't double-post" to a system prompt | Implement deterministic Redis check in publish-tool wrapper |
| Add "remember EU AI Act disclosure" to the writer prompt | Inject disclosure in `send_email_eu` wrapper |
| Add "stay on brand voice" to writer instructions | Run Gemma 4B classifier in Evaluator before publish |
| Train the agent on "don't post when rate-limited" | Token bucket in Cost Pacer + RateLimitCheck before tool dispatch |
| Hope the agent respects the kill switch | O(1) cached flag check at supervisor before every Executor dispatch |

The LLM is the agent's brain. The harness is the law. The brain can be brilliant or stupid on any given run; the law is the same every run.

---

## §9. Token + dollar pacing (Cost Pacer specifics)

Per `02 §16` + `05 §6.4`:

### 9.1 Token pacing

```python
# guardrails/budget_check.py (sketch)
class TokenBucket:
    def __init__(self, capacity: int, refill_per_minute: int):
        self.capacity = capacity
        self.tokens = capacity
        self.refill_per_minute = refill_per_minute
        self.last_refill = now()

    def consume(self, count: int) -> bool:
        self._refill()
        if self.tokens >= count:
            self.tokens -= count
            return True
        return False

    def _refill(self):
        elapsed_minutes = (now() - self.last_refill).total_seconds() / 60
        refill = int(elapsed_minutes * self.refill_per_minute)
        self.tokens = min(self.capacity, self.tokens + refill)
        self.last_refill = now()

# One bucket per (goal_id, agent_class) and one global bucket
```

Over-budget run trips `BudgetFuse`.

### 9.2 Dollar pacing

- Per-channel × goal × 1h / 24h / 7d / 30d rolling sums in memory + persisted to `CF_CHANNEL_STATE`
- `permits_spend(channel, amount_usd) → bool` called pre-tool by the Guardrail Engine
- Over-limit → `EXHAUSTION` trip code

### 9.3 Per-experiment ceiling

- No single experiment > `monthly_ceiling_usd / 10` without escalation
- Marginal CAC tripwire: if last-7d acquired-user count divided by last-7d spend exceeds `ltv_floor / 3`, pause + escalate

---

## §10. Anti-detection enforcement (per `06 §6`)

The fleet's anti-detection patterns are mechanical, not aspirational:

| Detection signal | Mechanical enforcement |
|---|---|
| Repeated phrasing across replies | Lint check on Comment Triage outputs: deny if same first-word streak >2 |
| Identical link UTMs across all posts | UTM injection wrapper varies per-channel |
| Replies under 8 seconds after a post | Min delay 30s enforced in publish wrapper; sampled from distribution |
| Engagement only during one timezone band | Stochastic distribution across operator's active hours per channel card |
| AI-detection signatures in copy (em-dash, "delve", etc.) | Brand Voice Scorer scores AI-likelihood; rewrite above threshold; forbidden-phrases regex deny-lists |
| Mass-engaging only on follower-of-follower accounts | Audience Sorter (S1) mixes engagement: peers, larger, smaller, randoms |

---

## §11. Operator panic hotkey

Per `06 §2.1` + `20 §3.5`:

```
Ctrl+Alt+Shift+P  →  Synapse-mediated kill switch
```

- Halts all Synapse activity within 50ms
- Triggers `OPERATOR_EMERGENCY_STOP` kill switch flag
- All Executors halt at next safe checkpoint
- Approval queue locked
- Operator restart via `marketing resume`

Implementation: Synapse registers the hotkey; on press, calls a halt RPC on the orchestration host's supervisor, which flips the kill flag.

---

## §12. Acceptance criteria — Tool Gateway and Guardrail Engine

These are the per-phase tests proving the gates work:

| Phase | Test |
|---|---|
| Phase 0 | KillSwitchCheck stops all execution within ≤2s; SchemaCheck refuses 100% of malformed inputs; CF_AUDIT_LOG captures every call with idempotency key |
| Phase 1 | BrandVoiceCheck refuses drafts below 0.85 threshold; RateLimitCheck respects platform limits; mock multi-attempt test does not double-publish |
| Phase 2 | BudgetCheck stops all over-budget calls; Cost Pacer logs every denial; Marginal CAC tripwire fires when synthetic test exceeds threshold |
| Phase 3 | ComplianceCheck honors all 6 jurisdictions; Tier3DoctrineCheck refuses automated Tier-3 outbound; PersonaLeakageCheck refuses cross-persona content |
| Phase 4 | LangSmith captures every guardrail decision; operator can replay any historical refusal with context |

Each gate's tests live in `tests/test_guardrail_<gate>.py`.

---

## §13. Cross-references

- Architecture for the gates: `docs/marketingplan/04_SYSTEM_ARCHITECTURE.md §8 §9 §10 §11 §12`
- Computer-use rate caps + accounts: `docs/marketingplan/06_COMPUTER_USE_PLAYBOOK.md`
- Compliance regimes: `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md`
- Tech stack tool inventory: `docs/marketingplan/16_TECH_STACK.md §5`
- Kill-switch + circuit-breaker: `docs/marketingplan/20_RISK_AND_FAILURE_MODES.md`
- Doctrine — trust the harness: `docs/marketingplan/02_FIRST_PRINCIPLES.md §3`
- Phase build sequences: `01_PHASE_0_FOUNDATIONS.md` through `05_PHASE_4_PRODUCTION_HARDENING.md`
- Repository layout (where each adapter file lives): `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md §1`
