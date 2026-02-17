# E5 Causal Embedder: Complete Technical Deep Dive

## What It Is

E5 is the 5th embedder (index 4) in Context Graph's 13-embedder stack. It is a **768-dimensional asymmetric causal embedding model** built on top of `nomic-ai/nomic-embed-text-v1.5`, fine-tuned with LoRA adapters and learned projection heads to detect and encode cause-effect relationships in text. It works in concert with a local 7B LLM (Hermes-2-Pro-Mistral-7B) that provides training data, confidence scores, and runtime causal analysis.

Its purpose is singular: given a piece of text, produce two different vectors — one representing the text *as a cause*, and one representing it *as an effect* — such that real causal pairs score high on asymmetric similarity and non-causal pairs score low.

---

## The Base Model: NomicBERT

E5 starts from **nomic-embed-text-v1.5**, a 137M-parameter BERT variant with several architectural differences from vanilla BERT:

| Component | NomicBERT | Vanilla BERT |
|-----------|-----------|--------------|
| Position encoding | Rotary (RoPE, base=1000) | Learned absolute |
| FFN activation | SwiGLU (gate + up projections) | GELU |
| QKV projection | Fused single matrix [3×768, 768] | Separate Q, K, V |
| Biases | None in attention or FFN | Present |
| Max sequence length | 8192 tokens | 512 tokens |
| Pre-training | Contrastive (query/document pairs) | MLM + NSP |

**Config**: 12 layers, 12 attention heads, hidden_size=768, intermediate_size=3072, vocab_size=30528.

**Memory**: ~547MB FP32 weights on GPU (CUDA).

The key property inherited from nomic-embed is **instruction-prefix asymmetry**: the model was contrastively pre-trained with `search_query:` and `search_document:` prefixes. Texts prefixed differently produce genuinely different embeddings, even for the same content. E5 exploits this by using different instruction prefixes for cause vs. effect roles.

---

## How Asymmetric Embeddings Work

### The Core Idea

Traditional embedding models produce a **single vector** per text. E5 produces **two vectors** per text: one encoding the text's causal role as a potential cause, and another as a potential effect. This is what makes directional causal retrieval possible — "why did X happen?" and "what happens when X?" route to different vector spaces.

### Base Model Asymmetry (Without LoRA)

Without fine-tuning, the model uses instruction-prefix-based asymmetry:

```
Cause vector:  gpu_forward("search_query: Identify the cause in: <text>")
Effect vector: gpu_forward("search_query: Identify the effect of: <text>")
```

Two separate forward passes through the same NomicBERT encoder, but with different instruction prefixes, produce genuinely different 768D embeddings. The contrastive pre-training of nomic-embed ensures these prefixes create meaningful divergence in the embedding space.

**Problem**: Without fine-tuning, this produces near-uniform similarity scores (0.93–0.98 for all pairs), making the causal gate a no-op — it can't distinguish causal from non-causal content.

### Fine-Tuned Asymmetry (With LoRA + Projections)

After training, the architecture becomes:

```
                         Input Text
                             |
                   "search_document: <text>"
                             |
                    ┌────────┴────────┐
                    │  NomicBERT      │
                    │  + LoRA on Q,V  │
                    │  (12 layers)    │
                    └────────┬────────┘
                             |
                     base_embedding [768D]
                             |
                 ┌───────────┴───────────┐
                 |                       |
      ┌──────────┴──────────┐  ┌─────────┴──────────┐
      │  Cause Projection   │  │  Effect Projection  │
      │  W_cause [768×768]  │  │  W_effect [768×768] │
      │  + b_cause [768]    │  │  + b_effect [768]   │
      └──────────┬──────────┘  └─────────┬──────────┘
                 |                       |
            L2 normalize            L2 normalize
                 |                       |
          cause_vec [768D]        effect_vec [768D]
```

Key insight: with trained weights, **both cause and effect text use the same `search_document:` prefix**. This matches the training distribution. The cause/effect asymmetry comes entirely from the **learned projection heads**, not from instruction prefixes.

Using causal instruction prefixes at inference after training with `search_document:` causes a distribution shift that compresses all scores toward zero.

---

## LoRA: Low-Rank Adaptation

### What LoRA Does

