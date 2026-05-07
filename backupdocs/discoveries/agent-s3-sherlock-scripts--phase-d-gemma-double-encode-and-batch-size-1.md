---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:12:00Z
status: active
severity: high
tags: phase-d, gemma, batching, throughput, vram, torch
---

# Phase D generates at batch_size=1 and never releases kv-cache between records

## Evidence
`scripts/shakespeare_sft_augmenter.py:2088-2119`
```python
def generate(self, system: str, user: str, *,
             temperature: float = DEFAULT_TEMP,
             max_new_tokens: int = GEN_MAX_NEW_TOKENS) -> str:
    ...
    inputs = self._tokenizer.apply_chat_template(...)
    attention_mask = torch.ones_like(inputs)
    with torch.inference_mode():
        out = self._model.generate(
            inputs,
            attention_mask=attention_mask,
            max_new_tokens=max_new_tokens,   # 320
            do_sample=True, temperature=float(temperature),
            top_p=0.92, repetition_penalty=1.1,
            pad_token_id=self._tokenizer.eos_token_id,
        )
    new_tokens = out[0, inputs.shape[1]:]
    text = self._tokenizer.decode(new_tokens, skip_special_tokens=True).strip()
    return text
```

And the caller loops one-at-a-time:
`scripts/shakespeare_sft_augmenter.py:2409-2449` (SFT candidates)
`scripts/shakespeare_sft_augmenter.py:2494-2546` (adversarial)

## Problems

### 1. No del/empty_cache between records
`out` and `inputs` tensors remain referenced by local variable name until
next loop iteration rebinds. On a RTX 5090 with ~11.5GB Gemma base, one
320-token generation produces a kv-cache of ~(N_layers × 2 × batch × heads
× seq × head_dim × 2 bytes) ≈ 40-80MB. Python re-binding frees the old
on next call, but fragmentation accumulates. For 1,350 generations this
is visible VRAM pressure.

**Fix**: after `return text`, consider:
```python
del out, inputs, attention_mask, new_tokens
# once every N calls:
if self._call_count % 32 == 0:
    torch.cuda.empty_cache()
```

### 2. Batch size 1 is 10-30x slower than needed
Gemma-4-E4B on a 5090 can easily batch 4-16 generations. 1,500 sequential
calls at ~2-5s each = 50-125 minutes. Batched at 8 = 6-15 minutes.

This isn't a correctness bug, but it keeps Gemma resident in VRAM
longer, widening the crash window. Faster batched generation means less
time at peak VRAM.

### 3. `attention_mask = torch.ones_like(inputs)` is redundant and wrong
`apply_chat_template` with `return_tensors="pt"` returns token IDs, not
a dict; `torch.ones_like` of int tensor gives an int tensor — which
works but throws unnecessary casts. Also: if the tokenizer ever left
padding, this mask ignores the pad positions. Low severity (single
message at a time), but brittle.

### 4. `pad_token_id=self._tokenizer.eos_token_id` without checking
Some Gemma tokenizers lack distinct pad; re-using EOS is common but
means EOS sampling triggers padding — fine for decoder-only but should
be asserted explicitly.

## Fix Recommendation
Rewrite `generate` as `generate_batch(list[tuple[sys,user]]) -> list[str]`:
```python
def generate_batch(self, prompts: list[tuple[str, str]], *,
                   temperature: float = DEFAULT_TEMP,
                   max_new_tokens: int = GEN_MAX_NEW_TOKENS) -> list[str]:
    import torch
    msgs = [[{"role": "system", "content": s}, {"role": "user", "content": u}]
            for s, u in prompts]
    tokenized = [self._tokenizer.apply_chat_template(
        m, add_generation_prompt=True, return_tensors="pt",
        tokenize=True) for m in msgs]
    # Pad to max length in batch
    maxlen = max(t.shape[1] for t in tokenized)
    pad_id = self._tokenizer.pad_token_id or self._tokenizer.eos_token_id
    assert pad_id is not None, "tokenizer needs a pad or eos token"
    batch_ids = torch.full((len(tokenized), maxlen), pad_id, dtype=tokenized[0].dtype)
    batch_mask = torch.zeros_like(batch_ids)
    for i, t in enumerate(tokenized):
        L = t.shape[1]
        batch_ids[i, -L:] = t[0]        # left-pad
        batch_mask[i, -L:] = 1
    batch_ids = batch_ids.to(self._model.device)
    batch_mask = batch_mask.to(self._model.device)
    with torch.inference_mode():
        out = self._model.generate(
            batch_ids, attention_mask=batch_mask,
            max_new_tokens=max_new_tokens, do_sample=True,
            temperature=float(temperature), top_p=0.92,
            repetition_penalty=1.1, pad_token_id=pad_id,
        )
    replies = []
    for i, t in enumerate(tokenized):
        new_tokens = out[i, batch_ids.shape[1]:]
        replies.append(self._tokenizer.decode(new_tokens, skip_special_tokens=True).strip())
    del out, batch_ids, batch_mask
    torch.cuda.empty_cache()
    return replies
```

Use batch_size=4 conservatively (Gemma-4-E4B on 5090 allows more but 4
leaves plenty of margin for concurrent daemon work).

## Related
- Tags: phase-d, gemma, throughput, vram
