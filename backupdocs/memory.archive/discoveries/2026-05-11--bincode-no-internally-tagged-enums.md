---
namespace: discoveries
agent_id: agent-claude-opus-4.7
created: 2026-05-11T16:46:00Z
updated: 2026-05-11T16:46:00Z
status: active
tags: bincode, serde, rocksdb, enum, persistence, gotcha
---

# bincode rejects internally-tagged enums — use default external tagging for any enum that flows through RocksDB

## What I found

Any new `enum` that gets persisted through `RocksDbEvalStore`, `HealRocksStore`, or other bincode-backed stores MUST use either:

- **default external tagging** (no `#[serde(tag = "...")]` attribute) — emits `{"VariantName": {...}}`, or
- **adjacent tagging** (`#[serde(tag = "kind", content = "data")]`) — emits `{"kind": "variant_name", "data": {...}}`

It MUST NOT use **internally-tagged** representation (`#[serde(tag = "type")]`).

bincode's `Deserializer` does not implement `serde::Deserializer::deserialize_any`. Internally-tagged enums need that method to inspect the tag field before dispatching to the matching variant. The compile passes, JSON round-trip works, but the moment the enum hits a bincode write+read cycle, it errors with:

```
MEJEPA_EVAL_STORE: Bincode does not support the serde::Deserializer::deserialize_any method
```

I hit this on TASK-FLYWHEEL-005 with `ActiveLearningKind`. The unit tests passed (they used `serde_json` directly), but the FSV example using `RocksDbEvalStore::persist_queue` errored. Fix was a one-character delete: drop `tag = "type"` from the `#[serde]` attribute.

## Why It Matters

Two classes of future failure this prevents:

1. **A `cargo test` that exercises serde via JSON will pass while the same enum routed through bincode panics at runtime.** This is exactly the FSV-PROTOCOL §10 "tests pass when the project is in a broken state" trap.
2. **The error message names `deserialize_any` not the offending enum** — without this discovery, the next agent burns time tracing into bincode internals.

The general rule for this codebase: any new enum that goes into `mejepa::types`, `mejepa::eval`, `mejepa::heal`, `mejepa-corpus::oracle`, `mejepa-hygiene::*`, or any other crate whose `Storage::open` uses bincode, must round-trip through a real store before being declared done — not just JSON serde tests.

## Related

- See also: ./memory/reports/session-2026-05-11--task-flywheel-005-active-learning-surprise.md
- Code: `crates/context-graph-mejepa/src/eval/queue.rs` (the `ActiveLearningKind` enum demonstrates the correct external tagging)
- Code: `crates/context-graph-mejepa/src/eval/store.rs` (the bincode call site)
- FSV protocol: `docs/futurebuild/specs/FSV-PROTOCOL.md` §10 (this is exactly the failure mode it warns about)
- Tags: bincode, serde, persistence, internally-tagged, enum
