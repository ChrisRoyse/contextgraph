---
namespace: decisions
agent_id: agent-03-coder-storage
timestamp: 2026-04-15T05:15:00Z
status: active
tags: storage, key-layout, dry, llm-validation, typed-edge-export
---

# Shared 33-byte composite key helper between typed_edge_export and llm_validation

## Decision
`typed_edge_record_key(source, target, edge_type) -> [u8; 33]` is defined
ONCE in `typed_edge_export.rs` and imported by `llm_validation.rs` via
`use super::typed_edge_export::{typed_edge_record_key, TYPED_EDGE_RECORD_KEY_LEN};`.

Alternative was duplicating the helper. Rejected because:
- The two CFs (`CF_TYPED_EDGE_RECORDS` + `CF_TYPED_EDGE_VALIDATIONS`)
  **must** share the exact same byte layout so a downstream join can
  locate the validation row for any exported record by reusing the same
  key bytes. Duplication is a correctness risk, not a maintenance risk.
- The spec explicitly says "Reuse the same 33-byte key helper (import
  `typed_edge_record_key` via `super::typed_edge_export::…` OR duplicate
  it — preference: import to avoid divergence)".

## Consequence
- `typed_edge_export.rs` exports `typed_edge_record_key` and
  `TYPED_EDGE_RECORD_KEY_LEN` as `pub` (for re-export) and `pub(crate)`
  respectively.
- `rocksdb_store/mod.rs` re-exports `typed_edge_record_key` alongside the
  encode/decode helpers so external callers (MCP handlers in Phase 4) can
  construct keys without depending on internal module paths.
- Any future bump of the key layout (e.g. adding a flag byte) happens in
  exactly one place and both CRUD layers pick it up automatically.
