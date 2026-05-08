# Phase 5 — Config Files

**Estimated duration:** 30 minutes.
**Status:** complete.
**Depends on:** Phase 1 (complete). Can be done before or after Phase 2, but must be done before Phase 3 or 4.
**Unblocks:** Phase 3 & Phase 4 (which read these files).

## What this phase does

Creates two JSON files under `config/`:

1. `config/ccreality_governed_paths.json` — the allow/deny path policy used by `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, and the PreToolUse Edit hook.
2. `config/ccreality_recommendation_schema.json` — the JSON Schema for `optimizer_record_recommendation` payloads.

Both are loaded by the MCP server (Phase 4) and consumed by hooks (Phase 6).

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
test -f config/ccreality_governed_paths.json && jq -e '.allow' config/ccreality_governed_paths.json >/dev/null
test -f config/ccreality_recommendation_schema.json && jq -e '.["$schema"]' config/ccreality_recommendation_schema.json >/dev/null

# Schema is self-valid. Python jsonschema is required; missing dependency is a verification failure.
python3 -c "import json, jsonschema; s = json.load(open('config/ccreality_recommendation_schema.json')); jsonschema.Draft202012Validator.check_schema(s)"

# Worked-example validates against the schema
python3 -c "
import json, jsonschema
schema = json.load(open('config/ccreality_recommendation_schema.json'))
example = json.load(open('docs/ccreality/phase-05-config-files/sample_recommendation.json'))
jsonschema.validate(example, schema)
print('OK')
"
```

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — exact JSON contents.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification commands.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

Plus a `sample_recommendation.json` artifact in this folder used by the verification.

## Completion Record

- Completed on 2026-05-05.
- Canonical verification log: `/tmp/phase-05-verify.log`, ending with `Phase 5 verification: ALL PASS`.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-05: PASS`.
- Source-of-truth files: `config/ccreality_governed_paths.json`, `config/ccreality_recommendation_schema.json`, and `docs/ccreality/phase-05-config-files/sample_recommendation.json`.
- Schema validation used Python `jsonschema` Draft 2020-12; the sample recommendation validates, and an intentionally incomplete payload is rejected.
- Hardened implementation FSV log: `/tmp/ccreality-phase5-fsv-1778001835.log`.
- Runtime FSV source of truth: `/tmp/cgreality-phase05-fsv-1778001835`.
- Physical ledger source of truth: `/tmp/cgreality-phase05-fsv-1778001835/ledger.sqlite`, with `agent_run|1` and `tool_call|1` after a real `optimizer_record_recommendation`.
- Root cause resolved during hardening: standard glob semantics could not express the documented `tests/oracle_**` policy rule without weakening the policy. The current `path_policy.rs` compiles the documented CCReality path-pattern dialect directly and fails closed on missing env, unsupported version, malformed policy, and denied paths.
- Edge cases manually verified with before/after source-of-truth reads: embedded globstar deny rule, missing `CGREALITY_GOVERNED_PATHS`, unsupported policy version, and recommendation schema maxItems rejection.

## Reference docs

- [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) §"Path policy file" — the exact allow/deny lists to use.
- [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) — the recommendation schema in full.
