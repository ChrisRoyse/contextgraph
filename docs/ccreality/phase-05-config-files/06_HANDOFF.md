# Phase 5 — Handoff

## What's true after Phase 5

- `config/ccreality_governed_paths.json` exists with allow/deny lists.
- `config/ccreality_recommendation_schema.json` exists; valid JSON Schema Draft 2020-12.
- A sample recommendation exists under `docs/ccreality/phase-05-config-files/sample_recommendation.json` that validates against the schema.
- Phase 3's `path_policy::load()` (when run) finds and loads the governed-paths file.
- Phase 4's `RecommendationValidator::load()` finds and loads the schema.

## Completion Record

- Completed on 2026-05-05.
- Canonical verification log: `/tmp/phase-05-verify.log`, ending with `Phase 5 verification: ALL PASS`.
- Progress log source of truth: `/tmp/ccreality-phase-progress.log`, containing `phase-05: PASS`.
- Schema validation source of truth: `docs/ccreality/phase-05-config-files/sample_recommendation.json` validates against `config/ccreality_recommendation_schema.json`, and `/tmp/bad-rec.json` was rejected during verification.

## Useful environment variables

When running the MCP server outside Claude Code (e.g., manual smoke), set:
```bash
export CONTEXTGRAPH_ROOT=/home/cabdru/contextgraph
export CGREALITY_GOVERNED_PATHS=$CONTEXTGRAPH_ROOT/config/ccreality_governed_paths.json
export CGREALITY_RECOMMENDATION_SCHEMA=$CONTEXTGRAPH_ROOT/config/ccreality_recommendation_schema.json
```

`.claude/settings.json` (Phase 7) sets these automatically inside Claude Code sessions via the `env` block.

## Next phase

→ [phase-04-mcp-interact-alter-shift/](../phase-04-mcp-interact-alter-shift/README.md)

Phase 3 is now complete. Phase 4 consumes both Phase 3's MCP foundation and Phase 5's config files; do not mark Phase 4 complete until the full Phase 4 verification block passes and records `phase-04: PASS`.
