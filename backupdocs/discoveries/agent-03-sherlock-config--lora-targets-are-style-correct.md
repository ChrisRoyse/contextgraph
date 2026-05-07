---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:00:00Z
status: active
tags: lora, target-modules, style-correctness, adapter-config, training-config
---

# LoRA Target Modules ARE Style-Correct

## Primary Evidence
File: `/home/cabdru/.contextgraph/models/shakespeare_lora/final/adapter_config.json`
- `r: 256`, `lora_alpha: 512` (line 19, 29)
- 258 target_modules covering all 42 layers of Gemma 4 E4B's language_model
- Layers 0-23: FULL coverage — q_proj, k_proj, v_proj, o_proj + gate_proj, up_proj, down_proj (7 modules each × 24 layers = 168 modules)
- Layers 24-41: PARTIAL attention — q_proj, o_proj only + full MLP (5 modules each × 18 layers = 90 modules)
- No k_proj/v_proj in layers 24-41 because Gemma 4 uses grouped-query attention (GQA) in higher layers; k_proj/v_proj become group-shared, so PEFT's module scan doesn't find them as distinct modules per layer

Total breakdown:
- 132 attention modules (51.2%) → governs SYNTACTIC patterns / rhythm
- 126 MLP modules (48.8%) → governs vocabulary / lexical association
- Roughly 50/50 balance — good coverage for STYLE

## Verdict
The training targeted both attention (style) AND MLP (vocabulary) modules across all 42 language_model layers. This is the recipe for DEEP style transfer, not shallow vocabulary-swap or topic-semantic alignment. Rank 256 / alpha=2×rank is the standard high-capacity configuration for overriding base model behavior.

## Why this is correct for STYLE
- **Attention modules (q/k/v/o)** govern how tokens attend to prior tokens — this captures Shakespeare's SYNTAX (inverted sentences, polyptoton, caesura placement). Targeting these is ESSENTIAL for style.
- **MLP modules (gate/up/down)** govern per-token transformations — this captures lexical choices (thou vs you, doth vs does). Essential for archaic vocabulary.
- Both together = full style coverage across 42 layers = dominant style signal

## Implication
If the model FAILS a style test, it is NOT because the LoRA configuration was topic-biased — the configuration is balanced and comprehensive. Any failure must be explained elsewhere (DPO pair composition, system prompt, decoder).

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--dpo-pairs-are-content-not-style.md
- See also: ./memory/discoveries/agent-03-sherlock-config--constrained-decoder-has-topic-blocks.md
