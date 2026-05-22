#!/usr/bin/env bash
set -euo pipefail

ARCHIVE_ROOT="${ARCHIVE_ROOT:-/zfs/archive/contextgraph}"
HOT_ROOT="${HOT_ROOT:-/zfs/hot/contextgraph}"
ARCHIVE_DATASET="${ARCHIVE_DATASET:-archive/contextgraph}"
HOT_DATASET="${HOT_DATASET:-hotpool/contextgraph}"
REPORT_BASE="${REPORT_BASE:-/tmp/contextgraph-zfs-root-migration}"
TS="${TS:-$(date -u +%Y%m%dT%H%M%SZ)}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/setup/aiwonder_contextgraph_zfs_roots.sh audit
  I_UNDERSTAND_CONTEXTGRAPH_DOWNTIME=1 sudo -E scripts/setup/aiwonder_contextgraph_zfs_roots.sh migrate

Purpose:
  Repair aiwonder ContextGraph roots so:
    /zfs/archive/contextgraph is mounted from ZFS dataset archive/contextgraph
    /zfs/hot/contextgraph     is mounted from ZFS dataset hotpool/contextgraph

The audit subcommand is read-only and can run without root.
The migrate subcommand is intentionally root-only and retains ext4 backups.
USAGE
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

mount_field() {
  local path="$1"
  local field="$2"
  findmnt -T "$path" -n -o "$field" 2>/dev/null || true
}

print_mount() {
  local path="$1"
  printf '%s\t%s\t%s\n' "$path" "$(mount_field "$path" SOURCE)" "$(mount_field "$path" FSTYPE)"
}

write_key_manifest() {
  local output="$1"
  shift
  : >"$output"
  local path
  for path in "$@"; do
    if [[ -f "$path" ]]; then
      sha256sum "$path" >>"$output"
    else
      printf 'MISSING  %s\n' "$path" >>"$output"
    fi
  done
}

key_artifacts() {
  cat <<EOF
$ARCHIVE_ROOT/corpus/swebench-lite-python-300x8-v1-20260511/index.json
$ARCHIVE_ROOT/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl
$HOT_ROOT/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl
EOF
}

audit() {
  need_cmd findmnt
  need_cmd df
  need_cmd du
  need_cmd sha256sum

  local report_dir="$REPORT_BASE/audit-$TS"
  mkdir -p "$report_dir"

  {
    printf 'timestamp=%s\n' "$TS"
    printf 'hostname=%s\n' "$(hostname)"
    printf '\n== zfs ==\n'
    if command -v zpool >/dev/null 2>&1; then zpool list; else printf 'zpool unavailable\n'; fi
    if command -v zfs >/dev/null 2>&1; then zfs list -o name,mountpoint,used,avail,refer; else printf 'zfs unavailable\n'; fi
    printf '\n== mounts ==\n'
    print_mount /archive
    print_mount /hotpool
    print_mount "$ARCHIVE_ROOT"
    print_mount "$HOT_ROOT"
    printf '\n== disk ==\n'
    df -B1 / /archive /hotpool "$ARCHIVE_ROOT" "$HOT_ROOT" 2>&1
    printf '\n== sizes ==\n'
    du -sh "$ARCHIVE_ROOT" "$HOT_ROOT" /archive /hotpool 2>&1
    printf '\n== key hashes ==\n'
    mapfile -t artifacts < <(key_artifacts)
    write_key_manifest "$report_dir/key-artifacts.sha256" "${artifacts[@]}"
    cat "$report_dir/key-artifacts.sha256"
  } | tee "$report_dir/audit.txt"

  local archive_source archive_fstype hot_source hot_fstype
  archive_source="$(mount_field "$ARCHIVE_ROOT" SOURCE)"
  archive_fstype="$(mount_field "$ARCHIVE_ROOT" FSTYPE)"
  hot_source="$(mount_field "$HOT_ROOT" SOURCE)"
  hot_fstype="$(mount_field "$HOT_ROOT" FSTYPE)"

  if [[ "$archive_source" == "$ARCHIVE_DATASET" && "$archive_fstype" == "zfs" &&
        "$hot_source" == "$HOT_DATASET" && "$hot_fstype" == "zfs" ]]; then
    printf 'VERDICT: ContextGraph roots are on intended ZFS datasets.\n'
    exit 0
  fi

  printf 'VERDICT: ContextGraph roots are not on intended ZFS datasets. See %s/audit.txt\n' "$report_dir"
  exit 2
}

