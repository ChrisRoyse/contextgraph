---
namespace: decisions
agent_id: claude-code-coordinator-pivot
timestamp: 2026-05-09T00:30:00Z
status: active
tags: pivot, mejepa, single-outer-loop, qwen-retired, locked, architecture
---

# LOCKED — ME-JEPA Pivot: Single Outer Reality Loop, No Inner LLM

## The decision

The previous "outer Claude + inner Qwen3.6-27B repair actor" two-loop architecture is **retired**. Going forward:

1. There is **one** reality loop only — Claude Code in the outer position.
2. Claude Code is the SWE-bench Lite 300 patch producer (sequentially, one task at a time).
3. ME-JEPA-Code is the predictive verifier being built — non-generative, trained on durable reality artifacts captured by the outer loop.
4. The reality loop's only purpose is to **capture as much reward signal as possible** from durable reality so ME-JEPA can be trained to predict whether code works in reality.
5. No inner-actor LLM. No Qwen, no Hermes, no Ollama, no llama-cpp generative inference.
6. The end goal is `predicted_oracle_pass ≥ 0.95` with calibrated confidence, at which point human code review of AI patches is statistically unjustified.

## Why

Operator directive 2026-05-09: *"the whole purpose of this system is to build the me jepa that can predict if code works or not so software engineers no longer have to review code... the reality loop system is solely to get training data for the me jepa system... it only needs to have one loop... we will be using claude code to pass the swe bench... it is very important this system is self healing and self learning."*

Operator's clarifications throughout 2026-05-09:
- ME-JEPA runs SEPARATE from any LLM.
- ME-JEPA must answer three questions: (1) Does what the AI said it did EXIST? (2) Does it WORK as intended? (3) How is it actually IMPACTING REALITY?
- The system must support any dynamic project in any of the supported coding languages (Python, Rust, C, C++, Java, C#, JS, TS, Ruby).
- UTML v1.0 (`docs2/UTL_v2_embodied.md`) is the source theory for the ME-JEPA learning formula: `L_step = ΔP × ΔK × ΔΩ × ΔΞ` (ΔΞ.target_collapse = 0 by construction in ME-JEPA).

## What was done in the pivot turn (2026-05-09)

- **Disk cleanup (~62 GB freed)**: deleted `models/qwen3-coder-30b-a3b/` (17 GB), `models/hermes-2-pro/` (4.8 GB), all `/tmp/contextgraph-target-*` build caches (~38 GB), `/tmp/contextgraph-rust-reality-loop/` runtime artifacts (251 MB), `contextgraph_data/` legacy RocksDB (206 MB), `data/utl_calibration` (2.4 GB), `data/memories/`, `logs/run_evaluation/`. Embedder weights (bge-m3-dense, code-1536, kepler, splade-v3, etc.) preserved.
- **New canonical docs**:
  - `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` — post-pivot architecture statement.
  - `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 `L_step = ΔP × ΔK × ΔΩ × ΔΞ` (ΔΞ.target_collapse = 0 by construction in ME-JEPA) operationalized for ME-JEPA.
- **Doc 09** (master plan) — pivot banner added; Qwen-specific lines re-framed (live attempts → outer-loop observations; Qwen CoT → outer-loop agent reasoning trace; reality-loop cli → mejepa-verify cli).
- **Docs 00-08** — pivot banners pointing to docs 10 + 11.
- **CLAUDE.md** — §1 (project at-a-glance) + §11 (operator role) + §13 (artifact layout) rewritten to reflect single-outer-loop architecture.
- **`.claude/loop.md`** — completely refactored (478 → 280 lines). Outer-loop only; two turn modes (Build mode shipping ME-JEPA primitives; Reality-capture mode attempting SWE-bench tasks); UTML v1.0 formula embedded; self-healing/self-learning properties enumerated.
- **Sitrep docs** — surgical edits to remove Qwen-specific instrument descriptions; pivot banner on `01_status_overview.md`.
- **Active state cleared** — `~/.cache/cgreality/active_runtime_root` and `active_target_instance` cleared so post-edit hooks take the clean "no active runtime root" path.

## Locked rules going forward

1. **No inner-actor LLM** — never reintroduce Qwen, Hermes, Ollama, or llama-cpp generative inference. Pivot is intentional.
2. **One outer loop** — Claude Code is the patch producer. /loop turns operate in Build mode (default until verify() ships) or Reality-capture mode.
3. **Capture every reward signal** — every Edit/Write produces a shift-log entry; every test run produces an oracle outcome; every recommendation binds reasoning to action. The richer the capture, the better ME-JEPA learns.
4. **One test at a time** — work through SWE-bench Lite 300 in queue order; park tasks with 3+ consecutive failures; resume them after ME-JEPA improves.
5. **Self-healing / self-learning is structural** — every primitive shipped should preserve or strengthen these properties (see `loop.md §7`).
6. **UTML v1.0 learning formula is the per-example weighting** — `L_step(ex) = ΔP × ΔK × ΔΩ × ΔΞ` clamped [0,1]. Examples low in any factor contribute nothing. ΔΞ.target_collapse = 0 by construction in ME-JEPA (frozen externally-defined instruments). Predictor-side ΔΞ defenses: VICReg + pairwise MI audit + constellation guard + negative-action ablation falsifier. See `11_MEJEPA_LEARNING_FORMULA.md`.

## What this supersedes

- `agents/cc-reality-harness-editor.md` and `cc-reality-diagnostician.md` etc. — these subagents were specialized for inner-actor harness work. They remain available but their typical use-case (editing the inner-actor harness) is retired. They can still be used for general harness/source edits.
- `memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md` — superseded; mark `status: superseded`.
- `memory/decisions/agent-79-harness-editor--tool-result-history-cap-tightened-to-4kb.md` — still valid for general Claude Code conversation hygiene.
- `memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md` — superseded; the `reality_run_attempt` MCP tool is queued for retirement.

## Migration follow-ups (queued for future turns)

The following source-code retirements are authorized in principle but not yet executed:

- Delete `crates/context-graph-cli/src/bin/reality-loop.rs` (Qwen GGUF host binary).
- Delete `crates/context-graph-causal-agent/src/llm/mod.rs` (Hermes GGUF host).
- Delete `vendor/llama-cpp-sys-2-0.1.145/` and `[patch.crates-io]` entry.
- Drop `llama-cpp-2` dep from `Cargo.toml` of `context-graph-cli` + `context-graph-causal-agent`.
- Retire `mcp__cgreality__reality_run_attempt` MCP tool; replace with `mejepa_verify` (Phase 7 deliverable).
- Update `docs/ccreality/01_ARCHITECTURE.md` and friends to reflect post-pivot state.

These were not done this turn because the operator authorized "Aggressive" cleanup of *data*, not source-code refactor. A follow-up turn should retire them.

## Related

- Pivot statement: `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md`
- Learning formula: `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`
- Master plan (re-framed): `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
- Operator role contract: `CLAUDE.md §11`
- Per-tick prompt: `.claude/loop.md`
- UTML v1.0 source theory: `docs2/UTL_v2_embodied.md`
