# Phase 1 — Preconditions

## Required state at start

1. **Phase 0 is complete.** `git rev-parse --quiet --verify ccreality-cutover` returns 0; `/tmp/ccreality-cutover-base.sha` exists.
2. **Current branch is `ccreality-cutover`.** Verify: `[ "$(git branch --show-current)" = "ccreality-cutover" ]`.
3. **Working tree is in expected state.** Either clean, OR has only the planning-doc changes from prior session (`docs/ccreality/`, `CLAUDE.md`, deleted `docs2/.raw/*`). Run `git status --porcelain` to confirm.
4. **Cargo workspace builds at base SHA.** Verify before touching anything:
   ```bash
   cargo build -p context-graph-cli --bin reality-loop --quiet
   ```
   Exit 0 expected. If this fails, the base state is broken — STOP, report to operator, do not proceed.
5. **Existing tests pass at base SHA.**
   ```bash
   cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored
   ```
   Exit 0 expected. Capture the count of passing tests for comparison after Phase 1.
6. **Tools available.** Confirm: `which rg cargo rustfmt sed jq` all return 0.

## Required reading

Before starting any task, the executing agent must have read:

- [../04_EXCISION_PLAN.md](../04_EXCISION_PLAN.md) (the line-range audit; THE primary reference).
- [../05_REALITY_ENGINE_RESPONSIBILITY.md](../05_REALITY_ENGINE_RESPONSIBILITY.md) (what the engine should look like after).
- [00_SPEC.md](00_SPEC.md) (this phase's deliverables).

## Reference snapshot of `reality-loop.rs` structure

As of 2026-05-05, `crates/context-graph-cli/src/bin/reality-loop.rs` is 10,873 lines. Key landmarks (from the audit):

| Lines | What |
|-------|------|
| 12–42 | Constants block (16 OpenAI/DeepSeek constants to delete; 18 to keep) |
| 46–139 | `struct Args` (17 flags to delete; ~25 to keep) |
| 143–186 | `RealityError` and helpers (KEEP) |
| 187–243 | `ProcessResult`, `TaskManifest`, `GpuState`, `AttemptRecord`, `AgentToolContext` (delete 2 fields from `AttemptRecord`) |
| 258–282 | `main()` (KEEP) |
| 283–1055 | `run()` (DELETE the trigger+optimizer block lines 803–960; KEEP rest) |
| 1056–1148 | `validate_task_manifest_contract` (KEEP) |
| 1149–1500 | problem-reality builder + helpers (KEEP) |
| 1511–1572 | `write_failure_artifact` (DELETE 11 keys from its payload) |
| 1574–1714 | `run_cmd`, `run_cmd_env`, `read_process_output`, `blackbox` (KEEP) |
| 1715–2114 | `ollama_chat_json` (KEEP) |
| 2117–2300 | `validate_optimizer_trigger_config`, `decide_deepseek_optimizer_trigger`, `reward_scalar_from_reality_signal` (DELETE all three) |
| 2302–2347 | `post_ollama_chat` (KEEP) |
| 2349–2627 | `run_openai_loop_optimizer` (DELETE) |
| 2628–2853 | `run_deepseek_optimizer_api_smoke_test` (DELETE) |
| 2853–3486 | DeepSeek/OpenAI HTTP and helpers (DELETE all) |
| 3487–3705 | `OpenAiFunctionCall`, function-call helpers (DELETE) |
| 3707–3907 | `deepseek_optimizer_tool_specs`, `openai_optimizer_tool_specs` (DELETE) |
| 3908–4257 | `execute_openai_optimizer_tool` and result helpers (DELETE) |
| 4258–4801 | Project edit helpers (LIFT to governed_edit.rs) |
| 4802–4985 | `validate_optimizer_project_path` (LIFT) + remaining helpers (DELETE) |
| 4986–5003 | `compact_openai_loop_optimizer` (DELETE) |
| 5004 onward | Agent tools and reality builders (KEEP) |
| 9395–9831 | Generic helpers (KEEP) |
| 9833–10873 | Tests (DELETE 11 named in [../04_EXCISION_PLAN.md](../04_EXCISION_PLAN.md); KEEP the rest) |

## Required commands

Before each task, run `pwd && git branch --show-current` to confirm location and branch.

## Forbidden actions during this phase

- No `git push`.
- No `git reset --hard` except for emergency recovery (see [05_RECOVERY.md](05_RECOVERY.md)).
- No commits until the entire phase verification passes (so the phase is atomic — partial work goes in one commit at end).
- No edits to `crates/context-graph-mcp/`, `crates/context-graph-core/`, or any other crate.

## What to do if a precondition fails

1. Precondition 1 (Phase 0 not done) → execute Phase 0 first.
2. Precondition 2 (wrong branch) → `git checkout ccreality-cutover`. If branch missing, redo Phase 0.
3. Precondition 3 (unexpected dirty tree) → STOP, report to operator. Do not attempt to clean up.
4. Precondition 4 (cargo build fails at base) → STOP, report. Phase 1 is destructive and cannot proceed against a broken base.
5. Precondition 5 (tests fail at base) → STOP, report. Same reason.
6. Precondition 6 (tool missing) → install the missing tool; retry precondition.
