# Phase 10 — File Specifications

## Master runner

See [02_TASKS.md](02_TASKS.md) Task 10.2 for the exact bash.

## Per-case script template

Every case follows this template:

```bash
#!/usr/bin/env bash
set -euo pipefail
CASE_NAME="$(basename "${BASH_SOURCE[0]}" .sh)"
WORK="/tmp/cgreality-fsv-${CASE_NAME}-$$"
mkdir -p "$WORK"
trap "rm -rf '$WORK'" EXIT
cd /home/cabdru/contextgraph

# --- setup ---
# (prepare known real smoke/attempt state)

# --- action ---
# (run the thing under test)

# --- assertion ---
# (assert the expected outcome with shell tests; non-zero exit = FAIL)

echo "$CASE_NAME PASS"
```

## Per-case implementations

For each case in [../18_FSV_PLAN.md](../18_FSV_PLAN.md), write a script. Below are the specifications; the agent generates the bash from the spec.

### A1 — Smoke writes consistent SHA

`scripts/cc/tests/fsv/cases/A1_smoke_sha_match.sh`:
- Setup: temp runtime root.
- Action: `reality-loop smoke --runtime-root TEMP --label A1`.
- Assert: `attempt-summary.json` exists; SHA256 of file is reproducible.

### A2 — Synthetic-passing task resolves

`scripts/cc/tests/fsv/cases/A2_passing_resolves.sh`:
- Build or reuse a real synthetic task fixture whose expected outcome is known before execution.
- Action: run the normal engine path against that fixture.
- Assert: source-of-truth artifacts show the attempt resolved. If the fixture is unavailable, Phase 10 is blocked; do not mark the case as pass or skip.

### A3 — Synthetic-failing task does not resolve

`scripts/cc/tests/fsv/cases/A3_failing_does_not_resolve.sh`:
- Use `--force-fail` smoke to mimic failure.
- Assert: exit non-zero; failure artifact exists.

### A4 — Repeated patch detected

Use a real known-answer fixture or a real the inner actor run whose expected repeated-patch signature is defined before execution. If that fixture is not available, record `BLOCKED` with the missing dependency and remediation. Do not mark the case PASS or SKIP.

### A5 — Engine crash mid-attempt

Use a real subprocess-management case that forces the engine to fail and then reads the failure artifact from disk. If that fixture is not available, record `BLOCKED` with the missing dependency and remediation. Do not mark the case PASS or SKIP.

### B1–B6 — MCP analyze cases

Each makes one MCP tool call against a known smoke runtime root and asserts the response shape.

`scripts/cc/tests/fsv/cases/B1_latest_root.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
cd /home/cabdru/contextgraph
RES=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"reality_latest_root","arguments":{}}}' \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/fsv-B1.err \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.runtime_root | type == "string"' >/dev/null
ROOT=$(echo "$RES" | jq -r '.runtime_root')
test -d "$ROOT"
echo "B1 PASS"
```

The case requires a real smoke runtime root. If no active root exists, run `reality-loop smoke` before the case; do not accept a null runtime root as pass.

### C1–C12 — MCP alter + shift cases

`scripts/cc/tests/fsv/cases/C1_open_then_apply.sh`:
- Create a governed probe file under `scripts/cc/tests/fsv/work/C1_probe.txt`.
- Open that probe file lines 1-1.
- Apply edit with the SHA → expect success.
- Verify shift_log entry exists.
- Remove the probe file during cleanup after source-of-truth readback.

`scripts/cc/tests/fsv/cases/C2_stale_sha_fails.sh`:
- Apply edit with deliberately wrong SHA.
- Assert: error_code contains SHA_MISMATCH.

`scripts/cc/tests/fsv/cases/C3_denied_path.sh`:
- Open `.env` → expect deny.

`scripts/cc/tests/fsv/cases/C4_shift_log_entry.sh`:
- After C1, query `reality_shift_log` for the session.
- Assert: at least one entry with subject.path matching.

`scripts/cc/tests/fsv/cases/C5_external_drift.sh`:
- Open file → get SHA.
- Mutate the file from outside (sed inline edit).
- Call `reality_shift_compare_to_my_view` → expect `external_drift_detected`.
- Revert.

(C6–C12 follow similar patterns; see [../18_FSV_PLAN.md](../18_FSV_PLAN.md).)

### D1–D10 — Hooks

These are largely the Phase 6 unit tests, re-run as FSV cases. Each case wraps `bash scripts/cc/tests/hooks/test_<name>.sh` as the action and asserts exit 0.

### E1–E3 — End-to-end

`E1`: full happy path — synthesize a failing attempt, run `claude --print "/ccreality-iterate"`, assert recommendation artifact.
`E2`: drift mid-edit — coordinate Claude session + external mutator.
`E3`: 3 fires of `/loop 2m` — run the real loop interval and verify three recorded firings in the source-of-truth artifacts. Do not mock the interval.

### F1–F3 — Cost and limits

`F1`: confirm `claude -p ... --max-turns 25` terminates within 25 rounds and reports total_cost_usd.
`F2`: write a hook that emits >10KB additionalContext; verify Claude Code truncates to 10KB.
`F3`: pre-create a 100MB JSON artifact; verify `reality_attempt_summary` returns a pointer not the inlined content.

## Blocked-case policy

Phase 10 is the full verification gate. Do not mark required cases as passed when their required infrastructure is missing.

If a case requires the inner actor, Docker, Claude API access, or spend budget and that dependency is unavailable, record the case as `BLOCKED` with the exact missing dependency, source-of-truth evidence checked, and remediation. A `BLOCKED` case means Phase 10 is not complete.

Skipped cases appear in the results doc as `SKIP` rather than PASS/FAIL.
