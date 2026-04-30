# 11 - Risks, Open Questions, And Decisions

## Locked Decisions

| Decision | Choice | Reason |
| --- | --- | --- |
| First paper demo data | Public career taxonomy benchmark | Safe and reproducible without private recruiting corpus |
| Real recruiting claims | Not made unless licensed corpus exists | Avoid overclaiming |
| Evidence format | Immutable research bundle | Supports paper, appendix, reproducibility |
| Smoke before full run | Mandatory | Reduces wasted 5090 runs |
| Fail behavior | Fail closed | Matches DynamicJEPA/ME-JEPA constraints |

## Open Questions

1. Which public taxonomy source is primary: O*NET, ESCO, or both?
2. Do we have permissioned real career/recruiting data for a later Phase 12 run?
3. What is the smallest career skill vector that is still meaningful?
4. Should pairwise features be implemented in DynamicJEPA storage for the paper
   run, or deferred and discussed as future ME-JEPA work?
5. Should the first career benchmark use generated taxonomy walks or a public
   career trajectory dataset if one is licensed and accessible?
6. What exact paper claim should be promoted: systems auditability, career
   transition modeling, or single-GPU feasibility?

## Main Risks

| Risk | Mitigation |
| --- | --- |
| Public taxonomy benchmark looks synthetic | Ground every event in source rows, record generator hash, state limitations clearly |
| Career-domain training underperforms | Present as systems demo first; include baselines and failure analysis |
| Evidence bundle becomes too large | Store hashes/tails for logs, sampled decoded rows, full artifacts on disk |
| 5090 run time grows | Keep smoke/paper-small tiers and scale only after gates pass |
| Real recruiting data absent | Do not block paper; use public benchmark and mark real-data work future |
| Compliance ambiguity | Fail closed and exclude any data without license/provenance |
| Domain-specific code slips in | Add source scan/audit for `if domain == career_taxonomy` outside fixture/generator code |

## Go / No-Go Criteria For Full Run

Go:

- Phase 10 controls pass.
- Career domain registration and ingest pass.
- Provenance/license manifest complete.
- Tiny career training beats random-init.
- Edge cases match expected deltas.
- Paper tables are generated from evidence bundle.

No-go:

- Missing data license.
- Any artifact hash mismatch.
- Any decoded readback mismatch.
- Any train/test leakage.
- Any failure hidden by passing tests.
- Any claim depends on unavailable real recruiting data.

