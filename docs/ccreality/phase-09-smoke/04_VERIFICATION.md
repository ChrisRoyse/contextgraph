# Phase 9 — Verification

```bash
cd /home/cabdru/contextgraph
bash scripts/cc/tests/smoke.sh
echo "Phase 9 verification exit=$?"
```

**Pass:** exit code 0. Last line of output: `Smoke pass.`.

## Side-effect check

After a successful smoke:
```bash
ls /tmp/cgreality-smoke-*  # at least one smoke directory exists
test -f docs/ccreality/SMOKE_RESULTS_$(date -u +%Y-%m-%d).md
```

## What to record

```bash
echo "phase-09: PASS, $(date -u +%FT%TZ), smoke artifacts in /tmp/cgreality-smoke-*" >> /tmp/ccreality-phase-progress.log
```
