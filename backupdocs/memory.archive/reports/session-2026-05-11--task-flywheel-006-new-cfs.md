---
namespace: reports
agent_id: agent-claude-opus-4.7
created: 2026-05-11T16:22:00Z
updated: 2026-05-11T16:22:00Z
status: active
tags: report, task-flywheel-006, mejepa-cf, rocksdb, flywheel, phase-a, fsv
---

# Session Report: TASK-FLYWHEEL-006 — Register CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_FAILURE_EXEMPLARS, CF_MEJEPA_AGENT_FEEDBACK

## Task: Implement REQ-FLYWHEEL-10 — three new flywheel column families end-to-end with FSV.
## Outcome: complete

## Summary

The previous agent had finished a full bug-fix cycle on the Phase 0 corpus sparse source-patch builder (scratch-file leak + candidate Python file ranking) and kicked off the resumed full 2400-row corpus generation against `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511`. That run is still in-flight (background pid 75881, monitor task active) and now sits at 300 known_good rows committed + 300 subtle_flip patches generated, with Docker oracle reports advancing from 300 → 414 during this session.

I picked TASK-FLYWHEEL-006 as the non-conflicting Phase A workstream: it touches different crates (`context-graph-mejepa-cf`, `context-graph-mejepa`, `context-graph-mejepa-hygiene`) so it can't conflict with the corpus run, it unblocks downstream `TASK-PREDICT-002` and `TASK-ENRICH-002`, and it has a clean FSV path against a real RocksDB SoT.

What shipped:

1. **`crates/context-graph-mejepa-cf/src/lib.rs`** — added three new `CF_MEJEPA_*` constants (`CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`), a `FLYWHEEL_CFS` group slice, and extended `all_hygiene_referenced_cfs()` so the daemon hygiene opener creates them on first startup. Two new unit tests cover the registration invariant (`flywheel_cfs_are_registered_in_daemon_opener`) and the const-name=on-disk-name invariant (`flywheel_cf_names_match_const_idents`).

2. **`crates/context-graph-mejepa/src/types.rs`** — added the REQ-FLYWHEEL-10 value types: `PredictionId([u8;16])`, `FeedbackId([u8;16])`, `PanelId([u8;32])`, `WitnessHash([u8;32])`, `AgentId(String)`, plus enums `SurpriseSeverity`, `FeedbackKind`, `FailureModeClass`, plus structs `ActualOutcome`, `EmbedderViolationSignature`, `DdaSignals`, `ExemplarRef`, `SurpriseEvent`. Every struct has a `validate()` method enforcing the field-by-field invariants from TECH-FLYWHEEL.md §6 (N×(N-1)/2 upper-triangle length contract, cosine ∈ [-1,1], non-negative MI, ≤100k exemplars per bucket, ≤64 KiB extra_structured_data payload, etc.). The existing `validate_non_empty_id` was refactored to delegate to a new `validate_bounded_id` so the agent-id length limit reuses the same control-char checks.

