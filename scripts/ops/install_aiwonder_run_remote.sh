#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE="${AIWONDER_HELPER_SOURCE:-$ROOT/scripts/ops/aiwonder_run_remote.sh}"
TARGET="${AIWONDER_HELPER_PATH:-/tmp/ssh-aiwonder/run-remote.sh}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/ops/install_aiwonder_run_remote.sh verify
  scripts/ops/install_aiwonder_run_remote.sh install

Environment:
  AIWONDER_HELPER_SOURCE  Override tracked helper template.
  AIWONDER_HELPER_PATH    Override live helper path.
USAGE
}

sha() {
  sha256sum "$1" | cut -d ' ' -f 1
}

mode() {
  stat -c '%a' "$1"
}

verify() {
  if [[ ! -f "$SOURCE" ]]; then
    echo "AIWONDER_HELPER_SOURCE_MISSING: $SOURCE" >&2
    exit 2
  fi
  if [[ ! -f "$TARGET" ]]; then
    echo "AIWONDER_HELPER_TARGET_MISSING: $TARGET" >&2
    exit 10
  fi
  local source_sha target_sha target_mode
  source_sha="$(sha "$SOURCE")"
  target_sha="$(sha "$TARGET")"
  target_mode="$(mode "$TARGET")"
  if [[ "$source_sha" != "$target_sha" ]]; then
    printf 'AIWONDER_HELPER_CHECKSUM_MISMATCH: source=%s target=%s source_sha=%s target_sha=%s\n' \
      "$SOURCE" "$TARGET" "$source_sha" "$target_sha" >&2
    exit 11
  fi
  if [[ "$target_mode" != "700" ]]; then
    printf 'AIWONDER_HELPER_MODE_MISMATCH: target=%s mode=%s expected=700\n' "$TARGET" "$target_mode" >&2
    exit 12
  fi
  printf 'AIWONDER_HELPER_VERIFY_OK source=%s target=%s sha256=%s mode=%s\n' "$SOURCE" "$TARGET" "$target_sha" "$target_mode"
}

install_helper() {
  if [[ ! -f "$SOURCE" ]]; then
    echo "AIWONDER_HELPER_SOURCE_MISSING: $SOURCE" >&2
    exit 2
  fi
  mkdir -p "$(dirname "$TARGET")"
  install -m 700 "$SOURCE" "$TARGET"
  verify
}

case "${1:-}" in
  verify)
    verify
    ;;
  install)
    install_helper
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage >&2
    echo "AIWONDER_HELPER_ACTION_UNKNOWN: ${1:-}" >&2
    exit 2
    ;;
esac
