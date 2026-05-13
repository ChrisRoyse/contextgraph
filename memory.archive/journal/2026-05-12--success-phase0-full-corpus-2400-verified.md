---
namespace: journal
created: 2026-05-12T04:39:01Z
updated: 2026-05-12T04:39:01Z
status: active
tags: mejepa, corpus, phase0, swebench, verification, production
---

# Success: Phase0 Full Corpus 2400 Verified

## What Happened
Resumed the production Phase0 SWE-bench Lite ME-JEPA corpus at:

`/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511`

The hardened generator completed the remaining categories and exited successfully:

- `wrong_file`: 300 rows
- `over_engineer`: 300 rows
- `compile_error`: 300 rows
- Total corpus: 2400 rows, 300 tasks x 8 mutation categories

The run used `--oracle-workers 6`, matching the host discovery that higher Docker Desktop + WSL2 concurrency can stall the overlay layer even on a 30 CPU machine.

## Verification Evidence
Source of truth:

- Index: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json`
- Store: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/oracle-store`
- Evidence: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json`

Final physical readback:

- Index entries: 2400
- Patch files: 2400
- Missing patch files: 0
- Orphan patch files: 0
- RocksDB `mejepa_mutation_corpus`: 2400
- RocksDB `mejepa_oracle_verdicts`: 2400
- RocksDB `mejepa_corpus_provenance`: 1
- Verify evidence `passes`: true
- Fresh oracle replay samples: 20
- Fresh oracle hash mismatches: 0
- Active corpus/verify containers after completion: 0

## Why It Matters
The corpus is now complete enough for Phase0 training/evaluation handoff. The prior DrvFs/9P durable patch write failure did not recur after moving parallel patch generation off the durable write path and resuming through the hardened generator.

One operational nuance: during final post-oracle persistence, `index.json` can expose partial final-category progress before the process exits. Treat a running generator as the writer of record. Treat the corpus as complete only after generator exit plus `verify-evidence.json` and RocksDB/index/patch readback all agree.

## Related
- See also: `./memory/journal/2026-05-12--success-mejepa-corpus-write-hardening-fsv.md`
- See also: `./memory/reports/session-2026-05-12--phase0-full-corpus-2400-verified.md`
- Tags: mejepa, phase0, corpus, swebench, oracle
