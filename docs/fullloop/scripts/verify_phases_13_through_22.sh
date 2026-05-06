#!/usr/bin/env bash
#
# verify_phases_13_through_22.sh
#
# Master verification script for the full forward plan (docs/fullloop/15_AI_VERIFIABLE_END_STATES.md).
# Exits 0 only when every phase's AI-verifiable end-state passes deterministically.
# Writes a consolidated JSON report to /tmp/fullloop-verify-<unix>.json.
#
# Phases that depend on durable production state (Phases 14, 17, 22 with real
# correlation reports, etc.) check whichever subset of artifacts already exists
# and report `skipped` for the rest. The master verifier never lies about state
# the loop has not yet produced.
#
# Usage:
#   bash docs/fullloop/scripts/verify_phases_13_through_22.sh [--strict]
#
# --strict  fail on any `skipped` phase (use after Phase 22 lands).
#
# Source of truth: this script is read-only with respect to the ContextGraph
# workspace except for /tmp/fullloop-verify-<unix>.json.

set -uo pipefail

STRICT=0
[ "${1:-}" = "--strict" ] && STRICT=1

ROOT="${CONTEXTGRAPH_ROOT:-$HOME/contextgraph}"
cd "$ROOT" || { echo "verify: cannot cd to $ROOT" >&2; exit 1; }

UNIX="$(date +%s)"
REPORT="/tmp/fullloop-verify-${UNIX}.json"
PASS=0
FAIL=0
SKIP=0
RESULTS=()

# ----- helpers -----

record() {
  local phase="$1" outcome="$2" detail="$3"
  RESULTS+=("$(jq -n --arg phase "$phase" --arg outcome "$outcome" --arg detail "$detail" \
    '{phase:$phase,outcome:$outcome,detail:$detail}')")
  case "$outcome" in
    pass) PASS=$((PASS+1)) ;;
    fail) FAIL=$((FAIL+1)) ;;
    skip) SKIP=$((SKIP+1)) ;;
  esac
  printf '  [%s] %s — %s\n' "$outcome" "$phase" "$detail"
}

active_runtime() {
  cat "$HOME/.cache/cgreality/active_runtime_root" 2>/dev/null | tr -d '[:space:]'
}

# ----- inviolable checks (run first) -----

echo "== inviolable checks =="
if jq -e '.deny | index(".env") and index("swebench/**") and index("tests/oracle_**")' \
     config/ccreality_governed_paths.json >/dev/null; then
  record "inviolable_deny_list" pass "deny list contains .env, swebench/**, tests/oracle_**"
else
  record "inviolable_deny_list" fail "deny list missing required entries"
fi

if grep -Fq 'ccreality — Outer Reality-Loop Optimizer Role' CLAUDE.md; then
  record "inviolable_claude_md_addendum" pass "CLAUDE.md retains the ccreality role addendum"
else
  record "inviolable_claude_md_addendum" fail "CLAUDE.md missing the ccreality role addendum"
fi

if git diff --quiet -- gemmafrontend/reward_spec.verified_outcome.json 2>/dev/null; then
  record "inviolable_production_reward_spec_untouched" pass "production reward spec unchanged on disk"
else
  record "inviolable_production_reward_spec_untouched" fail "production reward spec has uncommitted changes"
fi

# ----- phase 13 — hardening -----

echo "== phase 13 =="
if cargo test -p context-graph-mcp --lib --quiet \
     reality_loop::helpers::tests::required_u64_accepts_stringified_integer 2>/dev/null \
   && cargo test -p context-graph-mcp --lib --quiet \
        reality_loop::helpers::tests::coerce_stringified_args_round_trips_typed_recommendation 2>/dev/null; then
  record "phase_13_T13_2_arg_coercion" pass "stringified-integer + typed-arg coercion tests pass"
else
  record "phase_13_T13_2_arg_coercion" fail "coercion unit tests did not pass"
fi

if [ -x scripts/cc/tests/hooks/test_session_start.sh ]; then
  if bash scripts/cc/tests/hooks/test_session_start.sh >/dev/null 2>&1; then
    record "phase_13_T13_1_session_start_hook" pass "hook test passes"
  else
    record "phase_13_T13_1_session_start_hook" fail "hook test failed"
  fi
