# Phase 6 — Tasks

Eight ordered tasks. Each script's full body lives in [03_FILE_SPECS.md](03_FILE_SPECS.md) (which itself references [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md) verbatim — that doc IS the canonical bash).

## Current task snapshot

As of 2026-05-05, this phase is complete:

- Task 6.1 directory structure exists.
- Tasks 6.2 and 6.3 have executable files under `scripts/cc/`.
- Task 6.4 has executable `_common_helpers.sh`.
- Task 6.5 has all six tests present: `test_session_start.sh`, `test_user_prompt_submit.sh`, `test_pre_tool_use_edit.sh`, `test_post_tool_use_edit.sh`, `test_pre_compact.sh`, and `test_stop.sh`.
- Tasks 6.6 through 6.8 passed. `/tmp/ccreality-phase-progress.log` contains `phase-06: PASS`.

## Task 6.1 — Create the directory structure

```bash
cd /home/cabdru/contextgraph
mkdir -p scripts/cc/tests/hooks
ls scripts/cc/
```

Expected: `scripts/cc/` and `scripts/cc/tests/hooks/` exist.

## Task 6.2 — Write `scripts/cc/_common.sh`

Copy the body verbatim from [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md) §"Common helpers — `scripts/cc/_common.sh`".

```bash
chmod +x scripts/cc/_common.sh
command -v shellcheck >/dev/null 2>&1 || { echo "shellcheck is required for Phase 6"; exit 1; }
shellcheck scripts/cc/_common.sh
```

## Task 6.3 — Write each of the 6 hook scripts

For each, copy verbatim from [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md):

| Script | Source section in 07_HOOKS_SPEC.md |
|--------|-----------------------------------|
| `scripts/cc/hook_session_start.sh` | "1. SessionStart" |
| `scripts/cc/hook_user_prompt_submit.sh` | "2. UserPromptSubmit" |
| `scripts/cc/hook_pre_tool_use_edit.sh` | "3. PreToolUse Edit/Write" |
| `scripts/cc/hook_post_tool_use_edit.sh` | "4. PostToolUse Edit/Write" |
| `scripts/cc/hook_pre_compact.sh` | "5. PreCompact" |
| `scripts/cc/hook_stop.sh` | "6. Stop" |

Note: `hook_user_prompt_submit.sh` was UPDATED during planning to also inject the last 3 shift events. Use the version in 07_HOOKS_SPEC.md (it has the shift-log block).

After each:
```bash
chmod +x scripts/cc/hook_*.sh
command -v shellcheck >/dev/null 2>&1 || { echo "shellcheck is required for Phase 6 verification"; exit 1; }
shellcheck scripts/cc/hook_*.sh
```

If shellcheck reports issues (SC2086 unquoted vars, SC2155 declare-and-assign, etc.), fix them.

## Task 6.4 — Write the unit-test helper `scripts/cc/tests/hooks/_common_helpers.sh`

```bash
#!/usr/bin/env bash
# Helpers for hook unit tests.
set -euo pipefail

assert_exit() {
  local expected="$1" actual="$2" label="$3"
  if [ "$actual" != "$expected" ]; then
    echo "FAIL [$label]: expected exit $expected, got $actual" >&2
    return 1
  fi
}

assert_jq() {
  local input="$1" expr="$2" expected="$3" label="$4"
  local actual
  if ! actual="$(echo "$input" | jq -r "$expr")"; then
    echo "FAIL [$label]: jq failed for expression $expr against input: $input" >&2
    return 1
  fi
  if [ "$actual" != "$expected" ]; then
    echo "FAIL [$label]: $expr expected '$expected', got '$actual'" >&2
    return 1
  fi
}

mktempdir() {
  mktemp -d "/tmp/cgreality-hook-test-XXXXXX"
}

setup_real_smoke_runtime_root() {
  # Creates a real runtime root by invoking the Phase 2 smoke subcommand.
  local root="$1" label="${2:-hook-test}"
  cargo build -p context-graph-cli --bin reality-loop --quiet
  ./target/debug/reality-loop smoke --runtime-root "$root" --label "$label" >/dev/null
  local summary
  summary="$(find "$root" -name attempt-summary.json -type f -print -quit)"
  test -n "$summary" || { echo "FAIL: smoke wrote no attempt-summary.json under $root" >&2; return 1; }
  dirname "$summary"
}
```

## Task 6.5 — Write the 6 hook unit tests

For each of the 6 hooks, create a test under `scripts/cc/tests/hooks/test_<hookname>.sh`. See [03_FILE_SPECS.md](03_FILE_SPECS.md) §"Hook unit tests" for full bodies. Each test:
1. Sets up a real smoke runtime root by invoking `./target/debug/reality-loop smoke`.
2. Crafts representative Claude Code hook stdin JSON.
3. Runs the hook.
4. Asserts exit code + stdout shape.
5. Cleans up.

```bash
chmod +x scripts/cc/tests/hooks/*.sh
```

## Task 6.6 — Run all unit tests

```bash
for t in scripts/cc/tests/hooks/test_*.sh; do
  echo "Running $t..."
  bash "$t" || { echo "FAIL: $t"; exit 1; }
done
echo "All hook tests passed."
```

Expected: all 6 pass.

If a test fails, debug:
```bash
bash -x scripts/cc/tests/hooks/test_<failing>.sh 2>&1 | head -50
```

## Task 6.7 — Run shellcheck on every script (if available)

```bash
command -v shellcheck >/dev/null 2>&1 || { echo "shellcheck is required for Phase 6"; exit 1; }
shellcheck scripts/cc/*.sh scripts/cc/tests/hooks/*.sh
```

If shellcheck isn't installed, install it before continuing. Phase 6 is incomplete without this check.

## Task 6.8 — Verify

Execute [04_VERIFICATION.md](04_VERIFICATION.md). All clauses pass.
