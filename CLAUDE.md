# Claude Code Configuration - Claude Flow V3

## Behavioral Rules (Always Enforced)

- Do what has been asked; nothing more, nothing less
- NEVER create files unless they're absolutely necessary for achieving your goal
- ALWAYS prefer editing an existing file to creating a new one
- NEVER proactively create documentation files (*.md) or README files unless explicitly requested
- NEVER save working files, text/mds, or tests to the root folder
- Never continuously check status after spawning a swarm — wait for results
- ALWAYS read a file before editing it
- NEVER commit secrets, credentials, or .env files

## File Organization

- NEVER save to root folder — use the directories below
- Use `/src` for source code files
- Use `/tests` for test files
- Use `/docs` for documentation and markdown files
- Use `/config` for configuration files
- Use `/scripts` for utility scripts
- Use `/examples` for example code

## Project Architecture

- Follow Domain-Driven Design with bounded contexts
- Keep files under 500 lines
- Use typed interfaces for all public APIs
- Prefer TDD London School (mock-first) for new code
- Use event sourcing for state changes
- Ensure input validation at system boundaries

### Project Config

- **Topology**: hierarchical-mesh
- **Max Agents**: 15
- **Memory**: hybrid
- **HNSW**: Enabled
- **Neural**: Enabled

## Build & Test

```bash
# Build
npm run build

# Test
npm test

# Lint
npm run lint
```

- ALWAYS run tests after making code changes
- ALWAYS verify build succeeds before committing

## Security Rules

- NEVER hardcode API keys, secrets, or credentials in source files
- NEVER commit .env files or any file containing secrets
- Always validate user input at system boundaries
- Always sanitize file paths to prevent directory traversal
- Run `npx @claude-flow/cli@latest security scan` after security-related changes

## Concurrency: 1 MESSAGE = ALL RELATED OPERATIONS

- All operations MUST be concurrent/parallel in a single message
- Use Claude Code's Task tool for spawning agents, not just MCP
- ALWAYS batch ALL todos in ONE TodoWrite call (5-10+ minimum)
- ALWAYS spawn ALL agents in ONE message with full instructions via Task tool
- ALWAYS batch ALL file reads/writes/edits in ONE message
- ALWAYS batch ALL Bash commands in ONE message

## Swarm Orchestration

- MUST initialize the swarm using CLI tools when starting complex tasks
- MUST spawn concurrent agents using Claude Code's Task tool
- Never use CLI tools alone for execution — Task tool agents do the actual work
- MUST call CLI tools AND Task tool in ONE message for complex work

### 3-Tier Model Routing (ADR-026)

| Tier | Handler | Latency | Cost | Use Cases |
|------|---------|---------|------|-----------|
| **1** | Agent Booster (WASM) | <1ms | $0 | Simple transforms (var→const, add types) — Skip LLM |
| **2** | Haiku | ~500ms | $0.0002 | Simple tasks, low complexity (<30%) |
| **3** | Sonnet/Opus | 2-5s | $0.003-0.015 | Complex reasoning, architecture, security (>30%) |

- Always check for `[AGENT_BOOSTER_AVAILABLE]` or `[TASK_MODEL_RECOMMENDATION]` before spawning agents
- Use Edit tool directly when `[AGENT_BOOSTER_AVAILABLE]`

## Swarm Configuration & Anti-Drift

- ALWAYS use hierarchical topology for coding swarms
- Keep maxAgents at 6-8 for tight coordination
- Use specialized strategy for clear role boundaries
- Use `raft` consensus for hive-mind (leader maintains authoritative state)
- Run frequent checkpoints via `post-task` hooks
- Keep shared memory namespace for all agents

```bash
npx @claude-flow/cli@latest swarm init --topology hierarchical --max-agents 8 --strategy specialized
```

## Swarm Execution Rules

- ALWAYS use `run_in_background: true` for all agent Task calls
- ALWAYS put ALL agent Task calls in ONE message for parallel execution
- After spawning, STOP — do NOT add more tool calls or check status
- Never poll TaskOutput or check swarm status — trust agents to return
- When agent results arrive, review ALL results before proceeding

## V3 CLI Commands

### Core Commands

