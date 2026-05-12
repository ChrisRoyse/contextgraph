# 21 — Loop Contract (Single Outer Loop Around Claude Code)

Source files covered: `.claude/loop.md` (current), `docs/futurebuild/14_NEW_LOOP_MD.md`, `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md`, `CLAUDE.md §11`.

> **Per-tick contract**. Each `/loop` firing is one Claude Code turn. The turn (a) reads SMAP + futurebuild + spec status, (b) picks the highest-priority open work item (build OR capture), (c) executes end-to-end with FSV evidence, (d) records `optimizer_record_recommendation` + a SMAP `./memory/reports/...md` report, (e) ends.

---

## 1. Two work classes (no mode ceremony)

### 1.1 Build (futurebuild deliverable)

Ship one item from `docs/futurebuild/13_TASK_INDEX.md` — typically lowest-numbered `status: open` task in the active phase per `12_IMPLEMENTATION_PHASES.md`. Each task file under `docs/futurebuild/tasks/` is atomic per `prdtospec.md` Level 4.

Deliverables per turn:
- Code + tests + FSV evidence
- Docs / spec / task status update if behavior or roadmap state changes
- `cargo build --release -p <crate>` + `cargo test -p <crate>` + `cargo clippy -p <crate>`
- No mocks; real synthetic inputs only; fail-closed errors

### 1.2 Capture (attempt a SWE-bench Lite task)

Pick the next unresolved task from `tasks/swebench-lite/*.json` — lexicographic order; first task with no `mejepa_capture_audit` entry. **OR** when all 300 have audit, the task with lowest current ME-JEPA correlation on its (category × language) cell.

Attempt directly with Claude Code's Edit/Write/Bash — no inner LLM.

Each Edit → shift-log entry → daemon shift subscriber materializes Panel within 250ms → ME-JEPA `verify()` runs → `RealityPrediction` in `CF_MEJEPA_LIVE_PREDICTIONS` → next UserPromptSubmit hook injects markdown.

**Park rule**: 3+ consecutive non-resolved attempts → park; pick next. Parked tasks unblock when active-learning queue surfaces them.

---

## 2. Priority order

1. **Daemon health degraded** (subscriber lag > 500/min, scheduler flapping, drift catastrophic, witness corruption, quota near hard cap) → that takes precedence; surface as `blocked` if not fixable this turn.
2. **Phase-A futurebuild tasks** (Operational Unblock) until Phase A done.
3. **Phase-B/C/D/E/F futurebuild tasks** in order.
4. **Phase-G capture work** (SWE-bench attempts) in parallel with B–F when no urgent build item.
5. **Phase-H cleanup tasks** as opportunistic fillers.

If everything `status: done` and no capture work → `no_change + surface_to_operator: "phases A-H complete; choose scaling/domain target"`.

---

## 3. Per-tick orientation (the FIRST four actions)

```
1. cat ./memory/context/current-phase.md ./memory/context/mission-brief.md
2. cat docs/futurebuild/02_CURRENT_STATE_HONEST.md docs/futurebuild/12_IMPLEMENTATION_PHASES.md docs/futurebuild/13_TASK_INDEX.md
   cat docs/ruvectorfindings/specs/_README.md
3. cat ./memory/decisions/ ./memory/blockers/ (skip status:resolved) ./memory/handoff/
   read latest ./memory/reports/
4. mcp__cgreality__mejepa_subscriber_status + mejepa_heal_status + mejepa_quota_status
```

If a daemon health check fails → that may take precedence over the highest-priority task.

---

## 4. The 5-step iteration procedure

| Step | Action |
|---|---|
| 5.1 Orient | SMAP + futurebuild docs + spec status; daemon health |
| 5.2 Plan | Short internal plan; what ships? what's FSV? |
| 5.3 Execute | Implement task; OR attempt SWE-bench task. Subagents for parallel research; do file-reading + synthesis personally |
| 5.4 Verify | FSV per `docs/futurebuild/specs/FSV-PROTOCOL.md`: SoT readback + 3 boundary cases + cargo build/test/clippy |
| 5.5 Record | Dual-write: `optimizer_record_recommendation` + `./memory/reports/agent-{turn}-{role}--turn-{turn}-final-report.md` |

