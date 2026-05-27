# Phase 0 — Foundations (Weeks 1-2)

> Goal-spec parser works; RocksDB column families exist; basic CLI shipped; Anthropic SDK wired; leapable MCP integrated for memory queries; LangGraph baseline runs the OuterLoop on a mock channel end-to-end. After Phase 0, the operator can create a test goal, watch a cycle run, hit Ctrl+C, and resume from checkpoint. No external actions yet; the floor of the system exists.

**Source spec docs:** `21_BUILD_ROADMAP.md §2`, `04_SYSTEM_ARCHITECTURE.md §6`, `16_TECH_STACK.md §2 §4 §6`, `22_GOAL_SPEC_FORMAT.md`.

---

## §1. Phase summary

| Field | Value |
|---|---|
| **Duration** | 2 weeks (10 working days) |
| **Prerequisites** | Pre-flight checks in `00_README.md §6` complete |
| **Headline outcome** | `marketing goal create --spec=test.md` works; OuterLoop cycle runs end-to-end on mock channel; Ctrl+C and restart resumes mid-cycle from RocksDB checkpoint |
| **What's NOT in this phase** | Any real external action; any real channel publish; any LLM-as-judge evaluator (deferred to Phase 1); any agent class other than M-tier meta agents |
| **Acceptance gate to Phase 1** | All §5 acceptance checks pass |
| **Operator time** | ~10-15 hours review/decisions over 2 weeks; mostly engineering work |

---

## §2. Deliverables

### 2.1 Code modules to create

| Module path (proposed) | Purpose | Spec ref |
|---|---|---|
| `marketing-fleet/src/goal_spec/parser.py` | Pydantic schema for `GoalSpec`; markdown → struct; validation rules from `22_GOAL_SPEC_FORMAT.md §4` | spec §22 |
| `marketing-fleet/src/goal_spec/schema.py` | The `GoalSpec`, `Target`, `Guardrails`, `Autonomy`, `KillSwitches`, `Deadline` Pydantic models | spec §22 |
| `marketing-fleet/src/goal_spec/validators.py` | Per-product allowed-metrics list; vanity-metric deny-list; deterministic per-field checks | spec §22 §4 |
| `marketing-fleet/src/storage/rocksdb_cfs.py` | RocksDB CF registration: 11 CFs from §2.3 below | spec §04 §6 |
| `marketing-fleet/src/storage/cf_goals.py` | CF_GOALS read/write helpers; CRUD with audit-log emission | spec §04 §6 |
| `marketing-fleet/src/storage/cf_goal_cycles.py` | CF_GOAL_CYCLES append-only writer | spec §04 §6 |
| `marketing-fleet/src/storage/cf_experiment_nodes.py` | CF_EXPERIMENT_NODES DAG state | spec §04 §6 |
| `marketing-fleet/src/storage/cf_approval_queue.py` | CF_APPROVAL_QUEUE pending items | spec §04 §6 |
| `marketing-fleet/src/storage/cf_audit_log.py` | CF_AUDIT_LOG append-only with cryptographic chaining | spec §14 §7 |
| `marketing-fleet/src/storage/cf_kill_switches.py` | CF_KILL_SWITCHES cached flags with ≤2s TTL | spec §02 §17 |
| `marketing-fleet/src/outerloop/states.py` | LangGraph StateGraph nodes: `assess`, `plan`, `atomize`, `dispatch`, `aggregate`, `evaluate`, `communicate` | spec §04 §2 |
| `marketing-fleet/src/outerloop/cron.py` | Hourly cron scheduler; per-goal cycle dispatch; idempotency keys | spec §04 §2 |
| `marketing-fleet/src/outerloop/checkpointing.py` | LangGraph durable execution → RocksDB checkpoint adapter | spec §04 §6 §16 §2 |
| `marketing-fleet/src/llm/anthropic_client.py` | Anthropic SDK wrapper with model-routing (Opus / Sonnet / Haiku); prompt caching | spec §16 §3 §12 |
| `marketing-fleet/src/llm/model_routing.py` | Per-agent-class model selection from agent card | spec §16 §3 |
| `marketing-fleet/src/memory/leapable_client.py` | leapable MCP client wrapper: `wiki_search`, `wiki_get`, `vault_select` | spec §16 §4.2 |
| `marketing-fleet/src/memory/wiki.py` | LLM Wiki abstraction layered over leapable + filesystem markdown | spec §04 §5 |
| `marketing-fleet/src/cli/main.py` | Click/Typer CLI entry point: `goal create|list|status|pause|resume|kill`, `approve`, `audit`, `wiki query` | spec §22 §7 |
| `marketing-fleet/src/cli/mock_channel.py` | Mock channel sink for Phase 0 end-to-end testing (no external IO) | this doc |
| `marketing-fleet/tests/test_goal_parser.py` | Round-trip + validation tests for every error code in `22 §4` | spec §22 §4 |
| `marketing-fleet/tests/test_outerloop_resume.py` | Ctrl+C mid-cycle, restart, verify resumption from last checkpoint | spec §04 §6 |
| `marketing-fleet/tests/test_audit_log_chain.py` | Cryptographic chain integrity of CF_AUDIT_LOG | spec §14 §7 |

