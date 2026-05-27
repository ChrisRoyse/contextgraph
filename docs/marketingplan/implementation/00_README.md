# Implementation Plan — README + Critical Path

> Operator-executable build plan derived from the 22-doc specification in `docs/marketingplan/`. This subdirectory translates the spec into **phases** (sequential build milestones M0–M4) and **operational launches** (sequential 90-day product launches for Leapable → Synapse → Polis), plus cross-cutting concerns (repo layout, agent build order, tool integrations, guardrails, acceptance + bench harness).

**Authority hierarchy.** Spec wins on intent (`docs/marketingplan/00_README.md` and children). This implementation plan wins on sequencing + concrete build steps. When this plan and the spec conflict on **what** to build, the spec wins. When they conflict on **how/when** to build, this plan wins. When this plan and `02_FIRST_PRINCIPLES.md` conflict on a doctrine point, doctrine wins. Always.

**Goal of this plan.** Take an operator from "we have 22 docs describing what to build" to "we have a working goal-driven autonomous marketing fleet running three products" in **16 weeks of engineering + 9 months of operational launch sequence (Leapable → Synapse → Polis)**, with every intermediate week shipping a usable increment.

---

## 0. The plan in one screen — AGGRESSIVE POSTURE (operator-elected 2026-05-26)

```
ENGINEERING BUILD                            OPERATIONAL LAUNCHES
─────────────────                            ─────────────────────
Weeks 1-2   ─► Phase 0: Foundations         (no launch yet)
                + Infisical secrets bootstrap
                + BUILD_MODE=build (Haiku-only)
Weeks 3-6   ─► Phase 1: Single-product E2E  Wk 4-6: Leapable Phase A cycles (test mode)
                + M11 OperatorAsk (basic Slack channel)
Weeks 7-10  ─► Phase 2: Full fleet+Hormozi  Wk 7-10: Leapable A live → 500+ WAS
                + M11 full + Hormozi vault       Wk 7+: Synapse Phase A starts (overlapping)
Weeks 11-13 ─► Phase 3: Multi-product        Wk 11-13: ★ JOINT Leapable+Synapse Phase C ★
                + Goodhart, Mapper, O8 handoff           Both predicates: 3,000+ WAS, 2,500+ WAMSD
Weeks 14-16 ─► Phase 4: Production hardening Wk 14: Recovery week (operator)
                + flip BUILD_MODE=prod            Wk 15-16: Post-launch + open-source release
                                              ↓
                                            Wk 17-28: Both products in steady-state
                                                       (12 weeks of operator capacity freed
                                                        vs original sequential plan)
                                              ↓
                                            Wk 29-40: Polis 90-day launch (sequential;
                                                       different audience + operator
                                                       physical presence required)
```

**The build does not pause the launch.** From week 4 onward, every increment of the engineering build immediately enables more of the live launch. The platform is being used as it is being built.

**Aggressive vs Conservative posture.** Operator elected **aggressive** posture per `15_MAXIMUM_GROWTH_DESIGN.md`:
- Leapable + Synapse co-launch in Wk 11-13 (saves 12 weeks vs sequential)
- Tier-3 cadence 2-3/wk first 6 months (was 1/wk)
- Cross-product handoff active (O8 agent)
- Dogfood fleet as marketing (open-source releases + public audit log + Substack)
- 5 platform-health KPIs on dashboard

**Tradeoff:** higher growth ceiling, higher operator-burnout risk during Wk 11-13. Mitigations specified in `15 §8`. Fallback to sequential is always available.

---

## 1. File map of this implementation plan

