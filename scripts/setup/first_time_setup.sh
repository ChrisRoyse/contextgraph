#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
DROOT="${CONTEXTGRAPH_DATA_ROOT:-/zfs/archive/contextgraph}"
CONFIG_ROOT="${MEJEPA_CONFIG_ROOT:-${HOME}/.config/mejepa}"
READBACK_ROOT="${DROOT}/runtime/setup-readback"
EVIDENCE="${READBACK_ROOT}/setup-readback.json"

require() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'MEJEPA_SETUP_MISSING_COMMAND: %s\n' "$1" >&2
    exit 1
  }
}

require cargo
require jq
require rg
require bash
require python3

mkdir -p \
  "${DROOT}/storage/contextgraph-rocksdb" \
  "${DROOT}/runtime/cgreality-shift-log" \
  "${DROOT}/runtime/test-outcomes" \
  "${DROOT}/logs" \
  "${DROOT}/state/locks" \
  "${DROOT}/state/schedulers" \
  "${DROOT}/state/cgreality" \
  "${CONFIG_ROOT}" \
  "${READBACK_ROOT}"

chmod 700 "${DROOT}" "${CONFIG_ROOT}"

install -m 600 "${ROOT}/config/mejepa/daemon.toml.example" "${CONFIG_ROOT}/daemon.toml"
install -m 600 "${ROOT}/config/mejepa/backup.toml.example" "${CONFIG_ROOT}/backup.toml"
install -m 600 "${ROOT}/config/mejepa/security.toml.example" "${CONFIG_ROOT}/security.toml"

"${ROOT}/scripts/ci/check_no_c_drive_paths.sh"
"${ROOT}/scripts/ci/hook_script_lint.sh"

created_at="$(date -u +%FT%TZ)"
root_mode="$(stat -c '%a' "${DROOT}")"
config_mode="$(stat -c '%a' "${CONFIG_ROOT}")"
jq -n \
  --arg status "ok" \
  --arg created_at "${created_at}" \
  --arg repo_root "${ROOT}" \
  --arg droot "${DROOT}" \
  --arg config_root "${CONFIG_ROOT}" \
  --arg root_mode "${root_mode}" \
  --arg config_mode "${config_mode}" \
  '{
    status: $status,
    created_at_utc: $created_at,
    source_of_truth: {
      repo_root: $repo_root,
      droot: $droot,
      config_root: $config_root
    },
    permissions: {
      droot_mode: $root_mode,
      config_root_mode: $config_mode
    },
    required_paths: [
      ($droot + "/storage/contextgraph-rocksdb"),
      ($droot + "/runtime/cgreality-shift-log"),
      ($droot + "/runtime/test-outcomes"),
      ($droot + "/state/locks"),
      ($config_root + "/daemon.toml"),
      ($config_root + "/backup.toml"),
      ($config_root + "/security.toml")
    ]
  }' > "${EVIDENCE}"

jq -e '
  .status == "ok"
  and .permissions.droot_mode == "700"
  and .permissions.config_root_mode == "700"
  and all(.required_paths[]; (input_filename as $ignored | true))
' "${EVIDENCE}" >/dev/null

while IFS= read -r path; do
  [ -e "${path}" ] || {
    printf 'MEJEPA_SETUP_READBACK_MISSING_PATH: %s\n' "${path}" >&2
    exit 1
  }
done < <(jq -r '.required_paths[]' "${EVIDENCE}")

printf 'MEJEPA_SETUP_OK evidence=%s\n' "${EVIDENCE}"
