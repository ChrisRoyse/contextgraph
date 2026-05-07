---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:11:00Z
status: active
severity: medium
tags: exp2, fsv, replay, scalar-comparison, schema
---

# Defect: fsv.compare_replay only checks 3 scalars per pair, not all 13 from §13 schema

## Where
`scripts/exp2/fsv.py:64-81` — `compare_replay`

## Spec
`docs/exp2/07_validation_harness.md` §2 (`replay`):
> Compare per-pair MI estimates.
>
> ```python
> results.append(check_within_tolerance(
>     ref, rep, scalars=["infonce_estimate_nats", "ksg_estimate_nats", "redundancy"],
>     tolerance=1e-5,
> ))
> ```

`docs/exp2/03_methodology.md` §13 — full pair audit chain row has 18+ scalars:
- infonce_estimate_nats, infonce_ci_low, infonce_ci_high
- infonce_train_loss_final, infonce_val_loss_final, infonce_train_val_gap
- ksg_estimate_nats, ksg_ci_low, ksg_ci_high
- agreement_nats, agreement_pass, redundancy

## Observed
```python
for key in ["infonce_estimate_nats", "ksg_estimate_nats", "agreement_nats"]:
    _compare_scalar(...)
```
Only 3 scalars. The spec's example also uses 3 (`infonce_estimate_nats, ksg_estimate_nats, redundancy`) — different from what the implementation chose. The implementation swaps `redundancy` for `agreement_nats`.

## Why it matters
- Spec example listed `redundancy` for replay determinism. Redundancy is the headline-feeding scalar (it goes into the heatmap and N_eff). The implementation drops it.
- Bootstrap CIs (ci_low, ci_high) carry the variance of the headline; if they drift between replay runs, that's a methodological inconsistency. Spec implicitly expected them checked too. Implementation skips.

## Recommended fix
Compare the full set in `03_methodology.md` §13 — or document why a 3-scalar subset is sufficient (e.g., "the median val InfoNCE deterministically determines the rest"). Currently it's neither a complete check nor a documented narrowing.

Add `redundancy` at minimum.

## Cross-check
Agent-02 should test by writing a minimal pair JSON file and running `compare_replay` — drop a scalar and see what's caught.

## Severity
MEDIUM. Replay determinism is C6's mandate. Skipping replay-checks on 9 of 12 spec'd scalars is not fail-loud.
