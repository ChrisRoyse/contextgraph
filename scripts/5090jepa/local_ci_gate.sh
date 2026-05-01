#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/5090jepa/local_ci_gate.sh <preflight|static|smoke-a|smoke-b|release-c>

Runs the DynamicJEPA CI gate on the local machine or a self-hosted runner.
No GitHub-hosted runners, GitHub artifact uploads, remote caches, Codecov, or
paid CI services are used by this script. Evidence stays on local disk.

Environment:
  MEJEPA_CI_RUN_ROOT       default: tmp/mejepa_local_ci_runs
  ONET_TEXT_ZIP           default: tmp/onet_probe/db_30_2_text.zip
  LOCAL_CI_RUN_ID_PREFIX  default: local_<UTC timestamp>_<pid>
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

TIER="${1:-smoke-a}"
case "$TIER" in
  preflight|static|smoke-a|smoke-b|release-c) ;;
  *)
    echo "LOCAL_CI_INVALID_TIER: unsupported tier '$TIER'" >&2
    usage >&2
    exit 2
    ;;
esac

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

RUN_ROOT="${MEJEPA_CI_RUN_ROOT:-tmp/mejepa_local_ci_runs}"
ONET_TEXT_ZIP="${ONET_TEXT_ZIP:-tmp/onet_probe/db_30_2_text.zip}"
SYSTEM_SPECS="docs2/systemspecs.md"
RUN_ID_PREFIX="${LOCAL_CI_RUN_ID_PREFIX:-local_$(date -u +%Y%m%dT%H%M%SZ)_$$}"
MANIFEST_PATH="$RUN_ROOT/${RUN_ID_PREFIX}_local_ci_manifest.json"
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
STATUS="failed"
EVIDENCE_PATHS=()

log() {
  printf '[local-ci:%s] %s\n' "$TIER" "$*"
}

add_evidence() {
  EVIDENCE_PATHS+=("$1")
}

json_array_from_evidence() {
  if [[ "${#EVIDENCE_PATHS[@]}" -eq 0 ]]; then
    printf '[]'
  else
    printf '%s\n' "${EVIDENCE_PATHS[@]}" | jq -R . | jq -s .
  fi
}

write_manifest() {
  local exit_code="$1"
  mkdir -p "$RUN_ROOT"
  local finished_at
  finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local specs_sha
  if [[ -f "$SYSTEM_SPECS" ]]; then
    specs_sha="$(sha256sum "$SYSTEM_SPECS" | awk '{print $1}')"
  else
    specs_sha=""
  fi
  local evidence_json
  evidence_json="$(json_array_from_evidence)"
  jq -n \
    --arg operation "dynamicjepa_local_ci_gate" \
    --arg tier "$TIER" \
    --arg status "$STATUS" \
    --arg run_root "$RUN_ROOT" \
    --arg run_id_prefix "$RUN_ID_PREFIX" \
    --arg started_at "$STARTED_AT" \
    --arg finished_at "$finished_at" \
    --arg system_specs "$SYSTEM_SPECS" \
    --arg system_specs_sha256 "$specs_sha" \
    --argjson exit_code "$exit_code" \
    --argjson evidence_paths "$evidence_json" \
    '{
      operation: $operation,
      tier: $tier,
      status: $status,
      exit_code: $exit_code,
      run_root: $run_root,
      run_id_prefix: $run_id_prefix,
      started_at_utc: $started_at,
      finished_at_utc: $finished_at,
      system_specs: $system_specs,
      system_specs_sha256: $system_specs_sha256,
      evidence_paths: $evidence_paths
    }' > "$MANIFEST_PATH"
}

on_exit() {
  local code=$?
  if command -v jq >/dev/null 2>&1; then
    write_manifest "$code" || true
    if [[ -f "$MANIFEST_PATH" ]]; then
      log "manifest=$MANIFEST_PATH"
    fi
  fi
  exit "$code"
}
trap on_exit EXIT

require_command() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    echo "LOCAL_CI_MISSING_COMMAND: required command '$name' was not found" >&2
    exit 1
  }
}

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
  if [[ -e "$path" ]]; then
    echo "$code: refusing to overwrite existing path '$path'" >&2
    exit 1
  fi
}

