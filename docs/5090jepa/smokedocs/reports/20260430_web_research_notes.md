# 2026-04-30 Web Research Notes

## Purpose

Record implementation guidance checked against current external documentation
while hardening the research-smoke path.

## Sources Checked

- Rust `Result` documentation:
  <https://doc.rust-lang.org/core/result/>
- `tracing` crate documentation:
  <https://docs.rs/crate/tracing>
- `tracing-subscriber` documentation:
  <https://docs.rs/tracing-subscriber/>
- RocksDB project site:
  <https://rocksdb.org/>

## Applied Learnings

- Rust `Result` supports explicit fail-closed propagation instead of silent
  fallback paths. This matches the existing typed `DynamicJepaError` style.
- `tracing` supports structured events and spans, which matches the current
  error logs carrying operation, run ID, source-of-truth CF, status, error code,
  and remediation.
- `tracing-subscriber` supports formatted and JSON-oriented event collection;
  this reinforces the decision to keep command stdout JSON plus structured
  stderr logs.
- RocksDB is an embedded key-value store. The manual FSV process must avoid
  concurrent opens of the same DB path when the local binding owns an exclusive
  lock. The observed `LOCK: Resource temporarily unavailable` error is useful
  evidence that the DB refused unsafe concurrent inspection.

## Decision

No fallback behavior was added. The seed-control patch adds stricter validation
and more explicit source-of-truth metadata rather than relaxing any existing
gate.

