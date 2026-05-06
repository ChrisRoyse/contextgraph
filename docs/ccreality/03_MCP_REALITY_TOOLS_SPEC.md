# MCP Reality Tools Spec — `cgreality` server

This is the largest spec document because the MCP surface is **the** integration. Claude Code reaches ContextGraph through these tools and only these tools (plus the standard memory/graph/causal tools already exposed by `context-graph-mcp`).

## Server identity

- **MCP server name (in `.mcp.json`):** `cgreality`
- **Implementation:** `crates/context-graph-mcp/src/handlers/tools/reality_loop/` — new module added alongside existing tool families.
- **Process:** spawned as stdio child by Claude Code. Logging routed to stderr (rmcp protocol requirement; stdout is reserved for JSON-RPC).
- **Tool name format:** `mcp__cgreality__<tool_name>` per Claude Code MCP convention.
- **Lifecycle:** lives for the duration of one Claude Code session. Holds no persistent state; reads from disk + RocksDB on every call.

## Three tool families

```
ANALYZE   reality_*, search_*, traverse_*, audit_*, provenance_*, ...
INTERACT  reality_run_*, reality_replay_*, reality_query_ledger
ALTER     harness_*, store_memory, boost_importance, forget_concept,
          merge_concepts, optimizer_record_*
```

The split is conceptual; the actual MCP tool list is flat.

---

## Family A — ANALYZE reality

### `reality_latest_root`

**Purpose:** Get the active runtime root. Always called first in a fresh turn.

**Input:** `{}`

**Output:**
```json
{
  "runtime_root": "string (absolute path)",
  "active_run_id": "string or null",
  "active_task_id": "string or null",
  "active_target_instance": "string or null",
  "success_predicate": "string (verbatim)",
  "last_attempt_number": "integer or null",
  "source_of_truth": "file:..."
}
```

**Implementation:** Read `~/.cache/cgreality/active_runtime_root` then walk to the latest run subdirectory; read its `run-summary.json` to extract `task_id`, `target_instance`, `attempt_count`.

---

### `reality_attempt_summary`

**Purpose:** Pull the attempt summary artifact written by the reality engine. The default is full fidelity; `compact: true` returns a bounded, source-linked summary for Claude subagents that do not need the full model transcript.

**Input:**
```json
{ "run_id": "string", "attempt": "integer", "compact": "boolean (default false)" }
```

**Output:**
```json
{
  "attempt": { ... AttemptRecord ... },
  "model_response": { ... },
  "blackbox_verifier": { ... },
  "reality_signal_verifier": { ... },
  "score": { ... },
  "reward": { ... },
  "ledger_counts": { "string": integer, ... },
  "source_of_truth": "file:<runtime>/<run>/<task>/attempt-N/attempt-summary.json"
}
```

**Implementation:** Resolve path, read JSON, return as-is. No transformation.

---

### `reality_official_report`

**Purpose:** Pull the official SWE-bench Docker eval JSON for an attempt.

**Input:** `{ "run_id": "string", "attempt": "integer" }`

**Output:** the official report JSON unchanged + `source_of_truth`.

---

### `reality_problem_packet`

**Purpose:** Pull the problem-reality packet (oracle targets, candidate files, etc.).

**Input:** `{ "run_id": "string", "compact": "boolean (default false)" }`

**Output:** packet (full or compact via `compact_problem_reality_packet`).

---

### `reality_signal`

**Purpose:** Pull the reality-signal packet for a specific attempt.

**Input:** `{ "run_id": "string", "attempt": "integer", "compact": "boolean" }`

**Output:** packet (full or `compact_reality_signal_packet`).

---

### `reality_failure`

**Purpose:** Pull just the failure-evidence subset for an attempt.

**Input:** `{ "run_id": "string", "attempt": "integer" }`

**Output:** `compact_failure_evidence` of the attempt's validation JSON + source_of_truth.

---

### `reality_trigger_decision`

**Purpose:** Pull the trigger-decision artifact (why this turn should run, or why it should not).

**Input:** `{ "run_id": "string", "attempt": "integer" }`

**Output:** `{ "status", "should_run", "policy", "reasons", "source_of_truth" }`.

