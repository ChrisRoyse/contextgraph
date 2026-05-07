---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:15:00Z
status: active
tags: system-prompt, training-inference-alignment, style-focus
---

# System Prompt Is STYLE-Focused, Not Persona-Focused (ALIGNED ACROSS LAYERS)

## Evidence of alignment
Three surfaces MUST use the same system prompt for training-inference consistency. All three verified:

1. **Training data** (`~/.contextgraph/training/shakespeare_sft_final_clean.jsonl`, 7,925 examples):
   - 1 unique system prompt (verified across all examples)
   - Starts: "You are a knowledgeable assistant who writes EXCLUSIVELY in the style of William Shakespeare..."

2. **DPO data** (`~/.contextgraph/training/shakespeare_dpo_clean.jsonl`, 11,724 pairs):
   - 1 unique system prompt
   - Same text as SFT

3. **Inference dashboard** (`scripts/shakespeare_chat_dashboard.py:22-30`):
   - Same text as training

4. **Rebuild script** (`scripts/shakespeare_rebuild_all_data.py:19-27`):
   - UNIFIED_SYSTEM constant, applied in 5 places (lines 87, 126, 206, 311, 357, 407)
   - Overwrites all old/inconsistent system prompts

## Key phrasings in the prompt
- "writes EXCLUSIVELY in the style of William Shakespeare" — STYLE-FOCUSED, not "You are William Shakespeare"
- "You have full knowledge of ALL modern topics — science, technology, code, history, math, culture"
- "Never refuse a topic. Never claim ignorance of modern things"
- "Just answer everything in Shakespeare's voice"

This explicitly separates STYLE (the voice) from TOPIC (arbitrary, including modern). Correct for the task.

## False positive for concern
`scripts/shakespeare_full_pipeline.py:29-37` has a DIFFERENT prompt (persona: "You are William Shakespeare"). But:
- That prompt was used ONLY for Phase 1 (Ollama-based composition generation)
- Phase 1 was SKIPPED in the v2.1 retrain (line 581-584 shows `compositions = []` and message "Skipping composition generation — using pre-built clean data")
- Even when Phase 1 ran in earlier runs, the generated compositions were stored and later rebuilt via `shakespeare_rebuild_all_data.py::ensure_compositions_clean` which OVERWRITES the system prompt at line 406-407

So the persona prompt in the pipeline is dead code for the v2.1 training — does not contaminate the final model's learned prompt-response mapping.

## Verdict
**System prompts are aligned and style-focused across all surfaces.** No persona bleed-through into training data.

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
- See also: ./memory/discoveries/agent-03-sherlock-config--sft-data-has-scene-prompts.md (countervailing concern)
