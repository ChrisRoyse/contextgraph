# Phase 9 — Verification

```bash
cd /home/cabdru/contextgraph
bash scripts/cc/tests/smoke.sh
echo "Phase 9 verification exit=$?"
```

**Pass:** exit code 0. Last line of output: `Smoke pass.`.

Latest verified run:

- Smoke root: `/tmp/cgreality-smoke-1778013189`
- Log: `/tmp/cgreality-smoke-1778013189/smoke.log`
- Report JSON: `/tmp/cgreality-smoke-1778013189/smoke-report.json`
- Results doc: [../SMOKE_RESULTS_2026-05-05.md](../SMOKE_RESULTS_2026-05-05.md)
- Ledger readback: `agent_run|1`, `tool_call|2` from `/tmp/cgreality-smoke-1778013189/layer5-skill-runtime/ledger.sqlite`

## Side-effect check

After a successful smoke:
```bash
ls /tmp/cgreality-smoke-*  # at least one smoke directory exists
test -f docs/ccreality/SMOKE_RESULTS_$(date -u +%Y-%m-%d).md
jq -e '.status == "pass" and .layer_count == 9 and .edge_count >= 3' /tmp/cgreality-smoke-1778013189/smoke-report.json
sqlite3 /tmp/cgreality-smoke-1778013189/layer5-skill-runtime/ledger.sqlite \
  "select record_kind, count(*) from ledger_records group by record_kind order by record_kind;"
```

## What to record

```bash
echo "phase-09: PASS, $(date -u +%FT%TZ), smoke artifacts in /tmp/cgreality-smoke-*" >> /tmp/ccreality-phase-progress.log
```
