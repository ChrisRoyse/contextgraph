#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="${AIWONDER_REMOTE_HELPER:-$ROOT/scripts/ops/aiwonder_run_remote.sh}"
ENV_FILE="${AIWONDER_ENV_FILE:-$HOME/.config/aiwonder.env}"
REPO_URL="${CONTEXTGRAPH_REPO_URL:-https://github.com/ChrisRoyse/contextgraph.git}"
MIRROR="${CONTEXTGRAPH_AIWONDER_MIRROR:-/zfs/hot/contextgraph/git-mirrors/contextgraph.git}"
WORKTREE="${CONTEXTGRAPH_AIWONDER_WORKTREE:-/zfs/hot/contextgraph/repo-current}"
REFSPEC="${CONTEXTGRAPH_AIWONDER_REFSPEC:-+refs/heads/main:refs/heads/main}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/ops/sync_aiwonder_contextgraph_mirror.sh

Environment:
  AIWONDER_ENV_FILE                 Local aiwonder env file. Default: ~/.config/aiwonder.env
  AIWONDER_REMOTE_HELPER            Local SSH helper. Default: scripts/ops/aiwonder_run_remote.sh
  CONTEXTGRAPH_REPO_URL             GitHub repo URL. Default: https://github.com/ChrisRoyse/contextgraph.git
  CONTEXTGRAPH_AIWONDER_MIRROR      Remote bare mirror path.
  CONTEXTGRAPH_AIWONDER_WORKTREE    Remote repo-current worktree/symlink path.
  CONTEXTGRAPH_AIWONDER_REFSPEC     Remote fetch refspec. Default: +refs/heads/main:refs/heads/main

Requires AIWONDER_SUDO_PASSWORD locally. The GitHub PAT is read on aiwonder
from Infisical via /etc/leapable/secrets-loader.env and is passed only to the
git fetch child process through GIT_CONFIG_VALUE_0.
USAGE
}

case "${1:-}" in
  "" )
    ;;
  -h|--help|help )
    usage
    exit 0
    ;;
  * )
    usage >&2
    echo "AIWONDER_CONTEXTGRAPH_MIRROR_ACTION_UNKNOWN: $1" >&2
    exit 2
    ;;
esac

if [[ ! -x "$HELPER" ]]; then
  echo "AIWONDER_REMOTE_HELPER_MISSING_OR_NOT_EXECUTABLE: $HELPER" >&2
  exit 2
fi

if [[ ! -f "$ENV_FILE" ]]; then
  echo "AIWONDER_ENV_MISSING: $ENV_FILE" >&2
  exit 2
fi

set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

if [[ -z "${AIWONDER_SUDO_PASSWORD:-}" ]]; then
  echo "AIWONDER_SUDO_PASSWORD_MISSING: $ENV_FILE" >&2
  exit 2
fi

# The body is evaluated on aiwonder after non-secret path variables are passed
# through sudo env.
# shellcheck disable=SC2016
root_script='
set -euo pipefail

repo_url="${CONTEXTGRAPH_REPO_URL:?}"
mirror="${CONTEXTGRAPH_AIWONDER_MIRROR:?}"
worktree="${CONTEXTGRAPH_AIWONDER_WORKTREE:?}"
refspec="${CONTEXTGRAPH_AIWONDER_REFSPEC:?}"
target_user="${SUDO_USER:-croyse}"
target_group="$(id -gn "$target_user")"

. /etc/leapable/secrets-loader.env
env_name="${INFISICAL_ENV:-prod}"
login_token="$(infisical login \
  --method=universal-auth \
  --client-id="$INFISICAL_UA_CLIENT_ID" \
  --client-secret="$INFISICAL_UA_CLIENT_SECRET" \
  --plain --silent)"
export INFISICAL_TOKEN="$login_token"
github_pat="$(infisical secrets get github_pat \
  --projectId="$INFISICAL_PROJECT_ID" \
  --env="$env_name" \
  --plain)"
test -n "$github_pat"
basic_auth="$(printf "x-access-token:%s" "$github_pat" | base64 | tr -d "\n")"

install -d -o "$target_user" -g "$target_group" "$(dirname "$mirror")"
if ! runuser -u "$target_user" -- git --git-dir "$mirror" rev-parse --git-dir >/dev/null 2>&1; then
  runuser -u "$target_user" -- git init --bare "$mirror" >/dev/null
  echo "mirror_initialized=$mirror"
else
  echo "mirror_exists=$mirror"
fi

runuser -u "$target_user" -- git -C "$mirror" config remote.origin.url "$repo_url"
runuser -u "$target_user" -- env \
  GIT_CONFIG_COUNT=1 \
  GIT_CONFIG_KEY_0="http.https://github.com/.extraheader" \
  GIT_CONFIG_VALUE_0="Authorization: Basic $basic_auth" \
  git -C "$mirror" fetch --prune origin "$refspec"
runuser -u "$target_user" -- git --git-dir "$mirror" symbolic-ref HEAD refs/heads/main

echo "mirror_head=$(runuser -u "$target_user" -- git --git-dir "$mirror" symbolic-ref HEAD)"
echo "mirror_main=$(runuser -u "$target_user" -- git --git-dir "$mirror" rev-parse refs/heads/main)"

if [[ -d "$worktree/.git" || -f "$worktree/.git" ]]; then
  runuser -u "$target_user" -- git -C "$worktree" remote set-url origin "$mirror"
  runuser -u "$target_user" -- git -C "$worktree" fetch origin main
  echo "worktree_origin=$(runuser -u "$target_user" -- git -C "$worktree" remote get-url origin)"
  echo "worktree_origin_main=$(runuser -u "$target_user" -- git -C "$worktree" rev-parse refs/remotes/origin/main)"
  runuser -u "$target_user" -- git -C "$worktree" status --short --branch
else
  echo "worktree_missing_or_not_git=$worktree"
fi

echo "CONTEXTGRAPH_AIWONDER_MIRROR_SYNC_OK"
'

remote_command=$(printf 'sudo -S -p "" env CONTEXTGRAPH_REPO_URL=%q CONTEXTGRAPH_AIWONDER_MIRROR=%q CONTEXTGRAPH_AIWONDER_WORKTREE=%q CONTEXTGRAPH_AIWONDER_REFSPEC=%q bash -lc %q' \
  "$REPO_URL" \
  "$MIRROR" \
  "$WORKTREE" \
  "$REFSPEC" \
  "$root_script")

printf '%s\n' "$AIWONDER_SUDO_PASSWORD" | "$HELPER" "$remote_command"
