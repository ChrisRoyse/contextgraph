# Phase 3 — Multi-Product (Weeks 11-13)

> Synapse + Polis goal classes added; cross-goal scheduler prevents channel collision + operator-bandwidth thrash; Goodhart Auditor live with daily predicate-vs-reality cross-checks; Influence Network Mapper produces warm-intro paths; Warm-Intro Drafter + Pre-Call Briefer + Follow-Up Tracker handle Tier-3 mechanics; full compliance enforcement at the Guardrail Engine for all 6 jurisdictions. After Phase 3, the operator can run 3 concurrent goals across 3 products; the platform is ready for the Leapable Phase C mega-launch.

**Source spec docs:** `21_BUILD_ROADMAP.md §5`, `03_TIER_MODEL_AUDIENCE.md`, `10_INFLUENCE_NETWORK_MAPPING.md`, `11_PRODUCT_PLAYBOOKS.md §2 §3 §4`, `13_CONSUMER_SOCIAL_LAUNCH.md`, `14_COMPLIANCE_AND_ETHICS.md`, `20_RISK_AND_FAILURE_MODES.md §4`, `19_OPERATIONS_AND_CADENCE.md §7 §8`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 3 weeks (15 working days) |
| **Prerequisites** | Phase 2 acceptance passed; Leapable Phase B underway; operator-time available for Tier-3 work |
| **Headline outcome** | Operator can run Leapable + Synapse + Polis goals concurrently; Goodhart Auditor daily report goes live; warm-intro paths surfaced for 10+ named Tier-3 targets; full compliance gates pass on 14 days of clean operation |
| **What's NOT in this phase** | LangSmith full integration (Phase 4); bench-delta harness (Phase 4); polished dashboard (Phase 4); Wiki Gardener (Phase 4); operator vacation/incident modes (Phase 4) |
| **Acceptance gate to Phase 4** | All §5 criteria pass; Synapse launch Phase A can begin (concurrent with Leapable Phase C); platform supports 2-3 active goals without thrash |
| **Operator time** | 60-90 min/day routine + extra time for Tier-3 approvals + Polis seed-phase event presence (if Polis launches concurrent — usually deferred to Phase 4+) |

---

## §2. Deliverables — multi-product goal classes

### 2.1 Synapse goal class

Per `11 §2 §2.3`:

- Synapse-specific brand voice spec at `brandvoices/synapse.md`
- Synapse-specific channel mix (GitHub, HN, X-organic, MCP-builder Discords, AI agent hackathons)
- Synapse SoT readback: `rocksdb://synapse/cf_sessions` for `weekly_active_mcp_sessions_with_depth` predicate
- Synapse-specific vaults: `SynapseProductDocs` + reuse `DevRelLiterature`
- Synapse channel cards if any channels differ from Leapable's (most overlap)
- First Synapse goal spec: `marketing-fleet/goals/synapse-200-launch-2026-q3.md` ready to activate

### 2.2 Polis goal class

Per `11 §3 §3.3` + `13`:

- Polis-specific brand voice spec at `brandvoices/polis.md` (very different from Leapable/Synapse — intellectual / cultural, not technical)
- Polis-specific channel mix (TikTok organic, IG organic, Substack intellectual, campus ambassadors, in-person events, X-organic)
- Polis SoT readback: `postgres://polis/events_attendances` for `monthly_active_event_attendances` predicate
- Polis-specific vaults: `PolisProductDocs`, `IntellectualCreatorContacts` (Tier-3 + Tier-2 target list)
- Polis-specific channel cards (TikTok, IG, campus ambassador program tracking)
- Campus ambassador economics: $30/successful referral, $50/install+attendance per `13 §5`
- First Polis goal spec: `marketing-fleet/goals/polis-cambridge-seed-2026-q3.md` (per `22 §8.3`)

### 2.3 Cross-goal scheduler (the M-tier enhancement)

Per `19 §7`:

- Channel-collision prevention: no two goals post on operator's X account within same hour
- Operator-bandwidth coordination: high-priority goal approvals surface first
- Cross-goal cost pool: daily velocity cap across all active goals
- Cross-goal Reflexion: "X audiences respond best to Tuesday morning posts" learning shared across products where applicable

Implementation: extends OuterLoop's cron scheduler in `outerloop/cron.py` with multi-goal awareness.

---

## §3. Deliverables — Goodhart Auditor (S4)

