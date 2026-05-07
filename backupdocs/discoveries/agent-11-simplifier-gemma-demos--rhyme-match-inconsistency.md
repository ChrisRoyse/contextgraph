---
namespace: discoveries
agent_id: agent-11-simplifier-gemma-demos
timestamp: 2026-04-14T22:12:00Z
status: active
severity: low
tags: bug, rhyme, heuristic, sonnet, inconsistency
---

# `shakespeare_sonnet.py` rhyme extractor uses last-3, matcher compares last-2

## What Happened

`scripts/shakespeare_sonnet.py`:

```python
def end_rhyme(line: str) -> str:
    ...
    # Take last 2 letters as rhyme proxy (e.g. "day" / "may" → "ay")
    return w[-3:] if len(w) >= 3 else w          # <-- returns last 3

def rhyme_match(a: str, b: str) -> bool:
    if not a or not b:
        return False
    return a[-2:] == b[-2:]                      # <-- compares last 2
```

The comment on line 51 says *"last 2 letters"*, the code returns last **3**,
and the matcher then slices last **2** off that. Net effect is still a
last-2-char comparison (since slicing `[-2:]` of a 3-char suffix = last 2) —
so the matcher works by accident. But the docstring says one thing, the
return statement computes another, and the matcher re-slices it back.

## Why It Matters

- Confusing for maintenance: the comment, return, and comparison each claim
  a different slice size.
- If someone later "fixes" the comment to match the `[-3:]` return, they'll
  silently break rhyme detection.
- The rhyme heuristic already matches words like "fear" and "war" (both end
  in "ar") — not true rhymes. Reviewer's question in the prompt is whether a
  CMU-dict phoneme rhymer is needed.

## Verdict on CMU-dict

**Not worth it for this demo.** The FSV results show 7/7 pairs matching on
the tested sonnet; the heuristic is adequate for a pitch demo where the
audience sees 14 visibly Shakespearean lines and a green checkmark. Adding
`cmudict` (3.5 MB dep) to prove "day / may actually rhyme" is feature creep.

**But DO clarify the slice sizes.** Either:

```python
def end_rhyme(line: str) -> str:
    ...
    return w[-2:] if len(w) >= 2 else w  # last 2 letters

def rhyme_match(a: str, b: str) -> bool:
    return bool(a) and bool(b) and a == b
```

Or drop the accidental double-slice by returning the full word and doing all
the slicing in `rhyme_match`. Pick one, document it, and stop.

## Related

- Tags: rhyme, heuristic, string-slicing, consistency
