---
namespace: discoveries
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:25:00Z
status: active
tags: loss-function, training-config, standard-cross-entropy, style-weighted, observation
---

# Training Uses Standard Cross-Entropy (NOT Style-Weighted)

## Evidence
Files: `scripts/shakespeare_full_pipeline.py`, `scripts/shakespeare_train_lora.py`

### SFT phase
Uses `SFTTrainer` from TRL with default loss. Configuration:
```python
training_args = SFTConfig(
    per_device_train_batch_size=1,
    gradient_accumulation_steps=32,
    num_train_epochs=3,
    learning_rate=2e-4,
    ...
)
```
No custom `compute_loss` override. This means:
- Loss = standard causal-LM cross-entropy
- Every predicted token contributes equally to the loss
- No higher weight on archaic-style tokens (thou, doth, hark) vs common tokens (the, and, is)
- No lower weight on tokens that fall outside the Shakespeare distribution

### DPO phase
Uses `DPOTrainer` from TRL with default DPO loss:
```python
training_args = DPOConfig(
    beta=0.1,  # Standard DPO temperature
    ...
)
```
beta=0.1 is the standard value (not aggressive). Loss compares the log-probability ratio of chosen vs rejected.

## Implication for style-vs-semantic
A style-weighted loss would BOOST the gradient for archaic-vocabulary tokens:
```python
# Hypothetical style-weighted loss
weights = 1.0 + 2.0 * (tokens_in_archaic_vocab)  # 3× weight on style tokens
loss = -(weights * log_prob_of_target).mean()
```
This is NOT used. The model learns style via:
1. Data distribution (7,925 archaic-voice examples in SFT)
2. Uniform cross-entropy — which works because the target vocabulary is archaic

This is CORRECT design: if all training data is archaic Shakespeare, standard cross-entropy IS teaching style-correct token prediction. No custom loss needed — the SIGNAL is in the data, not the loss function.

## Token accuracy proves this works
SFT final state: token accuracy = 0.9910 (99.1%)
The model correctly predicts the NEXT token 99.1% of the time on training data. Since all training targets are archaic Shakespeare, this means the model has learned WHICH token (archaic vs modern) comes next with high accuracy.

## DPO beta evaluation
beta=0.1 is standard for DPO. No custom temperature tricks.
- Final rewards/accuracies: 98.0% (correctly prefers chosen over rejected)
- Final rewards/margins: 43.62 (very strong preference)

Interpretation: DPO is learning to prefer chosen over rejected with near-perfect accuracy. But WHAT it's learning depends on HOW chosen and rejected differ — which per my other discovery, is MOSTLY content-similarity to anchor, NOT style. So the 98% accuracy and 43.62 margin tell us the model can distinguish the pairs — but that distinction is mostly TOPIC-based.

## No eval metric for style
Neither training phase has a custom eval metric. No `compute_metrics` function. No per-stage style validation.
- Only loss and token_accuracy tracked
- No "archaic-token-prediction accuracy" metric
- No holdout style validation set

This is a MINOR gap. The trainer does not quantify "is the model getting more Shakespeare-stylish over time?" — just "is the loss going down?". Since loss is cross-entropy on Shakespeare targets, it conflates "getting more Shakespeare" with "getting better at predicting the next training token".

## Verdict
- Loss function: standard, no style weighting → CORRECT given the data distribution
- DPO beta: standard 0.1 → CORRECT
- Eval metrics: no style-specific metric → MINOR GAP but not blocking

## Related
- See also: ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
- See also: ./memory/discoveries/agent-03-sherlock-config--dpo-pairs-are-content-not-style.md
