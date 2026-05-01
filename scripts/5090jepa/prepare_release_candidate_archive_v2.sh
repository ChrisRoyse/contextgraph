#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/5090jepa/prepare_release_candidate_archive_v2.sh \
    --release-id mejepa_5090_artifact_v2.0.0_rc1 \
    --aggregate PATH \
    --local-ci-manifest PATH \
    --docker-tar-1 PATH \
    --docker-tar-2 PATH \
    --container-verify-json PATH \
    --reference reference/reference_release_v2.0.0.json \
    --output-root tmp/mejepa_release_artifacts \
    [--full-bundle-seed 42] \
    [--apptainer-sif PATH] \
    [--require-apptainer]

Builds a local, no-upload release-candidate archive from preserved release-D
evidence. The script fails closed if the source aggregate, reference, Docker
artifacts, Apptainer artifact, pairwise MI audit, container verification, or
selected full seed bundle are invalid.

This script does not upload to Zenodo and does not synthesize an Apptainer SIF.
Release-D packaging requires --require-apptainer and --apptainer-sif PATH.
USAGE
}

die() {
  local code="$1"
  local message="$2"
  echo "${code}: ${message}" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "RELEASE_ARCHIVE_COMMAND_MISSING" "required command '${cmd}' is not on PATH"
}

require_file() {
  local path="$1"
  local code="$2"
  [[ -f "$path" ]] || die "$code" "required file '${path}' does not exist"
}

require_executable_file() {
  local path="$1"
  local code="$2"
  [[ -x "$path" ]] || die "$code" "required executable '${path}' does not exist or is not executable"
}

require_dir() {
  local path="$1"
  local code="$2"
  [[ -d "$path" ]] || die "$code" "required directory '${path}' does not exist"
}

require_no_existing_path() {
  local path="$1"
  local code="$2"
  [[ ! -e "$path" ]] || die "$code" "refusing to overwrite existing path '${path}'"
}

sha_file() {
  sha256sum "$1" | awk '{print $1}'
}

size_file() {
  stat -c '%s' "$1"
}

path_state_json() {
  local path="$1"
  if [[ ! -e "$path" ]]; then
    jq -n --arg path "$path" '{path: $path, exists: false, kind: null, sha256: null, size_bytes: null}'
  elif [[ -f "$path" ]]; then
    jq -n \
      --arg path "$path" \
      --arg sha256 "$(sha_file "$path")" \
      --argjson size_bytes "$(size_file "$path")" \
      '{path: $path, exists: true, kind: "file", sha256: $sha256, size_bytes: $size_bytes}'
  elif [[ -d "$path" ]]; then
    jq -n \
      --arg path "$path" \
      --argjson file_count "$(find "$path" -type f | wc -l)" \
      --argjson size_bytes "$(du -sb "$path" | awk '{print $1}')" \
      '{path: $path, exists: true, kind: "directory", sha256: null, size_bytes: $size_bytes, file_count: $file_count}'
  else
    jq -n --arg path "$path" '{path: $path, exists: true, kind: "other", sha256: null, size_bytes: null}'
  fi
}

write_archive_file_hashes() {
  local artifact_dir="$1"
  local output="$artifact_dir/archive_file_hashes.json"
  local tmp
  tmp="$(mktemp)"
  (
    cd "$artifact_dir"
    while IFS= read -r rel_path; do
      rel_path="${rel_path#./}"
      printf '%s\t%s\t%s\n' "$(sha_file "$rel_path")" "$(size_file "$rel_path")" "$rel_path" >> "$tmp"
    done < <(find . -type f ! -name 'archive_file_hashes.json' -print | sort)
  )
  jq -Rn '
    [inputs
      | split("\t")
      | {sha256: .[0], size_bytes: (.[1] | tonumber), path: .[2]}]
  ' < "$tmp" > "$output"
  rm -f "$tmp"
}

