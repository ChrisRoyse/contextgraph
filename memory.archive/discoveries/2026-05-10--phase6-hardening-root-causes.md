---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-10T23:44:59Z
status: active
tags: mejepa, phase6, hardening, fsv, root-cause, witness, quota
---

# Phase 6 Hardening Root Causes

## What Happened

The Phase 6 hardening pass found issues that were deeper than scanner cleanup:

- Witness compression updated only the immediately following row after replacing a segment with a compressed Merkle entry. For exact multiples of the segment size this was masked, but a 1500-row chain with a 1024-row segment would leave trailing rows with stale `prev_hash` values unless every later row was re-chained.
- Quota eviction treated rows without hygiene metadata as evictable with `gold=false`. That could silently delete unknown source-of-truth rows and, for witness CFs, could destroy evidentiary chain data.
- The declared 115 GiB total quota did not match the sum of category default budgets. Category budgets summed to 116.5 GiB, allowing every category to be individually under budget while the system exceeded the declared total.
- Phase 6 directly used `bincode 1.3.3` for its own persisted CF records. `cargo audit` reports that crate as unmaintained. The rest of the workspace still has bincode through other crates, but Phase 6 no longer needs to add to that exposure.

## Why It Matters

These issues were root-cause hardening issues, not cosmetic bugs:

- The witness chain is an integrity artifact; partial rechain means the system can produce a compressed state that looks successful but later fails verification.
- Quota enforcement must not reclaim data whose ownership/protection state is unknown.
- Quota budgets need to be mathematically aligned with the actual configured quota.
- New Phase 6 storage should avoid avoidable supply-chain risk.

## Related

- `crates/context-graph-mejepa-hygiene/src/witness_compress.rs`
- `crates/context-graph-mejepa-hygiene/src/quota.rs`
- `crates/context-graph-mejepa-hygiene/src/categories.rs`
- `crates/context-graph-mejepa-hygiene/src/storage.rs`