precheck_tier_roots() {
  case "$TIER" in
    smoke-a)
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_smoke_a" "LOCAL_CI_SMOKE_A_ROOT_EXISTS"
      ;;
    smoke-b)
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_smoke_a" "LOCAL_CI_SMOKE_A_ROOT_EXISTS"
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_seed42_a" "LOCAL_CI_PAPER_SMALL_ROOT_EXISTS"
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_seed42_b" "LOCAL_CI_PAPER_SMALL_ROOT_EXISTS"
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_hp_sweep_batch_size" "LOCAL_CI_HP_SWEEP_ROOT_EXISTS"
      ;;
    release-c)
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_release_bundle_paths.txt" "LOCAL_CI_RELEASE_BUNDLE_LIST_EXISTS"
      for seed in 42 43 44 45 46; do
        require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_seed${seed}" "LOCAL_CI_RELEASE_SEED_ROOT_EXISTS"
      done
      require_no_existing_path "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_multiseed_aggregate" "LOCAL_CI_RELEASE_AGGREGATE_ROOT_EXISTS"
      ;;
    preflight|static)
      ;;
  esac
}

assert_free_workflows() {
  local matches
  matches="$(rg -n 'runs-on: ubuntu-latest|actions/upload-artifact|actions/cache|codecov|github-script|taiki-e/install-action|cargo-llvm-cov' .github/workflows || true)"
  if [[ -n "$matches" ]]; then
    echo "LOCAL_CI_PAID_SERVICE_REFERENCE: workflow files still reference hosted runners, paid storage/cache, or third-party reporting:" >&2
    echo "$matches" >&2
    exit 1
  fi
}

preflight_common() {
  log "preflight repo=$REPO_ROOT"
  require_command cargo
  require_command jq
  require_command rg
  require_command ruby
  require_file "$SYSTEM_SPECS" "LOCAL_CI_SYSTEM_SPECS_MISSING"
  assert_free_workflows
  mkdir -p "$RUN_ROOT"
  precheck_tier_roots
  log "source_of_truth.run_root=$RUN_ROOT"
  log "source_of_truth.manifest=$MANIFEST_PATH"
  log "system_specs_sha256=$(sha256sum "$SYSTEM_SPECS" | awk '{print $1}')"
}

preflight_cuda_and_data() {
  preflight_common
  require_command nvidia-smi
  require_command nvcc
  require_file "$ONET_TEXT_ZIP" "LOCAL_CI_ONET_ZIP_MISSING"
  require_file "reference/reference_smoke_hashes.json" "LOCAL_CI_REFERENCE_MISSING"
  require_file "reference/tolerances.json" "LOCAL_CI_TOLERANCES_MISSING"
  nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader
  nvcc --version | sed -n '1,4p'
}

