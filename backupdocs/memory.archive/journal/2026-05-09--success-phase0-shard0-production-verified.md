---
namespace: journal
created: 2026-05-09T23:22:00Z
updated: 2026-05-09T23:22:00Z
status: active
tags: phase0, corpus, shard0, swebench, fsv, production
---

# Phase 0 Shard 0 Production Verified

## What Happened

Shard 0 completed under:

```text
/tmp/contextgraph-mejepa-corpus-prod-shard0-20260509
```

Final command output:

```json
{
  "entries": 1200,
  "status": "generated",
  "index": "/tmp/contextgraph-mejepa-corpus-prod-shard0-20260509/index.json",
  "store": "/tmp/contextgraph-mejepa-corpus-prod-shard0-20260509/oracle-store"
}
```

## Source Of Truth Readback

`index.json`:

```text
complete=true
entries=1200
compile_error=150
delete_test_call=150
known_good=150
off_by_one=150
over_engineer=150
subtle_flip=150
swap_variable=150
wrong_file=150
```

SWE-bench artifact reports:

```text
known_good=150
subtle_flip=150
off_by_one=150
swap_variable=150
delete_test_call=150
wrong_file=150
over_engineer=150
compile_error=150
```

RocksDB physical column-family scan via `ldb`:

```text
mejepa_mutation_corpus=1200
mejepa_oracle_verdicts=1200
mejepa_corpus_provenance=1
```

`mejepa-corpus stats` passed with:

```text
passes=true
mutation_cf_count=1200
verdict_cf_count=1200
provenance_cf_count=1
by_oracle pass=664 fail_or_error=536
```

`mejepa-corpus verify --sample-fraction 0.001` passed and ran two real fresh
Docker oracle samples:

```text
django__django-16139|compile_error: persisted hash matched fresh hash
django__django-14411|known_good: persisted hash matched fresh hash
```

## Operational Notes

Shard 0 had to be resumed at `--oracle-workers 6` after `--oracle-workers 12`
caused Docker overlay page-cache stalls. The completed reports were preserved
and the harness skipped them correctly.

Shard 1 was started after this verification with:

```text
--skip-tasks 150 --max-tasks 150 --oracle-workers 6 --run-id-prefix mejepaPhase0ProdS1
```