release_id=""
aggregate=""
local_ci_manifest=""
docker_tar_1=""
docker_tar_2=""
container_verify_json=""
reference=""
output_root=""
full_bundle_seed="42"
apptainer_sif=""
require_apptainer=false
source_date_epoch="${SOURCE_DATE_EPOCH:-}"
context_graph_bin="${CONTEXT_GRAPH_BIN:-target/debug/context-graph}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release-id)
      release_id="$2"
      shift 2
      ;;
    --aggregate)
      aggregate="$2"
      shift 2
      ;;
    --local-ci-manifest)
      local_ci_manifest="$2"
      shift 2
      ;;
    --docker-tar-1)
      docker_tar_1="$2"
      shift 2
      ;;
    --docker-tar-2)
      docker_tar_2="$2"
      shift 2
      ;;
    --container-verify-json)
      container_verify_json="$2"
      shift 2
      ;;
    --reference)
      reference="$2"
      shift 2
      ;;
    --output-root)
      output_root="$2"
      shift 2
      ;;
    --full-bundle-seed)
      full_bundle_seed="$2"
      shift 2
      ;;
    --apptainer-sif)
      apptainer_sif="$2"
      shift 2
      ;;
    --require-apptainer)
      require_apptainer=true
      shift
      ;;
    --source-date-epoch)
      source_date_epoch="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "RELEASE_ARCHIVE_UNKNOWN_ARG: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

require_cmd jq
require_cmd sha256sum
require_cmd stat
require_cmd du
require_cmd tar
require_cmd gzip
require_cmd ldd
require_cmd realpath

[[ -n "$release_id" ]] || die "RELEASE_ARCHIVE_RELEASE_ID_REQUIRED" "--release-id is required"
[[ "$release_id" =~ ^[A-Za-z0-9]([A-Za-z0-9._-]*[A-Za-z0-9])?$ ]] ||
  die "RELEASE_ARCHIVE_RELEASE_ID_INVALID" "--release-id must start and end with a letter or number and may contain only letters, numbers, dot, underscore, and dash"
[[ -n "$aggregate" ]] || die "RELEASE_ARCHIVE_AGGREGATE_REQUIRED" "--aggregate is required"
[[ -n "$local_ci_manifest" ]] || die "RELEASE_ARCHIVE_LOCAL_CI_MANIFEST_REQUIRED" "--local-ci-manifest is required"
[[ -n "$docker_tar_1" ]] || die "RELEASE_ARCHIVE_DOCKER_TAR_1_REQUIRED" "--docker-tar-1 is required"
[[ -n "$docker_tar_2" ]] || die "RELEASE_ARCHIVE_DOCKER_TAR_2_REQUIRED" "--docker-tar-2 is required"
[[ -n "$container_verify_json" ]] || die "RELEASE_ARCHIVE_CONTAINER_VERIFY_JSON_REQUIRED" "--container-verify-json is required"
[[ -n "$reference" ]] || die "RELEASE_ARCHIVE_REFERENCE_REQUIRED" "--reference is required"
[[ -n "$output_root" ]] || die "RELEASE_ARCHIVE_OUTPUT_ROOT_REQUIRED" "--output-root is required"
[[ "$full_bundle_seed" =~ ^[0-9]+$ ]] || die "RELEASE_ARCHIVE_FULL_BUNDLE_SEED_INVALID" "--full-bundle-seed must be an integer"

require_dir "$aggregate" "RELEASE_ARCHIVE_AGGREGATE_MISSING"
require_file "$aggregate/aggregate_manifest.json" "RELEASE_ARCHIVE_AGGREGATE_MANIFEST_MISSING"
require_file "$aggregate/aggregate_fsv_report.json" "RELEASE_ARCHIVE_AGGREGATE_FSV_MISSING"
require_file "$local_ci_manifest" "RELEASE_ARCHIVE_LOCAL_CI_MANIFEST_MISSING"
require_file "$docker_tar_1" "RELEASE_ARCHIVE_DOCKER_TAR_1_MISSING"
require_file "$docker_tar_2" "RELEASE_ARCHIVE_DOCKER_TAR_2_MISSING"
require_file "$container_verify_json" "RELEASE_ARCHIVE_CONTAINER_VERIFY_JSON_MISSING"
require_file "$reference" "RELEASE_ARCHIVE_REFERENCE_MISSING"
require_file "Dockerfile" "RELEASE_ARCHIVE_DOCKERFILE_MISSING"
require_executable_file "reproduce.sh" "RELEASE_ARCHIVE_REPRODUCE_MISSING"
require_executable_file "$context_graph_bin" "RELEASE_ARCHIVE_CONTEXT_GRAPH_BIN_MISSING"
require_file "LICENSE" "RELEASE_ARCHIVE_LICENSE_MISSING"
require_file "LICENSING.md" "RELEASE_ARCHIVE_LICENSING_MISSING"
require_file "CITATION.cff" "RELEASE_ARCHIVE_CITATION_MISSING"

if [[ "$require_apptainer" != true ]]; then
  die "RELEASE_ARCHIVE_V2_APPTAINER_REQUIRED" "release-D v2 packaging requires --require-apptainer"
