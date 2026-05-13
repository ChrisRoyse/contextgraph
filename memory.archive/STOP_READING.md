# STOP — This directory is archived (2026-05-13)

This directory was the project's SAMP/SMAP shared mind (`./memory/`). It has been **renamed to `./memory.archive/` and is read-only**.

**Memory now lives in GitHub Issues**, per [`docs2/AICodingAgentSuperPrompt.md` §3](../docs2/AICodingAgentSuperPrompt.md) (the AI Coding Agent Doctrine).

## Do not write new files here.

| Old `./memory/` namespace | New home |
|---|---|
| `context/` (mission / current phase / scope) | Pinned `type:context` GitHub issue |
| `decisions/` (locked architectural choices) | Closed `type:decision` GitHub issue (ADR) |
| `discoveries/` (constraints / gotchas / edge cases) | Closed `type:discovery` GitHub issue |
| `patterns/` (reusable conventions) | Closed `type:pattern` GitHub issue (or `AGENTS.md` for stable ones) |
| `blockers/` (unresolved walls) | Open issue with `status:blocked` |
| `handoff/` (deliverable for next agent) | Comments on relevant issue + assignee change |
| `journal/` (chronological execution log) | Issue comments (CLAIM / milestone / PAUSE / RESOLVED / BLOCKED) |
| `reports/` (end-of-session summary) | Final RESOLVED comment on the worked issue + PR description |

## Reading old content here is fine

The historical files under this directory are read-only history. They are searchable for archaeology — they will NOT be used to drive current work. Anything still load-bearing should be migrated to a GitHub Issue on demand.

See [`AGENTS.md`](../AGENTS.md) and [`CLAUDE.md`](../CLAUDE.md) for the current memory protocol.