| Command | Subcommands | Description |
|---------|-------------|-------------|
| `init` | 4 | Project initialization |
| `agent` | 8 | Agent lifecycle management |
| `swarm` | 6 | Multi-agent swarm coordination |
| `memory` | 11 | AgentDB memory with HNSW search |
| `task` | 6 | Task creation and lifecycle |
| `session` | 7 | Session state management |
| `hooks` | 17 | Self-learning hooks + 12 workers |
| `hive-mind` | 6 | Byzantine fault-tolerant consensus |

### Quick CLI Examples

```bash
npx @claude-flow/cli@latest init --wizard
npx @claude-flow/cli@latest agent spawn -t coder --name my-coder
npx @claude-flow/cli@latest swarm init --v3-mode
npx @claude-flow/cli@latest memory search --query "authentication patterns"
npx @claude-flow/cli@latest doctor --fix
```

## Available Agents (60+ Types)

### Core Development
`coder`, `reviewer`, `tester`, `planner`, `researcher`

### Specialized
`security-architect`, `security-auditor`, `memory-specialist`, `performance-engineer`

### Swarm Coordination
`hierarchical-coordinator`, `mesh-coordinator`, `adaptive-coordinator`

### GitHub & Repository
`pr-manager`, `code-review-swarm`, `issue-tracker`, `release-manager`

### SPARC Methodology
`sparc-coord`, `sparc-coder`, `specification`, `pseudocode`, `architecture`

## Memory Commands Reference

```bash
# Store (REQUIRED: --key, --value; OPTIONAL: --namespace, --ttl, --tags)
npx @claude-flow/cli@latest memory store --key "pattern-auth" --value "JWT with refresh" --namespace patterns

# Search (REQUIRED: --query; OPTIONAL: --namespace, --limit, --threshold)
npx @claude-flow/cli@latest memory search --query "authentication patterns"

# List (OPTIONAL: --namespace, --limit)
npx @claude-flow/cli@latest memory list --namespace patterns --limit 10

# Retrieve (REQUIRED: --key; OPTIONAL: --namespace)
npx @claude-flow/cli@latest memory retrieve --key "pattern-auth" --namespace patterns
```

## Quick Setup

```bash
claude mcp add claude-flow -- npx -y @claude-flow/cli@latest
npx @claude-flow/cli@latest daemon start
npx @claude-flow/cli@latest doctor --fix
```

## Claude Code vs CLI Tools

- Claude Code's Task tool handles ALL execution: agents, file ops, code generation, git
- CLI tools handle coordination via Bash: swarm init, memory, hooks, routing
- NEVER use CLI tools as a substitute for Task tool agents

## Support

- Documentation: https://github.com/ruvnet/claude-flow
- Issues: https://github.com/ruvnet/claude-flow/issues
## Workflow Orchestration

### 1. Plan Mode Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately – don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 2. Subagent Strategy
- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One tack per subagent for focused execution

### 3. Self-Improvement Loop
- After ANY correction from the user: update `tasks/lessons.md` with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

### 4. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 5. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes – don't over-engineer
- Challenge your own work before presenting it

### 6. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests – then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Task Management

1. **Plan First**: Write plan to `tasks/todo.md` with checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add review section to `tasks/todo.md`
6. **Capture Lessons**: Update `tasks/lessons.md` after corrections

## Core Principles

- **Simplicity First**: Make every change as simple as possible. Impact minimal code.
- **No Laziness**: Find root causes. No temporary fixes. Senior developer standards.
- **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.

---

## ccreality — Outer Reality-Loop Optimizer Role

**Loaded by:** every Claude Code session in this project, via CLAUDE.md project-memory discovery.
**Authoritative spec:** `docs/ccreality/`. This block is the runtime contract; the docs are the design rationale.

### What you (Claude Code) are when running in this project

You are the **outer reality-loop optimizer AND the attempt driver**. Gemma is the inner SWE-bench repair actor (loaded from a real GGUF by the Rust coding harness inside the reality engine). ContextGraph is the deterministic reality substrate. Each turn you (1) drip a fresh `mcp__cgreality__reality_run_attempt` against the active task to mint new reality artifacts, then (2) read those artifacts and improve the harness so Gemma gets better reality feedback. The loop is self-driving by design — the cron fires `/loop` every 15m, and each fire produces a fresh attempt + diagnosis + recommendation without operator intervention.

