#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  ./reproduce.sh verify-aggregate --aggregate PATH --reference reference/reference_release_v1.0.0.json [--json]
  ./reproduce.sh verify-aggregate --aggregate PATH --reference reference/reference_release_v2.0.0.json [--json]
  ./reproduce.sh paper_small_1k --seeds 42,43,44,45,46 --run-root PATH --reference reference/reference_release_v1.0.0.json --onet-text-zip PATH [--json]

This script uses real O*NET data and existing DynamicJEPA commands. It never
creates mock data and never overwrites an existing bundle or aggregate root.
USAGE
}

json=false
mode="${1:-}"
if [[ -z "$mode" || "$mode" == "--help" || "$mode" == "-h" ]]; then
  usage
  exit 0
fi
shift || true

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"

context_graph_bin="${CONTEXT_GRAPH_BIN:-}"
if [[ -z "$context_graph_bin" ]]; then
  if [[ -x "$repo_root/target/debug/context-graph" ]]; then
    context_graph_bin="$repo_root/target/debug/context-graph"
  elif command -v context-graph >/dev/null 2>&1; then
    context_graph_bin="$(command -v context-graph)"
  else
    echo "REPRODUCE_CONTEXT_GRAPH_BIN_MISSING: set CONTEXT_GRAPH_BIN or build target/debug/context-graph" >&2
    exit 1
  fi
fi

aggregate=""
reference=""
seeds=""
run_root="${MEJEPA_REPRO_RUN_ROOT:-tmp/mejepa_reproduce_runs}"
onet_text_zip="${ONET_TEXT_ZIP:-tmp/onet_probe/db_30_2_text.zip}"
system_specs="${SYSTEM_SPECS:-docs2/systemspecs.md}"
fixtures_root="${FIXTURES_ROOT:-configs/dynamicjepa}"
run_id_prefix="${REPRODUCE_RUN_ID_PREFIX:-reproduce_$(date -u +%Y%m%dT%H%M%SZ)_$$}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --aggregate)
      aggregate="$2"
      shift 2
      ;;
    --reference)
      reference="$2"
      shift 2
      ;;
    --seeds)
      seeds="$2"
      shift 2
      ;;
    --run-root)
      run_root="$2"
      shift 2
      ;;
    --onet-text-zip)
      onet_text_zip="$2"
      shift 2
      ;;
    --system-specs)
      system_specs="$2"
      shift 2
      ;;
    --fixtures-root)
      fixtures_root="$2"
      shift 2
      ;;
    --run-id-prefix)
      run_id_prefix="$2"
      shift 2
      ;;
    --json)
      json=true
      shift
      ;;
    *)
      echo "REPRODUCE_UNKNOWN_ARG: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_file() {
  local path="$1"
  local code="$2"
  [[ -f "$path" ]] || {
    echo "$code: required file '$path' does not exist" >&2
    exit 1
  }
}

require_no_existing_path() {
  local path="$1"
  local code="$2"
  [[ ! -e "$path" ]] || {
    echo "$code: refusing to overwrite existing path '$path'" >&2
    exit 1
  }
}

case "$mode" in
  verify-aggregate)
    [[ -n "$aggregate" ]] || {
      echo "REPRODUCE_AGGREGATE_REQUIRED: --aggregate is required" >&2
      exit 1
    }
    [[ -n "$reference" ]] || {
      echo "REPRODUCE_REFERENCE_REQUIRED: --reference is required" >&2
      exit 1
    }
    "$context_graph_bin" dynamicjepa check-release-aggregate-reference \
      --aggregate "$aggregate" \
      --reference "$reference" \
      ${json:+--json}
    ;;
  paper_small_1k)
    [[ -n "$seeds" ]] || {
      echo "REPRODUCE_SEEDS_REQUIRED: --seeds is required" >&2
      exit 1
    }
    [[ -n "$reference" ]] || {
      echo "REPRODUCE_REFERENCE_REQUIRED: --reference is required" >&2
      exit 1
    }
    require_file "$onet_text_zip" "REPRODUCE_ONET_ZIP_MISSING"
    require_file "$system_specs" "REPRODUCE_SYSTEM_SPECS_MISSING"
    require_file "$reference" "REPRODUCE_REFERENCE_MISSING"
    mkdir -p "$run_root"
    IFS=',' read -r -a seed_array <<< "$seeds"
    bundle_args=()
    for seed in "${seed_array[@]}"; do
      run_id="${run_id_prefix}_paper_small_seed${seed}"
      bundle="$run_root/$run_id"
      require_no_existing_path "$bundle" "REPRODUCE_BUNDLE_EXISTS"
      CUDA_COMPUTE_CAP=120 "$context_graph_bin" dynamicjepa research-smoke \
        --run-id "$run_id" \
        --run-root "$run_root" \
        --fixtures-root "$fixtures_root" \
        --purpose paper_small \
        --career-event-count 1000 \
        --career-training-seed "$seed" \
        --onet-text-zip "$onet_text_zip" \
        --system-specs "$system_specs" \
        --json
      bundle_args+=(--bundle "$bundle")
    done
    aggregate="$run_root/${run_id_prefix}_paper_small_multiseed_aggregate"
    require_no_existing_path "$aggregate" "REPRODUCE_AGGREGATE_EXISTS"
    CUDA_COMPUTE_CAP=120 "$context_graph_bin" dynamicjepa multiseed-aggregate \
      "${bundle_args[@]}" \
      --output-root "$aggregate" \
      --expected-bundle-count "${#seed_array[@]}" \
      --min-passed "${#seed_array[@]}" \
      --expected-purpose paper_small \
      --expected-career-event-count 1000 \
      --required-career-training-seeds "$seeds" \
      --bootstrap-iters 10000 \
      --bootstrap-seed 20260430 \
      --json
    "$context_graph_bin" dynamicjepa check-release-aggregate-reference \
      --aggregate "$aggregate" \
      --reference "$reference" \
      ${json:+--json}
    ;;
  *)
    echo "REPRODUCE_UNKNOWN_MODE: $mode" >&2
    usage >&2
    exit 2
    ;;
esac
