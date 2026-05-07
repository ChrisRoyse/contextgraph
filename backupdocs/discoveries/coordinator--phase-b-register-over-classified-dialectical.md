---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-17T17:55:00Z
status: active
severity: low
tags: phase-b, analyzer, register-classifier, post-fix
---

# Phase B register classifier defaults 91% to "dialectical"

## What We Observed
Sampling first 65 Phase B records: register.primary distribution is:
- dialectical: 59 (91%)
- philosophical: 3
- lyrical: 2
- comic: 1

Expected: much broader distribution including pleading, commanding, tragic, jest, contemplation. "Dialectical" (back-and-forth debate) should be uncommon, not dominant.

## Why It Matters
Phase C uses `register.primary` to pick which of 4 REJECTED variants to emit first. With 91% "dialectical", Phase C will emit less-varied REJECTED failure modes. However:
- Phase C's PASS/FAIL gate is constellation `style_score`, NOT register match
- Phase C iterates through all 4 variant types per chosen-record — register just picks the preferred ORDER
- Net effect: DPO pair *count* should be fine; DPO pair *diversity* is reduced

## Decision
**Let Phase B continue.** The primary Phase B signal is the 13-dim `style_profile_vector` (constellation scoring) which is correct. Register is secondary metadata. Fixing the classifier would require restarting Phase B (~9h lost). Post-pipeline fix is feasible.

## Post-Pipeline Fix Plan
1. Inspect `shakespeare_stylistic_analyzer.py` register-classification rules
2. Likely cause: "dialectical" keyword list too broad (catches "thou/you/question/answer" patterns in any dialogue)
3. Fix rule priority / thresholds
4. Run re-classification pass over the final tags JSONL (CPU-only, fast) without redoing constellation scoring
5. Rebuild Phase C DPO pairs with corrected register distribution IF necessary

## Also Noted
`meter.rhyme_scheme` emits `?` and alphabet-through-z for prose passages — rhyme extractor misfiring on non-verse input. Cosmetic only, Phase C ignores this field.

## Related
- See also: ./memory/decisions/coordinator--stylistic-tag-schema.md
- Tags: phase-b, classifier, low-severity