### 2.2 Configuration files

| File | Purpose |
|---|---|
| `marketing-fleet/config/anthropic.yaml` | API key reference (keychain pointer); model identifiers; rate caps |
| `marketing-fleet/config/leapable.yaml` | MCP endpoint; vault allowlist per agent class (initial placeholder) |
| `marketing-fleet/config/rocksdb.yaml` | DB path; CF list; checkpoint cadence (default every 30s during active cycle) |
| `marketing-fleet/goals/test_mock.md` | A single test goal spec used for end-to-end Phase 0 validation |

### 2.3 RocksDB column families to register

Per `04_SYSTEM_ARCHITECTURE.md §6` + `16_TECH_STACK.md §6.1`:

| CF | Key | Value | Notes |
|---|---|---|---|
| `CF_GOALS` | `goal_id` | `GoalSpec` JSON | Active + historical goal specs |
| `CF_GOAL_CYCLES` | `(goal_id, cycle_ts_ns)` | `CycleRecord` JSON | Every assess→plan→build→communicate pass, with timestamps + summary diffs |
| `CF_EXPERIMENT_NODES` | `(goal_id, node_id)` | `ExperimentNode` JSON | DAG state per goal |
| `CF_APPROVAL_QUEUE` | `approval_id` | `ApprovalItem` JSON | Pending HITL items |
| `CF_REFLECTIONS` | `(channel, tactic, ts_ns)` | `Reflection` JSON | Reflexion memory |
| `CF_WIKI_INDEX` | `wiki_path_sha256` | `(path, last_updated, refs)` | Filesystem-wiki index |
| `CF_CONTACTS` | `contact_id` | `Contact` JSON with tier | Per-contact tier classification (Phase 0 stubbed; populated Phase 2-3) |
| `CF_AUDIT_LOG` | `audit_id` (UUIDv7) | `AuditRecord` JSON | Append-only, cryptographically chained |
| `CF_CHANNEL_STATE` | `(channel, account_id)` | `ChannelState` JSON | Rate-limit buckets, posting-history hashes |
| `CF_PERSONA_HISTORY` | `(persona, channel, post_hash)` | timestamp | Per-persona dedup history (Phase 2+ usage) |
| `CF_KILL_SWITCHES` | `(scope, switch_id)` | `(tripped: bool, reason: str, ts: int)` | Scope = `global`, `goal:<id>`, `channel:<name>`, `account:<id>`; TTL ≤2s in memory cache |

`CF_ATTRIBUTION_GRAPH` is deferred to Phase 2 when the Cohort Health Auditor lights up.

### 2.4 CLI surface (Phase 0 subset)

```bash
marketing goal create --spec=goals/<id>.md   # parse + validate + activate (mock-channel only in Phase 0)
marketing goal list                          # active goals
marketing goal status <goal_id>              # current predicate + spend + alerts
marketing goal pause <goal_id>               # operator pause (state preserved)
marketing goal resume <goal_id>              # resume from last checkpoint
marketing goal kill <goal_id> --reason="…"   # terminate; state archived
marketing wiki query "<query>"               # leapable-backed vault search test
marketing audit --since=24h                  # dump audit log
marketing kill                               # global emergency stop (works even with no active goals)
```

Approval queue commands (`approve`, `reject`) defer to Phase 1.

---

## §3. Week-by-week breakdown

### Week 1 — Schema + Storage + LLM + Memory

**Day 1-2 (Mon-Tue) — Goal spec parser**
- Implement `GoalSpec` Pydantic schema with every field from `22 §2`.
- Implement all 13 validation rules from `22 §4`; each returns its specific `SCREAMING_SNAKE_CASE` error code.
- Write `tests/test_goal_parser.py` with one positive test per product + one negative test per validation error code.
- Operator decision: any operator-specific overrides to default unit-economics floors? (Default: per `22 §5`.)

**Day 3-4 (Wed-Thu) — RocksDB CFs + audit log**
- Register all 11 CFs from §2.3.
- Implement `cf_audit_log.py` with cryptographic chaining (SHA-256 of `(prev_hash, audit_record_json)`).
- Write `tests/test_audit_log_chain.py` — verify chain integrity after 1000 random writes + 1 forged record (must detect).
- Operator decision: where lives the RocksDB instance? (Recommended: operator host at `~/.marketing-fleet/db/`.)

