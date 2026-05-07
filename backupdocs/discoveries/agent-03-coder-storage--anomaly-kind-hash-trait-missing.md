---
namespace: discoveries
agent_id: agent-03-coder-storage
timestamp: 2026-04-15T05:00:00Z
status: fixed-inline
severity: low
tags: storage, anomaly-kind, trait-derive, phase-3, hashmap
---

# AnomalyKind missing `Hash` derive blocked `HashMap<AnomalyKind, usize>`

## Context
`AnomalyDerivationSummary` needs `per_kind_counts: HashMap<AnomalyKind, usize>`
per the Phase-3 spec. `AnomalyKind` in
`crates/context-graph-core/src/contrastive/types.rs` already derives
`PartialEq, Eq, Serialize, Deserialize, Clone, Copy` but NOT `Hash`, so
`HashMap<AnomalyKind, _>` won't compile.

## Fix
Added `Hash` to the derive list on `AnomalyKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnomalyKind { ... }
```

This is a one-line additive change to a Phase-2 type. Safe because:
- Enum derive for `Hash` is canonical and symmetric with `Eq`.
- No on-disk encoding uses the auto-derived hasher (bincode serializes by
  variant index, unaffected).
- `as_u8()` / `from_u8()` discriminant contract is untouched.

## Lesson
Even "done" core types may lack derives that Phase-3 HashMap keys need.
Phase-3 author: if you hit `error[E0599] the method 'entry' exists for
struct 'HashMap<X, usize>', but its trait bounds were not satisfied` — check
whether `X: Hash` is actually derived, not just assumed.
