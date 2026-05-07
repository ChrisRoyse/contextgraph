---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:20:00Z
status: active
tags: sft-data, user-prompts, training-distribution, topic-skew, medium-severity
---

# SFT User Prompts Are 95% Shakespeare-Content, 4% Modern Topics (MEDIUM SEVERITY)

## Distribution analysis (7,925 SFT examples)
Classification by user-prompt keywords:

| Category | Count | % |
|----------|-------|----|
| Shakespeare-topic only (kings, characters, plays) | 2,183 | 27.5% |
| Modern-topic only (AI, code, internet, smartphones) | 525 | 6.6% |
| Both mentioned (bridging) | 747 | 9.4% |
| Generic narrative-continuation | 4,470 | 56.4% |

Sub-datasets:
- 323 compositions_clean.jsonl (modern topics only — pizza, neural nets, crypto)
- 43 adversarial_clean.jsonl (style-break refusals — "talk normally", etc.)
- 7,559 raw Shakespeare narrative-continuation (the "Neither/generic" bucket)

## Top-prompt prefixes
Top 4 prefixes account for ~31% of all prompts:
- 652× "What fate awaits the figures in..."
- 643× "What happens to the people in..."
- 639× "Tell me more about these characters"
- 607× "Continue the story of these chars"

All four are Shakespeare narrative-continuation prompts.

## The "write about SCENE..." concern from the DPO earlier
In DPO data, prompts like "Write about: SCENE III. An ante-chamber..." were common.
In SFT FINAL data: 0 such prompts remain (cleaning removed scene-header echoes).

Good — the scene-header echo issue in SFT is fixed. DPO still has them (see dpo-pairs-are-content-not-style.md).

## Impact on style-vs-semantic question
The SFT distribution biases the model toward:
- STYLE: archaic Shakespeare vocabulary (uniform across all 7,925 examples via UNIFIED_SYSTEM)
- TOPIC SKEW: Strongly toward Shakespeare plays' content (kings, love, fate, honour, battles)

What this means:
- For prompts about Shakespeare content ("Tell me of love"), model is DEEPLY trained → reliable style
- For prompts about modern topics ("Explain the internet in Shakespeare"), model is LIGHTLY trained (523+747+323 = ~1,595 mixed/modern/composition = 20% of total) → style may be weaker, compositions may fall into period-noun substitution ("electric fire" for "electricity")

The model's demonstrated capability on modern topics (5/5 FSV PASS including Go code and physics) comes from:
1. Base Gemma 4 E4B's general capability (knows Python, neural nets)
2. 1,595 examples that teach modern-topic-in-Shakespeare behavior
3. Constrained decoder's boost forcing archaic vocabulary even in modern explanations

## Severity: MEDIUM
- Does NOT invalidate the current deployment — 5/5 FSV pass
- DOES mean the model's modern-topic handling is undertrained relative to Shakespeare-content handling
- The style score on modern-topic outputs may be structurally weaker (less use of thee/thou because there's less Shakespeare-content vocabulary to recall)

## Alignment with the new guard
Good news for the guard: it uses E7 Code (syntax patterns), E9 HDC (structural robustness), E10 Multimodal (paraphrase), E12 ColBERT (phrase-level), E13 SPLADE (term-expansion). These capture STYLE even when topic is modern. So the model's potential modern-topic weakness won't fail the guard so long as archaic vocabulary is present.

But the model may START weaker on archaic vocabulary for modern topics — combined with the constrained decoder BLOCKING some modern topic tokens (internet, API) — user sees circumlocution. This is an alignment artifact, not a training failure.

## Fix recommendation
Increase modern-topic compositions from 323 to ~1,000 to close the gap (~12% of dataset instead of 4%). But this is NOT urgent — current performance is acceptable.

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
- See also: ./memory/discoveries/agent-03-sherlock-config--constrained-decoder-has-topic-blocks.md
- See also: ./memory/discoveries/agent-03-sherlock-config--dpo-pairs-are-content-not-style.md
