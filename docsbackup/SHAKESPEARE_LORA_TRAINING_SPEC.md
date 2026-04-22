# Shakespeare LoRA Training Pipeline — Design Specification

**Version**: 1.0 — April 15, 2026
**Author**: Chris Royse + Claude
**Status**: Ready to implement
**Prerequisites**: Context Graph daemon running with Shakespeare corpus (3,043 fingerprints, 44 constellations, 13,465 contrastive pairs)

---

## 1. The Goal

Train a LoRA adapter on Gemma4-8B that makes the model **write indistinguishably as Shakespeare on any topic** — not through prompting, but through learned weight modifications. The constellation guard (44 work-constellations × 13 embedders) serves as both training signal and runtime validator, exactly as the video constellation guard serves for EchoMimicV3.

**Success metric**: constellation guard passes the LoRA'd model's output on attempt 1, ≥90% of the time, with Sonnets score ≥0.50, on prompts about modern topics the training set never saw.

**The analogy to ClipCannon video (from the white paper):**

| Property | Video (EchoMimicV3) | Text (Gemma4) |
|---|---|---|
| Base model | EchoMimicV3 1.86B DiT | Gemma4 8B |
| Identity signal | 7-modality constellation (4,044 dims) | 13-embedder constellation (11,008 dims) |
| Training data source | 2,362 curated video clips | 3,043 memory fingerprints |
| Contrastive pairs | N/A (mouth-weighted loss) | 13,465 cross-embedder anomaly pairs |
| LoRA rank | 256 | 64-128 (text models need less) |
| Training loss | Flow-matching + 10× mouth weight | SFT + DPO with constellation-conditioned preference |
| Runtime guard | SigLIP re-embed + cosine ≥0.70 | 13-embedder re-embed + score ≥0.45 |
| Guard modalities | 7 (visual, semantic, emotion, speaker, prosody, sentiment, voice) | 13 (E1-E13 minus E11) |

---

## 2. Training Data Already Available

### 2.1 SFT Data (from `CF_TRAINING_RECORDS` + `CF_CONTENT`)

3,043 memories, each with raw text content. These ARE Shakespeare's actual text — sonnets, plays, poems, chunked at ~200 words each. The SFT training format:

```jsonl
{"messages": [
  {"role": "system", "content": "You are William Shakespeare. Write in his exact style — early modern English, iambic rhythm, nature/star/love/death imagery, thee/thou/thy vocabulary. Any topic, his voice."},
  {"role": "user", "content": "Write about the beauty of a sunset over the ocean"},
  {"role": "assistant", "content": "<actual Shakespeare chunk that matches the semantic theme>"}
]}
```

**How to build the SFT pairs**: For each of the 3,043 memories, use the E1 semantic embedding to find a modern-language "prompt" that the Shakespeare text would be a natural response to. Steps:
1. For each memory, extract its content (the Shakespeare text).
2. Use Gemma4 (base, no LoRA) to generate a modern-language question/prompt that the Shakespeare text plausibly answers. E.g., for "Shall I compare thee to a summer's day?" → prompt "Write a comparison of someone beautiful to nature."
3. The SFT pair is: `user=generated_prompt, assistant=original_shakespeare_text`.

This gives us **~3,000 SFT pairs** where the assistant output is always real Shakespeare.

### 2.2 DPO Data (from `CF_CONTRASTIVE_PAIRS`)

13,465 contrastive pairs, each with:
- `anchor_text` (Shakespeare)
- `negative_text` (Shakespeare that is structurally different — different work, different style register, different causal structure)
- `similarity_profile: [f32; 13]` — the 13-embedder cosine profile
- `anomaly_kind` — which axis they disagree on

**DPO pair construction**: For each contrastive pair:
- `prompt`: a theme/topic prompt derived from the anchor text
- `chosen`: the anchor text (correct Shakespeare for this topic)
- `rejected`: the negative text (wrong Shakespeare — right era, wrong work/style/register)

This teaches the model not just "write like Shakespeare" but "write like the RIGHT Shakespeare for the context." A love poem prompt should produce Sonnets-style, not Macbeth-style. A power/ambition prompt should produce Histories/Tragedies-style, not Comedy-style.

**13,465 DPO pairs is a massive dataset for text LoRA training.** Most DPO fine-tunes use 1,000–5,000 pairs. We have 2.5× the typical maximum.

### 2.3 Constellation Targets (from `CF_CONSTELLATIONS`)

