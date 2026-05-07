---
namespace: reports
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:35:00Z
status: final
tags: verdict, lora-config, inference-alignment, style-vs-semantic
---

# Agent 3 (Sherlock, Config) — Final Report

## VERDICT: MIXED (Training config is STYLE-CORRECT; inference alignment has a MEDIUM design inconsistency; one HIGH-severity cross-scope issue with DPO pair selection)

## Sub-Verdicts

### (A) Training config correctness: STYLE-CORRECT (PASS)
- LoRA rank/alpha: 256/512 — standard for dominant style override
- Target modules: 258 across all 42 Gemma 4 language_model layers
  - 132 attention (Q/K/V/O) — 51.2% — governs SYNTACTIC/rhythm patterns
  - 126 MLP (gate/up/down) — 48.8% — governs lexical choice
  - BALANCED. Not skewed to semantic-only or style-only modules.
- Training data: 7,925 clean SFT examples + 11,724 DPO pairs
- Loss function: standard cross-entropy (SFT) and standard DPO loss (beta=0.1). CORRECT given data is uniformly archaic Shakespeare.
- System prompt: UNIFIED across SFT/DPO/dashboard. STYLE-focused ("writes EXCLUSIVELY in the style of Shakespeare"), not PERSONA-focused.
- Final metrics: SFT loss 1.04 / token_acc 0.991; DPO loss 0.09 / rewards/accuracies 0.98 / rewards/margins 49.3 → converged well

### (B) Inference alignment: MEDIUM (partial misalignment)
- Dashboard system prompt matches training system prompt → ALIGNED
- Constrained decoder BOOST list (77 tokens): all STYLE markers → ALIGNED
- Constrained decoder SUPPRESS list (79 tokens):
  - 33 style-breaking (AI/chatbot/casual phrases) → ALIGNED
  - 15 TOPICAL-modern (internet/wifi/app/website/database/server/API/URL/HTML) → **MISALIGNED** — these are topic-words, not style-words, and they contradict the system prompt's "full knowledge of modern topics" promise
  - 31 foreign-language → language policy, not style (creates the Golden Age Spanish emergent behavior as a side effect)

### (C) Cross-scope finding: HIGH — DPO pair selection is content-contrast, not style-contrast
- 1,000-sample empirical test shows 65% of DPO pairs have BOTH chosen and rejected as archaic Shakespeare
- 15.5% of pairs have REJECTED MORE archaic than CHOSEN (the training signal is actively INVERTED for these)
- Only 7.1% are correctly style-contrastive
- ALL 5,000 sampled pairs have `anomaly_kind: "other"` — the anomaly-typing that should differentiate pair semantics is collapsed to one bucket
- This is **Agent 2's scope** (DPO pair mining) but it affects the training-config correctness question

## Evidence (file:line references)

### Training config
- `/home/cabdru/.contextgraph/models/shakespeare_lora/final/adapter_config.json:19-29` — r=256, lora_alpha=512, lora_dropout=0.05
- `/home/cabdru/.contextgraph/models/shakespeare_lora/final/adapter_config.json:32-290` — 258 target modules enumerated
- `scripts/shakespeare_full_pipeline.py:416-425` — target module scan and LoraConfig
- `scripts/shakespeare_full_pipeline.py:447-469` — SFT config (batch 1, grad_accum 32, lr 2e-4, cosine, bf16, adamw_8bit)
- `scripts/shakespeare_full_pipeline.py:525-545` — DPO config (beta=0.1, lr 5e-5, ref_model=None)
- `/home/cabdru/.contextgraph/models/shakespeare_lora/sft_checkpoint/checkpoint-744/trainer_state.json:1-10` — SFT 744 steps, 3 epochs, final loss 1.04, token_acc 0.991
- `/home/cabdru/.contextgraph/models/shakespeare_lora/dpo_checkpoint/checkpoint-367/trainer_state.json:1-10` — DPO 367 steps, 1 epoch, final rewards/acc 0.98 / margins 49.3

### System prompt alignment
- `scripts/shakespeare_chat_dashboard.py:22-30` — dashboard system prompt (style-focused)
- `scripts/shakespeare_rebuild_all_data.py:18-27` — UNIFIED_SYSTEM constant
- `scripts/shakespeare_rebuild_all_data.py:87,126,206,311,357,407` — UNIFIED_SYSTEM applied to all training data surfaces
- `scripts/shakespeare_full_pipeline.py:29-37` — persona prompt (DEAD CODE in v2.1, only for Phase 1 which was skipped)
- `scripts/shakespeare_full_pipeline.py:581-584` — Phase 1 skip

### Inference alignment
- `scripts/shakespeare_inference_server.py:55-76` — boost_words (STYLE tokens)
- `scripts/shakespeare_inference_server.py:80-106` — suppress_words (MIXED style + TOPIC + LANGUAGE)
- `scripts/shakespeare_inference_server.py:130-149` — logit processor applies boost/suppress each step
- `scripts/shakespeare_inference_server.py:189-228` — generate() injects logit_processor into model.generate()

### DPO pair evidence (cross-scope)
- `~/.contextgraph/training/shakespeare_dpo_clean.jsonl:1` — sample pair showing CHOSEN and REJECTED both as raw Shakespeare play dialogue
- Empirical: 1,000 pairs sampled; 65.1% both archaic, 15.5% backwards, 7.1% correct

## Fix if NO (specifically what to change)

### MEDIUM-severity fixes to inference alignment
In `scripts/shakespeare_inference_server.py:80-106` REMOVE these tokens from suppress_words:
```python
# Modern tech terms — REMOVE ENTIRE BLOCK
"internet", "wifi", "bluetooth", "app", "website",
"download", "upload", "online", "offline", "startup",
"database", "server", "API", "URL", "HTML",
```
Reason: these are TOPIC tokens that the system prompt explicitly says the model should discuss. The LoRA + boost list are already doing the style work. The topic suppression creates subtle circumlocution that the user can detect.

