---
namespace: decisions
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:45:00Z
status: declined-operator-decision
severity: high
tags: exp2, structural-upper-bound, b1, spec-vs-code, operator-decision
---

# R4 — DECLINED: B-1 structural_upper_bound 91 vs 105

## Brief instruction

> R4. (Decline) B-1 structural_upper_bound 91 vs 105
> This is an OPERATOR DECISION (spec-vs-code conflict). Do NOT change the code.
> Document in your final report that this requires operator sign-off on which
> interpretation is correct.

I declined the change as instructed.

## What the conflict is (recap)

- `scripts/exp2/effective_rank.py:145` writes `"structural_upper_bound": 105`.
- The math: `n_eff_redundancy_adjusted` formula = `N + Σ(1-r_jk) over all C(N,2) pairs`.
  At maximum independence (all r=0) this equals `N + C(N,2) = 14 + 91 = 105`.
- `docs/exp2/00_README.md:4` says "$\binom{14}{2} = 91$ embedder pairs".
- `docs/exp2/03_methodology.md` §7a writes the formula then says "If all pairs
  are independent, $N_{\mathrm{eff}}^{(1)} = N + \binom{N}{2} = 91$." — this
  arithmetic is wrong (the LHS is 105 by the formula above, not 91).
- `docs/exp2/05_technical_spec.md` §5f and `docs/exp2/08_output_deliverables.md`
  §1 schema example both say `"structural_upper_bound": 91`.

So the spec is internally inconsistent. The implementation chose the math (105).
The published examples chose the headline (91). agent-01 raised this as B-1.
agent-02 confirmed at runtime via grep + value check.

## Why a code fix would be premature

There are two valid resolutions:

a. **Change the code to 91** — interpret "structural_upper_bound" as the count
   of pairs (the paper's "up to 91 signals" headline). This breaks the
   methodology formula's max value but preserves the paper text.

b. **Change four spec locations to 105** — interpret as the formula's max.
   This breaks the paper headline ("up to 91 signals"); paper would need
   re-edit.

These are publication-impact decisions, not code-quality decisions. The
operator (chrisroyseai@gmail.com) needs to pick which interpretation ships.

## Operator action required (carry to final synthesis)

Pick one:

- **Option A**: Change `effective_rank.py:145` to `91`. Update formula prose
  in `docs/exp2/03_methodology.md` §7a to remove the "= 91" arithmetic claim
  (replace with "max = 91 if interpreted as pair count").
- **Option B**: Change `docs/exp2/05_technical_spec.md` §5f, `docs/exp2/08_output_deliverables.md`
  §1 schema example, AND `docs/exp2/03_methodology.md` §7a (line: "= 91") to `105`.
  Update paper headline accordingly ("up to 105 signals" rather than "91").

## Status

Code untouched. Spec untouched. Documented for operator arbitration.
