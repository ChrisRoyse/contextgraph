# Migration Runbook — execution order for the cutover

Step-by-step plan to apply the ccreality design. Each step has explicit pre/post conditions and a verification command. Do NOT skip steps; the order matters because some later steps depend on earlier-stage artifacts.

Pre-production rule (per project memory): no backwards-compat shims. Wipe DeepSeek/OpenAI cleanly.

## Execution status

As of 2026-05-05:

| Stage | Status | Evidence |
|-------|--------|----------|
| 0 — Branch + safety net | Complete | `ccreality-cutover` exists; `/tmp/ccreality-cutover-base.sha` contains `411443fdcbd8db4005cfe46d44f64f7698c2f11b`. |
| 1 — Excision | Complete | `/tmp/phase-01-verify.log` ends with `Phase 1 verification: ALL PASS`; `/tmp/ccreality-phase1-fsv-output/observations.json` contains manual FSV readback. |
| 2 — Reality-engine subcommands | Complete | `/tmp/phase-02-verify.log` ends with `Phase 2 verification: ALL PASS`; `/tmp/cgreality-phase02-ready` is the successful handoff smoke runtime root. |
| 5 — Config files | Complete | `/tmp/phase-05-verify.log` ends with `Phase 5 verification: ALL PASS`; config files and sample recommendation validate. |
| 3 — MCP `reality_loop` ANALYZE module | Complete | `/tmp/phase-03-verify.log` ends with `Phase 3 verification: ALL PASS`; `tools/list` over `--mode reality-loop` read back the 11 Phase 3 analyze tools and live analyze calls read smoke artifacts. |
| 4 — MCP INTERACT/ALTER/SHIFT tools | Complete | `cargo build -p context-graph-mcp --quiet` and `cargo test -p context-graph-mcp --lib reality_loop --quiet` pass; `/tmp/ccreality-phase4-ledger-fsv-1778000024.log` proves real harness edit, shift log, optimizer decision/recommendation artifacts, and SQLite ledger rows. |
| 6 — Hook scripts | Pending, WIP present | Preconditions are satisfied by completed Phases 0–5. Local WIP exists for the MCP-backed `context-graph-cli ccreality record-harness-transition` support command and `scripts/cc/` hooks, but the scripts are not executable and two tests are missing. Next action is finishing the WIP and running real-runtime hook tests. |
| 7 — Claude scaffolding | Pending | Blocked on Phase 6 hook scripts. |
| 8 — CLAUDE.md addendum | Pending | Blocked on Phase 7 scaffolding. |
| 9–12 — Smoke, FSV, docs cleanup, first production | Pending | Blocked on Phases 6–8. |

## Stage 0 — Branch + safety net

```bash
cd /home/cabdru/contextgraph
git status
git checkout -b ccreality-cutover
git rev-parse HEAD > /tmp/ccreality-cutover-base.sha
```

Pre: clean working tree (or known-acceptable diff documented).
Post: branch created, base SHA captured for rollback.

Current completion record: branch `ccreality-cutover` was created from `411443fdcbd8db4005cfe46d44f64f7698c2f11b`.

## Stage 1 — Excise DeepSeek/OpenAI from `reality-loop.rs`

Apply [04_EXCISION_PLAN](04_EXCISION_PLAN.md) in the order specified there:

1. Lift `project_file_window`, `project_apply_line_window_edits`, `run_openai_project_command`, `verify_openai_project_state`, `validate_optimizer_project_path` into a new module. New file: `crates/context-graph-cli/src/governed_edit.rs` (or a shared crate if one is created during this work). Adapt tests; keep them named for clarity.
2. Delete the 16 constants at lines 22–37.
3. Delete the 17 CLI flag fields in `struct Args` (lines 65, 107–139).
4. Delete the 2 `AttemptRecord` fields (`openai_optimizer_recommendation_path`, `_compact`).
5. Delete the matching fields in `test_args()` (line 9838).
6. Delete the 11 keys in the failure-artifact JSON (lines 1530–1556).
7. Delete the `optimizer_api_smoke_test` short-circuit at lines 326–344.
8. Delete the entire `decide_deepseek_optimizer_trigger` + `run_openai_loop_optimizer` block in `run()` at lines 803–960. Replace with a comment marking this as the engine's "write attempt and exit; outer optimizer lives in MCP" boundary.
9. Delete the 32 functions listed under "DELETE in entirety" in [04_EXCISION_PLAN](04_EXCISION_PLAN.md) §"Top-level functions".
10. Delete the 11 tests listed under "Tests to remove".
11. `cargo fmt -p context-graph-cli`.
12. `cargo check -p context-graph-cli` — fix any unused-import / unused-variable warnings exposed by the deletions.
13. `cargo test -p context-graph-cli --bin reality-loop -- --skip ignored` — remaining tests should pass.