Per `05 §6.2` + `20 §4`:

### 3.1 Auditor build

- Runs once per 24h.
- For each active goal, computes:
  - Current predicate value
  - 14-day predicate slope
  - 14-day slope of 3 correlated reality signals (per `15 §11` table)
- If predicate-slope > 2 × max(reality-signal-slope) over 14d: post `incident:goodhart` to MessageBoard.
- OuterLoop monitors this topic; trips `GoodhartFuse` immediately.

### 3.2 Reality signals per product

| Product | Predicate | Reality signals |
|---|---|---|
| Leapable | `weekly_active_subscribers` | Paid-subscribers count; GitHub star velocity; HN/X mention velocity; Discord question-rate |
| Synapse | `weekly_active_mcp_sessions_with_depth` | Tools-per-session distribution; third-party MCP servers referencing Synapse; hackathon project count |
| Polis | `monthly_active_event_attendances` | Return attendance rate; attendee NPS; photo-consent rate; friend-graph density delta |

### 3.3 Auditor's review questions (when tripped)

Per `20 §4.4`, the Auditor proposes hypotheses on trip:
1. Definition drift: did anyone change predicate computation?
2. Source drift: did SoT schema change?
3. Bot/fake traffic
4. Sampling bias
5. Cohort decay

Operator reviews; findings filed to wiki.

### 3.4 Daily report

Per `20 §4`: Auditor publishes weekly report even when not tripped. Signal-vs-predicate plot is a permanent dashboard element.

---

## §4. Deliverables — Influence Network Mapper (S3) + Tier-3 stack

Per `10` (whole doc):

### 4.1 Mapper graph build (S3)

- Ingest operator's network: LinkedIn export, Gmail contacts, prior podcast guest list
- Build directed graph: nodes = people, edges = typed relationships per `10 §2` weight table
- Tag every node with relationship-class metadata
- For each operator-confirmed Tier-3 target, run BFS to find paths from operator-side nodes
- Score paths by `min(edge_weights)` over the chain
- Output: per-target ranked paths

### 4.2 Tier-3 target list curation

Per `10 §3` + `11 §1.6 §2.6 §3.6` per-product Tier-3 lists:

| Product | Tier-3 targets (starter set) |
|---|---|
| Leapable | Named intellectual experts (Tyler Cowen, Sam Harris, Russ Roberts, Paul Graham, Patrick Collison, Marc Andreessen, Tim Ferriss, Lex Fridman, Andrew Huberman); AI lab leadership (Anthropic, OpenAI, GDM leads on provenance); VC partners (Sequoia, a16z, Index, NEA, GV); Creator-economy investors (a16z Crypto/Games, Atelier) |
| Synapse | MCP ecosystem leadership (Anthropic MCP team); AI agent platform founders (Cognition/Devin, Cursor, Replit Agent); accessibility advocates; OS-RPA decision-makers at Fortune 1000 |
| Polis | Tyler Cowen, Sam Harris, Russ Roberts, Patrick Collison, Marc Andreessen, Tim Ferriss; intellectual conference organizers; third-place revival leaders (Strong Towns adjacent); major Substack intellectual writers |

Operator curates this list; Mapper finds paths.

### 4.3 Warm-Intro Drafter (D5)

Per `10 §4`:

- Produces forwardable-paragraph drafts in bridge's voice (loads bridge's prior writing samples)
- Operator reviews + edits before sending
- Hard rules per `10 §4.2`: never sends; never fabricates relationship history; no mass templates
- `REQUIRE_APPROVAL` always, with no operator-configurable bypass (doctrine)

### 4.4 Pre-Call Briefer

Per `10 §6`:

- Triggered when operator has Tier-3 call scheduled
- Produces one-page brief: last-90-days public signal on target, what they care about right now, mapped operator talking points, what NOT to bring up
- Operator-eyes-only; never goes to target

### 4.5 Follow-Up Tracker

Per `10 §7`:

- Passive monitor over CF_CONTACTS for every Tier-3 contact ever touched
- Trigger conditions: 90d staleness, target ships launch, target moves company, target appears at relevant event
- Surfaces suggestions to operator; never sends

### 4.6 Tier-3 doctrine enforcement

Per `02 §19` + `10 §9`:

