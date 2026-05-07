---
namespace: discoveries
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:32:00Z
status: active
severity: medium
tags: shakespeare, demo-summary, error-handling, parquet, narrow-except
---

# Demo summary's parquet block catches only ImportError; runtime errors crash the demo

## What Happened

`scripts/shakespeare_demo_summary.py:82-113`:

```python
if parq.exists():
    try:
        from pyarrow.parquet import ParquetFile
        pf = ParquetFile(str(parq))
        total_bytes = sum(len(rb.as_py()) for b in pf.iter_batches(batch_size=100)
                          for rb in b['record_bytes'])
        ...
        md.append(...)
    except ImportError:
        pass
```

The `except` only swallows `ImportError`. If the parquet was produced by a
different code path with a different schema (e.g., column renamed from
`record_bytes` to `payload`), or if `iter_batches` fails because the parquet
file is partially written (the demo runs while the export is still streaming),
the script will crash mid-write with a `KeyError` or `OSError` and `tasks/SHAKESPEARE_DEMO.md`
will be left empty/partial.

Worse, `b['record_bytes']` returns a pyarrow `Array`, not a list. Iterating it
yields `Scalar` objects whose `as_py()` may be `None` if the column has nulls.
`len(None)` then raises `TypeError`.

Also unrelated but adjacent: there's no validation that the schema matches the
hard-coded `(memory_id: Utf8, record_bytes: Binary)` claim on line 98.
The script simply asserts this in markdown without checking — if Phase 6 ever
adds a column, the demo lies.

## Why It Matters

This is the GTM script — it has to either produce a clean output or fail with a
useful message. Today a partial parquet or schema drift causes a half-written
markdown that the user must then debug after the demo.

## Recommended Fix (Flag Only)

```python
if parq.exists():
    try:
        from pyarrow.parquet import ParquetFile
        pf = ParquetFile(str(parq))
    except ImportError:
        pf = None
    except Exception as e:
        print(f"Skipping parquet section: {e}", file=sys.stderr)
        pf = None

    if pf is not None and {f.name for f in pf.schema_arrow} == {"memory_id", "record_bytes"}:
        ... compute total_bytes ...
        md.append(...)
    elif pf is not None:
        print(f"Parquet schema mismatch: got {[f.name for f in pf.schema_arrow]}",
              file=sys.stderr)
```

i.e. catch broadly, log clearly, validate schema before formatting.

## Related

- Tags: error-handling, fail-fast, demo, gtm
