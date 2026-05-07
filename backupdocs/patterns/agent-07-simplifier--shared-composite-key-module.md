---
namespace: patterns
agent_id: agent-07-simplifier
timestamp: 2026-04-14T23:55:00Z
status: active
tags: pattern, rocksdb, composite-key, deduplication
---

# Pattern: Dedicated module for composite keys shared across CFs

## Problem

Two RocksDB column families that share a composite-key shape create a
naming dependency: one CF's CRUD module ends up importing the "key" helper
from the other CF's CRUD module. The dependency direction is arbitrary and
misleading — neither CF "owns" the key layout.

Example before refactor:

```rust
// llm_validation.rs
use super::typed_edge_export::{typed_edge_record_key, TYPED_EDGE_RECORD_KEY_LEN};
```

This reads as if `llm_validation` is borrowing from `typed_edge_export`,
but they're peers.

## Solution

Extract the key helper (constructor + parser + length constant) to a
dedicated `typed_edge_keys.rs` module. Both CRUD modules import from it as
a sibling. The coupling is now explicit, and a future third CF that keys
the same way imports from the same place.

```rust
// typed_edge_keys.rs
pub fn typed_edge_record_key(source: Uuid, target: Uuid, edge_type: u8) -> [u8; 33] { ... }
pub fn parse_typed_edge_record_key(key: &[u8]) -> Option<(Uuid, Uuid, u8)> { ... }
pub(crate) const TYPED_EDGE_KEY_LEN: usize = 33;
```

## Payoff (this session)

- Removes an implicit dependency from `llm_validation.rs` on `typed_edge_export.rs`.
- Centralizes key parsing (duplicated between two CFs) into one `parse_typed_edge_record_key` function with its own unit tests.
- Future migration of the key layout (e.g. adding a byte for direction) touches exactly one module.

## When to apply

- >=2 CFs share the same composite key shape AND the same key parser.
- The key shape is non-trivial (not just a UUID) and the parsing surface is identical.

## When NOT to apply

- Single-CF use only — inline the helper where it's used.
- Keys that are the same length but semantically different (e.g. different UUID role orderings) — they coincidentally share a length but diverge on meaning.
