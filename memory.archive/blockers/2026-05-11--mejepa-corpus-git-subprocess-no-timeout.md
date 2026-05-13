---
namespace: blockers
agent_id: agent-claude-opus-4.7
created: 2026-05-11T22:05:00Z
updated: 2026-05-11T22:05:00Z
status: active
severity: high
tags: mejepa-corpus, hang, timeout, source-prep, git-subprocess, root-cause
---

# mejepa-corpus source-prep hangs forever on stuck git subprocess (no timeout)

## What Happened

The full 2,400-row corpus generation against `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` hung on `django__django-15790` source-prep at 17:02 UTC and stayed wedged for 5 hours before the operator killed it at 22:04 UTC. Final state: 1,200 of 2,400 entries committed across 4 of 8 mutation categories (`known_good`, `subtle_flip`, `off_by_one`, `swap_variable`). Categories `delete_test_call` (97/300 patches generated then halted), `wrong_file`, `over_engineer`, `compile_error` did not start.

The mejepa-corpus PID 75881 was in state `Dl` (uninterruptible disk wait); SIGINT and SIGTERM had no effect — required SIGKILL. The main thread was in `do_sys_poll`, all 4 oracle workers in `futex_wait_queue`. The only suspicious FD was a directory handle on `/mnt/d/contextgraph/work/mejepa-corpus/source-work/75881-django__django-15790-delete_test_call-bdfab4b50c9fe7c1/.git` — opened 17:02 UTC, with the work tree empty (no checkout completed) and `.git` only 44 KB.

No alive child process held the pipe ends — the git subprocess died at some point without releasing pipe FDs cleanly to the parent, leaving the parent stuck in poll().

## Root Cause

`crates/context-graph-mejepa-corpus/src/source_patch_support.rs` has **6 invocations of `Command::new("git").*.output()` or `.status()` with no timeout**:

- line 181, 192: `read_git_blob`
- line 222: `run_git`
- line 245: `run_git_capture`
- line 338: a `git cat-file -e` check
- elsewhere in `source_patch.rs` and related modules

`std::process::Command::output()` waits *indefinitely* for the subprocess to exit. If git hangs (filesystem stall on `/mnt/d/`, kernel-level NFS-like deadlock, malformed repository, hung lockfile, etc.), the parent corpus generator waits forever. The `--instance-timeout-secs 1800` CLI flag applies only to the Docker oracle phase, not source-prep.

## Why It Matters

- The corpus generator is the foundational training-data pipeline for ME-JEPA. A silent hang means: hours of wallclock wasted, partial corpus on disk, no actionable error message in the runtime log. Per FSV-PROTOCOL §3 rule 5 ("FAIL CLOSED ON UNKNOWNS") and the operator's hardening directive ("If something doesn't work it should error out and have robust error logging"), this is the worst failure mode possible.
- A future agent running `--resume-incomplete` will hit the same hang on `django__django-15790` unless the timeout is added. So the 1,200-row partial corpus cannot be extended without first fixing this.

## Latest Verification

Re-run on 2026-05-11T22:04Z:
- `pgrep -af 'mejepa-corpus generate'` returned the live PID 75881; `ps -o stat= -p 75881` showed `Dl` (uninterruptible disk wait).
- Tree walk: `/mnt/d/contextgraph/work/mejepa-corpus/source-work/75881-django__django-15790-*/` empty working tree; `.git` 44 KB.
- 4 oracle worker threads idle in `futex_wait_queue`.
- Process consumed 200 s CPU over 6 h 4 m wallclock — ~99% idle.
- `inspect_corpus_store` readback: `{is_2400_complete: false, mutation_cf_count: 1200, verdict_cf_count: 1200, provenance_cf_count: 0}`. 1,200-row partial corpus persisted intact.

## Fix Plan (at root cause, no workaround)

1. Add a `wait_timeout = "0.2"` (or similar) dev-dep to `context-graph-mejepa-corpus/Cargo.toml`.
2. Replace each `.output()`/`.status()` call site in `source_patch_support.rs` with `.spawn() → .wait_timeout(Duration::from_secs(N))` where N is a per-operation cap:
   - cat-file / show: 60 s (sparse blob read)
   - apply / write / config: 30 s (in-repo metadata write)
   - read-tree / checkout: 300 s (worktree materialization)
3. On timeout, send SIGTERM → SIGKILL escalation and return a `MutationError::Hang { operation, duration_secs }` so the parent surfaces a structured error.
4. Add a new CLI flag `--source-prep-timeout-secs` (default 300) that bounds the *total* per-instance source-prep time, applied on top of the per-subprocess caps.
5. FSV: a regression test that points at a fake `GIT_EXEC_PATH` containing a `git` script that just `sleep 9999` — the corpus generator must terminate with a structured Hang error within (N+1) seconds, not hang forever.

## Workaround until fix lands

None — per FSV-PROTOCOL §3 rule 3 (NO WORKAROUNDS). The operator must wait until this fix lands before re-running corpus generation. Until then, the 1,200-row partial corpus on disk is the source of truth.

## Related

- See also: ./memory/reports/session-2026-05-11--corpus-partial-1200-rows-hang.md (the corpus-completion report covering this outcome)
- Source-prep code: `crates/context-graph-mejepa-corpus/src/source_patch_support.rs`
- Affected SoT: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`
- FSV protocol violated: `docs/futurebuild/specs/FSV-PROTOCOL.md` §3 rule 5
- Tags: mejepa-corpus, timeout, hang, source-prep, git-subprocess