LoRA adapts the frozen NomicBERT attention layers without modifying the original weights. For each of the 12 encoder layers, rank-16 adapters are added to the Query and Value projections:

```
Original:  q = x @ W_q
With LoRA: q = x @ W_q + scale * (x @ A_q @ B_q)
```

Where:
- `A_q` is [768, 16] — the down-projection (initialized with Kaiming uniform)
- `B_q` is [16, 768] — the up-projection (initialized to **zero**)
- `scale = alpha / rank = 16 / 16 = 1.0`
- Dropout = 0.1 during training (disabled at inference)

### Why It Works

1. **B initialized to zero**: At the start of training, LoRA contributes nothing (identity behavior). The model starts from the pre-trained representations and gradually learns causal-specific attention patterns.

2. **Only 589,824 trainable params**: 2 adapters (Q, V) × 12 layers × 2 × 768 × 16. This is <0.5% of the base model's 137M parameters. The small parameter count prevents catastrophic forgetting of the pre-trained knowledge.

3. **Teaching the encoder**: LoRA modifies the attention patterns so the encoder produces base embeddings that are *more amenable to causal projection*. Without LoRA, the projection heads must work harder to separate cause/effect from a general-purpose embedding. With LoRA, the encoder itself learns to emphasize causal structure in its representations.

### Dropout: A Critical Detail

LoRA dropout (p=0.1) is only applied during training via `forward_train()`. During inference, `forward()` skips dropout entirely.

**Lesson learned**: Dropout regularization compresses absolute scores. Whenever dropout is changed, the causal gate thresholds must be recalibrated.

---

## The Projection Heads

### Architecture

Two learned linear projections transform the base 768D embedding into cause-role and effect-role vectors:

```
cause_vec = L2_norm(base_emb @ W_cause^T + b_cause)
effect_vec = L2_norm(base_emb @ W_effect^T + b_effect)
```

Each projection is a full [768, 768] matrix + [768] bias — 590,592 parameters per head, 1,181,184 total for both.

### Initialization

Projections start as **perturbed identity matrices**: `W = I + N(0, 0.02)`. This creates immediate asymmetry (cause and effect projections start slightly different) while preserving most of the base embedding's information. Biases are initialized as uniform noise in [-0.01, 0.01].

The initialization uses a deterministic seed (`0xCA05A1`) for reproducibility.

### What They Learn

Through training, the cause projection learns to amplify dimensions that correlate with "being a cause" (agency, antecedence, mechanism initiation) while the effect projection amplifies dimensions correlating with "being an effect" (consequence, outcome, downstream impact). The same text, projected through both heads, produces vectors in different regions of the embedding space.

---

## The Training Pipeline

### Data: 252 Seeds → 2,498 Pairs

Training data comes from **causal training pairs** — structured records containing:
- `cause_text`: text describing the cause
- `effect_text`: text describing the effect
- `direction`: Forward, Backward, Bidirectional, or None
- `confidence`: LLM-assigned soft label [0.0, 1.0]
- `mechanism`: domain label (biological, economic, physical, etc.)
- `hard_negative`: semantically similar but non-causal text
- `rationale`: explanation of the causal mechanism

252 seed pairs are expanded to 2,498 training pairs through augmentation (reversed directions, hard negative mining, domain-specific variations).

### Curriculum: 3-Stage Progressive Training

Training is organized into three progressive stages with increasing difficulty:

#### Stage 1: Projection Warm-Up (25 epochs)
- **What trains**: Only the projection heads (W_cause, W_effect, biases)
- **LoRA**: Disabled (saves VRAM)
- **Data**: Easy pairs only (difficulty ≤ 0.2 — pairs with explicit markers like "because", "causes", "therefore")
- **Multi-task**: Disabled
- **Purpose**: Teach projection heads basic cause/effect separation on unambiguous examples before introducing encoder adaptation

#### Stage 2: LoRA Activation (20 epochs)
- **What trains**: Projection heads + LoRA adapters + multi-task heads
- **LoRA**: Enabled with dropout
- **Data**: All training pairs
- **Multi-task**: Active (direction + mechanism classification)
- **Purpose**: Adapt the encoder's attention patterns for causal structure detection

