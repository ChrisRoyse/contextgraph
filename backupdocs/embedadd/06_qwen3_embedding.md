# Phase 4 — Qwen3-Embedding-0.6B (E18)

**Model:** `Qwen/Qwen3-Embedding-0.6B`
**Architecture:** **Qwen2.5 decoder-only** transformer (NEW FAMILY)
**Output:** 1024D dense (last-token pooling, post task-prompt)
**Context:** 32,768 tokens
**Parameters:** 596M

## Why this is the hardest phase

Every prior embedder in the codebase is an **encoder** (BERT/RoBERTa-family).
Qwen3 is a **decoder** — causal attention, RMSNorm, SwiGLU FFN, grouped-query
attention, rotary position embeddings.

No existing code paths can be reused beyond the generic candle primitives
(tensor ops, CUDA kernels). The entire forward pass is written from scratch.

## Architectural novelties (vs BERT/RoBERTa family)

| Feature | BERT/RoBERTa (existing) | Qwen3 | Impact |
|---------|--------------------------|-------|--------|
| Attention | Bidirectional (no mask) | **Causal** (upper-triangular mask) | Rewrite attention |
| Position | Absolute learned | **Rotary (RoPE)** | New position module |
| Layer Norm | Post-LN, LayerNorm | **Pre-RMSNorm** | New norm module |
| FFN | GELU(W1 x) W2 | **SwiGLU: silu(W1 x) ⊙ (W2 x) • W3** | Rewrite FFN |
| Attention heads | Multi-head | **Grouped-query (fewer K/V heads)** | New attention mask expansion |
| Pooling | CLS or mean | **Last-token after task prompt** | New pooling |
| Tokenizer | WordPiece/SentencePiece | **Tiktoken-compatible BPE** | New tokenizer path |

## Directory structure

```
crates/context-graph-embeddings/src/models/pretrained/qwen3_embedding/
├── mod.rs                      (~50)
├── types.rs                    (~120)
├── constants.rs                (~40)
├── model.rs                    (~250)
├── loader.rs                   (~400)   Qwen2.5 weight layout
├── tokenizer.rs                (~150)   Qwen BPE
├── rotary.rs                   (~200)   RoPE position embedding (NEW)
├── attention.rs                (~400)   Causal GQA (NEW)
├── embeddings.rs               (~80)    Token only (no position — RoPE does it)
├── rms_norm.rs                 (~80)    RMSNorm (NEW)
├── swiglu.rs                   (~120)   SwiGLU FFN (NEW)
├── decoder_layer.rs            (~150)
├── decoder_stack.rs            (~100)
├── task_prompt.rs              (~80)    Format query with task prompt
├── pooling.rs                  (~60)    Last-token pooling after prompt
├── gpu_forward.rs              (~250)
├── trait_impl.rs               (~50)
└── tests/                      (~500)
```

**Total: ~3,000-3,500 lines. All new — no reuse.**

## Task prompt handling

Qwen3-Embedding requires a task prompt prepended:
```
"Instruct: <task description>\nQuery: <content>"
```

Task prompt is configurable at embed time, defaults to a style-classification
prompt for our use case:

```rust
pub const DEFAULT_STYLE_TASK_PROMPT: &str =
    "Classify the literary style of the text as Shakespearean early modern \
     English verse or as ordinary modern English prose.";
```

Expose a per-call override via `ModelInput::Text::instruction`.

## Decoder forward pass — key code sections

### RoPE (rotary position embeddings)

```rust
/// Apply rotary position embedding to query and key.
/// q, k: [batch, n_heads, seq_len, head_dim]
/// cos, sin: [seq_len, head_dim/2] precomputed
pub fn apply_rope(q: &Tensor, k: &Tensor,
                   cos: &Tensor, sin: &Tensor) -> (Tensor, Tensor) { ... }
```

### Causal attention with GQA

Qwen3-0.6B has n_heads=16 but n_kv_heads=2. Key/value tensors are broadcast
via `repeat_interleave(n_heads/n_kv_heads)` before attention:

```rust
let k = k.repeat_interleave(n_heads / n_kv_heads, dim=1);
let v = v.repeat_interleave(n_heads / n_kv_heads, dim=1);
```

Causal mask:
```rust
let mask = Tensor::tril(seq_len, seq_len);  // lower-triangular
let scores = q @ k.transpose(-2, -1) * scale;
let scores = scores.masked_fill(!mask, f32::NEG_INFINITY);
```

### RMSNorm

```rust
pub fn rms_norm(x: &Tensor, weight: &Tensor, eps: f32) -> Tensor {
    let mean_sq = x.pow(2).mean(last_dim, keepdim);
    let x_norm = x / (mean_sq + eps).sqrt();
    x_norm * weight
}
```

### SwiGLU FFN

```rust
pub fn swiglu_ffn(x: &Tensor, w_gate: &Tensor, w_up: &Tensor,
                    w_down: &Tensor) -> Tensor {
    let gate = silu(x @ w_gate);
    let up = x @ w_up;
    (gate * up) @ w_down
}
```

## Storage schema

- `CF_E18_QWEN3_EMBEDDING` — 4 + 1024×4 = 4100 bytes per record
- **Note:** Qwen3 supports 32K context but we truncate to 8192 at embed
  time to keep throughput reasonable (configurable).

## HNSW

- `EmbedderIndex::E18Qwen3Embedding`

## Tests

- Weight load: verify all Qwen2.5 tensor names map to candle tensors (layer
  count match, dim match)
- RoPE correctness: compare output to HF reference within bf16 tolerance
- Causal attention correctness: token N's output depends only on tokens
  0..N (perturbation test)
- GQA correctness: full output matches non-GQA equivalent
- Last-token pooling: embeddings differ if task prompt changes
- Shakespeare vs modern gap ≥ 0.09 (matches Python-side +0.0934)
- Python parity: cos ≥ 0.995 vs HF reference on 10 Sonnets

## Risks

| Risk | Mitigation |
|------|-----------|
| RoPE implementation drift vs HF (many edge cases in neox vs llama rope styles) | Test against HF reference outputs on 50 inputs before declaring done |
| GQA head expansion order bugs | Compare attention output to non-GQA equivalent cell-by-cell |
| bf16 numerics in silu()/RMSNorm divergence | Cast critical ops to f32 if needed; document |
| 32K context memory explosion | Hard-cap at 8192 tokens; error on longer input |
| Task prompt tokenization subtleties | Verify Qwen BPE handles the default prompt identically to HF |

## Definition of Done

- [ ] All 7 layers integrated
- [ ] All Qwen3-specific components (RoPE, GQA, RMSNorm, SwiGLU) tested vs
      HF reference
- [ ] 6-check FSV + Python parity test (cos ≥ 0.995)
- [ ] Shakespeare discrimination test passes
- [ ] Reingestion done

## Estimate

**~20-30 engineer hours** of focused work. Of the 4 extended embedders,
this is the most time-consuming by a large margin.
