# Phase 7 — Handoff

## What's true after Phase 7

- `.claude/settings.json` exists with the full ccreality config (model, permissions, env, hooks).
- `.mcp.json` declares the cgreality MCP server.
- `.claude/loop.md` is in place for the `/loop` skill default prompt.
- 5 subagent files under `.claude/agents/cc-reality-*.md`.
- 4 skill `SKILL.md` files under `.claude/skills/ccreality-*/`.
- `config/ccreality_governed_paths.json` allow list now includes `.claude/settings.json`, `.mcp.json`, `.gitignore`, and `CLAUDE.md`.
- Opening Claude Code in this project loads everything: MCP server connects, project settings allow `mcp__cgreality__*`, hooks fire, subagents are discoverable, skills are typeable.
- Root-cause hardening is recorded in the source specs: `.mcp.json` is no longer ignored at the repo root, `.claude/settings.json` uses the official SchemaStore schema URL, MCP permissions use Claude Code's `mcp__cgreality__*` wildcard, and the diagnostician references only tools implemented by the lightweight `cgreality` runtime server.

## What's still missing

- The CLAUDE.md addendum (Phase 8) — the project-memory contract that tells Claude Code how to behave.
- The smoke harness (Phase 9) — the end-to-end test.
- FSV harness automation (Phase 10). Phase 7 itself has a manual FSV log at `/tmp/ccreality-phase7-fsv-1778005930.log`.
- First production attempt + commit (Phase 12).

Without Phase 8's addendum, Claude Code knows the tools and hooks exist but doesn't have the role-defining "you are the outer optimizer" prompt loaded into project memory. It will still work for explicit `/ccreality-iterate` invocations, but won't have the rules-of-engagement automatically applied.

## Manual smoke test (optional)

If `claude` CLI is installed, you can do a quick interactive smoke:

```bash
cd /home/cabdru/contextgraph
claude
# Inside Claude Code:
> /mcp                       # confirm cgreality is listed and connected
> /agents                    # confirm cc-reality-* subagents listed
> /skills                    # confirm ccreality-* skills listed
> /ccreality-trigger-check   # smallest, read-only skill
> exit
```

If any of these fail, Phase 7 is incomplete; do not proceed to Phase 8.

## Verification record

```bash
echo "phase-07: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

Actual verification records from this implementation:

- Canonical verification: `phase-07: PASS, 2026-05-05T18:32:42Z`.
- Manual FSV: `/tmp/ccreality-phase7-fsv-1778005930.log`.

## Next phase

→ [phase-08-claude-md-addendum/](../phase-08-claude-md-addendum/README.md)

Phase 8 is the smallest phase: append the ccreality role addendum to `CLAUDE.md`. ~5 minutes.
