---
namespace: journal
agent_id: agent-02-tester
timestamp: 2026-04-28T23:51:00Z
status: active
severity: high
tags: exp2, blocker-verified, b1, structural-upper-bound, effective-rank
---

# VERIFIED: B-1 — `structural_upper_bound: 105` literal exists at effective_rank.py:145

## Static evidence
```
$ grep -n "structural_upper_bound\|105\|91" scripts/exp2/effective_rank.py
57:    if len(rows) != 91:
58:        raise Exp2Error(f"Expected 91 pair result files, found {len(rows)} in {pair_dir}")
145:            "structural_upper_bound": 105,
151:        "n_total_pairs": 91,
```

The literal `"structural_upper_bound": 105` is present on line 145 of `scripts/exp2/effective_rank.py` in the `headline` dict written to `n_eff_summary.json`. Spec `08_output_deliverables.md` example schema shows `"structural_upper_bound": 91`.

## Spec self-inconsistency reproduced
- `docs/exp2/03_methodology.md` §7a states: "If all pairs are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$."
  This is **mathematically wrong**: $N + \binom{N}{2} = 14 + 91 = 105$, not 91.
- `docs/exp2/08_output_deliverables.md` line 17 example: `"structural_upper_bound": 91`.
- Implementation chose 105 (the math), spec headline schema chose 91 (the paper headline number).

## Runtime verification (planned)
Will be executed in M3 by constructing a synthetic 14×14 zero MI matrix and an entropy vector, calling `n_eff_redundancy_adjusted`, and confirming the output matches `n + sum(1-r)`. The literal 105 is only emitted in `main()` (file writes), so I will simulate the headline dict construction directly.

## Status
Confirmed against source. No runtime test was needed for the literal — `grep` is sufficient. Coordinator must arbitrate spec ambiguity (91 vs 105).
