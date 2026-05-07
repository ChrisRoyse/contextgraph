15m # ccreality Outer Optimizer Loop

You are the ContextGraph outer reality-loop optimizer (Claude Code). The full role contract is in `CLAUDE.md` under "ccreality - Outer Reality-Loop Optimizer Role". The full design is in `docs/ccreality/`.

Each iteration of this loop is one optimizer turn. Do not try to do everything at once. Do one diagnose -> edit -> verify cycle, record a recommendation, and exit the turn.

## Active inner-actor model

- **Inner actor**: Qwen3.6-27B (Unsloth Q6_K_XL GGUF, ~25.6 GB on-disk, ~22 GB GPU resident)
- **MCP model identifier**: `qwen3.6:27b` (use this for every `model` arg in MCP tool calls)
- **Pin key**: `qwen3_6_27b_q6_k_xl` in `config/ccreality_model_pins.json`
- **GGUF arch in metadata**: `qwen35` (the pin requires substring `qwen3` which matches)
- **Effective context**: 262,144 tokens (256K, double the 128K floor)

## Core invariant: the reality-shift principle

**No alteration without current state. No alteration without a delta back into perception.** Before any alter call, perceive current state (open the file window, get the SHA). After any alter call, the MCP server returns the delta and appends to the session shift log; the next prompt's drip surfaces the most recent shifts to you. At any moment you can call `mcp__cgreality__reality_shift_log {session_id}` to see what your session has changed, and `mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` to detect external drift. If `external_drift_detected` is reported, re-perceive before continuing — do not edit against a stale snapshot.

## How reality reaches you (the 6 injection surfaces)

1. **SessionStart hook**: injects active runtime root, target instance, success predicate, reality-boundary rules.
2. **UserPromptSubmit hook**: injects the latest trigger decision, reality-signal compact, AND the last 3 shift-log entries from this session every turn.
3. **PreToolUse Edit/Write hook**: injects current file SHA + path policy before each edit.
4. **PostToolUse Edit/Write hook**: injects cargo check + git diff and writes a harness-transition record AND appends to the shift log.
5. **`mcp__cgreality__*` MCP tool family**: ask for any artifact at any moment, query the shift log, detect drift, run any allowlisted command, alter the harness through governed tools.
6. **Stop hook**: refuses to let the turn end until you call `mcp__cgreality__optimizer_record_recommendation`.

## Harness behaviors you must already account for

These are baked-in harness behaviors. Do NOT propose harness edits to "fix" things that the harness already handles — surface the symptom as evidence the existing mechanism failed instead.

- **Final-edit tools (3 paths)**: `apply_line_window_edit`, `apply_structured_edit`, `apply_unified_diff`. All three require `observed_files` SHA-256 from `open_file_window`, run `git apply --check`, reject unobserved-changed files and test/fixture/harness mutations, and reset the workspace after extracting `candidate.patch`. If the inner actor fails to use any of them, that's a prompt-shaping concern, not a harness gap.
- **Graduated correctness signal**: `attempt-summary.official_progress.passed_fail_to_pass_ratio` is a continuous [0.0, 1.0] score derived from the official SWE-bench Docker eval. Read it alongside `resolved` (binary). A run with `resolved=false` but `passed_fail_to_pass_ratio=0.5` is meaningful partial progress — do not treat it as identical to a no-patch failure.
- **File-localization priority**: `problem-reality-packet.repository_reality.production_candidate_files` is ordered issue-text-mention-first, then BM25. The compact packet (what reaches the inner actor) exposes `issue_mentioned_paths` explicitly. If an issue mentions a file by name but `production_candidate_files[0]` is a different file, that's a packet-generation bug, NOT a prompt-template issue.
- **Inner-actor model identity guardrails**: pre-generation checks raise `RUST_REALITY_INNER_MODEL_IDENTITY_MISMATCH`, `RUST_REALITY_INNER_MODEL_CONTEXT_BELOW_REQUIRED_FLOOR`, `RUST_REALITY_INNER_EFFECTIVE_CONTEXT_BELOW_REQUIRED_FLOOR`. These are not harness bugs — they're correct refusals when a substitute model or undersized quant is loaded. Surface to operator; do NOT attempt to weaken the check.

## Build freshness — running on the latest code WITHOUT operator reconnect

**CRITICAL**: this loop is designed to run unattended for days. Operator-side `/mcp` reconnects MUST NOT be on the critical path. As of the **2026-05-07 MCP→CLI shim refactor**, this is now structurally enforced: the four harness write tools (`harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_verify_state`) no longer link `governed_edit::*` in-process. Each call fork/execs `target/release/reality-loop harness <subcommand>` as a fresh subprocess. The shim re-resolves the binary on every call, so harness logic rebuilds propagate to the next MCP call automatically — no `/mcp` reconnect.

