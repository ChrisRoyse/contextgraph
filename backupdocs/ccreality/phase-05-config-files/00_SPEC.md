# Phase 5 — Specification

## Objectives

1. Create `config/ccreality_governed_paths.json` — the canonical path policy.
2. Create `config/ccreality_recommendation_schema.json` — the canonical JSON Schema for optimizer recommendations.
3. Both files committed; both loaded by the MCP server at runtime; both consumable by hooks.

## Scope

- `config/ccreality_governed_paths.json` — NEW.
- `config/ccreality_recommendation_schema.json` — NEW.
- `docs/ccreality/phase-05-config-files/sample_recommendation.json` — NEW (used by verification).

Original scope: only these data files. The MCP server loader code lives in Phase 3/4, and the schema-validator implementation lives in Phase 4.

Current implementation note: Phase 5 hardening later touched the loader tests and path-pattern compiler because the documented governed-paths policy includes `tests/oracle_**`, which cannot be preserved by weakening the policy. The final project state keeps the documented data contract and updates the loader to support it fail-closed.

## Deliverables

| File | Path | Format | Source of authority |
|------|------|--------|---------------------|
| Governed paths policy | `config/ccreality_governed_paths.json` | JSON | [../03_MCP_REALITY_TOOLS_SPEC.md](../03_MCP_REALITY_TOOLS_SPEC.md) §"Path policy file" |
| Recommendation schema | `config/ccreality_recommendation_schema.json` | JSON Schema (Draft 2020-12) | [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) |
| Sample recommendation | `docs/ccreality/phase-05-config-files/sample_recommendation.json` | JSON | [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md) §"Sample valid recommendation" |

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Three clauses:
- Both files exist and are valid JSON.
- The schema is itself valid against JSON Schema Draft 2020-12.
- The sample recommendation validates against the schema.

## Time budget

30 minutes. Mostly copy-paste from the reference docs into the right paths.

## Out-of-scope behaviors

- Loading or consuming these files. That's done by Phase 3/4 code.
- Updating documentation pointers (the docs already reference these paths).
- Adding additional schemas (e.g., for trigger-decision artifact). Out of scope; can be added later if validation gates emerge.
