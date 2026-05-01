# 20 — Implementation Plan Overview (Per-PR Critical Path)

This document is the per-PR operational plan that an implementer
follows to execute the upgrade. It maps each of the 17 PRs from
`13_implementation_phases.md` §9 to:

1. Files touched (paths, with one-line summary of the change).
2. Tests added (file path + test names).
3. Commit message template (per the existing v1 commit style).
4. Definition of done (DoD) for the PR.
5. The next PR that this one unblocks.

Detailed per-area diffs are in:

- `21_storage_implementation.md` (CFs, RocksDB descriptors, schema)
- `22_core_records_implementation.md` (typed records, ID wrappers,
  serde)
- `23_audit_log_implementation.md` (signal-yield instrumentation)
- `24_cli_subcommands_implementation.md` (the six new subcommands)
- `25_fsv_harnesses_implementation.md` (FSV-9..12 implementation)
- `26_mi_audit_implementation.md` (KSG estimator, MINE option)
- `27_first_commit_checklist.md` (the literal first-PR tasks)

## 1. Per-PR table

| # | Title | Phase | Files | LoC est. | Blocks | Reviewer |
| ---: | --- | ---: | ---: | ---: | --- | --- |
| 1 | feat(storage): add 3 new CFs, schema-version pre-check | 1 | 5 | ~250 | PR 2,3,4,7,8 | — |
| 2 | feat(core): add 3 new typed records + ID wrappers | 1 | 8 | ~400 | PR 4,7,8,9 | — |
| 3 | feat(audit): signal\_yield field + lookup table | 1 | 6 | ~200 | PR 12 | — |
| 4 | feat(panel): pairwise readings compute-on-write + write path | 2 | 12 | ~600 | PR 6,10 | — |
| 5 | feat(toml): pair\_kinds + constellation TOML schema | 2 | 6 | ~400 | PR 4,7,9 | — |
| 6 | feat(cli): inspect-cf decoders for new CFs | 1/2 | 4 | ~150 | — | — |
| 7 | feat(cli): build-constellation subcommand + persistence | 3 | 9 | ~700 | PR 9,11 | — |
| 8 | feat(cli): calibrate-threshold + recalibrate-threshold | 3 | 7 | ~500 | PR 9,11 | — |
| 9 | feat(plan): combined utility + G\_tau guard predicate | 3 | 11 | ~800 | PR 11 | — |
| 10 | feat(fsv): FSV-9 harness + tests | 2 | 4 | ~350 | — | — |
| 11 | feat(fsv): FSV-10 + FSV-11 harnesses + tests | 3 | 6 | ~600 | — | — |
| 12 | feat(cli): compute-mc-ratio subcommand + plot | 4 | 6 | ~450 | PR 13 | — |
| 13 | feat(fsv): FSV-12 harness + tests | 4 | 3 | ~250 | PR 14 | — |
| 14 | feat(cli): audit-pairwise-mi + KSG estimator | 5 | 10 | ~900 | PR 15 | — |
| 15 | feat(release): release-D local CI gate + v2 packager | 5 | 8 | ~500 | PR 17 | — |
| 16 | docs: research paper v1.1 update | post-5 | 14 | ~800 | — | — |
| 17 | feat(transfer): bridge domain pack + cross-domain pilot | 6 (v2.1) | 12 | ~1{,}000 | — | — |

Total estimated LoC across the upgrade: ~8{,}900 lines (Rust + tests +
docs + TOML).

## 2. Critical path

The minimal sequence for a v2.0 release (PR 1 → PR 15):

```
PR 1: Foundation (CFs, manifest version)
   ├── PR 2: Core records & IDs ────────────┐
   ├── PR 3: Audit log signal_yield         │
   ├── PR 6: Inspect-cf decoders ────┐     │
   └── PR 5: TOML v2 schema ────┐    │     │
                                │    │     │
                                ▼    ▼     ▼
                              PR 4: Pairwise compute-on-write
                                │
                                ├── PR 10: FSV-9
                                │
                                └── PR 7: build-constellation
                                      │
                                      └── PR 8: calibrate-threshold
                                            │
                                            └── PR 9: combined guard
                                                  │
                                                  └── PR 11: FSV-10/11
                                                        │
                                                        └── PR 12: compute-mc-ratio
                                                              │
                                                              └── PR 13: FSV-12
                                                                    │
                                                                    └── PR 14: audit-pairwise-mi
                                                                          │
                                                                          └── PR 15: release-D
                                                                                │
                                                                                └── PR 16: paper v1.1
```

PR 17 (cross-domain transfer) is v2.1; not on the critical path.

## 3. Per-PR commit message template

Following the existing v1 commit style (verified from
`git log --oneline | head -5`):

```
feat(<crate>): <one-line summary; imperative mood>

<paragraph 1: what this PR does, with concrete numbers / row counts /
hashes where applicable>

<paragraph 2: implementation notes — invariants preserved, edge cases
covered, FSV harness updates>

<paragraph 3 (optional): test counts, wall-time impact, any deferred
follow-ups>

Co-authored-by: Chris Royse <chrisroyseai@gmail.com>
```

Example for PR 4:

