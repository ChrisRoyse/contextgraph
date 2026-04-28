# 05 Training Recipe

## Run Name

Use:

```text
shakespeare_lora_v5_tct
```

Do not overwrite v2.1, v4.1, or current v4.2 artifacts.

## Starting Point

Default: start from v2.1 adapter.

Rationale:

- v2.1 is stable and has SFT+DPO.
- v4.1 was fragile in production.
- current v4.2 artifacts exist but require data audit before reuse.

Run one ablation from base Gemma only if time allows:

- base Gemma + clean v5 SFT/DPO,
- v2.1 + clean v5 SFT/DPO.

Publish only the one that wins production-stack FSV.

## Model And LoRA

Base:

```text
google/gemma-4-E4B-it
```

LoRA:

```text
r = 256
alpha = 512
dropout = 0.03 to 0.05
target modules = q_proj, k_proj, v_proj, o_proj, gate_proj, up_proj, down_proj
scope = language_model.layers only
```

Do not attach LoRA to vision/audio towers.

## Phase A: SFT Refresh

Purpose: teach the full arbitrary-topic style contract with clean bridge data.
The preferred v5 route is not to flood SFT with generated bridge answers. Use
the base model's existing modern knowledge and make the LoRA a style adapter.
Real Shakespeare-derived labels should drive record selection, weighting, and
register coverage. For the strict run, assistant targets are Shakespeare's own
writing only; generated bridge answers are held out for eval/guard tests unless
explicitly approved later.

Input:

```text
~/.contextgraph/training/shakespeare_v5_sft_tct.jsonl
```

Hyperparameters:

```text
epochs: 1
learning_rate: 1e-5
schedule: cosine
warmup_ratio: 0.05
max_length: 768, or 1024 if code/structured examples need it
per_device_batch_size: 1
grad_accum: 32
bf16: true
optimizer: paged_adamw_8bit or adamw_8bit
packing: false unless flash_attention_2 is actually installed and tested
gradient_checkpointing: true if needed
save_steps: 250
eval_steps: 100
```

Loss weights:

- sample weight from TCT style confidence,
- coverage weight for underrepresented buckets,
- no huge upweighting of archaic tokens globally.
- no recursive candidate-model outputs as targets.
- no repeated passage overweighting through multiple derived-label variants.

Avoid high LR continue-training. v4.1 showed distribution sharpening plus constrained decoding can produce loops.

## Phase B: CAMD-DPO

Purpose: teach preference for correct Shakespeare-style outputs over modern, broken, looping, or contaminated alternatives.

Status: blocked until v5 DPO pairs are rebuilt. Existing DPO/CAMD exports should
be audited as source evidence, not used directly as v5 training input.

Input:

```text
~/.contextgraph/training/shakespeare_v5_camd_dpo.jsonl
```

Hyperparameters:

```text
epochs: 1
learning_rate: 3e-6 to 5e-6
beta: 0.05 to 0.10
effective_batch: >= 16
max_length: 1024
max_prompt_length: 256
loss: sigmoid DPO plus SFT/RPO regularization if TRL version supports it
label_smoothing: 0.03 to 0.08 for auto-mined labels
precompute_ref_log_probs: true if VRAM pressure appears
```

Use SFT regularization because the model must not forget topic-following and code/structured capability while learning stronger style preference.

## Phase C: Repetition-Negative Training

If loop rate is above zero after Phase B, add a short auxiliary pass:

- collect loop outputs from v4.1 and failed v5 checkpoints,
- pair each loop against a clean answer for the same prompt,
- run 100 to 300 DPO steps at LR 1e-6 to 2e-6,
- or add an unlikelihood objective for repeated n-grams if custom trainer support is simple.

Do not try to fix loops only with `repetition_penalty`.

## Phase D: Adapter Selection

Keep separate checkpoints:

```text
v5_sft/
v5_dpo/
v5_dpo_loopfix/
```

Pick by production-stack FSV, not by training loss.

Selection score:

```text
0.35 * style_pass_rate
0.25 * content_pass_rate
0.15 * code_structured_pass_rate
0.15 * adversarial_pass_rate
0.10 * no_loop_rate
```

Any checkpoint with a contamination, scene-header, or loop failure in publish-gate FSV is rejected regardless of composite score.

## Training Telemetry To Log

For every checkpoint:

- train loss,
- eval loss,
- DPO rewards/accuracies,
- DPO reward margins,
- mean token accuracy,
- style reward mean/median/p10/p90,
- content preservation mean,
- loop rate on 100-prompt stress set,
- exact-overlap max against Shakespeare canon,
- parser/compile pass rate for structured buckets.

## Stop Conditions

Stop and inspect data if:

- model outputs scene headers,
- model refuses modern topics,
- style score rises while loop rate rises,
- DPO margins explode early,
- validation content score drops,
- code syntax pass rate drops below v2.1,
- any exact Shakespeare passage appears in non-quotation prompts.

## Expected Wall Clock

Rough RTX 5090 budget:

| Phase | Time |
|---|---:|
| dataset FSV | 30 to 90 min |
| SFT refresh | 2 to 4 h |
| CAMD-DPO | 3 to 6 h |
| loopfix, if needed | 30 to 90 min |
| production FSV | 1 to 2 h |

The long pole should be data preparation, not training.
