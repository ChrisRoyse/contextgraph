# Marketing Plan — Master Index

> Goal-driven autonomous growth engine for **Leapable** (lead, ready), **Synapse** (ready), **Polis** (ready). Operator types `/goal grow Leapable to 50,000 weekly-active subscribers by 2026-09-01`. Fleet of reasoning agents — full tool surface (300+ MCP integrations + Synapse computer-use + Playwright) — runs cron-scheduled `ASSESS → PLAN → BUILD → COMMUNICATE` until predicate `TRUE` × 4 windows. **Does not stop until complete.** Mechanical guardrails (circuit breakers, kill switches, approval gates, EU AI Act Art. 14 oversight) prevent runaway. Autonomy slider dials "approve every action" → "fully autonomous between status reports."

---

## The three products

| Product | What it is | Adoption pattern |
|---|---|---|
| **Leapable** | Local-first MCP knowledge-vault marketplace. Subscribers Ask source-grounded questions; Creators publish Vaults (70/30 rev share via Stripe Connect); Developers integrate via 163 MCP tools. Signed installer; no Node/Docker/Python required for end users. Already shipping on `leapable.ai`. | 3-sided: Subscribers (T1+T2 mass), Creators (T2+T3 intellectual experts), Developers (T2 MCP-native) |
| **Synapse** | Rust MCP server for Windows desktop perception + action. Open-source core; Pro tier for ViGEm + team features. AI agents drive any Win32 app. | Bottom-up developer adoption → MCP ecosystem default → enterprise OS-RPA replacement |
| **Polis** | Re-engineered social network around physical/synchronous gathering, friend-density, debate, knowledge growth. Anti-engagement-farming. Five gathering formats. Sortition governance. | Single-city seed → campus ambassadors → TikTok virality → cultural movement |

One agent fleet. Three goal contexts. Different physics per product; same architecture.

---

## Foundations of the architecture

| Pattern | Source | Role |
|---|---|---|
| AutoResearch markdown-goal + scalar metric + ~100 experiments/night | Karpathy `karpathy/AutoResearch` | The goal-spec format; the experiment-evaluator loop |
| AgentHub message board + sprawling DAG of attempts | Karpathy `karpathy/agenthub` | Cross-agent coordination memory |
| Autonomy slider (augmentation → agent) | Karpathy Software 3.0 + Tesla Autopilot | Per-action-class dial |
| ASSESS → PLAN → BUILD → COMMUNICATE cron loop | Yopedia (`karpathy-llm-wiki`) | The outer loop |
| ADaPT / ROMA recursive decomposition | Prasad et al. 2023 + ROMA framework | Plan only when execute fails |
| Reflexion verbal self-critique memory | Shinn et al. 2023 | Learn across attempts |
| LangGraph durable + HITL + state-graph | LangChain 2024 | Execution runtime |
| LLM-as-judge with G-Eval / AdaRubric calibration | Liu et al. + 2026 G-Eval guide | Evaluator stack |
| Markifact / Hyper MCP / Windsor / leapable.ai vault-search | 2026 MCP ecosystem | Tool surface |
| Hormozi Core Four + Lead Getters + Value Equation + Client-Financed Acquisition | `$100M Offers`, `$100M Leads`, `$100M Money Models` | Marketing-doctrine substrate |
| Cursor / Linear / Vercel / BeReal / Bluesky / Stripe patterns | 2025-2026 case studies | Per-channel tactics |

---

## The 60-second pitch

Marketing in 2026 = goal-execution. Set goal (50k weekly-active subscribers, $30 CAC ceiling, 90 days). Constrain autonomy. Lock guardrails (rate limits, EU AI Act, brand voice). Fleet runs `assess→plan→build→communicate` continuously. Planner decomposes goal → DAG of experiments. Executors run in parallel, one per channel/tactic. Evaluator (LLM-as-judge + deterministic metric readback) decides which experiments earned their cost. Memory (LLM Wiki + Leapable expert vaults) files every win/loss. Operator wakes to a status report; the goal-progress number has moved. When experiments stop moving the number, reflect, replan, retry. When predicate `TRUE` × 4 windows, park and surface "GOAL ACHIEVED — set a new one."

