#!/usr/bin/env bash
# Move durable ContextGraph data from WSL home/cache/tmp locations to D:.
#
# This is intentionally fail-closed:
# - D root must exist.
# - rsync must be available.
# - every migrated directory is copied, dry-run verified, then replaced by a
#   symlink to the D-backed source of truth.

set -euo pipefail

DATA_ROOT="${CONTEXTGRAPH_DATA_ROOT:-/mnt/d/contextgraph}"
EXECUTE=0

usage() {
  cat >&2 <<'EOF'
usage: scripts/migrate_contextgraph_data_to_d.sh --execute

Copies durable ContextGraph data to /mnt/d/contextgraph, verifies the copy with
rsync --dry-run --delete, then replaces old source directories with symlinks.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --execute) EXECUTE=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [ "$EXECUTE" != "1" ]; then
  echo "refusing to migrate without --execute" >&2
  usage
  exit 2
fi

command -v rsync >/dev/null 2>&1 || {
  echo "rsync is required for copy verification" >&2
  exit 1
}

[ -d "$DATA_ROOT" ] || {
  echo "data root is missing: $DATA_ROOT" >&2
  exit 1
}

mkdir -p "$DATA_ROOT/logs" "$DATA_ROOT/cache" "$DATA_ROOT/state" "$DATA_ROOT/migrated/tmp"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
LOG_JSONL="$DATA_ROOT/logs/migration-$STAMP.jsonl"
SUMMARY_JSON="$DATA_ROOT/logs/migration-$STAMP.json"

path_state_json() {
  local path="$1"
  python3 - "$path" <<'PY'
import json
import os
import sys
from pathlib import Path

path = Path(sys.argv[1])
if path.is_symlink():
    print(json.dumps({
        "path": str(path),
        "exists": True,
        "kind": "symlink",
        "target": os.readlink(path),
        "resolved": str(path.resolve()),
    }, sort_keys=True))
elif path.is_file():
    stat = path.stat()
    print(json.dumps({
        "path": str(path),
        "exists": True,
        "kind": "file",
        "size_bytes": stat.st_size,
    }, sort_keys=True))
elif path.is_dir():
    total = 0
    files = 0
    for root, _, names in os.walk(path):
        for name in names:
            p = Path(root) / name
            try:
                total += p.stat().st_size
                files += 1
            except OSError:
                pass
    print(json.dumps({
        "path": str(path),
        "exists": True,
        "kind": "directory",
        "file_count": files,
        "size_bytes": total,
    }, sort_keys=True))
else:
    print(json.dumps({"path": str(path), "exists": False}, sort_keys=True))
PY
}

emit_event() {
  local event="$1"
  local label="$2"
  local src="$3"
  local dst="$4"
  local details="${5:-}"
  if [ -z "$details" ]; then
    details="{}"
  fi
  python3 - "$event" "$label" "$src" "$dst" "$details" "$STAMP" "$LOG_JSONL" <<'PY'
import json
import sys
from pathlib import Path

event, label, src, dst, details_raw, stamp, log_jsonl = sys.argv[1:]
payload = {
    "event": event,
    "label": label,
    "source": src,
    "target": dst,
    "details": json.loads(details_raw),
    "timestamp": stamp,
}
line = json.dumps(payload, sort_keys=True)
Path(log_jsonl).open("a", encoding="utf-8").write(line + "\n")
print(line)
PY
}

verify_rsync_clean() {
  local src="$1"
  local dst="$2"
  local diff
  diff="$(rsync -a --delete --dry-run --itemize-changes "$src"/ "$dst"/)"
  if [ -n "$diff" ]; then
    emit_event "verify_failed" "$LABEL" "$src" "$dst" "$(printf '%s' "$diff" | python3 -c '
import json
import sys
print(json.dumps({"rsync_dry_run": sys.stdin.read()}))
')"
    echo "copy verification failed for $src -> $dst" >&2
    echo "$diff" >&2
    exit 1
  fi
}

migrate_dir() {
  LABEL="$1"
  local src="$2"
  local dst="$3"
  if [ ! -e "$src" ] && [ ! -L "$src" ]; then
    emit_event "source_absent" "$LABEL" "$src" "$dst" "{}"
    return 0
  fi

  if [ -L "$src" ]; then
    local resolved
    resolved="$(readlink -f "$src")"
    if [ "$resolved" = "$dst" ]; then
      emit_event "already_symlinked" "$LABEL" "$src" "$dst" "$(path_state_json "$src")"
      return 0
    fi
    echo "source symlink resolves elsewhere: $src -> $resolved (expected $dst)" >&2
    exit 1
  fi

  [ -d "$src" ] || {
    echo "source exists but is not a directory: $src" >&2
    exit 1
  }
  case "$dst" in
    "$DATA_ROOT"/*) ;;
    *) echo "target is outside data root: $dst" >&2; exit 1 ;;
  esac
  case "$dst" in
    "$src"|"$src"/*) echo "target must not be inside source: $src -> $dst" >&2; exit 1 ;;
  esac

  mkdir -p "$dst"
  emit_event "copy_start" "$LABEL" "$src" "$dst" "$(path_state_json "$src")"
  rsync -a --delete "$src"/ "$dst"/
  verify_rsync_clean "$src" "$dst"
  emit_event "copy_verified" "$LABEL" "$src" "$dst" "$(path_state_json "$dst")"

  rm -rf "$src"
  mkdir -p "$(dirname "$src")"
  ln -s "$dst" "$src"
  emit_event "source_replaced_with_symlink" "$LABEL" "$src" "$dst" "$(path_state_json "$src")"
}

migrate_tmp_dir() {
  local src="$1"
  [ -d "$src" ] || return 0
  local name
  name="$(basename "$src")"
  migrate_dir "tmp:$name" "$src" "$DATA_ROOT/migrated/tmp/$name"
}

migrate_dir "contextgraph-home" "/home/cabdru/.contextgraph" "$DATA_ROOT/user-data"
migrate_dir "mejepa-corpus-cache" "/home/cabdru/.cache/contextgraph-mejepa-corpus" "$DATA_ROOT/cache/mejepa-corpus"
migrate_dir "swebench-cache" "/home/cabdru/.cache/contextgraph-swebench" "$DATA_ROOT/cache/contextgraph-swebench"
migrate_dir "cgreality-state" "/home/cabdru/.cache/cgreality" "$DATA_ROOT/state/cgreality"
migrate_dir "docker-config-cache" "/home/cabdru/.cache/contextgraph-docker-config" "$DATA_ROOT/cache/contextgraph-docker-config"

for src in /tmp/contextgraph* /tmp/cgreality*; do
  [ -e "$src" ] || continue
  [ -d "$src" ] || continue
  migrate_tmp_dir "$src"
done

python3 - "$LOG_JSONL" "$SUMMARY_JSON" <<'PY'
import json
import sys
from collections import Counter
from pathlib import Path

log = Path(sys.argv[1])
summary = Path(sys.argv[2])
events = [json.loads(line) for line in log.read_text().splitlines() if line.strip()]
payload = {
    "status": "ok",
    "event_count": len(events),
    "events_by_kind": dict(Counter(event["event"] for event in events)),
    "log_jsonl": str(log),
    "source_of_truth": "/mnt/d/contextgraph",
}
summary.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
print(json.dumps(payload, indent=2, sort_keys=True))
PY
