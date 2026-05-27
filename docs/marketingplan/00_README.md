# Marketing Plan — Master Index

> **A goal-driven autonomous growth engine.** The operator types `/goal grow ContextGraph to 50,000 weekly-active developers by 2026-09-01` and a fleet of reasoning agents — equipped with every tool they need (300+ MCP integrations + computer-use via Synapse + Playwright) — works in a cron-scheduled `ASSESS → PLAN → BUILD → COMMUNICATE` loop until the success predicate evaluates `TRUE` for four consecutive windows. **It does not stop until complete.** Hard guardrails (circuit breakers, kill switches, approval gates, EU AI Act Article 14 human oversight) prevent any individual experiment from going off-rails; the autonomy slider lets the operator dial from "approve every external action" → "fully autonomous between status reports."

---

## What this is

Not a fixed playbook. An **autonomous marketing operating system**, modeled on:

- **Andrej Karpathy's AutoResearch** (`karpathy/AutoResearch`, March 2026) — give an agent a markdown goal file, a single scalar metric, and a tool budget; the agent runs ~100 experiments overnight, keeps wins, discards losses. We replace "improve loss curve" with "move the user-acquisition number."
- **Karpathy's AgentHub** — agent-first collaboration platform (sprawling DAG of attempts, message board for coordination, no main branch). Our fleet shares state and learnings the same way.
- **Karpathy's autonomy slider** — Cursor/Perplexity/Tesla pattern of "human does everything → AI handles workflows with approval → AI operates more independently." Marketing platform exposes the same dial.
- **Yopedia pattern** — cron-scheduled, vision-document-driven agents with mechanical gates (shell-script-enforced, not LLM-self-policed). ASSESS → PLAN → BUILD → COMMUNICATE every hour.
- **ROMA / ADaPT / TDP** recursive task decomposition — only decompose when execution fails. Planner-Executor-Aggregator per node. Local replanning, not whole-task replanning.
- **Reflexion** verbal self-critique memory — failed attempts become text reflections stored in episodic memory; next attempt sees them.
- **LangGraph** for durable, stateful, interruptible execution with first-class human-in-the-loop.
- **Markifact / Hyper MCP / Windsor MCP** + bespoke computer-use — comprehensive tool surface so the agents can actually act across every channel a real marketer touches.

It runs for three distinct products:

| Product | Audience | Adoption pattern |
|---|---|---|
| **ContextGraph / ME‑JEPA‑Code** | AI engineers, dev-platform teams, AI coding agent vendors | Bottom-up developer adoption → enterprise contracts |
| **Synapse** | AI engineers, automation builders, accessibility hackers, gamers | Tool ecosystem adoption → SDK partnerships |
| **Polis / Serendipity Engine** | Intellectually curious adults disillusioned with engagement-farming social media | Geographic-cluster network effects → cultural movement |

Same agent fleet, same goal-execution loop, three different goal specs.

---

## The 60-second pitch

> **You write a goal in markdown. The system runs until the goal is achieved.**
>
> Marketing in 2026 is a goal-execution problem, not a content-production problem. Set the goal (50,000 weekly active developers, $30 CAC ceiling, 90 days), constrain the autonomy slider, lock the guardrails (rate limits, EU AI Act disclosures, brand voice), and the agent fleet runs the assess→plan→build→communicate loop continuously. The Planner decomposes the goal into a DAG of experiments. Executors run experiments in parallel (one per channel/tactic). The Evaluator (LLM-as-judge + deterministic metric readback) decides which experiments earned their cost. The Memory (LLM Wiki) files every win and loss as searchable knowledge. The Operator wakes up in the morning to a status report — and the goal-progress number has moved. When all experiments stop moving the number, the system reflects, replans, and tries again. When the success predicate is `TRUE` for four consecutive windows, the system parks and surfaces "GOAL ACHIEVED — set a new one."

---

## The `/goal` command

```bash
/goal grow ContextGraph to 50,000 weekly-active developers by 2026-09-01 \
  with LTV:CAC >= 3.0 \
  --autonomy=medium \
  --budget=$25k/mo \
  --brand-voice=docs/brandvoices/contextgraph.md \
  --compliance=US,EU,CA \
  --kill-on=brand-violation,rate-limit-exceeded,goodhart-detected
```

The full grammar lives in `22_GOAL_SPEC_FORMAT.md`. The operator can also write a longer markdown goal spec (the Karpathy `program.md` pattern) and reference it by path.

---

## Reading order