This artifact is now written by the **MCP server itself** at the start of each Claude Code session via `mcp__cgreality__optimizer_record_decision` — see ALTER family. The reality engine no longer makes this decision (the previous DeepSeek-era `decide_deepseek_optimizer_trigger` is removed; see [04_EXCISION_PLAN](04_EXCISION_PLAN.md)).

---

### `reality_harness_transitions`

**Purpose:** List or fetch harness-transition records for an attempt.

**Input:**
```json
{ "run_id": "string", "attempt": "integer", "round": "integer or null" }
```

**Output:**
- If `round` null: list of `{ round, tool_use_id, file_path, outcome, source_of_truth }`.
- If `round` provided: full transition record JSON.

---

### `reality_compare_attempts`

**Purpose:** Diff two attempts at a high level.

**Input:** `{ "run_id": "string", "attempt_a": "integer", "attempt_b": "integer" }`

**Output:**
```json
{
  "patch_diff_summary": { "lines_added", "lines_removed", "files_changed" },
  "validation_status_change": { "from", "to" },
  "official_resolved_change": { "from": "boolean or null", "to": "..." },
  "reward_delta": "number or null",
  "patch_sha_a": "string", "patch_sha_b": "string",
  "patch_repeated": "boolean",
  "source_of_truth": ["file:...attempt-A/...", "file:...attempt-B/..."]
}
```

---

### `reality_shift_log`

**Purpose:** Read the per-session shift log. Every successful alter-tool call appends one record here. This is how the agent perceives "how reality is shifting" within its own session. See [01b_REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md).

**Input:**
```json
{
  "session_id": "string (Claude Code session_id)",
  "limit": "integer (default 20)",
  "since_shift_id": "string or null"
}
```

**Output:** array of shift records, each:
```json
{
  "shift_id": "uuid",
  "timestamp_unix_ns": "integer",
  "tool_name": "string (the alter tool that produced this shift)",
  "tool_use_id": "string",
  "subject": { "type": "file|memory|recommendation|decision|harness_transition", "...": "..." },
  "before": { "sha256": "string", "size_bytes": "integer", "mtime_unix": "integer" },
  "after":  { "sha256": "string", "size_bytes": "integer", "mtime_unix": "integer" },
  "delta_summary": { "lines_added": "integer", "lines_removed": "integer", "diff_stat": "string" },
  "verification": { "cargo_check": "PASS|FAIL|SKIPPED", "smoke": "PASS|FAIL|SKIPPED" },
  "harness_transition_path": "file:..."
}
```

**Implementation:** Reads `<runtime_root>/cgreality-shift-log/<session_id>.jsonl`. Filters by `since_shift_id` if provided. Always sorted by `timestamp_unix_ns` ascending.

---

### `reality_shift_compare_to_my_view`

**Purpose:** Drift detection. Compare each file's current SHA on disk to the SHA the session last saw (from its own shift log). If they differ AND no shift in this session caused the change, an external writer mutated reality between the agent's actions.

**Input:**
```json
{ "session_id": "string", "files": ["string (project-relative path)"] }
```

**Output:**
```json
{
  "files": [{
    "path": "string",
    "current_sha256": "string",
    "session_last_known_sha256": "string or null",
    "drift": "no_change | session_caused | external_drift_detected | unseen_file",
    "external_writer_hint": "string or null"
  }]
}
```

**`drift` values:**
- `no_change` — current SHA matches session's last-known SHA exactly.
- `session_caused` — current SHA matches the most recent `after.sha256` in this session's shift log for this file. The change is the agent's own work.
- `external_drift_detected` — current SHA differs from session's last-known SHA AND does not match any session shift's `after.sha256` for this file. **Strong signal**: the agent must re-perceive before continuing.
- `unseen_file` — the session has never opened or edited this file; no last-known SHA exists. The agent should call `harness_open_window` first.

**`external_writer_hint`** is filled when the MCP server can identify the cause (e.g. "git checkout was run", "another Claude Code session edited at 12:45:08"). Otherwise null.

---

### `reality_audit_trail`

**Purpose:** Pull the ContextGraph audit log for an entity.

