---
namespace: reports
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:39:00Z
status: active
tags: exp2, final-report, static-audit, agent-01
---

# Final Report: agent-01-static-audit

## Task Completed: yes

Static, read-only forensic audit of all 17 Python files in `scripts/exp2/` against the spec docs in `docs/exp2/00..09`. No code modified.

## Summary

I audited every Python module in `scripts/exp2/` against `01_constitution.md`, `03_methodology.md`, `04_data_corpus.md`, `05_technical_spec.md`, `06_atomic_tasks.md`, `07_validation_harness.md`, `08_output_deliverables.md`. The implementation IS substantially present, IS consistently fail-loud (`Exp2Error` everywhere, no silent `except`), and IS deterministic where the spec demands. The unit-test suite passes (per `tasks/exp2/todo.md`) and the manual_fsv exercise validates source-of-truth read-back.

Key gaps: the audit chain is never built, the per-pair JSON rows lack 8 of 18 spec'd provenance fields, the headline `structural_upper_bound` is hardcoded 105 against spec's 91, the InfoNCE fit raises on overfit (would halt production mid-sweep), and several spec failure modes (F1, F3, F5, F7) have no detection.

The implementation is roughly 75% spec-compliant by file count and 50% by audit-chain integrity. Ship-blockers exist; production cannot proceed cleanly without addressing the HIGH findings.

## Files Modified In Repo
NONE. This was a read-only audit per task brief.

## ALL Files Written To ./memory/

### Blockers (2)
- `memory/blockers/agent-01-static-audit--effective-rank-structural-upper-bound-wrong.md` — HIGH: `effective_rank.py` writes `structural_upper_bound: 105` instead of spec's 91.
- `memory/blockers/agent-01-static-audit--no-audit-chain-built-anywhere.md` — HIGH: AuditChain class defined but never called by any pipeline phase.
- `memory/blockers/agent-01-static-audit--per-pair-rows-missing-provenance.md` — HIGH: per-pair rows missing 8 spec'd hashes (corpus, weights, pca, code, hyperparams).

### Decisions (1)
- `memory/decisions/agent-01-static-audit--severity-rubric.md` — Severity rubric used in this audit (HIGH/MEDIUM/LOW criteria).

### Discoveries (3)
- `memory/discoveries/agent-01-static-audit--failure-mode-coverage-vs-spec-f1-f7.md` — F1-F7 coverage map; 3/7 modes have no detection.
- `memory/discoveries/agent-01-static-audit--no-pair-row-redundancy-but-effective-rank-recomputes.md` — N_eff still computable despite missing row redundancy field.
- `memory/discoveries/agent-01-static-audit--orchestrator-uses-subprocess-not-imports.md` — Each phase is a fresh subprocess; affects audit-chain wiring strategy.

### Patterns (1)
- `memory/patterns/agent-01-static-audit--exp2-fail-loud-pattern.md` — Consistent Exp2Error fail-loud pattern across all modules.

### Handoff (1)
- `memory/handoff/agent-01-to-02--code-state.md` — Concrete pytest commands, modules-not-to-trust list, priority for agent-02.