else
  record "phase_13_T13_1_session_start_hook" skip "scripts/cc/tests/hooks/test_session_start.sh not yet authored"
fi

if [ -x scripts/cc/tests/smoke.sh ]; then
  # The cutover smoke harness requires ripgrep + a few other env tools; if any
  # are missing, this is an operator environment problem, not a regression in
  # the Phase 13 hardening work. Skip rather than fail so the verifier remains
  # honest about what it actually proved.
  if ! command -v rg >/dev/null 2>&1; then
    record "phase_13_smoke_full_stack" skip "smoke harness needs ripgrep (rg) — install rg to enable this check"
  elif bash scripts/cc/tests/smoke.sh >/dev/null 2>&1; then
    record "phase_13_smoke_full_stack" pass "deterministic smoke passed"
  else
    record "phase_13_smoke_full_stack" fail "smoke harness failed"
  fi
else
  record "phase_13_smoke_full_stack" skip "smoke harness not present"
fi

# ----- phase 14 — inner capability (data-dependent) -----

echo "== phase 14 =="
RUNTIME="$(active_runtime || true)"
if [ -n "$RUNTIME" ] && [ -d "$RUNTIME" ]; then
  RESOLVED="$(find "$RUNTIME" -name 'official-success-evidence.json' -type f 2>/dev/null | head -n 1 || true)"
  if [ -n "$RESOLVED" ] && jq -e '.resolved | length > 0' "$RESOLVED" >/dev/null 2>&1; then
    record "phase_14_first_resolved_ids" pass "official success evidence found at $RESOLVED"
  else
    record "phase_14_first_resolved_ids" skip "no official_resolved evidence yet under $RUNTIME"
  fi
else
  record "phase_14_first_resolved_ids" skip "no active runtime root with attempt artifacts"
fi

# ----- phase 15 — autoresearch engine -----

echo "== phase 15 =="
if [ -n "$RUNTIME" ]; then
  AUTORES_DIR="$(find "$RUNTIME" -maxdepth 3 -type d -name reality-optimizer 2>/dev/null | head -n 1 || true)"
  if [ -n "$AUTORES_DIR" ] \
     && [ -s "$AUTORES_DIR/experiment-registry.json" ] \
     && [ -s "$AUTORES_DIR/champion-state.json" ] \
     && [ -s "$AUTORES_DIR/attempts.jsonl" ]; then
    record "phase_15_autoresearch_artifacts" pass "registry+champion+attempts all present at $AUTORES_DIR"
  elif [ -n "$AUTORES_DIR" ]; then
    record "phase_15_autoresearch_artifacts" skip "reality-optimizer dir partial at $AUTORES_DIR"
  else
    record "phase_15_autoresearch_artifacts" skip "reality-optimizer dir not yet created"
  fi
else
  record "phase_15_autoresearch_artifacts" skip "no active runtime"
fi

# ----- phase 16 — cross-model -----

echo "== phase 16 =="
if [ -n "$RUNTIME" ]; then
  CROSS="$(find "$RUNTIME" -name 'cross-model-summary.json' -type f 2>/dev/null | head -n 1 || true)"
  if [ -n "$CROSS" ] && jq -e '. | type == "object"' "$CROSS" >/dev/null 2>&1; then
    record "phase_16_cross_model_summary" pass "cross-model summary present at $CROSS"
  else
    record "phase_16_cross_model_summary" skip "no cross-model summary yet"
  fi
else
  record "phase_16_cross_model_summary" skip "no active runtime"
fi

# ----- phase 17 — reward shadow -----

echo "== phase 17 =="
if [ -n "$RUNTIME" ] && [ -d "$RUNTIME/reality-optimizer/reward-shadow" ]; then
  COUNT="$(find "$RUNTIME/reality-optimizer/reward-shadow/correlation" -name '*-correlation-report.json' 2>/dev/null | wc -l)"
  if [ "$COUNT" -ge 1 ]; then
    record "phase_17_reward_shadow_correlation" pass "$COUNT correlation report(s) on disk"
  else
    record "phase_17_reward_shadow_correlation" skip "no shadow-spec correlation reports yet"
  fi
else
  record "phase_17_reward_shadow_correlation" skip "no reward-shadow dir"
