#!/bin/bash
# ensure_daemon.sh — Ensures the Context Graph daemon is running on port 3100.
# Called automatically by the MCP server wrapper before connecting.
# Idempotent: safe to call repeatedly, only starts if needed.

PORT=3100
DATA_ROOT="${CONTEXTGRAPH_DATA_ROOT:-/mnt/d/contextgraph}"
STORAGE="${CONTEXT_GRAPH_STORAGE_PATH:-${DATA_ROOT}/storage/contextgraph-rocksdb}"
MODE="${CONTEXT_GRAPH_MCP_MODE:-reality-loop}"
MODELS="${CONTEXT_GRAPH_MODELS_PATH:-/mnt/d/cabdru/contextgraph/models}"
BINARY="/home/cabdru/contextgraph/target/release/context-graph-mcp"
LOGFILE="${DATA_ROOT}/logs/context-graph-daemon.log"
PIDFILE="${DATA_ROOT}/state/locks/context-graph-daemon.pid"
MAX_WAIT=300  # 5 minutes max wait for startup
MAX_LOG_BYTES=$((10 * 1024 * 1024))
LOG_BACKUPS=3

rotate_log_if_needed() {
    local path="$1"
    [ -f "$path" ] || return 0
    local size
    size=$(wc -c < "$path" 2>/dev/null || echo 0)
    [ "$size" -lt "$MAX_LOG_BYTES" ] && return 0

    rm -f "${path}.${LOG_BACKUPS}" 2>/dev/null || true
    local i
    for ((i=LOG_BACKUPS-1; i>=1; i--)); do
        [ -f "${path}.${i}" ] && mv "${path}.${i}" "${path}.$((i + 1))"
    done
    mv "$path" "${path}.1"
}

# Check if already listening
if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
    exit 0
fi

# Check if process exists but not yet listening (still loading)
if [ -f "$PIDFILE" ]; then
    PID=$(cat "$PIDFILE")
    if kill -0 "$PID" 2>/dev/null; then
        # Process alive, wait for it to start listening
        elapsed=0
        while [ $elapsed -lt $MAX_WAIT ]; do
            if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
                exit 0
            fi
            sleep 3
            elapsed=$((elapsed + 3))
        done
        echo "Daemon PID $PID alive but not listening after ${MAX_WAIT}s — killing" >&2
        kill -9 "$PID" 2>/dev/null
    fi
fi

# Verify binary exists
if [ ! -x "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY" >&2
    echo "Run: cd /home/cabdru/contextgraph && cargo build --release" >&2
    exit 1
fi

mkdir -p "$(dirname "$LOGFILE")" "$(dirname "$PIDFILE")" "$STORAGE"

# Start daemon
echo "[ensure_daemon] Starting context-graph daemon on port $PORT..." >&2
rotate_log_if_needed "$LOGFILE"
CONTEXTGRAPH_DATA_ROOT="$DATA_ROOT" \
CONTEXT_GRAPH_STORAGE_PATH="$STORAGE" \
CONTEXT_GRAPH_DB_PATH="$STORAGE" \
CONTEXT_GRAPH_MODELS_PATH="$MODELS" \
RUST_LOG=warn \
setsid "$BINARY" --mode "$MODE" --daemon-server-only --daemon-port "$PORT" >> "$LOGFILE" 2>&1 < /dev/null &
echo $! > "$PIDFILE"

# Wait for it to start listening
elapsed=0
while [ $elapsed -lt $MAX_WAIT ]; do
    if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
        echo "[ensure_daemon] Daemon ready on port $PORT (took ${elapsed}s)" >&2
        exit 0
    fi
    sleep 3
    elapsed=$((elapsed + 3))
done

echo "ERROR: Daemon failed to start within ${MAX_WAIT}s" >&2
echo "Check logs: $LOGFILE" >&2
exit 1