```
feat(panel): persist pairwise readings as first-class records

Adds dj_pairwise_readings CF with 24-byte fixed key
[event_uuid][instrument_j][instrument_k] and JSON value carrying
cosine_agreement (always present) plus 6 optional pair-kinds gated
by per-instrument TOML pair_kinds declaration. Compute-on-write at
panel materialisation; transactional with the panel write via
put_panel_with_readings_and_pairwise_batch. A 1,000-event career
bundle now persists 45,000 pairwise rows.

FSV-9 (added in PR 10) verifies pair count per event = C(N_d, 2),
recomputes cosine within 1e-4 tolerance, and validates the
kinds_emitted bitset against the TOML declaration. EC-9-A through
EC-9-D edge cases produce the documented typed errors without
mutating the filesystem.

Test additions: 12 unit tests in pairwise_store.rs; integration
suite extended in dynamicjepa_research_smoke_fsv.rs. Pairwise
materialisation under 50ms per event on RTX 5090.

Co-authored-by: Chris Royse <chrisroyseai@gmail.com>
```

## 4. PR review checklist (every PR)

```
[ ] cargo fmt --all -- --check
[ ] cargo check -p <crate> --all-features
[ ] cargo clippy --all-targets --all-features -- -D warnings
[ ] cargo test -p <crate>
[ ] integration tests where applicable
[ ] FSV evidence emitted where applicable
[ ] all new typed errors are in the edge-case catalog
[ ] all new audit log operation_kinds are in the signal_yield lookup
[ ] no new "we" / "our" / "us" in any prose (single-author voice)
[ ] no recruiting / hiring / fairness / Phase-12 language
[ ] commit message follows the template
```

## 5. PR sequencing guidance

**Parallelisable.** PRs 1, 2, 3 can be reviewed in parallel; they
touch different files within the same Phase 1.

**Strictly sequential.** PRs 4, 7, 8, 9, 11, 12, 13, 14 form a chain
where each consumes the previous one's persisted state for its tests.

**Independent.** PR 5 (TOML schema) is mostly independent of PRs 1–3
but blocks PR 4 (pairwise materialisation reads the new TOML
fields).

## 6. Pre-PR-1 setup tasks

Before opening PR 1, the implementer should:

1. Create a v2.0 working branch off `main`:
   `git checkout -b feature/v2-foundation`.
2. Verify the existing v1 test suite passes on the local host:
   `cargo test --workspace --all-features`.
3. Capture the v1 release bundle's row counts and SHA-256s from the
   existing `mejepa_5090_artifact_v1.0.0_rc5_sif_verified` archive
   for v1↔v2 comparison.
4. Read this entire `docs/upgradeplancombine/` directory linearly.
5. Run the existing `local_ci_gate.sh release-c` once to refresh
   the v1 release-C reference values.

These tasks take about 2 hours on the documented host.

## 7. Per-Phase exit-gate ceremony

When all PRs in a phase pass review and merge, the implementer:

1. Tags the merge commit `v2.0.0-phaseN`.
2. Runs the phase's exit-criteria integration test (per
   `13_implementation_phases.md` §1 table).
3. Writes a one-page phase-N FSV report under
   `docs/5090jepa/smokedocs/reports/<date>_phaseN_v2_fsv.md`
   following the existing report-style template.
4. Updates `docs/upgradeplancombine/00_master_index.md`'s status
   table.
5. Notifies the next phase's PRs they are unblocked.

This ceremony preserves the project's evidence-bundle discipline
across phases.

## 8. Communication discipline

This upgrade is a single-author project (per the v1 paper); no
external coordination is required. The PRs are reviewed by the
author against the criteria above. The "single-author voice" rule
applies to all commit messages, PR descriptions, and the v1.1
paper update.

## 9. Rollback plan

If any phase reveals an unanticipated defect:

1. Phase N's PRs are reverted in reverse order on the working
   branch.
2. The previous phase's tag remains valid as the rollback target.
3. The v1 release artifact is *unaffected*; it remains a valid
   reproducibility anchor.
4. The `docs/upgradeplancombine/` doc tree is updated to mark the
   reverted phase as "blocked" with the typed defect code.

Per the v1 paper's failure-preservation discipline, any partial
bundles produced by the failed phase are preserved on disk as
evidence.

## 10. Post-upgrade tasks (after v2.0 release-D passes)

After Phase 5 passes the release-D gate:

1. Tag the release commit `v2.0.0-rc1`.
2. Update `CITATION.cff` to `version: v2.0.0-rc1`.
3. Update the Zenodo record to v2.0 metadata via the documented
   API edit-mode protocol.
4. Upload the v2 archive to Zenodo as a new file slot.
5. Open PR 16 (the v1.1 paper update).
6. Once PR 16 merges, regenerate the paper PDF and upload to Zenodo.
7. Tag the final release: `v2.0.0`.

## 11. Implementation chunk size guidance

Each PR should ideally be reviewable in **45 minutes or less** by an
implementer who has read this directory. PRs estimated at >800 LoC
(PR 4, 9, 14) should be reviewed in two passes (architecture pass
+ test pass).

A ground-truth check during PR review: if a single PR is touching
more than ~15 files, it should probably be split.

## 12. Cross-doc references

Implementers reading this should also consult:

- `21_storage_implementation.md` — for the literal RocksDB
  descriptor wiring and key-format byte counts.
- `22_core_records_implementation.md` — for the literal Rust
  struct definitions and serde derive macros.
- `23_audit_log_implementation.md` — for the literal signal-yield
  lookup table and audit-log writer integration.
- `24_cli_subcommands_implementation.md` — for the literal `clap`
  argument structures of the six new subcommands.
- `25_fsv_harnesses_implementation.md` — for the literal FSV
  harness algorithm pseudocode.
- `26_mi_audit_implementation.md` — for the KSG-1 estimator
  formula and the MINE training loop.
- `27_first_commit_checklist.md` — for the literal first-PR
  shell-command checklist.
