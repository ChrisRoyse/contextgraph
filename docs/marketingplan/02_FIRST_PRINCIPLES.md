# 02 — First Principles (Doctrine)

> Read once at session start. Re-read when tempted to cut a corner. These are the rules the agent fleet (and the operator) cannot break. Tactics change weekly; the architecture changes quarterly; the doctrine never changes.

---

## §1. Goal-driven, not pipeline-driven

The system does not run a fixed marketing pipeline. The system **executes against an operator-supplied goal** until the goal's success predicate is `TRUE` for four consecutive windows or a hard guardrail trips. Channels, tactics, content formats, ad mixes, even agent topology are **emergent consequences of which experiments moved the goal-metric the most**.

A "marketing calendar" is a snapshot, not a plan. The plan is the goal. The calendar is whatever the Planner agent decided this week to maximize expected predicate movement.

---

## §2. Karpathy's autonomy slider

Every external action (anything that touches a third-party platform, a customer, or a public surface) has an autonomy tier:

| Tier | Behavior | Used for |
|---|---|---|
| `AUTO` | Agent executes, logs, no human review needed | Reads, internal analysis, A/B variant generation, draft content into staging |
| `LOG` | Agent executes; flags sampled audit for human review later | Low-cost reversible writes: organic post drafting (when brand-voice-passes), internal CRM updates, follow-ups within established threads |
| `REQUIRE_APPROVAL` | Agent proposes; pauses; waits for human approval before executing | Anything irreversible, high-cost, or first-time: paid ad creative, outbound to a named account, public statement, product proposal, partnership outreach |
| `ESCALATE` | Agent surfaces to operator with options + recommendation; operator's call | Brand-risk-sensitive cases, contradictions in evidence, regulatory edge cases, anomalies the agent isn't confident enough to recommend on |

The goal spec sets a default autonomy level and overrides per action class. **The operator can move any action class up or down the slider as confidence accumulates.** This is non-negotiable. A 99.7%-approved action class should be moved to LOG; a 25%-overridden action class should be moved to ESCALATE.

The slider starts conservative ("LOG for content, REQUIRE_APPROVAL for everything else") and only loosens with evidence.

---

## §3. Trust the harness, not the model

Borrowed verbatim from the Yopedia / `karpathy-llm-wiki` pattern.

The agent fleet **cannot enforce its own constraints**. Anything that matters — rate limits, brand voice, compliance, kill switches, budget caps, schema validation — is enforced by **shell-script-level or runtime-policy-level mechanical gates**, sitting outside the agent loop, that the model cannot reason past.

Example: "Don't post the same draft to LinkedIn twice in 48 hours" is enforced by a deterministic Redis check in the publish-tool wrapper, **not** by adding "please don't double-post" to a system prompt.

Example: "Every external email to an EU contact includes the EU AI Act disclosure line" is enforced by a templating wrapper around `send_email` that injects the disclosure, **not** by hoping the content agent remembered.

Example: "Brand voice score must be ≥0.85 before publish" is enforced by an evaluator agent and a deterministic gate, **not** by adding "stay on brand voice" to the writer agent's instructions alone.

The system prompt is a recommendation. The harness is a law.

---

## §4. Worthlessness test

If a proposed action does not move (or preserve) at least one number on the active goal's metric tree, **it does not ship**. This applies to every agent action — content piece, ad campaign, podcast pitch, partner outreach, blog post, ambassador hire, conference attendance.

The Planner agent rejects proposals without quantified expected impact. (`01_NORTH_STAR.md §4`.)

This is brutal. It kills a lot of "marketing activity" that feels productive but isn't. That is the entire point.

---

## §5. Reality is the verdict, not the dashboard

When the agent fleet asks "did experiment X work?", the answer comes from **the product's source of truth** (RocksDB, the SQL database, the event stream), not from a vendor analytics dashboard. Vendor dashboards lag, double-count, drop events, and silently change attribution models. Source-of-truth readback is the verdict.

