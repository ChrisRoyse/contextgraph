---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:01:00Z
status: active
severity: low
tags: exp2, audit-chain, success, hash-anchored
---

# Success: audit_chain.py correctly hashes the chain and fails loud on missing git

## What's good
- `chain_sha256` (line 61) is computed via `sha256_bytes(canonical_json(chain))` over the entire chain dict before writing it. This anchors the chain itself.
- `_git_commit` (lines 65-72) raises `Exp2Error` on `subprocess` failure. No silent fallback to "unknown commit". Good per C7.
- `add_artefact` calls `sha256_file(path)` synchronously — no lazy hash; the file must exist when added. (line 42)
- `write_json` is atomic (tmp-then-replace) via `common.write_json`.

## Notes
- The schema matches `08_output_deliverables.md` §8 closely: `run_id`, `creation_timestamp`, `seed`, `code_commit_sha`, `hyperparameters_sha256`, `artefacts[]`, `events[]`. Two minor differences:
  1. spec example shows `creator: "operator"`, implementation has neither — minor.
  2. spec includes `embedders: [{"slot": "E1", ...}, ...]` separately at top level — implementation logs them in `events[]` with `event_type="embedder_load"` (caller's responsibility). That's an acceptable design choice but should be documented.
- `chain_sha256` is added to the chain dict AFTER hashing — this means the file content includes its own pre-hash hash. Expected pattern. The chain_sha256 stored in a separate auxiliary file (or referenced in another artefact's hash) verifies this.
