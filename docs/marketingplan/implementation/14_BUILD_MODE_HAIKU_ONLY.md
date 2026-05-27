# BUILD_MODE — Haiku-Only Model Routing During Build/Test

> A single env var (`BUILD_MODE=build` vs `prod`) forces all 27 agent classes to route to Claude Haiku 4.5 during build/test phases. ~10-50× cheaper iteration; explicit acknowledgment that quality is degraded; deterministic flip-to-prod when ready. Operator-set per environment.

**Source spec docs:** `02_FIRST_PRINCIPLES.md §3 §16`, `04_SYSTEM_ARCHITECTURE.md §10.3`, `16_TECH_STACK.md §3 §12`, `15_METRICS_AND_OBSERVABILITY.md §5`.

---

## §1. Why this exists

Iterating on agent prompts costs LLM tokens. The original plan defaults Opus for high-stakes reasoning agents (M3 Assessor, M4 Planner, M7 Evaluator, M8 Reflector, M9 Communicator) and Sonnet for content drafting (P1, D4). At full fleet, a single cycle is **~$2-5** in inference cost; daily cycles plus bench-delta runs plus operator-iteration cycles push monthly cost to **$1,000-3,000** even before any real traffic.

**During build/test**, the operator is iterating prompts, debugging the cycle, validating gates — not optimizing for output quality. Running Opus/Sonnet during this iteration is wasted spend. Haiku is:
- ~10× cheaper than Sonnet
- ~50× cheaper than Opus
- Fast enough for tight iteration loops

The tradeoff: Haiku's reasoning quality is meaningfully lower. So `BUILD_MODE` is honest about that.

---

## §2. The env var

```bash
# In Infisical (or .env.local during bootstrap):
BUILD_MODE=build    # all agents route to Haiku
# or
BUILD_MODE=prod     # per-agent model routing per agent card
```

Three valid values:

| Value | Behavior |
|---|---|
| `build` | All agents → Haiku 4.5. Defaults at fleet startup; use during all Phase 0-3 work and most of Phase 4. |
| `staging` | Per-agent routing per card, but with cost-tracking turned up; useful before flip to `prod` |
| `prod` | Per-agent routing per card (Opus/Sonnet/Haiku as appropriate). Use only when staging validates quality + cost. |

Default if unset: `build` (safe-by-default for cost; operator must explicitly opt into prod-quality).

---

## §3. Implementation

Per `04 §10.3`, model routing is a mechanical wrapper around the LLM client. The `BUILD_MODE` check goes there:

```python
# src/llm/model_routing.py (sketch)
import os

BUILD_MODE = os.environ.get("BUILD_MODE", "build").lower()

def select_model(agent_card: AgentCard, override: str | None = None) -> str:
    """Select Claude model for a given agent + situation."""
    if BUILD_MODE == "build":
        return "claude-haiku-4-5-20251001"  # force Haiku for everything
    if override:
        return _validate_model(override)
    if agent_card.model_routing.high_stakes_situation:
        return agent_card.model_routing.high_stakes_override
    return agent_card.model_routing.default
```

Calling agent code never sees the model name; it calls `anthropic_client.complete(prompt, agent_card)` and the routing wrapper picks.

---

## §4. What's degraded under BUILD_MODE

