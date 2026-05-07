---
namespace: journal
agent_id: agent-02-tester
timestamp: 2026-04-28T23:52:00Z
status: active
severity: high
tags: exp2, blocker-verified, b2, audit-chain, c7
---

# VERIFIED (refined): B-2 — AuditChain IS instantiated once (verify_environment.py:98) but NEVER finalized; orchestrator does not run a chain phase

## Full grep across scripts/exp2/
```
$ grep -n "AuditChain\|audit_chain" scripts/exp2/*.py
scripts/exp2/audit_chain.py:12:class AuditChain:                                    # definition
scripts/exp2/verify_environment.py:15:from scripts.exp2.audit_chain import AuditChain  # import
scripts/exp2/verify_environment.py:98:    chain = AuditChain(cfg["run_id"], out / "audit_chain_environment.json", cfg["run"]["random_seed"])  # one instantiation
scripts/exp2/write_distribution.py:38: ... cite ... `artefacts/exp2/audit_chain.json`. # docstring text
```

## Refinement vs agent-01 claim
Agent-01 (blocker file line 22-25) wrote:
> "scripts/exp2/audit_chain.py:1: ..."
> "scripts/exp2/audit_chain.py:12:class AuditChain:"
> "No imports. No instantiations. No callers."

This is **partially incorrect**: there IS exactly one import + one instantiation in `verify_environment.py:15,98` producing a phase-local file `audit_chain_environment.json`. Agent-01 missed this.

However the substantive claim is **still correct**:
1. `run_exp2.py` (orchestrator) PHASE_COMMANDS dict has no `audit_chain` phase.
2. The single `verify_environment.py` instantiation produces only the *environment-phase* sub-chain; no phase calls `chain.add_artefact(...)` for embeddings/projections/estimators/n_eff.
3. No call to `chain.finalize(...)` to produce a master `audit_chain.json` with `chain_sha256`.
4. `write_distribution.py:38` references `artefacts/exp2/audit_chain.json` as a deliverable, but no producer.

## Conclusion
B-2 stands as HIGH severity but with refined wording: "AuditChain has a stub instantiation in verify_environment.py that produces an environment-only sub-chain; no master chain is built across phases, no `finalize()` is called, and the spec deliverable `audit_chain.json` is never written." Constitution C7 violation upheld.

## Test path
This is purely static. No runtime test changes the verdict.
