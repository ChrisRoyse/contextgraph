# Open Questions

Things this plan deliberately leaves unresolved. Each is a real architectural choice that should be revisited during implementation, not assumed away in planning.

## Q1 — Where does the lifted governed-edit module live?

**Status:** resolved in Phase 1.

**Decision:** Option A. The module lives at `crates/context-graph-cli/src/governed_edit.rs`, is exported by `crates/context-graph-cli/src/lib.rs` as `pub mod governed_edit;`, and `crates/context-graph-cli/Cargo.toml` now declares the `context_graph_cli` library target.

The excision plan calls for lifting `project_file_window`, `project_apply_line_window_edits`, `run_openai_project_command`, `verify_openai_project_state`, `validate_optimizer_project_path` out of `reality-loop.rs`.

Options:
- **A.** New module inside `context-graph-cli`: `crates/context-graph-cli/src/governed_edit.rs`. Easiest. The MCP server can `use context_graph_cli::governed_edit;` directly via the Cargo workspace.
- **B.** New shared crate `crates/context-graph-shared/`. Cleaner separation but adds a workspace member.
- **C.** Inline into `crates/context-graph-mcp/src/handlers/tools/reality_loop/governed_edit.rs` and let the engine continue to use it via the same path. Doesn't introduce a new shared crate but tightly couples engine and MCP through the MCP crate.

Original recommendation was **A**. Phase 1 implemented that recommendation.

## Q2 — Does `context-graph-mcp` already exist as a binary, or do we need to scaffold it?

**Status:** resolved in Phase 3.

**Decision:** `crates/context-graph-mcp/src/main.rs` already existed. Phase 3 added an additive `--mode reality-loop` flag to the existing binary; no new MCP binary was scaffolded. The lightweight stdio path keeps stdout JSON-RPC-only and routes logs to stderr.

## Q3 — How does the MCP server get the Claude Code session_id?

**Status:** resolved in Phase 4.

`reality_shift_log` and `reality_shift_compare_to_my_view` need a `session_id` to scope the log. Two options:
- **A.** Caller (Claude Code) passes it as a tool argument. Claude knows its own session_id from the SessionStart hook input that we re-inject.
- **B.** MCP server reads it from the JSON-RPC envelope (some MCP transports include caller identity in metadata).

**Decision:** Option A, with an environment fallback. Tool callers pass `session_id` or `_session_id`; if omitted, the MCP helper reads `CLAUDE_SESSION_ID`; otherwise it uses `cgreality-stdio-session`. Phase 6 hooks should inject the active Claude Code session id so the agent passes it explicitly.

## Q4 — What's the right behavior when two engine attempts run concurrently against different SWE instances?

Each gets its own runtime root, but they share the harness source. The edit lock from [16_PARALLEL_AGENT_POLICY](16_PARALLEL_AGENT_POLICY.md) covers writers; engine attempts (which only READ the harness source) are safe to run in parallel.

But: the active_runtime_root file is single-valued. Which one is "active" if two are running?

Options:
- **A.** Last-write-wins. Simple but breaks the SessionStart drip if operator A is mid-session for run X and operator B's engine starts run Y.
- **B.** Per-process active root: `~/.cache/cgreality/active_runtime_root.<pid>` and SessionStart looks for the most recent one tied to a live process.
- **C.** Per-operator: `~/.cache/cgreality/active_runtime_root.<USER>`. Simple if operators don't share accounts.

Decision for current cutover: **A** for now (single-operator default). Phase 2/4 FSV additionally verified that failed smoke paths do not publish a new active runtime root. The `--no-update-active-root` side-experiment flag remains deferred until multi-run operator workflows become real.

## Q5 — Do we need an `MCP elicitation` for risky alter operations?

The MCP protocol supports `elicitation` — a server can request user input mid-tool-call (and Claude Code has hook support for it). Could be used to require operator confirmation for, say, `harness_apply_line_window_edit` against the engine's own attempt loop.

Decision: **no** for the initial cutover. The deny list + edit lock + recommendation+readback chain provides enough safety. Elicitation adds friction that defeats unattended runs. Revisit if real attacks emerge.

## Q6 — Cost accumulator: where does `cost_today.json` live?

[15_COST_AND_MODEL_POLICY](15_COST_AND_MODEL_POLICY.md) mentions a `~/.cache/cgreality/cost_today.json` accumulator updated by the Stop hook from the session's `total_cost_usd`. Open: schema, rotation cadence (per UTC day? per local day?), aggregation across operators.

Recommendation: per-operator JSON file, rotated daily at UTC midnight, format `{"date": "YYYY-MM-DD", "total_usd": float, "by_session": {"sess_id": float}}`. Watcher script (stage E in [14_TRIGGER_AND_INVOCATION_POLICY](14_TRIGGER_AND_INVOCATION_POLICY.md)) reads this before spawning.

