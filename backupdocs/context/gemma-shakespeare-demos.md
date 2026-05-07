---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T21:00:00Z
status: active
tags: gemma, shakespeare, demos, gtm, mission
---

# Mission: 6 Gemma+Shakespeare Demos for Teleox GTM

## What we have
- **Gemma 4 e4b** (8B params, 131K context, multimodal, thinking, Q4_K_M) at `ollama run gemma4:e4b`
- **Context Graph daemon** on port 3100 with **2,741 Shakespeare memories**, **44 work-constellations**, **13,465 contrastive_pairs** in RocksDB
- **13 frozen embedders** running on RTX 5090 (Blackwell sm_120a, CUDA 13.2 PTX)
- **Authenticity Detector** already built (`scripts/shakespeare_authenticity.py`) — works, tested 4 inputs

## What we must build (NO FALLBACKS, fail fast)
1. **Constellation-Guard for Gemma** (`shakespeare_guard.py`) — generate via Gemma, embed output, score vs constellations, REJECT and regenerate if off-manifold. Live "try to break it" demo.
2. **Auto-Genre Routing** (`shakespeare_genre_router.py`) — embed prompt, find closest of 44 constellations, route Gemma with that constellation's tightest member as system context.
3. **Style Transfer** (`shakespeare_translate.py`) — modern→Shakespeare and Shakespeare→modern via Gemma + guard.
4. **Sonnet Completer** (`shakespeare_sonnet.py`) — first line in → 14-line ABAB-CDCD-EFEF-GG sonnet via Gemma + meter/rhyme verifier + constellation guard.
5. **Contrastive RLHF Export** (`shakespeare_rlhf.py`) — query CF_CONTRASTIVE_PAIRS via MCP, write DPO/ORPO JSONL with chosen/rejected (= anchor/negative texts).
6. **Verify Authenticity Detector across 8 fixed test cases** with before/after CF state.

## Source of truth
- `scripts/shakespeare_*.py` — code on disk
- `/tmp/shakespeare_*_out/*.jsonl` — generated outputs
- RocksDB CFs (cross-checked via `ldb --secondary_path`):
  - `CF_FINGERPRINTS`, `CF_CONSTELLATIONS`, `CF_CONTRASTIVE_PAIRS`
- Daemon on port 3100, queried via TCP JSON-RPC

## Hard rules
- NO fallbacks / mock data / backwards compat
- Every script fails fast with a descriptive error
- Each demo has a "synthetic test" with known input → known expected output
- Code-simplifier reviews all 6 scripts at the end
- Final report `tasks/GEMMA_DEMOS_FSV.md` with physical evidence