**Input:**
```json
{ "entity_id": "string (uuid)", "limit": "integer (default 50)" }
```

**Output:** runtime audit records from the active ccreality runtime tree.

This is the canonical "how did this runtime artifact get here" tool for lightweight `--mode reality-loop`.

---

### Memory-graph tools

Memory and graph tools remain available through the full ContextGraph MCP server. The lightweight `cgreality` MCP server must not advertise these names in `tools/list`, because its `tools/call` dispatcher does not execute them:

- `search_graph` — multi-space search across 13 embedders
- `search_by_embedder` — single-embedder search
- `get_memory_neighbors` / `get_unified_neighbors` — k-NN by edge type and direction
- `traverse_graph` — BFS/DFS with filter
- `get_audit_trail` — audit log per memory
- `get_provenance_chain` — full provenance ancestry
- `get_memory_fingerprint` — semantic + lexical fingerprint
- `extract_entities` — KEPLER E11 entity extraction
- `search_recent` — temporal recency
- `causal_discover` — causal graph traversal

**Why re-expose under `cgreality`?** Claude Code's allowlist works by server name. If we only allow `mcp__cgreality__*`, we cover everything in one rule and we can deny-list other MCP servers without touching memory access.

---

## Family B — INTERACT with reality

### `reality_run_attempt`

**Purpose:** Run one Gemma attempt and produce a fresh attempt-N artifact tree.

**Input:**
```json
{
  "task_manifest": "string (absolute path)",
  "runtime_root": "string (absolute path; default: latest)",
  "max_attempts": "integer (default 1)",
  "official_eval": "boolean (default true)",
  "model": "string (default gemma4:e4b)"
}
```

**Output:**
```json
{
  "status": "success | failed | error",
  "run_id": "string",
  "attempt": "integer",
  "exit_code": "integer",
  "stdout_tail": "string (last 4 KB)",
  "stderr_tail": "string (last 4 KB)",
  "attempt_summary_path": "file:...",
  "source_of_truth": "file:..."
}
```

**Implementation:** Spawn `target/debug/context-graph-cli reality-loop attempt --task-manifest=… --runtime-root=… --max-attempts=…` with `Stdio::piped`, capture exit + tails, return refs to the freshly written attempt JSON.

**Timeout:** configurable, default 1800 seconds (30 min). Returns `status: "error"` with `error_code: "REALITY_RUN_ATTEMPT_TIMED_OUT"` if exceeded.

---

### `reality_run_smoke`

**Purpose:** Run a synthetic smoke attempt that does not require Gemma + Docker.

**Input:** `{ "runtime_root": "string (default tmp)", "label": "string (default smoke)" }`

**Output:** as `reality_run_attempt`.

**Implementation:** Calls `reality-loop smoke …`. The smoke writes a deterministic synthetic attempt artifact tree. Used for first-light verification of the whole MCP↔engine wiring without burning Docker time.

---

### `reality_run_official_eval`

**Purpose:** Re-run official SWE-bench Docker eval against an existing attempt's patch.

**Input:** `{ "run_id": "string", "attempt": "integer" }`

**Output:** updated official report + new SHA.

Useful when the operator wants to re-confirm an official result without re-running Gemma.

---

### `reality_run_targeted_test`

**Purpose:** Run a single SWE-bench `fail_to_pass` test against the candidate patch in a workspace.

**Input:** `{ "run_id": "string", "attempt": "integer", "test_id": "string", "timeout_secs": "integer (default 300)" }`

**Output:** `{ "status", "exit_code", "stdout_tail", "stderr_tail", "source_of_truth" }`.

---

### `reality_recompile_reward`

**Purpose:** Re-run the BlackBoxFix reward compiler against an existing reality-signal packet, producing a new reward record.

**Input:** `{ "run_id": "string", "attempt": "integer" }`

**Output:** `{ "reward": { ... }, "source_of_truth": "file:..." }`.

Useful after the operator edits a reward feature in the harness; the change should produce a different reward without re-running the attempt.

---

### `reality_replay_artifact`

**Purpose:** Re-read a specific durable artifact and return it verbatim with a fresh SHA. Used when the optimizer wants to confirm an artifact is still on disk and unmodified between hook firings.