Read in numeric order on first pass. Each subsequent doc assumes the previous ones are loaded.

| # | File | What it answers |
|---|---|---|
| 00 | **[README](00_README.md)** | This file. The map. |
| 01 | [NORTH_STAR](01_NORTH_STAR.md) | The single numerical predicate that defines "the system works." The "don't stop until complete" definition. |
| 02 | [FIRST_PRINCIPLES](02_FIRST_PRINCIPLES.md) | The non-negotiable doctrine. Karpathy's autonomy slider. Trust the harness, not the model. Worthlessness test. |
| 03 | [TIER_MODEL_AUDIENCE](03_TIER_MODEL_AUDIENCE.md) | Steve's tier model formalized. Mass / Engaged / Elite — different physics per tier. |
| 04 | [SYSTEM_ARCHITECTURE](04_SYSTEM_ARCHITECTURE.md) | Atomizer → Planner → Executors → Aggregator. The cron loop. State persistence. HITL interrupt points. |
| 05 | [AGENT_FLEET_DESIGN](05_AGENT_FLEET_DESIGN.md) | Every agent class. Inputs, outputs, tools, prompts, supervision tier. |
| 06 | [COMPUTER_USE_PLAYBOOK](06_COMPUTER_USE_PLAYBOOK.md) | Synapse + Playwright + Browser-Use for every channel without a real API. Rate limits, ToS, account isolation. |
| 07 | [CONTENT_FACTORY](07_CONTENT_FACTORY.md) | The production pipeline — research → brief → draft → distribute → AI-search visibility → attribution. Brand voice. |
| 08 | [DISTRIBUTION_CHANNELS](08_DISTRIBUTION_CHANNELS.md) | Every channel with CAC range, fit per product, owner agent, ramp curve. |
| 09 | [VIRAL_LOOPS_AND_REFERRALS](09_VIRAL_LOOPS_AND_REFERRALS.md) | K-factor math, cycle time, referral mechanics. |
| 10 | [INFLUENCE_NETWORK_MAPPING](10_INFLUENCE_NETWORK_MAPPING.md) | The Elite tier. Power maps. Warm-intro pathfinding. Champion identification. |
| 11 | [PRODUCT_PLAYBOOKS](11_PRODUCT_PLAYBOOKS.md) | Per-product GTM: ContextGraph, Synapse, Polis. |
| 12 | [DEVELOPER_MARKETING](12_DEVELOPER_MARKETING.md) | The AI DevRel Flywheel: notebooks, OSS refs, hackathons, content DevRel, integrations guild. |
| 13 | [CONSUMER_SOCIAL_LAUNCH](13_CONSUMER_SOCIAL_LAUNCH.md) | Polis-specific cold start: campus ambassadors, TikTok virality, dense friend clusters. |
| 14 | [COMPLIANCE_AND_ETHICS](14_COMPLIANCE_AND_ETHICS.md) | CAN‑SPAM, GDPR, CCPA, CASL, EU AI Act (Aug 2026), platform ToS, ethical guardrails. |
| 15 | [METRICS_AND_OBSERVABILITY](15_METRICS_AND_OBSERVABILITY.md) | Goal predicates. Per-product number sheet. Attribution. LLM-as-judge calibration. |
| 16 | [TECH_STACK](16_TECH_STACK.md) | LangGraph runtime, Markifact/Hyper MCP, RocksDB-backed agent memory, evaluator stack. Build vs buy. |
| 17 | [LAUNCH_PLAYBOOK_90DAY](17_LAUNCH_PLAYBOOK_90DAY.md) | The first goal: 0 → 1,000 activated users per product in 90 days. Week-by-week. |
| 18 | [CASE_STUDIES](18_CASE_STUDIES.md) | Cursor, Linear, Vercel, BeReal, Bluesky teardowns — what to copy, what to skip. |
| 19 | [OPERATIONS_AND_CADENCE](19_OPERATIONS_AND_CADENCE.md) | Cron schedule, daily/weekly/monthly rhythms, agent supervisor responsibilities. |
| 20 | [RISK_AND_FAILURE_MODES](20_RISK_AND_FAILURE_MODES.md) | Circuit breakers, kill switches, Goodhart detection, reputation recovery. |
| 21 | [BUILD_ROADMAP](21_BUILD_ROADMAP.md) | How to construct the platform itself. Order of operations. M1-M5. |
| 22 | [GOAL_SPEC_FORMAT](22_GOAL_SPEC_FORMAT.md) | The exact `/goal` command syntax and the long-form `goal.md` schema. |