| # | File | What it covers |
|---|---|---|
| 00 | **README** (this file) | Map + critical path + how to use |
| 01 | [PHASE_0_FOUNDATIONS](01_PHASE_0_FOUNDATIONS.md) | Weeks 1-2 — goal-spec parser, RocksDB CFs, CLI, Anthropic SDK, leapable MCP wiring, LangGraph baseline |
| 02 | [PHASE_1_SINGLE_PRODUCT_E2E](02_PHASE_1_SINGLE_PRODUCT_E2E.md) | Weeks 3-6 — Leapable end-to-end: P1 Content Writer + D1 Multi-Platform Publisher + Brand Voice Scorer + approval queue + first live campaign |
| 03 | [PHASE_2_FULL_FLEET_HORMOZI](03_PHASE_2_FULL_FLEET_HORMOZI.md) | Weeks 7-10 — full 26-agent fleet, 8 channels live, Hormozi vault deeply integrated, Referral Engineer + Cohort Health Auditor, viral loop shipped |
| 04 | [PHASE_3_MULTI_PRODUCT](04_PHASE_3_MULTI_PRODUCT.md) | Weeks 11-13 — Synapse + Polis goal classes, cross-goal scheduler, Goodhart Auditor, Influence Network Mapper, full compliance enforcement |
| 05 | [PHASE_4_PRODUCTION_HARDENING](05_PHASE_4_PRODUCTION_HARDENING.md) | Weeks 14-16 — LangSmith, dashboard, bench-delta harness, vacation/incident modes, self-audit, backup+restore |
| 06 | [PHASE_5_LEAPABLE_90DAY_LAUNCH](06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md) | Days 0-90 operational — Phase A/B/C concurrent with engineering build M1+ |
| 07 | [PHASE_6_SYNAPSE_90DAY_LAUNCH](07_PHASE_6_SYNAPSE_90DAY_LAUNCH.md) | Days 90-180 operational — Synapse Phase A/B/C |
| 08 | [PHASE_7_POLIS_90DAY_LAUNCH](08_PHASE_7_POLIS_90DAY_LAUNCH.md) | Days 180-270 operational — Polis Phase A/B/C with single-city seed |
| 09 | [REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER](09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md) | The actual file/directory structure + the 26 agent classes in build order with skeleton card per class |
| 10 | [TOOL_INTEGRATIONS_AND_GUARDRAILS](10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md) | Which MCPs / APIs / computer-use surfaces wired when + the mechanical guardrail engine implementation |
| 11 | [ACCEPTANCE_AND_BENCH_HARNESS](11_ACCEPTANCE_AND_BENCH_HARNESS.md) | Per-phase acceptance criteria, evaluator calibration discipline, bench-delta gate for agent rollout |
| 12 | [SECRETS_AND_INFISICAL](12_SECRETS_AND_INFISICAL.md) | Infisical-first secret management; `.env.local` build-mode fallback; rotation drills; per-host scoped Machine Identities |
| 13 | [AUTONOMOUS_OPERATOR_INTERFACE](13_AUTONOMOUS_OPERATOR_INTERFACE.md) | M11 OperatorAsk agent (27th class); Slack-as-question channel at `leapable.slack.com` workspace `T0B3LAJFYGY` channel `C0B2Q0CSJTY`; structured ask protocol; deduplication; Reflexion integration |
| 14 | [BUILD_MODE_HAIKU_ONLY](14_BUILD_MODE_HAIKU_ONLY.md) | `BUILD_MODE` env flag forcing all 27 agents to Haiku during build/test; flip-to-prod checklist; 20-50× cost reduction during iteration |
| 15 | [MAXIMUM_GROWTH_DESIGN](15_MAXIMUM_GROWTH_DESIGN.md) | The 5 growth multipliers (concurrent Leapable+Synapse Phase C, 3× Tier-3 cadence, O8 cross-product handoff, dogfood-fleet-as-marketing, platform-health KPIs); operator-burnout mitigations |

16 docs. Phase docs (01-08) are self-contained. Cross-cutting docs (09/10/11) are referenced by every phase. Design-augmentation docs (12/13/14/15) updated the original plan based on operator feedback 2026-05-26 (Infisical secrets, autonomous Slack-ask, Haiku build mode, aggressive growth posture).

---

## 2. Critical path (read this section if you read nothing else)

The order below is **mandatory**. Re-ordering breaks the dependency chain.

```
1. Phase 0 (Foundations)           ──┐
2. Phase 1 (Single-product E2E)    ──┤  ← engineering critical path
3. Phase 2 (Full fleet + Hormozi)  ──┤
4. Phase 3 (Multi-product)         ──┤
5. Phase 4 (Production hardening)  ──┘

     ┌──────── concurrent ────────┐
6. Leapable launch Phase A        ──► Phase B ──► Phase C        (start Wk 4-6, finish Wk 16)
7. Synapse launch Phase A         ──► Phase B ──► Phase C        (start Wk 17, finish Wk 28)
8. Polis launch Phase A (seed)    ──► Phase B ──► Phase C        (start Wk 29, finish Wk 40)
```

### 2.1 What blocks what

