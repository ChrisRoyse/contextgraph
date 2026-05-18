---
type: discovery
scope: phase0-corpus
status: active
timestamp: 2026-05-09T15:08:29Z
---

# Phase 0 Corpus Live Gap Root Cause

The Phase 0 corpus implementation already has parser-backed Python mutation
operators, source-backed SWE-bench patch generation, a Docker SWE-bench oracle
bridge, split helpers, and CLI scaffolding.

The remaining root cause is persistence/verification incompleteness rather than
operator behavior: mutation rows and oracle verdict rows are written separately,
there is no corpus provenance column family or finalized provenance record, and
`mejepa-corpus verify` mostly validates local file/hash consistency instead of
using provenance plus a deterministic oracle sample.

Robust fix direction:

- add `CF_MEJEPA_CORPUS_PROVENANCE`;
- write mutation+verdict pairs through one RocksDB `WriteBatch`;
- finalize and persist provenance only after all planned rows complete;
- make verify fail closed on missing/inconsistent provenance and re-run the
  SWE-bench oracle for a deterministic sample;
- keep stats read-only and source all counts from the persisted corpus state.