fi

# ----- phase 18 — training export -----

echo "== phase 18 =="
if [ -n "$RUNTIME" ]; then
  TRAIN_DIR="$(find "$RUNTIME" -maxdepth 3 -type d -name training 2>/dev/null | head -n 1 || true)"
  if [ -n "$TRAIN_DIR" ] && [ -s "$TRAIN_DIR/manifest.json" ]; then
    if [ -s "$TRAIN_DIR/positive.jsonl" ] || [ -s "$TRAIN_DIR/negative.jsonl" ] || [ -s "$TRAIN_DIR/rejected_patches.jsonl" ]; then
      record "phase_18_training_export" pass "training manifest + at least one class present at $TRAIN_DIR"
    else
      record "phase_18_training_export" skip "training dir present but no class samples yet"
    fi
  else
    record "phase_18_training_export" skip "training dir not yet created"
  fi
else
  record "phase_18_training_export" skip "no active runtime"
fi

# ----- phase 19 — curriculum -----

echo "== phase 19 =="
if jq -e '.splits.dev and .splits.holdout and .splits.promotion' \
     gemmafrontend/swe_smoke_benchmark_suite.json >/dev/null 2>&1; then
  record "phase_19_curriculum_splits" pass "dev/holdout/promotion splits declared"
else
  record "phase_19_curriculum_splits" skip "smoke suite does not yet declare 3 splits"
fi

# ----- phase 20 — dashboard -----

echo "== phase 20 =="
if [ -n "$RUNTIME" ] && [ -d "$RUNTIME/reality-scoreboard" ]; then
  if [ -s "$RUNTIME/reality-scoreboard/attempt-scoreboard.jsonl" ] \
     && [ -s "$RUNTIME/reality-scoreboard/reality-scoreboard.md" ]; then
    record "phase_20_scoreboard_artifacts" pass "attempt scoreboard + reality-scoreboard.md present"
  else
    record "phase_20_scoreboard_artifacts" skip "reality-scoreboard partial"
  fi
else
  record "phase_20_scoreboard_artifacts" skip "no reality-scoreboard dir"
fi

# ----- phase 21 — production ops -----

echo "== phase 21 =="
if [ -x scripts/cc/cgreality-tick.sh ] && bash -n scripts/cc/cgreality-tick.sh; then
  record "phase_21_external_watcher" pass "watcher present, executable, syntax-valid"
else
  record "phase_21_external_watcher" fail "scripts/cc/cgreality-tick.sh missing/invalid"
fi

# ----- phase 22 — DynamicJEPA reward -----

echo "== phase 22 =="
DJ_FOUND=0
if [ -n "$RUNTIME" ]; then
  if find "$RUNTIME" -name 'reality-signal-packet.json' -type f -exec \
       jq -e '.dynamicjepa_reality.panel_count // empty' {} \; 2>/dev/null | grep -q .; then
    DJ_FOUND=1
  fi
fi
if [ "$DJ_FOUND" -eq 1 ]; then
  record "phase_22_dynamicjepa_reality_block" pass "dynamicjepa_reality.panel_count present in some signal packet"
else
  record "phase_22_dynamicjepa_reality_block" skip "no reality-signal-packet has dynamicjepa_reality yet"
fi

# ----- final report -----

echo "== summary =="
echo "PASS=$PASS FAIL=$FAIL SKIP=$SKIP"

REPORT_BODY="$(printf '%s\n' "${RESULTS[@]}" | jq -s --arg pass "$PASS" --arg fail "$FAIL" --arg skip "$SKIP" --arg unix "$UNIX" --arg root "$ROOT" --arg runtime "${RUNTIME:-}" '{schema_version:1,record_kind:"fullloop_verify_report",created_at_unix:($unix|tonumber),contextgraph_root:$root,active_runtime_root:$runtime,counts:{pass:($pass|tonumber),fail:($fail|tonumber),skip:($skip|tonumber)},results:.}')"
echo "$REPORT_BODY" > "$REPORT"
echo "report: $REPORT"

if [ "$FAIL" -gt 0 ]; then exit 1; fi
if [ "$STRICT" -eq 1 ] && [ "$SKIP" -gt 0 ]; then exit 2; fi
exit 0