fi
if [[ -z "$apptainer_sif" ]]; then
  die "RELEASE_ARCHIVE_APPTAINER_REQUIRED" "--require-apptainer requires --apptainer-sif PATH"
fi
require_file "$apptainer_sif" "RELEASE_ARCHIVE_APPTAINER_SIF_MISSING"
require_cmd singularity

if [[ -z "$source_date_epoch" ]]; then
  source_date_epoch="$(git log -1 --pretty=%ct)"
fi
[[ "$source_date_epoch" =~ ^[0-9]+$ ]] || die "RELEASE_ARCHIVE_SOURCE_DATE_EPOCH_INVALID" "SOURCE_DATE_EPOCH must be an integer Unix timestamp"

jq -e '.status == "passed" and .tier == "release-d" and .exit_code == 0' "$local_ci_manifest" >/dev/null ||
  die "RELEASE_ARCHIVE_LOCAL_CI_NOT_PASSED" "local CI manifest must be a passed release-d manifest"
jq -e '.status == "passed" and .failed_count == 0 and .passed_count == 5' "$aggregate/aggregate_manifest.json" >/dev/null ||
  die "RELEASE_ARCHIVE_AGGREGATE_NOT_PASSED" "aggregate manifest must have status=passed, failed_count=0, passed_count=5"
jq -e '.status == "passed"' "$aggregate/aggregate_fsv_report.json" >/dev/null ||
  die "RELEASE_ARCHIVE_AGGREGATE_FSV_NOT_PASSED" "aggregate FSV report must have status=passed"
jq -e '.status == "ok" and .comparison.diff_count == 0 and .readback.manifest_status == "passed" and .readback.fsv_status == "passed"' "$container_verify_json" >/dev/null ||
  die "RELEASE_ARCHIVE_CONTAINER_VERIFY_NOT_PASSED" "container verification JSON must be status=ok with zero reference diffs"

docker_sha_1="$(sha_file "$docker_tar_1")"
docker_sha_2="$(sha_file "$docker_tar_2")"
[[ "$docker_sha_1" == "$docker_sha_2" ]] ||
  die "RELEASE_ARCHIVE_DOCKER_TARS_DIFFER" "deterministic Docker tar hashes differ: ${docker_sha_1} != ${docker_sha_2}"
cmp -s "$docker_tar_1" "$docker_tar_2" ||
  die "RELEASE_ARCHIVE_DOCKER_TARS_NOT_BYTE_IDENTICAL" "Docker tar files are not byte-identical"

cuda_image="$(sed -n 's/^ARG CUDA_IMAGE=//p' Dockerfile | head -n 1)"
[[ "$cuda_image" == *@sha256:* ]] ||
  die "RELEASE_ARCHIVE_DOCKER_BASE_NOT_PINNED" "Dockerfile CUDA_IMAGE must use an immutable @sha256 digest"
grep -qx 'FROM ${CUDA_IMAGE}' Dockerfile ||
  die "RELEASE_ARCHIVE_DOCKER_FROM_NOT_ARG" "Dockerfile must use FROM \${CUDA_IMAGE}"

tmp_reference_check="$(mktemp)"
tmp_sif_inspect="$(mktemp)"
tmp_sif_list="$(mktemp)"
tmp_sif_verify="$(mktemp)"
if ! ./reproduce.sh verify-aggregate --aggregate "$aggregate" --reference "$reference" --json > "$tmp_reference_check"; then
  cat "$tmp_reference_check" >&2
  rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
  die "RELEASE_ARCHIVE_REFERENCE_CHECK_FAILED" "release aggregate does not match the checked-in reference"
fi
jq -e '.status == "ok" and .comparison.diff_count == 0' "$tmp_reference_check" >/dev/null ||
  die "RELEASE_ARCHIVE_REFERENCE_CHECK_NOT_OK" "reference check JSON did not report status=ok and diff_count=0"

full_bundle_path="$(
  jq -er --arg seed "$full_bundle_seed" '
    .source_bundles[]
    | select((.career_training_seed | tostring) == $seed)
    | select(.status == "passed" and .fsv_status == "passed")
    | .path
  ' "$aggregate/aggregate_manifest.json" | head -n 1
)"
[[ -n "$full_bundle_path" ]] ||
  die "RELEASE_ARCHIVE_FULL_BUNDLE_NOT_SELECTED" "aggregate manifest does not contain a passed full bundle for seed ${full_bundle_seed}"
