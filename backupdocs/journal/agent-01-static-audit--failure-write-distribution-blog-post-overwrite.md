---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:22:00Z
status: active
severity: medium
tags: exp2, write-distribution, c18-violation, file-placement, docs
---

# Defect: write_distribution writes blog_post.md, youtube_script.md, tweet_thread.md to docs/exp2/ which overrides distribution-plan content

## Where
`scripts/exp2/write_distribution.py:41-52`

## Issue 1: Files written to docs/exp2/ from a script
C18 (`docs/exp2/01_constitution.md`):
> EXP-2 artefacts go under `scripts/exp2/` for code, `docs/exp2/` for docs, `artefacts/exp2/` for output bundles.

The script writes:
- `docs/exp2/youtube_script.md`
- `docs/exp2/blog_post.md`
- `docs/exp2/tweet_thread.md`

These are GENERATED outputs — not docs. Per C18 they should go under `artefacts/exp2/`. Putting them under `docs/exp2/` mixes hand-authored docs with scripted output. If `docs/exp2/blog_post.md` already exists (per `09_distribution_plan.md` §D), it gets overwritten.

## Issue 2: Each file is one line, not a real draft
```python
(repo_path("docs/exp2/blog_post.md")).write_text(
    f"# We Measured How Much Information a 14-Embedder Memory System Actually Multiplies\n\nEXP-2 measured N_eff = {n_eff} (95% CI {ci}).\n",
    ...
)
```

`08_output_deliverables.md` §12: "1500-2500 words" for the blog. The script emits a single sentence. T-032 DoD says "1500-2500 words". The script silently doesn't satisfy DoD; running it and committing the result would produce a fake-looking placeholder doc.

## Why it matters
- C18 violation: scripted content written into docs/.
- Quality: the generated content is plausibly meant as a placeholder/scaffold but is not flagged as such; an operator running the script and seeing a blog_post.md might think the task is done.

## Recommended fix
1. Move outputs to `artefacts/exp2/`:
   - `artefacts/exp2/PAPER_PATCH.md` (already correct)
   - `artefacts/exp2/youtube_script.md`
   - `artefacts/exp2/blog_post_draft.md`
   - `artefacts/exp2/tweet_thread.md`
2. Mark each as "draft scaffold" with a checkbox list of what the operator must fill in.
3. OR: don't generate these at all from this module; let the operator hand-author them.

## Cross-check
Trivial: `python3 scripts/exp2/write_distribution.py --config scripts/exp2/exp2.yml` and check file contents and locations.
