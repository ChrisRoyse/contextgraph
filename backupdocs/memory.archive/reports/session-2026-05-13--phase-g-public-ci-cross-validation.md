---
namespace: reports
agent_id: agent-codex
created: 2026-05-13T16:42:08Z
updated: 2026-05-13T16:48:00Z
status: active
tags: phase-g,task-g-012,public-ci,ghalogs,fsv
---

# Session Report: 2026-05-13 — Phase G Public-CI Cross-Validation

## Task: Complete TASK-G-012 / #108 public CI cross-validation
## Outcome: complete

## Summary
Implemented `context-graph-cli mejepa cross-validate-public-ci` for Phase G. The command validates that the input public corpus identifies GHALogs (`10.5281/zenodo.10154920`, Zenodo record, and D2KLab repository), requires at least 500 selected examples, computes per-cell public-CI prediction/oracle correlations, compares them against a Lite baseline, and ingests the same rows through `mejepa train`.

FSV materializes a deterministic 560-row GHALogs-shaped holdout subset, compares all 80 cells, writes a report with fsync/readback, and reopens RocksDB to prove 560 rows landed in each SoT CF: panels, DDA signals, live predictions, oracle verdicts, and train certs.

## Files Created or Modified in the Repo
- `crates/context-graph-cli/src/commands/mejepa_public_ci_cross_validate.rs` — public-CI cross-validation command.
- `crates/context-graph-cli/src/commands/mejepa_public_ci_cross_validate/types.rs` — DTOs and persisted JSON schemas.
- `crates/context-graph-cli/src/commands/mejepa_public_ci_cross_validate/tests.rs` — focused correlation and validation tests.
- `crates/context-graph-cli/examples/phase_g_public_ci_cross_validation_fsv.rs` — FSV harness.
- `crates/context-graph-cli/src/commands/mejepa_train.rs` — optional `predicted_oracle_pass` support for corpus rows.
- `crates/context-graph-cli/src/commands/mejepa_active_learning.rs` and `mod.rs` — CLI registration.
- `docs/futurebuild/02_CURRENT_STATE_HONEST.md`, `12_IMPLEMENTATION_PHASES.md`, `13_TASK_INDEX.md` — TASK-G-012 status.

## Files Written or Updated in ./memory/
- `reports/session-2026-05-13--phase-g-public-ci-cross-validation.md` — this report.

## Decisions Made
- **Approach chosen**: add a narrow `mejepa cross-validate-public-ci` wrapper over the existing `mejepa train` SoT writer.
- **Considered alternatives**: a new ingestion pipeline was rejected because it would duplicate the Phase G writer; a generic arbitrary-corpus importer was rejected because #108 required a public CI corpus and GHALogs identity checks.
- **Why this one wins**: fewest new abstractions and it proves the same CF rows as TASK-G-001.
- **What changes the decision**: another public corpus task can add a sibling identity validator without changing the SoT writer.

## Discoveries
- Full `cargo test -p context-graph-cli` remains blocked by pre-existing #81 because `tmp/onet_probe/db_30_2_text.zip` is absent.

## Failures
- `cargo test -p context-graph-cli` failed in `dynamicjepa_research_smoke_fsv.rs` due the missing O*NET ZIP. Re-observed on #81.

## Successes
- Focused test: `cargo test -p context-graph-cli mejepa_public_ci_cross_validate` passed, 3 passed / 0 failed / 0 ignored.
- Release build: `cargo build --release -p context-graph-cli` passed.
- Clippy: `cargo clippy -p context-graph-cli --no-deps --tests --examples -- -D warnings` passed.
- FSV: `cargo run --release -p context-graph-cli --example phase_g_public_ci_cross_validation_fsv` passed.

## Open Blockers
- #81 — crate-wide `context-graph-cli` tests require a local real O*NET ZIP.

## Tests / Build
- Tests added: 3 focused unit tests for correlation and invalid score rejection.
- Tests passing: yes for focused TASK-G-012 tests; full crate test blocked by #81.
- Build status: passing.
- Clippy: clean.

## FSV evidence
- `/mnt/d/contextgraph/fsv/phase-g-public-ci-cross-validation-fsv/public_ci_cross_validation_fsv.json` — sha256:f7934ffda58fbed2cdd75be453a4e415e48683db8b8b4c73532f31201d135edb, build_release_sha:03c12701

## GitHub issues touched
- #108 — claimed, progress-commented, committed as 03c12701; ready to close.
- #81 — re-observed full-suite test blocker comment: https://github.com/ChrisRoyse/contextgraph/issues/81#issuecomment-4443274317

## Resume Here Next Session
- Start by reading: `docs/futurebuild/13_TASK_INDEX.md` Phase G table.
- Watch out for: #81 blocks crate-wide `cargo test -p context-graph-cli` unless the real O*NET 30.2 text ZIP is present.
- Priority: pick the next unclaimed Phase G issue after #108 closes.
- Open question: none for TASK-G-012.

## Confidence: high
The command uses the existing Phase G SoT writer, the FSV reopens RocksDB and reads back every target CF, and all TASK-G-012-specific verification is green.
