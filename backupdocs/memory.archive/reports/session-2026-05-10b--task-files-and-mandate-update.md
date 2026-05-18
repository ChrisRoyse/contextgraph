---
namespace: reports
agent_id: agent-208-coordinator
timestamp: 2026-05-10T01:00:00Z
status: active
tags: smap-report, task-update, ai-agent-mandate, fsv-protocol, code-bug-discovered
---

# Final Report: TASK files + AI-Agent Mandate update — 2026-05-10 (turn 2)

## Task Completed: yes

## Summary

Operator follow-up to the 2026-05-10 doc refresh: update all TASK-*.md files in 5 active forward-edge phase directories (phase6-hygiene, phase7-mcp-tools, phase7-shift-subscriber, phase8-evaluation, utml-learning-formula) so each becomes self-contained for a fresh-context AI agent. The user's directives required: project-state pointers, FSV protocol (Source of Truth + Execute & Inspect + Boundary & Edge Case Audit + Evidence of Success), trigger→outcome verification, fail-fast / no-mock / no-backwards-compat reminders, "ASSUME THIS TASK IS WRONG" audit mandate, manual database verification, synthetic input/output testing, Occam's razor framing.

Designed a single uniform "🔍 AI-Agent Implementation Mandate" block (~700 tokens, 7 sections covering all mandates) and dispatched 5 parallel subagents — one per directory — to prepend the block to every TASK file AND audit each task body against the actual codebase. Subagents reported per-file pre/post SHAs + drift findings.

**68 of 68 TASK files updated** (20 + 7 + 14 + 17 + 10).

## Files Created or Modified

### Documentation updates (68 TASK files + 4 SMAP files)

