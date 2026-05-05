# Master Implementation Plan

This is the execution plan for the ccreality cutover. Each phase has an **AI-verifiable end state** — a concrete shell command (or set of commands) whose successful execution is the definition of "phase complete." A phase is not done until its verification passes.

Each phase has its own subfolder under `docs/ccreality/phase-NN-shortname/` with the full task list, file specs, and verification an AI agent needs to execute the phase end-to-end without consulting other context.

## Phase summary

| # | Status | Phase | End-state command (must succeed) | Folder |
|---|--------|-------|----------------------------------|--------|
| 0 | Complete | Foundation | `git rev-parse ccreality-cutover && test -s /tmp/ccreality-cutover-base.sha` | [phase-00-foundation/](phase-00-foundation/) |
| 1 | Complete | Excision | `! rg -nq "deepseek\|openai\|gpt-5\|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs && cargo build -p context-graph-cli --bin reality-loop && cargo test -p context-graph-cli --bin reality-loop && cargo test -p context-graph-cli --lib governed_edit` | [phase-01-excision/](phase-01-excision/) |
| 2 | Complete | Engine subcommands | `./target/debug/reality-loop --help \| grep -E "attempt\|smoke\|replay\|verify"` | [phase-02-engine-subcommands/](phase-02-engine-subcommands/) |
| 3 | Complete | MCP foundation + analyze | MCP `tools/list` returns the 11 Phase 3 `reality_*` ANALYZE tools; analyze calls return payloads tied to `source_of_truth` readback | [phase-03-mcp-foundation-analyze/](phase-03-mcp-foundation-analyze/) |
| 4 | In progress | MCP interact + alter + shift | `harness_apply_line_window_edit` round-trip succeeds and writes shift_log entry; `reality_shift_compare_to_my_view` detects external drift | [phase-04-mcp-interact-alter-shift/](phase-04-mcp-interact-alter-shift/) |
| 5 | Complete | Config files | `jq -e . config/ccreality_governed_paths.json && jq -e . config/ccreality_recommendation_schema.json` | [phase-05-config-files/](phase-05-config-files/) |
| 6 | Pending | Hooks | All 6 `scripts/cc/hook_*.sh` scripts pass their bash unit tests under `scripts/cc/tests/hooks/` | [phase-06-hooks/](phase-06-hooks/) |
| 7 | Pending | Claude scaffolding | `.claude/settings.json`, `.mcp.json`, `.claude/loop.md`, 5 agent files, 4 skill `SKILL.md` files all exist with valid frontmatter | [phase-07-claude-scaffolding/](phase-07-claude-scaffolding/) |
| 8 | Pending | CLAUDE.md addendum | `rg -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md` | [phase-08-claude-md-addendum/](phase-08-claude-md-addendum/) |
| 9 | Pending | Smoke | `bash scripts/cc/tests/smoke.sh` exits 0 with all 8 layers PASS | [phase-09-smoke/](phase-09-smoke/) |
| 10 | Pending | FSV | `bash scripts/cc/tests/fsv.sh` exits 0; `docs/ccreality/FSV_RESULTS_*.md` created with all A–E PASS | [phase-10-fsv/](phase-10-fsv/) |
| 11 | Pending | Docs cleanup | `! test -f docs/reality/20_openai_outer_optimizer.md && grep -q ccreality docs/reality/00_index.md` | [phase-11-docs-cleanup/](phase-11-docs-cleanup/) |
| 12 | Pending | First production attempt | One real SWE-bench instance attempt produced + one valid recommendation artifact + commit on branch | [phase-12-first-production/](phase-12-first-production/) |

## Current State

As of 2026-05-05:

