# Phase 2 — State, Invariants, And Synthetic Tests

This document is the canonical reference for the **shipped** Phase 2
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| Git branch/commit/diff/clean verifiers | real `.git` object DB and working tree, read through the installed `git` binary |
| Filesystem connector | real file bytes, size, SHA-256, optional UTF-8 content |
| Command verifier | real process exit code/stdout/stderr plus declared artifact files on disk |

Phase 2 originally shipped `LEDGER_SCHEMA_VERSION = 2`. The current live
ledger schema is `LEDGER_SCHEMA_VERSION = 13` after Phase 23; `blackbox init`
removes older ledger files (`.sqlite`, `-wal`, `-shm`) before initializing the
current schema. There is no migration shim.

## Shipped Connectors

```text
sqlite_local_v1   ledger read/write and integrity verification
git_local_v1      Git repository readback via git rev-parse/status/diff/cat-file
fs_local_v1       local file readback via fs::read + sha256
cmd_local_v1      argv execution via std::process::Command + artifact readback
```

`blackbox connector test` must fail closed on unsupported connector arguments.
For `cmd_local_v1`, argv is passed as discrete arguments and hyphen-prefixed
values such as `-c` are accepted as command values.

## Shipped Verifiers

| Verifier | Required spec | Passed condition | Failed condition | Blocked condition |
|---|---|---|---|---|
| `git_branch_exists_v1` | `source.repo`, `expected.branch` | branch ref resolves to a commit | `GIT_BRANCH_NOT_FOUND` | missing/invalid spec or unreadable repo |
| `git_commit_exists_v1` | `source.repo`, `expected.commit` | commit-ish resolves to a commit | `GIT_COMMIT_NOT_FOUND` | missing/invalid spec or unreadable repo |
| `git_file_diff_matches_patch_v1` | `source.repo`, `expected.path`, `expected.patch` | exact patch string matches `git diff --no-ext-diff --no-color -- <path>` | `GIT_FILE_DIFF_MISMATCH` | missing/invalid spec or unreadable repo |
| `git_working_tree_clean_v1` | `source.repo`, `expected.clean` | porcelain status clean bool matches expected | `GIT_WORKING_TREE_DIRTY` | missing/invalid spec or unreadable repo |
| `cmd_exit_zero_v1` | `source.cwd`, `expected.command`, optional `expected.artifacts` | exit code `0` and all artifacts exist | `CMD_EXIT_NON_ZERO` or `CMD_ARTIFACT_MISSING` | invalid argv schema, missing workdir, missing executable |

Schema/type failures are `blocked` and still write `expected_delta` and
`verifier_result` when the top-level verifier spec can be associated with an
existing run/tool-call. They do not write `readback_snapshot` if no physical
read occurred.

## Ledger Records Written By `blackbox verify`

For each associated verifier spec:

1. `expected_delta` captures `tool_call_id`, `verifier_id`,
   `source_of_truth`, `expected_persisted_delta`, redacted spec, and timestamp.
2. `readback_snapshot` captures connector id, operation, source locator,
   observed physical state, and timestamp when readback succeeds.
3. `verifier_result` captures status, score, error fields, connector,
   source locator, expected delta, and actual readback.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase2_fsv -- --nocapture
```

Current suite: 19/19 passing. Coverage includes:

- connector list/test for Git, filesystem, and command sources;
- happy path for all five verifiers in one real run;
- three edge cases per verifier against real local sources;
- invalid command argv schema proving the command is not executed;
- v1 ledger wipe when initializing schema v2.

## Manual FSV Evidence

Manual FSV completed on 2026-05-02:

```text
Log: /tmp/bbx-phase2-manual-fsv.vMFZRR/fsv.log
Ledger source of truth: /tmp/bbx-phase2-manual-fsv.vMFZRR/blackbox.sqlite#ledger_records
Git source of truth: /tmp/bbx-phase2-manual-fsv.vMFZRR/repo/.git + working tree
Command artifact: /tmp/bbx-phase2-manual-fsv.vMFZRR/cmd-output.txt#sha256
Final ledger at Phase 2 completion: schema_version=2, record_count=25,
integrity.status=passed
Final record counts: agent_run=1, tool_call=1, expected_delta=8,
readback_snapshot=7, verifier_result=8
```

Happy-path physical proofs:

- branch readback matched `git rev-parse refs/heads/master^{commit}`;
- commit readback matched `git cat-file -t <HEAD> == commit`;
- clean readback matched empty `git status --porcelain=v1 --untracked-files=all`;
- diff readback patch SHA matched direct `git diff` SHA
  `95c41d195032bc05337ee0fbdb1cba1995c947718dfc8be7a7ba1e1b32a8d125`;
- command artifact existed with contents `phase2-command` and SHA
  `f1b24d344d4d6d596362796d0a53a21a7e283522280f1136712cbd5b04831f5c`.

Manual edge cases:

1. Missing branch: direct `git show-ref --verify` exit `1`; verifier persisted
   `status=failed`, `error_code=GIT_BRANCH_NOT_FOUND`.
2. Dirty working tree: direct porcelain status was `" M app.txt\n"` before and
   after; verifier persisted `status=failed`,
   `error_code=GIT_WORKING_TREE_DIRTY`.
3. Invalid command argv: artifact was absent before and after; verifier
   persisted `status=blocked`, `error_code=VERIFIER_SPEC_FIELD_TYPE`,
   `field_path=$.expected.command[1]`, and no readback row was written.

## Invariants

- No verifier may mark success from a process return value alone; it must store
  a physical readback or block.
- No non-string command argv item may be dropped or coerced; invalid argv
  blocks before execution.
- No Git spawn error may be converted into a repo/readback mismatch; it must
  surface as `GIT_EXEC_FAILED`.
- No command artifact may be assumed from exit code; declared artifacts must
  exist on disk and be hashed.
- No schema bump may preserve older ledger rows; older schemas are wiped on
  init.