- All Tier-3 actions `REQUIRE_APPROVAL` with no override
- Never send automated outbound to Tier-3 contact
- Never scrape personal data beyond public
- Never infer protected attributes
- Never store dossiers on health/family/politics/religion
- Never use Tier-3 names in T1/T2 marketing copy without explicit written permission

Implemented as Guardrail Engine fail-closed checks; agent prompts cannot override.

---

## §5. Deliverables — Persona Carrier (P5) and multi-persona handling

Per `05 §6.5` + `07 §8`:

- Operator declares per-persona brand voice signature (`brandvoices/personas/<persona>.md`)
- Per-persona posting history in `CF_PERSONA_HISTORY` for dedup
- Per-persona cadence cards
- Cross-persona content leakage = deny gate (e.g., "Polis" persona accidentally posting in "Leapable" voice → rejected)
- Persona-specific Brand Voice Scorer per persona (re-fine-tuned on per-persona approved content)

For Phase 3: operator runs 2-3 personae (personal account, Leapable team account, possibly Polis-specific account). All other personae implicitly deferred.

---

## §6. Deliverables — full compliance enforcement

Per `14` (whole doc):

### 6.1 Multi-jurisdiction wrappers

Tool wrappers for `send_email_eu`, `send_email_us`, `send_email_canada`, `send_email_au`, etc. — each enforces its jurisdiction's requirements:

| Jurisdiction | Enforcement |
|---|---|
| US (CAN-SPAM) | Physical address block; unsubscribe link; truthful sender; ad-classification when applicable |
| EU/EEA (GDPR + ePrivacy) | Lawful basis logged; balancing test referenced; right-to-erasure + right-to-access endpoints |
| EU/EEA (EU AI Act Art. 50) | AI-assistance disclosure on AI-drafted content |
| CA (CASL) | Consent verification; identification; unsubscribe |
| UK (UK GDPR mirror) | Same as EU |
| AU (Spam Act) | Consent; identification; unsubscribe |
| JP (APPI for personal info) | Disclosure on data use; opt-out for direct marketing |

### 6.2 Compliance Auditor agent

- Runs daily
- Samples 5% of recent outbound for spot-checks
- Flags any compliance check that should have fired but didn't
- Operator escalation within 6 hours of any failure

### 6.3 Right-to-erasure endpoint

- EU recipient sends "remove me"; system raises approval-queue item with proposed deletions + draft response
- Operator confirms; deletions execute across CRM, CF_CONTACTS, suppression lists
- Confirmation to requester within 30 days (target: <7 days)

### 6.4 Legitimate-interest balancing tests

- Operator signs per-campaign balancing test before any EU B2B outbound campaign per `14 §3.3`
- Compliance check refuses to launch a campaign without signed test

---

## §7. Deliverables — kill switch suite

Per `20 §3`:

| Kill switch | Implementation | Scope |
|---|---|---|
| Per-goal kill | `marketing goal kill <goal_id>` | One goal |
| Per-channel kill | `marketing channel kill <channel_name>` | All goals on that channel |
| Per-account kill | `marketing account kill <account_id>` | All goals on that operator-owned account |
| Global kill | `marketing kill` | All goals everywhere |
| Operator panic hotkey | `Ctrl+Alt+Shift+P` via Synapse | <50ms halt; bypasses CLI |

All implemented as flag-checks at runtime supervisor outside agent loop per `02 §3`.

---

## §8. Week-by-week breakdown

### Week 11 — Multi-product goal classes + cross-goal scheduler

**Day 51-52 (Mon-Tue) — Synapse goal class**
- Brand voice spec (operator-labeled samples, fine-tune scorer)
- Channel mix configuration
- SoT readback against Synapse RocksDB CF_SESSIONS
- First Synapse goal spec written, parses, validates

**Day 53-54 (Wed-Thu) — Polis goal class**
- Brand voice spec (very different voice — intellectual/cultural)
- Channel mix (TikTok, IG, Substack, campus, in-person)
- SoT readback against Polis Postgres
- Polis-specific persona separation (Polis-persona ≠ Leapable-persona)
- First Polis seed-city goal spec drafted

**Day 55 (Fri) — Cross-goal scheduler**
- Channel-collision prevention
- Operator-approval-queue prioritization across goals
- Cross-goal cost pool
- Test: two goals active, scheduler prevents conflict

### Week 12 — Goodhart Auditor + Influence Network Mapper

