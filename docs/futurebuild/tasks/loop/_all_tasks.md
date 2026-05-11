# Loop domain — all atomic tasks

LOOP-001/007 have detailed individual files. The rest:

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