---

## 5. Core invariants (8)

| # | Invariant |
|---|---|
| 1 | **Reality-shift principle**: no alteration without current state; no alteration without delta back into perception. Detect external drift with `mcp__cgreality__reality_shift_compare_to_my_view`. |
| 2 | **GENERIC-ONLY** (ME-JEPA training): no per-task overfit; primitives must generalize across every SWE-bench task and language. You can reason task-specifically when attempting; never bake task-specifics into the model. |
| 3 | **No mocks**: real synthetic data; real Docker; real RocksDB; real tree-sitter; fail-closed errors with explicit codes. |
| 4 | **No HTTP from runtime**: MCP is the only network surface. `WebSearch` / `WebFetch` are research-only (read-only). |
| 5 | **No inner LLM** (post-2026-05-09): no `llama-cpp-2`, Qwen, Hermes, Ollama. Claude Code is the only agent. |
| 6 | **D-drive only** for bulk state (`docs/futurebuild/16`); C-drive only for code/docs/configs; `/tmp/` only for ephemeral test scratch. |
| 7 | **Self-healing / self-learning**: every primitive shipped preserves or strengthens daemon's continuous self-optimization. Reject changes that make ME-JEPA less updateable. |
| 8 | **Multi-head prediction always emits everything**: every `RealityPrediction` includes all Q1–Q5 surfaces; empty vecs populated. |

---

## 6. Output contract (what THIS turn MUST produce)

| Required output | Location |
|---|---|
| ✅ ONE `optimizer_record_recommendation` call | Stop hook enforces (status: `changed` / `no_change` / `blocked`) |
| ✅ ONE SMAP report | `./memory/reports/agent-{turn}-{role}--turn-{turn}-final-report.md` |
| ✅ Zero or more SMAP artifacts (write IMMEDIATELY on trigger) | `./memory/discoveries/`, `decisions/`, `blockers/`, `patterns/`, `handoff/` |
| ✅ Zero or more shift-log entries (auto-minted by hooks) | `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` |
| ✅ (if build) code + tests + FSV evidence + spec/task update | crate source + `/mnt/d/contextgraph/fsv/<crate>-fsv/` |
| ✅ (if capture) `mejepa_capture_audit` MCP call + attempt summary | `tasks/swebench-lite/<task_id>` workspace |

Timestamps: `date -u +%FT%TZ` (ISO 8601 UTC).

---

## 7. SMAP discipline (`./memory/`)

8 namespaces:

| Namespace | Purpose |
|---|---|
| `context/` | mission, current phase, constraints — read first every turn |
| `decisions/` | locked architectural choices (do not contradict) |
| `discoveries/` | constraints, gotchas, edge cases learned during work |
| `handoff/` | concrete deliverables passed turn-to-turn |
| `journal/` | append-only chronological execution log |
| `blockers/` | unresolved issues with severity (resolved = update status, never delete) |
| `patterns/` | reusable conventions and standards |
| `reports/` | each turn's final report |

Every `.md` file requires YAML frontmatter:

```yaml
---
namespace: {context|decisions|discoveries|handoff|journal|blockers|patterns|reports}
agent_id: agent-{N}-{role}
timestamp: 2026-05-11T22:05:00Z
status: {active|resolved|superseded}
tags: comma-separated
---
```

Lifecycle: READ → ORIENT → WORK → WRITE → REPORT.

Per CLAUDE.md §6:
- **Resolved blockers stay** (`status: resolved`).
- **Superseded decisions stay** (`status: superseded`); link via `superseded_by:`.
- **Reports are append-only**; never delete.

---

## 8. Hard rules (the 10 most important)

