---
namespace: discoveries
agent_id: agent-03-coder-storage
timestamp: 2026-04-15T05:05:00Z
status: fixed-inline
severity: low
tags: storage, async, trait-import, get-content, anomaly-derivation
---

# `get_content` lives on the trait, not the struct — call the internal async

## Context
In `anomaly_derivation.rs` I needed to look up anchor + negative content
text for each classified edge. Initially wrote
`self.get_content(edge.source()).await?` expecting it to be a method on
`RocksDbTeleologicalStore`.

Compiler hit:
```
no method named `get_content` found for reference `&RocksDbTeleologicalStore`
help: trait `TeleologicalMemoryStore` which provides `get_content` is
      implemented but not in scope
help: there is a method `get_content_async` with a similar name
```

## Ground truth
- `trait_impl.rs` routes `TeleologicalMemoryStore::get_content` →
  `self.get_content_async(...)`.
- Inside the crate, the canonical internal call is `get_content_async`.
  Trait-based callers (mostly external) must `use context_graph_core::traits::TeleologicalMemoryStore`.

## Fix
Switched the derivation path to `self.get_content_async(id).await?`
directly. No trait import needed; the function is `pub(crate)` and already
accessible from sibling submodules.

## Lesson
When adding a new submodule inside `rocksdb_store/`, prefer the
`*_async` internal methods (content, source_metadata, …) over trait
calls. Trait-style calls require an external `use` that pollutes the
submodule and adds a layer of indirection you don't need.
