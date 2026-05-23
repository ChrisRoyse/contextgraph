#!/usr/bin/env bash
set -euo pipefail

ENV_FILE="${AIWONDER_ENV_FILE:-$HOME/.config/aiwonder.env}"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "AIWONDER_ENV_MISSING: $ENV_FILE" >&2
  exit 2
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

if [[ $# -lt 1 ]]; then
  echo "AIWONDER_REMOTE_COMMAND_MISSING: pass one remote shell command" >&2
  exit 2
fi

REMOTE_BODY="$1"
if [[ -n "${AIWONDER_REMOTE_TIMEOUT_SECONDS:-}" ]]; then
  if ! [[ "$AIWONDER_REMOTE_TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [[ "$AIWONDER_REMOTE_TIMEOUT_SECONDS" -lt 1 ]]; then
    echo "AIWONDER_REMOTE_TIMEOUT_INVALID: $AIWONDER_REMOTE_TIMEOUT_SECONDS" >&2
    exit 2
  fi
  REMOTE_COMMAND="export PATH=/zfs/hot/contextgraph/bin:\$PATH; timeout --kill-after=5s ${AIWONDER_REMOTE_TIMEOUT_SECONDS}s bash -lc $(printf '%q' "$REMOTE_BODY")"
else
  REMOTE_COMMAND="export PATH=/zfs/hot/contextgraph/bin:\$PATH; $REMOTE_BODY"
fi
SSH_PORT="${AIWONDER_SSH_PORT:-22}"
SSH_USER="${AIWONDER_USER:-croyse}"
SSH_TARGET="${AIWONDER_HOST:-aiwonder.mst.com}"
HOST_RESOLVES=1

if ! getent hosts "$SSH_TARGET" >/dev/null 2>&1; then
  HOST_RESOLVES=0
fi

if [[ "$HOST_RESOLVES" -eq 0 && -n "${AIWONDER_IP:-}" ]]; then
  SSH_TARGET="$AIWONDER_IP"
fi

SSH_COMMON_OPTS=(
  -T
  -o StrictHostKeyChecking=accept-new
  -o ConnectTimeout=5
  -o ServerAliveInterval=15
  -o ServerAliveCountMax=2
)

if [[ -n "${AIWONDER_SSH_CMD:-}" && "$HOST_RESOLVES" -eq 1 ]]; then
  read -r -a SSH_CMD_PARTS <<< "$AIWONDER_SSH_CMD"
  if [[ "${SSH_CMD_PARTS[0]:-}" == "ssh" ]]; then
    exec "${SSH_CMD_PARTS[0]}" "${SSH_COMMON_OPTS[@]}" "${SSH_CMD_PARTS[@]:1}" "$REMOTE_COMMAND"
  fi
  exec "${SSH_CMD_PARTS[@]}" "$REMOTE_COMMAND"
fi

ASKPASS="/tmp/ssh-aiwonder/askpass.sh"
cat > "$ASKPASS" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
ENV_FILE="${AIWONDER_ENV_FILE:-$HOME/.config/aiwonder.env}"
set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a
printf '%s\n' "${AIWONDER_SSH_PASSWORD:-}"
EOF
chmod 700 "$ASKPASS"

export SSH_ASKPASS="$ASKPASS"
export SSH_ASKPASS_REQUIRE=force
export DISPLAY="${DISPLAY:-aiwonder-helper:0}"

exec setsid ssh \
  "${SSH_COMMON_OPTS[@]}" \
  -p "$SSH_PORT" \
  "$SSH_USER@$SSH_TARGET" \
  "$REMOTE_COMMAND"
