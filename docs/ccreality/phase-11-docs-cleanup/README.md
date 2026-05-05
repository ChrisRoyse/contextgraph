# Phase 11 — Documentation Cleanup

**Estimated duration:** 15 minutes.
**Status:** pending.
**Depends on:** Phase 10.
**Unblocks:** Phase 12.

## What this phase does

Removes superseded docs and updates the index pointers.

Deletions:
- `docs/reality/20_openai_outer_optimizer.md` — superseded by ccreality.
- `docs2/DEEPSEEK_API_GUIDE.md` — no longer relevant.
- `docs2/.raw/*.md` files (already shown deleted in working tree per `git status`).

Updates:
- `docs/reality/00_index.md` — remove entries 20 and 21; add a one-line pointer to `docs/ccreality/`.
- `docs/reality/21_grand_scale_reality_autoresearch.md` — keep, but prepend a "Superseded for the model swap; architectural framing still valid; see docs/ccreality/" header.
- `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` — prepend a "Superseded by docs/ccreality/" header; keep for historical context.

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph
test ! -f docs/reality/20_openai_outer_optimizer.md
test ! -f docs2/DEEPSEEK_API_GUIDE.md
grep -q "ccreality" docs/reality/00_index.md
head -5 docs/reality/21_grand_scale_reality_autoresearch.md | grep -qi "supers"
head -5 docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md | grep -qi "supers"
```

All five must succeed.

## Files

- [00_SPEC.md](00_SPEC.md), [02_TASKS.md](02_TASKS.md), [04_VERIFICATION.md](04_VERIFICATION.md), [06_HANDOFF.md](06_HANDOFF.md).