require_dir "$full_bundle_path" "RELEASE_ARCHIVE_FULL_BUNDLE_MISSING"
require_file "$full_bundle_path/run_manifest.json" "RELEASE_ARCHIVE_FULL_BUNDLE_MANIFEST_MISSING"
require_file "$full_bundle_path/fsv_report.json" "RELEASE_ARCHIVE_FULL_BUNDLE_FSV_MISSING"
require_file "$full_bundle_path/data_manifest.json" "RELEASE_ARCHIVE_FULL_BUNDLE_DATA_MANIFEST_MISSING"
jq -e '.status == "passed"' "$full_bundle_path/run_manifest.json" >/dev/null ||
  die "RELEASE_ARCHIVE_FULL_BUNDLE_RUN_NOT_PASSED" "selected full bundle run_manifest.json is not passed"
jq -e '.status == "passed"' "$full_bundle_path/fsv_report.json" >/dev/null ||
  die "RELEASE_ARCHIVE_FULL_BUNDLE_FSV_NOT_PASSED" "selected full bundle fsv_report.json is not passed"

for table in \
  table_prediction_metrics_multiseed.csv \
  table_ablation_multiseed.csv \
  table_paired_ablation_tests.csv \
  table_seed_failures.csv
do
  require_file "$aggregate/paper_tables/$table" "RELEASE_ARCHIVE_AGGREGATE_TABLE_MISSING"
done
mi_root="$aggregate/pairwise_mi_audit_career_taxonomy"
require_dir "$mi_root" "RELEASE_ARCHIVE_PAIRWISE_MI_ROOT_MISSING"
require_file "$mi_root/pairwise_mi_matrix.csv" "RELEASE_ARCHIVE_PAIRWISE_MI_MATRIX_MISSING"
require_file "$mi_root/paper_tables/table_pairwise_mi_summary.csv" "RELEASE_ARCHIVE_PAIRWISE_MI_SUMMARY_MISSING"
require_file "$mi_root/pairwise_mi_heatmap.svg" "RELEASE_ARCHIVE_PAIRWISE_MI_HEATMAP_MISSING"
require_file "$mi_root/pairwise_mi_audit.json" "RELEASE_ARCHIVE_PAIRWISE_MI_AUDIT_JSON_MISSING"
jq -e '.summary.domain == "career_taxonomy" and (.summary.n_eff_strict >= 9.0 and .summary.n_eff_strict <= 10.0)' \
  "$mi_root/pairwise_mi_audit.json" >/dev/null ||
  die "RELEASE_ARCHIVE_PAIRWISE_MI_NOT_PASSED" "pairwise MI audit JSON must report career_taxonomy n_eff_strict in [9.0, 10.0]"
jq -e '(.estimator.sample_size == 1000) and (.matrix | length == 45)' "$mi_root/pairwise_mi_audit.json" >/dev/null ||
  die "RELEASE_ARCHIVE_PAIRWISE_MI_SHAPE_DRIFT" "pairwise MI audit JSON must report sample_size=1000 and 45 pair rows"

if [[ -n "$apptainer_sif" ]]; then
  if ! singularity inspect --json "$apptainer_sif" > "$tmp_sif_inspect"; then
    rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
    die "RELEASE_ARCHIVE_APPTAINER_INSPECT_FAILED" "singularity inspect failed for '${apptainer_sif}'"
  fi
  if ! singularity sif list "$apptainer_sif" > "$tmp_sif_list"; then
    rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
    die "RELEASE_ARCHIVE_APPTAINER_SIF_LIST_FAILED" "singularity sif list failed for '${apptainer_sif}'"
  fi
  if ! singularity exec --nv --cleanenv --no-home \
    -B "$(realpath "$aggregate"):/work/aggregate:ro" \
    "$apptainer_sif" \
      /workspace/reproduce.sh verify-aggregate \
      --aggregate /work/aggregate \
      --reference /workspace/reference/reference_release_v2.0.0.json \
      --json > "$tmp_sif_verify"; then
    cat "$tmp_sif_verify" >&2
    rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
    die "RELEASE_ARCHIVE_APPTAINER_VERIFY_FAILED" "SIF aggregate verification failed for '${apptainer_sif}'"
  fi
  if ! jq -e '.status == "ok" and .comparison.diff_count == 0 and .readback.manifest_status == "passed" and .readback.fsv_status == "passed"' "$tmp_sif_verify" >/dev/null; then
    rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
    die "RELEASE_ARCHIVE_APPTAINER_VERIFY_NOT_OK" "SIF verification JSON did not report status=ok, diff_count=0, and passed readback"
  fi
fi