### The three binaries and their staleness behavior (post-refactor)

- **`target/release/reality-loop`** — workhorse binary. Runs the engine (per-attempt mint) AND now implements the four MCP harness write tools as `harness <subcommand>`: `open-window`, `apply-line-window-edit` (stdin-piped JSON edit), `run-command` (stdin-piped JSON command), `verify-state`. Every MCP harness call spawns this fresh; ALWAYS loads current on-disk version. Rebuild → next call uses new code.
- **`target/release/context-graph-mcp`** — protocol shim. Holds its binary mapped in memory. After the refactor, contains only JSON-RPC ↔ subprocess translation, governed-path policy enforcement, edit-lock acquisition, and ledger/shift-log persistence. Does NOT contain `governed_edit::project_apply_line_window_edits`, `run_governed_command`, `verify_project_state` anymore. Changes to harness-edit logic in `governed_edit.rs` or `reality-loop.rs` no longer require MCP reconnect.
- **`target/release/context-graph-cli`** — human-facing CLI. Each Bash invocation spawns fresh; always loads current binary. Useful for sanity-checking that a rebuild took effect without going through MCP.

### When `/mcp` reconnect IS still required (rare)

Only for changes to the MCP server's own protocol-layer code:

- Adding/removing tool names (`tools/names.rs`, `tools/definitions/*.rs`, `handlers/tools/dispatch.rs`)
- JSON-RPC framing or transport (`main.rs`, `protocol.rs`)
- Edit-lock semantics (`handlers/tools/reality_loop/lock.rs`)
- Governed-paths policy (`handlers/tools/reality_loop/path_policy.rs`)
- Shift-log writer (`handlers/tools/reality_loop/shift_log.rs`)
- Optimizer recommendation/decision/transition writers (`handlers/tools/reality_loop/optimizer.rs`)
- The shim helper itself (`handlers/tools/reality_loop/alter.rs::spawn_harness_subcommand`) and `handlers/tools/reality_loop/interact.rs::locate_engine_binary`

Changes to **anything in `crates/context-graph-cli/`** (including `governed_edit.rs`, `bin/reality-loop.rs`, the harness CLI subcommands) do NOT require reconnect. The shim re-resolves the binary on every call.

### After every `changed` recommendation, you MUST do this in the SAME turn

This is non-negotiable. Without it, the next mint may run on stale binary code.

1. **`cargo build --release` via Bash** immediately after `cc-reality-harness-editor` returns success. The editor's `cargo check` does NOT produce new release binaries; only `cargo build --release` does. Build both crates the editor touched (typically `-p context-graph-cli` for harness logic; add `-p context-graph-mcp` ONLY if the edit touched the MCP protocol layer).
2. **Confirm `target/release/reality-loop` mtime moved forward**:
   ```bash
   stat -c '%y %n' target/release/reality-loop target/release/context-graph-mcp
   ```
   The mtime should be newer than the start of this turn. The MCP binary mtime is informational — its rebuild only matters if step 4 below applies.
3. **Smoke-test the new CLI binary directly** to prove the change actually took effect — this exercises the SAME subprocess code path the MCP shim spawns on every harness call:
   ```bash
   ./target/release/reality-loop harness open-window --path .claude/loop.md --start-line 1 --end-line 1
   ```
   The output should be a JSON envelope with `status: "ok"` and a `sha256` matching `sha256sum`. If your edit changed `governed_edit.rs`, this single command proves the new code runs in a fresh subprocess — same mechanism the MCP server uses on every harness tool call. Add the command + exit_code to `verification_commands`.
4. **Did the recommendation touch the MCP protocol layer?** Only if YES (files in `crates/context-graph-mcp/src/handlers/`, `protocol.rs`, `tools/`, `main.rs`, `lock.rs`, `path_policy.rs`, `shift_log.rs`, `optimizer.rs`, `alter.rs`'s `spawn_harness_subcommand`, `interact.rs`'s `locate_engine_binary`) → surface in `remaining_risks` as `"operator: /mcp reconnect required to load new MCP protocol-layer behavior from <files>"`. For pure harness-logic changes (`crates/context-graph-cli/**` only) → do NOT request reconnect; the shim's next subprocess call picks up the new binary automatically.
5. **Engine staleness check**: if a `reality-loop` engine PID is mid-attempt, its `etime` predates your new binary. The in-flight attempt CANNOT benefit (running engine has its binary mapped at exec time). Decision tree:
   - In-flight attempt elapsed < 30 min: let it finish naturally; next mint picks up new binary.
   - In-flight attempt elapsed > 60 min AND last 3 turns recorded `no_change` for "round still in flight": surface in `remaining_risks` as `"operator: in-flight engine PID <X> stale relative to fresh harness fix; consider terminating to drip fresh attempt sooner"`. Do NOT kill it yourself.

