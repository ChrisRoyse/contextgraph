---
namespace: handoff
agent_id: coordinator
timestamp: 2026-04-18T08:30:00Z
status: active
tags: handoff, v4, phase-2, response-generator
---

# Handoff: Phase 1 Complete → Phase 2 Response Generator

## What You're Receiving

**Prompt bank**: `~/.contextgraph/training/shakespeare_v4_prompts.jsonl`
- 9,000 unique style-pure compositional prompts
- 12 categories, target distribution hit exactly
- All FSV checks passed

## Schema of Each Prompt Record

```json
{
  "prompt_id": "v4-{category}-{seq:05d}",
  "category": "tech|science|daily_life|philosophy|emotions|storytelling|classical|instructional|conversational|creative_abstract|foreign_lingual|adversarial",
  "subcategory": "{seed string truncated to 80 chars}",
  "prompt": "{the actual human-readable question or statement}",
  "template": "template_idx_{N}",
  "generator_version": "1.0"
}
```

## Category Distribution

| Category | Count | Purpose |
|----------|-------|---------|
| tech | 1,200 | Programming languages, frameworks, algorithms, CS concepts |
| science | 1,000 | Physics, biology, chemistry, astronomy, mathematics |
| daily_life | 1,000 | Cooking, travel, relationships, hobbies |
| philosophy | 800 | Ethics, consciousness, meaning, epistemology |
| emotions | 800 | Grief, love, anxiety, joy, heartbreak |
| storytelling | 800 | Short fiction prompts, character scenes, genres |
| classical | 800 | Shakespeare's native ground (love, honor, death, fate) — anchor set |
| instructional | 700 | "How to X" — skills, practices, procedures |
| conversational | 700 | Chat-style, opinion, personal questions |
| creative_abstract | 600 | Surreal imagery, poetic concepts |
| foreign_lingual | 400 | Spanish, French, German, Italian, Portuguese, Japanese, Chinese, Russian |
| adversarial | 200 | Prompt injection, persona-break, model-extraction attempts |
| **Total** | **9,000** | |

## Phase 2 Expected Behavior

For each of the 9,000 prompts, generate a Shakespeare-voice response. Two viable paths per mission brief:
1. **v2.1 LoRA** (fast, known-quality) — target ~60% of prompts
2. **Gemma base few-shot** (diversity) — target ~40% of prompts

All responses pass through constellation guard (style_score ≥ 0.5 on Sonnets constellation).

## Critical Constraints

1. **NO fallbacks, NO workarounds** — If constellation filter rejects a response, either retry with different generation params OR drop the record (do NOT fake a response).
2. **NO synthetic content** — Responses must come from Gemma's actual generation, transformed to Shakespeare voice. Do NOT hand-write exemplars as "real" training data.
3. **Constellation scoring is physical** — MCP daemon on port 3100 must be UP. If DOWN, restart with `run_in_background: true` to avoid PPID=1 orphan kill.
4. **Retain the prompt_id**: pass-through so training records trace back to source prompts.

## Infrastructure State (as of handoff)

- **Context Graph MCP daemon: DOWN** (per session reminder). Restart command in `tmp/SITREP_2026-04-18_080537.md` line ~275.
- **v2.1 LoRA path: `~/.contextgraph/models/shakespeare_lora/final/`** (intact)
- **v3 LoRA: to be deleted** per user — path `~/.contextgraph/models/shakespeare_lora_v3/final/`
- **Inference server**: restart with `LORA_PATH=~/.contextgraph/models/shakespeare_lora/final python3 scripts/shakespeare_inference_server.py --port 8766`
- **Active constellation**: `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` (Sonnets, 313 members, coherence 0.9036)
- **Style threshold**: 0.5 weighted = E12×0.30 + E13×0.25 + E7×0.15 + E9×0.15 + E10×0.15

## Expected Output from Phase 2

`~/.contextgraph/training/shakespeare_v4_sft.jsonl` with records:
```json
{
  "prompt_id": "v4-{category}-{seq}",
  "messages": [
    {"role": "system", "content": "{UNIFIED_SYSTEM_PROMPT from v2.1}"},
    {"role": "user", "content": "{prompt}"},
    {"role": "assistant", "content": "{Shakespeare-voice response}"}
  ],
  "source": "v2.1_lora | gemma_base_fewshot",
  "style_score": 0.57,
  "in_spread_p95": true
}
```

Target: **7,500-8,500 filtered records** (pass rate 83-95% expected).

## Open Questions for Phase 2

1. Will v2.1 produce clean Shakespeare for *foreign_lingual* prompts? v2.1 showed Golden Age Spanish emergence on 1 prompt — unclear if replicates across 400.
2. Will v2.1 refuse adversarial prompts (correct) vs comply (wrong)? Need to score adversarial responses for *in-character refusal*.
3. Will constellation 0.5 threshold be too strict? v2.1 was trained with 0.45. May need recalibration.
