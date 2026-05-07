---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:12:00Z
status: active
severity: low
tags: exp2, manual-fsv, source-of-truth, edge-cases, success
---

# Success: manual_fsv.py demonstrates real source-of-truth verification with edge cases

## What's good
- Three real happy-path inputs are stored, fingerprints retrieved, training records exported, all 14 embedders verified.
- Edge cases test: empty batch, whitespace content, invalid UUID — each must FAIL (raising) without mutating CF_FINGERPRINTS / CF_TRAINING_RECORDS state. (lines 107-120)
- Snapshot-before / snapshot-after comparison (line 118) catches state drift from invalid inputs.
- `expect_error(action)` (lines 36-41) flips the assertion: the action must raise; if it succeeds, that's a defect.
- Per-record shape validation: e1_len=1024, e7_len=1536, e14_len=1024, e6_sparse_nnz>0, e12_token_count>0. (lines 96-102)

## Notes
- Hardcoded `HAPPY_INPUTS` (line 19) — fine for a manual canary; not part of the production corpus.
- `snapshot` accesses keys with both camelCase and snake_case fallbacks (`fingerprintCount` vs `total_fingerprints`). (lines 30-32) Defensive against MCP API drift; OK.
- Per-RocksDB session `storage = out / f"manual_fsv_rocksdb_{session_id}"` (line 52) — temp DB; doesn't pollute production state. Compliant with C18 (artefacts under artefacts/exp2/).

## C5 (read source of truth, not return values)
The implementation:
1. Reads back fingerprints (CF_FINGERPRINTS via get_memory_fingerprint).
2. Reads back training records (CF_TRAINING_RECORDS via get_training_record).
3. Snapshots `count_training_records` for state-mutation evidence.

Compliant.

## Issue: snapshot key fallback hides daemon API change
`status["fingerprintCount"] if "fingerprintCount" in status else status.get("total_fingerprints")` — if neither key exists, returns None silently. C2 (fail loud) would prefer raising on missing key. The current code can return `{"total_fingerprints": None}` and `before == after` of `{"total_fingerprints": None, "embedder_count": None, "training_records": None}` would compare equal even if state actually drifted. Low severity; the real signals are caught by `store_memories`/`get_training_record` count assertions.