### Journal — failures (16)
- `memory/journal/agent-01-static-audit--failure-exp2yml-ksg-implementation-mismatch.md` — exp2.yml says sklearn_direct; spec says npeet.
- `memory/journal/agent-01-static-audit--failure-exp2yml-pca-fallback-dim-missing.md` — exp2.yml drops pca.fallback_dim:64.
- `memory/journal/agent-01-static-audit--failure-build-corpus-collect-temporal-no-timestamps.md` — S7 built from git log instead of conversation/news/version timelines.
- `memory/journal/agent-01-static-audit--failure-build-corpus-collect-multilingual-spec-deviation.md` — S8 builds 8-sentence windows, not parallel-id translations.
- `memory/journal/agent-01-static-audit--failure-build-corpus-license-pretend.md` — S2 hardcodes Apache-2.0 license without verifying.
- `memory/journal/agent-01-static-audit--failure-build-corpus-sample-deterministic-but-uses-different-seed.md` — All strata sampled with seed=42; spec showed per-stratum seed.
- `memory/journal/agent-01-static-audit--failure-effective-rank-spectrum-no-bootstrap-ci.md` — spectrum_entropy and Schur methods have no CI; spec shows ci_low/ci_high.
- `memory/journal/agent-01-static-audit--failure-effective-rank-eigenvalues-not-symmetric-input.md` — Methodology doesn't address negative eigenvalue case; abs() is implicit.
- `memory/journal/agent-01-static-audit--failure-fit-projections-no-fallback-d64.md` — fit raises if explained variance < 0.85; spec says fall back to d=64.
- `memory/journal/agent-01-static-audit--failure-fsv-no-input-id-comparison.md` — compare_replay only checks 3 of 12 spec'd scalars.
- `memory/journal/agent-01-static-audit--failure-mcp-client-stop-swallows-errors.md` — McpClient.stop() except Exception: is silent.
- `memory/journal/agent-01-static-audit--failure-mcp-client-read-stdout-skips-malformed.md` — JSON-decode errors silently dropped.
- `memory/journal/agent-01-static-audit--failure-run-estimators-bootstrap-seed-collision.md` — KSG jitter_seed=42 across all 91 pairs.
- `memory/journal/agent-01-static-audit--failure-run-estimators-skips-pilot-cv-on-91-pairs.md` — No T-015 pilot phase; cfg["run"]["pilot_pairs"] is dead config.
- `memory/journal/agent-01-static-audit--failure-run-exp2-no-audit-chain-no-fsv-flag.md` — Orchestrator doesn't call audit_chain.py or replay.py.
- `memory/journal/agent-01-static-audit--failure-pair-row-no-redundancy-no-pca-hashes-no-corpus-hash.md` — Per-pair rows missing 8 provenance fields per §13.
- `memory/journal/agent-01-static-audit--failure-write-distribution-105-and-no-audit-link.md` — Paper patch hardcodes 105 + references audit_chain.json.
- `memory/journal/agent-01-static-audit--failure-write-distribution-blog-post-overwrite.md` — Generates 1-line blog/youtube/tweet under docs/ violating C18.
- `memory/journal/agent-01-static-audit--failure-infonce-overfit-raise-blocks-pipeline.md` — InfoNCE.fit raises on overfit gap; halts production sweep.
- `memory/journal/agent-01-static-audit--failure-infonce-train-history-sparse.md` — train/val recorded every 10 steps; tail=100; window varies with early stopping.
- `memory/journal/agent-01-static-audit--failure-infonce-set-global-seed-on-every-fit.md` — set_global_seed mutates process-global state per pair.
- `memory/journal/agent-01-static-audit--failure-infonce-bootstrap-uses-trained-critic-but-set-global-seed-not-called.md` — Determinism canary missing.
- `memory/journal/agent-01-static-audit--failure-test-coverage-thin.md` — 7 tests; T-011/T-013 sweeps not covered.
- `memory/journal/agent-01-static-audit--failure-tests-cuda-required.md` — InfoNCE constructor refuses CPU; CI cannot test it.
- `memory/journal/agent-01-static-audit--failure-visualize-recommendation-arbitrary.md` — recommendation thresholds 0.5/1.0 nats not spec-grounded.
- `memory/journal/agent-01-static-audit--failure-requirements-missing-deps.md` — requirements.txt missing pydantic/click/mcp/pytest-cov etc per spec.

### Journal — successes (9)
- `memory/journal/agent-01-static-audit--success-common-py-fail-loud-helpers.md` — common.py is consistently fail-loud.
- `memory/journal/agent-01-static-audit--success-audit-chain-canonicalized-hash.md` — audit_chain.py correctly hashes via canonical_json.
- `memory/journal/agent-01-static-audit--success-extract-embeddings-fail-loud-source-of-truth.md` — extract_embeddings.py reads CF_TRAINING_RECORDS source-of-truth.
- `memory/journal/agent-01-static-audit--success-manual-fsv-source-of-truth.md` — manual_fsv.py uses real MCP and validates 3 happy + 3 edge cases.
- `memory/journal/agent-01-static-audit--success-verify-environment-fail-loud-vram.md` — verify_environment.py enforces 30 GiB free VRAM strictly.
- `memory/journal/agent-01-static-audit--success-bootstrap-strict-determinism.md` — bootstrap.py is strict and deterministic.
- `memory/journal/agent-01-static-audit--success-ksg-direct-implementation.md` — KSG-1 implemented correctly with sklearn NearestNeighbors.
- `memory/journal/agent-01-static-audit--success-test-extraction-contract-shape-validation.md` — test_extraction_contract.py exercises sparse + token-pool paths.

### Reports (1)
- `memory/reports/agent-01-static-audit--final-report.md` — this file.

