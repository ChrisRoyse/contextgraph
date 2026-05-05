# Structured Output Schema — `optimizer_record_recommendation`

The optimizer's terminal artifact per turn. Validated by the MCP tool against the schema below. Persisted as `<runtime_root>/<task>/attempt-N/claude-code-optimizer/recommendation-turn-NN.json`. Mirrored as a row in the BlackBoxFix ledger (`record_kind = "ccreality_optimizer_recommendation"`).

## File: `config/ccreality_recommendation_schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://contextgraph.dev/schemas/ccreality_recommendation.v1.json",
  "title": "ccreality optimizer recommendation",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "schema_version",
    "record_kind",
    "claude_session_id",
    "claude_model",
    "run_id",
    "attempt",
    "turn_number",
    "status",
    "reason",
    "source_files_changed",
    "verification_commands",
    "source_of_truth_readbacks",
    "next_gemma_prompt_delta",
    "expected_reward_signal_delta",
    "remaining_risks",
    "memories_created",
    "memories_boosted",
    "harness_transitions_minted",
    "shift_log_excerpt",
    "created_at_unix",
    "source_of_truth"
  ],
  "properties": {
    "schema_version": { "const": 1 },
    "record_kind": { "const": "ccreality_optimizer_recommendation" },
    "claude_session_id": { "type": "string", "minLength": 1 },
    "claude_model": { "type": "string", "enum": ["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5-20251001", "opus", "sonnet", "haiku", "inherit"] },
    "run_id": { "type": "string", "minLength": 1 },
    "attempt": { "type": "integer", "minimum": 0 },
    "turn_number": { "type": "integer", "minimum": 1 },
    "status": { "enum": ["changed", "no_change", "blocked", "error"] },
    "reason": { "type": "string", "minLength": 4, "maxLength": 4000 },
    "source_files_changed": {
      "type": "array",
      "items": { "type": "string", "pattern": "^[a-zA-Z0-9_./-]+$" },
      "uniqueItems": true,
      "maxItems": 25
    },
    "verification_commands": {
      "type": "array",
      "items": { "type": "string", "maxLength": 400 },
      "maxItems": 25
    },
    "source_of_truth_readbacks": {
      "type": "array",
      "items": { "type": "string", "pattern": "^(file:|sqlite:|git:)" },
      "uniqueItems": true,
      "minItems": 0,
      "maxItems": 50
    },
    "next_gemma_prompt_delta": { "type": "string", "maxLength": 2000 },
    "expected_reward_signal_delta": { "type": "string", "maxLength": 2000 },
    "remaining_risks": {
      "type": "array",
      "items": { "type": "string", "maxLength": 600 },
      "maxItems": 25
    },
    "memories_created": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "uniqueItems": true,
      "maxItems": 50
    },
    "memories_boosted": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "uniqueItems": true,
      "maxItems": 50
    },
    "harness_transitions_minted": {
      "type": "array",
      "items": { "type": "string", "pattern": "^file:" },
      "uniqueItems": true,
      "maxItems": 25
    },
    "shift_log_excerpt": {
      "type": "array",
      "items": { "type": "string", "minLength": 8, "maxLength": 64 },
      "uniqueItems": true,
      "maxItems": 50,
      "description": "shift_ids minted by this turn (from cgreality-shift-log/<session>.jsonl)"
    },
    "diagnosis_summary": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "failure_class": {
          "enum": ["patch_apply_failure", "py_compile_failure", "static_check_failure",
                   "local_validation_failure", "official_eval_failure", "tool_grounding_failure",
                   "reward_unavailable", "external_drift", "other"]
        },
        "root_cause_hypothesis": { "type": "string", "maxLength": 2000 },
        "evidence_paths": {
          "type": "array",
          "items": { "type": "string", "pattern": "^file:" },
          "maxItems": 20
        },
        "intervention_surface": {
          "enum": ["prompt_reinjection", "tool_surface", "reward_features",
                   "structured_edit_path", "problem_reality_packet", "none"]
        }
      },
      "required": ["failure_class", "root_cause_hypothesis", "intervention_surface"]
    },
    "drift_detected": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "files": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["path", "current_sha256", "session_last_known_sha256"],
            "properties": {
              "path": { "type": "string" },
              "current_sha256": { "type": "string" },
              "session_last_known_sha256": { "type": ["string", "null"] },
              "external_writer_hint": { "type": ["string", "null"] }
            }
          },
          "maxItems": 25
        }
      }
    },
    "cost_estimate_usd": { "type": "number", "minimum": 0 },
    "created_at_unix": { "type": "integer", "minimum": 0 },
    "source_of_truth": { "type": "string", "pattern": "^file:" }
  },
  "allOf": [
    {
      "if": { "properties": { "status": { "const": "changed" } } },
      "then": { "properties": { "source_files_changed": { "minItems": 1 }, "harness_transitions_minted": { "minItems": 1 }, "shift_log_excerpt": { "minItems": 1 } } }
    },
    {
      "if": { "properties": { "status": { "const": "no_change" } } },
      "then": { "properties": { "source_files_changed": { "maxItems": 0 }, "harness_transitions_minted": { "maxItems": 0 } } }
    }
  ]
}
```

## Validation enforcement

`mcp__cgreality__optimizer_record_recommendation` validates the input against this schema before writing. On validation failure:

```json
{
  "status": "error",
  "error_code": "CCREALITY_RECOMMENDATION_SCHEMA_INVALID",
  "message": "...",
  "field_path": "/source_files_changed/0",
  "remediation": "ensure source_files_changed paths are project-relative",
  "details": { "json_pointer": "...", "schema_violation": "..." },
  "source_of_truth": null
}
```

The agent must correct and retry. The Stop hook will not allow turn end until a valid recommendation is recorded.

## Why these fields

- **`shift_log_excerpt`** — the principle from [01b_REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md). Every recommendation must declare which shifts it caused, so future turns can replay or revert.
- **`source_of_truth_readbacks`** — durable artifact pointers, not model claims. Every claim in the recommendation must be backed by one of these.
- **`next_gemma_prompt_delta`** — the actionable output that feeds into the next inner-Gemma attempt. This is what closes the meta-loop: optimizer → reality engine.
- **`expected_reward_signal_delta`** — the optimizer's prediction. Future turns can compare actual reward delta to this prediction; consistent over-prediction means the optimizer is hallucinating progress.
- **`drift_detected`** — when set, this recommendation acknowledges that reality shifted out from under the agent. The next turn knows to re-perceive.
- **`diagnosis_summary`** — structured form of what `cc-reality-diagnostician` returned. Lets the artifact be queried by failure class without parsing the natural-language reason.

## Why we use a JSON Schema, not just a Rust struct

1. The optimizer writes via MCP, not via Rust internals. Schema validation is the contract.
2. JSON Schema is queryable: future analytics can `jq` over recommendations and aggregate by `diagnosis_summary.failure_class`.
3. Schema can be hot-reloaded by the MCP server without recompiling, allowing iteration during the design phase.

## Sample valid recommendation (worked example)

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
  "reason": "Diagnosis showed Gemma's tool surface lacked an open_test_file action, so it was guessing test layout. Added that tool spec; cargo check + cargo test pass.",
  "source_files_changed": ["crates/context-graph-cli/src/bin/reality-loop.rs"],
  "verification_commands": [
    "cargo check -p context-graph-cli --bin reality-loop",
    "cargo test -p context-graph-cli --bin reality-loop -- --nocapture",
    "git diff --stat -- crates/context-graph-cli/src/bin/reality-loop.rs"
  ],
  "source_of_truth_readbacks": [
    "file:/tmp/contextgraph-rust-reality-loop/run-2026-05-05-1234/django__django-12345/attempt-3/attempt-summary.json",
    "file:/tmp/contextgraph-rust-reality-loop/run-2026-05-05-1234/django__django-12345/attempt-3/reality-signal-packet.json",
    "file:/tmp/contextgraph-rust-reality-loop/run-2026-05-05-1234/django__django-12345/attempt-3/harness-transitions/round-04.json"
  ],
  "next_gemma_prompt_delta": "Use the new open_test_file tool before drafting any patch that touches a file under tests/.",
  "expected_reward_signal_delta": "Tool grounding signal should rise from 0.42 to ~0.7 on next attempt; patch_signals.repeated_patch should drop to false.",
  "remaining_risks": [
    "open_test_file tool spec may need additional path-policy hardening if Gemma tries to read outside the cloned repo",
    "did not yet add corresponding telemetry signal in reality_signal_packet builder; second turn should add it"
  ],
  "memories_created": [],
  "memories_boosted": [],
  "harness_transitions_minted": [
    "file:/tmp/contextgraph-rust-reality-loop/run-2026-05-05-1234/django__django-12345/attempt-3/harness-transitions/round-04.json"
  ],
  "shift_log_excerpt": ["01J9XYZ..."],
  "diagnosis_summary": {
    "failure_class": "tool_grounding_failure",
    "root_cause_hypothesis": "Gemma was attempting blind test edits because no governed tool exposed the test file structure.",
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