- Branch: `ccreality-cutover`.
- Phase 0 base SHA: `411443fdcbd8db4005cfe46d44f64f7698c2f11b`, recorded in `/tmp/ccreality-cutover-base.sha`.
- Phase 1 verification log: `/tmp/phase-01-verify.log`.
- Phase 1 progress log row: `/tmp/ccreality-phase-progress.log`.
- Phase 1 manual FSV source of truth: `/tmp/ccreality-phase1-fsv-output/observations.json`.
- Phase 2 verification log: `/tmp/phase-02-verify.log`.
- Phase 2 ready smoke source of truth: `/tmp/cgreality-phase02-ready`; active-root readback is `~/.cache/cgreality/active_runtime_root=/tmp/cgreality-phase02-ready` and `~/.cache/cgreality/active_target_instance=cgreality-smoke-phase02-ready`.
- Phase 5 verification log: `/tmp/phase-05-verify.log`.
- Phase 5 source of truth: `config/ccreality_governed_paths.json`, `config/ccreality_recommendation_schema.json`, and `docs/ccreality/phase-05-config-files/sample_recommendation.json`.
- Phase 3 verification log: `/tmp/phase-03-verify.log`.
- Phase 3 source of truth: bounded MCP JSON-RPC readback from `./target/debug/context-graph-mcp --mode reality-loop`, with 11 Phase 3 analyze tools listed and live calls reading active smoke artifacts.
- Current source state on `ccreality-cutover`: `[lib]` in `crates/context-graph-cli/Cargo.toml`, `pub mod governed_edit` in `crates/context-graph-cli/src/lib.rs`, and `crates/context-graph-cli/src/governed_edit.rs` are already present on the branch.
- Current verified Phase 2 source state: `crates/context-graph-cli/src/bin/reality-loop.rs` implements `attempt|smoke|replay|verify`; `crates/context-graph-cli/Cargo.toml` includes `dirs = "5"`.
- Current Phase 4 working-tree state: `crates/context-graph-mcp/src/handlers/tools/reality_loop/{alter,interact,shift_log,optimizer,lock,schema_validator}.rs` and `crates/context-graph-mcp/src/tools/definitions/reality.rs` exist. `cargo build -p context-graph-mcp --quiet` and `cargo test -p context-graph-mcp --lib reality_loop --quiet` pass. A bounded `tools/list` probe read back 173 total tools and 32 cgreality reality/alter/shift/memory tools. There is not yet a `phase-04: PASS` row in `/tmp/ccreality-phase-progress.log`.
- Current branch history after Phase 0 base: incremental commits through `0472c199`.
- Current `reality-loop.rs` readback: 7,990 lines, with no `deepseek|openai|gpt-5|chatgpt` matches.
- Next phase to execute: finish [phase-04-mcp-interact-alter-shift/](phase-04-mcp-interact-alter-shift/) by running its canonical verification, fixing any no-fallback/no-workaround violations surfaced by that verification, and appending `phase-04: PASS` only after the source-of-truth readbacks pass.

## Dependency DAG

```
Phase 0 (foundation)
  └── Phase 1 (excision)
        ├── Phase 2 (engine subcommands)
        └── Phase 5 (config files)
              └── Phase 3 (MCP foundation + analyze; also requires Phase 2)
                    └── Phase 4 (MCP interact + alter + shift)
                          ├── Phase 6 (hooks)
                          └── Phase 7 (claude scaffolding)
                                └── Phase 8 (CLAUDE.md addendum)
                                      └── Phase 9 (smoke)
                                            └── Phase 10 (FSV)
                                                  └── Phase 11 (docs cleanup)
                                                        └── Phase 12 (first production)
```

Phase 2 and Phase 5 can run independently after Phase 1. Phase 3 requires both: Phase 2 provides the engine subcommands/smoke runtime contract, and Phase 5 provides the governed-paths and recommendation schema files.

## Atomicity rule

Each phase MUST be completed atomically — either all its tasks pass and verification succeeds, or all edits from that phase are reverted. The branch now uses incremental phase commits rather than one final monolithic commit; Phase 12 records the final production verification/cutover commit. If a phase fails:

1. Inspect the failing verification.
2. If recoverable (cargo error, missing dependency), fix and re-verify.
3. If not recoverable, `git restore` all files modified during the phase, then re-plan.

## AI-verifiable end-state principle

A "verification" is AI-verifiable if it satisfies all three:
- **Deterministic** — running the same command twice from the same starting state produces the same exit code.
- **Self-contained** — the command does not require human inspection of output to decide pass/fail.
- **Side-effect free OR fully captured** — verification commands either don't mutate state, or they write their output to a known artifact path the next phase can re-read.

