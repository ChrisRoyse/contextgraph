---
namespace: blockers
agent_id: agent-claude-opus-4.7
created: 2026-05-11T23:20:00Z
updated: 2026-05-11T23:20:00Z
status: active
severity: medium
tags: subprocess, timeout, hang, mejepa-corpus, follow-up
---

# Remaining untimed Command::output() / .status() call sites

## What I found

During the 2026-05-11 source-prep hang investigation I audited every
`Command::new(...).output()` and `.status()` invocation in the
`mejepa-*` crates. The source-prep call sites in
`crates/context-graph-mejepa-corpus/src/source_patch_support.rs` were
fixed via `run_git_timed` with pipe-drain threads and SIGTERM→SIGKILL
escalation. The following call sites remain untimed:

### Production code paths (could hang)

- `crates/context-graph-mejepa-corpus/src/generate.rs:532` — venv-python
  subprocess that loads SWE-bench official patches via a Python snippet.
  If the dataset loader wedges, the whole corpus run blocks.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs:1893`
  (`docker_stdout`) — generic docker subprocess wrapper used by
  `prepare-images`. Could hang on stuck Docker daemon.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs:1908`
  (`docker_status`) — same as above, status-only variant.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs:2208` —
  duplicate Python-subprocess pattern from `generate.rs:532`. CLI-side
  variant for `verify` command. Same hang risk.
- `crates/context-graph-mejepa-embedders/src/vram.rs:93` — nvidia-smi
  subprocess. Normally <1 s but can hang on broken GPU driver state.

### Intentionally untimed (the timeout machinery itself)

These MUST stay untimed — they are the kill-escalation path inside
`oracle::wait_with_timeout_interruptible` and would recurse infinitely
if forced to time themselves:

- `crates/context-graph-mejepa-corpus/src/oracle.rs:1084,1114` —
  `kill -TERM` / `kill -KILL` inside `terminate_child_process_group`.
- `crates/context-graph-mejepa-corpus/src/source_patch_support.rs:443,456`
  — same `kill` pattern inside `terminate_child_and_collect`.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs:1852,1865`
  — `kill -TERM` / `kill -KILL` in the CLI-side escalation.

### Test-only (out of scope)

- `crates/context-graph-mejepa-corpus/src/source_patch.rs:355,403` —
  `#[cfg(test)]` helpers; do not run in production.
- `crates/context-graph-mejepa-corpus/src/oracle.rs:1801,1829` —
  `#[cfg(unix)] #[test]` pgrep verifications.

## Why It Matters

Each production untimed site is a future-hang waiting to happen. The
2026-05-11 source-prep hang took 5 hours to surface (no error log, no
exit). Any of the above could do the same to a different subsystem.

Per FSV-PROTOCOL §3 rule 5 (FAIL CLOSED ON UNKNOWNS) and the operator's
hardening directive ("If something doesn't work it should error out
and have robust error logging"), every production subprocess MUST have
a deadline. The pattern is now well-established
(`source_patch_support::run_git_timed`); applying it to the remaining
sites is mechanical.

## Fix plan

1. Generalize the `run_git_timed` helper into a generic
   `run_subprocess_timed` (program-agnostic) and move it to
   `context-graph-paths` or a shared `mejepa-subprocess` crate.
2. Replace each production call site with the new helper, using
   per-operation timeout classes (e.g. Docker = 600 s, nvidia-smi = 30 s,
   Python dataset load = 300 s).
3. Add regression tests per FSV-PROTOCOL §5 for at least one site of
   each subprocess type (docker, python, nvidia-smi).

## Workaround until fix lands

None — the fix is mechanical and small. If a hang happens before the
fix lands, the operator will see the process in `Dl` state via `ps`
and have to SIGKILL it manually, then file the diagnosis here.

## Related

- See also: `./memory/blockers/2026-05-11--mejepa-corpus-git-subprocess-no-timeout.md`
  — the original source-prep hang that motivated this audit.
- The pattern fix: `crates/context-graph-mejepa-corpus/src/source_patch_support.rs::run_git_timed`
- FSV-PROTOCOL §10 (common failure modes) — explicitly lists "Sync
  write returning Ok() but disk write never happens" and the surrounding
  pipe-deadlock class.
- Tags: subprocess, timeout, hang, audit, mejepa
