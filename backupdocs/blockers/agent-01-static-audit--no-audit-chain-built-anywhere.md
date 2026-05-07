---
namespace: blockers
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:17:30Z
status: active
severity: high
tags: exp2, audit-chain, blocker, c7-violation
---

# BLOCKER: AuditChain class is defined but never instantiated/called by any pipeline phase

## What
`scripts/exp2/audit_chain.py` defines `class AuditChain` per the spec. But no other module imports or uses it. The orchestrator `run_exp2.py` doesn't call any audit-chain phase. Therefore `artefacts/exp2/audit_chain.json` will never be produced by `run_exp2.py --phase all`.

## Why this blocks the experiment
- Constitution C7: "The audit chain is written before the data is consumed for visualization. No artefact ships without the chain."
- `n_eff_summary.json` schema (per `05_technical_spec.md` §5f) includes `"audit_chain_sha256"` — the implementation cannot produce this hash.
- `08_output_deliverables.md` §8 makes the audit chain a first-class deliverable with `code_commit_sha`, every weight hash, every event.

## Detection
```
$ grep -rn "AuditChain\|audit_chain" scripts/exp2/ --include="*.py"
scripts/exp2/audit_chain.py:1:"""Audit-chain writer for EXP-2 artefacts."""
scripts/exp2/audit_chain.py:12:class AuditChain:
```

No imports. No instantiations. No callers.

## Severity
HIGH. Without the audit chain, the harness violates C7. Publication blocked.

## Fix priority
First. Should be fixed before any production run because the chain depends on the *original* run's hash provenance — running first then bolting on the chain destroys the value of the chain (it would just hash the post-hoc artefacts, not their construction events).

## Recommended fix
Wire AuditChain into the pipeline:
1. `run_exp2.py` constructs an AuditChain at the start, persists it to disk after each phase.
2. Each phase's script accepts a `--audit-chain` path and appends events when it runs.
3. Final phase calls `chain.finalize()` and writes `audit_chain.json` with the chain_sha256.
