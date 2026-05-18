---
namespace: journal
created: 2026-05-10T03:27:28Z
updated: 2026-05-10T03:27:28Z
status: active
tags: phase0, corpus, swebench, rocksdb, docker, fsv, success
---

# Phase 0 Full Corpus Normalized and Verified

## What Happened

The old merged corpus at:

```text
/tmp/contextgraph-mejepa-corpus-prod-merged-20260509
```

failed fresh verification on `sympy__sympy-13146|wrong_file` because the
canonical verdict hash included raw log-scraped exception class text.

The code now canonicalizes SWE-bench verdicts from structured `report.json` /
`tests_status` only, adds `mejepa-corpus normalize`, and adds targeted
verification via `verify --sample-key task_id|category`.

## Source Of Truth

The active Phase 0 full corpus is:

```text
/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509
```

Physical readback:

```text
index.complete=true
index.entries=2400
compile_error=300
delete_test_call=300
known_good=300
off_by_one=300
over_engineer=300
subtle_flip=300
swap_variable=300
wrong_file=300
RocksDB mejepa_mutation_corpus=2400
RocksDB mejepa_oracle_verdicts=2400
RocksDB mejepa_corpus_provenance=1
stats.passes=true
verify.passes=true
```

Final corpus SHA:

```text
f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33
```

Final Docker oracle verify sampled:

```text
django__django-13590|delete_test_call
sympy__sympy-15308|subtle_flip
matplotlib__matplotlib-23964|known_good
sympy__sympy-13146|wrong_file
```

All four matched persisted verdict hashes.

## Edge Cases

Manual edge checks passed:

- `verify --sample-key invalid-format` fails closed and leaves index/evidence hashes unchanged.
- `verify --sample-key 'sympy__sympy-13146|not_a_category'` fails closed and leaves index/evidence hashes unchanged.
- `normalize --output` pointing at the existing normalized corpus fails closed before writing because the output directory is nonempty.

## Docker Storage

Docker Desktop still stores its WSL data on D:

```text
CustomWslDistroDir="D:\\contextgraph\\docker-desktop-data"
DiskSizeMiB=2097152
/mnt/d/contextgraph/docker-desktop-data/disk/docker_data.vhdx exists
300 swebench/sweb.eval.* images present
```

One stale exited SWE-bench container was removed. The 300 evaluator images were
kept intentionally because they are the reusable cache for future targeted
verification and Phase 1/Phase 2 work.