1. Do what the operator asked, nothing more, nothing less.
2. Read `./memory/context/`, `./memory/decisions/`, futurebuild docs (02, 12, 13), specs `_README.md` at start of every turn.
3. Pick exactly one work item per priority order.
4. GENERIC-ONLY in the trained model; never bake task-specifics in.
5. No HTTP. No destructive git. No editing the deny set in `CLAUDE.md §8`.
6. No mocks; real data; fail-closed errors; FSV under `/mnt/d/contextgraph/fsv/`.
7. D-drive bulk only; C-drive code/docs/configs only; `/tmp/` ephemeral only.
8. Pair every recommendation JSON with a SMAP report.
9. Resolved blockers stay; superseded decisions stay; reports append-only.
10. Never silently violate a locked decision. If overriding, supersede explicitly.

---

## 9. Self-healing properties the loop preserves

Per `docs/futurebuild/14_NEW_LOOP_MD.md §7`:

| Property | Mechanism |
|---|---|
| Self-healing under drift | scheduler drift_tick + LoRA refresh + A/B/C |
| Self-learning from new shifts | scheduler observe_tick @ 250ms |
| Self-validating outputs | conformal interval + Gτ guard + `Verdict::GuardRejected` |
| Self-bounding storage | nightly GC + tier compression + 115 GB hard quota |
| Self-auditing provenance | witness chain + ed25519 + training certs |
| Self-correcting via active-learning | scheduler active_learning_tick consumes high-priority queue |
| Self-optimizing per-prediction | agent-feedback bumps sampling_weight |

Every commit message should mention which property it strengthens. Weakening any without an explicit pivoted-design note → recommendation must surface in `remaining_risks`.

---

## 10. Operator-driven skills

| Skill | Status |
|---|---|
| `/loop` | canonical per-tick skill (replaces all ccreality skills) |
| `/ccreality-smoke` | KEPT — verifies daemon + subscriber + GC boot |
| `/ccreality-readback` | KEPT — drift detection on past recommendations |
| `/ccreality-iterate` | RETIRED — superseded by `/loop` |
| `/ccreality-trigger-check` | RETIRED — inner-actor drip gone |

---

## 11. What the loop does NOT do anymore

| Behavior | Status |
|---|---|
| Drip `reality_run_attempt` | RETIRED (`CCREALITY_ENGINE_RETIRED`) |
| Spawn `cc-reality-harness-editor` | RETIRED |
| Diagnose via `cc-reality-diagnostician` | RETIRED |
| Mode A / Mode B switch | RETIRED — single loop, priority pick |
| Per-attempt wallclock budget for inner actor | RETIRED (inner actor gone) |
| Champion-attempt promotion | RETIRED (A/B/C now owned by heal scheduler) |

---

## 12. When `/loop` runs unattended

Defaults:
- Pick highest-priority Phase-A futurebuild task.
- If Phase A done, pick from Phase B in parallel with capture.
- If all build tasks complete, pick capture work.
- If everything `status: done` and no capture work → `no_change + surface_to_operator: "phases A-H complete; choose scaling/domain target"`.
- Never assume operator watching. Errors produce robust logs + clean blocker writes.

---

## 13. Bootstrap (fresh clone, first turn)

If `./memory/` does not exist:
```bash
mkdir -p ./memory/{context,decisions,discoveries,patterns,blockers,journal,reports,handoff}
```

If `/mnt/d/contextgraph/` missing or not writable: refuse with `MEJEPA_D_DRIVE_UNAVAILABLE`. Daemon will not start.

Write initial `./memory/context/mission-brief.md` + `current-phase.md` reflecting post-pivot state. Re-read `CLAUDE.md` + `docs/futurebuild/00_INDEX.md`. Recommendation = `changed` if SMAP files written; `no_change` if already in place.

---

## 14. Cross-references

[`CLAUDE.md §11`](../../CLAUDE.md), [`docs/futurebuild/14_NEW_LOOP_MD.md`](../futurebuild/14_NEW_LOOP_MD.md), [`22_operator_runbook.md`](22_operator_runbook.md), [`20_hooks_layer.md`](20_hooks_layer.md).