| If you skip... | These break |
|---|---|
| Phase 0 (goal-spec parser + RocksDB CFs) | Every other phase: no goal can be created |
| Phase 1 (single-product E2E) | Phases 2-4 build on the OuterLoop and approval queue established here |
| Phase 2 (full fleet + Hormozi vaults integrated) | Leapable launch Phase B (referral mechanic, 8-channel mix, Hormozi-grounded copy) |
| Phase 3 (multi-product + Goodhart Auditor + Mapper) | Synapse + Polis launches; Tier-3 outreach |
| Phase 4 (production hardening) | Long-term operation; quarterly recalibration; bench-delta-gated agent upgrades |
| Leapable Phase A (validation) | Phase B has no calibrated baseline; Phase C has no warm audience |
| Leapable Phase B (compounding) | Phase C mega-launch has no compounding base |
| Leapable launch | Synapse launch starts cold; fleet hasn't proven against any product yet |
| Synapse launch | Polis launch starts cold + operator has not yet proven multi-product execution |

### 2.2 What CAN be done in parallel

| Parallel pair | Why it works |
|---|---|
| Phase 1 engineering + Leapable Phase A operational | The first agents online (P1, D1, Brand Voice Scorer) are exactly what Phase A needs |
| Phase 2 engineering + Leapable Phase A→B transition | Each Phase 2 deliverable (Hormozi vault, viral loop, cohort auditor) lights up corresponding launch capability |
| Phase 3 multi-product + Leapable Phase C mega-launch | The Goodhart Auditor + Influence Network Mapper land in time for the mega-launch coordination requirements |
| Phase 4 hardening + Synapse launch Phase A | Hardening doesn't slow new launches; it makes them safer |
| Synapse launch Phase B + Polis launch Phase A | After 6 months of execution, the operator's fleet is mature enough to seed two products at once (different audiences, different physics) |

### 2.3 What MUST NOT be parallelized

- Two products in Phase C mega-launch within the same 14-day window. Operator bandwidth is the constraint; founder voice + Tier-3 conversations + launch-day engagement do not delegate.
- Polis launch Phase A and Synapse launch Phase A. Polis requires operator physical presence in seed city; cannot be combined with the deep Synapse engineering+content surge.
- Any new product launch before the previous product's launch has hit its 90-day predicate. Per `02_FIRST_PRINCIPLES.md §1`, the system does not multi-task on unfinished goals.

---

## 3. How to use this plan

### 3.1 The operator (you) reading order