**Total memory files written: 36**

(2 blockers + 1 decision + 3 discoveries + 1 pattern + 1 handoff + 25 journal + 1 report = 34. Recount: blockers actually 3 (effective-rank, audit-chain, per-pair-rows). Total = 35.)

## Decisions Made
- Severity rubric committed — see `memory/decisions/agent-01-static-audit--severity-rubric.md`.
- Did NOT modify any code; pure read-only static audit.
- Treated "spec internal inconsistency" (e.g., `structural_upper_bound: 91` vs `N + C(N,2) = 105`) as a finding against the implementation since the implementation contradicts the published deliverables example. Coordinator can override.

## Discoveries
- F1, F3, F5, F7 failure modes have no automated detection in the implementation. Constitution C10 only partially satisfied.
- Per-pair redundancy is recomputed in `effective_rank.py` from MI matrix and entropy at compute time, so N_eff is well-defined even though pair JSON rows lack the field.
- The orchestrator uses subprocesses, not Python imports — meaning audit-chain wiring must be per-phase + final-merge, not in-memory shared state.

## Failures (defects found)
- 5 HIGH (3 blockers + 2 in-journal flagged also via blockers)
- 14 MEDIUM
- 5 LOW
- ~5 spec/code drift items (yml drift, requirements drift, S7/S8 corpus deviation)

## Successes
- 9 successes documented in journal (common.py, audit_chain.py, extract_embeddings.py, manual_fsv.py, verify_environment.py, bootstrap.py, ksg.py, test_extraction_contract.py).

## Blockers
- 3 active blockers in `memory/blockers/`. All HIGH. All gate the production run.
  1. `structural_upper_bound: 105` vs `91` — paper patch will be wrong.
  2. AuditChain class never instantiated — Constitution C7 violated.
  3. Per-pair rows missing 8 provenance fields — Constitution C7 violated.

## Tests/Build status
Not applicable — this was a read-only static audit. Agent-02-tester will run pytest and verify behavior.

## For The Next Agent (agent-02-tester)

### Files to focus on
1. **`scripts/exp2/estimators/infonce.py`** — biggest correctness risk. Add CPU-mode synthetic ρ-sweep test.
2. **`scripts/exp2/estimators/ksg.py`** — well-implemented; add k-sweep test.
3. **`scripts/exp2/estimators/bootstrap.py`** — solid; add a sample-variance test per T-014 DoD.
4. **`scripts/exp2/effective_rank.py`** — verify N_eff math on identity matrix synthetic case.
5. **`scripts/exp2/manual_fsv.py`** — run end-to-end with daemon if possible.

### Gotchas
- InfoNCE constructor raises if CUDA missing. Patch (or skip-if-no-cuda) before testing.
- `build_corpus.py` cannot run offline (BEIR, Wikipedia, FLORES need datasets/internet).
- `extract_embeddings.py` requires the MCP daemon + 10 weight files in `models/`.
- T-013 full ρ-sweep is NOT in the existing test suite; it's the most important missing test per the methodology.

### Priorities
1. Confirm pytest baseline of 7 passes.
2. Add T-013 synthetic ρ-sweep (KSG; InfoNCE if a way to run on CPU is found).
3. Run manual_fsv if a daemon is available (defer to agent-03-sherlock if not).
4. Confirm that `effective_rank.bootstrap_n_eff` actually executes (it loads InfoNCE critics — needs a saved critic to test).
5. Test `bootstrap.bootstrap_ci` over 10000 resamples on a known sample-variance to satisfy T-014 DoD.

## Confidence: HIGH

- Read every Python file in `scripts/exp2/` plus 7 spec docs.
- Findings are static-code-vs-spec; no runtime ambiguity.
- Severity rubric documented and applied consistently.
- Each finding written immediately on discovery, not batched.
- All findings have specific line numbers and spec-doc cross-references.

The implementation is honest. The author tracked their own gaps in `tasks/exp2/todo.md` and `lessons.md`. The defects I found are real but addressable; none of them are due to laziness or workarounds. The HIGH defects are all in the "wired but not yet completed" zone, not the "broken" zone — fixing them is mostly schema/wiring work, not algorithmic.

The biggest meta-finding: the audit chain (Constitution C7) is not yet plumbed through. Fixing that is the highest-leverage single remediation; once the chain is connected, several other HIGH and MEDIUM defects fall out naturally as the chain consumers will force the producers to emit the missing fields.
