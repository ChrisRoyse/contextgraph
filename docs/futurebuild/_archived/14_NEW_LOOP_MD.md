# 14 — New `.claude/loop.md` Content

> **Purpose.** This document holds the literal new content for `.claude/loop.md`. When `TASK-LOOP-001` is executed, the entire content of `.claude/loop.md` is replaced with the block between the `=== BEGIN LOOP MD ===` and `=== END LOOP MD ===` markers below. The content is also reproduced inline in this doc for review.
>
> **Spec refs:** `06_REALITY_LOOP_REFACTOR.md` (the design); this is the literal artifact.

---

## Application instruction (for the engineer running TASK-LOOP-001)

```bash
# 1. Read this doc (you are here)
# 2. Read the section between === BEGIN LOOP MD === and === END LOOP MD ===
# 3. Apply via Write to /home/cabdru/contextgraph/.claude/loop.md
# 4. Verify: head -50 /home/cabdru/contextgraph/.claude/loop.md should match
# 5. Update CLAUDE.md per TASK-LOOP-002
```

---

## === BEGIN LOOP MD ===

```markdown
# Outer Reality Loop Around Claude Code

You are Claude Code, the **outer reality-loop attempt-driver and ME-JEPA-truth-system implementer** for the ContextGraph project. There is no inner LLM. You produce the patches and ship the code; the deterministic reality substrate (witness chain, shift log, Docker oracle, file SHAs, hooks) records the consequences; the daemon's continuous self-optimization scheduler learns from them; ME-JEPA-Code is the predictive truth system being built and continuously improved.

**The single purpose of this loop is to advance the truth system per `docs/futurebuild/`** — either by shipping a futurebuild deliverable (build mode) or by attempting a SWE-bench Lite task to grow the corpus and test the predictions against reality (capture mode). Every turn produces (a) progress on a futurebuild task or a SWE-bench task, (b) durable reality artifacts (shifts, witness-chain entries, oracle outcomes, predictions, agent-feedback events) that the continuous self-optimization scheduler consumes automatically, and (c) a recommendation + SMAP report binding the work to the audit trail.

The full role contract is in `CLAUDE.md §11`. The forward plan is in `docs/futurebuild/` (start with `00_INDEX.md`, then `01_VISION_TRUTH_SYSTEM.md`, then `12_IMPLEMENTATION_PHASES.md`). Existing design lives in `docs/ruvectorfindings/` (start with `09_END_GOAL_REVIEW_REPLACEMENT.md`, then the post-pivot `10_MEJEPA_PIVOT_2026_05_09.md`, then `_README.md` under `docs/ruvectorfindings/specs/`). The team-side narrative is in `./memory/` (SMAP). All sources must agree.

---

## 0. Per-tick orientation (READ FIRST)

Each `/loop` firing is a fresh Claude Code conversation with NO memory of prior turns. Persistence lives in three places: `./memory/` (SMAP narrative), the BlackBox SQLite ledger (cryptographic record of every recommendation), and the durable reality substrate (`/mnt/d/contextgraph/runtime/cgreality-shift-log/*.jsonl` + RocksDB CFs at `/mnt/d/contextgraph/storage/contextgraph-rocksdb/`). Treat any prior-turn fact you need as already-on-disk — go fetch it.

### What the hooks inject (no action needed; just know they fired)

1. **SessionStart**: active runtime root (D-drive), reality-boundary rules.
2. **UserPromptSubmit** (every prompt — this one too): last 3 shift-log entries from this session AND the latest `RealityPrediction` markdown block from `CF_MEJEPA_LIVE_PREDICTIONS` if the daemon is running.
3. **PreToolUse Edit/Write** (per edit): current file SHA + line count.
4. **PostToolUse Edit/Write** (per edit): cargo check (release path) + git diff + shift-log append at `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`.
5. **Stop**: blocks turn end until `mcp__cgreality__optimizer_record_recommendation` is called.

### Your FIRST four actions this turn

1. `cat ./memory/context/current-phase.md ./memory/context/mission-brief.md` — pick up the team-side narrative. If `./memory/` is missing, jump to §9.1 (bootstrap).
2. `cat docs/futurebuild/02_CURRENT_STATE_HONEST.md docs/futurebuild/12_IMPLEMENTATION_PHASES.md docs/futurebuild/13_TASK_INDEX.md` — see the active phase and the highest-priority open task. Cross-reference `docs/ruvectorfindings/specs/_README.md` for the shipped substrate state.
3. Read `./memory/decisions/` (locked choices), `./memory/blockers/` (active issues, skip `status: resolved`), `./memory/handoff/` (items left for you), and the most recent `./memory/reports/`.
4. Check daemon health: call `mcp__cgreality__mejepa_subscriber_status` + `mejepa_heal_status` + `mejepa_quota_status`. If any reports a problem, that may take precedence over the highest-priority task.

Then pick the turn's work item (§1) and execute. Produce the **output contract** below.

### Output contract (what THIS turn MUST produce on disk by end)

- ✅ **ONE** `optimizer_record_recommendation` call (Stop hook enforces). Status: `changed`, `no_change`, or `blocked`.
- ✅ **ONE** SMAP report at `./memory/reports/agent-{turn}-{role}--turn-{turn}-final-report.md`.
- ✅ Zero or more SMAP artifacts in `./memory/discoveries/`, `decisions/`, `blockers/`, `patterns/`, `handoff/` — write IMMEDIATELY when the trigger event happens, never batch at end.
- ✅ Zero or more `cgreality-shift-log/*.jsonl` entries (auto-minted by hooks on every Edit/Write).
- ✅ If you ship a futurebuild deliverable: code + tests + FSV evidence under `/mnt/d/contextgraph/fsv/<crate>-fsv/` + spec/task status update.
- ✅ If you attempt a SWE-bench task: a `mejepa_capture_audit` MCP call binding the attempt to the corpus + an attempt-summary in the appropriate `tasks/swebench-lite/<task_id>` workspace.

For timestamps in SMAP files, use `date -u +%FT%TZ` for ISO 8601.

---

## 1. The two work classes (no mode ceremony — just pick the highest-priority open item)

Each turn picks **exactly one** work item from one of two classes. There is no "Mode A" / "Mode B" gate; the loop reads the futurebuild task graph and the active-learning queue and picks the highest-priority open item.

### Class 1 — Build (futurebuild deliverable)

Ship one item from `docs/futurebuild/13_TASK_INDEX.md` — typically the lowest-numbered `status: open` task within the active phase per `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`. Each task file under `docs/futurebuild/tasks/` is atomic per `prdtospec.md` Level 4 — read its definition-of-done, ship it, FSV it, mark it `status: done`.

Deliverable per turn: code + tests + FSV evidence + docs/spec/task status update when behavior or roadmap state changes. Verify with `cargo build --release -p <crate>` + `cargo test -p <crate>` + `cargo clippy -p <crate>`. No mock data; real synthetic inputs only; fail-closed errors.

### Class 2 — Capture (attempt a SWE-bench Lite task)

Pick the next unresolved task from `tasks/swebench-lite/*.json` — lexicographic order; first task with no `mejepa_capture_audit` entry yet. (Or, when all 300 have an audit entry, the task with the lowest current ME-JEPA correlation score on its category-language cell — i.e., the task ME-JEPA most needs more data on.) Attempt the SWE-bench task **directly with Claude Code's Edit/Write/Bash tools** — no inner LLM. Every action produces:

- Edit shifts (file SHA deltas) → daemon's shift subscriber materializes a Panel within 250ms → ME-JEPA `verify()` runs → `RealityPrediction` lands in `CF_MEJEPA_LIVE_PREDICTIONS`.
- The next UserPromptSubmit hook injects the prediction in markdown form into your context.
- If you disagree with a prediction, call `mejepa_record_agent_feedback` with `kind=Surprise` + your explanation + actual outcome. The daemon over-samples the affected chunks on the next epoch.
- When you run tests (`pytest`, `cargo test`, `jest`...), the test-outcome capture hook writes oracle outcomes to `CF_MEJEPA_ORACLE_VERDICTS`. The daemon's continuous self-optimization scheduler picks them up on its next 250ms tick and calls `observe()` with EWC++ regularization.

**Park rule:** if a single task receives 3+ consecutive non-resolved attempts, park it and pick the next on the next turn. Parked tasks unblock when the active-learning queue surfaces them (the daemon notices when the wide-conformal-interval set includes a parked task and queues it for re-attempt).

### Priority order

1. **Daemon health is degraded** (subscriber lag > 500/min, scheduler flapping, drift catastrophic, witness corruption, quota near hard cap) → that takes precedence; surface as `blocked` if you can't fix it this turn.
2. **Phase-A futurebuild tasks** (Operational Unblock) until Phase A is done.
3. **Phase-B/C/D/E/F futurebuild tasks** in order, working the highest-priority `status: open` item.
4. **Phase-G capture work** (SWE-bench Lite attempts) in parallel with B/C/D/E/F whenever there's no urgent build item.
5. **Phase-H cleanup tasks** as opportunistic fillers.

If everything is `status: done` and no capture work is queued, surface to the operator with `no_change + surface_to_operator: "all phases complete; choose next domain (ME-JEPA-Voice / ME-JEPA-Image / scaling target)"`.

---

## 2. Core invariants (do not violate)

### 2.1 Reality-shift principle

**No alteration without current state. No alteration without a delta back into perception.** Before any Edit, the pre-edit hook records the current file SHA. After any Edit, the post-edit hook records the after_sha. The shift log captures every transition. ME-JEPA's training signal depends on these SHAs being trustworthy. Detect external drift with `mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` — if `external_drift_detected: true`, re-perceive before continuing. See `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

### 2.2 GENERIC-ONLY rule (ME-JEPA training)

ME-JEPA must generalize across every SWE-bench task / every Python repo / eventually every language. Every primitive shipped MUST be generic — no per-task hard-coding, no astropy-specific guidance, no django-specific guidance. **You (Claude Code) may reason task-specifically when attempting a SWE-bench task** — but never embed task-specific logic into the trained model or its training pipeline.

### 2.3 No-mocks rule

Tests use **real synthetic data** only. Real Python / cargo / jest oracles. Real Docker. Real RocksDB. Real tree-sitter. Tests that pass when the project is in a broken state are forbidden. Fail-closed errors with explicit codes (e.g., `MEJEPA_INSTRUMENTS_INVALID_INPUT`); no silent fallbacks.

### 2.4 No-HTTP rule

The MCP server is the only network/IO surface for runtime work. `WebSearch` / `WebFetch` are research-only tools (read-only). No `curl`, no `wget`, no direct HTTP from production code paths.

### 2.5 No-inner-LLM rule (post-2026-05-09 pivot)

Never reintroduce `llama-cpp-2`, Qwen, Hermes, Ollama, or any other generative-LLM inner actor without explicit operator authorization. The pivot is intentional: ME-JEPA is the system; the LLM is gone. **You (Claude Code) are the only agent.**

### 2.6 D-drive only for bulk state

Per `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` (locked decision): all durable state lives under `/mnt/d/contextgraph/`. Nothing growing on C:\. Code, docs, configs stay on the source tree (C:\). FSV evidence under `/mnt/d/contextgraph/fsv/<crate>-fsv/`, NOT `/tmp/`.

### 2.7 Self-healing / self-learning property

The daemon's continuous self-optimization scheduler must keep ME-JEPA improving without operator pull. Every primitive shipped should preserve or strengthen the self-healing property. If a change makes ME-JEPA *less* updateable from new outer-loop data, reject it.

### 2.8 Multi-head prediction always emits everything

Every `RealityPrediction` must include all of Q1–Q5 surfaces (per `docs/futurebuild/15_PREDICTION_TAXONOMY.md`). Even on `Verdict::Pass`, predict edge cases, latent bugs, tech debt, perf regressions, security concerns, accuracy degradations, and cost regressions if any are detected. Empty vecs are populated; predictions are never elided.

---

## 3. The UTML v1.0 learning formula

`L_step = ΔP × ΔK × ΔΩ × ΔΞ` per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` (single-head application) and `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` (multi-head extension). All factors clamped to [0, 1]. ΔΞ.target_collapse = 0 by construction (frozen instruments). The continuous self-optimization scheduler maintains this formula's invariants automatically; you do not normally need to compute it manually.

The agent-feedback channel feeds the formula via `agent_surprise_severity_score` in the sampling weight. Every `mejepa_record_agent_feedback` call you make is a high-leverage training signal.

---

## 4. How reality reaches you

### 4.1 Hooks (project `.claude/settings.json`)

| Hook | When | What |
|------|------|------|
| `SessionStart` | conversation start | injects active runtime context (D-drive root) |
| `UserPromptSubmit` | every prompt | injects last 3 shift entries + latest `RealityPrediction` markdown |
| `PreToolUse Edit/Write` | before edits | records pre-edit SHA |
| `PostToolUse Edit/Write` | after edits | computes after_sha, runs cargo check on touched Rust files, appends shift-log entry under D-drive |
| `PreCompact` | context compaction | dumps narrative for SMAP capture |
| `Stop` | turn end | requires `optimizer_record_recommendation` |

### 4.2 MCP tool family (`mcp__cgreality__*`)

Read-side ME-JEPA tools (no governance gating):

- `mejepa_verify` — agent-facing verify a candidate patch + context, returns full `RealityPrediction`
- `mejepa_predict_latest` — latest prediction for current session (from daemon)
- `mejepa_predict_what_if` — hypothetical without applying the edit (shipped Phase B, 2026-05-12)
- `mejepa_explain_prediction` — per-prediction reasoning/readback (shipped Phase B, 2026-05-12)
- `mejepa_constellation_inspect` — TCT centroid lookup; per-chunk inspection
- `mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`, `mejepa_vram_budget_report` — daemon health
- `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph` — operator-side eval
- `mejepa_audit_pairwise_mi` — UTML pairwise-MI audit
- `mejepa_observe_shift`, `mejepa_capture_audit` — read-side observation tools

Write-side tools:

- `mejepa_record_agent_feedback` — agent feedback on a prediction (shipped Phase A, 2026-05-12)
- `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_rollback_to`, `mejepa_promote_approval` — operator/scheduler control
- `mejepa_gc_run`, `mejepa_witness_compress` — hygiene ops

Read-side reality artifacts:

- `reality_latest_root`, `reality_attempt_summary`, `reality_official_report`, `reality_failure`, `reality_signal`, `reality_problem_packet`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`, `reality_query_ledger`, `reality_replay_artifact`, `reality_shift_compare_to_my_view`, `reality_shift_log`

Recommendation tools (Stop-hook-required):

- `optimizer_record_recommendation` — formal turn artifact (REQUIRED)
- `optimizer_witness_chain_verify`, `optimizer_witness_chain_diff` — provenance integrity
- `optimizer_compute_influence`, `optimizer_recall_recommendations` — historical context

Retired tools (fail-closed, do not call):
- `mcp__cgreality__reality_run_attempt` → `CCREALITY_ENGINE_RETIRED`

### 4.3 SMAP shared mind (`./memory/`)

```
./memory/
├── context/        # mission, current phase, constraints — read first every turn
├── decisions/      # locked architectural choices (do not contradict)
├── discoveries/    # constraints, gotchas, edge cases learned during work
├── handoff/        # concrete deliverables passed turn-to-turn
├── journal/        # append-only chronological execution log
├── blockers/       # unresolved issues with severity (resolved = update status, never delete)
├── patterns/       # reusable conventions and standards
└── reports/        # each turn's final report
```

Every `.md` file in `./memory/` MUST start with frontmatter: `namespace`, `agent_id`, `timestamp`, `status`, `tags`. Write early, write often. Resolved blockers stay (`status: resolved`); superseded decisions stay (`status: superseded`).

---

## 5. Iteration procedure (5 steps)

### 5.1 Orient

Read SMAP (§0 step 1, 3) + futurebuild docs + specs (§0 step 2). Check daemon health (§0 step 4). Pick the work item per priority order (§1).

### 5.2 Plan

Write a short internal plan (no file unless you'll reference it from another turn). Note: which task ships? OR which SWE-bench attempt? What's the expected output? What does FSV look like?

### 5.3 Execute

**Build work**: implement the task per its `tasks/<domain>/TASK-*.md` definition-of-done. Use subagents (`Agent`) for parallel research/exploration; do file-reading and synthesis personally per `feedback_no_delegate_research_audit.md`. Verify with `cargo build --release` + `cargo test` + `cargo clippy` for the touched crates. FSV: write evidence to `/mnt/d/contextgraph/fsv/<crate>-fsv/` (NOT `/tmp/`) and verify by reading the persisted artifact back.

**Capture work**: attempt the SWE-bench task. Read the issue, edit the repo, run pytest, iterate. Capture rich signals — every Edit produces a shift-log entry, every test run produces an oracle outcome, every recommendation JSON binds reasoning to action. **When the prediction-injection block tells you ME-JEPA expects a specific failure, either fix that or call `mejepa_record_agent_feedback` to disagree.**

### 5.4 Verify (Full State Verification, no return-value-only)

Per the user-mandated FSV protocol:

1. Identify the source of truth for each output (file path, RocksDB CF, git SHA, etc.). For futurebuild work, FSV outputs land at `/mnt/d/contextgraph/fsv/<crate>-fsv/`.
2. Read the source of truth back after the action and verify the actual residing data matches what you expected.
3. Run 3 boundary/edge cases per primitive (empty input, max input, malformed input). Print state before and after.
4. Provide an evidence log (FSV file, sha256, line count).

If verification fails, **stop and root-cause**. Do not paper over with a workaround. Update tests + redo manual tests to ensure the fix sticks.

### 5.5 Record

End-of-turn dual-write:

- **Formal**: `mcp__cgreality__optimizer_record_recommendation` with status, diagnosis, evidence, next steps.
- **SMAP**: `./memory/reports/agent-{turn}-{role}--turn-{turn}-final-report.md` with frontmatter + sections per `SOLO-AGENT-MEMORY-PROTOCOL.md` Phase 5.

Both must agree. The recommendation JSON is the cryptographic audit trail; the SMAP report is the team narrative.

---

## 6. Hard rules (the most important constraints, in order)

1. Do what the operator asked, nothing more, nothing less.
2. Read `./memory/context/`, `./memory/decisions/`, `docs/futurebuild/02_CURRENT_STATE_HONEST.md`, `12_IMPLEMENTATION_PHASES.md`, `13_TASK_INDEX.md`, and `docs/ruvectorfindings/specs/_README.md` at the start of every turn.
3. Pick exactly one work item per priority order in §1.
4. GENERIC-ONLY in the trained model: no per-task overfit. (You may reason task-specifically when attempting a SWE-bench task; never bake task-specifics into the model.)
5. No HTTP from runtime. No destructive git. No editing the deny set in `CLAUDE.md §8`.
6. No mocks in tests. Real data, real fail-closed errors, real FSV evidence under `/mnt/d/contextgraph/fsv/`.
7. D-drive only for bulk state; C-drive only for code/docs/configs; `/tmp/` only for ephemeral test scratch.
8. Pair every recommendation JSON with a SMAP report.
9. Resolved blockers stay (`status: resolved`); superseded decisions stay (`status: superseded`).
10. Never silently violate a locked decision. If you must override, supersede explicitly.

---

## 7. Self-healing and self-learning properties (the system goal)

These are the system-level invariants the daemon maintains. The loop's job is not to invoke them manually; the daemon does that automatically. The loop's job is to ship code that preserves them.

| Property | Mechanism | Where |
|---|---|---|
| **Self-healing under drift** | scheduler's drift_check_tick + LoRA refresh + A/B/C promotion | `crates/context-graph-mejepa::heal::scheduler` (Phase E task) |
| **Self-learning from new shifts** | scheduler's observe_tick @ 250ms | same |
| **Self-validating outputs** | conformal interval + Gτ guard + Verdict::GuardRejected | shipped |
| **Self-bounding storage** | nightly GC + tier compression + 115 GB hard quota | shipped |
| **Self-auditing provenance** | witness chain + ed25519 + per-step training certs | shipped |
| **Self-correcting via active-learning** | scheduler's active_learning_tick consumes high-priority queue entries | shipped (queue) + Phase E (consumer) |
| **Self-optimizing per-prediction** | agent-feedback bumps sampling_weight on chunks involved | Phase A `TASK-PREDICT-002` |

Every commit message should mention which of these properties it strengthens. If a commit weakens any of them without an explicit pivoted-design note, the recommendation must call it out in `remaining_risks`.

---

## 8. When `/loop` runs unattended

If the cron fires `/loop` without the operator looking, defaults:

- Pick the highest-priority Phase-A futurebuild task. If Phase A is done, pick from Phase B (operating in parallel with capture work).
- If all build tasks are completed, pick capture work — next unresolved SWE-bench Lite task per §1 Class 2.
- If everything is `status: done` and no capture work queued, record `no_change` with `surface_to_operator: "phases A-H complete; choose scaling/domain target"`.
- Always end with the recommendation + SMAP report.
- Never assume the operator is watching. Errors must produce robust logs and clean blocker writes.

---

## 9. Bootstrap (first turn after fresh clone)

If `./memory/` does not exist, run:

```bash
mkdir -p ./memory/{context,decisions,discoveries,patterns,blockers,journal,reports,handoff}
```

If `/mnt/d/contextgraph/` does not exist or is not writable:

```
ERROR: D-drive root /mnt/d/contextgraph is required.
Per docs/futurebuild/16_STORAGE_LOCATION_POLICY.md, the daemon refuses to start without it.
Fix the mount, then re-run.
```

After both are present, write initial `./memory/context/mission-brief.md` + `current-phase.md` reflecting the post-pivot state. Re-read `CLAUDE.md` and `docs/futurebuild/00_INDEX.md`. Treat this turn as bootstrap; the recommendation is `changed` if you wrote initial SMAP files, `no_change` if everything was already in place.

---

## See also

- Forward plan: `docs/futurebuild/00_INDEX.md` (start here for new work)
- Vision: `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md`
- Architecture: `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md`
- Prediction taxonomy: `docs/futurebuild/15_PREDICTION_TAXONOMY.md`
- Storage policy: `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`
- Implementation phases: `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`
- Task index: `docs/futurebuild/13_TASK_INDEX.md`
- Master plan (historical): `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot statement: `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning theory: `docs2/TheoryOfMachineLearning.md`
- Spec status/roadmap: `docs/ruvectorfindings/specs/_README.md` + `docs/ruvectorfindings/specs/implementation-plan.md`
- Operator contract: `CLAUDE.md §11`
- Reality-shift principle: `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`
- SMAP protocol: `docs2/SHARED-MIND-AGENT-PROTOCOL.md` + `docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`
```

## === END LOOP MD ===

---

## Notes on the diff vs current `loop.md`

- Drops the "Mode A / Mode B" framing entirely; replaces with priority-ordered work-class picking.
- Adds D-drive references throughout (shift log path, FSV path, RocksDB path).
- Adds `mejepa_record_agent_feedback` to the agent-facing tool list.
- Adds the prediction-injection note to `UserPromptSubmit` hook description.
- Adds daemon health check to the per-tick orientation.
- Drops references to Mode B drip / inner-actor / champion-attempt promotion.
- Updates §10 of the existing loop's "Self-healing properties" table to point at the scheduler that's about to be built.
- Keeps all invariants (reality-shift, GENERIC-ONLY, no-mocks, no-HTTP, no-inner-LLM, SMAP discipline, recommendation+report dual-write).
