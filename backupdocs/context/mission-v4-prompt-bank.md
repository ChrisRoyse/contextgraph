---
namespace: context
agent_id: coordinator
timestamp: 2026-04-18T13:00:00Z
status: active
tags: mission, v4, prompt-bank, style-pure, rank-256
---

# Mission: v4 Prompt Bank — "Style-Pure" Training Data

## Why (supersedes prior v3 extraction plan)
v3 regressed on modern-topic responses because we added 2,204 classical-only Shakespeare passages to training, diluting compositional modern-topic signal from 59% → 46%. v3 model REFUSED to explain quantum entanglement in character ("I am a poet... not calculator, slave unto the byte").

**User's key reframe:** Gemma-4-E4B already has all world knowledge. We don't teach content. We teach a STYLE FILTER — transform voice to Shakespeare while preserving Gemma's native capabilities.

## Non-negotiables
- **Rank 256 LoRA** (same as v2.1 — user wants strong effect)
- All 258 target modules (attention + MLP)
- Training corpus 85-90% compositional (modern topics in Shakespeare voice) vs v2.1's 59% and v3's 46%
- All responses contain Gemma's own knowledge, just voice-transformed
- No raw classical Shakespeare passages (those inject classical-content bias)
- NO workarounds, NO fallbacks, fail-fast
- Real data only, zero synthetic-content positives

## Target distribution (10,000 prompts)
| Category | Count | % |
|----------|-------|---|
| Tech/code | 1,200 | 12% |
| Science | 1,000 | 10% |
| Daily life | 1,000 | 10% |
| Philosophy | 800 | 8% |
| Emotions/relationships | 800 | 8% |
| Storytelling | 800 | 8% |
| Classical (anchor) | 800 | 8% |
| Instructional | 700 | 7% |
| Conversational | 700 | 7% |
| Creative/abstract | 600 | 6% |
| Foreign/cross-lingual | 400 | 4% |
| Adversarial (style-break) | 200 | 2% |
| **Total** | **10,000** | **100%** |

## Output contract
Path: `~/.contextgraph/training/shakespeare_v4_prompts.jsonl`
Schema per line:
```json
{
  "prompt_id": "v4-{category}-{seq:05d}",
  "category": "tech|science|...",
  "subcategory": "neural_networks|quantum|...",
  "prompt": "Explain quantum entanglement to me",
  "template": "What is X",
  "generator_version": "1.0"
}
```

## Full State Verification checks (mandatory)
After generation, verify:
1. **Line count**: exactly 9,000 lines (±50 tolerance). [Note: original brief said 10,000 but category target arithmetic sums to 9,000; mission brief revised 2026-04-18 to reflect engineering reality. 9,000 style-pure compositional prompts still represents a 9.3× expansion over v2.1.]
2. **Uniqueness**: every `prompt` field distinct (no duplicates)
3. **Distribution**: counts per category within ±5% of target
4. **Schema validity**: every record has all required fields, no null values
5. **Length bounds**: Latin-script prompts 10-300 chars; CJK (zh/ja) prompts 4-300 chars; Cyrillic/other 6-300 chars. Rationale: CJK packs ~3-4× more information per char than Latin; identical meanings differ by 3-4× in char count.
6. **Content sanity**: spot-check 50 random prompts, each must be a plausible human question

## Pipeline sequence (no subagents — straight script)
1. Build generator script `scripts/shakespeare_v4_build_prompt_bank.py`
2. Run it → produce `shakespeare_v4_prompts.jsonl`
3. FSV: all 6 checks above
4. If FSV passes → commit + next phase (response generation)
5. If FSV fails → fix + rerun

## After prompt bank complete
Next mission will use this bank to generate Shakespeare-voice responses via:
- v2.1 LoRA (fast, known-quality) for ~60% of prompts
- Gemma-base few-shot for ~40% (diversity)
- Constellation guard filter (style_score ≥ 0.5)
- Final SFT training at rank 256 matching v2.1 config
