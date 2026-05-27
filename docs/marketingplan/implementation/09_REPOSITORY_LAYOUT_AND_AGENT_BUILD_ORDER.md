# Repository Layout + Agent Build Order

> The concrete file/directory structure to scaffold for the marketing fleet build, plus the 26 agent classes in dependency-aware build order with skeletons for each. Read this before any phase doc to understand where things live and which agent to build when.

**Source spec docs:** `05_AGENT_FLEET_DESIGN.md` (all sections), `16_TECH_STACK.md §6 §8`, `21_BUILD_ROADMAP.md`.

---

## §1. Repository layout

```
marketing-fleet/
├── README.md                          # one-screen onboarding; links to docs/marketingplan/
├── CLAUDE.md                          # this codebase's CLAUDE.md addendum (LLM-coding-agent doctrine)
├── pyproject.toml                     # Python project metadata (or Cargo.toml if Rust-first)
├── .env.example                       # environment variables template (never commit real .env)
│
├── src/                               # the platform code
│   ├── goal_spec/                     # Pydantic schema + parser + validators (Phase 0)
│   │   ├── parser.py
│   │   ├── schema.py
│   │   └── validators.py
│   ├── storage/                       # RocksDB CFs + helpers (Phase 0)
│   │   ├── rocksdb_cfs.py
│   │   ├── cf_goals.py
│   │   ├── cf_goal_cycles.py
│   │   ├── cf_experiment_nodes.py
│   │   ├── cf_approval_queue.py
│   │   ├── cf_audit_log.py
│   │   ├── cf_reflections.py
│   │   ├── cf_wiki_index.py
│   │   ├── cf_contacts.py
│   │   ├── cf_channel_state.py
│   │   ├── cf_persona_history.py
│   │   ├── cf_attribution_graph.py    # Phase 2
│   │   └── cf_kill_switches.py
│   ├── outerloop/                     # LangGraph StateGraph + cron (Phase 0)
│   │   ├── states.py
│   │   ├── cron.py
│   │   ├── checkpointing.py
│   │   ├── atomizer.py
│   │   ├── planner.py
│   │   ├── aggregator.py
│   │   ├── evaluator.py               # stub Phase 0; full Phase 1+
│   │   ├── reflector.py
│   │   └── communicator.py
│   ├── llm/                           # Anthropic SDK wrapper + routing (Phase 0)
│   │   ├── anthropic_client.py
│   │   ├── model_routing.py
│   │   └── prompt_cache.py
│   ├── memory/                        # leapable + wiki abstraction (Phase 0)
│   │   ├── leapable_client.py
│   │   ├── wiki.py
│   │   └── reflection_query.py
│   ├── agents/                        # the 26 agent class implementations
│   │   ├── _base.py                   # AgentBase + shared LangGraph subgraph helpers
│   │   ├── meta/                      # M1-M10 (Phase 0 + 1 + 4)
│   │   │   ├── m1_operator_agent.py
│   │   │   ├── m2_goal_parser.py
│   │   │   ├── m3_assessor.py
│   │   │   ├── m4_planner.py
│   │   │   ├── m5_atomizer.py
│   │   │   ├── m6_aggregator.py
│   │   │   ├── m7_evaluator.py
│   │   │   ├── m8_reflector.py
│   │   │   ├── m9_communicator.py
│   │   │   └── m10_wiki_gardener.py   # Phase 4
│   │   ├── sensing/                   # S1-S6 (Phase 2 + 3)
│   │   │   ├── s1_audience_sorter.py
│   │   │   ├── s2_signal_listener.py
│   │   │   ├── s3_influence_mapper.py # Phase 3
│   │   │   ├── s4_goodhart_auditor.py # Phase 3
│   │   │   ├── s5_competitive_intel.py
│   │   │   └── s6_sentiment_tracker.py
│   │   ├── producing/                 # P1-P5 (Phase 1 + 2)
│   │   │   ├── p1_content_writer.py
│   │   │   ├── p2_technical_writer.py
│   │   │   ├── p3_visual_generator.py
│   │   │   ├── p4_brief_generator.py
│   │   │   └── p5_persona_carrier.py
│   │   ├── distributing/              # D1-D9 (Phase 1 + 2 + 3)
│   │   │   ├── d1_publisher.py
│   │   │   ├── d2_community_operator.py
│   │   │   ├── d3_github_maintainer.py
│   │   │   ├── d4_outreach_drafter.py
│   │   │   ├── d5_warm_intro_drafter.py  # Phase 3
│   │   │   ├── d6_comment_triage.py
│   │   │   ├── d7_newsletter_pitcher.py
│   │   │   ├── d8_podcast_booker.py
│   │   │   └── d9_event_coordinator.py
│   │   └── optimizing/                # O1-O7 (Phase 2 + 3)
│   │       ├── o1_seo_strategist.py
│   │       ├── o2_geo_optimizer.py
│   │       ├── o3_referral_engineer.py
│   │       ├── o4_ad_operator.py
│   │       ├── o5_conversion_optimizer.py  # Phase 3
│   │       ├── o6_cost_pacer.py        # mechanical, not LLM
│   │       └── o7_cohort_health_auditor.py
│   ├── guardrails/                    # the mechanical gates (Phase 0 → 4)
│   │   ├── engine.py                  # GuardrailEngine entrypoint
│   │   ├── kill_switch_check.py
│   │   ├── compliance_check.py
│   │   ├── rate_limit_check.py
│   │   ├── brand_voice_check.py
│   │   ├── schema_check.py
│   │   ├── budget_check.py
│   │   └── autonomy_check.py
│   ├── tool_gateway/                  # MCP + computer-use + bespoke adapters
│   │   ├── gateway.py                 # entry: every tool call routed through here
│   │   ├── markifact_adapter.py
│   │   ├── hyper_mcp_adapter.py
│   │   ├── windsor_adapter.py
│   │   ├── searchatlas_adapter.py
│   │   ├── leapable_adapter.py
│   │   ├── synapse_adapter.py
│   │   ├── playwright_adapter.py
│   │   ├── browser_use_adapter.py
│   │   ├── gh_cli_adapter.py
│   │   ├── gmail_adapter.py
│   │   ├── ms_graph_adapter.py
│   │   └── stripe_adapter.py
│   ├── evaluator/                     # the L1-L5 evaluator stack
│   │   ├── l1_schema.py
│   │   ├── l2_deterministic.py
│   │   ├── l3_classifier.py           # brand voice + toxicity + PII
│   │   ├── l4_llm_as_judge.py         # G-Eval / AdaRubric
│   │   ├── l5_reality_readback.py     # SoT queries
│   │   └── brand_voice_scorer.py      # fine-tuned Gemma 4B (Phase 1)
│   ├── cli/                           # operator CLI
│   │   ├── main.py
│   │   ├── goal_commands.py
│   │   ├── audit_commands.py
│   │   ├── wiki_commands.py
│   │   ├── kill_commands.py
│   │   ├── vacation_commands.py       # Phase 4
│   │   ├── incident_commands.py       # Phase 4
│   │   └── bench_commands.py          # Phase 4
│   ├── dashboard/                     # operator dashboard (React-served from same host)
│   │   ├── api.py                     # FastAPI/Express endpoints reading RocksDB
│   │   └── ui/                        # React app
│   └── observability/                 # LangSmith + Prometheus integration (Phase 4)
│       ├── langsmith_wrapper.py
│       ├── prometheus_metrics.py
│       └── alerts.py
│
├── agents/                            # per-agent CARDS (markdown, not code)
│   ├── _card_schema.md                # the schema all cards follow per `05 §2`
│   ├── M1_operator_agent.md
│   ├── M2_goal_parser.md
│   ├── ... (one .md per agent class)
│   └── O7_cohort_health_auditor.md
│
├── brandvoices/                       # per-product + per-persona brand voice specs
│   ├── leapable.md
│   ├── leapable.forbidden.md
│   ├── synapse.md
│   ├── synapse.forbidden.md
│   ├── polis.md
│   ├── polis.forbidden.md
│   └── personas/
│       ├── chris.md
│       └── ... (one per operator persona)
│
├── channel_cards/                     # per-channel operational state (YAML)
│   ├── x.yaml
│   ├── linkedin.yaml
│   ├── github.yaml
│   ├── hackernews.yaml
│   ├── reddit_machinelearning.yaml
│   ├── reddit_localllama.yaml
│   ├── reddit_programming.yaml
│   ├── discord_mcp_builders.yaml
│   ├── discord_anthropic.yaml
│   ├── devto.yaml
│   ├── hashnode.yaml
│   ├── tiktok.yaml
│   ├── instagram.yaml
│   ├── youtube.yaml
│   ├── substack.yaml
│   └── ... (one per active channel)
│
├── goals/                             # active + historical goal specs
│   ├── leapable-500-launch-2026-q2.md
│   ├── leapable-3k-launch-2026-q3.md
│   ├── leapable-50k-q4-2026.md
│   ├── synapse-200-launch-2026-q3.md
│   ├── synapse-2.5k-launch-2026-q4.md
│   ├── synapse-25k-2027-q1.md
│   ├── polis-cambridge-seed-2026-q4.md
│   ├── polis-6k-launch-2027-q1.md
│   └── ... (one per active/past goal)
│
├── skills/                            # per-channel + per-tactic playbooks
│   ├── launch-product-hunt.md
│   ├── launch-hackernews.md
│   ├── engage-rmachinelearning.md
│   ├── engage-anthropic-discord.md
│   ├── pitch-latentspace.md
│   ├── host-polis-salon.md
│   ├── recruit-campus-ambassador.md
│   └── ... (one per skill the fleet should remember)
│
├── accounts.yaml                      # operator-owned account registry per `06 §3.1`
├── platform_limits.yaml               # per-platform rate caps (30%-below-detection)
│
├── config/                            # platform configuration
│   ├── anthropic.yaml
│   ├── leapable.yaml
│   ├── rocksdb.yaml
│   ├── evaluators.yaml
│   ├── kill_switches.yaml
│   └── cron.yaml
│
├── bench/                             # bench-delta harness corpora (Phase 4)
│   └── marketing/
│       ├── p1_content_writer/
│       ├── d1_publisher/
│       ├── d4_outreach_drafter/
│       └── ... (one dir per agent class with 200-example holdout)
│
├── calibration/                       # quarterly calibration history (Phase 4)
│   └── rubrics/
│       ├── brand_voice_leapable/
│       ├── compliance/
│       ├── outreach_quality/
│       └── ... (one dir per rubric)
│
├── reflections/                       # filesystem-backed reflection wiki
│   ├── channel_x/
│   ├── channel_hackernews/
│   ├── tactic_outreach/
│   └── ... (organized by (channel, tactic, ts))
│
├── wiki/                              # LLM Wiki primary store (markdown)
│   ├── skills/                        # same skills as ./skills/ symlinked
│   ├── channel_cards/                 # same as ./channel_cards/ symlinked
│   ├── brand_voices/                  # same as ./brandvoices/ symlinked
│   ├── contact_graph.md               # operator's contact graph (Tier annotations)
│   └── personas/
│
├── evidence/                          # per-phase acceptance evidence
│   ├── phase_0_acceptance.md
│   ├── phase_1_acceptance.md
│   ├── phase_2_acceptance.md
│   ├── phase_3_acceptance.md
│   ├── phase_4_acceptance.md
│   ├── phase_5_leapable_launch_acceptance.md
│   ├── phase_6_synapse_launch_acceptance.md
│   └── phase_7_polis_launch_acceptance.md
│
└── tests/                             # automated tests
    ├── test_goal_parser.py
    ├── test_outerloop_resume.py
    ├── test_audit_log_chain.py
    ├── test_guardrail_engine.py
    ├── test_brand_voice_scorer.py
    └── ... (test per major module)
```

