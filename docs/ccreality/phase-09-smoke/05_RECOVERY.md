# Phase 9 — Recovery

The smoke is the integration gate. Failures here usually point at issues introduced in earlier phases. Recovery is per-layer.

## Layer 0: missing tool

Install the missing tool. `claude` requires Node.js: `npm install -g @anthropic-ai/claude-code`. Re-run smoke.

## Layer 1: cargo build fail

Re-do Phase 1 / Phase 2 verifications. Likely cause: an unused-import warning that's now a compile error, or a moved function that wasn't re-exported.

## Layer 2: MCP server doesn't respond

Re-do Phase 3 / Phase 4 verifications. Common causes:
- Server prints to stdout, corrupting the protocol.
- Tools/list returns empty because `register()` isn't called.
- The `--mode reality-loop` flag isn't recognized.

## Layer 3: Claude Code can't find cgreality

- `.mcp.json` is malformed.
- `.mcp.json` references env vars that don't expand.
- `.claude/settings.json` has `enableAllProjectMcpServers: false`.
- Claude Code was launched with `--bare`.

Fix: switch `.mcp.json` to literal paths. Verify with `claude --print "/mcp"`.

## Layer 4: hooks don't fire

- `.claude/settings.json` hooks block paths are wrong.
- Hook scripts aren't executable.
- `$CLAUDE_PROJECT_DIR` isn't set when Claude Code starts (it should be).

Fix: `chmod +x scripts/cc/*.sh`; check the `command` strings in `.claude/settings.json`.

## Layer 5: /ccreality-smoke skill doesn't complete

Most common: the Stop hook blocks because no `optimizer_record_recommendation` was called. The skill prompt may be too vague, or `optimizer_record_recommendation` isn't reachable.

Debug:
```bash
claude --print "/ccreality-smoke" --model opus --max-turns 25 --output-format stream-json --verbose 2>&1 | head -100
```

Inspect tool calls. If `optimizer_record_recommendation` is never called, the skill body needs a more explicit instruction to call it.

## Layer 6: drift detection mechanism wrong

The smoke's Layer 6 is a manual SHA check. If it fails, `sha256sum` isn't producing different hashes for different content — diagnose via:
```bash
echo "a" | sha256sum
echo "b" | sha256sum
```

If hashes differ, the test is fine; check the script's git restore step.

The deeper drift FSV (full Claude session that mutates a file mid-edit) is in Phase 10 case C5.

## Layer 7: deny doesn't work

- The deny-list in `.claude/settings.json` doesn't include the file pattern.
- The PreToolUse hook also needs to deny it (defense in depth).
- Claude Code's permission resolver is buggy in this version (rare).

Test directly:
```bash
echo '{"tool_name":"Edit","tool_input":{"file_path":"/home/cabdru/contextgraph/.env"}}' | bash scripts/cc/hook_pre_tool_use_edit.sh
```

Should return `permissionDecision: deny`.

## Layer 8: schema validation doesn't reject

- The MCP server's recommendation tool isn't using the validator.
- The validator points at the wrong schema file.
- The schema is too permissive.

Check `optimizer_record_recommendation`'s implementation in `crates/context-graph-mcp/src/handlers/tools/reality_loop/optimizer.rs`.

## Total smoke fail (no layer reaches PASS)

Likely cause: `claude` isn't installed or no API key. Run `claude doctor` for a self-check.

## Diagnostic dump

```bash
cd /home/cabdru/contextgraph
echo "=== Phases ==="
cat /tmp/ccreality-phase-progress.log
echo "=== Tools ==="
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null | jq '.result.tools | length' || echo "MCP failed"
echo "=== Hooks ==="
ls -la scripts/cc/hook_*.sh
echo "=== Settings ==="
jq -r '.hooks | keys[]' .claude/settings.json
echo "=== MCP config ==="
cat .mcp.json
```
