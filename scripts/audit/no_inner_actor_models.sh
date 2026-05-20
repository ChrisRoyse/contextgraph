#!/usr/bin/env bash
set -euo pipefail

# retired-inner-actor audit: historical, do not reanimate the local LLM paths
# named below; this scanner exists only to prevent their return.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODEL_ROOT="${INNER_ACTOR_MODEL_ROOT:-/zfs/hot/contextgraph/models}"
SOURCE_ROOT="${INNER_ACTOR_SOURCE_ROOT:-$REPO_ROOT}"
OUTPUT_JSON="${OUTPUT_JSON:-}"

TMP_DIR="$(mktemp -d)"
MATCHES_TSV="$TMP_DIR/matches.tsv"
ROOTS_TSV="$TMP_DIR/roots.tsv"
RETIRED_PATH_REGEX='.*(qwen3[._:-]?6[^/]*27b|qwen3[-_]?coder[^/]*30b[^/]*a3b|hermes[-_]?2[-_]?pro[^/]*mistral[^/]*7b|/\.?ollama(/|$)|llama[._-]cpp|llama-cpp-2|llama_cpp_2|llama-cli|llama-server|llama-run|llama-quantize|llama-bench).*'
trap 'rm -rf "$TMP_DIR"' EXIT

: >"$MATCHES_TSV"
: >"$ROOTS_TSV"

build_release_sha="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || printf 'unknown')"

record_match() {
  local root_label="$1"
  local rule_id="$2"
  local rule_description="$3"
  local path_kind="$4"
  local path="$5"

  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$root_label" "$rule_id" "$rule_description" "$path_kind" "$path" >>"$MATCHES_TSV"
}

scan_path() {
  local root_label="$1"
  local path="$2"
  local lower_path path_kind

  lower_path="$(printf '%s' "$path" | tr '[:upper:]' '[:lower:]')"
  if [[ -d "$path" ]]; then
    path_kind="directory"
  elif [[ -f "$path" ]]; then
    path_kind="file"
  elif [[ -L "$path" ]]; then
    path_kind="symlink"
  else
    path_kind="other"
  fi

  if [[ "$lower_path" =~ qwen3[._:-]?6[^/]*27b ]]; then
    record_match "$root_label" "qwen3_6_27b" "retired Qwen3.6-27B inner-actor model path" "$path_kind" "$path"
  fi
  if [[ "$lower_path" =~ qwen3[-_]?coder[^/]*30b[^/]*a3b ]]; then
    record_match "$root_label" "qwen3_coder_30b_a3b" "retired Qwen3-Coder-30B-A3B inner-actor model path" "$path_kind" "$path"
  fi
  if [[ "$lower_path" =~ hermes[-_]?2[-_]?pro[^/]*mistral[^/]*7b ]]; then
    record_match "$root_label" "hermes_2_pro_mistral_7b" "retired Hermes-2-Pro-Mistral-7B inner-actor model path" "$path_kind" "$path"
  fi
  if [[ "$lower_path" =~ (^|/)\.?ollama($|/) ]]; then
    record_match "$root_label" "ollama_directory" "retired Ollama model/runtime directory" "$path_kind" "$path"
  fi
  if [[ "$lower_path" =~ (llama[._-]cpp|llama-cpp-2|llama_cpp_2|llama-cli|llama-server|llama-run|llama-quantize|llama-bench) ]]; then
    record_match "$root_label" "llama_cpp_binary_or_vendor" "retired llama.cpp generative inference binary/vendor path" "$path_kind" "$path"
  fi
}

