# SPEC-RUNBOOK — Functional Specification

> **Level 2.** Operator runbook + CLI + config files.
> Implementation reference: [`19_OPERATOR_RUNBOOK.md`](../../19_OPERATOR_RUNBOOK.md).

## User stories

- **As an operator**, I want one document that tells me what to do daily / weekly / monthly / on alert.
- **As an operator**, I want CLI commands matching every operational task.
- **As a new operator**, I want a quickstart that gets me from fresh clone to running daemon in 30 minutes.

## Requirements

### REQ-RUN-01 — First-time setup script
`scripts/setup/first_time_setup.sh` creates D-drive subdirs, places systemd unit, builds binaries, runs smoke test.

### REQ-RUN-02 — Daily operations CLI
`context-graph-cli mejepa daemon status` aggregates daemon health (subscriber, heal, quota, vram).

### REQ-RUN-03 — Weekly operations CLI
`context-graph-cli mejepa report weekly --date <date>` opens the latest weekly markdown report.

### REQ-RUN-04 — Active-learning operations CLI
`context-graph-cli mejepa active-learning {list,label,dismiss}` per `19 §9`.

### REQ-RUN-05 — Hygiene operations CLI
`context-graph-cli mejepa hygiene {cold-audit,aggressive-cold-tier-down}`.

### REQ-RUN-06 — Rollback CLI
`context-graph-cli mejepa rollback --version <id>` wraps `mejepa_rollback_to`.

### REQ-RUN-07 — Eval CLI
`context-graph-cli mejepa eval {run,ship-gate}`.

### REQ-RUN-08 — Daemon control CLI
`context-graph-cli mejepa daemon {start,stop,restart,status}` wraps systemd OR runs in-foreground if systemd absent.

### REQ-RUN-09 — Pause-predictions CLI
`context-graph-cli mejepa pause --duration-mins N` wraps `mejepa_pause_predictions` for incident response.

### REQ-RUN-10 — Configuration file convention
`~/.config/mejepa/{daemon.toml,backup.toml,security.toml}` — operator-editable; documented in runbook.

### REQ-RUN-11 — Quickstart documentation
A top-level `INSTALL.md` (or section in CLAUDE.md) walks the operator through §1 of the runbook.

### REQ-RUN-12 — Incident response checklists
Every alert class (`MEJEPA_*` codes) has a documented response checklist per `19 §5`.

## Acceptance criteria

1. Following `19 §1` from a fresh clone gets the daemon running in ≤ 30 minutes.
2. Each CLI command in §9 of the runbook exists and works.
3. Each alert class in §5 of the runbook resolves to an actionable command.

## Non-functional

- CLI commands p50 < 2s.
- Quickstart fits on one screen.
- Runbook is searchable by error code (every MEJEPA_* code is a heading or anchor).
