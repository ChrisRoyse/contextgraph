# Phase 5 — Handoff

## What's true after Phase 5

- `config/ccreality_governed_paths.json` exists with allow/deny lists.
- `config/ccreality_recommendation_schema.json` exists; valid JSON Schema Draft 2020-12.
- A sample recommendation exists under `docs/ccreality/phase-05-config-files/sample_recommendation.json` that validates against the schema.
- Phase 3's `path_policy::load()` (when run) finds and loads the governed-paths file.
- Phase 4's `RecommendationValidator::load()` finds and loads the schema.

## Useful environment variables

When running the MCP server outside Claude Code (e.g., manual smoke), set:
```bash
export CONTEXTGRAPH_ROOT=/home/cabdru/contextgraph
export CGREALITY_GOVERNED_PATHS=$CONTEXTGRAPH_ROOT/config/ccreality_governed_paths.json
export CGREALITY_RECOMMENDATION_SCHEMA=$CONTEXTGRAPH_ROOT/config/ccreality_recommendation_schema.json
```

`.claude/settings.json` (Phase 7) sets these automatically inside Claude Code sessions via the `env` block.

## Next phase

Three options, depending on which path you took:

- If you completed Phases 0–4 already, → [phase-06-hooks/](../phase-06-hooks/README.md).
- If you came here before Phase 3 (recommended order: 0 → 1 → 5 → 2 → 3 → 4), → [phase-02-engine-subcommands/](../phase-02-engine-subcommands/README.md).
- If you're in the middle of Phase 4 and just needed the schema, → resume Phase 4.
