---
namespace: decisions
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:35:00Z
status: applied
severity: high
tags: exp2, audit-chain, c7, b2, fix
---

# R2 — Wire AuditChain into the orchestrator

## Decision

`scripts/exp2/run_exp2.py` now finalizes a master `audit_chain.json` at the end
of every run. Two phases (`estimators`, `n_eff`) accept `--audit-chain <path>`
and write per-phase chains; `verify_environment` already wrote
`audit_chain_environment.json`. The orchestrator merges all per-phase chains
plus headline deliverables into the master chain.

## Rationale (root-cause, minimal-impact)

agent-01's blocker B-2 + agent-02's refinement: AuditChain was defined but the
orchestrator never invoked it, so `artefacts/exp2/audit_chain.json` could never
be produced. Constitution C7 ("audit chain before viz; no artefact ships
without the chain") was therefore unenforceable.

Minimal-impact strategy chosen:

1. The orchestrator passes `--audit-chain <out>/audit_chain_<phase>.json` to
   audit-aware phase scripts (currently estimators, n_eff). Each phase appends
   events + artefacts and finalizes its own per-phase chain.
2. The orchestrator's new `finalize_master_audit_chain(config_path)` is called
   AFTER all phases finish (or fail), even on a partial run. It:
   - Hashes each per-phase chain file as `phase_chain:<phase>` artefact.
   - Hashes 11 headline deliverables (manifest, verification log, embeddings,
     projections, entropy, n_eff_summary, heatmaps, per_pair_table).
   - Hashes every `pair_results/E*_E*.json` row.
   - Calls `chain.finalize(sha256_file(config_yaml))` to compute `chain_sha256`.
3. Missing artefacts surface as `artefact_missing` / `phase_chain_missing`
   events — never crashing finalize. This honors agent-01's "spec internal
   inconsistency / honesty discipline": the chain *records* what's missing, it
   doesn't fabricate hashes.

## Why this is the right minimal pattern

- Matches agent-01's discovery `agent-01-static-audit--orchestrator-uses-subprocess-not-imports.md`:
  phases are subprocesses, not imports, so a per-phase chain file (consumed by
  the orchestrator) is the only viable wiring without restructuring.
- Master chain is always written, even on partial runs. Constitution C7 reads
  "no artefact ships without the chain" — a partial chain that records
  dangling references is itself a useful audit document.
- No phase script is forced to depend on audit_chain.py unless it opts in via
  `--audit-chain`. Backwards-compatible at the CLI level (no signature break).

## Files modified

- `scripts/exp2/run_exp2.py`:
  - New imports: `AuditChain`, `load_config`, `output_dir`, `sha256_file`.
  - New constant `AUDIT_AWARE_PHASES = {"estimators", "n_eff"}`.
  - `run()` accepts optional `audit_chain_path: Path | None` parameter and
    appends `--audit-chain <path>` to the subprocess command.
  - New `finalize_master_audit_chain(config_path)` function (~55 lines).
  - `main()` computes per-phase chain paths and always calls
    `finalize_master_audit_chain` at the end (whether or not all phases ran).
- `scripts/exp2/run_estimators.py`:
  - `--audit-chain` CLI flag forwards to a phase chain that records each pair
    result file and the cross_validation_report event.
- `scripts/exp2/effective_rank.py`:
  - `--audit-chain` CLI flag forwards to a phase chain that records the
    n_eff_summary + entropy_per_embedder artefacts.
- `scripts/exp2/tests/test_synthetic_full_sweep.py`:
  - New test `test_orchestrator_finalize_master_audit_chain_on_partial_run`
    verifies the master chain is written on a stub partial run (only an env
    chain present), with `chain_sha256` (64 hex), `phase_chain:environment`
    artefact, and `phase_chain_missing` / `artefact_missing` events.
  - Test isolates work under `artefacts/exp2/.test_master_chain_<short>/`
    because `AuditChain.add_artefact` stores paths relative to the repo root.

## What this does NOT change

- verify_environment.py's existing chain wiring is untouched.
- Phase scripts that don't yet have `--audit-chain` (corpus, embeddings,
  projections, visualize, fsv) still produce their artefacts; the master
  chain hashes the artefact contents directly. No correctness gap; future
  per-phase wiring is straightforward but out of scope here.

## Verification

- pytest before R2: 25/25 PASS (after R1 + R3).
- pytest after R2: 26/26 PASS — new orchestrator test passes; no regressions.
- Static import check: all modified modules import cleanly.