#### Stage 3: Directional Emphasis (20 epochs)
- **What trains**: Projection heads + LoRA adapters + multi-task heads
- **LoRA**: Enabled with dropout
- **Data**: All training pairs
- **Loss**: Directional margin weight increased from 0.3 → 0.6
- **Multi-task**: Active
- **Purpose**: Sharpen directional discrimination (cause→effect vs effect→cause)

### Cross-Stage Continuity

Each stage loads the previous stage's best projection checkpoint. Stage 2 loads Stage 1's best, Stage 3 loads Stage 2's best. Per-stage LoRA checkpoints are also saved so the final "best" model uses the LoRA weights from the best-performing stage, not just the latest.

**Best stage selection** uses `score_spread` (not directional accuracy) — the gap between top-1 and rank-5 scores, measuring retrieval discrimination power.

---

## The Loss Function: 4-Component Directional Contrastive Loss

The combined loss is:

```
L = λ_c × L_contrastive + λ_d × L_directional + λ_s × L_separation + λ_soft × L_soft
```

Default weights: λ_c=1.0, λ_d=0.3 (0.6 in Stage 3), λ_s=0.1, λ_soft=0.2.

### 1. InfoNCE Contrastive Loss (Primary)

```
L_NCE = -log(exp(sim(cause_i, effect_i) / τ) / Σ_j exp(sim(cause_i, effect_j) / τ))
```

Temperature τ = 0.05 (per Causal2Vec paper). Uses in-batch negatives: a batch of N pairs provides N×(N-1) free negatives.

**Semi-hard negative mining**: For each row, the top-k (k=3) hardest off-diagonal negatives with logit below the positive get their logits scaled by `hard_negative_scale`. Set to 1.0 (disabled) because values ≥2.0 with τ=0.05 cause **catastrophic score collapse** — cause/effect vectors become orthogonal.

### 2. Directional Margin Loss

```
L_dir = max(0, margin - (sim_paired - mean_off_diagonal))
```

Margin = 0.2. Enforces that correct cause→effect pairing scores higher than mismatched pairings by at least the margin. This is what teaches the model that "A causes B" should score higher than "A causes C" (where C is from a different pair).

### 3. Separation Loss

```
L_sep = mean(cos(cause_i, effect_i))  for same input text
```

Minimizes cosine similarity between the same text's cause and effect vectors. Without this, both projection heads might converge to produce similar vectors, defeating the purpose of asymmetry.

### 4. Soft Label Distillation Loss

```
L_soft = MSE(cos(cause, effect), confidence_LLM)
```

Uses the LLM's confidence score as a soft target instead of hard binary labels. This teaches the model to produce *calibrated* similarity scores — a pair the LLM rated at 0.85 confidence should produce ~0.85 cosine similarity, while a 0.3-rated pair should produce ~0.3.

---

## Multi-Task Auxiliary Heads

Active in Stages 2 and 3 to provide additional gradient signal:

### Direction Classification (3 classes)
- Forward (A→B), Backward (B→A), None
- 2-layer MLP: [1536, 256, 3] (input is concatenated cause+effect vectors)
- Loss weight: λ_direction = 0.2

### Mechanism Classification (7 classes)
- Biological, Economic, Physical, Technical, Social, Ecological, Other
- 2-layer MLP: [1536, 256, 7]
- Loss weight: λ_mechanism = 0.1

These auxiliary tasks help the shared encoder learn richer causal representations by forcing it to distinguish both *direction* and *mechanism type*, not just binary causal/non-causal.

---

## The LLM: Hermes-2-Pro-Mistral-7B

### Role in the System

The LLM serves three functions:

1. **Training data generation**: Analyzes memory pairs for causal relationships, producing the structured training data (direction, confidence, mechanism) used to train E5
2. **Runtime causal analysis**: Classifies individual texts for causal nature during memory storage, providing `CausalHint` metadata
3. **Multi-relationship extraction**: Extracts all distinct cause-effect relationships from text with source provenance

### Architecture

- **Model**: Hermes-2-Pro-Mistral-7B (Q5_K_M quantized GGUF)
- **Runtime**: llama-cpp-2 (Rust bindings)
- **VRAM**: ~5GB model + ~1GB KV cache = ~6GB total
- **Context**: 4096 tokens
- **Temperature**: 0.0 (deterministic analysis)
- **GPU**: Full offload to CUDA

