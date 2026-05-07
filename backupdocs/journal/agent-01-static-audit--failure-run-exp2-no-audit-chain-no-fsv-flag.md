---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:17:00Z
status: active
severity: high
tags: exp2, run-exp2, audit-chain, orchestrator, t-024
---

# Defect: run_exp2.py orchestrator NEVER builds the audit chain (T-024)

## Where
`scripts/exp2/run_exp2.py:17-36` (PHASE_COMMANDS)

## Spec
`docs/exp2/05_technical_spec.md` §4 data flow:
> [audit chain finalize] → audit_chain.json
> [FSV replay] → fsv_replay_log.json (matches reference)

`docs/exp2/06_atomic_tasks.md` T-024 Audit Chain Finalization. Outputs `audit_chain.json`. Listed in spec critical path.

## Observed
The orchestrator's `PHASE_COMMANDS` includes:
- environment, corpus, embeddings, pca_embeddings, projections, estimators, n_eff, visualize, fsv

It does **NOT** include:
- A `audit_chain` phase that runs `audit_chain.py` (which exists at `scripts/exp2/audit_chain.py`).
- A `replay` phase that runs `replay.py` (which exists too).

`run_exp2.py --phase all` will skip building the audit chain entirely.

The `audit_chain.py` module (T-024) exists as `class AuditChain` but is not called by any orchestrator-driven command. It must be imported and used by the producer scripts. Static check: `grep AuditChain scripts/exp2/*.py` — only declares it in `audit_chain.py`. So no other module IMPORTS or INSTANTIATES it.

## Conclusion
**The audit chain is never written.** Per Constitution C7:
> The audit chain is written *before* the data is consumed for visualization. No artefact ships without the chain.

This makes T-024 vacuously "implemented but unused". It also means `n_eff_summary.json` ships with no `audit_chain_sha256` (which spec §5f requires).

## Why this is HIGH-severity
- C7 is non-negotiable. No audit chain → no publication.
- The MCP daemon is fully wired to support hash-anchored extraction (extract_embeddings.py logs `manifest_sha256` in the npz). But the chain-builder isn't called.
- An external reviewer reading the bundle will see `audit_chain.json` is missing or empty.
- It's a 50-line patch to wire it in (single-touch fix).

## Recommended fix
1. Add `audit_chain` phase to `run_exp2.py`'s PHASE_COMMANDS.
2. Or have each producer script (`extract_embeddings`, `fit_projections`, `run_estimators`, `effective_rank`, `visualize`) construct an AuditChain and append events as it goes; finalize at end.
3. Per `08_output_deliverables.md` §8, the chain must contain ALL: artefacts (with sha256s and parent_artefact_ids), events (config_load, embedder_load, etc.), code_commit_sha, hyperparameters_sha256, embedders panel.

## Cross-check
Agent-02 should confirm by running `python3 scripts/exp2/run_exp2.py --phase n_eff` and checking that `artefacts/exp2/audit_chain.json` does NOT exist after.