**Input:** `{ "path": "string (file:... or absolute)" }`

**Output:** `{ "exists", "sha256", "size_bytes", "mtime_unix", "content_or_pointer", "source_of_truth" }`. Content is inlined only if `size_bytes < 32KB`; otherwise returns just the pointer + sha.

---

### `reality_query_ledger`

**Purpose:** Query the BlackBoxFix SQLite ledger.

**Input:**
```json
{ "run_id": "string", "where": "string (SQL WHERE clause)", "limit": "integer (default 50)" }
```

**Output:** `{ "rows": [...], "row_count", "source_of_truth": "sqlite:..." }`.

Read-only; injects `LIMIT` and rejects non-SELECT statements.

---

## Family C — ALTER reality

Every alter tool follows the **mutate → fsync → re-read → diff → log shift → return readback** pattern. The MCP tool is the source of write provenance: every alter call records `tool_use_id` (provided by Claude Code in the MCP call envelope) into the artifact's metadata AND appends a shift record to `<runtime_root>/cgreality-shift-log/<session_id>.jsonl` (see [01b_REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md)).

**Mandatory shift-log discipline:**
1. The shift record is appended only AFTER the mutation has been re-read and verified.
2. The append is fsync'd before the tool returns (so a crash either commits the shift or leaves no trace).
3. The shift record's `before.sha256` MUST equal the SHA the agent supplied as `observed_sha256` (or, for non-file mutations, the previous artifact's SHA).
4. The shift record's `after.sha256` is computed by re-reading the freshly-written artifact.
5. The tool's response includes the new `shift_id` so the agent can fetch it back via `reality_shift_log`.

### `harness_open_window`

**Purpose:** Open a window of project source for editing. Returns SHA so the next call can prove no concurrent change.

**Input:** `{ "path": "string (project-relative)", "start_line": "integer", "end_line": "integer" }`

**Output:** `{ "path", "start_line", "end_line", "total_lines", "text", "sha256", "source_of_truth" }`.

**Hard policy:**
- Path must be project-relative and inside the allowlisted project subtree.
- Path is denied if it matches any rule in `config/ccreality_governed_paths.json`; deny wins over allow. Current deny coverage includes `.env`, nested `.env.*`, `swebench/**`, `tests/oracle_**`, SWE-bench datasets, reward specs, verifiers, key material, credentials, `/etc/**`, and `/root/**`.
- Window cap: 400 lines.

This is the analyze-half of the editing pair. Claude must always call this before `harness_apply_line_window_edit`.

---

### `harness_apply_line_window_edit`

**Purpose:** Apply a tool-mediated edit, exactly like the existing `apply_line_window_edit` in `reality-loop.rs` but as an MCP tool.

**Input:**
```json
{
  "path": "string",
  "start_line": "integer",
  "end_line": "integer",
  "observed_sha256": "string (from harness_open_window)",
  "replace": "string"
}
```

**Output:**
```json
{
  "status": "ok",
  "path": "string",
  "start_line", "end_line",
  "before_sha256", "after_sha256",
  "lines_added", "lines_removed",
  "git_diff_path": "file:...",
  "harness_transition_path": "file:...",
  "source_of_truth": "file:..."
}
```

**Behavior:**
1. Validate path policy (same allow/deny as `harness_open_window`).
2. Re-read file → SHA must equal `observed_sha256`; else error `HARNESS_EDIT_SHA_MISMATCH`.
3. Apply edit (line-window replace, exactly like existing `project_apply_line_window_edits`).
4. Re-read → compute new SHA.
5. Write `git diff` artifact.
6. Mint a harness-transition record (round = max(existing) + 1) including the tool_use_id.
7. Return.

This deduplicates existing Rust code; we'll lift `project_apply_line_window_edits` (line 4336 of `reality-loop.rs`) into a shared crate utility used by both the MCP tool and the existing Gemma agent tool.

---

### `harness_run_command`

**Purpose:** Run an allowlisted command in the project root.

**Input:** `{ "command": "string", "timeout_secs": "integer (default 120)" }`

