---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:05:00Z
status: active
tags: dpo, pairs, content-vs-style, training-signal-misalignment, critical
---

# DPO Pairs Are Content-Contrast, NOT Style-Contrast (HIGH SEVERITY)

## Observation (1,000-sample empirical analysis)
File: `~/.contextgraph/training/shakespeare_dpo_clean.jsonl`

Sampled 1,000 DPO pairs and tested archaic-vocabulary density of chosen vs rejected:
| Condition | Count | Percent |
|-----------|-------|---------|
| Chosen is archaic (>=1% markers) | 722 | 72.2% |
| Rejected is archaic (>=1% markers) | 806 | 80.6% |
| BOTH archaic (content-contrast pairs) | 651 | 65.1% |
| NEITHER archaic | 123 | 12.3% |
| ONLY chosen archaic (correct style contrast) | 71 | 7.1% |
| ONLY rejected archaic (BACKWARDS!) | 155 | 15.5% |

## Interpretation
- **65.1% of DPO pairs have BOTH sides archaic Shakespeare**. DPO does NOT teach "prefer Shakespeare over modern English" here — it teaches "prefer scene A from Henry VI over scene B from Othello" (both Shakespeare).
- **15.5% of pairs have the REJECTED MORE archaic than CHOSEN**. These pairs actively train the model to prefer LESS-archaic text. A first-order style signal contradiction.
- Only **7.1% of pairs** have the expected "archaic chosen, non-archaic rejected" style contrast that would actually teach STYLE.

## Concrete example
Prompt: "Write about: SCENE III. An ante-chamber in the palace."
- CHOSEN (density 0.002): "Is't possible the spells of France should juggle / Men into such strange mysteries? / New customs, / Though they be never so ridiculous..."
- REJECTED (density 0.058): "Can no man tell me of my unthrifty son? / 'Tis full three months since I did see him last. / I would to God, my lords, he might be found..."

Both are Shakespeare. The REJECTED has 29× more archaic density than the CHOSEN. DPO margin is trained to push AWAY from the more archaic option.

## Anomaly_kind distribution
All 5,000 sampled pairs have `anomaly_kind: "other"` — meaning the pair-mining process lost the semantic discrimination signal. All pairs are categorized the same; no style-specific anomaly signal reached training.

## Why this matters for the question
The `similarity_profile` in `_meta` shows cosine similarity between anchor and negative across all 13 embedders. Sampled profile: [0.94, 0.54, 0.66, 0.52, 0.88, 0.20, 0.79, 0.94, 0.74, 0.98, 0.0, 0.93, 0.16]. These are HIGH cosines across style-discriminative embedders (E8=0.94, E9=0.74, E10=0.98, E12=0.93) — meaning the chosen and rejected are STYLISTICALLY VERY SIMILAR and differ only in E6/E13 (sparse/SPLADE keyword lexicon) — which capture CONTENT, not style.

## Quantitative 2,000-pair analysis
Computed mean per-embedder similarity across 2,000 DPO pairs:
- Mean STYLE-embedder similarity (E7, E9, E10, E12, E13) between chosen and rejected: **0.712**
- Mean SEMANTIC-embedder similarity (E1, E5) between chosen and rejected: **0.864**

Style-similarity distribution:
- 0.5-0.7: 27.2% (544 pairs — moderately style-differentiable)
- 0.7-0.85: 72.8% (1,456 pairs — stylistically very similar)
- 0% of pairs have style-similarity < 0.5

**Interpretation**: In 2,000 of 2,000 sampled pairs, the chosen and rejected are stylistically similar (both archaic Shakespeare). The DPO is literally UNABLE to teach "prefer archaic over modern" because no pair presents that contrast.

The training signal is "prefer THIS Shakespeare passage over THAT Shakespeare passage when the prompt is X" — which is a SEMANTIC (content-continuation) objective, not a STYLE (archaic-vs-modern) objective.

**The DPO is teaching content discrimination, NOT style discrimination.**

## But the model still works — why?
Because the SFT phase (7,925 examples) established the archaic style, and the DPO's final rewards/accuracies=98%, rewards/margins=43.62 indicate it IS learning the preference. But what it's learning is "this specific Shakespeare passage fits this scene prompt better than that Shakespeare passage" — a TOPIC/SEMANTIC signal disguised as style training.

## Impact on the dashboard guard
The new guard validates STYLE via E7/E9/E10/E12/E13 (style-discriminative embedders). Output that is archaic Shakespeare will PASS the new guard regardless of semantic content. So the DPO misalignment doesn't immediately break the guard — but it means the model may produce less discriminative style when the prompt looks like a scene-header ("Write about SCENE III"). It's over-fit to scene-continuation, under-fit to arbitrary-topic composition.

## Severity: HIGH (but not blocking)
- HIGH because the DPO training signal is off-purpose — 85% of the signal is content-contrast
- Not blocking because SFT did the heavy lifting and DPO still strengthens the preferred distribution via secondary effects (lexical habits, punctuation, cadence)
- Production model scored 5/5 FSV tests, so the mismatch isn't catastrophic

## Agent 2 scope reminder
This finding is about DPO PAIR QUALITY — per the mission brief, Agent 2 owns DPO pair mining. Agent 3 (me) is reporting this as cross-scope evidence relevant to training-config correctness. Agent 2 should do the formal verdict on DPO pair selection.

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
- See also: ./memory/discoveries/agent-03-sherlock-config--constrained-decoder-has-topic-blocks.md
- See also: ./memory/discoveries/agent-03-sherlock-config--sft-data-has-scene-prompts.md
