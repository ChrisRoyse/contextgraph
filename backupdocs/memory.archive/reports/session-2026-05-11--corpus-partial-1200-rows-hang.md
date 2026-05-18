---
namespace: reports
agent_id: agent-claude-opus-4.7
created: 2026-05-11T22:06:00Z
updated: 2026-05-11T22:06:00Z
status: active
tags: report, corpus, hang, source-prep, git-timeout, partial-completion
---

# Session Report: Phase 0 corpus partial completion (1,200 / 2,400 rows) + source-prep hang diagnosis

## Task: Monitor the full 2,400-row corpus run to completion, then run the post-corpus verification (task #17).
## Outcome: partial — 4 of 8 categories committed; remaining 4 blocked by a source-prep hang. Root cause filed; corpus state durably persisted; resume path identified.

## Summary

The previous-session corpus generation (mejepa-corpus PID 75881) had been running since 16:00 UTC against `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`. Throughout this session I monitored progress via a persistent shell-loop monitor `bufg0cd8k`:

- 16:34 → 19:34 UTC: steady progress, 2 → 4 categories committed (`known_good`, `subtle_flip`, `off_by_one`, `swap_variable`).
- 19:34 → 22:01 UTC: monitor showed zero new reports. Investigation revealed the process was hung on `django__django-15790` source-prep since 17:02 UTC, alive but in `Dl` uninterruptible-disk-wait state with all 4 oracle workers idle in `futex_wait_queue`. Main thread stuck in `do_sys_poll` on a directory FD pointing at the partial `.git` of the in-progress source-work dir.
- 22:04 UTC: operator authorized "kill + file bug + report partial corpus" plan. SIGINT and SIGTERM had no effect (uninterruptible state); SIGKILL terminated the process cleanly.

The 1,200-row partial corpus is durably persisted on D-drive — both as `index.json` entries and as RocksDB rows in `CF_MEJEPA_MUTATION_CORPUS` / `CF_MEJEPA_ORACLE_VERDICTS` (both count = 1200). The `inspect_corpus_store` readback confirms `is_2400_complete: false`, but the 4 completed categories are usable as-is for training experiments and as the partial corpus for resume.

The root cause is a missing per-subprocess timeout in the source-prep code path — `Command::new("git").*.output()` waits forever on a hung subprocess. Filed as a high-severity blocker; the fix lives in `crates/context-graph-mejepa-corpus/src/source_patch_support.rs` (6 invocation sites). Until fixed, `--resume-incomplete` would re-hang on the same instance.

## Files Created or Modified in the Repo

- `docs/futurebuild/02_CURRENT_STATE_HONEST.md` — updated banner block to reflect 1,200-row partial corpus state, the hang diagnosis, and the resume-blocked-by-timeout-fix dependency.

## Files Written or Updated in ./memory/

- `blockers/2026-05-11--mejepa-corpus-git-subprocess-no-timeout.md` — high-severity blocker filing. Documents the root cause (6 `Command::new("git").output()` call sites with no timeout), the FSV-PROTOCOL §3 rule 5 violation, the fix plan (`wait_timeout` crate + per-operation timeouts + SIGTERM→SIGKILL escalation + new `--source-prep-timeout-secs` CLI flag + regression test against a sleep-9999 fake git), and the no-workaround constraint.
- `reports/session-2026-05-11--corpus-partial-1200-rows-hang.md` — this report.

## FSV Evidence Path

`/mnt/d/contextgraph/fsv/corpus-durable-root-20260511/partial-1200-corpus-final.json` — captured at 2026-05-11T22:05Z, includes:

- `all_passed: false`, `complete: false` (honest status — we did not produce the 2,400-row dataset)
- `entries_committed: 1200`, `entries_planned: 2400`
- `categories_committed: [known_good=300, off_by_one=300, subtle_flip=300, swap_variable=300]`
- `rocksdb_cf_counts: {mutation_cf: 1200, verdict_cf: 1200, provenance_cf: 0}` — independent RocksDB readback via `target/release/examples/inspect_corpus_store`
- `hang_diagnosis: {stuck_instance: "django__django-15790", first_stuck_at_utc: "17:02", killed_at_utc: "22:04", elapsed_hang_hours: 5.0, root_cause_blocker: "memory/blockers/2026-05-11--mejepa-corpus-git-subprocess-no-timeout.md"}`

## Decisions Made

