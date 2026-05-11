# API stability domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

## TASK-API-001 — Semantic version emission in MCP responses

- **Phase:** A · **Effort:** 0.25d · **Spec:** 24_API_STABILITY.md §6
- **Outputs:** `mcp_response.daemon_version` field on every tool response.
- **DoD:** clients can read daemon version; matches `cargo pkgid` output.

## TASK-API-002 — `context-graph-cli mejepa storage migrate-cf`

- **Phase:** F · **Effort:** 1d · **Spec:** 24 §4
- **Outputs:** `crates/context-graph-cli/src/commands/mejepa_storage_migrate.rs`.
- **DoD:** old CF entries transformed → new CF; round-trip validation; opt-in dry-run.

## TASK-API-003 — Replay tool

- **Phase:** F · **Effort:** 0.5d · **Spec:** 24 §5
- = TASK-OBS-011 (cross-listed).

## TASK-API-004 — Deprecation framework

- **Phase:** F · **Effort:** 0.5d · **Spec:** 24 §3
- **Outputs:** `crates/context-graph-mcp/src/deprecation.rs`; warning emission helpers.
- **DoD:** deprecated tool emits warning; opt-in error escalation works.

## TASK-API-005 — Changelog format + first entries

- **Phase:** A · **Effort:** 0.25d · **Spec:** 24 §3
- **Outputs:** `CHANGELOG.md` at repo root.
- **DoD:** entries for current and Phase A milestone.
