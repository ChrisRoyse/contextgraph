#!/usr/bin/env bash
# Helpers for ccreality hook unit tests.
set -euo pipefail

CGREALITY_CACHE="${CGREALITY_CACHE_DIR:-${HOME}/.cache/cgreality}"
ACTIVE_ROOT_FILE="${CGREALITY_CACHE}/active_runtime_root"
ACTIVE_TARGET_FILE="${CGREALITY_CACHE}/active_target_instance"

assert_exit() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if [ "${actual}" != "${expected}" ]; then
    echo "FAIL [${label}]: expected exit ${expected}, got ${actual}" >&2
    return 1
  fi
}

assert_jq() {
  local input="$1"
  local expr="$2"
  local expected="$3"
  local label="$4"
  local actual
  if ! actual="$(jq -r "${expr}" <<<"${input}")"; then
    echo "FAIL [${label}]: jq failed for expression ${expr} against input: ${input}" >&2
    return 1
  fi
  if [ "${actual}" != "${expected}" ]; then
    echo "FAIL [${label}]: ${expr} expected '${expected}', got '${actual}'. input=${input}" >&2
    return 1
  fi
}

assert_file() {
  local path="$1"
  local label="$2"
  if [ ! -f "${path}" ]; then
    echo "FAIL [${label}]: missing file ${path}" >&2
    return 1
  fi
}

mktempdir() {
  mktemp -d "/tmp/cgreality-hook-test-XXXXXX"
}

backup_active_state() {
  mkdir -p "${CGREALITY_CACHE}"
  ACTIVE_ROOT_BACKUP=""
  ACTIVE_TARGET_BACKUP=""
  if [ -f "${ACTIVE_ROOT_FILE}" ]; then
    ACTIVE_ROOT_BACKUP="$(cat "${ACTIVE_ROOT_FILE}")"
  fi
  if [ -f "${ACTIVE_TARGET_FILE}" ]; then
    ACTIVE_TARGET_BACKUP="$(cat "${ACTIVE_TARGET_FILE}")"
  fi
  export ACTIVE_ROOT_BACKUP ACTIVE_TARGET_BACKUP
}

restore_active_state() {
  mkdir -p "${CGREALITY_CACHE}"
  if [ -n "${ACTIVE_ROOT_BACKUP:-}" ]; then
    printf '%s\n' "${ACTIVE_ROOT_BACKUP}" > "${ACTIVE_ROOT_FILE}"
  else
    rm -f "${ACTIVE_ROOT_FILE}"
  fi
  if [ -n "${ACTIVE_TARGET_BACKUP:-}" ]; then
    printf '%s\n' "${ACTIVE_TARGET_BACKUP}" > "${ACTIVE_TARGET_FILE}"
  else
    rm -f "${ACTIVE_TARGET_FILE}"
  fi
}

setup_real_smoke_runtime_root() {
  local root="$1"
  local label="$2"
  cargo build -p context-graph-cli --bin reality-loop --quiet
  ./target/debug/reality-loop smoke --runtime-root "${root}" --label "${label}" >/dev/null
  local summary
  summary="$(find "${root}" -name attempt-summary.json -type f -print -quit)"
  test -n "${summary}" || {
    echo "FAIL: smoke wrote no attempt-summary.json under ${root}" >&2
    return 1
  }
  dirname "${summary}"
}

build_ccreality_bins() {
  cargo build -p context-graph-cli --bin context-graph-cli --quiet
  cargo build -p context-graph-mcp --bin context-graph-mcp --quiet
}

active_target_from_attempt_dir() {
  local attempt_dir="$1"
  basename "$(dirname "${attempt_dir}")"
}
