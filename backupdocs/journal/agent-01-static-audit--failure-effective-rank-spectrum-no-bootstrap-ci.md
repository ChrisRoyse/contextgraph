---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:06:00Z
status: active
severity: medium
tags: exp2, effective-rank, bootstrap, ci, spec-mismatch
---

# Defect: effective_rank.py only bootstraps the redundancy-adjusted N_eff, not the spectrum or schur methods

## Where
`scripts/exp2/effective_rank.py:96-116` — `bootstrap_n_eff` only computes CI for `n_eff_redundancy_adjusted`.
`scripts/exp2/effective_rank.py:138-153` — `summary` writes `n_eff_spectrum_entropy` and `n_eff_schur_complement_int` with only `point`, no `ci_low`/`ci_high`.

## Spec
`docs/exp2/05_technical_spec.md` §5f example schema:
```json
{
  "n_eff_redundancy_adjusted": {"point": 47.3, "ci_low": 44.1, "ci_high": 50.6},
  "n_eff_spectrum_entropy": {"point": 11.8, "ci_low": 11.4, "ci_high": 12.2},
  "n_eff_schur_complement_int": {"point": 11, "ci_low": 10, "ci_high": 12},
  ...
}
```

`docs/exp2/06_atomic_tasks.md` T-021 DoD:
> [ ] Three N_eff methods computed per [03_methodology.md](03_methodology.md) §7.
> [ ] All three with bootstrap CIs.

## Observed
The implementation writes:
```python
"n_eff_redundancy_adjusted": {"point": primary, "ci_low": low, "ci_high": high},
"n_eff_spectrum_entropy": {"point": spectrum},   # missing CI
"n_eff_schur_complement_int": {"point": schur},  # missing CI
```

So the spectrum and Schur methods have only point estimates, not bootstrap CIs.

## Why it matters
- Constitution C4: "Every reported MI scalar carries a 95% bootstrap CI." Spectrum_entropy and Schur_complement are reported scalars; they need CIs.
- T-021 will be marked complete despite missing CIs for two of the three methods.
- Reviewers comparing the methods will lack the variance information needed to judge agreement.

## Recommended fix
Extend `bootstrap_n_eff` to emit three returns (one per method). For each bootstrap resample, recompute MI matrix from cached critics, then call all three N_eff functions, percentile each.

Plain-clean implementation: factor the per-resample computation into a helper that returns three values; aggregate three percentile arrays.

## Cross-check
None needed — this is a static code-vs-spec mismatch.