copy_and_verify() {
  local source_dir="$1"
  local target_dir="$2"
  local verify_log="$3"

  need_cmd rsync
  rsync -aHAX --numeric-ids --info=stats2 "$source_dir"/ "$target_dir"/
  rsync -aHAXn --checksum --delete --numeric-ids --itemize-changes "$source_dir"/ "$target_dir"/ >"$verify_log"
  if [[ -s "$verify_log" ]]; then
    cat "$verify_log" >&2
    die "post-copy rsync checksum verification found differences: $verify_log"
  fi
}

migrate_one_root() {
  local root_path="$1"
  local dataset="$2"
  local label="$3"
  local report_dir="$4"

  local current_source current_fstype backup_path
  current_source="$(mount_field "$root_path" SOURCE)"
  current_fstype="$(mount_field "$root_path" FSTYPE)"

  if [[ "$current_source" == "$dataset" && "$current_fstype" == "zfs" ]]; then
    printf '%s already mounted from %s; skipping.\n' "$root_path" "$dataset"
    return 0
  fi

  [[ "$current_fstype" == "ext4" ]] || die "$root_path is mounted from unexpected fstype '$current_fstype'"
  [[ -d "$root_path" ]] || die "$root_path does not exist"
  zfs list "$dataset" >/dev/null 2>&1 && die "dataset already exists: $dataset"

  backup_path="${root_path}.ext4-pre-zfs-${TS}"
  [[ ! -e "$backup_path" ]] || die "backup path already exists: $backup_path"

  printf 'Renaming %s to %s\n' "$root_path" "$backup_path"
  mv "$root_path" "$backup_path"
  mkdir -p "$root_path"
  chown --reference="$backup_path" "$root_path"
  chmod --reference="$backup_path" "$root_path"

  printf 'Creating ZFS dataset %s mounted at %s\n' "$dataset" "$root_path"
  zfs create -o mountpoint="$root_path" "$dataset"
  zfs mount "$dataset" >/dev/null 2>&1 || true

  current_source="$(mount_field "$root_path" SOURCE)"
  current_fstype="$(mount_field "$root_path" FSTYPE)"
  [[ "$current_source" == "$dataset" && "$current_fstype" == "zfs" ]] ||
    die "$root_path did not mount from $dataset after zfs create"

  copy_and_verify "$backup_path" "$root_path" "$report_dir/${label}-rsync-checksum-verify.txt"
  printf 'Verified %s content after migration. Backup retained at %s\n' "$label" "$backup_path"
}

migrate() {
  [[ "$(id -u)" == "0" ]] || die "migrate must run as root"
  [[ "${I_UNDERSTAND_CONTEXTGRAPH_DOWNTIME:-}" == "1" ]] ||
    die "set I_UNDERSTAND_CONTEXTGRAPH_DOWNTIME=1 to acknowledge ContextGraph downtime/mount cutover"
  need_cmd zfs
  need_cmd findmnt
  need_cmd sha256sum

  local report_dir="$REPORT_BASE/migrate-$TS"
  mkdir -p "$report_dir"

  mapfile -t artifacts < <(key_artifacts)
  write_key_manifest "$report_dir/key-artifacts.before.sha256" "${artifacts[@]}"

  migrate_one_root "$ARCHIVE_ROOT" "$ARCHIVE_DATASET" archive "$report_dir"
  migrate_one_root "$HOT_ROOT" "$HOT_DATASET" hot "$report_dir"

  write_key_manifest "$report_dir/key-artifacts.after.sha256" "${artifacts[@]}"
  diff -u "$report_dir/key-artifacts.before.sha256" "$report_dir/key-artifacts.after.sha256" \
    >"$report_dir/key-artifacts.diff" ||
    die "key artifact hashes changed; see $report_dir/key-artifacts.diff"

  printf 'Migration complete. Evidence directory: %s\n' "$report_dir"
  printf 'Ext4 backups are intentionally retained until the operator approves cleanup.\n'
  audit
}

case "${1:-}" in
  audit)
    audit
    ;;
  migrate)
    migrate
    ;;
  -h|--help|help|'')
    usage
    ;;
  *)
    usage >&2
    die "unknown subcommand: $1"
    ;;
esac
