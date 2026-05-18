---
namespace: discoveries
agent_id: agent-codex
created: 2026-05-13T10:45:00Z
updated: 2026-05-13T10:47:41Z
status: active
tags: phase-h,inner-loop,models,audit,github-134
---

# Phase H Inner-Actor Model Audit

`scripts/audit/no_inner_actor_models.sh` scans the live D-drive model root
`/mnt/d/cabdru/contextgraph/models` plus the active source tree at
`/home/cabdru/contextgraph` for retired inner-actor path names:

- Qwen3.6-27B model paths
- Qwen3-Coder-30B-A3B model paths
- Hermes-2-Pro-Mistral-7B model paths
- Ollama model/runtime directories
- llama.cpp generative inference binary/vendor paths

The real scan returned `match_count=0` and `missing_root_count=0`.

FSV evidence:

- `/mnt/d/contextgraph/fsv/phase-h-no-inner-actor-models-fsv/no_inner_actor_models_fsv.json`
- sha256: `d096a20f89018198f07477b9f8723b461d6c4dff66155191d33d6db8f307636d`
- `build_release_sha=cbe5f50b`
- `all_passed=true`, `readback_equal=true`

Boundary readback created synthetic retired paths under the FSV run root and
verified the scanner exits nonzero while detecting Qwen3.6-27B, Hermes,
Ollama, and llama.cpp placeholders.
