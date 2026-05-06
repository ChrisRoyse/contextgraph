# Smoke Results - 2026-05-05

- Run stamp: `1778037916`
- Session: `phase09-smoke-1778037916`
- Smoke root: `/tmp/cgreality-smoke-1778037916`
- Log: `/tmp/cgreality-smoke-1778037916/smoke.log`
- Report JSON: `/tmp/cgreality-smoke-1778037916/smoke-report.json`
- Source of truth: runtime files under `/tmp/cgreality-smoke-1778037916`, hook logs under the active runtime root, and BlackBox ledger `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/ledger.sqlite`.

| Layer | Status | Physical readback |
| --- | --- | --- |
| 0 - Environment | PASS | commands resolved; versions printed in /tmp/cgreality-smoke-1778037916/smoke.log |
| 1 - Reality loop | PASS | runtime root /tmp/cgreality-smoke-1778037916/layer1-runtime with attempt artifacts read back |
| 2 - MCP server | PASS | 35 callable cgreality tools listed; required tools present and full-server tools absent |
| 3 - Claude MCP registration | PASS | Claude reported cgreality Connected and invoked reality_latest_root |
| 4 - Hooks | PASS | SessionStart context, PreToolUse allow, and hook state file read back |
| 5 - Full loop | PASS | runtime, analysis, command, verify, recommendation, and ledger rows read back |
| 6 - Drift detection | PASS | external drift detected against governed shift log and probe restored |
| 7 - Deny enforcement | PASS | .env denied by settings and hook, file state unchanged |
| 8 - Invalid write rejection | PASS | schema-invalid recommendation rejected with no ledger, file, or shift mutation |

## Edge Cases

| Case | Before | After |
| --- | --- | --- |
| drift detection | sha=sha256:9d5b4f29f79f26d25eb1d3f5119eb6900362702ff26d782e8b7ac84ec7dbc68f | external_sha=sha256:4502886b505d92b4edfbe06c7dec05fcc24871a60f12a85affaee7a87653a20c; drift=external_drift_detected; restored_sha=sha256:c58cb2d4b74da0f4ce9013c65d63ec233a5882b8f848d2ed0dacc6e328a6689f |
| deny enforcement | .env_sha=sha256:04229cc57a020074e36f9bd14de3d480a2ccf7515d843d85ab85766ada6bfbda; hook_lines=0 | .env_sha=sha256:04229cc57a020074e36f9bd14de3d480a2ccf7515d843d85ab85766ada6bfbda; hook_lines=1; decision=deny |
| invalid recommendation | ledger=3; recommendations=1; shift_lines=4 | ledger=3; recommendations=1; shift_lines=4; error=CCREALITY_RECOMMENDATION_SCHEMA_INVALID |
| invalid files argument | shift_lines=4 | shift_lines=4; error=CCREALITY_ARG_FILES_NOT_ARRAY |
| over-limit recommendation | ledger=3; recommendations=1 | ledger=3; recommendations=1; error=CCREALITY_RECOMMENDATION_SCHEMA_INVALID |
| empty hook input | hook_lines=1 | hook_lines=2; decision=allow |

## Source-Of-Truth Reads

- `/home/cabdru/contextgraph/.claude/settings.json`
- `/home/cabdru/contextgraph/.claude/skills/ccreality-smoke/SKILL.md`
- `/home/cabdru/contextgraph/docs/ccreality/00_INDEX.md`
- `/tmp/cgreality-smoke-1778037916/claude-layer3.json`
- `/tmp/cgreality-smoke-1778037916/claude-mcp-list.txt`
- `/tmp/cgreality-smoke-1778037916/edge-empty-hook-input.json`
- `/tmp/cgreality-smoke-1778037916/edge-empty-invalid-files.json`
- `/tmp/cgreality-smoke-1778037916/edge-over-limit-recommendation.json`
- `/tmp/cgreality-smoke-1778037916/hook-pretool-allow.json`
- `/tmp/cgreality-smoke-1778037916/hook-pretool-deny-env.json`
- `/tmp/cgreality-smoke-1778037916/hook-session-start.json`
- `/tmp/cgreality-smoke-1778037916/layer1-runtime/cgreality-hook-events.jsonl`
- `/tmp/cgreality-smoke-1778037916/layer1-runtime/cgreality-hook-state/preedit/phase09-hook/phase09-pretool.json`
- `/tmp/cgreality-smoke-1778037916/layer1-runtime/smoke-1778037920/cgreality-smoke-phase09-layer1/smoke-attempt-1/attempt-summary.json`
- `/tmp/cgreality-smoke-1778037916/layer1-runtime/smoke-1778037920/cgreality-smoke-phase09-layer1/smoke-attempt-1/problem-reality-packet.json`
- `/tmp/cgreality-smoke-1778037916/layer1-runtime/smoke-1778037920/cgreality-smoke-phase09-layer1/smoke-attempt-1/reality-signal-packet.json`
- `/tmp/cgreality-smoke-1778037916/layer5-blackbox-run.json`
- `/tmp/cgreality-smoke-1778037916/layer5-ledger-rows.txt`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/cgreality-hook-events.jsonl`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/cgreality-shift-log/phase09-smoke-1778037916-drift.jsonl`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/ledger.sqlite`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/attempt-summary.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/commands/project-command-git-status---short.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/edits/project-diff-after-edit.patch`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/verification/project-verification-mcp/project-state-verification.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/harness-transitions/round-01.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/problem-reality-packet.json`
- `/tmp/cgreality-smoke-1778037916/layer5-skill-runtime/smoke-1778037951/cgreality-smoke-phase09-layer5/smoke-attempt-1/reality-signal-packet.json`
- `/tmp/cgreality-smoke-1778037916/layer6-apply.json`
- `/tmp/cgreality-smoke-1778037916/layer6-shift-compare.json`
- `/tmp/cgreality-smoke-1778037916/layer8-invalid-recommendation.json`
- `/tmp/cgreality-smoke-1778037916/mcp-tools-list.raw.json`

## Ledger Rows

```text
agent_run|1
tool_call|2
```
