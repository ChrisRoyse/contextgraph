---
namespace: discoveries
agent_id: agent-06-coder-sft-aug
timestamp: 2026-04-17T17:10:00Z
status: active
severity: low
tags: target-count, discrepancy, phase-d, planning
---

# Target count mismatch: spec says 1,500+ but sub-counts sum to 1,350

## What I Found
The Phase D task spec says:
> Your work will produce **1,500+** new high-quality SFT examples

and then breaks down per category:
- Modern-topic compositions: 750 (200+150+100+100+80+60+60)
- Emotional register diversity: 300 (50×6)
- Forms & shapes: 200 (50×4)
- Cross-lingual preservation: 100 (50+30+20)
- **Sum: 1,350**

Plus 150 adversarial (separate file, DPO, not counted in SFT).

## Decision
I honour the per-category targets as written. `CATEGORY_TARGETS` sums to 1,350 and
the script's assertion matches. Agent 7 (SFT retrain) can decide whether to top up
another 150 examples after review — easier than silently inflating a category the
coordinator picked for balance.

## Why It Matters for Next Agents
- Agent 7 (SFT retrain): corpus = 7,925 (existing clean) + ~1,350 (Phase D kept)
  = ~9,275 SFT examples. If the spec's "1,500+" was a hard number, inform the
  coordinator so they can either (a) raise one category's target, or (b) accept
  the 1,350 count.
- Rejection during Phase 3 constellation scoring will further reduce this to
  whatever fraction passes `style_score >= 0.45`. Expected keep rate: 70-85%
  based on how well base Gemma produces Shakespeare voice from few-shot anchors.

## Related
- See also: ./memory/context/mission-resurrection-plan.md
- Tags: target-count, sft-size, phase-d, planning-variance
