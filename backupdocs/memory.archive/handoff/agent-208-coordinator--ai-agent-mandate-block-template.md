---
namespace: handoff
agent_id: agent-208-coordinator
timestamp: 2026-05-10T00:00:00Z
status: active
tags: handoff, mandate-template, task-update, fsv-protocol
---

# Handoff: AI-Agent Mandate Block (apply to ALL 68 TASK files in 5 phase dirs)

## What

A uniform "🔍 AI-Agent Implementation Mandate (2026-05-10)" block that subagents will prepend to every TASK-*.md file in:

- `tasks/phase6-hygiene/` (20 files)
- `tasks/phase7-mcp-tools/` (7 files)
- `tasks/phase7-shift-subscriber/` (14 files)
- `tasks/phase8-evaluation/` (17 files)
- `tasks/utml-learning-formula/` (10 files)

Total: 68 files.

## The block (verbatim — apply BEFORE the existing task title or after the title, before `<task_spec>` XML)

```markdown
## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---
```

## Audit each task while applying — what to flag

For each TASK file, the subagent should also check:

- Does the crate the task says to "create" already exist? If yes, the task needs a "**2026-05-10 STATE AUDIT**" sub-block noting what's already shipped vs what remains.
- Are file paths in `<files_to_create>` still accurate?
- Are type signatures in `<signatures>` still aligned with what's in tree (if the type already exists)?

Specific pre-flagged drift (from the coordinator's sample reads):

- **TASK-UTML-001..006**: `crates/context-graph-mejepa-train/` ALREADY EXISTS with `learning_signal.rs`, `cert/`, `loss/`, `sampler/`, etc. The "create new crate" instruction is wrong. Tasks 003-006 (compute_delta_p/k/omega/xi) are likely SHIPPED — verify against `learning_signal.rs`.
- **TASK-MCP-001..006**: The MCP tools live in `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` (flat file), NOT in `tools/mejepa/` sub-module tree. 5 tools already live: `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`. Tasks 001-006 need rework to reflect the actual structure + flag which 5 tools are SHIPPED vs which 3 (`observe_shift`, `subscriber_status`, `capture_audit`) plus Phase 6 hygiene + Phase 8 eval tools are PENDING.
- **TASK-SHIFT-001..014**: `crates/context-graph-mejepa-shift-subscriber/` does NOT exist. Tasks accurate.
- **TASK-EVAL-001..017**: `crates/context-graph-mejepa/src/eval/` does NOT exist. Tasks accurate.
- **TASK-HYG-001..020**: `crates/context-graph-mejepa-hygiene/` does NOT exist. Tasks accurate.

## Cross-references

- Task 9 in TaskList tracks this work.
- `memory/discoveries/agent-{206,207}-inventory-*.md` for crate-state inventory.
- Updated docs at `docs/ruvectorfindings/specs/_README.md` Domain map + `implementation-plan.md` for current shipping status.
