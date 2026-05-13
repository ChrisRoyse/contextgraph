---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T00:47:52Z
status: active
tags: tier1, conformal, witness, ed25519, fail-closed
---

# Tier-1 Rank and Signature Hardening

## What Happened

Review of the most recent Tier-1 solver/witness changes found two coupled hardening issues:

- `NonconformityMeasure::InverseRank` used `1.0 / (1.0 + rank)` with rank 0 as best. That inverted the conformal nonconformity polarity: best candidates received the highest score and were excluded by `score <= threshold`.
- `context-graph-witness::sign` signed arbitrary byte slices as "segments", including empty bytes and lengths not divisible by the 73-byte witness entry size. The signature proved byte possession but did not prove the signed bytes were a valid contiguous witness segment.

## Why It Matters

Conformal prediction sets are only meaningful when larger nonconformity scores mean worse or stranger candidates. Rank-based scores must therefore increase with rank, not decrease. Witness segment signatures must bind to the witness-chain data model, otherwise a valid signature can certify bytes that are not a chain segment.

## Related

- `crates/context-graph-solver/src/conformal.rs`
- `crates/context-graph-solver/src/mmr.rs`
- `crates/context-graph-witness/src/sign.rs`
