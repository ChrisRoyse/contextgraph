# Gap Analysis — what is missing to fully realize the meta-reality-loop

The cutover delivered the plumbing. This doc enumerates the *productive capability* gaps between "smoke green" and the long-term goal documented in `docs/reality/13_path_to_100_percent_swe.md` (the single-model loop driving `gemma4:e4b` resolves every selected smoke instance with full source-of-truth coverage and no reward hacking). Earlier drafts of this gap analysis tracked multi-model coverage as a productive gap; that gap was closed by deliberately scoping the loop to a single model.

## Gap dimensions

The gaps fall into nine dimensions. Each dimension maps to one or more forward phases (13–22) in this directory.

### 1. Inner-loop edit reliability  (→ Phase 13, 14)

Symptom: Phase 12's real attempt ended at `RUST_REALITY_FINAL_EDIT_TOOL_FAILED` because Gemma submitted invalid line-window edits twice. FSV-09 also showed `RUST_REALITY_STRUCTURED_EDIT_FIND_AMBIGUOUS` (find block matched three locations).

Root causes (suspected):
- Gemma's tool-use prompt omits the SHA-handoff requirement clearly enough.
- `apply_line_window_edit` returns errors that don't compose well with Gemma's reasoning trace.
- Repository context selection still occasionally surfaces the wrong file for the issue.
- No structured "second chance" path inside one attempt — failure ends the attempt rather than reprompting Gemma with the rejection reason.

Forward work: in-attempt retry budget for tool-rejection failures, sharpen tool descriptions, and add a `tool_grounding_failure` recovery prompt template.

### 2. Outer-loop sustained iteration  (→ Phase 14, 15)

Symptom: Only one real `/ccreality-iterate` recommendation has been recorded against a real attempt (Phase 12). The loop has never run for hours and accumulated a long shift log.

Forward work: drive `/loop` for an extended period against `psf__requests-2317`, watch shift log growth, prove that diagnoses + harness edits actually shift Gemma's behavior across attempts. Verify by running 5 attempts on the same instance and showing patch parse rate, fail_to_pass count, or reward improving over the sequence.

### 3. Champion state + experiment registry  (→ Phase 15)

Documented in `docs/reality/05_auto_optimization_engine.md` and `docs/reality/14_operational_scoreboard.md` but not built.

Missing artifacts:
```
<run_root>/reality-optimizer/
  experiment-registry.json
  champion-state.json
  attempts.jsonl
  prompt-compiler-runs.jsonl
  reward-series.jsonl
```

Without these, the outer loop cannot answer "what change did I make and what did it produce?" beyond reading individual attempt summaries.

### 4. Cross-model coverage — RETIRED

Earlier project drafts planned multi-model coverage (`gemma4:e4b`, `qwen3:8b`, `mistral-nemo:12b`) with shared verified-failure memory. The plan was retired: the loop is now intentionally single-model (`gemma4:e4b`), cryptographically pinned via `config/ccreality_model_pins.json` and validated at load time. Optimizer effort focuses on harness quality, not on cross-model generalisation.

### 5. Reward optimization (shadow mode)  (→ Phase 17)

`gemmafrontend/reward_spec.verified_outcome.json` is the production reward spec. The `cc-reality-reward-analyst` subagent can propose new components, but there is no mechanism to *try* a candidate reward spec without changing production. No held-out outcome correlation pipeline.

Forward work: shadow-spec slot, replay every existing attempt's signal packet through it, score correlation with verified outcomes before allowing promotion.

### 6. Training data export  (→ Phase 18)

`docs/reality/07_implementation_roadmap.md` Phase 5 calls for verified-episode export to `<run_root>/training/{positive,negative,preference_pairs,rejected_patches}.jsonl`. Not built. LoRA training driver exists (`crates/context-graph-embeddings/src/bin/train_causal.rs`) but is not connected to reality-loop episodes.

### 7. Curriculum + promotion gate  (→ Phase 19)

The smoke suite is intentionally tiny (2 instances). There is no holdout split, no promotion gate, no expansion criterion. `gemmafrontend/swe_smoke_benchmark_suite.json` declares `expand_only_after_manual_review: true`; the manual-review machinery is undefined.

### 8. Operational dashboard + scoreboard  (→ Phase 20)

`docs/reality/14_operational_scoreboard.md` defines three scoreboard levels and a `reality_score` aggregate. None of those JSON/MD outputs are written today. `gemmafrontend/server.py` exposes evidence tools but does not surface ccreality optimizer artifacts (recommendations, shift logs, harness transitions, recommendations per turn).

### 9. DynamicJEPA reward integration  (→ Phase 22)

`docs2/contextgraphspecs/21_dynamicjepa_subsystem.md` describes a 28-CF DynamicJEPA store with surprise events, latent-state predictions, planning traces, etc. Reality-signal packets currently include no DJ projection counts beyond what the reward signal already records. The DJ "surprise" signal is not yet a reward feature, even though `gemmafrontend/reward_spec.verified_outcome.json` reserves space for one.

## Cross-cutting gaps

- **Concurrency** — the engine and MCP both run as single-writer; concurrent `/loop` invocations are not yet stress-tested.
- **External-watcher cron** — `docs/ccreality/14_TRIGGER_AND_INVOCATION_POLICY.md` Section E describes the production watcher; no script exists in repo or `/usr/local/bin/`.
- **Cost cap enforcement** — `cost_estimate_usd` is in the recommendation schema but no enforcement closes the loop when totals exceed a per-day budget.
- **Drift detection coverage** — `reality_shift_compare_to_my_view` is implemented but is not part of the canonical /loop iteration step. Should be invoked at every `/loop` start.
- **One known parser glitch** — the SessionStart hook in this active session reported "Latest attempt summary read failed" against `…/smoke-attempt-1/attempt-summary.json`, but the file is well-formed JSON. Hook script likely tries to parse a substring or expected a key it doesn't find. Tracked under Phase 13.

## Effort sizing (rough)

| Phase | Effort | Risk |
|-------|--------|------|
| 13 hardening | <1 day | low |
| 14 inner capability | 2–4 days | medium (model behavior dependency) |
| 15 autoresearch engine | 2–3 days | low |
| 16 cross-model | 1–2 days | low (mostly plumbing) |
| 17 reward shadow | 2–3 days | medium (correlation methodology) |
| 18 training export | 1–2 days | low |
| 19 curriculum | 1 day | low |
| 20 dashboard | 2–4 days | low |
| 21 production ops | 1 day | low |
| 22 DynamicJEPA reward integration | 3–5 days | high (cross-system) |

Total: ~16–28 days of focused execution. Aggressive timeline assuming `/loop` is sustaining 4–6 hours of unattended optimization work per calendar day.
