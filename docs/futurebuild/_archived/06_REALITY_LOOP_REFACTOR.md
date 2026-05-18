# 06 — Reality Loop Refactor: Single Outer Loop Around Claude Code

> **Purpose.** Refactor the per-tick `/loop` semantics so there is exactly one loop — Claude Code-driven — whose job is to (a) ship futurebuild deliverables when build work is open, and (b) attempt SWE-bench Lite tasks when capture work is open, with no inner LLM and no Mode A/B split. This document specifies the new loop design; the literal text of the new `.claude/loop.md` is in [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md).
>
> **Operator directive (verbatim):** *"we can actually remove any reality loop stuff all together. the reality loop system is solely to get training data for the me jepa system. that is the whole purpose of the reality loop. and it only needs to have one loop, we can delete the inner loop entirely and everything having to do with it. we just need to optimize the outer loop, the one reality loop around claude code."*

---

## 1. The retired bipartite design

The current `.claude/loop.md` (commit `8a37ce7c`, 292 lines) operates in two modes:

- **Mode A (Build mode)** — ship one ME-JEPA primitive end-to-end. Default until Phase 4 verify() ships. (Phase 4 verify() did ship; the trigger never updated.)
- **Mode B (Reality-capture mode)** — pick the next unresolved task from `tasks/swebench-lite/*.json`, attempt with Claude Code, capture rich signals.

This was sensible during Phase 0–4 build-out. Post-pivot it has three problems:

1. **Mode B was supposed to gate on "Phase 4 verify() ships"** — that gate has been met (per `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`), so we should already be in capture mode, but the loop never explicitly switched because no operator forced the transition.
2. **Mode A's "ship one ME-JEPA primitive" framing is mostly exhausted.** Phase 0–8 are SHIPPED at code level. The remaining build work is operational integration (the futurebuild plan), not greenfield primitive shipping.
3. **The inner-actor LLM language permeates the document** — references to "GENERIC-ONLY" framing assume there's a non-Claude-Code agent doing the patching. There isn't anymore. Claude Code (the outer agent) does the patching directly.

**Decision: collapse to a single loop with two open work classes (build / capture), pick the highest-priority open class per turn, no mode switch ceremony.**

---

## 2. The new loop, one paragraph

Each `/loop` firing is one Claude Code turn. The turn reads SMAP context + the futurebuild docs + the existing specs/status, picks the highest-priority open work item, executes it end-to-end with FSV evidence, records a recommendation + a SMAP report, and ends. The work item is either a **build deliverable** (a futurebuild task such as wiring the agent-feedback MCP tool) or a **capture deliverable** (attempt the next unresolved SWE-bench Lite task to grow the corpus). No "modes," no "drips," no inner-LLM, no champion-attempt promotion. Everything Claude Code does becomes a panel update; observe() runs continuously in the daemon. The turn's only deliverables are the work + the audit artifacts.

---

## 3. Per-turn flow (replacement for current §1 + §5)

```
1. Read SMAP + specs + futurebuild
   - cat ./memory/context/{mission-brief,current-phase}.md
   - cat docs/futurebuild/{02_CURRENT_STATE_HONEST,12_IMPLEMENTATION_PHASES,13_TASK_INDEX}.md
   - cat docs/ruvectorfindings/specs/_README.md
   - cat ./memory/decisions/ ./memory/blockers/ (skip resolved) ./memory/handoff/
   - read latest ./memory/reports/

2. Pick the highest-priority open work item
   - First class: futurebuild tasks marked status=open in tasks/_index.md (build work)
   - Second class: next SWE-bench Lite task with no mejepa_capture_audit entry yet (capture work)
   - If both are empty: record no_change with surface_to_operator hint

3. Execute the work item end-to-end
   - For build work: code + tests + FSV evidence + spec/status update
   - For capture work: attempt the SWE-bench task; the daemon's continuous self-optimization
     consumes the resulting shifts/oracle outcomes automatically — no manual observe() call

4. Verify (FSV per CLAUDE.md §11.5.4 / loop.md §5.4 invariants)
   - Read the source-of-truth back; assert SHA matches expectation
   - Run 3 boundary cases per primitive
   - cargo build --release + cargo test + cargo clippy on touched crates

5. Record (dual-write)
   - mcp__cgreality__optimizer_record_recommendation (Stop hook enforces)
   - ./memory/reports/agent-{turn}-{role}--turn-{turn}-final-report.md
```

Compared to today's loop:

| Aspect | Today | New |
|---|---|---|
| Modes | A (build) / B (capture) explicit; switch gate broken | single loop, work-class-driven priority pick |
| Inner LLM | referenced; pivot acknowledged but Mode B implies a runner | absent; Claude Code is the only agent |
| Drip semantics | Mode B drips a fresh attempt via `mcp__cgreality__reality_run_attempt` | retired tool; agent works tasks directly with Edit/Write/Bash |
| observe() invocation | implicit | continuous daemon consumes shifts at 250ms tick |
| Champion / promotion | implicit Mode B promotion | retired; A/B/C promotion is owned by the heal scheduler, not the loop |
| Recommendation | required (Stop hook) | unchanged — required |
| SMAP report | required | unchanged — required |

