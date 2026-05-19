#!/bin/bash
# ensure_daemon.sh — Ensures the Context Graph daemon is running on ports 3100/3101.
# Called automatically by the MCP server wrapper before connecting.
# Idempotent: safe to call repeatedly, only starts if needed.

PORT=3100
HTTP_PORT="${CONTEXT_GRAPH_HTTP_PORT:-3101}"
DATA_ROOT="${CONTEXTGRAPH_DATA_ROOT:-/zfs/archive/contextgraph}"
STORAGE="${CONTEXT_GRAPH_STORAGE_PATH:-${DATA_ROOT}/storage/contextgraph-rocksdb}"
MODE="${CONTEXT_GRAPH_MCP_MODE:-reality-loop}"
MODELS="${CONTEXT_GRAPH_MODELS_PATH:-/zfs/hot/contextgraph/models}"
BINARY="/home/cabdru/contextgraph/target/release/context-graph-mcp"
LOGFILE="${DATA_ROOT}/logs/context-graph-daemon.log"
PIDFILE="${DATA_ROOT}/state/locks/context-graph-daemon.pid"
LAUNCH_LOCKFILE="${DATA_ROOT}/state/locks/context-graph-daemon-launch.lock"
MAX_WAIT=300  # 5 minutes max wait for startup
MAX_LOG_BYTES=$((10 * 1024 * 1024))
LOG_BACKUPS=3

is_daemon_ready() {
    ss -tlnp 2>/dev/null | grep -q ":${PORT} " || return 1
    ss -tlnp 2>/dev/null | grep -q ":${HTTP_PORT} " || return 1
    curl -fsS -H "Accept: application/json" "http://127.0.0.1:${HTTP_PORT}/healthz" >/dev/null 2>&1
}

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

validate_production_data_root() {
    if [ -z "$DATA_ROOT" ]; then
        echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: CONTEXTGRAPH_DATA_ROOT is empty" >&2
        exit 1
    fi

    case "$DATA_ROOT" in
        /*) ;;
        *)
            echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: CONTEXTGRAPH_DATA_ROOT must be an absolute aiwonder path: $DATA_ROOT" >&2
            exit 1
            ;;
    esac

    case "$DATA_ROOT" in
        /zfs/archive/contextgraph|/zfs/archive/contextgraph/*) ;;
        /mnt/d/contextgraph|/mnt/d/contextgraph/*)
            echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: production daemon root cannot be local /mnt/d; move required data to aiwonder under /zfs/archive/contextgraph" >&2
            exit 1
            ;;
        *)
            echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: production daemon root must be under /zfs/archive/contextgraph: $DATA_ROOT" >&2
            exit 1
            ;;
    esac
}

terminate_daemon_pid() {
    local pid="$1"
    local elapsed=0
    echo "Daemon PID $pid alive but not listening after ${MAX_WAIT}s — sending SIGTERM" >&2
    kill -TERM "$pid" 2>/dev/null || return 0
    while [ $elapsed -lt 30 ]; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "Daemon PID $pid exited after SIGTERM" >&2
            return 0
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
    echo "MEJEPA_DAEMON_SHUTDOWN_STUCK: PID $pid ignored SIGTERM for 30s; sending SIGKILL" >&2
    kill -KILL "$pid" 2>/dev/null || true
}

validate_production_data_root

# Check if already listening
if is_daemon_ready; then
    exit 0
fi

# Check if process exists but not yet listening (still loading)
if [ -f "$PIDFILE" ]; then
    PID=$(cat "$PIDFILE")
    if kill -0 "$PID" 2>/dev/null; then
        # Process alive, wait for it to start listening
        elapsed=0
        while [ $elapsed -lt $MAX_WAIT ]; do
            if is_daemon_ready; then
                exit 0
            fi
            sleep 3
            elapsed=$((elapsed + 3))
        done
        terminate_daemon_pid "$PID"
    fi
fi

# Verify binary exists
if [ ! -x "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY" >&2
    echo "Run: cd /home/cabdru/contextgraph && cargo build --release" >&2
    exit 1
fi

if [ ! -d "$DATA_ROOT" ]; then
    echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: $DATA_ROOT does not exist" >&2
    exit 1
fi

if [ ! -w "$DATA_ROOT" ]; then
    echo "MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: $DATA_ROOT is not writable" >&2
    exit 1
fi

mkdir -p "$(dirname "$LOGFILE")" "$(dirname "$PIDFILE")"

exec 9>"$LAUNCH_LOCKFILE"
if ! flock -n 9; then
    elapsed=0
    while [ $elapsed -lt $MAX_WAIT ]; do
        if is_daemon_ready; then
            exit 0
        fi
        sleep 3
        elapsed=$((elapsed + 3))
    done
    echo "MEJEPA_DAEMON_LOCK_HELD: daemon launcher lock is held and daemon did not become ready within ${MAX_WAIT}s" >&2
    exit 1
fi

if is_daemon_ready; then
    exit 0
fi

# Start daemon
echo "[ensure_daemon] Starting context-graph daemon on TCP port $PORT and HTTP port $HTTP_PORT..." >&2
rotate_log_if_needed "$LOGFILE"
CONTEXTGRAPH_DATA_ROOT="$DATA_ROOT" \
CONTEXT_GRAPH_STORAGE_PATH="$STORAGE" \
CONTEXT_GRAPH_DB_PATH="$STORAGE" \
CONTEXT_GRAPH_MODELS_PATH="$MODELS" \
RUST_LOG=warn \
setsid "$BINARY" --mode "$MODE" --daemon-server-only --d-root "$DATA_ROOT" --daemon-port "$PORT" --http-port "$HTTP_PORT" >> "$LOGFILE" 2>&1 < /dev/null &
DAEMON_PID=$!

# Wait for it to start listening
elapsed=0
while [ $elapsed -lt $MAX_WAIT ]; do
    if is_daemon_ready; then
        if [ ! -s "$PIDFILE" ]; then
            echo "MEJEPA_DAEMON_PID_MISSING: daemon ready but $PIDFILE is missing or empty" >&2
            exit 1
        fi
        echo "[ensure_daemon] Daemon ready on TCP port $PORT and HTTP port $HTTP_PORT (took ${elapsed}s)" >&2
        exit 0
    fi
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo "ERROR: Daemon process $DAEMON_PID exited before becoming ready" >&2
        echo "Check logs: $LOGFILE" >&2
        exit 1
    fi
    sleep 3
    elapsed=$((elapsed + 3))
done

echo "ERROR: Daemon failed to start within ${MAX_WAIT}s" >&2
echo "Check logs: $LOGFILE" >&2
terminate_daemon_pid "$DAEMON_PID"
exit 1