1. Read this README (you're here).
2. Read `01_PHASE_0_FOUNDATIONS.md` end-to-end. This is the foundation; understanding it sets up every other phase.
3. Read the launch phase doc for the product you'll launch first (likely `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md`). This is what you'll actually be doing in week 4+.
4. Read `11_ACCEPTANCE_AND_BENCH_HARNESS.md` to understand the gate criteria for moving between phases.
5. Glance at `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md` so the file structure is unsurprising when engineering work starts.
6. Other docs become relevant as their phases approach.

### 3.2 The engineer (you-or-contractor) reading order

1. This README.
2. `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md` — the file structure to scaffold.
3. `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md` — the mechanical guardrail engine to build first; everything else relies on it.
4. `11_ACCEPTANCE_AND_BENCH_HARNESS.md` — the testing/calibration discipline.
5. `01_PHASE_0_FOUNDATIONS.md` then sequentially through `05_PHASE_4_PRODUCTION_HARDENING.md`.
6. Launch phase docs (06/07/08) for understanding what the engineering build is enabling.

### 3.3 Phase doc structure (universal)

Every phase doc (01–08) follows this structure:

```
1. Phase summary (goal, weeks, prerequisites)
2. Deliverables (specific files/agents/integrations to ship)
3. Week-by-week breakdown
4. Acceptance criteria (the gate to next phase)
5. Risks specific to this phase + mitigations
6. Cross-references to source spec docs
```

### 3.4 The doctrine cross-cuts every phase

`docs/marketingplan/02_FIRST_PRINCIPLES.md` is the doctrine. Every phase doc here references it. **The doctrine is not summarized or restated here** because no implementation choice can override it. If you find yourself in a phase doc wanting to do something doctrine forbids, the answer is "don't" — the doctrine wins regardless of what the implementation plan says.

Specifically these rules cross-cut every phase:
- Goal-driven not pipeline-driven (§1)
- Trust the harness, not the model (§3)
- Worthlessness test on every action (§4)
- SoT readback as verdict, not vendor dashboards (§5)
- Slot identity sacred — one agent, one job (§13)
- Bench-delta gate on every agent class change (§14)
- No vanity-metric predicates (§9 + §21)
- Hard "never" list (§19) — even with operator approval

---

## 4. The 16-week engineering build at-a-glance

| Phase | Week | Headline deliverable | Operator-visible result |
|---|---|---|---|
| **M0 — Foundations** | 1-2 | Goal-spec parser + RocksDB + CLI + leapable wiring + LangGraph baseline | `marketing goal create --spec=test.md` works; system survives Ctrl+C and resumes |
| **M1 — Single-product E2E** | 3-6 | Two agents live (P1 Writer, D1 Publisher); Brand Voice Scorer; approval queue; first Leapable campaign | Operator sees daily X.com publishes by the fleet, approves via Slack |
| **M2 — Full fleet + Hormozi** | 7-10 | All 26 agent classes; 8 channels live; Hormozi vault deeply integrated; first viral loop shipped | Daily output: 1 anchor + 12 derivatives across 8 channels; LTV:CAC dashboard live |
| **M3 — Multi-product** | 11-13 | Synapse + Polis goal classes; cross-goal scheduler; Goodhart Auditor; Influence Network Mapper | Three concurrent goals possible; warm-intro paths produced for Tier-3 contacts |
| **M4 — Production hardening** | 14-16 | LangSmith + dashboard polish + bench-delta gate + vacation/incident modes + self-audit | Self-audit produces full report <1h; 30+ days of clean operation; operator's daily marketing time ≤4h |

---

## 5. The 9-month operational launch sequence at-a-glance

| Launch | Weeks | Predicate target (90-day) | Channels at start | Operator commitment |
|---|---|---|---|---|
| **Leapable** | 4-16 (overlapping engineering) | `weekly_active_subscribers ≥ 3,000` | GitHub, HN, X-organic, Dev.to, Hashnode, MCP-builder Discords | 30-40 hr/wk Phase A; 50-60 hr launch week |
| **Synapse** | 17-28 | `weekly_active_mcp_sessions_with_depth ≥ 2,500` | GitHub, HN, X-organic, MCP Discords, AI agent hackathons | 25-35 hr/wk |
| **Polis** | 29-40 | `monthly_active_event_attendances ≥ 6,000` | TikTok, Instagram, campus ambassadors, in-person events, intellectual Substack creators | 35-50 hr/wk; non-delegable physical presence at events |

After Polis hits its 90-day predicate, the fleet operates **three concurrent steady-state goals**, the operator's daily marketing time stabilizes at ≤4 hours, and the long-horizon predicates (50k WAS / 25k WAMS / 50k WAE) become the active goals.

---

## 6. Pre-flight checks before starting Phase 0

The following must be true at start of week 1:

**Account ownership:**
- [ ] Operator owns and controls leapable.ai instance + has API credentials for the leapable MCP server
- [ ] Operator has Anthropic API account (key in `marketing-fleet/.env.local`; BUILD_MODE budget ~$200/mo; PROD_MODE ~$3k/mo)
- [ ] Operator owns and has admin access to all marketing accounts they'll use: X / LinkedIn / GitHub / HN / Reddit / Discord / TikTok / Instagram (per product launches)
- [ ] Operator has signed-installer infrastructure for Leapable (already shipping at `leapable.ai`)
- [ ] Operator has Synapse MCP shipping on operator's Win11 desktop (already at `v0.1.0-m3`)
- [ ] Operator has Polis substrate (postgres event_attendances schema; even if no events yet, the schema exists)
- [ ] Operator has filled in the goal-spec format with at least one validation goal per `22_GOAL_SPEC_FORMAT.md §8`

**Secrets + comms (per 12 + 13):**
- [ ] `marketing-fleet/` skeleton with `.env.local` populated (ANTHROPIC_API_KEY set; Slack workspace IDs set)
- [ ] Infisical CLI installed (verify with `infisical --version`)
- [ ] Operator ready to run `infisical login` + `infisical init` + `bash secrets/setup-infisical.sh` once Phase 0 reaches secrets-handling
- [ ] Slack app created in workspace `T0B3LAJFYGY`; bot added to channel `C0B2Q0CSJTY`; `SLACK_BOT_TOKEN` + `SLACK_SIGNING_SECRET` added to `.env.local`

**Doctrine + emergency:**
- [ ] Operator has read `02_FIRST_PRINCIPLES.md` end-to-end and accepts every doctrine point
- [ ] Operator has read `14_COMPLIANCE_AND_ETHICS.md §5` ethical-floor commitments and signed off
- [ ] Operator has emergency-stop muscle memory: knows `Ctrl+Alt+Shift+P` (Synapse), `/marketing kill`, `/marketing goal kill <id>`

**Aggressive-posture pre-commit (per 15):**
- [ ] Operator pre-committed to 60-80hr/week availability during Wk 11-13 joint Leapable+Synapse Phase C
- [ ] Operator pre-blocked Wk 14 as recovery week (no marketing work)
- [ ] Backup contact identified for incidents during launch week

Missing any of these → fix them before week 1.

---

## 7. The non-negotiable doctrine cross-cuts (recap)

Every phase plan in this directory is **subordinate** to these doctrine points from `02_FIRST_PRINCIPLES.md`:

| Doctrine | Implementation implication |
|---|---|
| §1 Goal-driven not pipeline-driven | The OuterLoop is goal-spec-driven; no hard-coded marketing calendars |
| §2 Karpathy autonomy slider | Every agent class card declares its autonomy default; operator overrides per goal |
| §3 Trust the harness, not the model | Mechanical guardrails (Cost Pacer, Brand Voice gate, Compliance wrappers) are deterministic shell-level gates, not LLM-self-policed |
| §4 Worthlessness test | Planner rejects nodes without `expected_metric_impact` blocks |
| §5 SoT readback | Evaluator queries product DB (RocksDB/Postgres) directly; never vendor dashboards |
| §6 GENERIC-ONLY agents | Agent code stays generic; playbooks carry per-channel specifics |
| §7 Reflexion memory | Every failure writes structured reflection; Planner reads before next attempt |
| §8 ADaPT recursive decomposition | Atomizer decides per node whether to dispatch or decompose; not pre-decomposed up front |
| §9 No mocks gate completion | Acceptance criteria reference SoT, not agent self-reports |
| §10 Compliance is upstream | Compliance check at Guardrail Engine before tool dispatch, not after publish |
| §11 Human-shaped computer-use | Per-platform rate caps 30% below detection; stochastic cadence |
| §12 Iron Man suit not robot | Founder voice + Tier-3 conversations + product proposals stay operator-only forever |
| §13 Slot identity | One agent, one job; cross-job composition through Orchestrator, never within an agent |
| §14 Bench-delta gate | Every prompt change, model swap, evaluator update ships only after delta-tested against held-out set |
| §15 Failure documentation | Reflector writes structured post-mortems on every failure |
| §16 Cost discipline | Cost Pacer is mechanical; no LLM-mediated budget decisions |
| §17 Circuit breakers + kill switches | Seven trip codes, O(1) cached, ≤2s TTL; LLM cannot bypass |
| §19 Hard "never" list | Synthetic personae, buying engagement, scraping behind logins, etc. — not bypassable |
| §21 One number per product, one predicate per goal | No composite predicates; multiple goals OK but each scalar |

If any phase plan in this directory drifts from these, **stop and fix the plan**, do not ship the drift.

---

## 8. The 5-question pre-execution test (universal, every phase, every deliverable)

Before any task in any phase doc ships:

1. **Which goal-tree number does this move?** (or which doctrine invariant does it preserve?) Be specific.
2. **By how much, by when?** Quantified expectation.
3. **What's the falsification condition?** Under what observed outcome is this work classified as failed?
4. **Does it preserve every doctrine point?** Cross-check against §7 above.
5. **What's the SoT readback that proves completion?** Not "the agent says so" — the source database / file / external system record.

If any can't be answered, the task is not ready. (Same as `docs/futurebuild/specs/NORTH-STAR.md §7` worthlessness test, applied to the marketing platform build.)

---

## 9. Cross-references

### 9.1 Source spec docs (read for *what*)

- `docs/marketingplan/00_README.md` — spec index
- `docs/marketingplan/01_NORTH_STAR.md` — the success predicate
- `docs/marketingplan/02_FIRST_PRINCIPLES.md` — doctrine
- `docs/marketingplan/22_GOAL_SPEC_FORMAT.md` — goal spec grammar

### 9.2 This implementation plan (read for *how/when*)

- `01_PHASE_0_FOUNDATIONS.md` → `05_PHASE_4_PRODUCTION_HARDENING.md` — engineering build
- `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md` → `08_PHASE_7_POLIS_90DAY_LAUNCH.md` — operational launches
- `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md` — concrete file structure + agent skeletons
- `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md` — MCP/computer-use/guardrail engine concrete build
- `11_ACCEPTANCE_AND_BENCH_HARNESS.md` — per-phase gate criteria + evaluator calibration

### 9.3 ContextGraph repo conventions (read for *style*)

- `CLAUDE.md` — project addendum
- `docs2/AICodingAgentSuperPrompt.md` — universal AI-coding-agent doctrine
- `docs/futurebuild/specs/FSV-PROTOCOL.md` — verification contract (the same SoT-readback discipline this plan inherits)

---

**Last updated:** 2026-05-26. Update this README whenever a phase doc changes its dependency structure or a new phase doc is added.
