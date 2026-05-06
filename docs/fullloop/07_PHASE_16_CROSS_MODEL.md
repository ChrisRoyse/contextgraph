# Phase 16 — Cross-Model Inner Loop (RETIRED)

**Status:** RETIRED. The cross-model phase is no longer a forward goal.

The reality loop is intentionally single-model:

- **Production model:** `gemma4:e4b`, cryptographically pinned via `config/ccreality_model_pins.json` and validated at load time by `validate_gemma4_model_identity` in `crates/context-graph-cli/src/bin/reality-loop.rs`.
- **Removed scope:** the original Phase-16 plan to drive `gemma4:e4b`, `qwen3:8b`, and `mistral-nemo:12b` in parallel with shared verified-failure memory was retired so optimizer effort stays focused on harness quality for a single model. The `qwen3:8b` and `mistral-nemo:12b` Ollama models have been removed from the local cache.
- **Engine surface:** `--model` accepts a single model name. There is no comma-separated matrix, no per-(model, task, attempt) result registry, and no `reality_loop_cross_model` memory namespace.

This file is retained as a navigation stub so that links from `00_INDEX.md` and other historical documents continue to resolve. There is no implementation work associated with this phase.

If a future operator wants to revive multi-model coverage, the reintroduction must update the model-pin gate, the harness identity check, the smoke curriculum, the prompt compiler's prior-attempt block, and the success predicate documentation in lockstep, and must justify the reintroduction against the discipline of "concentrate optimizer effort on a single, pinned model."

## Related forward work

- Single-model harness improvements continue under [05_PHASE_14_INNER_CAPABILITY](05_PHASE_14_INNER_CAPABILITY.md) and [06_PHASE_15_AUTORESEARCH_ENGINE](06_PHASE_15_AUTORESEARCH_ENGINE.md).
- The successor of "cross-model verified-failure memory" is the per-task, per-model attempts history under `<run_root>/reality-optimizer/attempts.jsonl`, scoped to the single pinned model.