---

## §2. The 27 agent classes — dependency-aware build order

> **2026-05-26 update:** added **M11 OperatorAsk** (Slack-mediated question channel per `13_AUTONOMOUS_OPERATOR_INTERFACE.md`) and **O8 Cross-Product Handoff** (active cross-product prospecting per `15_MAXIMUM_GROWTH_DESIGN.md §4`). The catalog grew from 26 → 28 agents. (M11 + O8 = 2 new; one offset by O5 deferred to "product-team-dependent.")

### 2.1 The dependency graph

```
Phase 0:  M1, M2  →  M3, M4, M5, M6, M7(stub), M8, M9
Phase 1:  + P1, D1, M7(L1-L3 active), brand voice scorer
          + M11 OperatorAsk (basic Slack)
Phase 2:  + P2, P3, P4, P5
          + D2, D3, D4, D6, D7, D8, D9
          + S1, S2, S5, S6
          + O1, O2, O3, O4(skeleton), O6, O7
          + M7 L4 active
          + M11 OperatorAsk (full with Reflexion + dedup)
Phase 3:  + S3, S4
          + D5
          + O5
          + O8 Cross-Product Handoff
          + P5 multi-persona
Phase 4:  + M10
          + bench harness
          + observability stack
          + M11 OperatorAsk (vacation + high-urgency DM routing)
```

