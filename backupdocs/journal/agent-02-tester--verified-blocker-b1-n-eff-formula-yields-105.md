---
namespace: journal
agent_id: agent-02-tester
timestamp: 2026-04-29T00:15:00Z
status: active
severity: high
tags: exp2, blocker-verified, b1, n-eff, runtime-evidence
---

# VERIFIED (runtime): B-1 — `n_eff_redundancy_adjusted` produces 105.0 on synthetic zero-MI input

## Test 1: Zero MI matrix (all 91 pairs independent)
```python
N = 14
zero_mi = np.zeros((14, 14))
entropy = np.ones(14) * 1.0
result = n_eff_redundancy_adjusted(zero_mi, entropy)
# result == 105.0
```

Output: **N_eff^(1) = 105.0**, matches `N + C(N,2) = 14 + 91 = 105` mathematically.

## Test 2: All-redundant matrix (all r=1)
```python
H = 2.0
red_mi = np.ones((14, 14)) * H
red_mi[np.eye(14, dtype=bool)] = 0
ent = np.ones(14) * H
result = n_eff_redundancy_adjusted(red_mi, ent)
# result == 14.0
```

Output: **N_eff^(1) = 14.0**, matches the lower bound (only marginal embedders contribute).

## Conclusion

The function in `scripts/exp2/effective_rank.py` implements the spec formula correctly:
- Lower bound 14 (all redundant, r=1): ✓
- Upper bound 105 (all independent, r=0): ✓

The hardcoded literal `"structural_upper_bound": 105` at line 145 matches the function's actual mathematical maximum.

## So which is wrong?

The spec is internally inconsistent (per agent-01's blocker writeup):
- `docs/exp2/03_methodology.md` §7a prose says "If all pairs are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$" — but $14 + 91 = 105$, so "= 91" is a math typo.
- `docs/exp2/08_output_deliverables.md` example shows `"structural_upper_bound": 91` — using the "paper headline" interpretation.
- Implementation chose 105 (the math).

**The implementation is mathematically correct; the spec has the typo.** Coordinator must arbitrate: either fix the spec to say 105 OR change the implementation to 91 (re-interpreting "structural upper bound" to mean "number of pairs", not "max value of N_eff^(1)").

## Status
B-1 is real but the resolution is a SPEC fix, not a CODE fix. Severity unchanged (HIGH) because the paper headline number is at stake.
