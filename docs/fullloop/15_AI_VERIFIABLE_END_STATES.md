# AI-Verifiable End-States — One Command Per Phase

Each forward phase is "done" only when its end-state command exits 0 deterministically. These commands are the contract between this plan and any AI agent picking it up. None require human inspection of free-text output to decide pass/fail.

## Phase 13 — Hardening

```bash
bash scripts/cc/tests/smoke.sh \
  && cargo test -p context-graph-mcp --lib reality_loop --quiet \
  && bash scripts/cc/tests/hooks/test_session_start.sh
```

All three exit 0. The smoke report JSON shows `drift_detection: PASS`.

## Phase 14 — Inner Capability

```bash
RUN=<run_id>; ATT=<attempt>
INSTANCE=$(jq -r '.run_id' <(mcp__cgreality__reality_latest_root))
mcp__cgreality__reality_official_report run_id=$RUN attempt=$ATT \
  | jq -e '.resolved_ids | index(env.TARGET_INSTANCE) != null'
```

Plus `docs/fullloop/PHASE14_RESULTS_<date>.md` exists, schema-valid, and cites at least one recommendation_path that contributed to the resolution.

## Phase 15 — Autoresearch Engine

```bash
test -s <run_root>/reality-optimizer/experiment-registry.json \
  && test -s <run_root>/reality-optimizer/champion-state.json \
  && [[ $(wc -l < <run_root>/reality-optimizer/attempts.jsonl) -ge 5 ]] \
  && mcp__cgreality__experiment_registry_list status_filter=pending \
  && mcp__cgreality__champion_state_get
```

## Phase 16 — Cross-Model

```bash
target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --model gemma4:e4b,qwen3:8b,mistral-nemo:12b \
  --official-eval-each-attempt --max-attempts 1 \
  --runtime-root /tmp/ccreality-cross-model-test

for M in gemma4-e4b qwen3-8b mistral-nemo-12b; do
  test -s "/tmp/ccreality-cross-model-test/<task>/$M/attempts/attempt-000001/attempt-summary.json" || exit 1
done
test -s "/tmp/ccreality-cross-model-test/<task>/cross-model-summary.json"
mcp__cgreality__cross_model_failures_for_task instance_id=psf__requests-2317
```

## Phase 17 — Reward Shadow

```bash
SPEC_ID=<spec-id>
mcp__cgreality__reward_shadow_propose spec='{...}' proposal_markdown='...'
mcp__cgreality__reward_shadow_recompile_all spec_id=$SPEC_ID
mcp__cgreality__reward_shadow_correlation_report spec_id=$SPEC_ID
test -s <run_root>/reality-optimizer/reward-shadow/correlation/$SPEC_ID-correlation-report.json
git diff --quiet -- gemmafrontend/reward_spec.verified_outcome.json
```

The last `git diff --quiet` MUST exit 0 (production reward spec untouched).

## Phase 18 — Training Export

```bash
test -s <run_root>/training/positive.jsonl \
  && test -s <run_root>/training/negative.jsonl \
  && test -s <run_root>/training/preference_pairs.jsonl \
  && test -s <run_root>/training/manifest.json \
  && mcp__cgreality__training_export_summary | jq -e '.positive_count >= 1 and .negative_count >= 1 and .preference_pair_count >= 1'
```

## Phase 19 — Curriculum

```bash
mcp__cgreality__curriculum_split_info | jq -e '.splits | keys | length == 3'

target/release/reality-loop attempt --split holdout \
  --task-manifest tasks/swebench-lite/<holdout-instance>.json --max-attempts 1 \
  --runtime-root /tmp/ccreality-curriculum-holdout

# attempt to fire optimizer turn → should be denied
mcp__cgreality__optimizer_record_decision \
  run_id=$(jq -r .run_id ...) attempt=1 \
  policy=failure should_run=true split=holdout 2>&1 \
  | grep -q CCREALITY_SPLIT_FORBIDS_OPTIMIZER
```

## Phase 20 — Dashboard

```bash
test -s <run_root>/reality-scoreboard/attempt-scoreboard.jsonl \
  && test -s <run_root>/reality-scoreboard/model-task-scoreboard.json \
  && test -s <run_root>/reality-scoreboard/artifact-scoreboard.json \
  && test -s <run_root>/reality-scoreboard/reality-scoreboard.md \
  && curl -fsS http://localhost:8788/scoreboard/ > /dev/null \
  && curl -fsS http://localhost:8788/scoreboard/attempts.json | jq -e 'length >= 1' \
  && mcp__cgreality__scoreboard_summary | jq -e '.reality_score'
```

## Phase 21 — Production Ops

```bash
test -x scripts/cc/cgreality-tick.sh \
  && bash -n scripts/cc/cgreality-tick.sh \
  && target/release/context-graph-cli ccreality cost-summary --today --json | jq -e '.total_usd >= 0'

# simulate watcher tick without claude attached
SIMULATE_NO_CLAUDE=1 bash scripts/cc/cgreality-tick.sh
test $? -eq 0
```

## Phase 22 — DynamicJEPA Integration

```bash
mcp__cgreality__dynamicjepa_reality_for_attempt run_id=<r> attempt=<a> \
  | jq -e '.panel_count >= 1 and (.prediction_error_l2 | type) == "number"'

ls <run_root>/reality-optimizer/reward-shadow/candidate-specs/ \
  | xargs -I{} jq '.components[].id' <run_root>/reality-optimizer/reward-shadow/candidate-specs/{} \
  | grep -q '^"dj_'

ls <run_root>/reality-optimizer/reward-shadow/correlation/ \
  | xargs -I{} test -s <run_root>/reality-optimizer/reward-shadow/correlation/{}
```

## Master verification (entire forward plan complete)

```bash
bash docs/fullloop/scripts/verify_phases_13_through_22.sh
```

That script (to be authored as part of Phase 13) chains all of the above. It writes a single JSON report and exits 0 only when every phase passes. The report is the durable artifact that proves the forward plan is done.

## Inviolable principles (verified continuously, not phase-gated)

```bash
# governed-paths deny list intact
jq -e '.deny | index(".env") and index("swebench/**") and index("tests/oracle_**")' config/ccreality_governed_paths.json

# CLAUDE.md still has the ccreality role addendum
rg -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md

# success predicate unchanged across all measurement contracts
find /tmp/cgreality* -name measurement-contract.json | xargs jq -e '.claim_scope.success_predicate == "official_report.resolved_ids contains task.instance_id"'
```

These should pass at all times, regardless of phase.
