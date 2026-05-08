# Phase 8 — Tasks

## Task 8.1 — Confirm CLAUDE.md exists and current state

```bash
cd /home/cabdru/contextgraph
test -f CLAUDE.md
wc -l CLAUDE.md
tail -20 CLAUDE.md
```

Expected: file exists; tail does not already contain `## ccreality —`.

If the addendum is already present, STOP — Phase 8 is already done.

## Task 8.2 — Append the addendum block

Open [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md), find the section "## Append-only block (drop in verbatim)", and append the block from the first `---` through the closing `---` to `/home/cabdru/contextgraph/CLAUDE.md`.

Verify:
```bash
grep -q "ccreality — Outer Reality-Loop Optimizer Role" /home/cabdru/contextgraph/CLAUDE.md
echo "exit=$?"
```
Expected: `exit=0`.

Verify the root project memory file is not ignored:

```bash
! git check-ignore -q CLAUDE.md
git check-ignore -q CLAUDE.local.md
```

Expected: root `CLAUDE.md` is trackable, while local/private overrides stay ignored.

Also verify the appended block matches the source block:

```bash
SRC_START=$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | head -1 | cut -d: -f1)
SRC_END=$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | tail -1 | cut -d: -f1)
sed -n "${SRC_START},${SRC_END}p" docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md > /tmp/phase8-source-addendum.md
CLAUDE_START=$(grep -n "^## ccreality — Outer Reality-Loop Optimizer Role$" CLAUDE.md | cut -d: -f1)
CLAUDE_START=$((CLAUDE_START-2))
tail -n +"$CLAUDE_START" CLAUDE.md > /tmp/phase8-claude-addendum.md
diff -u /tmp/phase8-source-addendum.md /tmp/phase8-claude-addendum.md
```

## Task 8.3 — Verify

Run [04_VERIFICATION.md](04_VERIFICATION.md). All three clauses pass.
