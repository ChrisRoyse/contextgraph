# Security domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

Some security tasks are duplicated/cross-referenced across other domains (e.g., TASK-PREDICT-010 secret redaction).

## TASK-SEC-001 — Secret redaction module + integration

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-01 + TECH-SECURITY §1
- = TASK-PREDICT-010 (cross-listed).

## TASK-SEC-002 — Adversarial test corpus

- **Phase:** F · **Effort:** 1d · **Spec:** SPEC-SECURITY REQ-SEC-02 + TECH-SECURITY §1 + 22 §7
- = TASK-TEST-010 (cross-listed).

## TASK-SEC-003 — Multi-head contradiction detection

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mejepa/src/verdict_assembly.rs`; focused verdict-assembly tests passed during Phase B verification.

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-03 + TECH-SECURITY §3
- **Outputs:** `verdict_assembly.rs::assemble_verdict` extension.
- **DoD:** synthetic prediction with oracle Pass + failure_mode High → verdict = Abstain.

## TASK-SEC-004 — Cross-language label gating

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-04 + TECH-SECURITY §4
- = TASK-PREDICT-LABEL-011 (cross-listed).

## TASK-SEC-005 — Two-person rule for catastrophic promotion

- **Phase:** E · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-05 + TECH-SECURITY §5
- **Outputs:** `mejepa_promote_approval` extension; security config entry.
- **DoD:** with config enabled, second distinct approver required; same-approver-twice rejected.

## TASK-SEC-006 — Markdown render hardening

- **Phase:** A · **Effort:** 0.25d · **Spec:** SPEC-SECURITY REQ-SEC-06 + TECH-SECURITY §6
- **Outputs:** `scripts/cc/render_prediction_markdown.py` extension with `escape_dangerous`.
- **DoD:** dangerous patterns escaped; truncation at 4 KB preserved.

## TASK-SEC-007 — D-drive permissions verified at startup

- **Phase:** A · **Effort:** 0.25d · **Spec:** SPEC-SECURITY REQ-SEC-07 + TECH-SECURITY §7
- **Outputs:** `daemon_validate.rs::validate_d_drive_permissions`.
- **DoD:** world-readable D-root → daemon refuses with `MEJEPA_D_DRIVE_PERMS_TOO_OPEN`.

## TASK-SEC-008 — Hook script lint in CI

- **Phase:** A · **Effort:** 0.25d · **Spec:** SPEC-SECURITY REQ-SEC-10 + TECH-SECURITY §8
- **Outputs:** `ci/lints/hook_script_lint.sh`.
- **DoD:** shell-injection patterns in hooks → CI fails.