---

## 4. Build work taxonomy (the futurebuild deliverables)

When the loop picks a build item, it's drawn from `13_TASK_INDEX.md` priority order. The futurebuild plan lays out the active phases in `12_IMPLEMENTATION_PHASES.md`: Phase A is the operational unblock (code-shipped; burn-in remains), Phase B is the multi-head extension (core code/FSV shipped; quality hardening remains), Phase C corpus completion is code/FSV-shipped, and the current forward work shifts to neural enrichment, autonomy, observability, rolling holdout, and cleanup.

Examples of build items the loop might pick:

- `TASK-FLYWHEEL-001-mcp-daemon-mode.md` — implement the supervised daemon that runs shift subscriber + heal scheduler + GC scheduler as one process
- `TASK-PREDICT-002-record-agent-feedback-tool.md` — add `mejepa_record_agent_feedback` MCP tool + CF + sampling-weight bump
- `TASK-PREDICT-001-prediction-injection-hook.md` — extend UserPromptSubmit hook to read `CF_MEJEPA_LIVE_PREDICTIONS` and render a markdown injection block
- `TASK-ENRICH-001-neural-forward-wrappers.md` — replace deterministic embedder placeholders with real neural forward
- Future corpus broadening — add scraped/public CI corpora beyond the completed Python SWE-bench and 10-language fixture seed
- `TASK-HEAL-001-continuous-self-optimization-daemon.md` — implement the scheduler tokio task

Each is atomic (per `prdtospec.md` Level 4), with explicit definition-of-done, signature-level scope, and dependency on prior tasks via the DAG in `13_TASK_INDEX.md`.

---

## 5. Capture work taxonomy (Claude Code attempts SWE-bench)

When the loop picks a capture item, it's the next SWE-bench Lite task with no `mejepa_capture_audit` entry yet (or, if all 300 have one entry, the task with the lowest current ME-JEPA correlation score on its category-language cell — i.e., the task ME-JEPA most needs more data on).

For the picked task, Claude Code:

1. **Reads the issue.** `gh issue view <task_id>` against the upstream repo; fall back to `cat tasks/swebench-lite/<task_id>.json` for the offline manifest.
2. **Sets up the workspace.** Clone the upstream repo at the task's base SHA; install dev deps (`pip install -e .[test]` for Python tasks, etc.).
3. **Investigates.** Read the failing test; trace the failure with the AST tools and `grep` / `Read`. **Every Read/Grep is captured automatically by the hooks** — these are the fine-grained "reading shifts" that distinguish exploration from action.
4. **Edits.** Each `Edit` / `Write` produces a shift entry; the daemon's shift subscriber materializes a panel within 250ms; ME-JEPA verify() runs; predictions land in `CF_MEJEPA_LIVE_PREDICTIONS`.
5. **On the next agent prompt** (within the same turn), the prediction is injected. If the agent disagrees with a high-severity prediction, it calls `mejepa_record_agent_feedback` — surprise event recorded.
6. **Runs tests.** `pytest <test_file>` (or language equivalent); E_Oracle picks up the outcome.
7. **Iterates** until either (a) tests pass + verdict is `Pass` at confidence ≥ ship gate, or (b) a per-attempt budget expires (default 1 hour wall + bounded round count).
8. **Records.** Each turn ends with `optimizer_record_recommendation` describing whether the attempt converged, plus a `mejepa_capture_audit` call binding the attempt to the corpus.

**The agent's iteration loop is the data-generating process.** Every Edit, every test run, every prediction-vs-outcome reconciliation becomes a training example. The daemon consumes these continuously, so by the time the same SWE-bench task comes around again, ME-JEPA's prediction on it should be measurably better.

---

## 6. Park rule (how stuck tasks roll forward)

Per `loop.md §1` today: "if a single task receives 3+ consecutive non-resolved attempts, park it and move to the next." Carried forward unchanged with one extension:

- **Parked tasks unblock when the active-learning queue surfaces them.** The continuous self-optimization daemon notices when ME-JEPA's wide-conformal-interval set includes a parked task; the active-learning queue suggests re-attempt; the loop picks up the parked task on the next available turn.
- **A task parked twice is added to a "needs operator" list** that surfaces in the weekly report; the operator may decide to mark it intractable or to commit specific guidance.

---

## 7. Invariants (carried from today's loop, restated)

These don't change:

