---
namespace: patterns
agent_id: agent-07-simplifier
timestamp: 2026-04-14T23:55:00Z
status: active
tags: pattern, bincode, versioning, deduplication, rocksdb, storage
---

# Pattern: Shared `[version: u8][bincode]` helper for RocksDB CFs

## Problem

When two or more column families store a versioned bincode payload, the
encode/decode pair gets copy-pasted. Each copy hard-codes:

1. `bincode::serialize(...)` + `CoreError::SerializationError(format!(...))` wrap
2. A `Vec` with `version` byte prepended
3. `bincode::deserialize(...)` + error wrap
4. Empty-payload rejection
5. Version-byte mismatch rejection with a retry hint

That is ~30 identical lines per CF that drift apart over time (error
phrasing diverges, one site forgets the empty-payload check, etc.).

## Solution

Extract two generic functions into a single module — here,
`crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs`:

```rust
pub(super) fn encode_versioned<T: Serialize>(
    value: &T,
    version: u8,
    type_name: &'static str,
) -> CoreResult<Vec<u8>> { ... }

pub(super) fn decode_versioned<T: DeserializeOwned>(
    bytes: &[u8],
    expected_version: u8,
    type_name: &'static str,
    retry_hint: &'static str,
) -> CoreResult<T> { ... }
```

Per-CF wrappers stay public so the external API is unchanged:

```rust
pub fn encode_typed_edge_record(r: &TypedEdgeTrainingRecord) -> CoreResult<Vec<u8>> {
    encode_versioned(r, TYPED_EDGE_RECORD_VERSION, "TypedEdgeTrainingRecord")
}
```

## Payoff (this session)

- 62 lines of duplicated encode/decode logic collapsed to 6-line wrappers + 103-line shared module (with 3 unit tests).
- Error messages now uniformly carry the payload type name and the retry hint operators need.
- New CFs that pick up the same pattern get the exact same error taxonomy for free.

## When to apply

- Any CF whose wire layout is `[version: u8][bincode <T>]`.
- Multiple CFs sharing the same wire shape, even if they differ only in type and error message.

## When NOT to apply

- Single-CF one-offs — YAGNI; inline the two helper calls.
- Non-bincode wire formats (CBOR, MessagePack) — add a sibling module with its own name.