output_root="${output_root%/}"
artifact_dir="$output_root/$release_id"
archive_path="$output_root/$release_id.tar.gz"
require_no_existing_path "$artifact_dir" "RELEASE_ARCHIVE_OUTPUT_DIR_EXISTS"
require_no_existing_path "$archive_path" "RELEASE_ARCHIVE_OUTPUT_ARCHIVE_EXISTS"
mkdir -p "$artifact_dir"/{bin,bundles,container,manifests,paper_tables_reference,THIRD_PARTY_LICENSES,docs}

cp -a LICENSE "$artifact_dir/LICENSE"
cp -a LICENSING.md "$artifact_dir/LICENSING.md"
cp -a CITATION.cff "$artifact_dir/CITATION.cff"
cp -a reproduce.sh "$artifact_dir/reproduce.sh"
cp -a Dockerfile "$artifact_dir/Dockerfile"
cp -a "$context_graph_bin" "$artifact_dir/bin/context-graph"
ldd "$context_graph_bin" > "$artifact_dir/manifests/context_graph_ldd.txt"
cp -a "$reference" "$artifact_dir/reference_release_v2.0.0.json"
cp -a "$aggregate" "$artifact_dir/bundles/$(basename "$aggregate")"
cp -a "$full_bundle_path" "$artifact_dir/bundles/$(basename "$full_bundle_path")"
cp -a "$aggregate"/paper_tables/*.csv "$artifact_dir/paper_tables_reference/"
cp -a "$mi_root/paper_tables/table_pairwise_mi_summary.csv" "$artifact_dir/paper_tables_reference/table_pairwise_mi_summary.csv"
cp -a "$mi_root/pairwise_mi_matrix.csv" "$artifact_dir/paper_tables_reference/pairwise_mi_matrix.csv"
cp -a "$docker_tar_1" "$artifact_dir/container/contextgraph-mejepa.docker.tar"
cp -a "$local_ci_manifest" "$artifact_dir/manifests/local_ci_manifest.json"
cp -a "$container_verify_json" "$artifact_dir/manifests/docker_container_verify_aggregate.json"
cp -a "$tmp_reference_check" "$artifact_dir/manifests/release_reference_verify.json"
if [[ -n "$apptainer_sif" ]]; then
  cp -a "$apptainer_sif" "$artifact_dir/container/contextgraph-mejepa.sif"
  cp -a "$tmp_sif_inspect" "$artifact_dir/manifests/sif_inspect.json"
  cp -a "$tmp_sif_list" "$artifact_dir/manifests/sif_list.txt"
  cp -a "$tmp_sif_verify" "$artifact_dir/manifests/sif_verify_aggregate.json"
fi

if [[ -f THIRD_PARTY_LICENSES/ONET.md ]]; then
  cp -a THIRD_PARTY_LICENSES/ONET.md "$artifact_dir/THIRD_PARTY_LICENSES/ONET.md"
else
  onet_license="$(jq -er '.sources[0].license' "$full_bundle_path/data_manifest.json")"
  onet_source_urls="$(jq -r '.sources[0].source_urls[]' "$full_bundle_path/data_manifest.json")"
  {
    printf '# O*NET Attribution\n\n'
    printf '%s\n\n' "$onet_license"
    printf 'Source URLs:\n\n'
    while IFS= read -r source_url; do
      printf -- '- %s\n' "$source_url"
    done <<< "$onet_source_urls"
  } > "$artifact_dir/THIRD_PARTY_LICENSES/ONET.md"
fi

cat > "$artifact_dir/CHANGELOG.md" <<'CHANGELOG'
# Changelog

## v2.0.0-rc1 Release Candidate

- Preserves the passed release-D five-seed `paper_small` aggregate.
- Includes the career_taxonomy KSG pairwise MI audit and heatmap.
- Includes one full passed seed bundle for source-of-truth inspection.
- Includes deterministic Docker image tar evidence from two byte-identical exports.
- Includes deterministic Apptainer SIF evidence and runtime verification.
- Includes local reference verification and Docker/Apptainer container readback.
CHANGELOG

onet_zip_sha="$(jq -er '.sources[0].source_zip_sha256' "$full_bundle_path/data_manifest.json")"
onet_row_count="$(jq -er '.sources[0].row_count' "$full_bundle_path/data_manifest.json")"
cat > "$artifact_dir/README.md" <<README
# Context Graph DynamicJEPA RTX 5090 Artifact

This is a local release-candidate archive built from preserved release-D
evidence. It contains the passed five-seed aggregate, one full passed source
bundle for manual inspection, the pairwise MI audit, deterministic Docker and
Apptainer evidence, and reference verification output.

## Source Of Truth

- Aggregate: \`bundles/$(basename "$aggregate")\`
- Full seed bundle: \`bundles/$(basename "$full_bundle_path")\`
- Docker image archive: \`container/contextgraph-mejepa.docker.tar\`
- Apptainer image archive: \`container/contextgraph-mejepa.sif\`
- Local verifier binary: \`bin/context-graph\`
- Reference: \`reference_release_v2.0.0.json\`
- Pairwise MI audit: \`bundles/$(basename "$aggregate")/pairwise_mi_audit_career_taxonomy\`
- File hashes: \`archive_file_hashes.json\`

## Local Verification

\`\`\`bash
CONTEXT_GRAPH_BIN=bin/context-graph ./reproduce.sh verify-aggregate \\
  --aggregate bundles/$(basename "$aggregate") \\
  --reference reference_release_v2.0.0.json \\
  --json
\`\`\`

## Data Boundary

The career benchmark is generated from the public O*NET 30.2 text database.
It is a public taxonomy benchmark, not real recruiting history. The packaged
selected bundle records O*NET source ZIP SHA-256 \`${onet_zip_sha}\` and row
count \`${onet_row_count}\` in \`data_manifest.json\`.

## Release Boundary

This archive is not a published DOI artifact. Zenodo upload and DOI publication
must happen separately after the archive is reviewed.
README

reference_manifest_sha="$(sha_file "$aggregate/aggregate_manifest.json")"
reference_fsv_sha="$(sha_file "$aggregate/aggregate_fsv_report.json")"
prediction_sha="$(sha_file "$aggregate/paper_tables/table_prediction_metrics_multiseed.csv")"
ablation_sha="$(sha_file "$aggregate/paper_tables/table_ablation_multiseed.csv")"
paired_sha="$(sha_file "$aggregate/paper_tables/table_paired_ablation_tests.csv")"
seed_failures_sha="$(sha_file "$aggregate/paper_tables/table_seed_failures.csv")"
pairwise_mi_summary_sha="$(sha_file "$mi_root/paper_tables/table_pairwise_mi_summary.csv")"
pairwise_mi_matrix_sha="$(sha_file "$mi_root/pairwise_mi_matrix.csv")"
pairwise_mi_heatmap_sha="$(sha_file "$mi_root/pairwise_mi_heatmap.svg")"
pairwise_mi_audit_json_sha="$(sha_file "$mi_root/pairwise_mi_audit.json")"
full_bundle_manifest_sha="$(sha_file "$full_bundle_path/run_manifest.json")"
full_bundle_fsv_sha="$(sha_file "$full_bundle_path/fsv_report.json")"
reference_json_sha="$(sha_file "$reference")"

apptainer_status="not_included"
apptainer_sha=""
apptainer_size="0"
release_stage="local_docker_sif_pairwise_mi_release_candidate"
if [[ -n "$apptainer_sif" ]]; then
  apptainer_status="included"
  apptainer_sha="$(sha_file "$apptainer_sif")"
  apptainer_size="$(size_file "$apptainer_sif")"
fi

jq -n \
  --arg operation "dynamicjepa_release_candidate_archive_v2" \
  --arg release_id "$release_id" \
  --arg release_stage "$release_stage" \
  --arg aggregate "$aggregate" \
  --arg selected_full_bundle "$full_bundle_path" \
  --arg reference "$reference" \
  --arg docker_tar_1 "$docker_tar_1" \
  --arg docker_tar_2 "$docker_tar_2" \
  --arg docker_tar_sha256 "$docker_sha_1" \
  --arg context_graph_bin_sha256 "$(sha_file "$context_graph_bin")" \
  --arg dockerfile_sha256 "$(sha_file Dockerfile)" \
  --arg reproduce_sha256 "$(sha_file reproduce.sh)" \
  --arg reference_json_sha256 "$reference_json_sha" \
  --arg aggregate_manifest_sha256 "$reference_manifest_sha" \
  --arg aggregate_fsv_report_sha256 "$reference_fsv_sha" \
  --arg prediction_sha256 "$prediction_sha" \
  --arg ablation_sha256 "$ablation_sha" \
  --arg paired_sha256 "$paired_sha" \
  --arg seed_failures_sha256 "$seed_failures_sha" \
  --arg pairwise_mi_summary_sha256 "$pairwise_mi_summary_sha" \
  --arg pairwise_mi_matrix_sha256 "$pairwise_mi_matrix_sha" \
  --arg pairwise_mi_heatmap_sha256 "$pairwise_mi_heatmap_sha" \
  --arg pairwise_mi_audit_json_sha256 "$pairwise_mi_audit_json_sha" \
  --arg full_bundle_manifest_sha256 "$full_bundle_manifest_sha" \
  --arg full_bundle_fsv_sha256 "$full_bundle_fsv_sha" \
  --arg cuda_image "$cuda_image" \
  --arg apptainer_status "$apptainer_status" \
  --arg apptainer_sha256 "$apptainer_sha" \
  --argjson apptainer_size_bytes "$apptainer_size" \
  '{
    operation: $operation,
    release_id: $release_id,
    status: "packaged",
    release_stage: $release_stage,
    source_of_truth: {
      aggregate: $aggregate,
      selected_full_bundle: $selected_full_bundle,
      reference: $reference,
      docker_tar_1: $docker_tar_1,
      docker_tar_2: $docker_tar_2
    },
    critical_hashes: {
      aggregate_manifest_sha256: $aggregate_manifest_sha256,
      aggregate_fsv_report_sha256: $aggregate_fsv_report_sha256,
      table_prediction_metrics_multiseed_sha256: $prediction_sha256,
      table_ablation_multiseed_sha256: $ablation_sha256,
      table_paired_ablation_tests_sha256: $paired_sha256,
      table_seed_failures_sha256: $seed_failures_sha256,
      table_pairwise_mi_summary_sha256: $pairwise_mi_summary_sha256,
      pairwise_mi_matrix_sha256: $pairwise_mi_matrix_sha256,
      pairwise_mi_heatmap_sha256: $pairwise_mi_heatmap_sha256,
      pairwise_mi_audit_json_sha256: $pairwise_mi_audit_json_sha256,
      selected_full_bundle_run_manifest_sha256: $full_bundle_manifest_sha256,
      selected_full_bundle_fsv_report_sha256: $full_bundle_fsv_sha256,
      reference_release_json_sha256: $reference_json_sha256,
      docker_tar_sha256: $docker_tar_sha256,
      context_graph_bin_sha256: $context_graph_bin_sha256,
      dockerfile_sha256: $dockerfile_sha256,
      reproduce_sha256: $reproduce_sha256
    },
    container: {
      cuda_image: $cuda_image,
      docker_tar_sha256: $docker_tar_sha256,
      docker_tars_byte_identical: true,
      apptainer: {
        status: $apptainer_status,
        sha256: (if $apptainer_sha256 == "" then null else $apptainer_sha256 end),
        size_bytes: (if $apptainer_size_bytes == 0 then null else $apptainer_size_bytes end)
      }
    },
    blockers: (
      if $apptainer_status == "included"
      then ["zenodo_doi_not_published"]
      else ["apptainer_sif_not_included_on_this_host", "zenodo_doi_not_published"]
      end
    )
  }' > "$artifact_dir/release_manifest.json"

jq -n \
  --arg docker_tar "container/contextgraph-mejepa.docker.tar" \
  --arg docker_tar_sha256 "$docker_sha_1" \
  --arg dockerfile_sha256 "$(sha_file Dockerfile)" \
  --arg apptainer_status "$apptainer_status" \
  --arg apptainer_sha256 "$apptainer_sha" \
  --arg cuda_image "$cuda_image" \
  --argjson docker_tar_size_bytes "$(size_file "$docker_tar_1")" \
  --argjson apptainer_size_bytes "$apptainer_size" \
  --slurpfile container_verify "$container_verify_json" \
  --slurpfile sif_verify "$tmp_sif_verify" \
  '{
    docker: {
      archive_path: $docker_tar,
      archive_sha256: $docker_tar_sha256,
      archive_size_bytes: $docker_tar_size_bytes,
      dockerfile_sha256: $dockerfile_sha256,
      cuda_image: $cuda_image,
      verification: $container_verify[0]
    },
    apptainer: {
      status: $apptainer_status,
      archive_path: (if $apptainer_status == "included" then "container/contextgraph-mejepa.sif" else null end),
      archive_sha256: (if $apptainer_sha256 == "" then null else $apptainer_sha256 end),
      archive_size_bytes: (if $apptainer_size_bytes == 0 then null else $apptainer_size_bytes end),
      verification: (if $apptainer_status == "included" then $sif_verify[0] else null end)
    }
  }' > "$artifact_dir/container/container_manifest.json"

jq -n \
  --arg aggregate_manifest_sha256 "$reference_manifest_sha" \
  --arg aggregate_fsv_report_sha256 "$reference_fsv_sha" \
  --arg table_prediction_metrics_multiseed_sha256 "$prediction_sha" \
  --arg table_ablation_multiseed_sha256 "$ablation_sha" \
  --arg table_paired_ablation_tests_sha256 "$paired_sha" \
  --arg table_seed_failures_sha256 "$seed_failures_sha" \
  --arg table_pairwise_mi_summary_sha256 "$pairwise_mi_summary_sha" \
  --arg pairwise_mi_matrix_sha256 "$pairwise_mi_matrix_sha" \
  --arg pairwise_mi_heatmap_sha256 "$pairwise_mi_heatmap_sha" \
  --arg pairwise_mi_audit_json_sha256 "$pairwise_mi_audit_json_sha" \
  --arg container_docker_tar_sha256 "$docker_sha_1" \
  --arg context_graph_bin_sha256 "$(sha_file "$context_graph_bin")" \
  --arg container_sif_sha256 "$apptainer_sha" \
  --arg reference_release_json_sha256 "$reference_json_sha" \
  '{
    aggregate_manifest_sha256: $aggregate_manifest_sha256,
    aggregate_fsv_report_sha256: $aggregate_fsv_report_sha256,
    table_prediction_metrics_multiseed_sha256: $table_prediction_metrics_multiseed_sha256,
    table_ablation_multiseed_sha256: $table_ablation_multiseed_sha256,
    table_paired_ablation_tests_sha256: $table_paired_ablation_tests_sha256,
    table_seed_failures_sha256: $table_seed_failures_sha256,
    table_pairwise_mi_summary_sha256: $table_pairwise_mi_summary_sha256,
    pairwise_mi_matrix_sha256: $pairwise_mi_matrix_sha256,
    pairwise_mi_heatmap_sha256: $pairwise_mi_heatmap_sha256,
    pairwise_mi_audit_json_sha256: $pairwise_mi_audit_json_sha256,
    container_docker_tar_sha256: $container_docker_tar_sha256,
    context_graph_bin_sha256: $context_graph_bin_sha256,
    container_sif_sha256: (if $container_sif_sha256 == "" then null else $container_sif_sha256 end),
    reference_release_json_sha256: $reference_release_json_sha256
  }' > "$artifact_dir/reference_hashes.json"

write_archive_file_hashes "$artifact_dir"
archive_hashes_sha="$(sha_file "$artifact_dir/archive_file_hashes.json")"
jq --arg archive_file_hashes_sha256 "$archive_hashes_sha" \
  '.critical_hashes.archive_file_hashes_sha256 = $archive_file_hashes_sha256' \
  "$artifact_dir/release_manifest.json" > "$artifact_dir/release_manifest.json.tmp"
mv "$artifact_dir/release_manifest.json.tmp" "$artifact_dir/release_manifest.json"

(
  cd "$output_root"
  tar --sort=name --mtime="@${source_date_epoch}" --owner=0 --group=0 --numeric-owner -cf - "$release_id" | gzip -n > "$(basename "$archive_path")"
)

archive_sha="$(sha_file "$archive_path")"
jq -n \
  --arg operation "dynamicjepa_release_candidate_archive_build" \
  --arg status "ok" \
  --arg release_id "$release_id" \
  --arg artifact_dir "$artifact_dir" \
  --arg archive "$archive_path" \
  --arg archive_sha256 "$archive_sha" \
  --argjson artifact_dir_state "$(path_state_json "$artifact_dir")" \
  --argjson archive_state "$(path_state_json "$archive_path")" \
  --slurpfile release_manifest "$artifact_dir/release_manifest.json" \
  '{
    operation: $operation,
    status: $status,
    release_id: $release_id,
    source_of_truth: {
      artifact_dir: $artifact_dir,
      archive: $archive,
      release_manifest: ($artifact_dir + "/release_manifest.json"),
      archive_file_hashes: ($artifact_dir + "/archive_file_hashes.json")
    },
    archive_sha256: $archive_sha256,
    readback: {
      artifact_dir_state: $artifact_dir_state,
      archive_state: $archive_state,
      release_manifest_status: $release_manifest[0].status,
      release_stage: $release_manifest[0].release_stage,
      blockers: $release_manifest[0].blockers
    }
  }' | tee "$artifact_dir/build_report.json"

rm -f "$tmp_reference_check" "$tmp_sif_inspect" "$tmp_sif_list" "$tmp_sif_verify"
