---
namespace: journal
agent_id: agent-03-sherlock-config
timestamp: 2026-04-17T20:30:00Z
status: active
tags: success, investigation-complete, forensic
---

# Investigation Complete — Training Config + Inference Alignment

## Files examined
1. `scripts/shakespeare_full_pipeline.py` (605 lines) — main SFT+DPO pipeline
2. `scripts/shakespeare_train_lora.py` (344 lines) — standalone trainer
3. `scripts/shakespeare_inference_server.py` (313 lines) — inference with constrained decoder
4. `scripts/shakespeare_chat_dashboard.py` (1,206 lines) — dashboard with 3-layer guard
5. `scripts/shakespeare_rebuild_all_data.py` (first 280 lines) — data rebuilder
6. `~/.contextgraph/models/shakespeare_lora/final/adapter_config.json` — 258 target modules
7. `~/.contextgraph/models/shakespeare_lora/sft_checkpoint/checkpoint-744/trainer_state.json`
8. `~/.contextgraph/models/shakespeare_lora/dpo_checkpoint/checkpoint-367/trainer_state.json`
9. `~/.contextgraph/training/shakespeare_sft_final_clean.jsonl` (7,925 examples sampled)
10. `~/.contextgraph/training/shakespeare_dpo_clean.jsonl` (11,724 pairs, 1,000 sampled)
11. `~/.contextgraph/training/shakespeare_compositions_clean.jsonl` (323 examples)
12. `~/.contextgraph/training/shakespeare_adversarial_clean.jsonl` (43 examples)
13. `docs/SHAKESPEARE_LORA_FINAL_REPORT.md` (358 lines)
14. `docs2/LORA_STYLE_TRANSFER_COMPLETE_REPORT.md` (first 350 lines)
15. `tmp/SITREP_2026-04-17_102816.md` (261 lines)
16. `tmp/SITREP_2026-04-16_135543.md` (261 lines)

## Key findings
1. **LoRA target modules are style-correct** — 132 attention (51.2%) + 126 MLP (48.8%) across all 42 layers. Rank 256 / alpha 512 is standard for high-capacity override of base model. BALANCED coverage, not skewed toward any one module type.
2. **System prompt is unified and style-focused** — same text in SFT, DPO, and dashboard. Says "writes EXCLUSIVELY in the style of Shakespeare" (STYLE), not "You are Shakespeare" (PERSONA). Explicitly allows full modern-topic knowledge.
3. **Constrained decoder boost list is style-correct** — 77 archaic tokens, mostly pronouns/interjections/contractions.
4. **Constrained decoder suppress list is MIXED** — 33 style-breaking + 15 TOPICAL-modern (internet, wifi, API, etc.) + 31 language (Spanish/French/German). The 15 topical tokens contradict the system prompt's "full knowledge of modern topics" promise.
5. **DPO pairs are content-contrast, NOT style-contrast** — 65% of pairs have BOTH chosen and rejected as archaic Shakespeare. 15.5% have REJECTED more archaic than CHOSEN. Only 7.1% are correctly style-contrastive. This is Agent 2's scope but highly relevant.
6. **SFT user-prompt distribution is Shakespeare-content-heavy** — only ~20% of prompts touch modern topics. Model is undertrained for arbitrary-topic requests.
7. **Standard cross-entropy loss** — no style-weighted loss, no style-specific eval metric. Correct given data distribution.

## Time taken
~45 minutes of forensic investigation.

## Confidence level
HIGH on all findings. Evidence is from direct file inspection, config JSON, trainer state JSON, and 1,000-pair empirical sampling.

## Limitations
- Did not run the inference server live to reproduce the 5/5 FSV test
- Did not run the dashboard to test new guard verdict on each prompt type
- Did not measure the model's token-level archaic-vocabulary usage on arbitrary modern prompts
- Agent 2's DPO pair-mining is out of my scope — my analysis of DPO data is supportive, not definitive

## Related
- Final report: ./memory/reports/agent-03-sherlock-config--final-report.md