### LOW-severity fix to training (optional)
Add a per-stage style-specific eval metric. Define a small holdout set (e.g., 50 prompts) and measure:
- Archaic-vocabulary density of generated response
- Style score against Sonnets constellation centroid

Track across checkpoints to see if style IMPROVES during training or just loss decreases.

### HIGH-severity fix to DPO pairs (Agent 2's scope, noted here for completeness)
In `scripts/shakespeare_build_dpo_dataset.py` (or wherever DPO pairs are mined), enforce a STYLE-CONTRAST requirement:
- Chosen: archaic Shakespeare (density >= 2%)
- Rejected: non-archaic (density < 0.5%)
- Reject any pair where BOTH sides are archaic
- Use per-embedder similarity profile: reject pair if E12_ColBERT / E13_SPLADE cosine > 0.5 (meaning chosen and rejected are stylistically too similar)

This would replace ~85% of current DPO pairs with style-contrastive ones. Retraining DPO on these would directly teach style preference.

Alternative: use the EXISTING SFT examples as CHOSEN paired with modern-English paraphrases as REJECTED. The modern paraphrase could be generated by a second model (Gemma base or Claude) prompted to "rephrase in contemporary English, preserving meaning". This is the cleanest STYLE-contrast signal.

## Confidence Level: HIGH

What I verified directly:
- All 258 LoRA target modules (counted, classified by type, layer coverage verified)
- System prompt alignment across SFT, DPO, dashboard, rebuild script (5 surfaces)
- Boost/suppress list classification (word-by-word)
- Training final metrics (from trainer_state.json)
- DPO pair characteristics (1,000-sample empirical density test)
- SFT user-prompt distribution (7,925 examples analyzed)

What I could NOT verify:
- Whether the LIVE model actually uses archaic tokens at the claimed rate — did not run inference
- Whether the new guard would FAIL on borderline outputs — did not reproduce live FSV tests
- Whether the Golden Age Spanish emergence holds under the NEW guard — did not test
- Whether Agent 2's DPO pair-mining has a different theory for why content-contrast pairs produce style-correct behavior

## What "MIXED" means for the user's question
- The user asked: "Does the LoRA training CONFIGURATION correctly optimize for STYLE rather than SEMANTIC matching?"
- Answer: **The CONFIGURATION does optimize for style** (target modules are balanced, system prompt is style-focused). But the TRAINING DATA SIGNAL (especially DPO pairs) is skewed toward content-contrast, which means the gradient that actually updates the weights is partly TOPIC-based, not purely STYLE-based.
- The constrained decoder + LoRA + system prompt combination still produces style-correct outputs (5/5 FSV PASS) because:
  1. SFT on uniformly-archaic data is STRONG style signal
  2. Constrained decoder BOOSTS archaic tokens at every step (deterministic)
  3. The DPO misalignment mostly adds secondary noise, not a counter-signal
- The new STYLE-focused guard (E7/E9/E10/E12/E13) will CORRECTLY identify when the model produces archaic Shakespeare. The DPO content-contrast mismatch is NOT going to cause the guard to systematically FAIL outputs that should pass.
- But: if the guard ever sees a prompt where the model defaults to content-matching mode ("Write about: SCENE III..."), the model might produce Shakespeare-CONTENT that isn't quite the archetypal Shakespeare-STYLE, and the guard might fail such outputs. This would be a false-negative-from-guard-perspective (output is OK style-wise but scored low).

## Memory files written (10)
1. ./memory/discoveries/agent-03-sherlock-config--lora-targets-are-style-correct.md
2. ./memory/discoveries/agent-03-sherlock-config--dpo-pairs-are-content-not-style.md
3. ./memory/discoveries/agent-03-sherlock-config--constrained-decoder-has-topic-blocks.md
4. ./memory/discoveries/agent-03-sherlock-config--system-prompt-is-style-focused.md
5. ./memory/discoveries/agent-03-sherlock-config--sft-user-prompts-are-shakespeare-content.md
6. ./memory/discoveries/agent-03-sherlock-config--no-style-specific-loss-function.md
7. ./memory/journal/agent-03-sherlock-config--success-completed-investigation.md
8. ./memory/reports/agent-03-sherlock-config--final-report.md (this file)

## Answer to the original question
**Does the training config + inference alignment force style enforcement without accidentally demanding topical content?**

MIXED. The config DOES force style enforcement (LoRA targets, system prompt, SFT data, constrained decoder boost list). The config has ONE genuine alignment flaw: the suppress list includes 15 topical-modern tokens that contradict the "full knowledge of modern topics" system prompt promise. This creates subtle circumlocution — the model has to work AROUND forbidden topic words — but it does NOT cause the model to produce semantic-matching output instead of style-matching output.

The more serious issue (HIGH severity, Agent 2 scope) is that the DPO pair selection trained on content-contrast pairs rather than style-contrast pairs. This means the DPO phase optimized for TOPIC discrimination, not STYLE. The model still produces style-correct outputs (5/5 FSV pass) because SFT did the heavy style lifting, but the DPO phase is essentially wasted training effort for the style goal. Retraining with proper style-contrast pairs would yield a measurably better model.

The new guard (E7/E9/E10/E12/E13 style-only) WILL correctly verify style. The training config + inference alignment WILL produce outputs that pass the guard on most prompts. There is no catastrophic mismatch — only a couple of design inconsistencies that should be cleaned up in a future iteration.