Example AI-verifiable check:
```bash
# Phase 1 end-state: no DeepSeek/OpenAI text remains in the engine
! rg -nq "deepseek|openai|gpt-5|chatgpt" \
    crates/context-graph-cli/src/bin/reality-loop.rs \
  && cargo build -p context-graph-cli --bin reality-loop --quiet \
  && cargo test -p context-graph-cli --bin reality-loop --quiet
echo "Phase 1: $? (0 = pass)"
```

Each phase's `04_VERIFICATION.md` (or equivalent) gives the exact commands.

## Per-phase folder structure

Standard contents of each phase folder:

```
phase-NN-shortname/
├── README.md          # phase overview + links to all docs in this folder
├── 00_SPEC.md         # objectives, scope, deliverables, AI-verifiable end-state
├── 01_PRECONDITIONS.md # what must be true before starting (omitted for trivial phases)
├── 02_TASKS.md        # ordered atomic tasks with explicit instructions
├── 03_FILE_SPECS.md   # detailed specs for every file the phase creates/modifies
├── 04_VERIFICATION.md # the AI-verifiable end-state checks
├── 05_RECOVERY.md     # what to do if a task or verification fails
└── 06_HANDOFF.md      # what's true after completion; pointer to next phase
```

Smaller phases omit `01_PRECONDITIONS.md`, `03_FILE_SPECS.md`, or `05_RECOVERY.md` when they have no preconditions beyond the previous phase, no new files, or no recoverable failure modes. `README.md`, `00_SPEC.md`, `02_TASKS.md`, `04_VERIFICATION.md`, `06_HANDOFF.md` are present in every phase.

## Time estimates

Wall-clock estimates (single AI agent, no human in the loop):

| Phase | Estimate | Rationale |
|-------|----------|-----------|
| 0 | 5 min | git ops only |
| 1 | 1–2 hours | ~3,400 lines deleted across many regions; cargo iteration |
| 2 | 30 min | clap refactor + dispatch |
| 3 | 2–4 hours | new MCP module skeleton + ~10 analyze tools |
| 4 | 3–5 hours | ~10 alter/interact tools with shift-log discipline |
| 5 | 30 min | two JSON files |
| 6 | 1–2 hours | 6 bash hook scripts + their unit tests |
| 7 | 1–2 hours | 11+ files with frontmatter |
| 8 | 5 min | one append to CLAUDE.md |
| 9 | 1 hour | smoke script + iteration to make all 8 layers pass |
| 10 | 2–4 hours | FSV per-case scripts |
| 11 | 15 min | git rm + index update |
| 12 | 1 hour | one real attempt + one optimizer turn + commit |

**Total: 14–22 hours** of AI execution time across the full cutover. Realistically 2–3 working days with verification gating between phases.

## Execution-order constraint with `simplify` skill

After each phase's verification passes, the AI agent should invoke the `simplify` skill on any code/docs changed during the phase before proceeding to the next phase. This is a project rule (`/simplify` is enabled) and catches duplication early.

## Where to find what

- **Architecture rationale**: `01_ARCHITECTURE.md`, `01b_REALITY_SHIFT_PRINCIPLE.md`, `02_REALITY_DRIP_PROTOCOL.md`
- **What to build**: `03_MCP_REALITY_TOOLS_SPEC.md`, `06_SETTINGS_JSON_SPEC.md`, `07_HOOKS_SPEC.md`, `08_SUBAGENTS_SPEC.md`, `09_SKILLS_SPEC.md`, `10_CLAUDE_MD_PROJECT_MEMORY.md`, `11_LOOP_PROMPT.md`, `13_STRUCTURED_OUTPUT_SCHEMA.md`
- **What to delete**: `04_EXCISION_PLAN.md`
- **How to execute**: this file + each `phase-NN-shortname/` folder
- **How to verify**: each phase's `04_VERIFICATION.md`
- **What's still undecided**: `20_OPEN_QUESTIONS.md`

## Reading order for an AI agent picking this up cold

1. `00_INDEX.md`
2. `01_ARCHITECTURE.md`
3. `01b_REALITY_SHIFT_PRINCIPLE.md`
4. `21_IMPLEMENTATION_PLAN.md` (this file)
5. `phase-00-foundation/README.md`
6. Then work through phases 0 → 12, consulting the spec docs (00–20) as referenced in each phase's tasks.
