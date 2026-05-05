# Phase 8 — Specification

## Objectives

Append the ccreality role addendum (from [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md)) to the project's `CLAUDE.md` file.

## Scope

- `/home/cabdru/contextgraph/CLAUDE.md` — append-only modification at the bottom.

Out of scope: any other content modification to CLAUDE.md.

## Deliverables

After Phase 8:
- `CLAUDE.md` is unchanged in its existing content (V3 swarm rules, build/test, security, etc.).
- A new section appears at the bottom, fenced with `---` markers, titled "ccreality — Outer Reality-Loop Optimizer Role".
- The new section is exactly the verbatim block from [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md) §"Append-only block (drop in verbatim)".

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Three clauses:
- The header section title is present.
- The reality-shift principle subsection is present.
- The recommendation tool name is referenced in the addendum.

## Time budget

5 minutes. One file edit.

## Idempotence

If the addendum is already present (e.g., from a previous attempt), do NOT duplicate it. Check first:
```bash
if grep -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md; then
  echo "addendum already present; skipping"
else
  # do the append
fi
```