Pre: branch from stage 0.
Post: `reality-loop.rs` is ~7,500 lines; cargo check + cargo test pass; no deepseek/openai/gpt-5 substrings remain (`rg -n "deepseek\|openai\|gpt-5" crates/context-graph-cli/src/bin/reality-loop.rs` returns nothing).

Current completion record: `reality-loop.rs` currently reads back at 7,990 lines; the Phase 1 completion log recorded 7,145 lines. `crates/context-graph-cli/src/governed_edit.rs` exists; `cargo build -p context-graph-cli --bin reality-loop`, `cargo test -p context-graph-cli --bin reality-loop -- --skip ignored`, and `cargo test -p context-graph-cli --lib governed_edit` passed for Phase 1.

Verification:
```bash
rg -n "deepseek|openai|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs
# expected: no matches
cargo build -p context-graph-cli --bin reality-loop
# expected: success
cargo test -p context-graph-cli --bin reality-loop
# expected: all remaining tests pass
```

## Stage 2 — Reality-engine subcommand structure

Refactor `Args` into:
```rust
#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
struct TopArgs {
    #[command(subcommand)]
    cmd: Cmd,
}
#[derive(Subcommand, Debug, Clone)]
enum Cmd {
    Attempt(AttemptArgs),
    Smoke(SmokeArgs),
    Replay(ReplayArgs),
    Verify(VerifyArgs),
}
```

`AttemptArgs` is the existing `Args` minus the deleted flags. `SmokeArgs` writes a synthetic attempt artifact tree. `ReplayArgs` reads an attempt-summary and re-emits the reality-signal-packet (for testing the MCP analyze tools without re-running Gemma). `VerifyArgs` runs full-state verification against an existing attempt.

Update `main()` to dispatch.

Add a tiny helper at the top of `attempt` that updates `~/.cache/cgreality/active_runtime_root` and `~/.cache/cgreality/active_target_instance` when a run starts.

Pre: stage 1 complete.
Post: `./target/debug/reality-loop attempt --task-manifest=… --runtime-root=… --official-eval-each-attempt` works as the old default invocation. `reality-loop smoke` runs and writes the synthetic tree.

Current completion record: `/tmp/phase-02-verify.log` ends with `Phase 2 verification: ALL PASS`; `/tmp/cgreality-phase02-ready` is the successful handoff smoke runtime root; `~/.cache/cgreality/active_runtime_root` reads back `/tmp/cgreality-phase02-ready`.

Verification:
```bash
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-smoke --label runbook
find /tmp/cgreality-smoke -name attempt-summary.json -type f | head -1
# expected: at least one smoke-attempt-1/ directory with the standard artifacts
```

## Stage 3 — MCP `reality_loop` module

Current ordering correction: Phase 5 config files must exist before this MCP stage because path policy now fails closed when `CGREALITY_GOVERNED_PATHS` is missing. Execute Stage 4 / [phase-05-config-files/](phase-05-config-files/) before starting this section.

Create `crates/context-graph-mcp/src/handlers/tools/reality_loop/`:

- `mod.rs` — module declaration; tool registry.
- `analyze.rs` — every analyze tool from [03_MCP_REALITY_TOOLS_SPEC](03_MCP_REALITY_TOOLS_SPEC.md) family A.
- `interact.rs` — every interact tool, family B. Spawns the engine binary via `std::process::Command`.
- `alter.rs` — every alter tool, family C. Reuses the lifted `governed_edit` module from stage 1 for `harness_open_window` / `harness_apply_line_window_edit` / `harness_run_command` / `harness_verify_state`.
- `shift_log.rs` — `reality_shift_log` and `reality_shift_compare_to_my_view`. Append-only JSONL writer with fsync.
- `optimizer.rs` — `optimizer_record_decision` and `optimizer_record_recommendation`. Validates against the JSON Schema from `config/ccreality_recommendation_schema.json`.
- `path_policy.rs` — loads `config/ccreality_governed_paths.json`; offers `is_allowed(path) -> bool`.
- `errors.rs` — `CCRealityError` mirroring the existing `RealityError` shape.