Concretely, every experiment node carries `source_of_truth: rocksdb://contextgraph/cf_integrations` or equivalent. The Evaluator agent queries the SoT directly, not Mixpanel/Posthog/Amplitude.

Vendor dashboards are diagnostic tools. They never decide.

---

## §6. Generic primitives only

The agent fleet builds generic capabilities, not task-specific hacks. A `discord_engager` agent that knows how to operate well in *any* technical Discord beats nine specialized agents for nine specific Discords. The cost of specialization is fragility — when the channel evolves, the specialization breaks.

Borrowed from ContextGraph CLAUDE.md (GENERIC-ONLY rule). The fleet is allowed to specialize on the **inside of a tactic** ("how the agent operates on r/MachineLearning specifically") in playbooks, but the **agent code** stays generic and reads the playbook.

---

## §7. Reflexion memory, not amnesia

Every failed experiment leaves a **verbal reflection** in the goal's memory wiki. Next attempt at a similar experiment reads the reflection before planning. "Last time we ran a PH launch on a Tuesday after an OpenAI announcement and got buried" is a fact the fleet should know on attempt #2.

The mechanism is the Karpathy LLM Wiki pattern: the Memory Agent reads the run log, summarizes the win/loss with structured fields (channel, hypothesis, expected, actual, why-it-differed, retry-conditions), writes a `reflection.md` page, and cross-references it. The Planner queries the wiki by tactic and channel before proposing experiments.

Reflexion is the difference between an agent that re-tries identical failures and an agent that improves over time.

---

## §8. Recursive decomposition, only when needed (ADaPT)

The Planner does **not** decompose a goal into a giant flat list of every imaginable subtask up-front. It tries the goal-level execution first. If the Executor agents make progress, the Planner stays out of the way. **If and only if execution stalls or fails**, the Planner triggers a decomposition step, producing a tighter sub-DAG. This recurses as deep as needed.

The reason: pre-decomposition produces task graphs full of mid-level abstractions ("build awareness," "drive consideration") that don't map to atomic agent actions. ADaPT-style late binding produces task graphs that are atomic at the leaves and shallow at the root.

---

## §9. No mocks gate completion

An experiment is not "successful" because the agent reports success. It is successful when the **source of truth shows the metric moved**. If the metric didn't move, the experiment failed, regardless of how good the post looked or how many likes it got.

Vanity metrics (impressions, reach, follows, opens) are not allowed as completion criteria for any node. They can appear in reports as diagnostic signal; they cannot gate progression.

---

## §10. Compliance is an upstream gate, not a downstream review

CAN‑SPAM disclaimers, EU AI Act Article 50 AI-content disclosure, GDPR legitimate-interest documentation, CCPA opt-out, platform ToS rate limits — **all of these are pre-execution gates**, enforced by mechanical wrappers around the relevant tools. The fleet cannot ship an EU outbound email without the disclosure block because the `send_email_eu` tool wrapper refuses to send without it.

Compliance is never a post-hoc audit. It's a syntax error at the tool call.

---

## §11. Computer-use is real-human-shaped use