**Day 5 (Fri) — Anthropic + Leapable wiring**
- Implement `anthropic_client.py` with model-routing per `16 §3`. Wire Opus / Sonnet / Haiku as named identifiers.
- Implement `leapable_client.py` against the operator's live leapable MCP server. Test `wiki_search` returns results from the `AlexHormoziTranscripts` vault.
- Verify prompt-caching headers being sent on system-prompt-heavy calls.

### Week 2 — OuterLoop + Mock Channel + CLI

**Day 6-7 (Mon-Tue) — LangGraph OuterLoop baseline**
- Define StateGraph with nodes `assess`, `plan`, `atomize`, `dispatch`, `aggregate`, `evaluate`, `communicate`.
- Each node has a stub Phase-0 implementation:
  - `assess` — read goal status from CF_GOALS, emit `AssessmentReport` (mostly empty in Phase 0)
  - `plan` — propose one mock experiment node ("post mock content to mock channel")
  - `atomize` — mark mock node atomic
  - `dispatch` — invoke mock-channel sink
  - `aggregate` — collect mock outcome
  - `evaluate` — no-op in Phase 0 (full Evaluator stack in Phase 1)
  - `communicate` — write cycle summary to CF_GOAL_CYCLES
- Implement LangGraph durable checkpointing → RocksDB.

**Day 8 (Wed) — Cron + idempotency**
- Implement hourly cron in `outerloop/cron.py`. Default once-per-hour per goal; configurable.
- Idempotency keys on every external-action stub (Phase 0 mocks must still carry idempotency keys so Phase 1+ inherits the discipline).

**Day 9 (Thu) — CLI shipping**
- Implement the CLI surface listed in §2.4.
- Wire `marketing audit` to dump CF_AUDIT_LOG with filtering.
- Operator-driven: try creating a goal, observing 2-3 cycles, killing it. Document any UX friction.

**Day 10 (Fri) — End-to-end shakedown + restart test**
- Create `goals/test_mock.md` with a goal that hits the mock channel.
- Run for 2 hours; verify 2 cycles complete; verify audit log has expected entries.
- Kill the process mid-cycle (during `dispatch` ideally). Restart. Verify the cycle resumes and completes without duplicate audit entries (idempotency check).
- Write the Phase 0 acceptance evidence file (per §5).

---

## §4. Phase 0 architecture decisions to lock

| Decision | Recommended default | Rationale | When operator can override |
|---|---|---|---|
| Database — RocksDB vs Postgres for primary state | **RocksDB** | Same primitive as Leapable; single-host doctrine (`16 §8`); CF model fits the audit + cycle + experiment-node schemas | Never (architectural invariant) |
| LangGraph version | **Pin to current stable** | Production tier maturity; checkpoint API stability | At Phase 4 hardening if needed |
| Filesystem-wiki vs RocksDB-wiki | **Filesystem markdown indexed by CF_WIKI_INDEX** | Karpathy Obsidian pattern (`16 §6.3`); operator can browse with normal editor | Never (operator-readability invariant) |
| Cron implementation | **System cron + lockfile** | Simple, durable across reboots; one-host doctrine | Phase 4 hardening if multi-host needed |
| Mock channel for Phase 0 | **In-memory queue with audit-log writes** | Validates the end-to-end flow without external IO risk | N/A (Phase 0 only) |
| Audit log chaining | **SHA-256 of (prev_hash, record)** | Standard for tamper-evidence; cheap | Never (compliance invariant) |
| Kill switch TTL | **≤2s cached** | Per `02 §17`; balances reaction time vs read load | Never (doctrine invariant) |
| Anthropic prompt caching | **Enabled for all Planner / Reflector / Evaluator calls** | 90%+ hit rate on system-prompt-heavy meta agents (`16 §12`) | Phase 4 if cost analysis suggests otherwise |

---

## §5. Acceptance criteria (gate to Phase 1)

Phase 0 is complete when **all** of the following are demonstrably true:

| # | Criterion | How to verify |
|---|---|---|
| 1 | `marketing goal create --spec=goals/test_mock.md` succeeds; `marketing goal list` shows it | CLI output |
| 2 | Every validation error from `22 §4` triggers correctly on a crafted bad spec | `pytest tests/test_goal_parser.py -v` |
| 3 | All 11 CFs from §2.3 are registered and accept reads + writes | `marketing audit --since=1h` shows CF activity |
| 4 | Hourly cron fires; `CF_GOAL_CYCLES` has a row per cycle with non-trivial summary | `marketing goal status <goal_id>` shows cycle count growing |
| 5 | OuterLoop completes the mock cycle without errors | Audit log has `cycle_complete` event |
| 6 | Process killed mid-cycle, restarted; resumes from last checkpoint; **no duplicate audit-log entries for the resumed action** | Idempotency test — pre-restart audit log + post-restart audit log have unique idempotency keys; resumed action does not double-fire |
| 7 | `marketing wiki query "hormozi referral mechanics"` returns ≥3 results from leapable | CLI output with vault provenance |
| 8 | `marketing kill` halts the cron immediately (≤2s) | Kill-switch check via dashboard or `marketing goal status` |
| 9 | Audit log chain integrity passes on 1000 random writes | `pytest tests/test_audit_log_chain.py` |
| 10 | Operator has reviewed and signed off on the agent-card schema (deferred from `05 §2` template) that Phase 1 will populate | Operator approval comment in `marketing-fleet/agents/_card_schema.md` |

**Evidence file:** create `marketing-fleet/evidence/phase_0_acceptance.md` with this matrix filled in (command, output snippet, pass/fail) before declaring Phase 0 complete.

---

## §6. Risks specific to Phase 0

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| LangGraph version churn during Phase 0 | Medium | Medium | Pin to a specific stable version; do not auto-upgrade until Phase 4 |
| RocksDB schema mistakes that require migration | Medium | High | No backwards compat doctrine (per Leapable §6.8) — wipe DB if schema needs change during Phase 0; document the wipe in audit log |
| Leapable MCP authentication / network issues | Medium | Medium | Test the connection on Day 5; if blocked, escalate before Day 6 LangGraph work depends on it |
| Goal-spec parser over-strict, rejects valid operator inputs | Medium | Low | Use the three example specs from `22 §8` as canonical positive tests |
| Cron lock-file race conditions (two cron firings overlap) | Low | High | Use `flock` or equivalent; abort second firing if first is still active; log abort |
| Idempotency keys not actually unique across restart | Medium | High | UUIDv7 with monotonic component; test 7 explicitly |
| Audit-log chain corruption from concurrent writes | Medium | Critical | Single writer per CF; serialize writes through a queue if needed |

---

## §7. Operator decisions required during Phase 0

These cannot be decided by the engineer alone. Block on operator response:

| Decision | Day deadline | Default if no response |
|---|---|---|
| RocksDB instance location | Day 3 | `~/.marketing-fleet/db/` on operator host |
| Anthropic API key rotation policy | Day 5 | Monthly rotation; keychain-stored |
| Goal-spec defaults override (LTV:CAC, budgets) | Day 9 | Use `22 §5` defaults |
| Sign-off on the agent-card schema for Phase 1 | Day 10 | Use the example from `05 §2` |

---

## §8. What Phase 0 explicitly does NOT do

- No content production (P1, P2 deferred to Phase 1)
- No external publishing (D1 deferred to Phase 1)
- No outreach (D4 deferred to Phase 2)
- No paid ads (O4 deferred to Phase 2 with operator approval)
- No tier classification (S1 deferred to Phase 2-3)
- No Influence Network Mapper (S3 deferred to Phase 3)
- No Goodhart Auditor (S4 deferred to Phase 3)
- No brand voice fine-tuning (initial Brand Voice Scorer ships Phase 1)
- No LLM-as-judge calibration (deferred to Phase 4 quarterly cadence)
- No real-channel computer-use (Synapse integration deferred to Phase 1 single channel; Phase 2 multi-channel)

Phase 0 is **floor only**. It is intentionally boring.

---

## §9. Cross-references

- Goal spec format (the schema being parsed): `docs/marketingplan/22_GOAL_SPEC_FORMAT.md`
- System architecture (the OuterLoop being built): `docs/marketingplan/04_SYSTEM_ARCHITECTURE.md`
- Tech stack (the tool choices being implemented): `docs/marketingplan/16_TECH_STACK.md`
- Doctrine the build must respect: `docs/marketingplan/02_FIRST_PRINCIPLES.md`
- Audit log schema: `docs/marketingplan/14_COMPLIANCE_AND_ETHICS.md §7`
- Next phase: `02_PHASE_1_SINGLE_PRODUCT_E2E.md`
- Repository layout: `09_REPOSITORY_LAYOUT_AND_AGENT_BUILD_ORDER.md`
- Guardrail engine (skeletal in Phase 0, fully built in Phase 1): `10_TOOL_INTEGRATIONS_AND_GUARDRAILS.md`
- Acceptance discipline: `11_ACCEPTANCE_AND_BENCH_HARNESS.md`
