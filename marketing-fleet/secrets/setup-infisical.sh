#!/usr/bin/env bash
# Infisical bootstrap for marketing-fleet.
# Run once after `infisical login` (interactive browser flow).
# Reads secrets from ../.env.local and pushes them to Infisical.
# After successful push, prompts to delete .env.local.
#
# Prerequisites:
#   - infisical CLI installed (v0.43+): https://infisical.com/docs/cli/overview
#   - infisical login completed: `infisical login`
#   - marketing-fleet/.env.local exists with values filled in
#
# Usage:
#   cd marketing-fleet/secrets
#   bash setup-infisical.sh

set -euo pipefail

cd "$(dirname "$0")/.."

ENV_FILE=".env.local"
PROJECT_NAME="marketing-fleet"
ENVIRONMENT="${INFISICAL_ENV:-dev}"

if [[ ! -f "$ENV_FILE" ]]; then
    echo "ERROR: $ENV_FILE not found. Copy .env.example to .env.local first." >&2
    exit 1
fi

# Verify infisical login
if ! infisical user get token >/dev/null 2>&1; then
    echo "ERROR: not logged in to Infisical. Run: infisical login" >&2
    exit 1
fi

# Verify project initialized; if not, prompt operator
if [[ ! -f ".infisical.json" ]]; then
    echo "No .infisical.json found. Initializing project link..."
    echo "Run interactively: infisical init"
    echo "Then re-run this script."
    exit 1
fi

# Push every key=value from .env.local (skipping comments + blanks)
pushed=0
skipped=0
while IFS='=' read -r key value || [[ -n "$key" ]]; do
    # Skip comments and blank lines
    [[ "$key" =~ ^#.*$ ]] && continue
    [[ -z "${key// }" ]] && continue
    # Skip empty values
    if [[ -z "${value// }" ]]; then
        echo "SKIP: $key (empty value)"
        ((skipped++))
        continue
    fi
    echo "PUSH: $key"
    infisical secrets set "$key=$value" --env="$ENVIRONMENT" >/dev/null
    ((pushed++))
done < "$ENV_FILE"

echo ""
echo "Pushed $pushed secrets to Infisical project '$PROJECT_NAME' env '$ENVIRONMENT'"
echo "Skipped $skipped empty-value entries"
echo ""
echo "Verify with: infisical secrets --env=$ENVIRONMENT"
echo ""
read -r -p "Delete $ENV_FILE now? (y/N) " confirm
if [[ "$confirm" =~ ^[Yy]$ ]]; then
    rm -f "$ENV_FILE"
    echo "Deleted $ENV_FILE"
    echo "From now on, run the fleet with: infisical run --env=$ENVIRONMENT -- <command>"
else
    echo "Kept $ENV_FILE. Delete it manually when ready."
fi