Honest acknowledgment per `02 §3` (trust the harness, don't hope the LLM compensates):

| Agent / Function | Quality impact under Haiku | Mitigation |
|---|---|---|
| **M3 Assessor** (cycle status reads) | Mild; primarily extraction-class work | Acceptable |
| **M4 Planner** (DAG decomposition + worth-test enforcement) | Moderate — Haiku decomposes shallower than Opus; may miss subtle dependencies | Acceptable for build; flip to Sonnet in staging |
| **M5 Atomizer** (atomic-or-decompose classification) | Mild | Acceptable |
| **M6 Aggregator** (rolling up child results) | Mild | Acceptable |
| **M7 Evaluator L4 (LLM-as-judge)** | Significant — Haiku scores quality more loosely than Sonnet | Use L1-L3 gates more aggressively; L4 calibrate against operator labels every 50 outputs in build mode |
| **M8 Reflector** (post-mortem writing) | Moderate — Haiku writes shallower reflections | Acceptable for build; deeper reflections will be re-written when flipping to prod |
| **M9 Communicator** (digest writing) | Mild | Acceptable |
| **M11 OperatorAsk** (free-text answer parsing) | Mild — Haiku may miss subtle operator-guidance nuance | Operator can re-state if M11 misparses |
| **P1 Content Writer** (drafts) | Significant — Haiku produces noticeably AI-flavored prose | Brand Voice Scorer L3 catches; drafts in build mode are throwaway iteration artifacts, not for publication |
| **P2 Technical Writer** | Significant | Same as P1 |
| **P4 Brief Generator** | Moderate | Acceptable for build |
| **D4 Outreach Drafter** | Significant | Mark drafts as "build-mode; do not send" |
| **D5 Warm-Intro Drafter** | Significant | Mark drafts as "build-mode; do not send" |
| **S3 Influence Network Mapper** | Mild (mostly structured-graph work) | Acceptable |
| **S4 Goodhart Auditor** (hypothesis generation on trip) | Moderate | Acceptable for build; operator reviews any real trip with prod-mode rerun |
| **Brand Voice Scorer** (fine-tuned Gemma 4B) | N/A — not routed via Anthropic; uses its own model | Unaffected |

The pattern: **structured / extraction / classification work is Haiku-adequate. Creative-judgment / nuanced-reasoning / final-published-content work is degraded.** Build mode is for iterating the structure of the pipeline; it is not for producing publishable artifacts.

---

## §5. What BUILD_MODE forbids

When `BUILD_MODE=build`:

| Forbidden | Why |
|---|---|
| Publishing to any operator-owned production account | Build-mode drafts may not be publish-quality |
| Sending any email to a real recipient | Same |
| Sending any DM to a real Tier-2 or Tier-3 contact | Same |
| Spending any paid-ad budget | Build mode is iteration, not acquisition |
| Goodhart Auditor trip → kill switch | Build mode noise can false-positive; operator reviews any trip manually |

Mechanism: `BUILD_MODE` is a guardrail-engine check. Per-channel `account_id` registry has a `build_mode_allowed: bool`; only test accounts (operator's `*_test` accounts) have it true.

---

## §6. The flip-to-prod checklist

When operator is ready to flip from `build` to `prod`, this checklist is mandatory:

- [ ] All Phase 0-3 acceptance criteria pass (`11_ACCEPTANCE_AND_BENCH_HARNESS.md §1.1-1.4`)
- [ ] Brand Voice Scorer (L3) Kappa ≥ 0.65 on operator-labeled holdout
- [ ] L4 LLM-as-judge rubrics calibrated with kappa ≥ targets (`11 §3`)
- [ ] Cost projection at prod-mode pricing: monthly LLM cost ≤ goal-spec budget
- [ ] First-week prod-mode dry-run on a no-publish goal (Phase 4 staging idea)
- [ ] Operator reviewed 10 sample agent outputs at staging quality; signed off
- [ ] Production account registry (`accounts.yaml`) updated with `build_mode_allowed: false` on real accounts
- [ ] Infisical `BUILD_MODE` secret updated in `prod` env

Once flipped, the next cycle uses per-agent routing per agent cards.

---

## §7. Cost projection

| Mode | Cycle cost (estimate) | Daily cost (24 cycles) | Monthly cost |
|---|---|---|---|
| BUILD_MODE=build (all Haiku) | $0.10-0.30 | $2.40-7.20 | **$72-216** |
| Staging (per-card routing, validation traffic) | $1-2 | $24-48 | $720-1,440 |
| Prod (per-card routing, full fleet) | $2-5 | $48-120 | **$1,440-3,600** |

Build mode is **20-50× cheaper** than prod. For a 12-week build/test phase, that's the difference between $200 and $10,000 in LLM iteration costs.

The Cost Pacer (O6) tracks actual vs projected; alerts operator if build mode somehow drifts above $300/mo (anomaly investigation).

---

## §8. Per-environment defaults

| Environment | BUILD_MODE default | Override allowed? |
|---|---|---|
| Phase 0 / Phase 1 / Phase 2 (operator dev box) | `build` | Yes, by operator for one-off staging test |
| Phase 3 (operator dev box, multi-product) | `build` | Yes |
| Phase 4 staging tests | `staging` | Operator-set |
| Operational launches (Phase 5/6/7) | `prod` | Only after flip-to-prod checklist passed |
| Bench-delta harness runs (Phase 4 onwards) | `prod` for the evaluated agent; `build` for surrounding context agents | Mixed-mode supported per agent |

Mixed-mode = bench-delta evaluates a candidate prompt at prod quality without paying prod cost for the surrounding cycle.

---

## §9. The bench-delta interaction

When `marketing bench delta <agent_class>` runs:
- The agent under test runs at prod-mode model per its card
- The surrounding fleet (other agents in the cycle context) runs at BUILD_MODE
- This isolates the delta to the single agent's quality change without polluting the comparison

Per `11 §2.3`: bench-delta is the only place agent quality is measured at prod cost.

---

## §10. Operator sign-off pattern

Whenever the operator wants to test something with prod quality temporarily:

```bash
# One-off prod-mode cycle for a specific goal
BUILD_MODE=prod marketing goal cycle <goal_id> --once

# Or persistent per-environment override
infisical secrets set BUILD_MODE=prod --env=dev   # if operator wants dev to use prod-quality
```

Audit log captures every BUILD_MODE flip with operator + timestamp.

---

## §11. What this REPLACES

Original `04 §10.3` model routing was per-agent-card driven from Phase 0 onward. Adding BUILD_MODE inverts the default: **all-Haiku unless operator explicitly opts in**. The per-agent cards still exist; they describe what `prod` mode looks like.

Original implication: $1,000-3,000/mo LLM cost from Phase 0 day 1.
New implication: $72-216/mo LLM cost during Phase 0-3; ramps to $1,440-3,600/mo only after flip-to-prod (Phase 4 onwards).

12-week saving: **~$10,000 of unspent LLM cost.**

---

## §12. Acceptance criteria (woven into per-phase gates)

### Phase 0 addition

- [ ] `BUILD_MODE=build` is default; verified by `marketing config get BUILD_MODE`
- [ ] All Anthropic calls route to Haiku regardless of agent card
- [ ] Cost telemetry shows Haiku usage 100% of LLM calls in build mode

### Phase 4 addition

- [ ] `BUILD_MODE=prod` flip tested in staging environment
- [ ] Bench-delta mixed-mode tested (agent under test on prod model; surrounding fleet on Haiku)
- [ ] Cost projection at prod-mode validated against actual; ≤10% variance

---

## §13. Cross-references

- Model routing source: `docs/marketingplan/04_SYSTEM_ARCHITECTURE.md §10.3`
- Tech stack model preferences: `docs/marketingplan/16_TECH_STACK.md §3 §12`
- Cost discipline doctrine: `docs/marketingplan/02_FIRST_PRINCIPLES.md §3 §16`
- Bench-delta gate (where mixed-mode matters): `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §9` + `11_ACCEPTANCE_AND_BENCH_HARNESS.md §2`
- Secrets where BUILD_MODE is set: `12_SECRETS_AND_INFISICAL.md §5`
- Phase 0 build (where this is introduced): `01_PHASE_0_FOUNDATIONS.md`
- Phase 4 (where flip-to-prod happens): `05_PHASE_4_PRODUCTION_HARDENING.md`