44 work-constellations with per-embedder centroids. These serve as:
1. **Training-time evaluation**: after each epoch, generate 10 test outputs and score against constellations. Track convergence.
2. **Constellation-conditioned SFT**: tag each training example with its closest constellation (work label). Add the work label to the system prompt during training so the model learns "when I'm told 'Sonnets style', I produce THIS particular subspace."

### 2.4 Typed-Edge Relational Data (from the session we just built)

200+ typed-edge records with 8-dim relational signatures. The hubs (memories with high incoming-edge count) are the most central texts in the corpus — they should be **oversampled** in training, just as ClipCannon oversampled rare visemes with WeightedRandomSampler.

### 2.5 Edge-Type Distribution (new this session)

Each memory's `edge_type_distribution: [u32; 8]` identifies its graph role. Training batches can be stratified by role: ensure each batch contains a mix of hub nodes (high SemanticSimilar), causal-root nodes (high CausalChain), and leaf nodes (low everything). This is **curriculum learning informed by graph structure** — impossible without the features we just built.

---

## 3. Training Architecture

### 3.1 Two-Phase Training (mirrors ClipCannon video pipeline)

**Phase 1 — SFT (Supervised Fine-Tuning)**

```
Base: Gemma4-8B (gemma4:e4b GGUF → convert to HF format for training)
LoRA config:
  rank: 64
  alpha: 128
  target_modules: [q_proj, v_proj, k_proj, o_proj, gate_proj, up_proj, down_proj]
  dropout: 0.05
Training:
  lr: 2e-4, cosine schedule with 10% warmup
  batch_size: 4 (gradient accumulation 8 → effective 32)
  epochs: 3
  max_seq_len: 2048
  dataset: ~3,000 SFT pairs (Shakespeare text as assistant output)
  optimizer: AdamW, weight_decay=0.01
Hardware: RTX 5090 32GB, QLoRA with 4-bit base model → ~14GB VRAM
Framework: unsloth (fastest) or axolotl (most configurable)
```

**Phase 2 — DPO (Direct Preference Optimization)**

```
Base: Phase 1 SFT checkpoint
DPO config:
  beta: 0.1 (standard)
  dataset: 13,465 contrastive pairs → DPO format {prompt, chosen, rejected}
  lr: 5e-5 (10× lower than SFT)
  epochs: 1
  max_seq_len: 2048
Framework: trl.DPOTrainer (HuggingFace TRL)
```

DPO is the key differentiator. The 13,465 pairs teach the model to discriminate between "right Shakespeare for this context" and "wrong Shakespeare." This is what makes the LoRA produce Sonnets-style for love topics and Macbeth-style for power topics — without any prompting.

### 3.2 Constellation-Conditioned Training (the TCT analog)

During SFT, each training example gets a system-prompt tag identifying its source constellation:

```
System: You are William Shakespeare. Style: Sonnets. Write in the voice of the Sonnets — 
intimate, lyrical, focused on beauty, time, and mortality. Use iambic pentameter where natural.
```

This teaches the model to condition on style tags. At inference time, the query classifier (or the user) selects a constellation, and the model shifts register accordingly. This is the text analog of EchoMimicV3's per-frame constellation prompts.

### 3.3 Training-Time Evaluation via Constellation Guard

After each training epoch, run the evaluation loop:
1. Generate 20 test outputs on held-out modern-topic prompts.
2. For each output, call `store_memory` + `score_against_constellation` on all 44 work-constellations.
3. Track: mean top-constellation score, % passing guard threshold (≥0.45), mean spread (top - median).
4. Log to a training-metrics file for convergence visualization.

This is identical to how ClipCannon's video training tracked visual cosine similarity to the constellation centroid at each checkpoint.

---

## 4. Export Pipeline (what to build)

### 4.1 SFT Dataset Builder

New script: `scripts/shakespeare_build_sft_dataset.py`

```python
# For each memory in CF_CONTENT:
# 1. Get the Shakespeare text
# 2. Get its closest constellation (via score_against_constellation)
# 3. Generate a modern prompt that this text answers (via Gemma4 base)
# 4. Format as ChatML: system + user + assistant
# Output: shakespeare_sft.jsonl (~3,000 rows)
```

### 4.2 DPO Dataset Builder

New script: `scripts/shakespeare_build_dpo_dataset.py`