---

## The `/goal` command

```bash
/goal grow Leapable to 50000 weekly-active-subscribers by 2026-09-01 \
  --autonomy=medium --budget=$25k/mo --compliance=US,EU,CA
```

Full grammar: `22_GOAL_SPEC_FORMAT.md`. Long-form `goals/<id>.md` files for complex specs.

---

## Reading order

| # | File | Answers |
|---|---|---|
| 00 | **README** | This file. The map. |
| 01 | [NORTH_STAR](01_NORTH_STAR.md) | One predicate defining "the system works." `Don't stop until complete` semantics. |
| 02 | [FIRST_PRINCIPLES](02_FIRST_PRINCIPLES.md) | Non-negotiable doctrine. Autonomy slider. Trust the harness. Worthlessness test. |
| 03 | [TIER_MODEL_AUDIENCE](03_TIER_MODEL_AUDIENCE.md) | Steve's Mass/Engaged/Elite formalized. Different physics per tier. |
| 04 | [SYSTEM_ARCHITECTURE](04_SYSTEM_ARCHITECTURE.md) | Atomizer → Planner → Executors → Aggregator. Cron loop. State persistence. HITL. |
| 05 | [AGENT_FLEET_DESIGN](05_AGENT_FLEET_DESIGN.md) | 26 agent classes: inputs, outputs, tools, prompts, supervision tier. |
| 06 | [COMPUTER_USE_PLAYBOOK](06_COMPUTER_USE_PLAYBOOK.md) | Synapse + Playwright + Browser-Use. Rate limits. ToS. Account isolation. |
| 07 | [CONTENT_FACTORY](07_CONTENT_FACTORY.md) | research→brief→draft→distribute→GEO→attribution pipeline. Brand voice gate. |
| 08 | [DISTRIBUTION_CHANNELS](08_DISTRIBUTION_CHANNELS.md) | Every channel: Core-Four quadrant, CAC, tier fit, owner agent, ramp curve. |
| 09 | [VIRAL_LOOPS_AND_REFERRALS](09_VIRAL_LOOPS_AND_REFERRALS.md) | K-factor math, cycle time, referral mechanics, Hormozi Lead Getters. |
| 10 | [INFLUENCE_NETWORK_MAPPING](10_INFLUENCE_NETWORK_MAPPING.md) | Elite tier. Power maps. Warm-intro pathfinding. Bridge cultivation. |
| 11 | [PRODUCT_PLAYBOOKS](11_PRODUCT_PLAYBOOKS.md) | Per-product GTM: Leapable / Synapse / Polis. |
| 12 | [DEVELOPER_MARKETING](12_DEVELOPER_MARKETING.md) | AI DevRel Flywheel: notebooks, OSS refs, hackathons, content DevRel, integrations guild. |
| 13 | [CONSUMER_SOCIAL_LAUNCH](13_CONSUMER_SOCIAL_LAUNCH.md) | Polis-specific cold start: campus ambassadors, TikTok, dense friend clusters. |
| 14 | [COMPLIANCE_AND_ETHICS](14_COMPLIANCE_AND_ETHICS.md) | CAN-SPAM, GDPR, CCPA, CASL, EU AI Act (Aug 2026), platform ToS, ethical floor. |
| 15 | [METRICS_AND_OBSERVABILITY](15_METRICS_AND_OBSERVABILITY.md) | Predicates. Per-product number sheet. Attribution. LLM-as-judge calibration. |
| 16 | [TECH_STACK](16_TECH_STACK.md) | LangGraph runtime, Markifact / Hyper MCP, Leapable vaults, evaluator stack. |
| 17 | [LAUNCH_PLAYBOOK_90DAY](17_LAUNCH_PLAYBOOK_90DAY.md) | First goal: 0 → 1,000 activated users per product in 90 days. Week-by-week. |
| 18 | [CASE_STUDIES](18_CASE_STUDIES.md) | Cursor, Linear, Vercel, BeReal, Bluesky, Stripe, Hormozi teardowns. |
| 19 | [OPERATIONS_AND_CADENCE](19_OPERATIONS_AND_CADENCE.md) | Cron schedule, daily/weekly/monthly rhythms, agent supervisor. |
| 20 | [RISK_AND_FAILURE_MODES](20_RISK_AND_FAILURE_MODES.md) | Circuit breakers, kill switches, Goodhart detection, reputation recovery. |
| 21 | [BUILD_ROADMAP](21_BUILD_ROADMAP.md) | M0–M4. How to construct the platform itself. |
| 22 | [GOAL_SPEC_FORMAT](22_GOAL_SPEC_FORMAT.md) | Exact `/goal` syntax + long-form `goal.md` schema. |

