# Phase 19 — Curriculum: Dev / Holdout / Promotion Splits

**Goal:** stop reasoning about a single tiny smoke suite. Add curriculum splits so champions are only promoted when they generalize.
**Effort:** 1 day.
**Risk:** low — config + engine flags.

## Today

`gemmafrontend/swe_smoke_benchmark_suite.json` declares two SWE-bench Lite instances and `expand_only_after_manual_review: true`. There is no holdout, no promotion gate, no expansion mechanism beyond operator instruction.

## Three splits

```
suite/
  dev/         ← optimization runs against this; the loop sees these tasks
  holdout/     ← scheduled runs only; never used by the optimizer's diagnoses
  promotion/   ← run only when champion ready; no optimizer turns allowed
```

### Dev split (≤ 5 instances)

The loop optimizes against this. Every attempt feeds the optimizer's diagnosis and harness edits. Initial dev split: `psf__requests-2317`, `django__django-12908`, plus 1–2 operator-curated easy instances.

### Holdout split (≥ 10 instances)

Run on a fixed cron schedule (weekly). The optimizer sees the *outcomes* (resolved counts, reward distribution) but the holdout attempts produce no harness changes — they are scoring runs only. The reality engine refuses to run optimizer turns whose target attempt is in the holdout split.

### Promotion split (≥ 30 instances)

Run only when an experiment from `experiment-registry.json` reaches `kept` status on dev. Promotion split is the gate before declaring a harness change is genuinely better.

## Suite file changes

`gemmafrontend/swe_smoke_benchmark_suite.json` extended:

```json
{
  "suite_id": "contextgraph_swe_lite_v2",
  "dataset": "princeton-nlp/SWE-bench_Lite",
  "split": "test",
  "limits": { ... },
  "splits": {
    "dev": {
      "instances": [
        {"instance_id": "psf__requests-2317", "tier": "smoke"},
        {"instance_id": "django__django-12908", "tier": "smoke"},
        {"instance_id": "<operator_easy_1>", "tier": "easy"},
        {"instance_id": "<operator_easy_2>", "tier": "easy"}
      ]
    },
    "holdout": { "instances": [ ... 10+ ... ] },
    "promotion": { "instances": [ ... 30+ ... ] }
  },
  "promotion_rules": { ... },
  "expand_only_after_manual_review": true
}
```

## Engine changes

`reality-loop attempt` adds `--split <dev|holdout|promotion>` (default `dev`).
- When `--split holdout`, the engine refuses to honor any `optimizer_record_decision` calls for attempts in this run. The MCP server's decision tool returns `CCREALITY_SPLIT_FORBIDS_OPTIMIZER` when called against a holdout-split attempt.
- When `--split promotion`, the engine adds an extra readback comparing the promotion result to the dev/holdout baseline reward; the result determines `champion-state.json` promotion.

## Promotion rule (engine-enforced)

A candidate champion `(model, task, harness-state)` is promoted iff:

1. Its dev attempt resolves (`official_resolved: true`).
2. Holdout split run after the dev resolution has equal or better aggregate `reality_score` (from Phase 20) compared to the *previous* champion.
3. Promotion split run resolves all instances the previous champion resolved (no regression).
4. Source-of-truth coverage is 100% (every attempt has official Docker readback artifact).
5. No `incident_packet` flagged as "reward_hacking_suspected" exists for any sample.

If all five hold, the engine writes `champion-state.json` with `promoted_at_unix` set; otherwise stays `candidate`.

## New MCP tools

| Tool | Purpose |
|------|---------|
| `curriculum_split_info` | list instances per split with current pass status |
| `curriculum_run_holdout` | trigger a holdout run for the current harness state |
| `curriculum_run_promotion` | trigger a promotion run (operator-gated; refuses if no candidate champion exists) |

## /loop body changes

`.claude/loop.md` step 4 (Decide):

> If the trigger is for a holdout-split or promotion-split attempt, the optimizer turn is read-only by policy. Do NOT propose a harness edit. Record `no_change` with reason citing the split policy.

## AI-verifiable end-state for Phase 19

```bash
target/debug/reality-loop attempt --split holdout --task-manifest <task> ...

mcp__cgreality__curriculum_split_info
# expected: response includes counts per split

# attempt the optimizer trigger on a holdout attempt:
mcp__cgreality__optimizer_record_decision attempt=<n> ... split=holdout ...
# expected: error CCREALITY_SPLIT_FORBIDS_OPTIMIZER
```

## Out of scope

- Adaptive curriculum (auto-promote tasks from holdout to dev based on solve rate). Operator-curated for now.
- Cross-suite generalization (other benchmarks beyond SWE-bench Lite).
