# Target Architecture — fully realized meta-reality-loop

This is the end-state diagram and the responsibility split once phases 13–22 are complete. It is a *strict superset* of the architecture in `docs/ccreality/01_ARCHITECTURE.md`; nothing already shipped is removed.

## One picture

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                      OPERATOR / EXTERNAL WATCHER                             │
│  /loop 30m   |   cron tick    |   interactive operator session                │
└────────────────────────────────┬─────────────────────────────────────────────┘
                                 │ launches Claude Code
┌────────────────────────────────▼─────────────────────────────────────────────┐
│                              CLAUDE CODE  (outer reality-loop optimizer)      │
│                                                                               │
│  ┌── reality drip via hooks ──┐   ┌── /loop body ──┐   ┌── subagents ──┐      │
│  │ SessionStart, UserPromptSubmit│   │ diagnose →      │   │ diagnostician │      │
│  │ PreEdit/PostEdit, Stop,       │   │ edit → verify  │   │ harness-editor│      │
│  │ PreCompact                    │   │ → record       │   │ readback      │      │
│  └───────────────────────────────┘   │                 │   │ reward-analyst│      │
│                                       └────────────────┘   │ swe-triage    │      │
│                                                            └───────────────┘      │
│  ┌── ALWAYS-ON cgreality MCP ───────────────────────────────────────────┐         │
│  │ ANALYZE  reality_*, search_*, traverse_*, audit_*, provenance_*       │         │
│  │ INTERACT reality_run_*, replay, recompile_reward, query_ledger        │         │
│  │ ALTER    harness_*, store_memory, boost_importance, optimizer_record_*│         │
│  │ SHIFT    shift_log, shift_compare_to_my_view                          │         │
│  └──────────────────────────────────────────────────────────────────────┘         │
└────────────────────────────────┬─────────────────────────────────────────────────┘
                                 │ MCP stdio (JSON-RPC)
┌────────────────────────────────▼─────────────────────────────────────────────────┐
│                          CONTEXTGRAPH (Rust workspace)                            │
│                                                                                   │
│  ┌── reality engine ──────┐  ┌── memory store (RocksDB 101 CFs) ─────────────┐    │
│  │ context-graph-cli      │  │ 13 production embedders, HNSW, ledger, audit, │    │
│  │   reality-loop binary  │  │ provenance, causal graph, harness deltas,     │    │
│  │   attempt|smoke|replay │  │ DynamicJEPA panels, training samples          │    │
│  │   verify|official-eval │  └────────────────────────────────────────────────┘    │
│  └────────────┬───────────┘                                                       │
│               │                                                                   │
│               ▼                                                                   │
│  ┌── durable reality artifacts (per run) ────────────────────────────────────┐    │
│  │ <run_root>/<task>/attempt-N/  problem-reality, reality-signal,           │    │
│  │   harness-transitions, official-evidence, attempt-summary,               │    │
│  │   claude-code-optimizer/{recommendation,decision,commands,verification}/ │    │
│  │   ledger.sqlite (BlackBox), reality-events.jsonl,                        │    │
│  │   cgreality-shift-log/<session>.jsonl                                    │    │
│  │ <run_root>/reality-optimizer/  experiment-registry, champion-state,      │    │
│  │   attempts.jsonl, prompt-compiler-runs.jsonl, reward-series.jsonl        │    │
│  │ <run_root>/training/  positive, negative, preference_pairs,              │    │
│  │   rejected_patches (post-Phase 18)                                       │    │
│  │ <run_root>/reality-scoreboard/  attempt-, model-task-, artifact-         │    │
│  │   scoreboard, reality-branches, reality-scoreboard.md (post-Phase 20)    │    │
│  └──────────────────────────────────────────────────────────────────────────┘    │
│                                                                                   │
│  ┌── inner SWE actor (THREE local models) ─────────────────────────────────┐     │
│  │  gemma4:e4b   ←  governed coding-agent harness  → real repo / git apply │     │
│  │  qwen3:8b      ←   (open_file_window, apply_line_window_edit, ...)       │     │
│  │  mistral-nemo  ← + cross-model verified-failure memory (Phase 16)        │     │
│  │  ↓                                                                        │     │
│  │  official SWE-bench Docker eval → resolved_ids → success predicate       │     │
│  └──────────────────────────────────────────────────────────────────────────┘     │
│                                                                                   │
│  ┌── DynamicJEPA  (Phase 22 integration) ──────────────────────────────────┐     │
│  │  panels, predictions, surprise events, latent state                       │     │
│  │  → DJ surprise becomes reward feature                                     │     │
│  │  → DJ predictions guide optimizer's intervention surface choice           │     │
│  └──────────────────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Three roles  (unchanged from cutover)

