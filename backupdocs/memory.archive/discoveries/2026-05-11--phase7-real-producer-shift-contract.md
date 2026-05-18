---
namespace: discoveries
created: 2026-05-11T02:09:55Z
updated: 2026-05-11T02:09:55Z
status: active
severity: high
tags: phase7, shift-subscriber, mcp, fsv, source-of-truth, ccreality
---

# Phase 7 Real Producer Shift Contract

## What Happened

Phase 7 subscriber and MCP replay tests were green because fixtures included synthetic inline fields such as `before.text`, `after.text`, `subject.language`, and `verification.witness_chain_segment_hex`. Real `ccreality` producer shifts were often sparse and pointed to harness transition artifacts instead. The live transform required the synthetic shape, so real hook-driven shifts could fail during subscriber/replay even though FSV tests passed.

The hardened contract is now source-backed:

- File-edit shifts must include before text from `before.text_source_of_truth` or a harness transition/preedit state source.
- After text must come from `after.source_of_truth` or transition `after_source_path`.
- Before/after SHA values are mandatory and validated against the actual bytes.
- Witness evidence must come from the witness chain; synthetic witness fallback is rejected.
- Non-code reality shifts, such as optimizer decisions, are explicitly watermarked as `no_oracle_signal` instead of being treated as malformed code edits.

## Why It Matters

Future Phase 7 tests must use real producer-shaped records and inspect RocksDB/panel/log source-of-truth state. Synthetic inline-only shift fixtures can hide producer/consumer contract drift.

## Related

- See also: ./memory/reports/session-2026-05-11--phase7-producer-source-backed-hardening.md
- Tags: phase7, shift-contract, fsv
