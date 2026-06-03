#!/usr/bin/env bash
set -euo pipefail

LDB_BIN="${CONTEXTGRAPH_LDB_BIN:-/zfs/hot/contextgraph/bin/ldb}"
ROCKSDB_LIB_DIR="${CONTEXTGRAPH_ROCKSDB_LIB_DIR:-/zfs/hot/contextgraph/lib}"
ROCKSDB_LIB="${ROCKSDB_LIB_DIR}/librocksdb.so.8.9"

if [[ $# -lt 1 ]]; then
  echo "AIWONDER_LDB_READBACK_ARGS_MISSING: pass ldb arguments, for example --db=<path> scan --column_family=<cf>" >&2
  exit 2
fi

if [[ ! -x "$LDB_BIN" ]]; then
  echo "AIWONDER_LDB_BINARY_MISSING: $LDB_BIN" >&2
  exit 2
fi

if [[ ! -f "$ROCKSDB_LIB" ]]; then
  echo "AIWONDER_ROCKSDB_LIBRARY_MISSING: $ROCKSDB_LIB" >&2
  exit 2
fi

export LD_LIBRARY_PATH="${ROCKSDB_LIB_DIR}:${LD_LIBRARY_PATH:-}"
exec "$LDB_BIN" "$@"
