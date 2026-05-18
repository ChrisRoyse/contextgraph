---
namespace: reports
agent_id: agent-codex
created: 2026-05-13T19:45:00Z
updated: 2026-05-13T19:45:00Z
status: active
tags: phase-g,task-g-019,oracle-flakiness,quarantine,fsv
---

# Session Report: 2026-05-13 — Phase G Oracle Flakiness Quarantine

## Task: TASK-G-019 oracle determinism / flakiness audit per cell
## Outcome: complete

## Summary
Implemented the Phase G oracle flakiness gate end to end. `context-graph-cli mejepa oracle-flakiness-audit` consumes strict repeated-oracle observations, computes outcome/hash flakiness with the 3-run default and >5% threshold, writes `config/corpus_quarantine.toml`, and readbacks the TOML bytes. `context-graph-cli mejepa train` now loads that quarantine fail-closed and skips quarantined task IDs before writing panel, DDA, live-prediction, oracle, and train-cert SoTs.

After reviewing the issue DoD, added the missing producer side: `mejepa-corpus verify --oracle-repeat-runs 3 --oracle-repeat-output <json>` reruns selected corpus oracle samples through the existing SWE-bench oracle path and emits the strict JSON schema accepted by the flakiness audit. The FSV harness was rerun at commit `afb48580` and records `all_passed=true`.

## Files Created or Modified in the Repo
- `config/corpus_quarantine.toml` — default empty quarantine config.
- `crates/context-graph-cli/src/commands/mejepa_oracle_flakiness.rs` — audit command, flakiness computation, TOML readback.
- `crates/context-graph-cli/src/commands/mejepa_train.rs` — fail-closed quarantine loading and train/holdout skip accounting.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs` — repeat-oracle observation export for verifier samples.
- `crates/context-graph-cli/examples/phase_g_oracle_flakiness_quarantine_fsv.rs` — TASK-G-019 FSV harness.
- `scripts/ci/phase_g_oracle_flakiness_quarantine_fsv.sh` — reproducible FSV runner.
- `docs/futurebuild/*` and `docs/futurebuild/19_OPERATOR_RUNBOOK.md` — Phase G ledgers and operator runbook updated.

## Files Written or Updated in ./memory/
- `reports/session-2026-05-13--phase-g-oracle-flakiness-quarantine.md` — this report.

## Decisions Made
- Split producer from classifier: `mejepa-corpus verify` produces real repeat-oracle observations, while `context-graph-cli mejepa oracle-flakiness-audit` classifies them and writes quarantine state. Rationale: reuses the existing SWE-bench oracle execution path and keeps quarantine classification deterministic and testable.

## Discoveries
- The first implementation satisfied quarantine consumption but not the issue's repeat-oracle producer requirement. Impact: added `--oracle-repeat-runs` / `--oracle-repeat-output` before resolving #115.
- Full `cargo test -p context-graph-cli` is still blocked by missing O*NET fixture `tmp/onet_probe/db_30_2_text.zip`; existing #81 tracks it and was updated with the fresh observation.

## Failures
- A GitHub milestone comment was initially posted with shell-interpreted Markdown backticks. Corrected immediately with a follow-up issue comment using `--body-file -`.

## Successes
- `cargo test -p context-graph-mejepa-corpus` passed: 117 lib + 6 bin + 4 integration tests.
- `cargo test -p context-graph-cli oracle_flakiness -- --nocapture` passed: 4 tests.
- TASK-G-019 FSV passed with reopened CF readback and `readback_equal=true`.

## Open Blockers
- #81 — full `context-graph-cli` test suite needs the missing O*NET ZIP fixture restored or the test fixture path corrected.

## Tests / Build
- Tests added: corpus repeat-output schema/validation tests; oracle flakiness audit unit tests; FSV harness boundary cases.
- Tests passing: yes for touched crates/scoped tests; full `context-graph-cli` remains blocked by #81.
- Build status: passing for `cargo build --release -p context-graph-mejepa-corpus` and `cargo build --release -p context-graph-cli`.
- Clippy: clean for `context-graph-mejepa-corpus`; clean for `context-graph-cli` after CLI implementation.

## FSV evidence
- `/mnt/d/contextgraph/fsv/phase-g-oracle-flakiness-quarantine-fsv/oracle_flakiness_quarantine_fsv.json` — sha256:f5dc3deaf91e274775a98e1b6aeb9a257222cd4af198ffa18c728d719825f739

## GitHub issues touched
- #115 — claimed, implemented, resolved, and closed.
- #81 — commented fresh full-suite blocker observation.

## Resume Here Next Session
- Start by reading issue #116 and the Phase G task index around `TASK-G-020`.
- Watch out for #81 when running the full `context-graph-cli` test suite; use focused tests plus documented blocker until the fixture issue is fixed.
- Priority: continue Phase G open issues in order after pre-work issue scan.
- Open question: none for TASK-G-019.

## Confidence: high
The implementation has unit tests, release builds, clippy, and FSV evidence with source-of-truth readback. The only unverified path is a full real-corpus three-run oracle sweep, which is operationally expensive and now has a concrete command in the runbook.