1. **Reality-shift principle.** No alteration without current state; no alteration without delta back into perception.
2. **GENERIC-ONLY (in the corpus / instrument / head sense).** No per-task heuristics in the trained model. The agent (Claude Code) may reason task-specifically — but ME-JEPA's predictor must remain task-generic; per-task tuning happens via fine-tuned LoRA adapters that are scoped to retrievable activation, not by hardcoding.
3. **No HTTP from runtime.** MCP is the only network surface.
4. **No mocks in tests.** Real synthetic data only; real Docker oracle; real RocksDB; real tree-sitter.
5. **Single-writer for the harness.** Edits to source code go through `Edit` / `Write` (Claude Code's standard tools); no concurrent harness-editor.
6. **Pair every recommendation JSON with a SMAP report.** Two views of the same reality.
7. **Resolved blockers stay** (status: resolved); superseded decisions stay (status: superseded).
8. **No inner LLM.** Per the pivot. Claude Code is the only agent.

---

## 8. What the loop NO LONGER does

These are explicit retirements:

| Behavior | Status | Why |
|---|---|---|
| Drip a fresh `mcp__cgreality__reality_run_attempt` | RETIRED | engine binary deleted; tool fails closed with `CCREALITY_ENGINE_RETIRED` |
| Spawn `cc-reality-harness-editor` for harness source edits | RETIRED | the "harness" was the reality-loop runner; it's retired. Claude Code's standard `Edit`/`Write` are the only writers. |
| Diagnose via `cc-reality-diagnostician` for inner-actor failures | RETIRED | no inner actor to diagnose |
| Switch between Mode A and Mode B based on gate | RETIRED | one loop, work-class priority pick |
| Per-attempt wallclock budget for the inner actor | RETIRED | doesn't apply; outer-loop budget is bounded by Claude Code session length |
| Champion / promotion of "best inner attempt so far" | RETIRED | A/B/C promotion is for ME-JEPA model variants, owned by heal scheduler, not the loop |
| Tool-result history cap (4 KB) | KEPT | applies to UserPromptSubmit injection size; still relevant |

---

## 9. What the loop newly DOES

| Behavior | Notes |
|---|---|
| Read futurebuild docs at orientation | adds `cat docs/futurebuild/{02,12,13}.md` to the orientation step |
| Pick from futurebuild task graph | priority: build first, then capture |
| Trust the daemon for observe() / drift / GC | the loop does not call `mejepa_observe_shift` manually; the daemon does, continuously |
| Surface daemon health on every turn | one-line check via `mejepa_subscriber_status` MCP tool; if degraded, escalate |
| Render structured prediction injection | UserPromptSubmit hook reads `CF_MEJEPA_LIVE_PREDICTIONS` and renders the Q1–Q5 prediction block (per `15_PREDICTION_TAXONOMY.md §8`) |
| Record agent-feedback on surprises | when the agent disagrees with a prediction, `mejepa_record_agent_feedback` writes a `SurpriseEvent`; loop's recommendation references it |

---

## 10. Operator skills (mostly unchanged, with replacements)

| Skill | Status | Notes |
|---|---|---|
| `/ccreality-iterate` | superseded by `/loop` | one canonical entrypoint |
| `/ccreality-smoke` | KEPT | verifies daemon mode + shift subscriber + GC scheduler boot |
| `/ccreality-trigger-check` | retired | the "trigger" was for the inner-actor drip; gone |
| `/ccreality-readback` | KEPT | still useful for drift detection on past recommendations |
| `/loop` | refactored per [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md) | the canonical per-tick skill |

---

## 11. Migration plan for `.claude/loop.md`

Per [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md), the literal new content for `.claude/loop.md`:

1. Read [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md) (the new content).
2. Apply via `Write /home/cabdru/contextgraph/.claude/loop.md` with that content.
3. Verify via `cat .claude/loop.md | head -50` — first 50 lines should match.
4. Update `CLAUDE.md §11.11` (operator-driven skills) to mark `/ccreality-trigger-check` as retired and `/loop` as canonical.
5. Update `agents/README.md` to drop the inner-actor agent references (cc-reality-harness-editor, cc-reality-diagnostician become read-only legacy markers).
6. Run a smoke `/loop` turn to confirm nothing breaks.

The migration is a single commit with the loop.md replacement + the small CLAUDE.md / agents README updates. No code changes required.

---

## 12. What this refactor commits us to

1. Apply the new `.claude/loop.md` content from [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md). (Task: `TASK-LOOP-001-apply-new-loop-md.md`.)
2. Update `CLAUDE.md §11.11` and `agents/README.md`. (Task: `TASK-LOOP-002-update-claude-md-and-agents-readme.md`.)
3. Stand up the daemon mode (so the loop can trust continuous observe()). (Task: `TASK-FLYWHEEL-001-mcp-daemon-mode.md` — same task that delivers the flywheel.)
4. Retire references to inner-actor concepts in `memory/context/` files via natural inheritance — old SMAP files keep `status: superseded`; new files reflect post-pivot reality. (Task: `TASK-LOOP-003-supersede-inner-actor-context.md`.)
5. Delete (or mark retired) the `cc-reality-trigger-check` and `cc-reality-iterate` skills if they reference inner-actor drip. (Task: `TASK-LOOP-004-retire-legacy-skills.md`.)
