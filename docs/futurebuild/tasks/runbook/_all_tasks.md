# Runbook domain — all atomic tasks

Most CLI / runbook items are small wrappers over existing MCP tools or shell scripts.

## TASK-RUN-001 — First-time setup script

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-RUNBOOK REQ-RUN-01 + TECH-RUNBOOK §3
- **Outputs:** `scripts/setup/first_time_setup.sh`, `INSTALL.md` (top-level).
- **DoD:** fresh-clone install completes daemon-running in ≤ 30 min on a reference machine.

## TASK-RUN-002..009 — Operator CLI commands

- **Phase:** F · **Effort:** 0.5d each (8 cmds × 0.5d ≈ 4d total) · **Spec:** SPEC-RUNBOOK REQ-RUN-02..09
- Each adds one `context-graph-cli mejepa <subcommand>` per `TECH-RUNBOOK §1`:
  - `daemon {start,stop,restart,status}` — RUN-002
  - `report weekly --date <date>` — RUN-003
  - `active-learning {list,label,dismiss}` — RUN-004 (= TASK-OBS-010)
  - `hygiene {cold-audit,aggressive-cold-tier-down}` — RUN-005
  - `rollback --version <id>` — RUN-006
  - `eval {run,ship-gate}` — RUN-007
  - `pause --duration-mins <n>` — RUN-008 (+ `mejepa_pause_predictions` MCP handler)
  - `session cleanup --session-id <id>` — RUN-009 (= TASK-MAI-003)
- Outputs: per-subcommand module under `crates/context-graph-cli/src/commands/`.
- DoD: each subcommand wraps the appropriate MCP tool / shell op; round-trip tested.

## TASK-RUN-010 — Configuration file convention + parser

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-RUNBOOK REQ-RUN-10 + TECH-RUNBOOK §6
- **Outputs:** TOML parser for `~/.config/mejepa/{daemon,backup,security}.toml`; documented defaults.
- **DoD:** all 3 config files load + validate; daemon respects values.

## TASK-RUN-011 — Quickstart `INSTALL.md`

- **Phase:** A · **Effort:** 0.25d · **Spec:** SPEC-RUNBOOK REQ-RUN-11 + TECH-RUNBOOK §7
- **Outputs:** `INSTALL.md` at repo root.
- **DoD:** fits on one screen; new operator can follow it from cold start.

## TASK-RUN-012 — Incident response checklist verification

- **Phase:** F · **Effort:** 0.25d · **Spec:** SPEC-RUNBOOK REQ-RUN-12
- **Goal:** Verify every `MEJEPA_*` alert class in `19 §5` has documented response. CI lint script grep-asserts every emitted error code has a runbook entry.
- **Outputs:** `ci/lints/runbook_coverage.sh`.
- **DoD:** lint passes; new error codes added to runbook before merge.
