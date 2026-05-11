# Loop domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

LOOP-001/007 have detailed individual files. The rest:

**Status snapshot (2026-05-11):**
- LOOP-001: `open` — Apply new `.claude/loop.md` from `14_NEW_LOOP_MD.md`
- LOOP-002: `open` — CLAUDE.md / agents/README retirements (blocked by LOOP-001)
- LOOP-003: `open` — Mark superseded SMAP decisions (blocked by LOOP-001)
- LOOP-004: `open` — Retire legacy MCP tools
- LOOP-005: `open` — Audit absence of inner-actor model files (Phase H cleanup)
- LOOP-006: `open` — Delete stale `target/release/reality-loop` binary
- LOOP-007: `complete (2026-05-11)` — code-level migration already shipped (audit found hooks already resolve runtime root from D-drive SoT file); see TASK-LOOP-007 file for audit details


## TASK-LOOP-002 — Update CLAUDE.md + agents/README.md retirements

- **Phase:** A · **Effort:** 0.25d · **Blocked by:** TASK-LOOP-001 · **Spec:** TECH-LOOP §6-7
- **Goal:** Apply edits per `TECH-LOOP §6-7` to CLAUDE.md (§3, §11.11, §13) and `agents/README.md` (mark cc-reality-* agents retired).
- **Outputs:** edited CLAUDE.md, edited agents/README.md, edited 3 agent files with `status: retired` frontmatter.
- **DoD:** grep verifies expected lines exist in CLAUDE.md; agent files have status: retired.

## TASK-LOOP-003 — Mark superseded SMAP decisions

- **Phase:** H · **Effort:** 0.25d · **Blocked by:** TASK-LOOP-001 · **Spec:** TECH-LOOP §5
- **Goal:** For each decision in 11_DELETION_LIST_INNER_LOOP.md §4 supersede list, add `status: superseded` + `superseded_by:` pointer to corresponding new decision.
- **Outputs:** edits to ~7 SMAP decision files.
- **DoD:** every superseded decision has a valid `superseded_by` pointer; no duplicate active decisions on the same topic.

## TASK-LOOP-004 — Retire legacy MCP tools

- **Phase:** H · **Effort:** 0.25d · **Blocked by:** none
- **Goal:** Confirm `mcp__cgreality__reality_run_attempt` fail-closes with `CCREALITY_ENGINE_RETIRED`. Audit other tools for inner-actor concept references; mark retired.
- **Outputs:** verification + fail-closed paths.
- **DoD:** calling retired tool returns expected error.

## TASK-LOOP-005 — Audit absence of inner-actor model files

- **Phase:** H · **Effort:** 0.1d · **Blocked by:** none · **Spec:** TECH-LOOP §10
- **Goal:** `scripts/audit/no_inner_actor_models.sh` confirms no qwen/hermes/ollama files anywhere.
- **Outputs:** script (NEW); CI step invocation.
- **DoD:** script returns 0 violations on current tree; intentional violation triggers fail.

## TASK-LOOP-006 — Delete stale `target/release/reality-loop` binary

- **Phase:** H · **Effort:** 0.1d · **Blocked by:** none
- **Goal:** Audit Cargo.toml entries to ensure no `[[bin]] name = "reality-loop"`. Delete artifact if present.
- **Outputs:** verification + cleanup.
- **DoD:** `cargo metadata --format-version 1 | jq` does not list reality-loop bin.
