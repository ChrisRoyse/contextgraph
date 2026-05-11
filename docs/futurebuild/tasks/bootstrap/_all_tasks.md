# Bootstrap domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

## TASK-BOOT-001 — `mejepa_bootstrap_status` MCP tool + bootstrap CLI

- **Phase:** A · **Effort:** 0.5d · **Spec:** 21_COLD_START_AND_BOOTSTRAP.md §7
- **Outputs:** new MCP tool; `crates/context-graph-cli/src/commands/mejepa_bootstrap.rs`.
- **DoD:** returns stage (warm / cold / bootstrapping) + counts; CLI wraps it.

## TASK-BOOT-002 — Baseline checkpoint build pipeline

- **Phase:** C · **Effort:** 1d · **Blocked by:** TASK-CORPUS-002 · **Spec:** 21 §3
- **Outputs:** `scripts/release/build_baseline_checkpoint.sh`; release-prep job.
- **DoD:** produces `models/mejepa-baseline-vN.safetensors` with verifiable holdout correlation ≥ 0.65.

## TASK-BOOT-003 — Widened-interval cold-start path

- **Phase:** A · **Effort:** 0.5d · **Spec:** 21 §4
- **Outputs:** `crates/context-graph-mejepa/src/conformal.rs` extension with cold-start widening.
- **DoD:** first 100 oracle outcomes → `confidence_interval = [0.0, 1.0]`; first 1000 → coverage_target = 0.99; > 1000 → standard 0.90.
