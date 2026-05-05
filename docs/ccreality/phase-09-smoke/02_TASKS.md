# Phase 9 — Tasks

## Task 9.1 — Confirm Phases 0–8 verifications passed

```bash
cat /tmp/ccreality-phase-progress.log
```

Expected: 9 lines (phase-00 through phase-08), all PASS.

If a phase didn't pass, do not proceed; go fix that phase first.

## Task 9.2 — Write `scripts/cc/tests/smoke.sh`

Use the body from [../17_SMOKE_TEST_PLAN.md](../17_SMOKE_TEST_PLAN.md) §"Smoke replay script". Adapt to current paths.

Make executable:
```bash
chmod +x scripts/cc/tests/smoke.sh
```

## Task 9.3 — First smoke run

```bash
cd /home/cabdru/contextgraph
bash scripts/cc/tests/smoke.sh 2>&1 | tee /tmp/ccreality-smoke-run-$(date +%s).log
```

Most likely outcome on first run: one or two layers fail. Iterate:

### If Layer 0 fails:
Install missing tool. `claude` CLI: `npm install -g @anthropic-ai/claude-code`. `jq`: `apt install jq`.

### If Layer 1 fails:
The engine doesn't build. Re-do Phase 1 verification.

### If Layer 2 fails:
The MCP server doesn't respond. Re-do Phase 3 verification.

### If Layer 3 fails:
Claude Code can't find the cgreality server. Check `.mcp.json` and `.claude/settings.json` (Phase 7). Common cause: env-var expansion not working — switch to literal paths.

### If Layer 4 fails:
Hooks don't fire. Check `.claude/settings.json` `hooks` block paths. Confirm scripts are executable (`chmod +x scripts/cc/*.sh`). Run `claude` interactively and watch for hook output.

### If Layer 5 fails:
The `/ccreality-smoke` skill exists but doesn't complete. Most common: a tool referenced by the skill isn't registered, OR the Stop hook is blocking because no recommendation was recorded. Check the skill body matches the recommendation flow.

### If Layer 6 fails:
Drift detection isn't working. Re-check `reality_shift_compare_to_my_view` implementation (Phase 4 task 4.3).

### If Layer 7 fails:
The deny-list isn't being enforced. Check `.claude/settings.json` `permissions.deny` and the PreToolUse hook's path-policy check.

### If Layer 8 fails:
The recommendation schema isn't being validated. Check `RecommendationValidator::load()` is reading the right file (Phase 5).

## Task 9.4 — Record results

After the smoke passes:
```bash
DATE=$(date -u +%Y-%m-%d)
cat > docs/ccreality/SMOKE_RESULTS_${DATE}.md <<EOF
# Smoke Results — $DATE

Run-id: smoke-$(date +%s)
Operator: <name>
Branch: $(git branch --show-current)
HEAD: $(git rev-parse HEAD)

## Layers

| Layer | Result | Notes |
|-------|--------|-------|
| 0 — Environment | PASS | |
| 1 — Engine | PASS | reality-loop.rs lines: $(wc -l < crates/context-graph-cli/src/bin/reality-loop.rs) |
| 2 — MCP starts | PASS | |
| 3 — Claude Code MCP load | PASS | |
| 4 — Hooks fire | PASS | |
| 5 — /ccreality-smoke | PASS | |
| 6 — Drift detection | PASS | |
| 7 — Deny enforcement | PASS | |
| 8 — Schema validation | PASS | |

## Artifacts retained

\`\`\`
$(find /tmp/cgreality-smoke-* -type f 2>/dev/null | head -20)
\`\`\`

## Issues observed

(none, or list)
EOF
```

## Task 9.5 — Verify

[04_VERIFICATION.md](04_VERIFICATION.md). Single clause: smoke.sh exits 0.
