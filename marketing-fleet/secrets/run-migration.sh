#!/usr/bin/env bash
# All-in-one Infisical migration for marketing-fleet.
#
# PREREQUISITE: you've already run `infisical login` and authenticated in the browser.
# Verify by running `infisical user get token` — should print a token without re-prompting.
#
# This script:
#   1. Runs `infisical init` (interactive — pick or create the project, pick env)
#   2. Runs setup-infisical.sh (pushes every key from .env.local to Infisical)
#   3. After successful push, optionally deletes .env.local
#
# Usage (from anywhere):
#   bash /home/cabdru/contextgraph/marketing-fleet/secrets/run-migration.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== Step 0: verify infisical auth ==="
if ! infisical user get token --plain >/dev/null 2>&1; then
    echo "ERROR: not logged in. Run \`infisical login\` first." >&2
    exit 1
fi
echo "Logged in: yes"
echo ""

echo "=== Step 1: infisical init (project link for marketing-fleet) ==="
if [[ -f .infisical.json ]]; then
    echo "Already initialized (.infisical.json exists); skipping init."
else
    echo "Interactive: create or pick project 'marketing-fleet', then pick env (recommended 'dev')."
    infisical init
fi
echo ""

echo "=== Step 2: push .env.local secrets to Infisical ==="
bash secrets/setup-infisical.sh
