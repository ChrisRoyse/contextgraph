---
namespace: discoveries
agent_id: agent-09-simplifier-v2
timestamp: 2026-04-14T17:30:00Z
status: deferred
tags: constellation, atomic-swap, observability, nit
severity: nit
topic: atomic-swap-guard-silently-repairs-corrupt-index-bytes
---

# Atomic-swap probe silently treats a corrupt index value as "no prior entry"

## Scope
`crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs:82-103`

## Observation
The probe that decides whether the prior primary row needs deleting is:

```rust
let prior_id: Option<[u8; 16]> = match self.db.get_cf(index_cf, &index_key) {
    Ok(Some(bytes)) if bytes.len() == 16 && bytes.as_slice() != new_id_bytes => {
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&bytes);
        Some(buf)
    }
    Ok(_) => None,                    // <-- also swallows bytes.len() != 16
    Err(e) => { ... }
};
```

The two guard conditions do different jobs (both are correct and non-redundant):
- `bytes.len() == 16` — format sanity.
- `bytes.as_slice() != new_id_bytes` — skips the degenerate "re-store same UUID" case so the batch doesn't emit a self-cancelling `delete+put` on the same key.

However, when `bytes.len() != 16` (i.e. the secondary-index value is malformed) the probe falls into `Ok(_) => None` — the overwrite path later replaces the bad bytes with a well-formed UUID. That is effectively self-healing and safe, but it silently erases evidence of a real invariant violation (a 17-byte key with a non-16-byte payload). `find_constellation_by_selector` already treats the same condition as a hard `SerializationError`; `store_constellation` treats it as a no-op. That inconsistency is what matters.

## Severity
**nit** — no incorrect data is produced; only the fault-detection surface shrinks by one callsite.

## Minimal diff (if applied)
```rust
Ok(Some(bytes)) if bytes.len() == 16 && bytes.as_slice() != new_id_bytes => { ... }
Ok(Some(bytes)) => {
    warn!(
        len = bytes.len(),
        new_id = %c.id,
        "CF_CONSTELLATION_BY_SELECTOR had malformed value — overwriting during store_constellation"
    );
    None
}
Ok(None) => None,
Err(e) => { ... }
```

## Verdict
Not worth fixing in this pass. The invariant violation is only reachable via out-of-band DB tampering, and the existing `error!` log on the read side already catches it when any selector-lookup path touches the row.
