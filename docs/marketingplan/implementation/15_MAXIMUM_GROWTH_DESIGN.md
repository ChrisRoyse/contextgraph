# Maximum Growth Design — Cross-Product Multipliers, Parallelization, Dogfood

> The original plan was conservative — sequential product launches, 1 Tier-3 conversation per week, separate audience funnels per product. **Operator has selected "aggressive" posture.** This doc specifies the upgrades: concurrent Leapable+Synapse Phase C, 2-3 Tier-3 conversations per week, the marketing fleet as its own marketing artifact, cross-product handoff layer, platform-health KPI surface. The result: higher growth ceiling at the cost of operator burnout risk that's explicitly managed.

**Source spec docs:** `11_PRODUCT_PLAYBOOKS.md §4 §5`, `10_INFLUENCE_NETWORK_MAPPING.md §5`, `12_DEVELOPER_MARKETING.md §3 §8 §10`, `17_LAUNCH_PLAYBOOK_90DAY.md §1 §11`, `18_CASE_STUDIES.md §8`.

---

## §1. The five maximum-growth multipliers

| # | Multiplier | Original plan | Aggressive plan |
|---|---|---|---|
| 1 | Launch sequencing | Sequential (Leapable Wk 4-16 → Synapse Wk 17-28 → Polis Wk 29-40) | Concurrent Leapable + Synapse Phase C (Wk 11-13); Polis still sequential (Wk 29-40) — saves 12 weeks |
| 2 | Tier-3 cadence | 1 endorsement-quality conversation/wk | 2-3/wk first 6 months; cultivated bridges deepen faster |
| 3 | Cross-product handoff | Mentioned in `11 §4`; not an active mechanism | Active prospecting: every Leapable dev-user becomes a Synapse candidate; every Polis intellectual is a Leapable Vault prospect |
| 4 | Dogfood-fleet-as-marketing | Implicit | Explicit: open-source part of fleet → developer marketing for Synapse; public audit log → trust signal across all 3 products |
| 5 | Platform-health KPI surface | Not measured | Cycles-per-hour; cost-per-acquired-user across portfolio; time-to-first-publish; brand-voice-pass-rate; operator-time-per-published-item |

Each multiplier compounds with the others.

---

## §2. Multiplier 1 — Concurrent Leapable + Synapse Phase C

### 2.1 Why it works

Leapable + Synapse share audience: AI engineers, MCP-ecosystem builders, hackathon attendees, Latent Space listeners. The same warm-supporter list, the same journalist pitches, the same PH-launch-day mechanic. Doing them in separate quarters duplicates operator effort and dilutes mindshare ("again? another launch?").

Concurrent launch creates a **single mindshare moment**: "ContextGraph's founder shipped two major products this month: a verifier and a computer-use server. The verifier proves the agent did what it said; the computer-use server lets it act. Together they're a complete AI agent substrate."

This is the Hormozi mega-launch pattern compounded: same Core Four, double the leverage.

### 2.2 The schedule change

| Original (sequential) | Aggressive (concurrent Phase C) |
|---|---|
| Wk 4-16: Leapable A → B → C | Wk 4-13: Leapable A → B |
| Wk 17-28: Synapse A → B → C | Wk 7-13: Synapse A (overlaps Leapable B) → B (overlaps Leapable late B) |
| | **Wk 11-13: Joint Leapable + Synapse Phase C mega-launch week** |
| | Wk 14-16: Post-launch conversion + hardening |
| | Wk 17-28: Both products in steady-state operation; operator-time freed for Polis seed |

Savings: ~12 weeks of calendar time. Polis launch starts Wk 29 either way (Polis needs different audience + operator-physical-presence).

### 2.3 What this costs the operator

Launch week (Wk 11-13): **60-80 hours/week** vs the original plan's 50-60. This is the burnout-risk window. Operator should:
- Block all non-launch work for 3 weeks
- Pre-coordinate with co-founder / spouse / personal life
- Have backup contacts pre-identified (panic-button people)
- Schedule recovery week immediately after (Wk 14)

