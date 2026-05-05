# Master Implementation Plan

This is the execution plan for the ccreality cutover. Each phase has an **AI-verifiable end state** — a concrete shell command (or set of commands) whose successful execution is the definition of "phase complete." A phase is not done until its verification passes.

Each phase has its own subfolder under `docs/ccreality/phase-NN-shortname/` with the full task list, file specs, and verification an AI agent needs to execute the phase end-to-end without consulting other context.

## Phase summary

| # | Phase | End-state command (must succeed) | Folder |
|---|-------|----------------------------------|--------|
| 0 | Foundation | `git rev-parse ccreality-cutover && test -s /tmp/ccreality-cutover-base.sha` | [phase-00-foundation/](phase-00-foundation/) |
| 1 | Excision | `! rg -nq "deepseek\|openai\|gpt-5\|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs && cargo build -p context-graph-cli --bin reality-loop && cargo test -p context-graph-cli --bin reality-loop` | [phase-01-excision/](phase-01-excision/) |
| 2 | Engine subcommands | `./target/debug/context-graph-cli reality-loop --help \| grep -E "attempt\|smoke\|replay\|verify"` | [phase-02-engine-subcommands/](phase-02-engine-subcommands/) |
| 3 | MCP foundation + analyze | MCP `tools/list` returns ≥10 `mcp__cgreality__reality_*` tools; each analyze tool returns a payload with `source_of_truth` field | [phase-03-mcp-foundation-analyze/](phase-03-mcp-foundation-analyze/) |
| 4 | MCP interact + alter + shift | `harness_apply_line_window_edit` round-trip succeeds and writes shift_log entry; `reality_shift_compare_to_my_view` detects external drift | [phase-04-mcp-interact-alter-shift/](phase-04-mcp-interact-alter-shift/) |
| 5 | Config files | `jq -e . config/ccreality_governed_paths.json && jq -e . config/ccreality_recommendation_schema.json` | [phase-05-config-files/](phase-05-config-files/) |
| 6 | Hooks | All 6 `scripts/cc/hook_*.sh` scripts pass their bash unit tests under `scripts/cc/tests/hooks/` | [phase-06-hooks/](phase-06-hooks/) |
| 7 | Claude scaffolding | `.claude/settings.json`, `.mcp.json`, `.claude/loop.md`, 5 agent files, 4 skill `SKILL.md` files all exist with valid frontmatter | [phase-07-claude-scaffolding/](phase-07-claude-scaffolding/) |
| 8 | CLAUDE.md addendum | `rg -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md` | [phase-08-claude-md-addendum/](phase-08-claude-md-addendum/) |
| 9 | Smoke | `bash scripts/cc/tests/smoke.sh` exits 0 with all 8 layers PASS | [phase-09-smoke/](phase-09-smoke/) |
| 10 | FSV | `bash scripts/cc/tests/fsv.sh` exits 0; `docs/ccreality/FSV_RESULTS_*.md` created with all A–E PASS | [phase-10-fsv/](phase-10-fsv/) |
| 11 | Docs cleanup | `! test -f docs/reality/20_openai_outer_optimizer.md && grep -q ccreality docs/reality/00_index.md` | [phase-11-docs-cleanup/](phase-11-docs-cleanup/) |
| 12 | First production attempt | One real SWE-bench instance attempt produced + one valid recommendation artifact + commit on branch | [phase-12-first-production/](phase-12-first-production/) |

## Dependency DAG

```
Phase 0 (foundation)
  └── Phase 1 (excision)
        ├── Phase 2 (engine subcommands)
        │     └── Phase 5 (config files) ─┐
        └── Phase 5 (config files) ───────┤
              └── Phase 3 (MCP foundation + analyze)
                    └── Phase 4 (MCP interact + alter + shift)
                          ├── Phase 6 (hooks)
                          └── Phase 7 (claude scaffolding)
                                └── Phase 8 (CLAUDE.md addendum)
                                      └── Phase 9 (smoke)
                                            └── Phase 10 (FSV)
                                                  └── Phase 11 (docs cleanup)
                                                        └── Phase 12 (first production)
```

Phase 5 has two upstream dependents (2 and 3) because both refer to the schema file. It can be executed any time after Phase 1.

## Atomicity rule

Each phase MUST be completed atomically — either all its tasks pass and verification succeeds, or all are reverted. Partial completion of a phase is forbidden. If a phase fails:

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
