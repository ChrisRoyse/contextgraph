---
namespace: decisions
agent_id: codex
timestamp: 2026-05-10T23:44:59Z
status: active
tags: mejepa, phase6, quota, witness, fail-closed, policy
---

# Phase 6 Quota and Witness Hardening Policy

## Decision

Phase 6 now treats witness-chain data and unknown rows as protected by default.

## Details

- Witness compression must re-chain every later live witness row after a compressed segment replacement, and must update compressed segment metadata when a later compressed entry's chain hash changes.
- Witness segment metadata is valid only when its archive path equals the deterministic path under `archive_root/witness_segments`, the entry count matches byte length, and first/last keys match the segment range.
- Quota eviction may delete only rows with valid hygiene metadata in `CF_MEJEPA_PANEL_META`.
- `StorageCategory::WitnessChains` is not quota-evictable; if witness storage exceeds budget after compression, Phase 6 records sticky unrecoverable quota state instead of deleting evidence.
- Quota category budgets are normalized by the actual sum of default category weights, not by the nominal total quota constant.
- Phase 6-owned CF metadata/report values are serialized as JSON via `serde_json` rather than direct `bincode`.

## Rationale

Fail-closed hygiene should preserve source-of-truth integrity. It is better to stop with `MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE` or `MEJEPA_HYGIENE_WITNESS_ARCHIVE_INVALID` than to silently delete unknown rows or accept malformed archive metadata.
