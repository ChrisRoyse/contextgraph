---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:05:00Z
status: active
severity: high
tags: shakespeare, pipeline, default-args, breakage, contract-mismatch
---

# Default ingest-log filename is inconsistent across the pipeline

## What Happened

`scripts/shakespeare_ingest.py:88` defines:

```python
ap.add_argument("--output", default="/tmp/shakespeare_ingest_log.jsonl", ...)
```

But every downstream consumer defaults to a different filename:

- `scripts/shakespeare_extract.py:71`     → `/tmp/shakespeare_ingest.jsonl`
- `scripts/shakespeare_demo_queries.py:104` → `/tmp/shakespeare_ingest.jsonl`
- `scripts/shakespeare_demo_summary.py:23`  → `/tmp/shakespeare_ingest.jsonl`

Run the pipeline with all defaults and the consumer scripts will see an empty/missing
ingest log, silently producing zero sessions, zero `mid_map` entries, and a demo
report that claims "0 ingested" — even though 1,189 rows were just written.

The user worked around this on the live run by passing explicit `--output` /
`--ingest-log` flags, but the next operator that follows the README block in
`shakespeare_demo_summary.py:243-256` will hit it head-on.

## Why It Matters

This is a silent contract break: nothing crashes, the demo summary just shows
zeroed numbers. For a customer-facing GTM script that's worse than a hard error.

## Recommended Fix (Flag Only)

Pick ONE canonical name (`/tmp/shakespeare_ingest.jsonl` is the natural choice
since 3-of-4 scripts already default to it) and change `shakespeare_ingest.py:88`
to match. Better still, define the path as a module-level constant
shared via a single helper so future scripts can `from shakespeare_paths import ...`.

## Related

- See also: `./memory/discoveries/agent-10-simplifier-shakespeare--demo-summary-verdict-field-missing.md`
- Tags: pipeline-handoff, defaults, gtm-demo