### 2.2 Per-class build skeleton (universal)

Every agent class follows this skeleton per `05 §2`:

```yaml
# agents/<ID>_<name>.md
---
id: <ID>
name: <Name>
class: meta | sensing | producing | distributing | optimizing
slot_identity: "<one-sentence responsibility>"
input_schema: contracts/<id>_input.schema.json
output_schema: contracts/<id>_output.schema.json
tools_allowed:
  - <tool>
tools_forbidden:
  - <tool>
model_routing:
  default: opus | sonnet | haiku
  high_stakes_override: opus
  cost_floor_override: haiku
vaults_allowed:
  - <vault_name>
context_load:
  - <wiki_path>
evaluator: <evaluator_id>
retry_policy: <count> attempts with Reflexion memory
cost_budget_per_run: $<amount> default; goal-spec can override
autonomy_default: AUTO | LOG | REQUIRE_APPROVAL | ESCALATE
concurrent_max: <int>
per_cycle_max: <int>
phase_first_active: <phase>
---

# <Name> (<ID>)

## Responsibility
<one-paragraph what this agent does AND explicitly what it does NOT do>

## Inputs
<bullet list of input fields with types>

## Outputs
<bullet list of output fields with types>

## Forbidden behaviors
<bullet list of things this agent must never do>

## Failure modes
<bullet list of common failures + the mitigations>
```

