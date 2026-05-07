---
namespace: blockers
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:20:30Z
status: active
severity: high
tags: exp2, blocker, audit-chain, pair-rows, provenance, c7
---

# BLOCKER: per-pair JSON rows missing 8 provenance fields specified in 03_methodology.md §13

## Summary
`run_estimators.run_pair` writes pair rows that omit:
- 4 hashes (embedder weights × 2, pca × 2)
- corpus_manifest_sha256
- redundancy (the redundancy-adjusted N_eff input)
- code_commit_sha
- hyperparams_sha256

## Severity HIGH
- C7 (audit chain first) is non-negotiable.
- Every per-pair row is supposed to be a "unit of replicable evidence" (methodology §13).
- 8 missing fields = an external reviewer can't trace this row back to corpus, weights, code, or hyperparams.

## Block what?
- T-024 audit chain finalization.
- Open-source release (would publish unprovable rows).
- Independent replication: a peer can't verify they're using the right inputs.

## Detection
`grep -E "weight_sha256|pca_.*_sha256|corpus_manifest_sha256|hyperparams_sha256" scripts/exp2/run_estimators.py` — no matches.

## Fix priority
Tier 1. Coupled to the AuditChain wiring blocker. Both fixes are needed before the production run; otherwise re-running is required.

See: agent-01-static-audit--failure-pair-row-no-redundancy-no-pca-hashes-no-corpus-hash.md for full details.
