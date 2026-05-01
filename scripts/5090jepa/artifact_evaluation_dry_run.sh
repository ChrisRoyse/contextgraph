#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/5090jepa/artifact_evaluation_dry_run.sh --aggregate PATH --reference PATH --output PATH [--json]

Runs a local, no-upload artifact-evaluation dry run against the release
aggregate source of truth.
USAGE
}

aggregate=""
reference=""
output=""
json=false

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
    --output)
      output="$2"
      shift 2
      ;;
    --json)
      json=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "ARTIFACT_EVAL_UNKNOWN_ARG: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$aggregate" ]] || { echo "ARTIFACT_EVAL_AGGREGATE_REQUIRED" >&2; exit 1; }
[[ -n "$reference" ]] || { echo "ARTIFACT_EVAL_REFERENCE_REQUIRED" >&2; exit 1; }
[[ -n "$output" ]] || { echo "ARTIFACT_EVAL_OUTPUT_REQUIRED" >&2; exit 1; }
[[ -f Dockerfile ]] || { echo "ARTIFACT_EVAL_DOCKERFILE_MISSING" >&2; exit 1; }
[[ -x reproduce.sh ]] || { echo "ARTIFACT_EVAL_REPRODUCE_NOT_EXECUTABLE" >&2; exit 1; }

tmp_verify="$(mktemp)"
if ! ./reproduce.sh verify-aggregate --aggregate "$aggregate" --reference "$reference" --json > "$tmp_verify"; then
  cat "$tmp_verify"
  rm -f "$tmp_verify"
  exit 1
fi

dockerfile_digest_refs="$(rg -n '^(FROM .*@sha256:|ARG CUDA_IMAGE=.*@sha256:)' Dockerfile || true)"
if [[ -z "$dockerfile_digest_refs" ]] || ! rg -q '^FROM \$\{CUDA_IMAGE\}$' Dockerfile; then
  echo "ARTIFACT_EVAL_DOCKERFILE_BASE_NOT_PINNED: Dockerfile must pin CUDA_IMAGE with @sha256 and use FROM \${CUDA_IMAGE}" >&2
  rm -f "$tmp_verify"
  exit 1
fi

mkdir -p "$(dirname "$output")"
jq -n \
  --arg operation "dynamicjepa_artifact_evaluation_dry_run" \
  --arg status "ok" \
  --arg aggregate "$aggregate" \
  --arg reference "$reference" \
  --arg dockerfile_sha256 "$(sha256sum Dockerfile | awk '{print $1}')" \
  --arg reproduce_sha256 "$(sha256sum reproduce.sh | awk '{print $1}')" \
  --argjson reference_check "$(cat "$tmp_verify")" \
  '{
    operation: $operation,
    status: $status,
    source_of_truth: {
      aggregate: $aggregate,
      reference: $reference,
      dockerfile: "Dockerfile",
      reproduce_script: "reproduce.sh"
    },
    checks: {
      dockerfile_base_pinned_by_digest: true,
      dockerfile_sha256: $dockerfile_sha256,
      reproduce_sha256: $reproduce_sha256,
      release_reference_check: $reference_check
    }
  }' | tee "$output"

rm -f "$tmp_verify"