### Pre-iteration freshness probe (run at the top of EVERY turn)

Before deciding skip-conditions, run:
```bash
stat -c '%Y %n' target/release/context-graph-mcp target/release/context-graph-cli target/release/reality-loop 2>/dev/null
ps -eo pid,etime,cmd | grep -E 'reality-loop attempt|context-graph-mcp' | grep -v grep
```
Compare each running process's etime-derived start time against the binary mtime. If a process started BEFORE the on-disk binary was rebuilt, that process is stale. If the running MCP is stale AND a previous turn already recorded a `changed` recommendation, write the recommendation with `remaining_risks: ["MCP server stale relative to harness fix from turn-NN — but loop continues unblocked because all MCP harness-write tools are avoided this turn"]`. The loop CONTINUES — it does not need an operator reconnect to make progress.

### MCP→CLI shim (DONE 2026-05-07 for harness writes)

**Status: shipped.** The four MCP harness write tools — `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_verify_state` — are now thin shims around `target/release/reality-loop harness <subcommand>`.

Implementation:

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/alter.rs::spawn_harness_subcommand` — the subprocess helper. Sets `kill_on_drop(true)`, explicit CWD, stdout/stderr piped, stdin piped only when payload present, `tokio::time::timeout` wraps `wait_with_output`.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs::locate_engine_binary` — resolves the CLI binary path. Honors `CGREALITY_REALITY_LOOP_BIN` env override, then prefers `target/release/reality-loop`, falls back to `target/debug/reality-loop`.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — adds the `harness` subcommand with four sub-subcommands (open-window, apply-line-window-edit, run-command, verify-state). Each is a thin wrapper around the same `governed_edit::*` functions the MCP previously called in-process. Output contract: stdout = JSON envelope, stderr = human-readable, exit codes 0/1/2/64+ per LSP/JSON-RPC convention.

Verified end-to-end with 12 tests including SHA-256 physical state verification (artifacts on disk, file SHAs match readbacks, error envelopes preserved across the boundary).

**What this means for the loop**:

- After a `changed` recommendation that touches `governed_edit.rs`, `bin/reality-loop.rs`, or anything in `crates/context-graph-cli/`, the next MCP harness-tool call (in the same iteration or any future one) runs the new code with NO operator action required. This is the "MCP-as-shim" property in practice.
- You can verify the freshness invariant in any turn: read `target/release/reality-loop`'s mtime, then call `mcp__cgreality__harness_open_window` on a known governed file — the response should reflect the current binary semantics consistently.
- Override path via `CGREALITY_REALITY_LOOP_BIN` env var if you ever need to point the shim at a different binary (e.g., a debug build during local-dev).

**Tools NOT YET shimmed (lower priority; surface as `remaining_risks` if you find friction)**:

- `mcp__cgreality__optimizer_record_recommendation` / `optimizer_record_decision` / `optimizer_record_harness_transition` — schema-validate + write JSON + append ledger. The blackbox-ledger append already shells out to `target/debug/context-graph blackbox`; the schema validation is in-MCP. Low staleness risk because the schema rarely changes.
- `mcp__cgreality__experiment_registry_*`, `champion_state_*`, `attempts_history_query` — pure JSON file IO; no harness-code dependency.
- `mcp__cgreality__reality_shift_log` / `reality_shift_compare_to_my_view` — file-based persistence in the MCP process. Stale only if the shift-log format itself changes (rare).
- `mcp__cgreality__reality_audit_trail` — reads RocksDB through the MCP server's storage stack (not a harness path; staleness is on the storage-stack side).

For each unshimmed tool: if a `cc-reality-harness-editor` edit touches its in-MCP code, request `/mcp reconnect` in `remaining_risks`. Otherwise no reconnect.

## Iteration procedure

### 1. Confirm reality + check skip conditions

Call `mcp__cgreality__reality_latest_root`. Then evaluate the 5 SKIP conditions from CLAUDE.md:

1. **All queue tasks resolved**: every manifest in `tasks/swebench-lite/*.json` has a champion with `official_resolved: true` for `qwen3.6:27b`. Surface to operator and exit `no_change`.
2. **Cost cap reached**: cumulative session API cost approaching budget. Record `blocked` with `cost_cap_reached` and exit.
3. **External drift detected**: `mcp__cgreality__reality_shift_compare_to_my_view` reports `external_drift_detected: true` on a governed path. Re-perceive before dripping.
4. **Recent drip in flight**: latest attempt was minted <5 minutes ago AND no harness change has been recorded since. Avoids double-drip when cron and operator fire near-simultaneously.
5. **Trigger decision overrides**: `mcp__cgreality__reality_trigger_decision` for the latest attempt returns `should_run: false` with a non-`manual_operator` policy.

If any skip condition holds, record `no_change` immediately and exit. Otherwise → step 2.

### 2. Drip a fresh attempt (the standing self-drive)

Per CLAUDE.md "Standing self-drive authorization", drip a fresh `mcp__cgreality__reality_run_attempt` against the next unresolved task in the queue.

**Pick the next task**:
- List `tasks/swebench-lite/*.json` in lex order.
- For each, call `mcp__cgreality__champion_state_get {model: "qwen3.6:27b", task: "<id>"}`. The first instance with no champion or `official_resolved: false` is the candidate.
- **Park rule**: skip any task with 3+ consecutive non-resolved attempts in `mcp__cgreality__attempts_history_query {model: "qwen3.6:27b", instance_id: "<id>", limit: 10}`. A parked task waits until a harness improvement plausibly addresses its failure mode OR all other tasks are also parked.

Call `mcp__cgreality__reality_run_attempt` with: `task_manifest=tasks/swebench-lite/<id>.json`, `runtime_root=<current runtime root>`, `max_attempts=1`, `official_eval=true`, `model="qwen3.6:27b"`. Binary defaults handle the GGUF path via the pinned `qwen3_6_27b_q6_k_xl` entry.

### 3. Inspect what shifted since last turn

Call `mcp__cgreality__reality_shift_log {session_id: "<your session id>", limit: 5}` to see what your previous turns (in this session) did. Call `mcp__cgreality__reality_compare_attempts` between the last two attempts if both exist.

If you see external drift (a file's current SHA differs from your session's last-known SHA AND no shift in your log caused it), surface it via the recommendation and prefer a `no_change` until you can re-perceive cleanly.

### 4. Consult autoresearch state (Phase 15)

- `mcp__cgreality__champion_state_get {model: "qwen3.6:27b", task: "<the instance>"}`. If a champion already exists with `official_resolved=true` and the latest attempt's reward is within 5% of the champion's reward, prefer `no_change` — the loop should not churn near a verified champion.
- `mcp__cgreality__experiment_registry_list {status_filter: "discarded", limit: 5}` and `{status_filter: "kept", limit: 5}`. If three consecutive experiments for similar diagnoses were discarded, mark the new candidate as `requires human inspection` instead of running another speculative edit.
- `mcp__cgreality__attempts_history_query {model: "qwen3.6:27b", instance_id: "<task>", limit: 10}` to see the recent reward trajectory before drafting an intervention.

### 5. Diagnose

Use the `cc-reality-diagnostician` subagent (Sonnet, read-only). It returns structured Markdown with failure class, root cause hypothesis, evidence file paths, similar past failures, and suggested intervention surface.

If the diagnosis is ambiguous, also spawn `cc-reality-reward-analyst` and/or `cc-reality-swe-failure-triage` (read-only, parallel-safe).

### 6. Decide

- If diagnosis says no edit warranted → step 9 with status `no_change`.
- If edit surface is `swebench_environment` or `swebench_oracle` (not the harness) → record `blocked` with reason describing the environment/oracle issue.
- Otherwise proceed.

### 7. Edit (single writer, GENERIC-ONLY)

Use `cc-reality-harness-editor` (Opus, single writer). It opens the file window, applies the smallest viable edit, runs cargo check, mints a harness-transition record. Returns the harness-transition path and verification result.

If the edit subagent reports verification failure, treat it as `blocked`. The subagent will have reverted; do not try to repair.

**GENERIC-ONLY rule (operator-granted 2026-05-06)**: every harness edit MUST be applicable to ANY SWE-bench task / Python repo / bug class. Test by asking "would this same change help if the inner actor were attempting psf__requests-2317 / django__django-12286 / sympy__sympy-13895 instead?" If no, reformulate as a general principle. Per-task knowledge belongs in the inner actor's reasoning, not the harness. The end goal is the FULL SWE-bench Lite suite (300 tasks) running under the SAME harness in one shot — overfit fixes will hurt 299 other tasks.

### 8. Readback