- **Chose**: kill + file bug + report partial corpus. Operator-confirmed via AskUserQuestion.
- **Considered alternatives**:
  1. Wait for the 8h overall timeout to fire (~2h away). Rejected — pure clock waste, no new data, and the overall timeout doesn't run a structured-error report.
  2. Spend more time tracing the specific syscall before killing. Rejected — the diagnosis (no timeout on `Command::output()`) was already nailed down by the FD list + thread state + process tree inspection.
- **Why this one wins**: matches FSV-PROTOCOL §3 rule 5 (fail closed on unknowns) and rule 7 (Occam's razor — file the bug, capture what's persisted, move on).

## Discoveries

- **`Command::output()` is unsafe in production code paths.** Six call sites in `source_patch_support.rs` use it without timeouts. Any of them could hang the parent indefinitely if the spawned git subprocess wedges (filesystem stall, lockfile deadlock, malformed repo). The codebase should adopt a `wait_timeout`-wrapped helper as the standard subprocess pattern.
- **`--instance-timeout-secs` is misleading.** The CLI flag suggests it caps total per-instance time, but it actually applies only to the Docker oracle phase. The source-prep phase has no timeout. This naming ambiguity should be fixed alongside the timeout addition.
- **`Dl` state cannot be interrupted with SIGINT/SIGTERM.** When the process is in uninterruptible disk wait (e.g., stuck on `/mnt/d/` I/O), only SIGKILL works. This matters for any future supervisor design — graceful shutdown needs a timeout-then-SIGKILL escalation, not just SIGTERM.

## Failures

- **The corpus run did not complete.** This is the headline failure. Recovery path is well-defined (fix timeouts → resume). Per FSV-PROTOCOL §4 "Failures are as valuable as successes" — captured the diagnosis + root cause + fix plan rather than blaming the user or papering over.

## Successes

- Diagnosed the hang at root cause (specific FD + thread state + missing timeout) rather than restarting blindly.
- Persistent monitor + AskUserQuestion gave the operator a clear decision point with enumerated options.
- Partial corpus durably persisted on D-drive — RocksDB intact, index.json intact, all 1,200 rows independently verifiable via `inspect_corpus_store`.
- FSV evidence file written with full provenance, hang diagnosis, and next-action pointer.
- No regression: the partial corpus is usable for any downstream work that doesn't require all 8 categories (e.g., training experiments on the 4-category subset, embedder-instrument validation, predictor stress testing).

## Open Blockers

- **`2026-05-11--mejepa-corpus-git-subprocess-no-timeout.md`** (high severity) — must be fixed before any corpus resume attempt.

## Tests / Build

- No new code touched this session post-kill (the work is the diagnosis + reporting).
- The partial corpus passes its own integrity readback: `mutation_cf_count == verdict_cf_count == 1200 == entries.len()`.

## Resume Here Next Session

- **Highest priority next-up**: implement the fix in the blocker — add `wait_timeout = "0.4"` dev-dep to `context-graph-mejepa-corpus/Cargo.toml`, wrap each `.output()`/`.status()` call in `source_patch_support.rs` with a per-operation timeout, escalate SIGTERM → SIGKILL on timeout, return `MutationError::Hang { operation, duration_secs }`. Add the `--source-prep-timeout-secs` CLI flag. Add a regression test using a fake `git` script that sleeps forever.
- **Second priority**: re-run `target/release/mejepa-corpus generate --output /mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511 --language python --seed 17 --run-id-prefix mejepaPhase0Droot20260511 --oracle-workers 4 --instance-timeout-secs 1800 --source-prep-timeout-secs 300 --overall-timeout-secs 28800 --resume-incomplete` to fill in the remaining 4 categories.
- **Watch out for**: `--instance-timeout-secs` does NOT cover source-prep currently. Until that flag is unified or a new flag is added, both must be passed.
- **Open question**: why did `django__django-15790` source-prep hang specifically? The `.git` was only 44 KB at hang time — partial init or partial fetch. Worth investigating whether the upstream repository state is corrupt or whether `/mnt/d/` had a transient I/O event around 17:02 UTC. Note this in the bug fix PR description.
- **Existing partial corpus is usable**: 1,200 rows across 4 well-balanced categories (300 each) is enough to start downstream training-stack validation. Don't block experimentation on the missing 4 categories.

## Confidence: high

Real-RocksDB readback confirms 1,200-row state intact (independent verification via `inspect_corpus_store`, not just trust in the index file). Root cause located at specific source files with specific line numbers. FSV evidence file written. Operator decision captured in this report.
