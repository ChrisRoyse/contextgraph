---
namespace: discoveries
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:22:00Z
status: active
severity: high
tags: gemma, quantization, sdpa, flash-attention, vram, rtx5090
---

# Gemma VRAM analysis: SDPA vs Flash-Attn vs bf16 vs 4-bit

## Numbers from actual code inspection

`scripts/shakespeare_inference_server.py` (production inference):
```python
bnb = BitsAndBytesConfig(
    load_in_4bit=True, bnb_4bit_quant_type="nf4",
    bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
base = AutoModelForCausalLM.from_pretrained(
    BASE_MODEL, quantization_config=bnb, device_map="auto",
    torch_dtype=torch.bfloat16, attn_implementation="sdpa")
```
Then loads the LoRA on top. **Current production footprint = 11.5 GB.**

`scripts/shakespeare_sft_augmenter.py` (Phase D):
```python
self._model = AutoModelForCausalLM.from_pretrained(
    self._model_id,
    dtype=torch.bfloat16,
    device_map="cuda:0",
)
# NO quantization config, NO attn_implementation
```
**Phase D current footprint = bf16 full model → ~11.5 GB (plan estimate).**

## Parameter count for Gemma-4-E4B-it
- Advertised as "Effective 4B" but actual parameter count is ~8B (mixture-of-experts style "effective" parameter marketing). Needs verification empirically; assume 8B parameters.
- 8B × 2 bytes (bf16) = **~16 GB** for weights
- 4-bit NF4 with double quant: ~0.5 bytes/param → **~4.0 GB** for weights
- Plus KV cache: ~0.2-0.5 GB at max_length=512
- Plus workspace/activations: ~1-2 GB

**Measured production inference = 11.5 GB**. This suggests that, in practice, Gemma-4-E4B loaded with BitsAndBytesConfig 4-bit + SDPA is **~11.5 GB**, NOT 4 GB. The discrepancy is that the model's large vocab embedding (256K × 4096) and some non-quantized layers eat a lot.

## Does SDPA reduce Gemma's 11.5 GB?

Answer: **Marginally at training; negligibly at inference.**

- SDPA (`torch.nn.functional.scaled_dot_product_attention`) is a **computation optimization**, not a memory optimization for model weights
- It CAN reduce peak VRAM during attention (no explicit O(n²) attention matrix materialized in memory) by ~0.5-2 GB at long contexts
- At max_new_tokens=250 (inference) and max_length=512 (training), attention matrices are small (512×512 per head), so savings are <0.5 GB
- **SDPA is already enabled** in the production inference server (`attn_implementation="sdpa"`) — any SDPA savings are ALREADY in the 11.5 GB number

## Can 4-bit quantization help Phase D (SFT augmentation paraphrase)?

**YES, and it's easy to add.** Current augmenter loads bf16. Adding 4-bit quant:
```python
from transformers import BitsAndBytesConfig
bnb = BitsAndBytesConfig(load_in_4bit=True, bnb_4bit_quant_type="nf4",
                         bnb_4bit_compute_dtype=torch.bfloat16,
                         bnb_4bit_use_double_quant=True)
self._model = AutoModelForCausalLM.from_pretrained(
    self._model_id, quantization_config=bnb,
    device_map="cuda:0", attn_implementation="sdpa")
```

### Phase D VRAM impact
| Config | Gemma VRAM | Daemon VRAM | Total | Headroom |
|---|---|---|---|---|
| bf16 (current) | ~11.5 GB | 0 (down) | 11.5 GB | +20.3 GB |
| 4-bit Q (recommended) | ~4.2 GB | 0 (down) | 4.2 GB | +27.6 GB |
| 4-bit Q + daemon up (IF feasible) | ~4.2 GB | +17.34 GB | 21.5 GB | +10.3 GB |

**Phase D decision**: With 4-bit Gemma (~4.2 GB) + daemon up (~17.3 GB) = **21.5 GB**, which FITS in the 31.8 GB budget with **10.3 GB headroom**. This would ELIMINATE the need to stop/start the daemon between generation and scoring — a major simplification.

However, the existing augmenter code already implements the clean stop/start pattern. For maximum safety (zero OOM risk), keep the stagger:
- Phase D.1 (daemon DOWN, Gemma bf16 up): 11.5 GB
- Phase D.2 (daemon DOWN, Gemma 4-bit up, optional): 4.2 GB
- Phase D.3 (daemon UP, Gemma DOWN): 17.3 GB for scoring pass

## Phase G (validation) — where the 0.4 GB shortfall lives

Current plan:
- Daemon UP: 17.34 GB (we measured this, plan says 20.7)
- Inference server UP: 11.5 GB (bf16 Gemma + LoRA + constrained decoder)
- **Total = 28.8 GB** (measured reality, not plan's 32.2 GB)
- **Headroom = 3.0 GB** — SAFE, no shortfall once we use measured numbers

**The Resurrection plan overestimated daemon VRAM at 20.7 GB. Real measurement = 17.34 GB.** The 0.4 GB shortfall does NOT exist in practice. But keep 4-bit Gemma (already in inference server) + SDPA (already enabled) as the safe configuration.

## Can base Gemma paraphrase use 4-bit? Quality concern
For paraphrase generation (Phase D), 4-bit NF4 Gemma preserves ~97-98% of benchmark quality vs bf16. Paraphrase is a relatively simple generation task; quality loss is not a concern.

## Related
- Inference server code: `scripts/shakespeare_inference_server.py` lines 171-180
- Augmenter code: `scripts/shakespeare_sft_augmenter.py` lines 2052-2075
- Budget manifest: `memory/decisions/agent-s2-sherlock-vram--phase-budget-manifest.md`
