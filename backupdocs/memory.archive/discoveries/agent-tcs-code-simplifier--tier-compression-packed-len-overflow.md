---
namespace: discoveries
agent_id: agent-tcs-code-simplifier
timestamp: 2026-05-08T00:00:00Z
status: active
tags: tier-compression, packed_len, overflow, defense
---

# `packed_len` can overflow `usize` for adversarial n

`crates/context-graph-tier-compression/src/lib.rs:399-401`:

```rust
pub fn packed_len(n: usize, bits: u32) -> usize {
    (n * bits as usize + 7) / 8
}
```

For embedding-sized inputs (n ≤ 768, bits ≤ 8) the product is tiny. But this
function is also called inside `deserialize` against an on-disk `n` that an
attacker (or corrupted disk) controls. With `n = usize::MAX / 4` and `bits =
8`, the multiplication overflows in release mode and silently wraps,
producing an undersized `expected` — `payload.len() < expected` would no
longer catch a malformed blob.

Two-line fix:

```rust
pub fn packed_len(n: usize, bits: u32) -> Option<usize> {
    n.checked_mul(bits as usize).and_then(|p| p.checked_add(7)).map(|p| p / 8)
}
```

…and have callers convert `None` to `TierCompressionError::invalid`.

Severity: LOW (defense). Touches every call site, so the cleanup is a
deliberate refactor, not a one-liner.