- **Gemma 4 (and peers Qwen, Mistral)** — inner SWE-bench repair actor. Reads problem reality, navigates the repo through governed tools, submits a candidate patch via `apply_line_window_edit` or `apply_structured_edit`, gets graded by the official Docker evaluator.
- **ContextGraph (Rust workspace)** — deterministic reality substrate. Runs the inner attempts, runs official SWE-bench Docker eval per attempt, persists every artifact, exposes the artifacts and memory graph through the `cgreality` MCP server, holds the BlackBox ledger and 101-CF RocksDB.
- **Claude Code (outer optimizer)** — reads durable reality, diagnoses why the inner loop failed, edits the harness/prompts/tool surface/reward features through governed MCP `alter` tools, triggers re-attempts, records a structured recommendation per turn.

## Forward additions vs the cutover architecture

### A. Auto-research engine layer (Phase 15)

```
<run_root>/reality-optimizer/
  champion-state.json          ← best (model, task) result so far
  experiment-registry.json     ← list of every harness change candidate, status
  attempts.jsonl               ← compact one-line per attempt for analytics
  prompt-compiler-runs.jsonl   ← every prompt manifest hash + outcome
  reward-series.jsonl          ← reward over time
```

Claude Code reads these to know "am I improving?"; the engine writes them. The MCP server gets two new tools: `experiment_registry_list`, `champion_state_get`.

### B. Cross-model layer (Phase 16)

- Engine accepts `--model gemma4:e4b,qwen3:8b,mistral-nemo:12b` as a matrix.
- Per-(model, task, attempt) result rows in the registry.
- Shared verified-failure memory in the ContextGraph memory graph (namespace `reality_loop_cross_model`). Memories are stored only after official Docker readback proves the failure.
- Inner-loop prompt compiler reads cross-model facts: "Gemma already tried patch sha256:abc...; Qwen failed on test foo()." (verified facts only, never raw model claims).

### C. Reward shadow layer (Phase 17)

- Production reward spec: `gemmafrontend/reward_spec.verified_outcome.json` (read-only to the optimizer).
- Shadow specs: `<run_root>/reality-optimizer/reward-shadow/<spec-id>.json`.
- For every existing attempt's reality-signal packet, the engine recompiles reward against each shadow spec and writes `shadow-reward-<spec-id>.json` beside the canonical reward record.
- A shadow spec is promotable to production only if (a) Pearson correlation of its scalar with `official_resolved` on a held-out attempt set exceeds the production spec's correlation by a threshold, AND (b) human review approves.

### D. Training export layer (Phase 18)

- After every attempt with official readback, episodic samples are appended to:
  ```
  <run_root>/training/positive.jsonl       (resolved + clean)
  <run_root>/training/negative.jsonl       (unresolved with verified failure)
  <run_root>/training/preference_pairs.jsonl  (same task, higher reward preferred)
  <run_root>/training/rejected_patches.jsonl  (malformed/repeated patches)
  ```
- LoRA training driver (`crates/context-graph-embeddings/src/bin/train_causal.rs` and friends) reads only these files for reality-grounded fine-tuning.

### E. Curriculum + promotion layer (Phase 19)

- `gemmafrontend/swe_smoke_benchmark_suite.json` extended with `dev`, `holdout`, `promotion` splits.
- Smoke run only against `dev`. Holdout run on schedule (weekly). Promotion run only after holdout PASS.
- Champion is not promoted unless promotion split shows no source-of-truth coverage regression, no reward hacking flag, and incident rate ≤ baseline.

### F. Dashboard layer (Phase 20)

- New tool: `gemmafrontend/scoreboard_server.py` (or extension of `server.py`) that reads:
  - Every `claude-code-optimizer/recommendation-*.json` in the active runtime root.
  - The `experiment-registry.json` and `champion-state.json`.
  - The BlackBox ledger.
  - Live shift log.
- Renders three scoreboards (attempt, model-task, artifact) per `docs/reality/14_operational_scoreboard.md`.
- `reality_score` is computed and surfaced.

### G. DynamicJEPA reward integration (Phase 22)

- Each attempt's reality-signal packet gains a `dynamicjepa_reality` block with surprise count, latent-panel divergence, prediction error, plan-trace coverage.
- `gemmafrontend/reward_spec.verified_outcome.json` gains `dynamicjepa_surprise_penalty` (negative weight) and `dynamicjepa_prediction_accuracy_bonus` (positive weight). Shadow-tested first per Phase 17 rule.

## What stays sacred (never altered by any forward phase)

- `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md` — no alteration without current state, no alteration without delta back into perception.
- The deny list in `config/ccreality_governed_paths.json` — `.env`, `swebench/**`, `tests/oracle_**`, reward specs, verifiers.
- The success predicate — official `resolved_ids` only.
- One source-editing optimizer at a time (no parallel writers on the same file).
- Pre-production: no backwards-compat shims (per `feedback_no_backwards_compat`).
