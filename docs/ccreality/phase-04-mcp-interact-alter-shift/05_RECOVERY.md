# Phase 4 — Recovery

## Failure: Lock not released after a panic

**Symptom:** `edit_lock` file persists after a crashed test or aborted tool call.

**Cause:** The `Drop` impl on `EditLock` runs on normal returns and unwinding panics, but not on abort or signal-kill.

**Fix:** Manually `rm ~/.cache/cgreality/edit_lock`. The `try_acquire` code also auto-removes locks older than `STALE_LOCK_AGE_SECS` (1 hour by default), so the lock will self-clear over time.

For more robust handling, consider switching to `fs2::FileExt::try_lock_exclusive` which uses kernel-level POSIX locks released on process exit.

## Failure: shift record corrupted (one line is invalid JSON)

**Symptom:** `reality_shift_log` skips the bad line; tool list shows fewer shifts than expected.

**Cause:** A failed `append_shift` left a partial line, or a concurrent writer interleaved.

**Fix:** Concurrent writers shouldn't happen if the single-writer lock is enforced (which is the whole point of the lock). If shift-log lines are getting corrupted, audit:
1. Is `append_shift` being called inside the `EditLock` scope?
2. Are we using `OpenOptions::append(true)` (atomic on POSIX for small lines)?

Manual cleanup:
```bash
RUNTIME=$(cat ~/.cache/cgreality/active_runtime_root)
SESS=<session_id>
# Filter to only lines that parse as JSON
jq -c '.' <"$RUNTIME/cgreality-shift-log/$SESS.jsonl" >"$RUNTIME/cgreality-shift-log/$SESS.jsonl.cleaned" 2>/dev/null
mv "$RUNTIME/cgreality-shift-log/$SESS.jsonl.cleaned" "$RUNTIME/cgreality-shift-log/$SESS.jsonl"
```

## Failure: jsonschema crate version mismatch

**Symptom:** `cargo build` fails with version-resolution errors.

**Fix:** Check `Cargo.lock` for `jsonschema` version. If a different version is already in the workspace, align to that. Adjust `crates/context-graph-mcp/Cargo.toml`:
```toml
jsonschema = { workspace = true }  # if a workspace dependency exists
# OR
jsonschema = "0.17"  # match the version in Cargo.lock
```

## Failure: `optimizer_record_recommendation` rejects valid payload

**Symptom:** Recommendation that should pass schema validation returns `CCREALITY_RECOMMENDATION_SCHEMA_INVALID`.

**Cause:** The schema in `config/ccreality_recommendation_schema.json` is wrong, OR the validator path resolution is wrong.

**Fix:**
1. Validate the schema itself: `jq -e . config/ccreality_recommendation_schema.json`.
2. Test the schema against the worked example in [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md):
   ```bash
   cat config/ccreality_recommendation_schema.json | jq . > /dev/null
   # Use https://www.jsonschemavalidator.net/ or python jsonschema CLI to test
   ```
3. Ensure `CGREALITY_RECOMMENDATION_SCHEMA` env var points at the right file (set in `.claude/settings.json`).

## Failure: `harness_run_command` allows a denied command

**Symptom:** `cargo publish` is allowed.

**Cause:** Allowlist regex too permissive, OR `governed_edit::run_governed_command` has a bug.

**Fix:** Audit the allowlist in `governed_edit::run_governed_command`. The expected regex set:
- `^cargo (fmt|check|test|build|clippy)( |$)` — note `|fmt|` doesn't include `publish`.
- `^rustfmt( |$)`
- `^git (status|diff|log|show|rev-parse)( |$)` — no `push`, `reset`, etc.

Test:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"harness_run_command","arguments":{"command":"cargo publish"}}}' | ./target/debug/context-graph-mcp --mode reality-loop | jq '.result.content[0].text' | jq fromjson
```
Expected: `error_code` containing "DENIED".

## Failure: drift detection false positive

**Symptom:** `reality_shift_compare_to_my_view` reports `external_drift_detected` for a file the session DID edit.

**Cause:** Either:
1. The shift record's `subject.path` field doesn't match the path the caller asked about (different relative/absolute representation).
2. The shift record didn't get written (silently failed `append_shift`).

**Fix:**
1. Normalize paths consistently — always use project-relative paths (no leading `./`, no absolute) in both shift records and `reality_shift_compare_to_my_view` queries. Define a `normalize_project_path` helper in `helpers.rs`.
2. Check the JSONL file directly: `tail ~/.cache/cgreality/$RUNTIME/cgreality-shift-log/$SESSION.jsonl`.

## Failure: `reality_run_attempt` times out

**Symptom:** Tool returns `CCREALITY_INTERACT_TIMEOUT`.

**Cause:** Default timeout (1800s) wasn't enough. SWE-bench attempts can take longer with Docker pulls.

**Fix:** Make timeout configurable via tool args:
```rust
let timeout_secs = args.get("timeout_secs").and_then(Value::as_u64).unwrap_or(1800);
```

Operator can pass higher value when needed.

## Diagnostic dump

```bash
cd /home/cabdru/contextgraph
echo "=== Build ==="; cargo build -p context-graph-mcp 2>&1 | tail -10
echo "=== Tests ==="; cargo test -p context-graph-mcp --lib reality_loop --quiet 2>&1 | tail -10
echo "=== tools/list ==="
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase04-recovery-tools.err | jq -r '.result.tools[].name'
echo "=== Lock state ==="
if [ -e ~/.cache/cgreality/edit_lock ]; then
  ls -la ~/.cache/cgreality/edit_lock
else
  echo "no lock file present"
fi
echo "=== Active runtime ==="; cat ~/.cache/cgreality/active_runtime_root 2>/dev/null
echo "=== Shift logs ==="; ls -la "$(cat ~/.cache/cgreality/active_runtime_root)/cgreality-shift-log/" 2>/dev/null
```