**Allowlist (regex):**
- `^cargo (fmt|check|test|build|clippy)( |$)`
- `^rustfmt( |$)`
- `^git (status|diff|log|show)( |$)`
- `^nvidia-smi$`

**Deny:** anything else, including command chains via `&&`, `;`, `|`, backticks, `$()`.

**Output:** `{ "exit_code", "stdout", "stderr", "duration_ms", "source_of_truth": "file:..." }`. Output is also persisted to a JSON artifact under `<run>/<task>/<claude-session>/commands/`.

---

### `harness_git_diff`

**Purpose:** Convenience wrapper for `git diff` against working tree or a specific path.

**Input:** `{ "path": "string or null", "base": "string (default HEAD)" }`

**Output:** `{ "diff": "string", "diff_stat": "string", "changed_files": ["string"], "source_of_truth": "file:..." }`.

---

### `harness_git_status`

**Purpose:** `git status --porcelain` with structured parse.

**Input:** `{}`

**Output:** `{ "untracked": [...], "modified": [...], "staged": [...], "source_of_truth": "git:..." }`.

---

### `harness_verify_state`

**Purpose:** Run a canned verification suite (rustfmt --check, cargo check, cargo test focused, git status).

**Input:** `{ "scope": "reality_loop_only | full" }`

**Output:**
```json
{
  "rustfmt": { "passed", "files_needing_fmt": [...] },
  "cargo_check": { "passed", "stderr_tail" },
  "cargo_test": { "passed", "test_count", "failed_tests": [...] },
  "git_status": { "clean", "modified": [...] },
  "source_of_truth": "file:..."
}
```

This is the "would a staff engineer approve this" tool. Claude is expected to call it before `optimizer_record_recommendation`.

---

### Memory alter tools (re-exposed, scoped)

These already exist in `context-graph-mcp`; re-exposed under `cgreality` with one extra rule: every call gets `namespace: "ccreality"` injected so memories the optimizer creates are isolated from operator-level memories.

- `store_memory` — store a new memory with embeddings
- `boost_importance` — raise a memory's importance score
- `forget_concept` — soft-delete a memory
- `merge_concepts` — merge two memories with provenance

Use these for: storing successful failure-diagnosis patterns, boosting harness-transition records that produced reward gains, forgetting noisy artifacts after an attempt is fully resolved.

---

### `optimizer_record_decision`

**Purpose:** Mint a fresh trigger-decision artifact at the **start** of an optimization turn. This replaces the Rust-side `decide_deepseek_optimizer_trigger` function — the decision now lives where the decision is made (in the optimizer process, i.e. Claude Code via this MCP call).

**Input:**
```json
{
  "run_id": "string",
  "attempt": "integer (the attempt this turn is reacting to)",
  "policy": "disabled | every_attempt | failure | failure_or_regression | manual_operator",
  "should_run": "boolean",
  "reasons": [{ "reason": "string", "source_of_truth": "string", ... }],
  "claude_session_id": "string",
  "claude_model": "string"
}
```

**Output:** `{ "decision_path": "file:...", "sha256": "..." }`.

The decision artifact is read by the SessionStart and UserPromptSubmit hooks, so subsequent prompts see it.

---

### `optimizer_record_recommendation`

**Purpose:** Mint the structured recommendation that closes a turn. This is what the `Stop` hook waits for.

**Input:** see [13_STRUCTURED_OUTPUT_SCHEMA](13_STRUCTURED_OUTPUT_SCHEMA.md). Briefly:

```json
{
  "run_id": "string",
  "attempt": "integer",
  "claude_session_id": "string",
  "status": "changed | no_change | blocked | error",
  "reason": "string",
  "source_files_changed": ["string"],
  "verification_commands": ["string"],
  "source_of_truth_readbacks": ["file:..."],
  "next_gemma_prompt_delta": "string",
  "expected_reward_signal_delta": "string",
  "remaining_risks": ["string"],
  "memories_created": ["uuid"],
  "memories_boosted": ["uuid"],
  "harness_transitions_minted": ["file:..."]
}
```

**Output:** `{ "recommendation_path": "file:...", "sha256": "...", "ledger_record_id": "string" }`.

