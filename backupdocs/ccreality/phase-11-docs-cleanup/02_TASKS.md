# Phase 11 — Tasks

## Task 11.1 — Delete superseded docs

```bash
cd /home/cabdru/contextgraph
git rm docs/reality/20_openai_outer_optimizer.md
git rm docs2/DEEPSEEK_API_GUIDE.md
# The docs2/.raw/*.md files are already shown deleted in git status; just stage:
git add -u docs2/.raw/
```

## Task 11.2 — Add "Superseded" header to `21_grand_scale_reality_autoresearch.md`

Prepend at the top of `docs/reality/21_grand_scale_reality_autoresearch.md`:

```markdown
> **Status (2026-05-05):** Architectural framing still valid; the model swap (DeepSeek → Claude Code via MCP) is described in [docs/ccreality/](../../docs/ccreality/00_INDEX.md). This doc is retained for the broader Karpathy-AutoResearch context.

```

(Use Read first, then Edit with old_string being the current first line.)

## Task 11.3 — Add "Superseded" header to `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`

Prepend at the top:

```markdown
> **Status (2026-05-05):** Superseded by [docs/ccreality/](../docs/ccreality/00_INDEX.md). Kept for historical context — this draft proposed `claude -p` shelling; the final architecture uses MCP-driven integration instead. See `docs/ccreality/01_ARCHITECTURE.md` for the rationale.

```

## Task 11.4 — Update `docs/reality/00_index.md`

Read current content. Find the entries for `20_openai_outer_optimizer.md` (line probably exists) and remove. The entry for `21_grand_scale_reality_autoresearch.md` stays.

Add at the top (or in an appropriate place) a pointer:

```markdown
> **For the ccreality (Claude Code outer optimizer) docs**, see [`docs/ccreality/`](../ccreality/00_INDEX.md).
```

## Task 11.5 — Verify

[04_VERIFICATION.md](04_VERIFICATION.md). Five clauses must pass.
