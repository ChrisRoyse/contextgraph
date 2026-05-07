---
namespace: context
agent_id: coordinator
timestamp: 2026-04-22T00:30:00Z
status: active
tags: v4.2, extended-ensemble, bge-m3, jina-v3, qwen3, reranker, coordinator, phase-start
---

# Current Phase: v4.2 Extended Embedder Ensemble Implementation

## Mission
Extend the v4.2 CAMD-DPO stack with additional SOTA embedder views beyond the
daemon's built-in 13, driven by the finding in
`discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md`
that the existing 13-embedder signal is not style-discriminative enough for
production DPO training.

User directive (verbatim paraphrase from this session):
> "Build out everything and use all advanced techniques. No workarounds or
> fallbacks. If something doesn't work it should error out with robust logging.
> Use real data in tests. Verify subagents' work personally. Run ALL subagents
> synchronously in correct dependency order. Full State Verification — check
> source-of-truth (RocksDB, filesystem, model outputs) not just return values.
> code-simplifier reviews at end."

## Scope

ADD to v4.2 stack (Python-side, no Rust daemon changes):
- E14: BGE-M3 (dense + sparse + multi-vector, 8K context, 100+ languages)
- E15: Jina-embeddings-v3 with `classification` task-LoRA (style-targeted)
- E16: Qwen3-Embedding-0.6B with instruction prompt
- E17: STAR/LUAR-style authorship embedder (option: fine-tune ModernBERT on our
       labeled Shakespeare-vs-non-Shakespeare corpus)
- Reranker post-stage: BGE-reranker-v2-m3 cross-encoder

Techniques (retrofit):
- Late chunking (Jina method) for long-context embedders
- Matryoshka shortening (128D fast-path + 1024D final)
- Task prompts for instruction-aware models

OUT OF SCOPE this phase:
- Actual DPO training run
- HF push
- Browser FSV
- Rust daemon modifications (would require ONNX integration rework)

## Hard constraints (user directives)

1. NO workarounds, NO fallbacks. If a model fails to load, ERROR OUT with
   detailed logging; do NOT silently degrade.
2. NO mock data in tests. Use real Shakespeare text, real modern text, real
   Sonnets references.
3. NO tests that pass when broken. Every test must verify source-of-truth
   (disk/DB/embedding contents).
4. NO backwards compatibility. If interfaces change, update call sites.
5. Synchronous subagent coordination per SMAP — each writes to `./memory/`
   with frontmatter; coordinator verifies each output personally before
   spawning next.
6. Full State Verification before declaring any task done.

## Success criteria

- Extended ensemble class loads all 4 new embedders on RTX 5090 with < 15 GB
  VRAM total (current free budget 14.5 GB).
- Each new embedder produces a non-zero discrimination gap between a known
  Shakespeare sample and a known modern sample (gap ≥ 0.10 for dense
  embedders, ≥ 0.08 for sparse/multi-vec).
- Sonnets reference vectors persisted to
  `~/.contextgraph/models/extended_ensemble/sonnets_references.npz`
  with provenance metadata.
- Extended CAMD reward function integrates daemon-13 + ensemble-5 views;
  passes all 3 validator tests with real mined pairs.
- Sonnet 18 test passage scores > 0.60 on extended combined reward.
- code-simplifier review passes (PASS or applied-refinement).
- SMAP memory coordinator trail complete.

## Agent pipeline (sequential, dependency-ordered)

| # | Agent ID | Role | Depends on | Memory trail |
|---|----------|------|-----------|--------------|
| 1 | agent-v42-1-ensemble-builder | coder | nothing | writes scripts/v4_2/extended_ensemble.py + smoke-test output |
| 2 | agent-v42-2-reference-builder | coder | Agent 1's ensemble class | writes scripts/v4_2/build_references.py + references.npz |
| 3 | agent-v42-3-integrator | coder | Agent 1 + 2 | updates scripts/v4_2/camd_lib.py + extended tests |
| 4 | agent-v42-4-verifier | sherlock-holmes | Agent 3 | real-data E2E, physical verification |
| 5 | agent-v42-5-simplifier | code-simplifier | Agent 4 | reviews all v4_2 changes |

Coordinator (me) verifies each agent's artifacts AND memory writes between
spawns. If any agent fails the verification gate, STOP, diagnose, re-spawn.

## Related files to read

- `docs/V4_2_NOVEL_METHODS.md` — full 15-method plan (Methods 1,2,4 baselined)
- `scripts/v4_2/camd_lib.py` — existing ConstellationScorer (will be extended)
- `scripts/v4_2/mine_camd_pairs.py` — CLI miner (will consume extended reward)
- `scripts/v4_2/train_camd_dpo.py` — DPO trainer stub (will train on v4.2 pairs)
- `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md` — embedder dimension specs
- `memory/discoveries/agent-02-sherlock-dpo--pair-mining-uses-semantic-not-style.md` — THE core motivation
- `memory/handoff/coordinator-v4_1-shipped-v4_2-candidates.md` — v4.2 candidates

## Daemon state expectations

- Context Graph daemon running on 127.0.0.1:3100
- 13,465 CF_CONTRASTIVE_PAIRS available (but low style-discriminative yield;
  see Sherlock discovery)
- Sonnets constellation `18ef2460-9ab6-44fc-8d7e-8d7513174ac8` with 313 members
- `list_constellation_members` or `get_constellation` returns member_ids

## Fresh HF token available

Coordinator received HF_TOKEN this session. Tokens-requiring model downloads
(BGE-M3, Jina-v3, Qwen3-Embedding, BGE-reranker-v2-m3) should succeed.