The artifact is written under `<run>/<task>/attempt-N/claude-code-optimizer/recommendation-<turn-N>.json` and a corresponding ledger row is appended (`record_kind: "ccreality_optimizer_recommendation"`). The MCP server owns `source_of_truth` and `created_at_unix`: callers provide schema-shaped placeholders, then the server stamps the actual path and write timestamp before the final schema validation and durable write.

---

### `optimizer_record_harness_transition`

**Purpose:** Called by the PostToolUse Edit/Write hook (via stdio MCP client) to write a harness-transition record, since the existing reality-engine harness-transition writer is in the inner-Gemma loop.

**Input:**
```json
{
  "run_id": "string",
  "attempt": "integer",
  "tool_use_id": "string",
  "file_path": "string",
  "before_sha256": "string",
  "after_sha256": "string",
  "lines_added": "integer",
  "lines_removed": "integer",
  "cargo_check": { "exit_code": "integer", "stderr_tail": "string" },
  "git_diff_stat": "string"
}
```

**Output:** `{ "transition_path": "file:...", "round": "integer" }`.

---

## Path policy file

`config/ccreality_governed_paths.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ccreality governed paths policy",
  "description": "Allowed and denied repo-relative paths for CC Reality harness mutation tools.",
  "version": 1,
  "allow": [
    "crates/context-graph-cli/src/bin/reality-loop.rs",
    "crates/context-graph-cli/src/governed_edit.rs",
    "crates/context-graph-cli/src/lib.rs",
    "crates/context-graph-cli/src/**",
    "crates/context-graph-mcp/src/handlers/tools/reality_loop/**",
    "crates/context-graph-mcp/src/handlers/tools/reality_loop/*.rs",
    "scripts/cc/**",
    "scripts/cc/tests/**",
    ".claude/loop.md",
    ".claude/agents/cc-reality-*.md",
    ".claude/skills/ccreality-*/**",
    ".claude/skills/ccreality-*/SKILL.md",
    "docs/ccreality/**",
    "docs/ccreality/phase-*/**",
    "docs/reality/**",
    "config/ccreality_governed_paths.json",
    "config/ccreality_recommendation_schema.json"
  ],
  "deny": [
    ".env",
    ".env.*",
    "**/.env",
    "**/.env.*",
    "swebench/**",
    "tests/oracle_**",
    "tests/oracle_**/**",
    "crates/context-graph-benchmark/datasets/swe_bench_*",
    "crates/context-graph-benchmark/datasets/swe_bench_*/**",
    "crates/*/src/reward_specs/**",
    "crates/*/src/verifiers/**",
    "**/*.pem",
    "**/*.key",
    "**/credentials.json",
    "/etc/**",
    "/root/**"
  ]
}
```

The MCP server reads this from `$CGREALITY_GOVERNED_PATHS` and fails closed if the env var is missing, the file is malformed, the version is unsupported, or a path matches a deny rule. The current implementation compiles this CCReality path-pattern dialect directly so documented rules such as `tests/oracle_**` remain intact instead of being weakened to fit a library glob dialect.

## Error model

Every MCP tool returns either a structured success payload or:

```json
{
  "status": "error",
  "error_code": "CCREALITY_<UPPER_SNAKE>",
  "message": "string",
  "field_path": "string",
  "remediation": "string",
  "details": { ... },
  "source_of_truth": "string or null"
}
```

Error codes are stable. They mirror the existing Rust `RealityError` style so the same error-classification machinery applies.

## Logging

- **Stdout:** JSON-RPC only. Anything else corrupts the protocol.
- **Stderr:** structured logs via `tracing` to `stderr`. Format: `cgreality.<tool>.<phase>` with timestamp, tool_use_id, run_id.
- **Persistent:** every tool call also writes a JSON line to `<runtime_root>/cgreality-mcp-events.jsonl` for replay/debugging.

## Performance budget

- ANALYZE tools: target <200 ms (just disk reads).
- INTERACT tools: bounded by the underlying process timeout.
- ALTER tools (non-command): <500 ms.
- `harness_run_command` and `reality_run_attempt`: bounded by their own timeouts.

The MCP server holds no in-memory state across calls; all state is on disk. This makes it crash-safe and replayable.