Add a `--mode reality-loop` flag to the existing `context-graph-mcp` binary that registers these tools alongside the existing memory/graph/causal tools.

Logging: route `tracing` to stderr only.

Pre: stage 2 complete and the config files from Stage 4 / Phase 5 are present. Current ordering correction: the config-file stage was executed before this MCP stage so the path-policy loader could fail closed without fallback files.
Post: `cargo build -p context-graph-mcp` succeeds. Manual MCP-protocol smoke (using a simple client) lists every tool name in the spec.

Current completion record: `/tmp/phase-03-verify.log` ends with `Phase 3 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` contains `phase-03: PASS`. Phase 4 later completed the INTERACT/ALTER/SHIFT modules in this same `reality_loop` module tree.

Verification:
```bash
cargo build -p context-graph-mcp
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop | head -c 4000
# expected: JSON-RPC reply with tools array including mcp-spec tool names
```

## Stage 4 — Config + governed-paths file

Write:
- `config/ccreality_governed_paths.json` with the allow/deny lists from [03_MCP_REALITY_TOOLS_SPEC](03_MCP_REALITY_TOOLS_SPEC.md) "Path policy file".
- `config/ccreality_recommendation_schema.json` with the schema from [13_STRUCTURED_OUTPUT_SCHEMA](13_STRUCTURED_OUTPUT_SCHEMA.md).

Pre: stage 1 complete. This was executed before Phase 3/4 consumption so path policy and schema loaders could fail closed without fallback files.
Post: both files committed. `harness_open_window` against an allowed path succeeds; against `.env` returns `CCREALITY_HARNESS_PATH_DENIED`. Phase 5 FSV at `/tmp/ccreality-phase5-fsv-1778001835.log` additionally verifies strict policy shape, the documented `tests/oracle_**` deny rule, unsupported-version rejection, schema maxItems rejection, real recommendation artifact write, shift-log append, and BlackBox ledger append.

## Stage 5 — Hooks + `.claude/` scaffolding

Current state: **in progress but unverified**. This runbook stage maps to Phases 6, 7, and 8 in [21_IMPLEMENTATION_PLAN.md](21_IMPLEMENTATION_PLAN.md). Local WIP exists for `scripts/cc/_common.sh`, all six `scripts/cc/hook_*.sh` files, `_common_helpers.sh`, and four hook tests. The files are not executable, `test_pre_compact.sh` and `test_stop.sh` are absent, and Phase 6 verification has not passed. Do not wire `.claude/settings.json` until the hook scripts pass their real-runtime unit tests.

Drop in:
- `scripts/cc/_common.sh`, `hook_session_start.sh`, `hook_user_prompt_submit.sh`, `hook_pre_tool_use_edit.sh`, `hook_post_tool_use_edit.sh`, `hook_pre_compact.sh`, `hook_stop.sh` — bodies from [07_HOOKS_SPEC](07_HOOKS_SPEC.md). All `chmod +x`.
- `.claude/settings.json` — replaces the empty file with the contents from [06_SETTINGS_JSON_SPEC](06_SETTINGS_JSON_SPEC.md).
- `.mcp.json` — new file with the `cgreality` server entry.
- `.claude/loop.md` — body from [11_LOOP_PROMPT](11_LOOP_PROMPT.md).
- `.claude/agents/cc-reality-{diagnostician,harness-editor,reward-analyst,source-of-truth-readback,swe-failure-triage}.md` — bodies from [08_SUBAGENTS_SPEC](08_SUBAGENTS_SPEC.md).
- `.claude/skills/ccreality-{iterate,smoke,trigger-check,readback}/SKILL.md` — bodies from [09_SKILLS_SPEC](09_SKILLS_SPEC.md).

