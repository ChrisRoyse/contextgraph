# Phase 9 — Tasks

## Task 9.1 — Confirm Phases 0–8 verifications passed

```bash
cat /tmp/ccreality-phase-progress.log
```

Expected: 9 lines (phase-00 through phase-08), all PASS.

If a phase didn't pass, do not proceed; go fix that phase first.

## Task 9.2 — Write `scripts/cc/tests/smoke.sh`

Implement the deterministic harness in `scripts/cc/tests/smoke.sh`. It must run direct MCP JSON-RPC calls, invoke hooks with real stdin, inspect returned `file:` source-of-truth artifacts, inspect the BlackBox SQLite `ledger_records` table, simulate drift on a restored tracked fixture, verify deny enforcement without attempting a real `.env` edit, reject malformed recommendations, and write `docs/ccreality/SMOKE_RESULTS_<date>.md`.

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

The script auto-generates `docs/ccreality/SMOKE_RESULTS_<date>.md` from the actual run stamp, runtime root, layer results, edge-case before/after state, source-of-truth reads, and ledger rows. Do not hand-write a smoke result that was not produced by the harness.

## Task 9.5 — Verify

[04_VERIFICATION.md](04_VERIFICATION.md). Single clause: smoke.sh exits 0.
