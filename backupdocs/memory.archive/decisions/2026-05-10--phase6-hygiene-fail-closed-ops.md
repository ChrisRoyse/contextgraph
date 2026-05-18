# Phase 6 Hygiene Uses Fail-Closed Source-of-Truth Ops

## Timestamp
2026-05-10T23:12:28Z

## Decision
Phase 6 hygiene operations must fail closed and persist inspectable evidence instead of silently falling back.

## Details
- Tier transitions write data and metadata in a RocksDB `WriteBatch`, then read both back.
- Witness compression writes archive files through temp files, syncs the file, renames, syncs the directory, then persists segment metadata and verifies archive SHA/Merkle state.
- Quota overflow deletes only evictable rows and writes sticky unrecoverable state to `CF_MEJEPA_QUOTA_STATE` when protected data prevents compliance.
- Nightly GC is serialized by a D-rooted file lock and persists a `CF_MEJEPA_GC_HISTORY` row that can be independently scanned.
- MCP and CLI paths use the same hygiene crate; they do not synthesize success from return values alone.

## Rationale
The Phase 6 docs require hygiene to prevent unbounded ME-JEPA storage growth and witness-chain bloat without hiding broken states. Explicit error kinds and source-of-truth readbacks make later failures debuggable.