Update `CLAUDE.md` per [10_CLAUDE_MD_PROJECT_MEMORY](10_CLAUDE_MD_PROJECT_MEMORY.md) — append the addendum block.

Pre: stages 0–4 done.
Post: `claude` started in the project sees the cgreality MCP server (run `claude` then `/mcp` then verify `cgreality` appears as connected).

Verification:
```bash
claude -p "List the MCP servers and their tools. Return JSON." --output-format json | jq -r '.result' | grep cgreality
# expected: cgreality listed with tool count > 0
```

## Stage 6 — Smoke

Run the full smoke from [17_SMOKE_TEST_PLAN](17_SMOKE_TEST_PLAN.md):
```bash
bash scripts/cc/tests/smoke.sh
```

All eight layers pass.

## Stage 7 — FSV

Run [18_FSV_PLAN](18_FSV_PLAN.md):
```bash
bash scripts/cc/tests/fsv.sh
```

All A–E cases pass; F within tolerance.

Generate `docs/ccreality/FSV_RESULTS_<date>.md`.

## Stage 8 — Tear down old `docs/reality/2*` files

```bash
git rm docs/reality/20_openai_outer_optimizer.md
git rm docs/reality/21_grand_scale_reality_autoresearch.md   # superseded; replaced by ccreality docs
# (optionally KEEP 21 with a note pointing to ccreality)
```

Update `docs/reality/00_index.md` to remove the deleted entries and add a one-line pointer to `docs/ccreality/`.

`docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`: prepend a header `> Superseded by docs/ccreality/. Kept for historical context.` and otherwise leave untouched.

`docs2/DEEPSEEK_API_GUIDE.md`: delete (`git rm`).

The `docs2/.raw/*.md` files referenced in `git status` as deleted are already removed in working tree; just `git add -u` to commit the removal.

## Stage 9 — First real attempt

Pick one SWE-bench Lite instance to target. Update or create a task manifest at `tasks/swebench-lite/<instance-id>.json`.

Run:
```bash
./target/release/context-graph-cli reality-loop attempt \
  --task-manifest tasks/swebench-lite/<instance-id>.json \
  --runtime-root /tmp/ccreality-prod-1 \
  --max-attempts 1 \
  --official-eval-each-attempt
```

Confirm `attempt-1/` is fully populated. Confirm `~/.cache/cgreality/active_runtime_root` updated.

Then start an interactive optimizer session:
```bash
claude --model opus --permission-mode acceptEdits
> /ccreality-trigger-check
> /ccreality-iterate
```

Observe the full loop: SessionStart hook injects reality, the diagnostician runs, the harness-editor edits if warranted, the readback confirms, the recommendation is recorded, the Stop hook allows end.

## Stage 10 — Commit + PR

```bash
git status
git add -A
git commit -m "feat(ccreality): replace DeepSeek outer optimizer with Claude Code MCP integration

- Excise DeepSeek/OpenAI from reality-loop.rs (~3,400 lines removed)
- Reframe reality-loop.rs as pure reality engine with attempt|smoke|replay|verify subcommands
- Add cgreality MCP module exposing analyze/interact/alter tool families
- Add reality-shift principle: shift log + drift detection in every alter call
- Add .claude scaffolding: settings, hooks, agents, skills, loop prompt
- Append ccreality role to CLAUDE.md
- Add docs/ccreality/ planning package (21 documents)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

Open a PR for review (no force-push to main; PR review is the human checkpoint).

## Rollback plan

If anything goes wrong post-merge:
```bash
git checkout main
git revert <merge-sha>
# OR
git checkout ccreality-cutover-base.sha -- crates/ docs/ .claude/
```

`reality-loop.rs` pre-cutover state is preserved in git history. The `ccreality-cutover-base.sha` captured in stage 0 is the rollback target.

## Post-cutover hygiene

- Run `bash scripts/cc/tests/smoke.sh` daily for the first week as a CI gate.
- Run FSV weekly for the first month.
- Update `docs/ccreality/FSV_RESULTS_<date>.md` for each FSV run.
- After 100 production turns, review `cost_estimate_usd` in recommendations and decide whether to switch the writer's default model (Opus → Sonnet for routine turns).
- After 30 days, audit the shift log for any patterns that suggest principle violations (e.g., consecutive turns recording status=changed without intervening drift checks).