**Day 56-57 (Mon-Tue) — Goodhart Auditor (S4)**
- Daily cadence cron
- Predicate-slope + reality-signal-slope computation
- Trip detection at 2× divergence over 14d
- Auditor report renders for operator daily

**Day 58-59 (Wed-Thu) — Influence Network Mapper (S3)**
- Operator ingests LinkedIn export + Gmail contacts
- Graph build with typed edges per `10 §2`
- BFS pathfinding for each Tier-3 target
- First 10 warm-intro paths surfaced

**Day 60 (Fri) — Warm-Intro Drafter (D5) + Pre-Call Briefer + Follow-Up Tracker**
- D5 drafts forwardable paragraphs in bridge voice
- Pre-Call Briefer ready for next scheduled Tier-3 call
- Follow-Up Tracker active over CF_CONTACTS

### Week 13 — Compliance + Kill switches + Acceptance

**Day 61-62 (Mon-Tue) — Full compliance enforcement**
- All 7 jurisdiction wrappers shipped
- Compliance Auditor daily sampling
- Right-to-erasure endpoint operational
- Operator signs balancing tests for any planned EU B2B outreach

**Day 63 (Wed) — Persona Carrier (P5) + per-persona Brand Voice Scorers**
- Operator labels per-persona samples (60-90 min)
- Per-persona scorers fine-tuned
- Cross-persona leakage gate tested

**Day 64-65 (Thu-Fri) — Kill switch suite + Phase 3 acceptance**
- All 5 kill switch levels implemented
- Operator panic hotkey tested end-to-end (<50ms halt)
- Phase 3 acceptance file written; operator sign-off

---

## §9. Phase 3 architecture decisions to lock

| Decision | Recommended default | When operator can override |
|---|---|---|
| Cross-goal cost-pool ceiling | **Sum of per-goal monthly ceilings** | Per operator's portfolio strategy |
| Channel-collision prevention | **Hard: no two goals post on same operator account within 1h** | Never relax (looks coordinated) |
| Goodhart divergence threshold | **2× over rolling 14d** | Per `01 §6` — never relax |
| Tier-3 autonomy | **`REQUIRE_APPROVAL` always, no bypass** | Permanent (doctrine `02 §19`) |
| Compliance Auditor sample rate | **5% of recent outbound** | Increase if any failures detected |
| Right-to-erasure SLA | **30 days max; target 7 days** | Cannot loosen (legal) |
| Per-persona Brand Voice Scorer | **Separate scorer per persona** | Cannot merge (cross-persona leakage detection) |
| Polis ambassador economics | **$30 referral + $50 install/attend per `13 §5.1`** | Operator per Polis goal |

---

## §10. Acceptance criteria (gate to Phase 4)

Phase 3 is complete when **all** are true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | Phase 2 acceptance still passes (regression) | Re-run Phase 2 tests |
| 2 | Three goals can run concurrently (Leapable + Synapse + Polis) without thrash | Test: spin up all 3 in test mode; observe ≥7 days |
| 3 | Cross-goal scheduler prevents channel collision in 100% of tested conflicts | Test cases pass |
| 4 | Goodhart Auditor produces daily report; tripped flag halts goal within 1 cycle (verified by test injection) | Audit log + scenarios |
| 5 | Influence Network Mapper has produced ≥10 warm-intro paths the operator has reviewed | Mapper output + operator approval log |
| 6 | At least 1 warm-intro path executed (bridge approached, operator sent, target responded) | Outreach log + operator confirmation |
| 7 | Pre-Call Briefer used on at least 1 real Tier-3 conversation | Operator confirmation |
| 8 | Compliance Auditor runs daily for 14 days clean; no compliance trips | Audit log |
| 9 | Right-to-erasure tested end-to-end (synthetic request, deletion verified within 7 days) | Test case |
| 10 | All 5 kill switches tested (per-goal, per-channel, per-account, global, panic hotkey) | Test scenarios |
| 11 | Operator personally signed off on Polis ethical commitments per `14 §6` | Operator commitment doc |
| 12 | Leapable Phase B (operational) — `weekly_active_subscribers` at 1,500+ on target | SoT readback |
| 13 | Operator's daily time stable at ≤90 min routine + Tier-3 extras (≤30 min/day average) | Operator self-report |

**Evidence file:** `marketing-fleet/evidence/phase_3_acceptance.md`.

---

