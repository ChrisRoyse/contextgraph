---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:21:00Z
status: active
severity: medium
tags: exp2, paper-patch, headline, distribution, drift, c12
---

# Defect: write_distribution.py paper patch claims "structural upper bound 105" and references audit_chain.json that doesn't exist

## Where
`scripts/exp2/write_distribution.py:30-39`

## Spec
- `docs/exp2/00_README.md`: "up to 91 structured signals per input."
- `docs/exp2/08_output_deliverables.md` §1: `"structural_upper_bound": 91`.
- C12 (paper patch is drop-in): "If the patch breaks paper continuity, the patch is wrong, not the surrounding text."

## Observed
```
paper = f"""# EXP-2 Paper Patch
## Section 3.1
The structural upper bound for the production 14-embedder panel is 105 signals per input.
...
```

The patch hardcodes "105" — matching the `effective_rank.py` `structural_upper_bound: 105` defect (already a HIGH blocker; see `agent-01-static-audit--effective-rank-structural-upper-bound-wrong.md`). So the paper headline replacement says "105" while the paper text says "91".

Plus references to `artefacts/exp2/audit_chain.json` (line 38) — file that no orchestrator phase produces.

## Why it matters
- C12: the patch must be drop-in. A patch that mentions "105" against a paper that mentions "91" breaks continuity.
- C7: referencing audit_chain.json that doesn't exist makes the patch unverifiable.

## Recommended fix
- Either change to "91" (and fix `effective_rank.py`).
- Or operator-decision sequence (acknowledge spec internal inconsistency and choose one).
- Wire audit_chain.json into the pipeline first.

## Cross-check
None.