If operator cannot commit to the 60-80hr week, fall back to sequential plan. This is a **must-not-fake** commitment — partial-effort on a mega-launch produces 0 of the 2x signal.

### 2.4 What this requires from engineering

Concurrent Phase C requires:
- Phase 3 engineering complete by Wk 10 (Cross-goal scheduler, Goodhart Auditor for two simultaneous predicates)
- M11 OperatorAsk wired so operator can be asked questions across both goals during the launch noise without queue conflation
- Per-product brand voice scorer + persona handling working (P5 Persona Carrier prevents cross-product copy leakage)
- Email-deliverability vendor warmed up (the launch volume will burn through Gmail/MS Graph caps)

### 2.5 Goodhart risk in concurrent mode

Two predicates rising at once requires the Goodhart Auditor to evaluate both with the same rigor. Specific risk: cross-attribution. A Leapable signup may be a Synapse-launch-traffic Leapable click. Attribution must dedupe by visitor before crediting either predicate.

### 2.6 Acceptance criteria (new)

- [ ] Phase 3 engineering complete by end of Wk 10
- [ ] Operator pre-committed to Wk 11-13 60-80hr availability
- [ ] Cross-product attribution dedup tested (one visitor counted to either Leapable OR Synapse predicate, never both unless they take both products' first-action)
- [ ] Goodhart Auditor produces concurrent reports for both goals
- [ ] Joint launch-day script written + rehearsed

---

## §3. Multiplier 2 — Aggressive Tier-3 cadence

### 3.1 The change

Original: 1 endorsement-quality conversation per week per active goal.
Aggressive: **2-3/wk first 6 months across all goals.**

### 3.2 What this requires

Per `10 §8`, Tier-3 is bridge-cultivation. Aggressive cadence works only if:
- The Mapper has surfaced 30+ ranked warm-intro paths per product
- The operator has 30+ pre-cultivated bridges (not 5-10)
- Pre-Call Briefer can produce briefs in <30 min
- Follow-Up Tracker is keeping bridges warm without operator manual effort

If any of these aren't in place, asking the operator to triple cadence just burns bridges.

### 3.3 Engineering support

- Influence Network Mapper (S3) Phase 3 ships with target-list expansion: each product's Tier-3 list grows from 10 to 30 names
- Mapper auto-refreshes ranked paths weekly (was: as-needed)
- Follow-Up Tracker becomes proactive: surfaces "send X relevant signal" suggestions weekly per bridge, not just on 90-day staleness

### 3.4 Operator commitment

- Block 6 hours per week for Tier-3 work (was: 2-3 hours)
- Pre-Call Briefer is operator's primary daily read
- Reflexion writes after every Tier-3 conversation (operator's voice recording or 5-min written summary)
- Bridge cultivation cadence per `10 §8` strictly enforced

### 3.5 Acceptance criteria (new)

- [ ] Tier-3 target list ≥ 30 per product
- [ ] Mapper ranked paths refreshed weekly with new public signals
- [ ] Operator completes ≥ 2 Tier-3 conversations per week for 8 consecutive weeks
- [ ] Bridge satisfaction (post-cultivation reflexion) ≥ 80% positive

---

## §4. Multiplier 3 — Cross-product handoff layer

### 4.1 The opportunity

Every Leapable subscriber who's a developer is a Synapse acquisition candidate. Currently the plan treats them as independent funnels.

### 4.2 The mechanism

A new agent class (or O3 Referral Engineer extension): **O8 Cross-Product Handoff**.

```yaml
---
id: O8
name: Cross-Product Handoff
class: optimizing
slot_identity: "Identify cross-product opportunity per user/contact; orchestrate handoff suggestions"
phase_first_active: Phase 3 (extension of O3 Referral Engineer)
---
```

O8 reads:
- Leapable user database: who's an AI dev (signals: query topics about RAG, AI agents, MCP, computer-use)
- Synapse user database: who'd care about provenance (signals: query topics about verification, audit, oracle correctness)
- Polis attendee database: who has intellectual creator background (Substack, podcaster, author signals)

O8 proposes handoff suggestions:

```yaml
handoff_proposal:
  source: leapable
  source_user_id: dev_2389
  source_signal: "asked 12 Asks about MCP-builder Discord in last 14 days; queried 3 Vaults about computer-use"
  target: synapse
  expected_action: in-product banner "your queries suggest Synapse might help"; or email "we built a thing for the use case you've been asking about"
  attribution_credit: 0.5 to leapable (already retained user), 0.5 to synapse (new acquisition)
  approval_required: yes (first 100 handoffs/quarter operator-approved; then auto)
```

### 4.3 Per-product handoff opportunities

| From → To | Signal | Mechanism |
|---|---|---|
| Leapable → Synapse | Dev queries about computer-use / agent automation | In-product banner; email at activation |
| Synapse → Leapable | User builds automation that needs verification | In-product suggestion: "verify with Leapable" |
| Polis → Leapable | Attendee is intellectual creator (Substack, podcast) | Post-event email: "publish your content as a Vault" |
| Leapable Creator → Polis | Creator hosts events / has audience | Email: "host a Polis event for your audience" |
| Polis → Synapse | Attendee has technical day-job, automation interest | Curated content drop on dev-related Polis events |

### 4.4 Implementation cost

O8 ships in Phase 3 (3-4 days engineering). Per-handoff payload uses LLM only for signal extraction; subsequent matching is mechanical.

### 4.5 Acceptance criteria (new)

- [ ] O8 Cross-Product Handoff agent operational
- [ ] ≥ 50 handoff suggestions generated in first 30 days
- [ ] ≥ 25% handoff conversion rate (sign-up at target product)
- [ ] Attribution dedup confirmed: no double-counting

---

## §5. Multiplier 4 — Dogfood-fleet-as-marketing

### 5.1 The asymmetric leverage

The marketing fleet is itself:
- A Synapse showcase (uses Synapse for every computer-use marketing action)
- A Leapable case study (uses Leapable Vaults as memory)
- A novel AI agent architecture (uses LangGraph + ROMA recursive control)

This is **world-class case-study material that's currently invisible to the market.**

### 5.2 The three dogfood marketing mechanisms

#### 5.2.1 Open-source release of fleet primitives

Ship 3 open-source releases over 6 months:

| Release | Content | Marketing benefit |
|---|---|---|
| `marketing-fleet-core` (Wk 16) | OuterLoop + RocksDB CFs + agent base classes; no agent-specific prompts | Developer marketing for Synapse; shows MCP-native architecture in action |
| `agent-cards-library` (Wk 24) | The 27 agent class YAML cards as reusable templates | DevRel artifact; positions operator as marketing-automation thought leader |
| `goal-spec-grammar` (Wk 32) | Pydantic schema + validators + bench harness | Becomes reference implementation for goal-driven autonomous systems |

Each release: PR-spec + Show HN + X thread + Substack. Cross-promotes Leapable + Synapse via real-use examples.

#### 5.2.2 Public audit log subset

The operator publishes a **redacted weekly audit log subset** on a transparency dashboard:
- Anonymized contacts (no PII)
- Spend totals per channel
- Decisions made by operator vs auto
- LTV:CAC achieved
- Goodhart Auditor reports (clean or trip)

This is unprecedented in marketing-automation industry. It's both:
- Trust signal (transparency)
- Recruitment magnet (engineers respect this rigor)
- Anti-competitor moat (competitors can't fake what this shows)

Implementation: Phase 4 deliverable; operator-approved-per-week before publishing.

#### 5.2.3 The "Marketing Fleet" Substack publication

A weekly Substack written by operator (with fleet-drafted derivatives) about:
- What the fleet shipped this week
- What worked / what failed
- Reflexions filed this week
- Hormozi vault queries that produced surprising results
- Cross-product audience-handoff stories

Audience: marketing-operators, AI-agent-builders, founders. Cross-promotes all 3 products.

### 5.3 Cost vs benefit

| Activity | Operator time / week | Expected reach |
|---|---|---|
| Open-source release (one per ~8 weeks) | 4-6 hours | 5k-50k stars + dev-press coverage per release |
| Public audit log subset | 1 hour | Trust signal; recruitment + investor confidence |
| Substack publication | 3-4 hours (operator-led; fleet-assisted) | 1k-10k subs by Wk 40; cross-promotes all products |

Total: ~8-11 hours/week. Net positive — these are also operator's brand-building hours that would otherwise be unfocused.

### 5.4 Acceptance criteria (new)

- [ ] First open-source release (`marketing-fleet-core`) shipped by Wk 16
- [ ] Public audit log subset published weekly from Wk 20
- [ ] Substack publication launched Wk 8; weekly cadence sustained
- [ ] At least 1 Tier-2 publication covers the open-source release organically

---

## §6. Multiplier 5 — Platform-health KPI surface

### 6.1 The original gap

The plan tracks per-goal predicates rigorously. It does NOT track **fleet-itself performance**:
- How many cycles per hour
- How much cost per acquired user across the portfolio
- How fast from goal-creation to first publish
- What % of drafts pass brand-voice on first attempt
- How much operator time per published artifact

These platform-health KPIs are the operator's signal that the fleet is *getting more efficient*, separate from whether goals are *getting hit*.

### 6.2 The KPI surface (added to dashboard)

| KPI | Why it matters | Target by Phase 4 |
|---|---|---|
| Cycles per hour per active goal | Throughput | ≥1.0 |
| Cost per acquired user (portfolio-wide) | Efficiency | Trending down quarter-over-quarter |
| Time-to-first-publish from new goal | Cold-start speed | <24h |
| Brand-voice first-attempt-pass rate | Iteration efficiency | ≥80% |
| Operator-time per published artifact | Iron-Man-suit leverage | ≤30 sec |
| Approval-queue median clear time | Operator-bottleneck health | <2h |
| OperatorAsk answered rate | HITL channel health | ≥85% |
| Reflexion-application rate (Reflexions consulted ÷ written) | Memory health | ≥40% |
| Goodhart Auditor trips per quarter | Predicate hygiene | <1 |
| Bench-delta gate pass rate | Agent-class evolution speed | ≥70% of attempted upgrades pass |

### 6.3 Implementation

Phase 4 dashboard (per `05_PHASE_4_PRODUCTION_HARDENING.md §2.2`) extends with a "Platform Health" tab showing these KPIs.

The KPIs are diagnostic, not gating. But quarterly review pulls each — anomalies investigated.

### 6.4 Acceptance criteria (new)

- [ ] Platform Health dashboard tab live in Phase 4
- [ ] All 10 KPIs reporting actuals
- [ ] First quarterly platform-health review completed; ≥7 of 10 KPIs at or trending toward target

---

## §7. Combined growth impact (estimate)

Combining all 5 multipliers (subject to operator capacity + execution quality):

| Metric | Original plan (40-week predicate) | Aggressive plan |
|---|---|---|
| Leapable WAS at Wk 40 | 8,000-15,000 (extrapolating from Phase C end) | 25,000-40,000 |
| Synapse WAMSD at Wk 40 | 6,000-12,000 | 20,000-35,000 |
| Polis MAEA at Wk 40 | 18,000-30,000 | (same — Polis still sequential; no change) |
| Combined LTV:CAC | 3.5-4.5 | 4.5-6.0 (cross-product handoff lifts retention) |
| Tier-3 endorsements at Wk 40 | 10-15 | 30-50 |
| Open-source GitHub stars (fleet) | 0 | 5k-20k |
| Substack subs | 0 | 2k-10k |
| Operator burnout risk | Medium | High during Wk 11-13 launch; sustainable otherwise |

The 2-3× growth-ceiling lift requires operator to absorb the launch-week peak (Wk 11-13) and sustain 8-11 hours/week additional brand-building work. If operator can't, fall back to sequential.

---

## §8. Burnout mitigation

This plan increases operator load. Three mitigations:

1. **Pre-launch recovery week.** Wk 14 immediately after concurrent Phase C is blocked for operator recovery. No Tier-3, no founder thread, no marketing work. Fleet runs auto.
2. **Co-pilot recruitment.** By Wk 20, operator hires or partners with one part-time human for: Tier-3 follow-up logistics, podcast booking ops, conference logistics. Frees operator's irreplaceable hours.
3. **Hard veto on adding goals.** While the 5 multipliers are running, operator does NOT add a 4th product or a parallel side-project. Fleet capacity is fully committed.

If operator violates 3, the burnout becomes structural. Plan it like an athlete plans peak weeks: brutal, but bounded.

---

## §9. Decision tree — when to dial back

The fleet self-monitors operator-load signals and surfaces "dial back?" recommendations:

| Signal | Recommendation |
|---|---|
| Operator-approval-queue median clear time > 6h sustained 7 days | Reduce ask volume; promote more action classes to LOG |
| OperatorAsk answer rate drops below 70% | Reduce ask volume; rate-cap M11 lower |
| Operator-time-per-published-artifact > 60 sec | Tune brand-voice threshold up; promote more drafts to AUTO |
| Goodhart trip + operator on vacation = pause concurrent launch | Sequential fallback mode auto-enabled |
| 14 consecutive days of operator marketing time > 50 hours | Slack DM to operator with "considering dialing back" diagnostic |

Operator overrides anytime; system never auto-dial-backs without operator approval (per `02 §12` operator-as-ceiling doctrine).

---

## §10. What this REPLACES

| Original | Aggressive (replaces) |
|---|---|
| Sequential 9-month launch (Leapable → Synapse → Polis) | Concurrent Leapable+Synapse Phase C (saves 12 weeks); Polis still sequential |
| Tier-3 cadence 1/wk per product | 2-3/wk first 6 months across goals |
| Cross-product synergy noted in `11 §4` | Active O8 Cross-Product Handoff agent |
| Fleet as private tool | Dogfood: open-source releases + public audit log + Substack |
| Per-goal predicate tracking only | + Platform-health KPI surface |

Original plan is still valid as **fallback if operator capacity erodes**.

---

## §11. Cross-references

- Per-product cross-synergies (original): `docs/marketingplan/11_PRODUCT_PLAYBOOKS.md §4`
- Hormozi mega-launch pattern: `docs/marketingplan/17_LAUNCH_PLAYBOOK_90DAY.md §1 §11`
- Cursor case study (founder-voice + concurrent shipping): `docs/marketingplan/18_CASE_STUDIES.md §1`
- DevRel flywheel (open-source as marketing): `docs/marketingplan/12_DEVELOPER_MARKETING.md §3`
- Vercel-pattern distribution (Next.js as the multiplier): `docs/marketingplan/18_CASE_STUDIES.md §3`
- Influence Network Mapper cadence: `docs/marketingplan/10_INFLUENCE_NETWORK_MAPPING.md §8`
- OperatorAsk (handles HITL during concurrent launch): `13_AUTONOMOUS_OPERATOR_INTERFACE.md`
- Cost discipline (the Substack + open-source release costs): `14_BUILD_MODE_HAIKU_ONLY.md §7`
- Phase 3 engineering deadline (Wk 10 hard date): `04_PHASE_3_MULTI_PRODUCT.md`
- Operator-vacation + incident modes (recovery week mechanism): `05_PHASE_4_PRODUCTION_HARDENING.md §6`
