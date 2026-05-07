---
namespace: blockers
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:05:00Z
status: active
severity: high
tags: exp2, effective-rank, n-eff, schema-violation, headline
---

# BLOCKER: effective_rank.py emits `structural_upper_bound: 105` — wrong, must be 91

## Where
`scripts/exp2/effective_rank.py:145` — in the `summary` dict written to `n_eff_summary.json`:
```python
"headline": {
    ...
    "structural_upper_bound": 105,
}
```

## Spec
- `docs/exp2/00_README.md` line 4: "$\binom{14}{2} = 91$ embedder pairs"
- `docs/exp2/03_methodology.md` §7a: "If all pairs are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$."
- `docs/exp2/05_technical_spec.md` §5f schema example, line 215: `"structural_upper_bound": 91`
- `docs/exp2/08_output_deliverables.md` §1, line 17: `"structural_upper_bound": 91,`
- `docs/exp2/06_atomic_tasks.md` T-021 success criterion: `Three N_eff methods computed per [03_methodology.md](03_methodology.md) §7.`

The math: $N + \binom{N}{2} = 14 + \frac{14 \cdot 13}{2} = 14 + 91 = 105$ — wait, this is actually the formula in the methodology. Let me re-read:

`docs/exp2/03_methodology.md` §7a:
> $$N_{\mathrm{eff}}^{(1)} = N + \sum_{j < k} (1 - r_{jk})$$
>
> Interpretation: the $N$ marginal embedders contribute $N$ signals; each pair adds a signal proportional to its non-redundancy $1 - r_{jk}$. **If all pairs are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$.**

So per the spec, the upper bound of N_eff^(1) is N + C(N,2) = 14 + 91 = **105**.

But the spec then SAYS this equals 91, which is mathematically wrong. C(14,2)=91; N+C(N,2) = 105. **Either the spec has a math error or the headline number meaning is "91" referring just to pairs, not to N+pairs.**

Cross-checking `08_output_deliverables.md` example:
```json
"n_eff": 47.3,
"ci_low": 44.1, "ci_high": 50.6,
"panel_size": 14,
"structural_upper_bound": 91,
```
This says upper bound is 91 with N_eff in [44.1, 50.6]. If the formula is `n + sum(1-r)`, then with n=14 and 91 pairs, the *theoretical max* is 105 (all r=0) and theoretical min is 14 (all r=1). 47.3 sits within [14, 105].

## So which is the "structural upper bound"?
Two valid readings:
1. **Reading A (the spec's prose intent)**: Upper bound = 91 (the paper headline "up to 91 signals per input"). N_eff measures how many of those 91 effective signals survive after redundancy is netted out. Then the formula `N + sum(1-r)` is wrong, and the formula should just be `sum(1-r) + N` which... is still 105 max.
2. **Reading B (the math)**: Upper bound = N + C(N,2) = 105.

The implementation chose reading B. The spec headline + outputs schema chose reading A. **The spec is internally inconsistent.**

## Why this is a HIGH-severity blocker
- `n_eff_summary.json` schema must match `08_output_deliverables.md` example exactly because that's what the paper, blog, video, and tweet thread will ingest. Changing 91 to 105 in the headline breaks the **paper headline replacement** ("up to 91 signals" → "measured ~47 of 91" makes sense; "measured ~47 of 105" does not match the paper text).
- Operator approval (C16) on the headline number cannot proceed if the upper-bound is uncertain.
- C7 (audit chain): every artefact records this number; if 105 ships and is later corrected to 91, the chain breaks.

## Recommended fix
**Operator decision required.** Most likely:
- Change implementation to `"structural_upper_bound": 91` (paper headline interpretation).
- OR fix the spec methodology §7a to say `N + sum(1-r), max = N + C(N,2) = 105` and update `08_output_deliverables.md` and `05_technical_spec.md` to also say 105.

The spec says 91 in the example for `08_output_deliverables.md`. The implementation says 105. **The implementation deviates from the spec; this is a defect against spec.**

## Cross-check
Agent-02 doesn't need to test; this is pure code review. Agent-04 (simplifier) should propose the elegant fix.