- All 20 `tasks/phase6-hygiene/TASK-HYG-001..020.md` (16 clean / 4 flagged for CF-const collision with `heal/cf.rs`)
- All 7 `tasks/phase7-mcp-tools/TASK-MCP-001..007.md` (all 7 flagged — module structure drift: tasks assume `tools/mejepa/` sub-tree but reality is flat `mejepa_tools.rs`; 8 of 18 planned tools already shipped)
- All 14 `tasks/phase7-shift-subscriber/TASK-SHIFT-001..014.md` (clean — crate doesn't exist yet, tasks accurate; cross-references to flat-file MCP tool location flagged)
- All 17 `tasks/phase8-evaluation/TASK-EVAL-001..017.md` (clean — eval/ module doesn't exist; type-namespace collision risk flagged for canonical `MutationCategory` location)
- All 10 `tasks/utml-learning-formula/TASK-UTML-001..010.md` (TASK-UTML-001 + 007 SUPERSEDED — crate + functions already shipped; 002-006/008 partially superseded with semantic drift; 009 fully actionable)

### Memory writes

- `handoff/agent-208-coordinator--ai-agent-mandate-block-template.md` — verbatim mandate block template + audit instructions for the 5 subagents
- `discoveries/agent-209-task-updates-phase6-hygiene.md` — agent-209's per-file audit table
- `discoveries/agent-210-task-updates-phase7-mcp-tools.md` — agent-210's per-file audit table
- `discoveries/agent-211-task-updates-phase7-shift-subscriber.md` — agent-211's per-file audit table
- `discoveries/agent-212-task-updates-phase8-evaluation.md` — agent-212's per-file audit table
- `discoveries/agent-213-task-updates-utml-learning-formula.md` — agent-213's per-file audit table with critical UTML safety finding
- `blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md` — high-severity blocker on the safety-critical UTML invariant
- `reports/session-2026-05-10b--task-files-and-mandate-update.md` — this file

## The mandate block (7 sections)

1. **Project state pointers** — direct fresh-context agent to specs/_README.md, implementation-plan.md, constitution.md, this phase's _index.md, `git log -30`, docs 09-14, CLAUDE.md
2. **ASSUME THIS TASK IS WRONG** — audit instructions: verify file paths exist, type signatures still match, crate not already created
3. **Hard rules ZERO tolerance** — no backwards compat, no mocks, fail-closed structured errors, robust tracing, no comments unless WHY, files <500 lines, provenance comments
4. **Mandatory FSV protocol** — Source of Truth + Execute & Inspect + Boundary & Edge Case Audit (3 cases minimum) + Evidence of Success
5. **Trigger → outcome verification** — manual database/artifact check is REQUIRED; synthetic input → known expected output → manual readback
6. **Occam's razor** — simplest implementation, no premature abstraction
7. **Subagent / verification expectations** — cargo build/test/clippy, code-simplifier review, SMAP report, traceability update

## Decisions Made

- **One uniform mandate block** across all 68 files. Rationale: Occam's razor. Per-task customization would have been 68× the work and would have produced inconsistent mandates.
- **Audit findings go into `## 📋 2026-05-10 STATE AUDIT` H2 sub-sections** (per-file). Subagents customize these per file based on actual codebase audit.
- **Skipped 236 unchanged TASK files** in phases 0/1/2/3/4/5. Those phases are SHIPPED or don't need refresh attention; the 5 active forward-edge phases are where the implementer focus belongs.
- **Did not edit code despite finding a code bug.** The user directive was a doc/task-spec update. Code bug surfaced and recorded in `memory/blockers/` for the next implementation turn.

## Discoveries

### Critical safety bug in shipped code (high-severity blocker)

`compute_delta_xi` at `crates/context-graph-mejepa-train/src/learning_signal.rs:184` silently averages `target_collapse` with the other ΔΞ sub-terms instead of failing closed when `target_collapse != 0.0`. Per UTML v1.0 §VI.2 + constitution `<utml_v1_invariants>` rule 2, ME-JEPA's central architectural innovation — structural collapse-immunity — depends on this being a hard runtime invariant emitting `MEJEPA_INSTRUMENT_GRADIENT_LEAK` when violated. Currently the invariant is **unenforced at runtime**. Recorded as blocker; remediation plan included.

### Module-structure drift on Phase 7 MCP tools

The original TASK-MCP-001..006 specs assumed a sub-module tree `crates/context-graph-mcp/src/handlers/tools/mejepa/{mod,verify,predict_latest,...}.rs`. Actual implementation went with a flat file `mejepa_tools.rs`. 5 of the 8 shipped MEJEPA MCP handlers (verify, predict_latest, constellation_inspect, heal_status, rollback_to) live in `mejepa_tools.rs`; the other 3 (load_learner_state, routing_lookup, vram_budget_report) live in `embedder_tools.rs`. Tasks need rework before they can be implemented as written.

### UTML compute_delta_* semantic drift

All four `compute_delta_p/k/omega/xi` functions in shipped `learning_signal.rs` use arithmetic mean over their components, but doc 11 specifies weighted-sum (`0.5·cos_align + 0.3·(1-fisher) + 0.2·(1-ECE)` for ΔK) and geometric/power forms (`(eff_plast)^0.4 · (landscape)^0.4 · (stability)^0.2` for ΔΩ). Tasks UTML-003..006 carry these formulas in their `<scope>` blocks; current implementation does not match. Subagent-213 flagged each in their respective STATE AUDIT sub-sections.

### Phase 6 hygiene CF-const collision

`crates/context-graph-mejepa/src/heal/cf.rs` already declares 10 `CF_MEJEPA_*` consts (FISHER_SNAPSHOTS, DRIFT_WINDOW, DRIFT_HISTORY, HEAL_REPORTS, DISTILL_STEPS, ACTIVE_POINTERS, CALIBRATION_HISTORY, WEIGHT_BLOBS, SHIFT_WATERMARK, TRAIN_CERTS). 4 Phase 6 hygiene tasks attempt to redeclare some of these as `pub const`. Recommendation: extract heal/cf.rs CF consts into a shared crate (e.g., `context-graph-mejepa-cf`) before TASK-HYG-001 ships.

### TASK-UTML-001 + TASK-UTML-007 fully superseded

The crate `crates/context-graph-mejepa-train/` already exists with `learning_signal.rs` (containing `compute_delta_p/k/omega/xi/l_step`), `cert/` module, `loss/`, `sampler/`, `optim/`, `trainer/`. TASK-UTML-001's "create new crate" instruction is obsolete; TASK-UTML-007's `compute_l_step` instruction is shipped (with the semantic-drift caveat above).

## Failures

- None. All 5 subagents reported success on all 68 files. Spot-checks of 5 files (one per phase) confirmed mandate block correctly placed.

## Successes

- 68/68 task files updated in ~25 minutes wall-clock via 5 parallel subagents.
- Cross-cutting code-state findings captured in 5 discovery files + 1 high-severity blocker.
- 0 code changes; documentation-only refresh as requested.
- Per-task STATE AUDIT sub-sections customized per-file based on actual codebase grep/ls/inventory results — not generic boilerplate.
- Subagents successfully Read each file before Edit (per Edit-tool protocol).

## Tests / Build

- No code changes. No build / test runs.
- All edits verified via PreToolUse / PostToolUse Edit hook SHA progression.

## For the Next Agent

- **Highest-priority code work**: address `memory/blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md`. This is a safety-critical invariant currently unenforced.
- **Highest-priority task-spec rework**: TASK-MCP-001..006 need restructuring to match the shipped flat-file `mejepa_tools.rs` (and `embedder_tools.rs` for 3 tools). Read agent-210's discovery file for the full reconciliation map.
- **Phase 6 hygiene blocker**: decide canonical CF-const home before TASK-HYG-001. Recommendation: extract `heal/cf.rs` consts into shared crate.
- **UTML semantic drift**: `compute_delta_p/k/omega/xi` need re-implementation per doc 11 §2-§5 weighted/geometric formulas, not arithmetic mean. TASK-UTML-003..006 carry the canonical formulas in their `<scope>` / `<pseudo_code>` blocks.
- **Safe to start fresh from any TASK file**: each TASK now carries its own AI-Agent Mandate block. A fresh-context agent reading TASK-EVAL-005 (for example) gets the project-state pointers, FSV protocol, fail-fast rules, "ASSUME THIS TASK IS WRONG" audit mandate, and Occam's razor framing inline.

## Confidence: high

68/68 file updates verified via spot-check (5 files inspected post-write; mandate block correctly placed in each). Code-bug claim by agent-213 personally verified by reading the actual function body at `learning_signal.rs:184`. The mandate block design was personally authored (not delegated) and captures all 8 user mandates from the operator prompt: FSV protocol with 4 mandatory sections, source-of-truth verification, edge case audit (≥3 cases), trigger→outcome chain, fail-fast / no-mock / no-backwards-compat, manual database verification, synthetic input/output testing, Occam's razor.
