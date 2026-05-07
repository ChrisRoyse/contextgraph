---
namespace: context
agent_id: coordinator-exp2-verification
timestamp: 2026-04-28T22:50:00Z
status: active
tags: exp2, constraints, no-mocks, fail-fast, no-backwards-compat
---

# EXP-2 Verification — Hard Constraints

These are inherited from operator instructions and `docs/exp2/01_constitution.md`.

## C1. No Mocks, No Fallbacks, No Workarounds
Every test must use real data (real MCP, real RocksDB, real Python torch) OR synthetic data with closed-form ground truth (Gaussians where MI = $-0.5 \log(1-\rho^2) \cdot d$). **Never** mock embedders. **Never** stub the MCP server. If a dependency is unavailable, the test must fail loud with diagnostics, not green via stub.

## C2. Fail Loud
Any silent error, any `except: pass`, any unwrapped Result that hides a fault is a finding. Errors must surface with actionable diagnostics: what was expected, what was observed, what to inspect next.

## C3. No Backwards Compatibility
Pre-production. The system must work after changes or fail fast so it can be debugged. No migration shims, no deprecated-flag gates. Wipe state on schema change. (See `./memory/feedback_no_backwards_compat.md` in the user's auto-memory.)

## C4. Files Off Repo Root
Per `CLAUDE.md`: artefacts go to `artefacts/exp2/`, code to `scripts/exp2/`, docs to `docs/exp2/`, tests to `scripts/exp2/tests/`. Never write to repo root.

## C5. Read Source-Of-Truth, Not Return Values
For storage operations: open the RocksDB CF and read it. For files: read the file from disk. For embeddings: confirm shape, finiteness, and L2 norm > 0. Don't trust return-value summaries.

## C6. Bootstrap Determinism
Seed = 42 throughout. Same seed → same numbers within fp32 precision (per Constitution C6).

## C7. Hash-Anchored Provenance
Per Constitution C7, every artefact carries a sha256 chain back to the corpus manifest, embedder weight hashes, and code commit. Verifying this chain integrity is part of the audit.

## C8. Subagent Discipline
- Read `./memory/` first (context, decisions, patterns, discoveries, blockers, handoff, reports, journal).
- Write `.md` files immediately on every discovery, decision, blocker, or noteworthy success/failure.
- Final report MUST list every memory file written.
- Never contradict a locked decision in `./memory/decisions/`.
- Never finish without `./memory/reports/agent-NN-{role}--final-report.md`.

## C9. Coordinator Verification
The coordinator personally checks every subagent's claimed output exists and contains the claimed content before trusting the report. (Operator rule: "You MUST check the database or tables or anything that might show physical proof.")

## C10. Synthetic Test Data Discipline
Use synthetic data for which we know the expected output by construction:
- T-013 Gaussian rho sweep: closed-form MI = $-0.5 \log(1-\rho^2) \cdot d$.
- 2+2=4 style: store memory with content X → fingerprint should appear in CF_FINGERPRINTS with content X.
- Edge cases: empty batch, whitespace content, invalid UUID — must produce VALIDATION_ERROR, not silent success.

If any synthetic test produces an output we cannot predict from spec, that is a methodology bug.