### Grammar-Constrained Generation

The critical innovation is **GBNF grammar constraints** — the LLM's output is forced to conform to a formal grammar, guaranteeing 100% valid JSON output. This solved the ~40% JSON parse failure rate of the previous Candle/Qwen implementation.

Five grammar types are defined:
- **Causal**: Pair analysis → `{causal_link, direction, confidence, mechanism}`
- **SingleText**: Text classification → `{is_causal, direction, confidence, key_phrases, description, asymmetry_strength, cause_entities, effect_entities}`
- **MultiRelationship**: Full extraction → `{relationships: [{cause, effect, explanation, confidence, mechanism_type, source_spans}]}`
- **Graph**: Graph relationship analysis
- **Validation**: Link validation

### Prompt Engineering

Uses Hermes-2-Pro's **ChatML format** (`<|im_start|>`/`<|im_end|>` tags) with:
- Structured 6-step decision procedure for single-text analysis
- Few-shot examples (5 examples) for pair analysis
- Explicit calibration guidelines (0.9-1.0 for established mechanisms, 0.0-0.2 for no causal link)
- Mechanism extraction requirements with anti-tautology rules ("Bad: 'A causes B'. Good: 'Increased cortisol impairs hippocampal function'")

---

## The Causal Gate: Score-to-Decision Conversion

### The Problem

E5's continuous scores (cosine similarity between cause/effect vectors) need to be converted into a binary decision: is this result causally relevant? The **causal gate** does this conversion.

### Thresholds (Post-Training Calibration)

After training with prefix-aligned LoRA+projection on 252 seeds → 2,498 pairs:

| Metric | Value |
|--------|-------|
| Causal mean score | 0.138 |
| Non-causal mean score | 0.016 |
| Score gap | 0.121 |
| CAUSAL_THRESHOLD | 0.04 |
| NON_CAUSAL_THRESHOLD | 0.008 |
| TPR (causal correctly boosted) | 83.9% |
| TNR (non-causal correctly demoted) | 84.3% |

### Gate Logic

```rust
if e5_score >= 0.04:        // Definitely causal
    score *= 1.10           // CAUSAL_BOOST
elif e5_score <= 0.008:     // Definitely non-causal
    score *= 0.85           // NON_CAUSAL_DEMOTION
else:                       // Ambiguous zone
    score unchanged
```

The gate only activates for **causal queries** — detected via keyword matching against 100+ cause-seeking and effect-seeking indicators (with negation-aware preprocessing).

### Why These Thresholds

The untrained model produces scores of 0.93–0.98 for everything. After training, causal content clusters around 0.138 and non-causal around 0.016 — a 0.121 gap. The thresholds were calibrated to capture ~85% of each distribution: 0.04 catches most causal content while excluding most non-causal, and 0.008 catches most non-causal while excluding the ambiguous zone.

---

## Asymmetric Similarity: The Direction Modifiers

### Constitution-Specified Formula

```
sim = base_cosine × direction_modifier
```

| Query Direction | Result Direction | Modifier | Rationale |
|----------------|-----------------|----------|-----------|
| Cause | Effect | **1.2** | Forward inference amplified — "what does X cause?" finds effects |
| Effect | Cause | **0.8** | Backward inference dampened — reverse direction penalized |
| Same | Same | 1.0 | No modification |
| Unknown | Any | 1.0 | No modification |

### Fingerprint-Based Asymmetric Retrieval

For "why" queries (searching for causes), the system compares:
- `query.e5_as_effect` vs `doc.e5_as_cause`

For "what happens" queries (searching for effects):
- `query.e5_as_cause` vs `doc.e5_as_effect`

This cross-pairing is the core of asymmetric retrieval — the query's role vector is matched against documents' complementary role vectors.

### Direction Inference from Stored Vectors

Without explicit metadata, the system infers causal direction from stored fingerprints by comparing **component variance** of cause and effect vectors. Higher variance indicates a more peaked/concentrated distribution, meaning the projection head found a more specific representation for that role:

```
if cause_variance > effect_variance × 1.05: → Cause
if effect_variance > cause_variance × 1.05: → Effect
else: → Unknown
```

### Intervention Overlap (Disabled)

