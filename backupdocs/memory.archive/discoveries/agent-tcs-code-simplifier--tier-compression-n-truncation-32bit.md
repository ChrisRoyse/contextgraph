---
namespace: discoveries
agent_id: agent-tcs-code-simplifier
timestamp: 2026-05-08T00:00:00Z
status: active
tags: tier-compression, deserialize, n-truncation, 32-bit
---

# Header `n: u64 → as usize` truncates silently on 32-bit hosts

`crates/context-graph-tier-compression/src/lib.rs:365-366` reads the on-disk
`n_le: [u8; 8]` and converts via `u64::from_le_bytes(n_le) as usize`. On a
32-bit target an `n > u32::MAX` would silently truncate, then
`packed_len(n_truncated, bits.bits())` would later mismatch the payload size
and reject — so corruption surfaces as `INVALID_INPUT` rather than UB.

ContextGraph runs only on 64-bit targets today, but the cost of guarding is
two lines:

```rust
let n_u64 = u64::from_le_bytes(n_le);
let n = usize::try_from(n_u64).map_err(|_| TierCompressionError::invalid(
    "bytes.n",
    format!("n={} exceeds usize on this host", n_u64),
    "rebuild on a 64-bit target or reduce vector length",
))?;
```

Severity: LOW. Defense in depth, not a live bug.