## §11. Risks specific to Phase 3

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Goodhart Auditor false-positives | Medium | Medium | Threshold tuning; operator review of first 30 days of reports |
| Influence Mapper produces unviable paths (bridges too cold or wrong fit) | High | Low | Operator rejects; Mapper learns over time; bridge-strength scoring tuned |
| Tier-3 contact reacts negatively to outreach | Medium | High | Operator drafts/edits every message; Mapper's bridge cultivation has cadence rules; Reflexion captures lessons |
| EU compliance failure on first EU outbound campaign | Low | Critical | Balancing test must be signed before launch; Compliance Check refuses without it |
| Polis goal cannot launch yet (operator-time, seed-city presence) | Medium | Medium | Polis launch can be deferred to post-Phase-4; Phase 3 ships the goal-class capability, not the active goal |
| Cross-goal cost-pool exhaustion | Medium | Medium | Cost Pacer reroutes; operator-set per-goal floors prevent starvation |
| Channel collision between Leapable and Synapse (both target dev audience on operator's X) | High | Low | Scheduler enforces 1h gap; per-goal posting frequency tuned |
| Per-persona scorer Kappa fails for Polis (different voice, smaller training set) | Medium | Medium | Operator labels more samples; iterative re-tune; default to operator-approval-on-everything until Kappa achieved |
| Right-to-erasure deletions cascade incompletely | Medium | High | Test cases cover CF_CONTACTS, suppression lists, audit-log markers; checklist before declaring complete |

---

## §12. Operator decisions required during Phase 3

| Decision | Day deadline | Default if no response |
|---|---|---|
| Provide LinkedIn export + Gmail contacts for Mapper graph | Day 58 | Mapper blocked |
| Curate Tier-3 target list per product (10-30 names per product) | Day 59 | Use default from `11 §1.6 §2.6 §3.6` (less personalized) |
| Sign Polis ethical commitments per `14 §6` | Day 63 | Polis goal-class blocked |
| Label Polis persona samples for Brand Voice Scorer | Day 63 | Polis goal blocked |
| Decide whether Polis goal launches concurrent with Phase 4 (operator-time question) | Day 65 | Default: defer Polis launch until after Phase 4 |
| Decide whether Leapable launch goes into Phase C mega-launch during Phase 4 or later | Day 65 | Default: Leapable Phase C during Phase 4 (per `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md` weekly plan) |
| Phase 3 acceptance sign-off | Day 65 | Phase 4 blocked |

---

## §13. What Phase 3 explicitly does NOT do

- LangSmith production-grade tracing (Phase 4)
- Polished operator dashboard (Phase 4 — Phase 3 dashboard is functional)
- Bench-delta harness for agent upgrades (Phase 4)
- Wiki Gardener pruning (Phase 4)
- Vacation mode / Incident mode CLI (Phase 4)
- Self-audit `marketing audit --depth=full` end-to-end (Phase 4)
- Backup + restore tested (Phase 4)
- LLM-as-judge quarterly recalibration framework (Phase 4)
- Polis active launch (deferred to post-Phase-4 unless operator commits seed-city time)
- Cross-product transfer learning (post-Phase-4 enhancement)

---

## §14. Cross-references

- Multi-product GTM: `docs/marketingplan/11_PRODUCT_PLAYBOOKS.md`
- Polis consumer launch playbook: `docs/marketingplan/13_CONSUMER_SOCIAL_LAUNCH.md`
- Tier model + Audience Sorter: `docs/marketingplan/03_TIER_MODEL_AUDIENCE.md`
- Influence Network Mapping (the full Tier-3 stack): `docs/marketingplan/10_INFLUENCE_NETWORK_MAPPING.md`
- Goodhart Auditor design: `docs/marketingplan/05_AGENT_FLEET_DESIGN.md §6.2` + `20_RISK_AND_FAILURE_MODES.md §4`
- Full compliance regime: `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md`
- Operations cadence with multi-goal: `docs/marketingplan/19_OPERATIONS_AND_CADENCE.md §7 §8`
- Risk and kill switches: `docs/marketingplan/20_RISK_AND_FAILURE_MODES.md`
- Prior phase: `03_PHASE_2_FULL_FLEET_HORMOZI.md`
- Next phase: `05_PHASE_4_PRODUCTION_HARDENING.md`
- Concurrent ops: Leapable Phase B→C transition per `06_PHASE_5_LEAPABLE_90DAY_LAUNCH.md`
