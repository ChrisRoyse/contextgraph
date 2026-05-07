---
namespace: journal
agent_id: agent-06-coder-sft-aug
timestamp: 2026-04-17T17:00:00Z
status: active
tags: journal, start, phase-d, sft-augmentation
---

# Journal: Context loaded, starting Phase D

## What I Read
- mission-resurrection-plan.md — 7-phase plan; Phase D spec: ~1,500 SFT + 150 adversarial, daemon DOWN for Gemma, UP for scoring
- coordinator--stylistic-tag-schema.md — schema for Phase B tags (register primary/secondary, meter.form, semantic_topic). Phase D uses register distribution to BALANCE augmentation
- mission-brief-shakespeare-guard-hardening.md — three-layer guard + 13-embedder constellation scoring
- SHARED-MIND-AGENT-PROTOCOL.md — SMAP lifecycle
- shakespeare_generate_compositions.py — prior art: Ollama-based, non-daemon-scored, no retry loop
- shakespeare_enhance_prompts.py — prior art: Ollama-based
- shakespeare_rebuild_all_data.py — UNIFIED_SYSTEM prompt (27-line canonical system message)
- shakespeare_chat_dashboard.py — DaemonScorer MCP protocol with store → score_against_constellation → forget pattern
- GOLDEN_AGE_SPANISH_EMERGENT_BEHAVIOR.md — Siglo de Oro markers (vos/vuestro, ojalás, ocaso, reino celestial, polysyndeton "y... y... y...")
- shakespeare_sft_final_clean.jsonl — 7,925 lines, SFT format `{messages:[{system,user,assistant}], _meta:{...}}`

## State at Start
- Daemon up on :3100 (20.7 GB VRAM) — will need to stop before Gemma load
- Inference server off (8766 not listening) — correct for Phase D
- Gemma-4-E4B-it present in HF cache ✓
- GPU: 21,181 MB used / 32,607 MB total — 11 GB free
- transformers 5.5.4, torch 2.11.0+cu130, CUDA available ✓
- Phase B output `shakespeare_stylistic_tags.jsonl` DOES NOT exist yet — Agent 4 hasn't run

## Plan
1. Build `scripts/shakespeare_sft_augmenter.py` with 3 subcommands via internal phases:
   - Phase 1: Load curated modern-topic prompt bank + register templates
   - Phase 2: Stop daemon (optional --auto-daemon), load Gemma, generate candidates into .candidates.jsonl
   - Phase 3: Unload Gemma, restart daemon, score candidates via constellation, reject and regenerate below threshold
   - Phase 4: Atomically write resurrection SFT + adversarial outputs
2. Resumable: inspect existing .candidates.jsonl / .partial output to skip done work
3. Mini test with --target-count 20 end-to-end

## Non-Negotiables
- NO workarounds, NO fallbacks, fail-fast on Gemma load error or daemon scoring error
- NO mock data, every candidate goes through constellation scoring
