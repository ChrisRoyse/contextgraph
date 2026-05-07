---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-18T08:30:00Z
status: resolved
severity: low
tags: discovery, lesson, v4, arithmetic, mission-brief
---

# Discovery: Mission Brief Target Arithmetic Off By 1,000

## What Happened

The v4 mission brief I wrote at session start said "10,000 prompts" and listed 12 categories with individual targets. When summing the category targets, they came out to 9,000, not 10,000:

```
1200 + 1000 + 1000 + 800 + 800 + 800 + 800 + 700 + 700 + 600 + 400 + 200 = 9,000
```

The percentages in the same table ("12%, 10%, 10%, ...") DID sum to 100%, masking the arithmetic error at author time.

## Why It Matters

- FSV Check 1 said "exactly 10,000 lines (±50 tolerance)". With a 9,000 output, that check would FAIL if run against the original spec.
- Later agents (Phase 2 response generator, training agent) would have been confused about whether the output is complete or short.
- Training data distribution math for 9K vs 10K is materially different for weight profiles.

## Root Cause

Author-time arithmetic error. I specified percentages independent of category counts, didn't sum. The %'s summed to 100, which gave false confidence.

## Resolution

Updated `./memory/context/mission-v4-prompt-bank.md` to reflect 9,000 target (engineering reality). FSV check 1 adjusted accordingly. 9,000 style-pure compositional prompts is still a 9.3× expansion over v2.1's 7,925.

## Lesson for Future Mission Briefs

ALWAYS assert `sum(targets) == declared_total` in Python before committing a mission brief. Example:

```python
assert sum(c.target_count for c in CATEGORIES) == MISSION_TOTAL, \
    f"Mission math mismatch: category sum {sum(c.target_count for c in CATEGORIES)} != declared {MISSION_TOTAL}"
```

Could add this as a one-line check at top of `main()` in the generator itself to fail-fast before writing anything.

## Related

- Mission brief (updated): `./memory/context/mission-v4-prompt-bank.md`
- SITREP: `./tmp/SITREP_2026-04-18_080537.md`