An intervention overlap factor `(0.7 + 0.3 × overlap)` was originally part of the formula but was disabled after benchmark analysis showed -15.9% performance impact with only 0.063 correlation. The simplified formula `sim = base_cos × direction_mod` performs better.

---

## Causal Query Intent Detection

Before applying any causal logic, the system must determine whether a query is causal at all. This is done via **keyword-based intent detection** with:

- **100+ cause-seeking indicators**: "why", "what causes", "root cause", "diagnose", "troubleshoot", "stems from", "depends on", "precursor", etc.
- **90+ effect-seeking indicators**: "what happens", "consequence of", "leads to", "downstream", "predict", "prognosis", "cascading", etc.
- **Negation-aware**: 12 negation tokens ("not", "never", "doesn't", etc.) with 15-character lookback window to avoid false positives
- **Score-based disambiguation**: When both cause and effect indicators are present, the side with more matches wins; ties go to Cause (more common in natural language)
- **UTF-8 safe**: Uses `floor_char_boundary`/`ceil_char_boundary` for safe string slicing

---

## How It All Fits Together: The Full Pipeline

### At Memory Storage Time

1. User stores a memory via MCP `store_memory` tool
2. The **causal-agent** (background service) picks up the new memory
3. LLM analyzes the text → `CausalHint` (is_causal, direction, confidence, key_phrases)
4. If causal, E5 generates dual embeddings:
   - Forward pass with LoRA → base embedding
   - Cause projection → cause_vec [768D]
   - Effect projection → effect_vec [768D]
5. Both vectors stored in the `SemanticFingerprint` at E5's index (slot 4)
6. Vectors persisted to RocksDB in teleological column families

### At Search Time

1. User queries via `search_graph` or similar
2. **Causal intent detection** analyzes query text
3. If causal query detected:
   - Query text embedded as cause AND effect via E5
   - Appropriate cross-pairing selected based on query direction
   - Results scored with asymmetric similarity
   - **Causal gate** applied: boost causal results (×1.10), demote non-causal (×0.85)
   - **Direction modifiers** applied: cause→effect ×1.2, effect→cause ×0.8
4. E5 scores fused with other 12 embedders via RRF or weighted combination

### At Training Time

1. Causal seed pairs created (manual or LLM-generated)
2. Seeds expanded to 2,498 training pairs with hard negatives
3. 3-stage pipeline runs on GPU:
   - Stage 1: projection-only on easy pairs (25 epochs)
   - Stage 2: LoRA+projection on all pairs with multi-task heads (20 epochs)
   - Stage 3: directional emphasis with increased margin weight (20 epochs)
4. Best checkpoint selected by `score_spread` metric
5. `lora_best.safetensors` and `projection_best.safetensors` saved
6. At next server start, `load_trained_weights()` loads the checkpoint

---

## Why It Works: The Theory

### Why Asymmetric Embeddings Can Encode Causality

Causation is **asymmetric** — "A causes B" does not imply "B causes A". Standard symmetric cosine similarity cannot capture this: cos(A, B) = cos(B, A) always. By projecting through separate learned heads, E5 breaks this symmetry.

The cause projection learns a subspace where texts that tend to be causes are well-separated, while the effect projection learns a complementary subspace. The training objective (InfoNCE + directional margin) directly optimizes for the property that `cos(cause_A, effect_B) > cos(cause_A, effect_C)` when A causes B but not C.

### Why LoRA + Projection Is Better Than Either Alone

- **Projection-only** (Stage 1): Works for easy pairs with explicit markers but struggles with implicit causation because the base embeddings aren't optimized for causal structure
- **LoRA-only** (without projection): Would need the instruction prefix to create asymmetry, but instruction prefixes create near-uniform scores without domain-specific training
- **Both together** (Stages 2-3): LoRA teaches the encoder to produce base embeddings that highlight causal structure, then projections separate those embeddings into directional role vectors. The encoder and projections co-adapt during training.

### Why the LLM Is Essential

The LLM provides three things that make the system possible:

1. **Soft labels**: Instead of hard 0/1 binary labels, the LLM produces calibrated confidence scores (0.0–1.0) that the model learns to match. This teaches nuanced scoring rather than binary classification.

2. **Mechanism descriptions**: The LLM generates rich mechanism explanations ("Chronic stress activates the HPA axis, elevating cortisol which binds to hippocampal glucocorticoid receptors...") that serve as high-quality training signal for the contrastive objective.