```python
# For each contrastive pair:
# 1. Get anchor_text (chosen) and negative_text (rejected)
# 2. Generate a prompt from the anchor text (same as SFT builder)
# 3. Format as: {prompt, chosen, rejected, anomaly_kind, similarity_profile}
# Output: shakespeare_dpo.jsonl (~13,465 rows)
```

### 4.3 Training Script

New script: `scripts/shakespeare_train_lora.py`

```python
# Phase 1: SFT with unsloth
# Phase 2: DPO with trl
# Eval: constellation guard scoring after each epoch
# Output: LoRA adapter at ~/.contextgraph/models/shakespeare_lora/
```

### 4.4 Inference Integration

Modify `shakespeare_chat_dashboard.py`:
- If LoRA exists at `~/.contextgraph/models/shakespeare_lora/`, load it via Ollama's `--adapter` flag (Ollama supports GGUF LoRA adapters natively).
- Remove the heavy system prompt — the LoRA should carry the style without it.
- Keep the constellation guard as defense-in-depth (same architecture as ClipCannon video).

---

## 5. Why This Will Impress Google/Anthropic

### 5.1 The Complete Loop

No other system demonstrates:
1. **Automated corpus decomposition** → 13 independent measurements per text
2. **Automated training-data generation** → SFT + DPO from decomposed signals
3. **Automated hard-negative mining** → 13,465 structurally-classified pairs
4. **LoRA fine-tuning** → style transfer without prompting
5. **Runtime geometric verification** → constellation guard validates every output
6. **All in one pipeline** → no human labelers, no manual curation, no external APIs

The demo is: "We fed Shakespeare's complete works into Context Graph. It produced 220K+ multi-dimensional training signals. We trained a LoRA. The model now writes as Shakespeare on any topic. The constellation guard proves it, per-output, across 13 axes. The whole pipeline runs on one RTX 5090."

### 5.2 The Generalization Argument

"If it works for Shakespeare, it works for any author. Feed in your CEO's last 50 emails → constellation → LoRA → the model writes as your CEO with measurable per-axis fidelity. Feed in your law firm's brief style → constellation → LoRA → the model writes briefs in your house voice. The pipeline is the product. The Shakespeare demo is the proof point."

### 5.3 The Data-Economics Argument

"Scale AI charges $50-$500 per labeled training example. Context Graph produced 13,465 DPO pairs + 3,000 SFT pairs automatically from a public-domain corpus. Total compute: one RTX 5090 for ~4 hours. At Scale's rates, that's $800K–$8M of equivalent labeled data. We produced it for the cost of electricity."

---

## 6. Implementation Priority

| Step | What | Time | Dependencies |
|---|---|---|---|
| 1 | `shakespeare_build_sft_dataset.py` | 2 hours | Running daemon with Shakespeare corpus |
| 2 | `shakespeare_build_dpo_dataset.py` | 1 hour | Step 1 + CF_CONTRASTIVE_PAIRS |
| 3 | Install unsloth + trl | 30 min | pip install |
| 4 | `shakespeare_train_lora.py` Phase 1 (SFT) | 2-4 hours GPU time | Steps 1-3 |
| 5 | Phase 2 (DPO) | 1-2 hours GPU time | Step 4 |
| 6 | Eval: constellation guard scoring | 30 min | Step 5 |
| 7 | Integrate LoRA into dashboard | 1 hour | Step 6 passing |
| 8 | Demo: modern-topic Shakespeare with per-axis fidelity | 30 min | Step 7 |

**Total: ~1 day of focused work to go from "training data exists" to "demo-ready LoRA."**

---

## 7. What Makes This Different from Fine-Tuning on Raw Text

A naive approach: fine-tune Gemma4 on Shakespeare's raw text as SFT data. That produces a model that knows Shakespeare's words but:
- Can't transfer the style to modern topics (no SFT pairs mapping modern→Shakespeare)
- Can't discriminate between Sonnets-style and Macbeth-style (no DPO signal)
- Has no runtime verification (no constellation guard)
- Has no per-axis quality metrics (no 13-embedder scoring)
- Used zero training signals beyond raw text (wasted the 91× multiplication)

Our approach uses **every signal Context Graph produces**: SFT pairs from content + constellation matching, DPO pairs from cross-embedder anomalies, constellation conditioning from work-level clustering, curriculum weighting from graph-structural signatures, and runtime guard from the same 13 embedders. The raw text is the input. The 220K+ derived signals are the training data. That's the "training data factory" thesis made concrete.

---

*End of spec. Implementation starts with Step 1.*
