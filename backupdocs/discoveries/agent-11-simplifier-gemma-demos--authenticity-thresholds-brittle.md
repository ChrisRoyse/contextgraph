---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:14:00Z
status: active
severity: low
tags: thresholds, calibration, magic-numbers, authenticity
---

# Authenticity verdict thresholds are hand-tuned magic numbers — document and centralize

## What Happened

`_gemma_shakespeare_lib.py::authenticity_verdict` (lines 241-253) has three
thresholds:

| Threshold     | Verdict       | Source                                 |
|---------------|---------------|----------------------------------------|
| `spread≥0.08` | SHAKESPEAREAN | Hand-tuned on 4 test inputs            |
| `abs≥0.45`    | SHAKESPEAREAN | Hand-tuned on 4 test inputs            |
| `spread≥0.04` | ARCHAIC       | Hand-tuned on 4 test inputs            |
| `abs≥0.40`    | ARCHAIC       | Hand-tuned on 4 test inputs            |

Reviewer's prompt asks: "too brittle?"

## Verdict: THEY ARE BRITTLE, but that's acceptable for v0.

Evidence already in hand:

- Sonnet 18 scored SHAKESPEAREAN ✓
- Modern English → REJECTED ✓
- Python code → REJECTED ✓
- GPT-4 fake Shakespeare → SHAKESPEAREAN (FALSE POSITIVE — noted in prompt)

4 of 4 "correct" verdicts is a 75% TP rate (with a known FP). Thresholds
calibrated on 4 inputs will NOT generalize to, e.g.:

- Edge cases: Early Modern English that isn't Shakespeare (Marlowe, Jonson)
- Translations of Shakespeare back-translated from other languages
- Shakespearean parody / mock-archaic
- Lorem-ipsum in capital letters

## Recommendations

1. **Document provenance.** Add a comment above the constants naming the 4
   inputs they were tuned on:
   ```python
   # Thresholds calibrated on: Sonnet 18, modern prose, Python, GPT-4 fake.
   # Known FP: GPT-4 trained on Shakespeare passes "SHAKESPEAREAN".
   ```
2. **Centralize.** Move to module-level constants so future re-tuning is a
   one-line change:
   ```python
   SHAKESPEAREAN_SPREAD_THRESHOLD = 0.08
   SHAKESPEAREAN_ABS_THRESHOLD = 0.45
   ARCHAIC_SPREAD_THRESHOLD = 0.04
   ARCHAIC_ABS_THRESHOLD = 0.40
   ```
3. **Plan a re-calibration pass.** Before v1 ships, score ~50-100 held-out
   inputs (authentic / archaic / modern / fake) and fit thresholds to
   maximize F1. Not needed for the demo.

## Why It Matters

Thresholds that weren't documented as calibrated will be treated as gospel
by future agents — they'll tune other things around these numbers. Flag them
as approximate now.

## Related

- Tags: calibration, thresholds, magic-numbers, verdict-logic