When the agent operates a web platform via Synapse + Playwright + Browser-Use (because the platform doesn't expose a real API, or because the API is gated to enterprise tiers), the agent operates **at human-shaped rates**, **with human-shaped pauses**, and **only on operator-owned accounts**. No multi-account farming. No simultaneous-session amplification. No buying impressions.

Specifics:
- Per-platform rate limits set 30%+ below the platform's published bot-detection thresholds.
- Action cadence stochastic, not periodic (real humans don't post every 47 minutes).
- One session per operator-owned account; account ownership documented in an internal registry.
- Computer-use is recorded as a screen capture for post-hoc audit on operator request.

This both keeps the operator's accounts alive and keeps the fleet ethically defensible.

---

## §12. The Iron Man suit, not the Iron Man robot

Karpathy: *"It's less like building flashy demos of autonomous agents and more building partial autonomy products."* The marketing platform is **augmentation, not replacement**. The operator's judgment is the ceiling on quality, not the agent's. The fleet handles the volume the operator could not personally cover, at the operator's brand voice, against the operator's chosen goals.

The autonomy slider moves right over months and years as the fleet earns trust on specific action classes. It never moves all the way right. Some classes (legal commitments, partner contracts, public crisis response) are permanently `ESCALATE`.

---

## §13. Slot identity: each agent owns one job

Borrowed from ContextGraph's "Slot identity is sacred" rule. Each agent class has **one** primary responsibility. The Content Writer doesn't also do scheduling. The Scheduler doesn't also do brand-voice evaluation. The Evaluator doesn't also do writing.

Why: cross-responsibility agents accumulate prompts. Prompts that try to do five things do all five badly. Specialized agents with one job stay tight.

Multi-step workflows are composed by the Orchestrator across single-job agents, not by stuffing one agent with five jobs.

---

## §14. Bench delta gate

A material change to any agent class (prompt rewrite, model swap, tool reorder, evaluator rubric update) ships only after a **bench delta** against a held-out evaluation set of past experiments. The delta must be:
- Non-regression on goal-relevant metrics (the change must not make experiments perform worse).
- No more than +5% cost-per-experiment increase without justification.

This prevents prompt-churn drift: a series of "small improvements" that each look fine but collectively degrade performance. The Reflexion-trained Cursor / Claude Code pattern: every meaningful agent change is a small commit with a measurable delta or it doesn't ship.

---

## §15. Failure documentation is the next agent's training data

When an experiment fails, the Reflector agent writes a structured post-mortem — not a long narrative, a tight structured block:

```yaml
experiment_id: ph-launch-2026-01-22
goal_id: contextgraph-50k-q3-2026
outcome: missed
expected: +200-400 new weekly_active_integrations
actual: +47
diagnostics:
  - launched_same_day_as_anthropic_claude_5_announcement
  - HN_front_page_held_by_anthropic_for_4_hours
  - PH_traffic_at_2pm_PT_lower_than_modeled
  - 22_warm_supporters_engaged_vs_planned_25
reflections:
  - "Pre-launch check: never launch within 24h of a major model-vendor announcement."
  - "Warm-supporter list needs +30% buffer over modeled engagement."
takeaways_filed_to: skills/launch-product-hunt.md
retry_conditions: yes_with_revised_pre-launch_check
```

The takeaways propagate into the relevant skill page. Future experiments under that skill inherit them automatically.

---

## §16. Cost discipline

Every active goal has a budget ceiling per goal-spec. The Cost Pacer agent runs continuously and enforces:

1. **Daily velocity cap** — no spending more than `goal.guardrails.budget.daily_velocity_cap_usd` in any 24h.
2. **Per-experiment ceiling** — no single experiment > `monthly_ceiling_usd / 10` without escalation.
3. **Marginal CAC tripwire** — if last-7d acquired-user count divided by last-7d spend exceeds `ltv_floor / 3`, pause and escalate.
4. **Token budget** — every agent run has a token cap; over-budget runs trip the BudgetFuse circuit breaker before they get to LLM hosts.

Cost overruns are not failures of intent; they are failures of mechanical limits. The Pacer is mechanical.

---

## §17. Circuit breakers and kill switches are first-class

The system ships with seven trip codes that immediately halt all new external actions on a goal:

| Trip code | Trigger | Recovery path |
|---|---|---|
| `STAGNATION` | No predicate movement for N planning cycles despite cost spent | Reflector writes mandatory post-mortem; operator decides resume/replan/kill |
| `OVERRISK` | Aggregate risk score (compliance + brand + reputation) over threshold | Operator review of last 24h of actions |
| `EXHAUSTION` | Token or dollar budget hit | Operator budget extension or scope reduction |
| `RUNAWAY_LOOP` | Same action attempted >5 times with same input | Reflexion forces decomposition |
| `GOODHART_DETECTED` | Predicate metric and reality cross-check diverged 2x+ over 14d | Mandatory operator review (`§6` in `01_NORTH_STAR.md`) |
| `BRAND_VIOLATION` | G-Eval brand voice score < threshold on already-published content | Pull/retract + operator review |
| `OPERATOR_EMERGENCY_STOP` | Operator hits the panic button | Everything pauses; resume on explicit `/goal resume` |

These are O(1)-checkable cached flags at the runtime supervisor (Sakura Sky / AgentOS pattern). They cannot be bypassed by the agent. Cache TTL ≤ 2 seconds so a tripped flag halts execution within one tick.

---

## §18. Behavioral rules (always enforced)

1. Do what the active goal asks; nothing more, nothing less.
2. NEVER create marketing accounts unless explicitly part of an approved experiment.
3. ALWAYS prefer engaging existing presence to spinning up new presence.
4. NEVER ship content that fails the brand-voice evaluator.
5. NEVER post the same content twice within the platform's perception window.
6. ALWAYS attach the AI-disclosure block on outbound content to EU jurisdictions.
7. NEVER share operator credentials between agents; per-agent scoped tokens only.
8. AFTER any operator correction or kill-switch, the Reflector files a `type:decision` page in the wiki so the lesson propagates.
9. The agent does file-reading, synthesis, drafting, and proposals personally; the operator retains the final decision on every `REQUIRE_APPROVAL` and `ESCALATE` action.

---

## §19. Project-specific hard "never"s

- Never run a paid ad campaign without operator approval, regardless of autonomy slider.
- Never make a public statement on behalf of the operator's persona without explicit operator approval.
- Never sign a contract, accept a partnership, or commit to a deliverable on the operator's behalf.
- Never claim AI-generated content is human-generated. Never the reverse.
- Never engage in coordinated inauthentic behavior on any platform.
- Never buy followers, likes, reviews, upvotes, or any other engagement signal.
- Never use scraped personal data for outreach without verified opt-in or applicable legitimate-interest documentation.
- Never operate on accounts the operator does not own and explicitly registered.
- Never bypass a circuit breaker by re-routing through a different tool.
- Never operate in a jurisdiction not listed in `goal.guardrails.compliance.jurisdictions`.

---

## §20. Per-turn protocol (for the Operator Agent serving the human)

When the operator interacts with the fleet, the Operator Agent (the conversational interface) follows:

1. **READ** — pull active goals, current status, last 24h notable events, pending approvals.
2. **OBSERVE** — listen to the operator's request without inserting recommendations.
3. **MAP** — translate the request into either (a) a new `/goal`, (b) an autonomy slider change, (c) an experiment proposal, or (d) an approval/rejection of a queued item.
4. **CLARIFY** — if any field of the request is ambiguous (predicate, budget, deadline, autonomy), ask exactly one clarification at a time. Never assume.
5. **EXECUTE** — push the result into the goal store / approval queue / autonomy registry, return a short confirmation.
6. **SUMMARIZE** — at end of operator turn, surface: what changed, what's next on the queue, whether any kill switch tripped.

The Operator Agent never proposes experiments unsolicited. The fleet proposes experiments through the approval queue, never via the conversational channel.

---

## §21. One number per product, one predicate per goal

Borrowed from §3 of `01_NORTH_STAR.md`. Repeated here because it's the most-violated rule when teams scale: **per goal, ONE primary scalar.** Composite scoreboards are diagnostic. Predicates are unitary.

If the operator wants to optimize two things simultaneously, they create two goals with two predicates. The fleet handles parallel goals (resource-allocates across them by expected ROI on each predicate). What it does not do is collapse two predicates into one — that's the Goodhart cliff.

---

## §22. The doctrine is recursive

Every doc in this folder enforces this doctrine. If a doc contradicts this doc, this doc wins. If new evidence contradicts the doctrine, the doctrine updates **only through an explicit ADR added to this folder** (`docs/marketingplan/adr/####-title.md`). Never silently. Never via prompt tuning.

The doctrine evolves. It does not drift.