Defer implementation until first turn of real production usage.

## Q7 — Telemetry export

We're producing rich artifacts (`cgreality-shift-log/`, `cgreality-mcp-events.jsonl`, recommendations, harness-transitions). Are we exporting anything outside the local filesystem (Honeycomb, Datadog, etc.)?

Decision: **no** for the initial cutover. ContextGraph already emits OpenTelemetry from the BlackBoxFix Phase 23 work (`docs/blackboxfix/...`); ccreality artifacts can be plumbed into that pipeline later. For now, local files + ledger are the source of truth; aggregation is a future concern.

## Q8 — How do we handle `claude.ai` login auth in unattended mode?

`claude.ai` login is interactive (browser flow). For unattended runs we need `ANTHROPIC_API_KEY`. But operators may have a Claude.ai subscription and prefer that for cost.

Options:
- **A.** Document both. Operator picks via their `claude` config.
- **B.** Force API key for unattended. The watcher script refuses to spawn without `ANTHROPIC_API_KEY` set.

Recommendation: **A**. Don't force; document the tradeoff (Claude.ai subscription is cheaper per turn but requires operator presence).

## Q9 — What if the MCP server crashes mid-tool?

Two failure modes:
- Crash during a non-mutating call: client retry is safe.
- Crash during a mutating call: shift_log may not have been appended; the file may or may not have been mutated.

Mitigation: the alter pattern is `mutate → fsync → re-read → diff → log shift → return`. If the MCP server crashes between mutate and log, the next session's `reality_shift_compare_to_my_view` will detect the orphaned mutation as `external_drift_detected` (since no shift logged it). The recovery path is correct; the orphaned mutation is just attributed to "unknown writer" until manually inspected.

Decision: document this as expected behavior. Don't add transactional guarantees beyond fsync; the cost is too high for the rare failure case.

## Q10 — JSON Schema validation library in Rust?

**Status:** resolved in Phase 4/5.

**Decision:** use the `jsonschema` crate with Draft 2020-12. `RecommendationValidator::load()` reads `CGREALITY_RECOMMENDATION_SCHEMA`, compiles the schema, and returns `CCREALITY_RECOMMENDATION_SCHEMA_INVALID` with per-error details when payload validation fails. Phase 5 FSV verified both a valid recommendation write and a 26-file maxItems rejection with no artifact or ledger mutation.

## Q11 — Should we also expose Claude Code itself as an MCP server (`claude mcp serve`)?

Useful if a future ContextGraph component wanted to call Claude Code's editing tools through MCP rather than spawning the CLI. Not needed for the initial cutover; defer.

## Q12 — Naming: `cgreality` vs `contextgraph-reality` for the MCP server name

Short name (`cgreality`) is faster to type and shows up in tool names like `mcp__cgreality__reality_*`. Long name is more discoverable.

Decision: **`cgreality`**. Used throughout the docs. Easy to grep.

## Q13 — Do we keep `docs/reality/21_grand_scale_reality_autoresearch.md`?

The Karpathy-AutoResearch framing in that doc is still architecturally correct after the migration; only the model swap is different. Two options:
- Keep it; add a header note pointing to ccreality.
- Delete it; the framing is captured in [01_ARCHITECTURE](01_ARCHITECTURE.md) anyway.

Decision: **keep with header note**. The longer architectural framing has reference value.

## Q14 — Long-term: does ccreality replace the BlackBoxFix outer loop concepts?

BlackBoxFix has its own ledger, evidence packets, FSV protocol. ccreality writes into the same ledger (`record_kind = "ccreality_optimizer_recommendation"`), so they coexist. But conceptually there's overlap (both are "optimizer turns" in some sense).

Decision: ccreality is one **subsystem** of BlackBoxFix's broader telemetry/reward universe. Not a replacement. Document the relationship after first 30 days of production data.

## Q15 — Should the operator be able to trigger an attempt-replay for fitness testing?

E.g.: take attempt-3, replay just the validation + reward computation against a candidate harness change, without re-running the inner actor. This would let the operator A/B-test reward feature changes cheaply.

The `reality-loop replay` subcommand from [05_REALITY_ENGINE_RESPONSIBILITY](05_REALITY_ENGINE_RESPONSIBILITY.md) is the seed of this. Defer fleshing out until there's a real reward-feature change to test.

## Current open items after Phase 5

The remaining open implementation questions are deferred intentionally:

- Q6 cost accumulator schema/rotation.
- Q8 unattended `claude.ai` login versus `ANTHROPIC_API_KEY` policy.
- Q11 future `claude mcp serve` exposure.
- Q14 30-day production-data relationship between ccreality and broader BlackBoxFix concepts.
- Q15 replay-based fitness testing for reward-feature changes.

Phases 6–8 should not resolve these unless the hook/scaffolding implementation directly needs them.
