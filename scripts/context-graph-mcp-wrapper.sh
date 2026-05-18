#!/bin/bash
# context-graph-mcp-wrapper.sh — MCP server wrapper that auto-starts the daemon.
# Use this in .mcp.json instead of calling the binary directly.
# Ensures the daemon is running before launching the stdio MCP client.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="/home/cabdru/contextgraph/target/release/context-graph-mcp"
MODE="${CONTEXT_GRAPH_MCP_MODE:-reality-loop}"
DAEMON_PORT="${CONTEXT_GRAPH_DAEMON_PORT:-3100}"
DATA_ROOT="${CONTEXTGRAPH_DATA_ROOT:-/zfs/archive/contextgraph}"

# Ensure daemon is running (idempotent, blocks until ready)
"$SCRIPT_DIR/ensure_daemon.sh"
if [ $? -ne 0 ]; then
    echo '{"jsonrpc":"2.0","error":{"code":-1,"message":"Failed to start daemon"},"id":null}' >&2
    exit 1
fi

# Launch the stdio MCP client that connects to the daemon
exec "$BINARY" --mode "$MODE" --daemon --d-root "$DATA_ROOT" --daemon-port "$DAEMON_PORT"