static_gate() {
  preflight_common
  log "cargo fmt"
  cargo fmt --all -- --check
  log "cargo check"
  cargo check -p context-graph-cli --all-features
  log "clippy"
  cargo clippy -p context-graph-cli --all-targets --all-features -- -D warnings
  log "workflow yaml parse"
  ruby -e 'require "yaml"; ARGV.each { |path| YAML.load_file(path); puts "ok #{path}" }' .github/workflows/*.yml
}

build_dynamicjepa() {
  log "dynamicjepa FSV integration test"
  cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv --all-features -- --nocapture
  log "build context-graph CLI"
  cargo build -p context-graph-cli --bin context-graph --all-features
}

run_smoke_a_bundle() {
  local run_id="${RUN_ID_PREFIX}_smoke_a"
  local bundle="$RUN_ROOT/$run_id"
  require_no_existing_path "$bundle" "LOCAL_CI_SMOKE_A_ROOT_EXISTS"
  add_evidence "$bundle"
  log "run smoke-a bundle=$bundle"
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
    --run-id "$run_id" \
    --run-root "$RUN_ROOT" \
    --fixtures-root configs/dynamicjepa \
    --purpose smoke \
    --career-event-count 120 \
    --career-training-seed 42 \
    --career-ablation-training-seed 42 \
    --onet-text-zip "$ONET_TEXT_ZIP" \
    --system-specs "$SYSTEM_SPECS" \
    --json | tee "$RUN_ROOT/${run_id}_research_smoke.json"
  add_evidence "$RUN_ROOT/${run_id}_research_smoke.json"
  jq -e '.status == "passed"' "$bundle/run_manifest.json"
  jq -e '.status == "passed"' "$bundle/fsv_report.json"
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa inspect-counts \
    --db "$bundle/dbs/${run_id}_career_taxonomy_rocksdb" \
    --json | tee "$bundle/local_ci_inspect_counts_career_taxonomy.json"
  jq -e '.counts.dj_verification_runs >= 1 and .counts.dj_model_artifacts >= 1' \
    "$bundle/local_ci_inspect_counts_career_taxonomy.json"
  jq -e '[.[].artifact_hashes[] | select(.equal != true)] | length == 0' "$bundle/artifact_hashes.json"
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa check-reference \
    --bundle "$bundle" \
    --reference reference/reference_smoke_hashes.json \
    --tolerances reference/tolerances.json \
    --json | tee "$RUN_ROOT/${run_id}_reference_check.json"
  add_evidence "$RUN_ROOT/${run_id}_reference_check.json"
}

run_paper_seed42_twice() {
  require_file "reference/reference_paper_small_v1.json" "LOCAL_CI_REFERENCE_MISSING"
  local run_a="${RUN_ID_PREFIX}_paper_small_seed42_a"
  local run_b="${RUN_ID_PREFIX}_paper_small_seed42_b"
  for run_id in "$run_a" "$run_b"; do
    require_no_existing_path "$RUN_ROOT/$run_id" "LOCAL_CI_PAPER_SMALL_ROOT_EXISTS"
  done
  for run_id in "$run_a" "$run_b"; do
    local bundle="$RUN_ROOT/$run_id"
    add_evidence "$bundle"
    log "run paper-small seed42 bundle=$bundle"
    CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
      --run-id "$run_id" \
      --run-root "$RUN_ROOT" \
      --fixtures-root configs/dynamicjepa \
      --purpose paper_small \
      --career-event-count 1000 \
      --career-training-seed 42 \
      --onet-text-zip "$ONET_TEXT_ZIP" \
      --system-specs "$SYSTEM_SPECS" \
      --json | tee "$RUN_ROOT/${run_id}_research_smoke.json"
    jq -e '.status == "passed"' "$bundle/run_manifest.json"
    jq -e '.status == "passed"' "$bundle/fsv_report.json"
    CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa inspect-counts \
      --db "$bundle/dbs/${run_id}_career_taxonomy_rocksdb" \
      --json | tee "$bundle/local_ci_inspect_counts_career_taxonomy.json"
    jq -e '[.[].artifact_hashes[] | select(.equal != true)] | length == 0' "$bundle/artifact_hashes.json"
    CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa check-reference \
      --bundle "$bundle" \
      --reference reference/reference_paper_small_v1.json \
      --tolerances reference/tolerances.json \
      --json | tee "$RUN_ROOT/${run_id}_reference_check.json"
  done
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa capture-reference \
    --bundle "$RUN_ROOT/$run_a" \
    --output "$RUN_ROOT/${run_a}_normalized_reference.json" \
    --reference-id local_seed42_a \
    --json
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa capture-reference \
    --bundle "$RUN_ROOT/$run_b" \
    --output "$RUN_ROOT/${run_b}_normalized_reference.json" \
    --reference-id local_seed42_b \
    --json
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa check-reference \
    --bundle "$RUN_ROOT/$run_b" \
    --reference "$RUN_ROOT/${run_a}_normalized_reference.json" \
    --tolerances reference/tolerances.json \
    --json | tee "$RUN_ROOT/${RUN_ID_PREFIX}_seed42_same_seed_reference_check.json"
  jq -e '.status == "bit_equal" or .status == "within_tolerance"' \
    "$RUN_ROOT/${RUN_ID_PREFIX}_seed42_same_seed_reference_check.json"
}

run_hp_sweep_gate() {
  local hp_root="$RUN_ROOT/${RUN_ID_PREFIX}_hp_sweep_batch_size"
  require_no_existing_path "$hp_root" "LOCAL_CI_HP_SWEEP_ROOT_EXISTS"
  add_evidence "$hp_root"
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa hp-sweep \
    --base-config configs/dynamicjepa/verification/career_taxonomy_train_config.json \
    --sweep-axis batch_size \
    --sweep-values 16,32 \
    --output-root "$hp_root" \
    --fixtures-root configs/dynamicjepa \
    --purpose smoke \
    --career-event-count 1000 \
    --career-training-seeds 42,43 \
    --onet-text-zip "$ONET_TEXT_ZIP" \
    --system-specs "$SYSTEM_SPECS" \
    --json | tee "$RUN_ROOT/${RUN_ID_PREFIX}_hp_sweep_batch_size.json"
  jq -e '.status == "passed"' "$hp_root/sweep_manifest.json"
  jq -e '.status == "passed"' "$hp_root/sweep_fsv_report.json"
  jq -e '.readback_checks.cell_count == 4' "$hp_root/sweep_fsv_report.json"
  test -s "$hp_root/paper_tables/table_hp_sensitivity.csv"
  test -s "$hp_root/paper_tables/table_collapse_diagnostics.csv"
  test -s "$hp_root/plots/collapse_diagnostic_f5.svg"
}

run_release_matrix_and_aggregate() {
  require_file "reference/reference_release_v1.0.0.json" "LOCAL_CI_REFERENCE_MISSING"
  local bundle_list="$RUN_ROOT/${RUN_ID_PREFIX}_release_bundle_paths.txt"
  require_no_existing_path "$bundle_list" "LOCAL_CI_RELEASE_BUNDLE_LIST_EXISTS"
  : > "$bundle_list"
  add_evidence "$bundle_list"
  local failed=0
  for seed in 42 43 44 45 46; do
    local run_id="${RUN_ID_PREFIX}_paper_small_seed${seed}"
    local bundle="$RUN_ROOT/$run_id"
    if [[ -e "$bundle" ]]; then
      echo "LOCAL_CI_RELEASE_SEED_ROOT_EXISTS: refusing to overwrite $bundle" >&2
      failed=1
      continue
    fi
    add_evidence "$bundle"
    set +e
    CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
      --run-id "$run_id" \
      --run-root "$RUN_ROOT" \
      --fixtures-root configs/dynamicjepa \
      --purpose paper_small \
      --career-event-count 1000 \
      --career-training-seed "$seed" \
      --onet-text-zip "$ONET_TEXT_ZIP" \
      --system-specs "$SYSTEM_SPECS" \
      --json | tee "$RUN_ROOT/${run_id}_research_smoke.json"
    local smoke_status=${PIPESTATUS[0]}
    set -e
    printf '%s\n' "$bundle" >> "$bundle_list"
    if [[ "$smoke_status" -ne 0 ]]; then
      echo "LOCAL_CI_RELEASE_SEED_FAILED: seed $seed failed; bundle preserved at $bundle" >&2
      failed=1
      continue
    fi
    jq -e '.status == "passed"' "$bundle/run_manifest.json" || failed=1
    jq -e '.status == "passed"' "$bundle/fsv_report.json" || failed=1
    CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa inspect-counts \
      --db "$bundle/dbs/${run_id}_career_taxonomy_rocksdb" \
      --json | tee "$bundle/local_ci_inspect_counts_career_taxonomy.json"
    jq -e '.counts.dj_verification_runs >= 1 and .counts.dj_model_artifacts >= 1' \
      "$bundle/local_ci_inspect_counts_career_taxonomy.json" || failed=1
    jq -e '[.[].artifact_hashes[] | select(.equal != true)] | length == 0' "$bundle/artifact_hashes.json" || failed=1
  done
  if [[ "$failed" -ne 0 ]]; then
    echo "LOCAL_CI_RELEASE_MATRIX_FAILED: at least one seed failed; bundles preserved under $RUN_ROOT" >&2
    exit 1
  fi
  mapfile -t bundles < "$bundle_list"
  if [[ "${#bundles[@]}" -ne 5 ]]; then
    echo "LOCAL_CI_RELEASE_AGGREGATE_BUNDLE_COUNT: expected 5 bundles, got ${#bundles[@]}" >&2
    exit 1
  fi
  local agg_root="$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_multiseed_aggregate"
  require_no_existing_path "$agg_root" "LOCAL_CI_RELEASE_AGGREGATE_ROOT_EXISTS"
  add_evidence "$agg_root"
  local bundle_args=()
  for bundle in "${bundles[@]}"; do
    bundle_args+=(--bundle "$bundle")
  done
  CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa multiseed-aggregate \
    "${bundle_args[@]}" \
    --output-root "$agg_root" \
    --expected-bundle-count 5 \
    --min-passed 5 \
    --expected-purpose paper_small \
    --expected-career-event-count 1000 \
    --required-career-training-seeds 42,43,44,45,46 \
    --bootstrap-iters 10000 \
    --bootstrap-seed 20260430 \
    --json | tee "$RUN_ROOT/${RUN_ID_PREFIX}_release_multiseed_aggregate.json"
  jq -e '.status == "passed"' "$agg_root/aggregate_manifest.json"
  jq -e '.status == "passed"' "$agg_root/aggregate_fsv_report.json"
  jq -e '.readback_checks.prediction_multiseed_rows > 0' "$agg_root/aggregate_fsv_report.json"
  jq -e '.readback_checks.ablation_multiseed_rows > 0' "$agg_root/aggregate_fsv_report.json"
  test -s "$agg_root/paper_tables/table_prediction_metrics_multiseed.csv"
  test -s "$agg_root/paper_tables/table_ablation_multiseed.csv"
  test -s "$agg_root/paper_tables/table_paired_ablation_tests.csv"
  test -s "$agg_root/plots/multiseed_val_mse_box.svg"
  test -s "$agg_root/plots/multiseed_paired_delta_forest.svg"
}

case "$TIER" in
  preflight)
    preflight_cuda_and_data
    ;;
  static)
    static_gate
    ;;
  smoke-a)
    preflight_cuda_and_data
    static_gate
    build_dynamicjepa
    run_smoke_a_bundle
    ;;
  smoke-b)
    preflight_cuda_and_data
    static_gate
    build_dynamicjepa
    run_smoke_a_bundle
    run_paper_seed42_twice
    run_hp_sweep_gate
    ;;
  release-c)
    preflight_cuda_and_data
    require_command docker
    require_file Dockerfile "LOCAL_CI_DOCKERFILE_MISSING"
    require_file "scripts/5090jepa/artifact_evaluation_dry_run.sh" "LOCAL_CI_ARTIFACT_EVALUATION_MISSING"
    test -x ./reproduce.sh || {
      echo "LOCAL_CI_REPRODUCE_SCRIPT_MISSING: release-c requires executable reproduce.sh" >&2
      exit 1
    }
    static_gate
    build_dynamicjepa
    run_release_matrix_and_aggregate
    ./reproduce.sh verify-aggregate \
      --aggregate "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_multiseed_aggregate" \
      --reference reference/reference_release_v1.0.0.json \
      --json | tee "$RUN_ROOT/${RUN_ID_PREFIX}_release_reference_check.json"
    jq -e '.status == "ok"' "$RUN_ROOT/${RUN_ID_PREFIX}_release_reference_check.json"
    export DOCKER_BUILDKIT=1
    export SOURCE_DATE_EPOCH=1777600000
    docker_tar_1="$RUN_ROOT/${RUN_ID_PREFIX}_docker_image_1.tar"
    docker_tar_2="$RUN_ROOT/${RUN_ID_PREFIX}_docker_image_2.tar"
    docker_sha_1="$RUN_ROOT/${RUN_ID_PREFIX}_docker_image_1.sha256"
    docker_sha_2="$RUN_ROOT/${RUN_ID_PREFIX}_docker_image_2.sha256"
    docker buildx build --progress=plain --platform linux/amd64 --provenance=false --sbom=false \
      --build-arg SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" \
      --output "type=docker,rewrite-timestamp=true,dest=$docker_tar_1" .
    docker buildx build --progress=plain --platform linux/amd64 --provenance=false --sbom=false --no-cache \
      --build-arg SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" \
      --output "type=docker,rewrite-timestamp=true,dest=$docker_tar_2" .
    sha256sum "$docker_tar_1" | tee "$docker_sha_1"
    sha256sum "$docker_tar_2" | tee "$docker_sha_2"
    cmp -s "$docker_tar_1" "$docker_tar_2" || {
      echo "LOCAL_CI_DOCKER_IMAGE_TAR_DRIFT: deterministic Docker tar exports differ" >&2
      exit 1
    }
    docker load -i "$docker_tar_1" | tee "$RUN_ROOT/${RUN_ID_PREFIX}_docker_load.txt"
    loaded_image_id="$(awk '/Loaded image ID:/ {print $4}' "$RUN_ROOT/${RUN_ID_PREFIX}_docker_load.txt" | tail -n 1)"
    [[ -n "$loaded_image_id" ]] || {
      echo "LOCAL_CI_DOCKER_LOAD_IMAGE_ID_MISSING: docker load did not report an image ID" >&2
      exit 1
    }
    aggregate_abs="$(realpath "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_multiseed_aggregate")"
    docker run --rm --gpus all \
      -v "$aggregate_abs:/work/aggregate:ro" \
      "$loaded_image_id" verify-aggregate \
      --aggregate /work/aggregate \
      --reference /workspace/reference/reference_release_v1.0.0.json \
      --json | tee "$RUN_ROOT/${RUN_ID_PREFIX}_docker_container_verify_aggregate.json"
    jq -e '.status == "ok"' "$RUN_ROOT/${RUN_ID_PREFIX}_docker_container_verify_aggregate.json"
    scripts/5090jepa/artifact_evaluation_dry_run.sh \
      --aggregate "$RUN_ROOT/${RUN_ID_PREFIX}_paper_small_multiseed_aggregate" \
      --reference reference/reference_release_v1.0.0.json \
      --output "$RUN_ROOT/${RUN_ID_PREFIX}_artifact_evaluation_dry_run.json" \
      --json
    jq -e '.status == "ok"' "$RUN_ROOT/${RUN_ID_PREFIX}_artifact_evaluation_dry_run.json"
    ;;
esac

STATUS="passed"
log "passed"