---

## What this plan is NOT

- **Not a fully autonomous robot.** It's an **Iron Man suit** (Karpathy's analogy): an augmentation that becomes more autonomous as evidence accumulates that it doesn't break things. The autonomy slider starts low.
- **Not a content factory.** Content is one of ~25 agent classes. The point is goal achievement, not posts-per-week.
- **Not a get-rich-quick scheme.** Cursor took 18 months of obsessive product work before "organic virality" was even available as a phrase. BeReal took 2.5 years to mainstream. Plans this aggressive only work behind exceptional products.
- **Not an excuse to spam.** Hard guardrails (rate limits well below platform thresholds, EU AI Act disclosure, ToS-compliant computer use) are encoded as **shell-script-enforced mechanical gates**, not LLM-self-policed. They cannot be reasoned around.
- **Not platform-agnostic.** Different channels have different physics. The fleet is opinionated.

---

## Authority order

When the docs disagree, follow:

1. **`02_FIRST_PRINCIPLES.md`** — doctrine. Never override.
2. **`14_COMPLIANCE_AND_ETHICS.md`** — legal and ethical floor. Always binding.
3. **`20_RISK_AND_FAILURE_MODES.md`** — circuit breakers and kill switches. Mechanical gates.
4. **`01_NORTH_STAR.md`** — the predicate that defines success.
5. **`22_GOAL_SPEC_FORMAT.md`** — what an operator can command.
6. Everything else — operational.

If a tactic conflicts with doctrine or the compliance floor, the tactic loses. Without discussion.

---

## Quick architectural picture

```
                       ┌──────────────────────────────────────────────┐
   /goal "…" ────────► │  GoalSpec  (markdown, parsed, success pred.) │
                       └──────────┬───────────────────────────────────┘
                                  │
                                  ▼
        ┌─────────────────────────────────────────────────────────┐
        │                  OUTER LOOP (cron, hourly)              │
        │   ASSESS → PLAN → BUILD → COMMUNICATE  ┐                │
        │       ↑─────────────────────────────────┘                │
        └──┬────────────────────────────────────────┬─────────────┘
           │                                        │
           ▼                                        ▼
  ┌─────────────────┐                    ┌─────────────────────┐
  │ Atomizer:       │                    │ Memory  (LLM Wiki)  │
  │ atomic task?    │                    │  - Skills           │
  │  yes → execute  │                    │  - Reflections      │
  │  no  → planner  │                    │  - Win/loss log     │
  └────────┬────────┘                    │  - Channel cards    │
           │                             │  - Brand voice      │
           ▼                             └─────────────────────┘
  ┌─────────────────┐         ┌──────────────────────────┐
  │ Planner:        │────────►│  Executor agents (fleet) │
  │ DAG of subtasks │         │  - Content writer        │
  │ MECE, w/ deps   │         │  - Hacker News launcher  │
  └─────────────────┘         │  - PH launcher           │
                              │  - X/Twitter operator    │
                              │  - LinkedIn outreach     │
                              │  - Discord ambassador    │
                              │  - SEO/GEO publisher     │
                              │  - Influencer mapper     │
                              │  - Podcast booker        │
                              │  - Ad budget pacer       │
                              │  - Referral engineer     │
                              │  - …+ ~15 more           │
                              └────────────┬─────────────┘
                                           │
                                           ▼
                              ┌─────────────────────────┐
                              │ Aggregator + Evaluator  │
                              │  - LLM-as-judge         │
                              │  - Deterministic metric │
                              │    readback (north-star)│
                              │  - Trajectory eval      │
                              └────────────┬────────────┘
                                           │
            ┌──────────────────────────────┼─────────────────────────────┐
            ▼                              ▼                             ▼
   ┌────────────────┐          ┌────────────────────┐         ┌────────────────────┐
   │ Mechanical     │          │ Approval Queue     │         │ Circuit Breakers   │
   │ gates          │          │ (HITL)             │         │ Kill switches      │
   │ - rate limits  │          │ - operator chat    │         │ - Fuse (loops)     │
   │ - brand voice  │          │ - Slack/email      │         │ - BudgetFuse       │
   │ - compliance   │          │ - autonomy slider  │         │ - TimeoutFuse      │
   │ - schema match │          │   decides routing  │         │ - Goodhart watch   │
   └────────────────┘          └────────────────────┘         └────────────────────┘
```

---

## Status

This is a planning document. Implementation lives in `21_BUILD_ROADMAP.md`.