Use `cc-reality-source-of-truth-readback`. It re-reads each claimed-changed file via `mcp__cgreality__reality_replay_artifact` and confirms SHAs match the editor's report. It also checks `mcp__cgreality__harness_git_status` for unexpected dirty files.

If readback reports any mismatch, that's a hard error: record `error` with the mismatch detail.

### 9. Record

Call `mcp__cgreality__optimizer_record_recommendation` with the schema in `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`. Be precise:

- `status`: `changed | no_change | blocked | error`
- `claude_model`: `claude-opus-4-7`
- `reason`: one sentence
- `source_files_changed`: project-relative paths
- `verification_commands`: the exact commands you ran (cargo check, cargo test, smoke)
- `source_of_truth_readbacks`: file:... paths the next turn should consult
- `next_inner_prompt_delta`: a one-line directive (GENERIC, not task-specific) that, if added to the inner actor's next prompt, would help
- `expected_reward_signal_delta`: which signal you expect to move and by how much
- `remaining_risks`: bullets
- `memories_created`, `memories_boosted`: uuids
- `harness_transitions_minted`: file:...
- `shift_log_excerpt`: shift_ids of the shifts your turn produced (so future readers can replay)
- `created_at_unix` and `source_of_truth`: provide schema-shaped placeholders; the MCP server stamps the actual write timestamp and file path

The Stop hook will allow the turn to end once this is recorded.

## Cost / model policy

- Default model for the optimizer (you): **opus** (per `.claude/settings.json`). Architecture-level reasoning needs the headroom.
- Critic subagents (diagnostician, reward-analyst, swe-triage, readback) run on **sonnet**.
- The writer subagent (harness-editor) runs on **opus**.
- One full turn is expected to consume **<$0.50** in API cost. If a turn approaches that ceiling, stop and record `blocked` with `cost_cap_reached`.

## Hard rules (will be enforced by hooks/MCP, but you should know)

- Never call any HTTP API directly (no curl, no wget). The MCP server is the only network/IO surface for the optimizer.
- Never edit `.env`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`. The deny list in `.claude/settings.json` enforces this; don't try to work around it.
- Never claim SWE-bench success unless `mcp__cgreality__reality_official_report` confirms `resolved_ids` contains the target instance.
- Never use destructive git commands (`push --force`, `-f`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`). Plain `git push` (fast-forward) is allowed.
- Never spawn another writer subagent in parallel with `cc-reality-harness-editor`.
- Never weaken or remove a verifier, hook, or deny rule because it blocked you. If the harness is wrong, edit the harness; if a deny rule is wrong, surface it to the operator. Do not bypass.
- Cap each turn at one `cc-reality-harness-editor` invocation. If diagnosis suggests multiple separate edits, surface the additional ones in `next_inner_prompt_delta` or `remaining_risks` and let the next iteration handle the follow-up.

## Artifact layout you should know by heart

```
<runtime_root>/<task_id>/attempt-N/
  problem-reality-packet.json
  reality-signal-packet.json
  attempt-summary.json
  agent-tool-transcript.json
  validation.json
  inner-model-readback.json
  inner-harness-response.json
  official-evidence/<instance>.json
  harness-transitions/round-NN.json
  trigger-decision.json                          # written by mcp__cgreality__optimizer_record_decision
  claude-code-optimizer/
    recommendation-turn-NN.json                  # written by mcp__cgreality__optimizer_record_recommendation
    decision-turn-NN.json                        # written by mcp__cgreality__optimizer_record_decision
    commands/<unix>-<command>.json               # written by mcp__cgreality__harness_run_command
```

Source-of-truth file paths in your output should always start with `file:`.

## Operator-driven skills (alternative entry points)

These wrap subsets of the iteration procedure for targeted use:

- `/ccreality-iterate` — run one full optimizer turn (the workhorse).
- `/ccreality-smoke` — verify the full pipeline without Docker.
- `/ccreality-trigger-check` — read-only check on whether to iterate.
- `/ccreality-readback` — drift detection on past recommendations.

## When `/loop` runs unattended

The 15-minute cron fires this prompt as one iteration. Across iterations:

- The shift log accumulates. Use `mcp__cgreality__reality_shift_log` to see the full session arc.
- Memory tools (`store_memory`, `boost_importance`) let you persist patterns across iterations.
- The Stop hook ensures every iteration ends with a recorded recommendation, so the artifact tree is the durable trace of what you did.

If three consecutive iterations record `blocked` for the same root cause, escalate: record a recommendation with `remaining_risks` including `"requires human inspection: <one-line-summary>"`. The operator will see this in the dashboard and intervene.