---

## What this plan is NOT

- **Not a fully autonomous robot.** Iron Man suit (Karpathy). Autonomy slider starts low; only loosens with evidence.
- **Not a content factory.** Content is 1 of ~26 agent classes. Goal achievement, not posts-per-week.
- **Not a get-rich-quick scheme.** Cursor: 18 months obsessive product work before "organic virality." BeReal: 2.5 years to mainstream. Aggressive plans behind exceptional products only.
- **Not an excuse to spam.** Hard guardrails (rate limits 30% below platform thresholds, EU AI Act disclosure, ToS-compliant computer use) are **shell-script-enforced mechanical gates**, not LLM-self-policed. Cannot be reasoned around.
- **Not platform-agnostic.** Different channels, different physics. Opinionated.

---

## Authority order (on conflict)

1. **`02_FIRST_PRINCIPLES.md`** — doctrine. Never override.
2. **`14_COMPLIANCE_AND_ETHICS.md`** — legal/ethical floor. Always binding.
3. **`20_RISK_AND_FAILURE_MODES.md`** — circuit breakers + kill switches. Mechanical.
4. **`01_NORTH_STAR.md`** — success predicate.
5. **`22_GOAL_SPEC_FORMAT.md`** — what an operator can command.
6. Everything else — operational.

Tactic conflicts with doctrine/compliance → tactic loses. No discussion.

---

## Quick architecture picture

```
   /goal "…" ──► GoalSpec (markdown, parsed, success predicate)
                       │
                       ▼
        OUTER LOOP (cron, hourly default)
        ASSESS → PLAN → BUILD → COMMUNICATE  ──┐
            ▲────────────────────────────────────┘
            │
   ┌────────┴───────┐         ┌──────────────────────────┐
   │ Atomizer       │         │ Memory (LLM Wiki +       │
   │ (atomic?)      │         │   Leapable expert vaults)│
   │  yes → execute │         │ - skills, reflections    │
   │  no → planner  │         │ - win/loss, channel cards│
   └────────┬───────┘         │ - brand voice            │
            │                 │ - contact graph          │
            ▼                 └──────────────────────────┘
   ┌────────────────┐
   │ Planner: DAG   │────► Executor agents (fleet, ~26)
   │ of subtasks    │      Content · Publisher · Community
   │ MECE, w/ deps  │      Outreach · Podcast · SEO · GEO
   └────────────────┘      Influence Mapper · Ad Pacer · …
                                       │
                                       ▼
                       Aggregator + Evaluator
                       (LLM-as-judge + SoT readback)
                                       │
            ┌──────────────────────────┼──────────────────┐
            ▼                          ▼                  ▼
   Mechanical gates          Approval Queue       Circuit Breakers
   - rate limits             (HITL)               Kill switches
   - brand voice             - operator chat      - Fuse (loops)
   - compliance              - Slack / email      - BudgetFuse
   - schema match            - autonomy slider    - TimeoutFuse
                              decides routing     - Goodhart watch
```

Implementation: `21_BUILD_ROADMAP.md`.
