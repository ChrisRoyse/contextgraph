# Phase 6 — Verification

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 6.V1 All hook scripts exist and are executable ==="
for s in _common.sh hook_session_start.sh hook_user_prompt_submit.sh hook_pre_tool_use_edit.sh hook_post_tool_use_edit.sh hook_pre_compact.sh hook_stop.sh; do
  test -x "scripts/cc/$s" || { echo "  MISSING or not executable: scripts/cc/$s"; exit 1; }
done
echo "  pass"

echo "=== 6.V2 All test scripts exist and are executable ==="
for t in _common_helpers.sh test_session_start.sh test_user_prompt_submit.sh test_pre_tool_use_edit.sh test_post_tool_use_edit.sh test_pre_compact.sh test_stop.sh; do
  test -x "scripts/cc/tests/hooks/$t" || { echo "  MISSING or not executable: scripts/cc/tests/hooks/$t"; exit 1; }
done
echo "  pass"

echo "=== 6.V3 Each hook unit test passes ==="
for t in scripts/cc/tests/hooks/test_*.sh; do
  bash "$t" >/dev/null
done
echo "  pass"

echo "=== 6.V4 Hooks emit valid JSON ==="
# Pipe a synthetic stdin to each hook; pipe output through jq
for s in hook_session_start hook_user_prompt_submit hook_pre_compact; do
  OUT="$(echo '{"session_id":"v4-test","source":"startup","prompt":"x","trigger":"auto","current_context_size":100}' | bash "scripts/cc/$s.sh" 2>/dev/null || true)"
  if [ -n "$OUT" ]; then
    echo "$OUT" | jq -e . >/dev/null || { echo "  FAIL: $s emitted non-JSON: $OUT"; exit 1; }
  fi
done
echo "  pass"

echo "=== 6.V5 PreToolUse Edit denies .env ==="
OUT="$(echo '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(echo "$OUT" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
echo "  pass"

echo "=== 6.V6 PreToolUse Edit allows benign project file ==="
OUT="$(echo '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh)"
[ "$(echo "$OUT" | jq -r '.hookSpecificOutput.permissionDecision')" = "allow" ]
echo "  pass"

echo "=== 6.V7 ShellCheck (if available) is clean ==="
if command -v shellcheck >/dev/null; then
  shellcheck scripts/cc/*.sh scripts/cc/tests/hooks/*.sh
  echo "  pass (shellcheck clean)"
else
  echo "  pass (shellcheck not installed; skipped)"
fi

echo
echo "Phase 6 verification: ALL PASS"
```

## What to record

```bash
echo "phase-06: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```