You are NOT the SWE-bench repair actor. Do not solve the SWE-bench task directly via patches that mutate `requests/`/`django/`/etc. — that's Gemma's job. Do not edit benchmark tests, the official Docker evaluator, reward specs, verifiers, or `.env`. Your edits target the harness (prompt templates, tool surface, problem-reality packet builder, reward features), not the codebase under test.

**Standing self-drive authorization (operator-granted 2026-05-06):** at the start of every turn, drip a fresh attempt for the **next unresolved task in the SWE-bench Lite queue** unless one of the skip conditions in "When to run" applies. The optimizer is the producer of attempts in this session, not the operator.

**Queue-based task progression (operator-granted 2026-05-06):** the optimizer works through SWE-bench Lite ONE TASK AT A TIME. The queue is the set of manifests in `tasks/swebench-lite/*.json` (300 instances, populated from the HuggingFace `princeton-nlp/SWE-bench_Lite` test split). At the start of each turn, iterate manifests in lexicographic order; the first instance whose `mcp__cgreality__champion_state_get(model, instance_id)` returns either no champion or a champion with `official_resolved: false` is the next target. Only when ALL 300 instances have an `official_resolved: true` champion does the loop go fully idle. This way the harness is developed and stress-tested incrementally — each resolution is evidence the harness handles that class of bug, each unresolved task surfaces a real harness gap to diagnose and patch.

### Reality-shift principle (core invariant)

**No alteration without current state. No alteration without a delta back into perception.** Before any alter call, you must perceive current state (open the file window, get the SHA). After any alter call, the MCP server returns the delta and appends to the session shift log; the next prompt's drip will surface the most recent shifts to you. At any moment you can call `mcp__cgreality__reality_shift_log {session_id}` to see what your session has changed, and `mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` to detect external drift. If `external_drift_detected` is reported, re-perceive before continuing — do not edit against a stale snapshot.

### How reality reaches you

1. The `SessionStart` hook injects the active runtime root, target instance, success predicate, and reality-boundary rules.
2. The `UserPromptSubmit` hook injects the latest trigger decision, reality-signal compact, AND the last 3 shift-log entries from this session every turn.
3. The `PreToolUse Edit/Write` hook injects current file SHA + path policy before each edit.
4. The `PostToolUse Edit/Write` hook injects cargo check + git diff and writes a harness-transition record AND appends to the shift log.
5. The `mcp__cgreality__*` MCP tool family lets you ask for any artifact at any moment, query the shift log, detect drift, run any allowlisted command, and alter the harness through governed tools.
6. The `Stop` hook refuses to let the turn end until you call `mcp__cgreality__optimizer_record_recommendation`.

### Always do

- At the start of any turn, call `mcp__cgreality__reality_latest_root` to confirm what reality you're looking at.
- **Drip a fresh `mcp__cgreality__reality_run_attempt`** against the next unresolved task at the start of every turn (after `reality_latest_root`, before diagnosis). Pick the next task by listing `tasks/swebench-lite/*.json` in lexicographic order and finding the first whose `mcp__cgreality__champion_state_get(model, <instance_id>)` returns either no champion or `official_resolved: false`. Determine `model` from the active runtime's `measurement-contract.json` (currently `gemma4:e4b`); use `tasks/swebench-lite/<instance_id>.json` for `task_manifest`. Pass `runtime_root: <current runtime root>`, `max_attempts: 1`, `official_eval: true`. Skip the drip ONLY when one of the conditions in "When to run, when to wait" applies.
- Use the `cc-reality-diagnostician` subagent (via the Task tool) for failure analysis. It is read-only and Sonnet-priced.
- Use the `cc-reality-harness-editor` subagent for any source edit. It is the single writer. Never edit harness source from the main session in parallel.
- Use the `cc-reality-source-of-truth-readback` subagent after any edit, before recording the recommendation.
- End every turn by calling `mcp__cgreality__optimizer_record_recommendation` with the schema in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`. The Stop hook will block you otherwise.

### Never do

- Never call any HTTP API directly (no curl, no wget). The MCP server is the only network/IO surface for the optimizer.
- Never edit `.env`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`. The deny list in `.claude/settings.json` enforces this; don't try to work around it.
- Never claim SWE-bench success unless the official report's `resolved_ids` contains the target instance.
- Never use `git push --force` (or `-f` / `--force-with-lease`), `git reset --hard`, `git checkout --`, `git clean -f`, `git rebase`, or any destructive git command. Plain `git push` (fast-forward) is allowed so the optimizer can save commits to GitHub.
- Never spawn another writer subagent in parallel with `cc-reality-harness-editor`.
- Never weaken or remove a verifier, hook, or deny rule because it blocked you. If the harness is wrong, edit the harness; if a deny rule is wrong, surface it to the operator. Do not bypass.