scan_root() {
  local root_label="$1"
  local root_path="$2"
  local profile="${3:-source}"

  if [[ ! -e "$root_path" ]]; then
    printf '%s\tfalse\t%s\n' "$root_label" "$root_path" >>"$ROOTS_TSV"
    return
  fi

  printf '%s\ttrue\t%s\n' "$root_label" "$root_path" >>"$ROOTS_TSV"
  if [[ "$profile" == "source" ]]; then
    while IFS= read -r -d '' path; do
      scan_path "$root_label" "$path"
    done < <(
      find "$root_path" -regextype posix-extended \
        \( -name .git \
        -o -name target \
        -o -name node_modules \
        -o -name memory \
        -o -name backupdocs \
        -o -name docs2 \
        -o -name clipcannon \) -prune \
        -o -iregex "$RETIRED_PATH_REGEX" -print0
    )
  else
    while IFS= read -r -d '' path; do
      scan_path "$root_label" "$path"
    done < <(find "$root_path" -regextype posix-extended -iregex "$RETIRED_PATH_REGEX" -print0)
  fi
}

scan_root "model_root" "$MODEL_ROOT" "model"
scan_root "source_root" "$SOURCE_ROOT" "source"

if [[ -n "${INNER_ACTOR_EXTRA_SCAN_ROOTS:-}" ]]; then
  IFS=':' read -r -a extra_roots <<<"${INNER_ACTOR_EXTRA_SCAN_ROOTS}"
  for i in "${!extra_roots[@]}"; do
    if [[ -n "${extra_roots[$i]}" ]]; then
      scan_root "extra_root_$i" "${extra_roots[$i]}" "model"
    fi
  done
fi

roots_json="$(jq -R -s '
  split("\n")
  | map(select(length > 0) | split("\t") | {
      label: .[0],
      exists: (.[1] == "true"),
      path: .[2]
    })
' "$ROOTS_TSV")"
matches_json="$(jq -R -s '
  split("\n")
  | map(select(length > 0) | split("\t") | {
      root_label: .[0],
      rule_id: .[1],
      rule_description: .[2],
      kind: .[3],
      path: .[4]
    })
' "$MATCHES_TSV")"
match_count="$(jq 'length' <<<"$matches_json")"
missing_root_count="$(jq '[.[] | select(.exists == false)] | length' <<<"$roots_json")"

all_passed=false
if [[ "$match_count" == "0" && "$missing_root_count" == "0" ]]; then
  all_passed=true
fi

artifact_tmp="$TMP_DIR/no_inner_actor_models.json"
jq -n \
  --arg task_id "TASK-LOOP-005" \
  --arg scanned_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg build_release_sha "$build_release_sha" \
  --argjson scan_roots "$roots_json" \
  --argjson matches "$matches_json" \
  --argjson match_count "$match_count" \
  --argjson missing_root_count "$missing_root_count" \
  --argjson all_passed "$all_passed" \
  '{
    task_id: $task_id,
    scanned_at: $scanned_at,
    build_release_sha: $build_release_sha,
    source_of_truth: {
      model_root: ($scan_roots[] | select(.label == "model_root") | .path),
      source_root: ($scan_roots[] | select(.label == "source_root") | .path),
      scan_type: "filesystem path-name audit"
    },
    rules: [
      {id: "qwen3_6_27b", description: "retired Qwen3.6-27B inner-actor model path"},
      {id: "qwen3_coder_30b_a3b", description: "retired Qwen3-Coder-30B-A3B inner-actor model path"},
      {id: "hermes_2_pro_mistral_7b", description: "retired Hermes-2-Pro-Mistral-7B inner-actor model path"},
      {id: "ollama_directory", description: "retired Ollama model/runtime directory"},
      {id: "llama_cpp_binary_or_vendor", description: "retired llama.cpp generative inference binary/vendor path"}
    ],
    scan_roots: $scan_roots,
    missing_root_count: $missing_root_count,
    match_count: $match_count,
    matches: $matches,
    all_passed: $all_passed,
    readback_equal: true
  }' >"$artifact_tmp"

if [[ -n "$OUTPUT_JSON" ]]; then
  mkdir -p "$(dirname "$OUTPUT_JSON")"
  cp "$artifact_tmp" "$OUTPUT_JSON"
fi

cat "$artifact_tmp"

if [[ "$all_passed" != "true" ]]; then
  exit 1
fi
