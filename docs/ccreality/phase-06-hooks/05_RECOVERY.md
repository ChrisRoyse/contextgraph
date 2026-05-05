# Phase 6 — Recovery

## Failure: hook outputs invalid JSON

**Symptom:** `jq -e .` on hook stdout fails.

**Cause:** The bash printf pipeline corrupted the JSON, OR a `printf` inside the hook accidentally went to stdout instead of the JSON envelope.

**Fix:**
1. Run with `bash -x` to trace: `echo '{}' | bash -x scripts/cc/hook_<name>.sh 2>&1 | head -50`.
2. Check that all printfs/echos that aren't the final JSON go to `>&2`.
3. Use `jq -nc --arg ... '...'` patterns from `_common.sh::cgreality_emit_additional_context` consistently.

## Failure: PostToolUse hook hangs

**Symptom:** The hook script doesn't return; cargo check is taking forever.

**Cause:** No timeout on cargo, OR cargo is rebuilding everything because dependencies changed.

**Fix:** Confirm the hook uses `timeout 60 cargo check ...`. If 60s isn't enough on cold cache, raise to 120 — but log a warning.

## Failure: Stop hook always blocks

**Symptom:** Even after recommendation is recorded, Stop hook returns `decision: block`.

**Cause:** The grep for `mcp__cgreality__optimizer_record_recommendation` in the transcript is wrong (case sensitivity, transcript format).

**Fix:**
1. Inspect a real transcript: `cat <(claude --print --output-format json -p 'echo' | jq -r '.session_id') | xargs -I{} cat ~/.claude/transcripts/{}`.
2. Adjust the grep pattern. If MCP tool calls in transcripts use a different format, adjust.
3. Alternative: check for the recommendation file's existence under the active runtime root directly.

## Failure: hook can't find context-graph-cli

**Symptom:** `command not found` in hook.

**Cause:** PATH doesn't include `target/debug/`.

**Fix:** Reference the binary by path inside the hook:
```bash
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-/home/cabdru/contextgraph/target/debug/context-graph-cli}"
```

Or fall back through several candidates as the existing `pre_tool_use.sh` does (lines 27–44).

## Failure: hook log file not created

**Symptom:** `cgreality-hook-events.jsonl` doesn't exist after multiple hook fires.

**Cause:** Either the runtime root file is missing, OR the helper silently swallows the write error.

**Fix:** Test `cgreality_log_event` directly:
```bash
source scripts/cc/_common.sh
cgreality_log_event "test" "test payload"
ls "$(cgreality_runtime_root)/cgreality-hook-events.jsonl" 2>/dev/null
```

If runtime root isn't set (e.g., during smoke before the engine runs), the log function should be a no-op (return 0). Confirm this in the helper.

## Failure: hook permission denied

**Symptom:** `bash: scripts/cc/hook_x.sh: Permission denied`.

**Cause:** Forgot `chmod +x`.

**Fix:**
```bash
chmod +x scripts/cc/*.sh scripts/cc/tests/hooks/*.sh
```

## Failure: shellcheck SC2086 (unquoted variables)

**Symptom:** ShellCheck warns about word splitting.

**Fix:** Quote the variables:
- `echo $FOO` → `echo "$FOO"`
- `[ $X = $Y ]` → `[ "$X" = "$Y" ]`

These are real bugs (whitespace and globbing surprises). Don't ignore.

## Failure: SC2155 (declare and assign in one line)

**Symptom:** `local x=$(cmd)` triggers warning.

**Fix:**
```bash
local x
x=$(cmd)
```

The reason: `local x=$(cmd)` always exits 0 (for the local statement), masking errors from the subshell.

## Diagnostic dump

```bash
cd /home/cabdru/contextgraph
echo "=== Files ==="
ls -la scripts/cc/ scripts/cc/tests/hooks/
echo "=== Permissions ==="
stat -c '%a %n' scripts/cc/*.sh scripts/cc/tests/hooks/*.sh
echo "=== ShellCheck ==="
shellcheck scripts/cc/*.sh
echo "=== Test runs ==="
for t in scripts/cc/tests/hooks/test_*.sh; do
  echo "--- $t ---"
  bash "$t" 2>&1 | head -5 || echo "FAILED"
done
```