### 2.3 Build order with phase + summary per agent

| Order | Phase | ID | Name | Notes |
|---|---|---|---|---|
| 1 | 0 | **M2** | Goal Parser | Build first; validates every `/goal`; no upstream deps |
| 2 | 0 | **M1** | Operator Agent | Conversational interface; needs CLI in place |
| 3 | 0 | **M3** | Assessor | Per-cycle goal status reader |
| 4 | 0 | **M4** | Planner | Decomposer; worth-test enforcer (Phase 1 light, Phase 2 full) |
| 5 | 0 | **M5** | Atomizer | Decides atomic-or-decompose per node |
| 6 | 0 | **M6** | Aggregator | Rolls up child results |
| 7 | 0 | **M7** | Evaluator (stub) | Phase 0 schema-only; Phase 1 + L2/L3; Phase 2 + L4 |
| 8 | 0 | **M8** | Reflector | Failure post-mortems |
| 9 | 0 | **M9** | Communicator | End-of-cycle digest |
| 10 | 1 | **P1** | Content Writer | First Producing agent; needs Brand Voice Scorer |
| 11 | 1 | **D1** | Multi-Platform Publisher | First Distributing agent; X-only Phase 1 |
| 11.5 | 1 | **M11** | OperatorAsk (basic) | Slack-mediated ask channel; per `13_AUTONOMOUS_OPERATOR_INTERFACE.md`; Phase 1 ships basic post+reply+parse; Phase 2 full Reflexion+dedup |
| 12 | 2 | **P4** | Brief Generator | Topic → brief; Hormozi-vault queries |
| 13 | 2 | **P2** | Technical Writer | Deep technical content; Opus-routed |
| 14 | 2 | **P3** | Visual Generator | Images/diagrams/video |
| 15 | 2 | **P5** | Persona Carrier | Single-persona Phase 2; multi-persona Phase 3 |
| 16 | 2 | **D2** | Community Operator | Discord/Reddit/forum engagement |
| 17 | 2 | **D3** | GitHub Maintainer | Issue triage; PR review; release notes |
| 18 | 2 | **D4** | Outreach Drafter | Cold + warm follow-up emails |
| 19 | 2 | **D6** | Comment Triage | Replies on operator's content |
| 20 | 2 | **D7** | Newsletter Pitcher | Pitch operator to other newsletters |
| 21 | 2 | **D8** | Podcast Booker | Podcast pitch tracker |
| 22 | 2 | **D9** | Event Coordinator | Hackathon/meetup RSVP coordination |
| 23 | 2 | **S1** | Audience Sorter | Tier classification |
| 24 | 2 | **S2** | Signal Listener | Mention polling |
| 25 | 2 | **S5** | Competitive Intelligence | Competitor monitoring |
| 26 | 2 | **S6** | Sentiment Tracker | Brand sentiment delta |
| 27 | 2 | **O1** | SEO Strategist | Keyword research; programmatic SEO |
| 28 | 2 | **O2** | GEO Optimizer | AI-engine-citation structuring |
| 29 | 2 | **O3** | Referral Engineer | Viral loop design |
| 30 | 2 | **O4** | Ad Operator (skeleton) | Phase 2 skeleton; live Phase 3 with operator approval |
| 31 | 2 | **O6** | Cost Pacer | Mechanical (no LLM); enforces budget |
| 32 | 2 | **O7** | Cohort Health Auditor | Per-channel cohort retention; pauses referrals on decay |
| 33 | 3 | **S3** | Influence Network Mapper | T3 warm-intro pathfinding |
| 34 | 3 | **S4** | Goodhart Auditor | Daily predicate-vs-reality cross-check |
| 35 | 3 | **D5** | Warm-Intro Drafter | Forwardable-paragraph generator in bridge voice |
| 36 | 3 | **O5** | Conversion Optimizer | Landing-page A/B (product-team dependent) |
| 37 | 3 | **O8** | Cross-Product Handoff | Active prospecting across Leapable / Synapse / Polis; per `15_MAXIMUM_GROWTH_DESIGN.md §4` |
| 38 | 3 | **P5** | Persona Carrier (multi-persona) | Multi-persona handling + cross-persona leakage gate |
| 39 | 4 | **M10** | Wiki Gardener | Weekly memory cleanup |