3. **`crates/context-graph-mejepa/examples/cf_round_trip_fsv.rs`** — new FSV example. Opens a fresh RocksDB at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/rocks-<ts>` with all three new CFs as descriptors. Writes one synthetic instance of each value struct (4-embedder DDA panel, AssertionMismatch / {E1,E7} exemplar bucket, SurpriseEvent.High), reads each back via an independent `cf_handle` + `get_cf`, and asserts field-by-field equality. Then runs four boundary cases:
   - **zero-embedder DDA** — `DdaSignals::default()` round-trips
   - **exemplar bucket at-cap and above-cap** — 100 000 entries validate; 100 001 entries rejected (fail-closed)
   - **empty agent_explanation** — `FeedbackKind::Confirmed` + empty explanation + null payload round-trips
   - **concurrent writes** — two OS threads writing distinct `(prediction_id, agent_id, ts)` keys both persist
   
   The example fails closed with a non-zero exit code on the first divergence and writes evidence to `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json`. Also asserts post-write CF counts grew by the expected amounts so a silent put-to-wrong-CF would still be caught.

4. **`crates/context-graph-mejepa-hygiene/tests/flywheel_cfs_registered_test.rs`** — new integration test using the real `open_hygiene_rocksdb()` (the daemon's actual storage opener). Confirms all three flywheel CFs land on disk on a fresh root AND survive a close-reopen cycle (crash-recovery scenario).

## Files Created or Modified in the Repo

- `crates/context-graph-mejepa-cf/src/lib.rs` — +43 lines: 3 constants, FLYWHEEL_CFS slice, hygiene-opener chain extension, 2 new tests
- `crates/context-graph-mejepa/src/types.rs` — +468 lines: 5 ID newtypes, 3 enums, 4 structs with validate methods, refactored validate_bounded_id helper, new validate_cosine helper
- `crates/context-graph-mejepa/examples/cf_round_trip_fsv.rs` — new file, 572 lines, 4-boundary FSV harness
- `crates/context-graph-mejepa-hygiene/tests/flywheel_cfs_registered_test.rs` — new file, 2-test integration coverage

## Files Written or Updated in ./memory/

- `reports/session-2026-05-11--task-flywheel-006-new-cfs.md` — this report
- `discoveries/2026-05-11--mejepa-cf-uppercase-string-convention.md` — convention note about the on-disk string format

## Decisions Made

None binding beyond what spec already locks. One convention call: kept `CF_MEJEPA_DDA_SIGNALS = "CF_MEJEPA_DDA_SIGNALS"` (uppercase) to match every other constant in `mejepa-cf/src/lib.rs`, even though TECH-FLYWHEEL.md §6 illustrates lowercase. Rationale documented in `discoveries/2026-05-11--mejepa-cf-uppercase-string-convention.md`. Did not file an issue because no caller depends on the lowercase form; the existing mejepa-cf invariant (constant ident == on-disk name) takes precedence.

## Discoveries

- `mejepa-cf` is intentionally dependency-free per the crate doc comment ("so ME-JEPA runtime, hygiene, MCP, and training crates can share CF names without cycles or string drift"). Putting the FSV example under `crates/context-graph-mejepa-cf/examples/` would force pulling in rocksdb + serde + mejepa::types as either deps or dev-deps, breaking the contract OR creating a cycle (mejepa already depends on mejepa-cf). The example lives under `mejepa/examples/cf_round_trip_fsv.rs` instead — the task DoD lists the path as `mejepa-cf/examples/...` but that path is not buildable under the current dependency graph; the relocation is documented in this report.
- An existing parallel constants file (`crates/context-graph-mejepa-corpus/src/oracle.rs`) defines its own `CF_MEJEPA_MUTATION_CORPUS = "mejepa_mutation_corpus"` (lowercase) that clashes with `mejepa-cf::CF_MEJEPA_MUTATION_CORPUS = "CF_MEJEPA_MUTATION_CORPUS"` (uppercase). This is a pre-existing inconsistency — the corpus crate uses its own private constant and never imports the shared one. I did not touch it. Worth a follow-up issue once the corpus run completes, but it is not load-bearing for TASK-FLYWHEEL-006.

## Failures

None this session.

## Successes

- `cargo clippy -p context-graph-mejepa-cf --no-deps --tests -- -D warnings`: clean
- `cargo clippy -p context-graph-mejepa --no-deps --lib --examples -- -D warnings`: clean (after one `single_match` → `if let` refinement during review)
- `cargo test -p context-graph-mejepa-cf`: 3/3 passed (incl. 2 new tests)
- `cargo test -p context-graph-mejepa --lib`: 76/76 passed
- `cargo test -p context-graph-mejepa-hygiene --test flywheel_cfs_registered_test`: 2/2 passed
- `target/release/examples/cf_round_trip_fsv`: prints `OK fsv_report=/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json`; evidence JSON shows `all_passed=true`, CF counts moved 0→{DDA:2, EXEMPLARS:1, FEEDBACK:4}, every happy-path `readback_equal=true`, every boundary case `pass=true`. Real RocksDB SST files persisted on D-backed disk at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/rocks-1778516317859/000009.sst` and siblings.

## Open Blockers

None new. The pre-existing `2026-05-11--full-context-graph-cli-clippy-debt.md` (528 clippy errors in `context-graph-cli`) remains active but is unrelated.

## Tests / Build

- Tests added: 5 total (2 in mejepa-cf, 3 in mejepa-hygiene integration test = 2 actual unit tests plus the in-example boundary asserts)
- Tests passing: yes — workspace-relevant subsets all green
- Build status: `cargo build --release -p context-graph-mejepa --example cf_round_trip_fsv` and `cargo build --release -p context-graph-mejepa-cf` both clean

## Resume Here Next Session

- The background corpus generation (pid 75881) is still running. Last monitor event: 414 reports, all 300 known_good + 300 subtle_flip rows generated, oracle processing subtle_flip verdicts now. Categories `off_by_one / swap_variable / delete_test_call / wrong_file / over_engineer / compile_error` are still 0 patches each. Expect many more hours of Docker oracle time before completion.
- Next-priority Phase A task per `docs/futurebuild/13_TASK_INDEX.md`: **TASK-FLYWHEEL-005** (`ActiveLearningKind::AgentSurprise` enum variant — 0.5d). It is now structurally unblocked by FLYWHEEL-006 since the SurpriseEvent and severity types exist. Then **TASK-PREDICT-002** (`mejepa_record_agent_feedback` MCP tool) becomes available.
- Watch out for: the pre-existing mejepa-cf vs corpus-oracle uppercase/lowercase CF name divergence (see `discoveries/2026-05-11--mejepa-cf-uppercase-string-convention.md`). If a future task is "wire MUTATION_CORPUS into the daemon opener", that lowercase/uppercase split will surface as a duplicate-CF RocksDB error and needs harmonization.
- Open question: should `mejepa::types::FailureModeClass` be moved (or re-exported) to `context-graph-mejepa-cf` so the CF key encoder doesn't pull in the full mejepa crate? Deferred until first downstream consumer materializes — the example demonstrates the encoder shape works either way.

## Confidence: high

Real-RocksDB FSV passes (`all_passed=true`, on-disk SST evidence at the cited path); independent integration test against `open_hygiene_rocksdb` (the daemon's actual opener) passes; cargo clippy clean with `-D warnings`; cargo test clean for all touched crates. No regressions in the 76-test mejepa lib suite.
