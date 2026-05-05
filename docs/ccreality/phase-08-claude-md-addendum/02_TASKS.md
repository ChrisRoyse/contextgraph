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

Open [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md), find the section "## Append-only block (drop in verbatim)". Copy the block between the triple-backticks (the entire markdown block starting with `---` and ending with `---`) into a temp file:

```bash
cat > /tmp/ccreality-addendum.md <<'EOF'
<paste the verbatim block here>
EOF
```

Then append:
```bash
cat /tmp/ccreality-addendum.md >> /home/cabdru/contextgraph/CLAUDE.md
rm /tmp/ccreality-addendum.md
```

Verify:
```bash
grep -q "ccreality — Outer Reality-Loop Optimizer Role" /home/cabdru/contextgraph/CLAUDE.md
echo "exit=$?"
```
Expected: `exit=0`.

## Task 8.3 — Verify

Run [04_VERIFICATION.md](04_VERIFICATION.md). All three clauses pass.
