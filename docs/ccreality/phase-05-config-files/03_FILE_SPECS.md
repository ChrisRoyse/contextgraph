# Phase 5 — File Specifications

## `config/ccreality_governed_paths.json`

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
    ".claude/settings.json",
    ".claude/loop.md",
    ".claude/agents/cc-reality-*.md",
    ".claude/skills/ccreality-*/**",
    ".claude/skills/ccreality-*/SKILL.md",
    ".mcp.json",
    ".gitignore",
    "CLAUDE.md",
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

## `config/ccreality_recommendation_schema.json`

(Copied verbatim from [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md). The full schema is too long to repeat here; copy it byte-for-byte from the parent doc.)

The skeleton (must be `additionalProperties: false`, all top-level fields required, `allOf` rules for status-conditional requirements, and `$id`/`$schema` headers):

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://contextgraph.dev/schemas/ccreality_recommendation.v1.json",
  "title": "ccreality optimizer recommendation",
  "type": "object",
  "additionalProperties": false,
  "required": [...],
  "properties": { ...full property set per 13_STRUCTURED_OUTPUT_SCHEMA.md... },
  "allOf": [...status-conditional rules...]
}
```

**Action:** open `docs/ccreality/13_STRUCTURED_OUTPUT_SCHEMA.md`, find the section "## File: `config/ccreality_recommendation_schema.json`", copy the JSON between the triple-backticks verbatim into `config/ccreality_recommendation_schema.json`.

## `docs/ccreality/phase-05-config-files/sample_recommendation.json`

(Copied verbatim from [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) §"Sample valid recommendation".)

```json
{
  "schema_version": 1,
  "record_kind": "ccreality_optimizer_recommendation",
  "claude_session_id": "sess-2026-05-05-abc123",
  "claude_model": "claude-opus-4-7",
  "run_id": "run-2026-05-05-1234",
  "attempt": 3,
  "turn_number": 1,
  "status": "changed",
  "reason": "Diagnosis showed the inner actor's tool surface lacked an open_test_file action, so it was guessing test layout. Added that tool spec; cargo check + cargo test pass.",
  "source_files_changed": ["crates/context-graph-cli/src/bin/reality-loop.rs"],
  "verification_commands": [
    "cargo check -p context-graph-cli --bin reality-loop",
    "cargo test -p context-graph-cli --bin reality-loop -- --nocapture",
    "git diff --stat -- crates/context-graph-cli/src/bin/reality-loop.rs"
  ],
  "source_of_truth_readbacks": [
    "file:/tmp/.../attempt-3/attempt-summary.json",
    "file:/tmp/.../attempt-3/reality-signal-packet.json",
    "file:/tmp/.../attempt-3/harness-transitions/round-04.json"
  ],
  "next_inner_prompt_delta": "Use the new open_test_file tool before drafting any patch that touches a file under tests/.",
  "expected_reward_signal_delta": "Tool grounding signal should rise from 0.42 to ~0.7 on next attempt; patch_signals.repeated_patch should drop to false.",
  "remaining_risks": [
    "open_test_file tool spec may need additional path-policy hardening if the inner actor tries to read outside the cloned repo",
    "did not yet add corresponding telemetry signal in reality_signal_packet builder; second turn should add it"
  ],
  "memories_created": [],
  "memories_boosted": [],
  "harness_transitions_minted": [
    "file:/tmp/.../attempt-3/harness-transitions/round-04.json"
  ],
  "shift_log_excerpt": ["01J9XYZ..."],
  "diagnosis_summary": {
    "failure_class": "tool_grounding_failure",
    "root_cause_hypothesis": "the inner actor was attempting blind test edits because no governed tool exposed the test file structure.",
    "evidence_paths": [
      "file:/tmp/.../attempt-3/agent-tool-transcript.json",
      "file:/tmp/.../attempt-3/reality-signal-packet.json"
    ],
    "intervention_surface": "tool_surface"
  },
  "cost_estimate_usd": 0.18,
  "created_at_unix": 1714896000,
  "source_of_truth": "file:/tmp/contextgraph-rust-reality-loop/run-2026-05-05-1234/django__django-12345/attempt-3/claude-code-optimizer/recommendation-turn-01.json"
}
```

This is a known-valid sample. If the schema rejects it, the schema is wrong. If the schema accepts it, the schema is at least permissive enough for typical recommendations.

## Why version field on the policy

`config/ccreality_governed_paths.json` includes `"version": 1` so we can evolve the policy schema without breaking existing readers. The MCP server's loader can refuse versions it doesn't understand.

## Recommended placement of these in operator's local environment

The MCP server reads from `$CGREALITY_GOVERNED_PATHS` env var first. The `.claude/settings.json` (Phase 7) sets this env to point at `config/ccreality_governed_paths.json` in the project root, so the policy is always project-pinned for any operator running in the project.

For users who want a personal override, they can set `CGREALITY_GOVERNED_PATHS` in their shell profile to a different file. Project policy is the safe default.