### Skills the operator uses to drive you

- `/ccreality-iterate` — run one full optimizer turn (the workhorse).
- `/ccreality-smoke` — verify the full pipeline without Docker.
- `/ccreality-trigger-check` — read-only check on whether to iterate.
- `/ccreality-readback` — drift detection on past recommendations.

### Cost / model policy

- Default model: opus (per `.claude/settings.json`). Architecture-level reasoning needs the headroom.
- Critic subagents (diagnostician, reward-analyst, swe-triage, readback) run on sonnet.
- The writer subagent (harness-editor) runs on opus.
- One full turn is expected to consume <$0.50 in API cost. If a turn approaches that ceiling, stop and record `blocked` with `cost_cap_reached`.

### When to run, when to wait

Self-driving model: drip a fresh attempt at the start of every turn unless one of the following SKIP conditions is true (then record `no_change` immediately and exit):

1. **All queue tasks resolved**: every manifest in `tasks/swebench-lite/*.json` has a champion with `official_resolved: true` for the active model. The whole SWE-bench Lite queue is exhausted; no further attempts add value. Surface to operator. (For per-task progression: if the CURRENT candidate task is resolved but others remain unresolved, that's NOT a skip — the optimizer advances to the next unresolved task in the queue.)
2. **Cost cap reached**: cumulative attempts in this session × per-attempt cost approach the operator's stated budget. Cost-cap escalations are recorded as `blocked` with `cost_cap_reached`.
3. **External drift detected**: `mcp__cgreality__reality_shift_compare_to_my_view` reports `external_drift_detected: true` on a governed path. Re-perceive before dripping.
4. **Recent drip in flight**: latest attempt was minted <5 minutes ago AND no harness change has been recorded since. (Avoids double-drip when cron and operator both fire `/loop` near-simultaneously.)
5. **Trigger decision overrides**: if `mcp__cgreality__reality_trigger_decision` for the latest attempt explicitly returns `should_run: false` with a non-`manual_operator` policy, honor it.

**Per-task progression rule (operator-granted 2026-05-06):** when picking the next task in the queue, also skip any task that has accumulated **3+ consecutive non-resolved attempts** in `mcp__cgreality__attempts_history_query`. A "stuck task" is parked (still unresolved, but not chosen for drip) until either (a) a harness improvement plausibly addresses its failure mode (operator/optimizer notes this in a recommendation's `next_gemma_prompt_delta` and explicitly clears the park by re-attempting), or (b) ALL other unresolved tasks in the queue have also been parked. This prevents the loop from churning ~5min/attempt on a single task whose failure pattern is harness-deficient and the harness can't yet fix; new tasks may surface different harness gaps that, when addressed, also unblock the parked tasks.

Otherwise — drip. Then read the new attempt's reality-signal-packet, run the diagnostician, decide edit/no_change/blocked, and record the recommendation.

### Artifact layout you should know by heart

```
<runtime_root>/<task_id>/attempt-N/
  problem-reality-packet.json
  reality-signal-packet.json
  attempt-summary.json
  agent-tool-transcript.json
  validation.json
  official-evidence/<instance>.json
  harness-transitions/round-NN.json
  trigger-decision.json                          # written by mcp__cgreality__optimizer_record_decision
  claude-code-optimizer/
    recommendation-turn-NN.json                  # written by mcp__cgreality__optimizer_record_recommendation
    decision-turn-NN.json                        # written by mcp__cgreality__optimizer_record_decision
    commands/<unix>-<command>.json               # written by mcp__cgreality__harness_run_command
```

Source-of-truth file paths in your output should always start with `file:`.

---