(M3-M9 are Phase 0; total visible distinct agents now **28** (was 26 in `05 §1` original catalog + M11 + O8 from 2026-05-26 design augmentation). M7 spans phases; M11 spans phases 1+2+4.)

---

## §3. Storage layout — directories outside code

| Path | Purpose |
|---|---|
| `~/.marketing-fleet/db/` | RocksDB primary instance (Phase 0) |
| `~/.marketing-fleet/db/snapshots/daily/` | Daily backup snapshots (Phase 4) |
| `~/.marketing-fleet/db/snapshots/weekly/` | Weekly snapshots (Phase 4) |
| `~/.marketing-fleet/db/snapshots/monthly/` | Monthly snapshots (Phase 4) |
| `~/.marketing-fleet/keychain/` | OAuth tokens + API keys (OS keychain ideally) |
| `~/.marketing-fleet/logs/` | Structured JSON logs (audit + trace + error) |
| `~/.marketing-fleet/logs/archive/` | S3-bound archive when ≥30d old |
| `~/.marketing-fleet/cache/` | Anthropic prompt cache + leapable query cache |

---

## §4. The `accounts.yaml` registry (the operator's account ownership doc)

Per `06 §3.1`:

```yaml
- account_id: x_chris_personal
  platform: x.com
  persona: chris
  url: https://x.com/<handle>
  owner_documented: yes
  recovery_phone_owned_by: operator
  recovery_email_owned_by: operator
  agents_allowed: [D1, D6, S2, P5]
  rate_limits:
    posts_per_24h: 4
    replies_per_24h: 25
    likes_per_24h: 60
    follows_per_24h: 8
    dms_per_24h: 5
  brand_voice_spec_path: brandvoices/personas/chris.md
  ai_disclosure_required: false  # US-only account
  authentication: synapse_session  # vs api_token vs oauth
  active: true
  last_audit: 2026-05-26

- account_id: x_leapable_team
  platform: x.com
  persona: leapable_team
  ...

- account_id: github_chris
  ...
```

The Guardrail Engine refuses tool calls referencing accounts not in this file. The operator owns this file; agents only read.

---

## §5. The `platform_limits.yaml` (per-platform rate caps)

Per `06 §4`:

```yaml
x.com:
  detection_thresholds_published:
    posts_per_24h: 100
    replies_per_24h: 100
    follows_per_24h: 50
    likes_per_24h: 200
    dms_per_24h: 50
  internal_rate_caps:
    posts_per_24h: 4   # operator-specific; goes in accounts.yaml
    replies_per_24h: 25
    follows_per_24h: 8
    likes_per_24h: 60
    dms_per_24h: 5
  posting_cadence_distribution:
    type: log_normal
    mean_hours: 4
    stddev_hours: 2.5
  detection_signals_to_avoid:
    - perfectly_periodic_posting
    - identical_phrasing_across_replies
    - ...

linkedin.com:
  ...

hackernews:
  ...
```