3. **Hard negative generation**: The LLM identifies semantically similar but non-causal texts, which are the hardest negatives for contrastive learning. Without these, the model might learn topical similarity rather than causal structure.

### Why E5 Is Structural, Not Topical

A key lesson: **E5 detects causal markers and structure, not topical relevance**. It cannot tell whether two paragraphs about the same topic are causally related if they lack structural causal indicators. Real E1 (e5-large-v2) cosine similarity can't discriminate 250 similar causal passages (5.8% top-1 accuracy). E5's value is as a **gate and re-ranker** — it boosts genuinely causal content and demotes superficially similar but non-causal content within results already retrieved by the topical embedders (E1, E7, E11, etc.).

---

## Key Numbers

| Parameter | Value |
|-----------|-------|
| Base model | nomic-ai/nomic-embed-text-v1.5 (137M params) |
| Output dimension | 768D per vector (1536D total: cause + effect) |
| LoRA rank | 16 |
| LoRA params | 589,824 (Q + V adapters × 12 layers) |
| Projection params | 1,181,184 (2 × [768×768] + 2 × [768]) |
| Multi-task params | ~15K (direction 3-class + mechanism 7-class MLPs) |
| Total trainable | ~1.79M (1.3% of base model) |
| Training pairs | 2,498 (from 252 seeds) |
| Training stages | 3 (25 + 20 + 20 = 65 epochs max) |
| Batch size | 16 |
| InfoNCE temperature | 0.05 |
| Directional margin | 0.2 |
| Max tokens | 512 (of 8192 supported) |
| P95 latency target | 8ms |
| LLM | Hermes-2-Pro-Mistral-7B Q5_K_M (~5GB VRAM) |
| LLM context | 4096 tokens |
| Causal gate TPR | 83.9% |
| Causal gate TNR | 84.3% |
| Causal mean score | 0.138 |
| Non-causal mean score | 0.016 |
| Direction boost | cause→effect: 1.2×, effect→cause: 0.8× |
| Causal boost | 1.10× |
| Non-causal demotion | 0.85× |

---

## File Map

| File | Purpose |
|------|---------|
| `crates/context-graph-embeddings/src/models/pretrained/causal/mod.rs` | Module root, re-exports |
| `crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` | CausalModel struct, embed/load/dual methods |
| `crates/context-graph-embeddings/src/models/pretrained/causal/config.rs` | NomicConfig, dimension/prefix constants |
| `crates/context-graph-embeddings/src/models/pretrained/causal/weights.rs` | NomicWeights, CausalProjectionWeights, TrainableProjection |
| `crates/context-graph-embeddings/src/models/pretrained/causal/forward.rs` | GPU forward pass (RoPE attention, SwiGLU FFN) |
| `crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs` | Safetensors weight loading |
| `crates/context-graph-embeddings/src/training/lora.rs` | LoRA adapters (rank-16 Q+V) |
| `crates/context-graph-embeddings/src/training/loss.rs` | 4-component loss function |
| `crates/context-graph-embeddings/src/training/pipeline.rs` | 3-stage progressive training pipeline |
| `crates/context-graph-embeddings/src/training/data.rs` | Training pair structures, data loader |
| `crates/context-graph-embeddings/src/training/evaluation.rs` | Evaluation metrics (DirAcc, MRR, AUC, spread) |
| `crates/context-graph-embeddings/src/training/multitask.rs` | Direction + mechanism classification heads |
| `crates/context-graph-embeddings/src/training/optimizer.rs` | AdamW with param groups |
| `crates/context-graph-embeddings/src/bin/train_causal.rs` | CLI training binary |
| `crates/context-graph-core/src/causal/asymmetric.rs` | Asymmetric similarity, causal gate, query intent detection |
| `crates/context-graph-causal-agent/src/llm/mod.rs` | CausalDiscoveryLLM (Hermes-2-Pro via llama-cpp-2) |
| `crates/context-graph-causal-agent/src/llm/prompt.rs` | Prompt templates (ChatML, few-shot, multi-relationship) |
| `crates/context-graph-benchmark/src/causal_bench/phases.rs` | Benchmark evaluation phases |
