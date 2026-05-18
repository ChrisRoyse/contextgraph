# Archived Post-Ship Roadmap — Phase J

**Archived:** 2026-05-18.

This roadmap is intentionally inactive until the Python ship gate closes:

```text
prediction_oracle_correlation >= 0.95
  stable x 4 consecutive rolling windows
  per (mutation_category x language)
  on the 300x8 Python SWE-bench Lite corpus
```

## Deferred Until The Gate Is Green

- Non-Python language activation, one language at a time, each with its own four-window stability cycle.
- Future modality templates such as `ME-JEPA-Voice` and `ME-JEPA-Image`.
- Post-ship hardening maturity tracks: supply chain, observability uplift, scalability tests, and soak-test cadence.
- API stability commitments and compatibility locks.
- Auto-trust policy rollout beyond the guarded Python verifier.

## Active Rule

Do not spend Phase G execution bandwidth on Phase J unless the work directly reduces risk for the current Python `>= 0.95 x 4 windows` gate. Post-ship ideas remain searchable here, but they are not active backlog.