Operator updates when platform policies change. Channel Cards consume this.

---

## §6. The bench corpus structure (Phase 4)

Per `21 §6.1`:

```
bench/marketing/<agent_class>/
├── corpus.jsonl                       # 200 historical inputs + ground-truth outputs
├── evaluator_config.yaml              # which Evaluator pipeline applies + thresholds
├── runs/
│   ├── 2026-05-26_v0.1.0_phase_4_initial.json
│   ├── 2026-06-15_v0.2.0_prompt_refactor.json
│   └── ...
└── decisions.md                       # operator-readable history of accepts/rejects
```

Operator can run `marketing bench delta <agent_class> --new-prompt=...`; produces a run.json with quality + cost delta vs prior. Merge blocked on regression unless operator overrides with justification.

---

## §7. The wiki / reflections / contacts structure

### 7.1 LLM Wiki primary

Filesystem-backed markdown per `16 §6.3`:

```
wiki/
├── skills/
│   ├── launch-product-hunt.md
│   ├── engage-rmachinelearning.md
│   └── ...
├── channel_cards/
│   └── (symlinks to ../../channel_cards/*)
├── brand_voices/
│   └── (symlinks to ../../brandvoices/*)
├── contact_graph.md
└── personas/
    └── chris.md
```

`CF_WIKI_INDEX` indexes this by SHA-256 of path → (path, last_updated, refs).

### 7.2 Reflections

Per-tactic-per-channel:

```
reflections/
├── channel_x/
│   ├── 2026-05-15-thread-strategy.md
│   └── ...
├── channel_hackernews/
│   ├── 2026-05-20-show-hn-launch.md
│   └── ...
└── tactic_outreach/
    ├── 2026-05-18-cold-email-hormozi-template.md
    └── ...
```

Each file is a structured Reflection per `02 §15` template.

### 7.3 Contact graph

`wiki/contact_graph.md` (or RocksDB CF_CONTACTS if scale demands):

```yaml
- contact_id: alice_kowalski
  tier: 3
  name: Alice Kowalski
  role: CEO, FrontierAI Lab
  last_meaningful_exchange: 2026-04-12
  channel_history:
    - {date: 2026-04-12, channel: warm_intro_via_bob_lee, outcome: meeting_confirmed_2026_06_10}
  reflection_refs:
    - reflections/tier3/2026-04-12-alice-warm-intro.md
  tags: [ai_lab_leadership, verification_thoughtleader]
  bridge_paths:
    - {via: bob_lee, strength: 0.85, length: 2}
    - {via: carol_mendes, strength: 0.55, length: 3}
```

---

## §8. The CLAUDE.md addendum for the marketing-fleet repo

The repo gets its own `CLAUDE.md` mirroring the ContextGraph project pattern:

```markdown
# CLAUDE.md — marketing-fleet codebase

This is the autonomous marketing fleet for Leapable, Synapse, Polis.

## Authority hierarchy
1. docs/marketingplan/02_FIRST_PRINCIPLES.md (doctrine, in the meta repo)
2. docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md (legal/ethical floor)
3. docs/marketingplan/implementation/* (this implementation plan)
4. docs2/AICodingAgentSuperPrompt.md (universal coding doctrine)

## What you can change
- Code in src/
- Tests in tests/
- Agent cards in agents/
- Brand voice specs in brandvoices/
- Channel cards in channel_cards/
- Goal specs in goals/
- Skills in skills/

## What you cannot change without operator approval
- accounts.yaml (operator owns account ownership)
- platform_limits.yaml (rate caps; doctrine)
- config/kill_switches.yaml
- Anything in evidence/ (acceptance records are append-only)

## Build commands
[per the Python or Rust toolchain decision in Phase 0]
```

---

## §9. Cross-references

- Agent class catalog (canonical): `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §1`
- Agent card schema template: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §2`
- Tech stack inventory: `docs/marketingplan/16_TECH_STACK.md`
- Compliance bound on outbound: `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md`
- Per-channel rate caps: `docs/marketingplan/06_COMPUTER_USE_PLAYBOOK.md §4`
- Per-phase build sequence: `01_PHASE_0_FOUNDATIONS.md` through `05_PHASE_4_PRODUCTION_HARDENING.md`
- Tool gateway implementation: `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md`
- Bench harness discipline: `11_ACCEPTANCE_AND_BENCH_HARNESS.md`
