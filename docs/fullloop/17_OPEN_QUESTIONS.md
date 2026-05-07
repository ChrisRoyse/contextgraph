# Open Questions

Items that need a decision before or during the relevant forward phase. None block phase 13–14; some are needed for phase 17+.

## OQ-1 — Single runtime root vs per-(task, model) root

**Today:** one `active_runtime_root` per operator session. Multiple (task, model) attempts share that root.

**Question:** should we instead have one root per (task), with cross-task aggregation in a higher-level "experiment" root?

**Trade-off:**
- Single root: simpler, scoreboard naturally aggregates across tasks, fewer pointer files. Risk: long-running root accumulates artifacts.
- Per-task root: tidier on disk, easier to nuke a single task's history. Risk: cross-task analytics need a join layer.

**Decision deadline:** before Phase 15 (autoresearch engine landing — registry shape depends on root layout).

**Default:** keep single root unless dashboard performance degrades.

## OQ-2 — Where does shadow reward correlation methodology live

**Question:** Pearson + Spearman are baseline. Do we want bootstrap CI? Do we want per-task vs aggregate correlation? Do we want non-parametric tests for label-imbalanced runs (early on, very few resolved samples)?

**Decision deadline:** Phase 17.

**Default:** start with Pearson + Spearman across all attempts; add bootstrap CI when sample size > 100 across both classes.

## OQ-3 — Cross-model preference pairs

**Question:** if Qwen resolves and the inner actor doesn't, is Qwen's patch a "preferred" sample for the inner actor's training set?

**Trade-off:**
- Yes: fastest signal for the inner actor to learn what worked.
- No: cross-model patches may use idioms the inner actor can't produce at its capacity.

**Decision deadline:** Phase 18.

**Default:** No. Same-model preference pairs only. Cross-model facts feed prompts (Phase 16), not training samples.

## OQ-4 — Holdout split frequency

**Question:** weekly (default) or after every N optimizer turns?

**Trade-off:**
- Weekly: predictable; minimal Docker churn.
- Per-N-turns: tight feedback loop; high Docker cost.

**Decision deadline:** Phase 19.

**Default:** weekly + on-demand `mcp__cgreality__curriculum_run_holdout` for the operator.

## OQ-5 — DynamicJEPA training cadence

**Question:** does the reality loop trigger DJ training, or is training operator-initiated?

**Trade-off:**
- Auto-trigger: keeps DJ panels fresh; risks runaway training cost.
- Operator-initiated: bounded; risks DJ becoming stale.

**Decision deadline:** Phase 22.

**Default:** operator-initiated with cron suggestions emitted by the dashboard.

## OQ-6 — Multi-GPU parallel inner-loop

**Question:** when does cross-model parallelism become worth the engineering cost?

**Trade-off:**
- 3 sequential models × 5-min each = 15 min/attempt.
- 3 parallel = 5 min/attempt (best case) but requires multi-GPU isolation, separate `nvidia-smi` budgeting, plus harness reentrance review.

**Decision deadline:** post-Phase 16 once the cross-model serial path is producing data.

**Default:** sequential for the first 1,000 attempts; parallelize only when sequential queue saturates.

## OQ-7 — External-watcher hosting

**Question:** local systemd vs Anthropic-side scheduled agents (claude-cli `/schedule`) vs cloud (Vercel cron, GitHub Actions)?

**Trade-off:**
- Local systemd: full control, no third-party dependency, the watcher sees the runtime root directly.
- Anthropic-side: turn-key but currently requires the runtime root to be reachable.
- Cloud: most reliable but introduces an external IP/network surface.

**Decision deadline:** Phase 21.

**Default:** local systemd. Templates committed; operator copies to /etc/systemd/.

## OQ-8 — Recommendation cost-estimate accuracy

**Question:** the model fills `cost_estimate_usd` based on its self-knowledge. Should we override with a real cost calculation from `claude --print` exit metadata?

**Trade-off:**
- Self-reported: low effort, "good enough" for rate limiting.
- Real cost: precise but requires plumbing claude-cli output into the recommendation tool.

**Decision deadline:** Phase 13 T13.4.

**Default:** self-reported. Add real-cost overlay if rate-limit false-positives become a problem.

## OQ-9 — Champion promotion human-review medium

**Question:** is human review of `promote-recommendation.md` an email approval, a PR review, a `/ccreality-promote` skill that locks until approved?

**Decision deadline:** Phase 19.

**Default:** dashboard shows "PROMOTION PENDING" panel; operator runs `/ccreality-promote <experiment-id>` after manual review; that skill writes a signed approval JSON next to the candidate spec.

## OQ-10 — When to seed Phase 14 with operator-curated easy instances

**Question:** if `psf__requests-2317` and `django__django-12908` both resist resolution after 50 turns, which "easy" SWE-bench Lite instances do we add to dev?

**Decision deadline:** Phase 14 (operator picks if Layer 5 fails).

**Default:** operator manually inspects SWE-bench Lite leaderboards for instances solved by smaller models in published baselines, picks 2.

## How to track decisions

Each OQ above is closed by a Markdown row in this file when decided:

```
**OQ-N — DECIDED 2026-MM-DD: <brief decision>. Source of truth: <PR or recommendation>.**
```

The MCP server has no special handling for these; they are documentation-only.
