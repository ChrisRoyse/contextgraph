# 02 Research Notes

This file records the external research pass and how it changes the plan. Sources are primary sources or official documentation.

## LoRA Supports The Adapter Strategy

LoRA freezes the base model and injects low-rank trainable matrices, reducing trainable parameters and memory while preserving quality for adaptation tasks.

Source:

- Hu et al., "LoRA: Low-Rank Adaptation of Large Language Models", arXiv 2106.09685: https://arxiv.org/abs/2106.09685
- Hugging Face PEFT LoRA docs: https://huggingface.co/docs/peft/en/package_reference/lora

Planning implication:

- Keep rank 256 and alpha 512 for a strong style adapter because the project's empirical record says lower ranks did not dominate the base model under style attacks.
- Keep explicit language-model target modules. Do not use broad shorthand that catches Gemma multimodal towers.
- Treat LoRA as the style prior, not as the sole enforcement mechanism.

## DPO Is Still The Right Alignment Class, But Pair Quality Is Everything

DPO optimizes preference pairs without a separate learned reward model or PPO loop. TRL supports multiple DPO-family losses and options such as precomputed reference logprobs to reduce memory.

Sources:

- Rafailov et al., "Direct Preference Optimization", arXiv 2305.18290: https://arxiv.org/abs/2305.18290
- Hugging Face TRL DPOTrainer docs: https://huggingface.co/docs/trl/v0.26.0/en/dpo_trainer

Planning implication:

- Use DPO or DPO-plus-SFT regularization for style preference training.
- Do not trust old contrastive pairs unless they pass content-same/style-different gates.
- Use `precompute_ref_log_probs=True` or equivalent when VRAM is tight.
- Use label smoothing when pair labels are auto-mined and not human-authored.

## Text Style Transfer Requires Content Preservation

Text style transfer research frames the task as changing style while preserving core meaning. StylePTB stresses fine-grained style dimensions, and STRAP shows a paraphrase framing can work but also warns that automatic metrics can be gamed.

Sources:

- Lyu et al., "StylePTB", arXiv 2104.05196: https://arxiv.org/abs/2104.05196
- Krishna et al., "Reformulating Unsupervised Style Transfer as Paraphrase Generation", EMNLP 2020: https://aclanthology.org/2020.emnlp-main.55/

Planning implication:

- The Shakespeare LoRA should be trained as style transfer over arbitrary content, not as next-scene continuation.
- Evaluation must measure both style strength and content preservation.
- Automatic style scores are not enough. They must be paired with content and coherence checks.

## Repetition Needs Training-Time And Runtime Treatment

Neural text generation can degenerate into repetitive loops under likelihood training and decoding. Unlikelihood training specifically penalizes unwanted repetitions.

Sources:

- Welleck et al., "Neural Text Generation with Unlikelihood Training", arXiv 1908.04319: https://arxiv.org/abs/1908.04319
- Holtzman et al., "The Curious Case of Neural Text Degeneration", arXiv 1904.09751: https://arxiv.org/abs/1904.09751

Planning implication:

- Add repetition-negative DPO pairs and/or an auxiliary unlikelihood loss over loop examples.
- Do not rely only on `repetition_penalty`.
- Add runtime loop detection outside the style score.

## E14 BGE-M3 Should Be Used For Content And Cross-Lingual Equivalence

BGE-M3 supports dense, sparse, and multi-vector retrieval, over 100 languages, and long inputs up to 8,192 tokens.

Source:

- Chen et al., "M3-Embedding", arXiv 2402.03216: https://arxiv.org/abs/2402.03216

Planning implication:

- Use E14 to verify content preservation between prompt, modern reference, and Shakespeare-style response.
- Use E14 for multilingual and translation-pair tests.
- Do not overweight E14 in the style reward until measured style discrimination supports it.

## ColBERT And SPLADE Justify Token-Level Style Diagnostics

ColBERT uses late interaction to retain fine-grained token-level matching. SPLADE produces sparse lexical expansion representations with exact-match-like properties.

Sources:

- Khattab and Zaharia, "ColBERT", arXiv 2004.12832: https://arxiv.org/abs/2004.12832
- Formal et al., "SPLADE", arXiv 2107.05720: https://arxiv.org/abs/2107.05720

Planning implication:

- E12 ColBERT and E13 SPLADE are appropriate style-sensitive views.
- Use E12/E13 for token attribution, style gap detection, and constrained-decoder vocabulary weights.
- E13-derived logit boosts are better than hand-written archaic token lists if validated.

## Structured Output Needs Grammar Constraints

Structured-output literature and official implementation notes agree that constrained decoding is the reliable way to guarantee syntax such as JSON Schema. Training alone does not guarantee schema validity.

Sources:

- JSONSchemaBench, arXiv 2501.10868: https://arxiv.org/abs/2501.10868
- OpenAI Structured Outputs constrained decoding notes: https://openai.com/index/introducing-structured-outputs-in-the-api/

Planning implication:

- For JSON, SQL, and code syntax, use grammar or schema constrained decoding where possible.
- The Shakespeare style layer must not be allowed to corrupt strict syntax.
- FSV must parse or compile generated structured outputs.

## Synthetic Data Needs Verification And Boundaries

Model collapse work is specifically about recursive training on model-generated data, with loss of distribution tails and variance collapse. TCT's fixed-corpus projection avoids that structure when it uses frozen embedders over real data, but teacher-generated compositional bridge examples still need provenance and verification.

Sources:

- Shumailov et al., "AI models collapse when trained on recursively generated data", Nature 2024: https://www.nature.com/articles/s41586-024-07566-y
- Feng et al., "Beyond Model Collapse: Scaling Up with Synthesized Data Requires Verification", arXiv 2406.07515: https://arxiv.org/abs/2406.07515

Planning implication:

- Prefer real Shakespeare as anchor and frozen-embedder labels as supervisory data.
- Use teacher-generated compositional examples only as verified bridge data.
- Never train recursively on unverified outputs from the latest candidate model.
- Keep generated bridge data capped, labeled, overlap-filtered, and guard-scored.

