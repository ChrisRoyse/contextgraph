---
namespace: decisions
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:37:00Z
status: active
tags: exp2, severity-rubric, audit
---

# Decision: severity rubric used in this audit

## High
A finding is HIGH when at least one of:
- Violates a Constitution rule (C1-C18) explicitly.
- Blocks a critical-path task (T-001, T-006, T-016, T-021, T-024) from completing as spec'd.
- Produces an artefact whose published number contradicts another published artefact.
- Eliminates a fail-loud guard the spec required (e.g., audit chain not produced; per-pair provenance hashes missing).

Findings flagged HIGH:
- effective_rank.py `structural_upper_bound: 105` vs spec's `91` (`agent-01-static-audit--effective-rank-structural-upper-bound-wrong.md`)
- AuditChain class never instantiated by pipeline (`agent-01-static-audit--no-audit-chain-built-anywhere.md`)
- Per-pair JSON rows missing 8 spec'd provenance fields (`agent-01-static-audit--per-pair-rows-missing-provenance.md`)
- InfoNCE.fit() raises on overfit gap → halts production sweep mid-run (`agent-01-static-audit--failure-infonce-overfit-raise-blocks-pipeline.md`)
- run_exp2.py PHASE_COMMANDS doesn't include audit_chain or replay (`agent-01-static-audit--failure-run-exp2-no-audit-chain-no-fsv-flag.md`)

## Medium
- Spec drift that doesn't break the run but breaks audit-chain integrity or hyperparameter pre-registration.
- F1-F7 failure-mode coverage gaps.
- Test coverage missing for spec'd behavior.

## Low
- Cosmetic, e.g., recommendation thresholds without basis.
- Minor `except Exception:` swallows in non-critical paths.

## Counts (final)
- HIGH: 5
- MEDIUM: 14
- LOW: 5
- Successes: 9 (documented as positive findings)

## Rubric calibration
The bar for HIGH is "ship-blocker or audit-chain breaker." Implementation issues that produce an arguable but consistent number (e.g., `structural_upper_bound: 105`) qualify because they contradict a spec example used elsewhere in the deliverables.
