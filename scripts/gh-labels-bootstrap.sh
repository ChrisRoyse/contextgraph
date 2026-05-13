#!/usr/bin/env bash
# Apply labels from .github/labels.yml using gh CLI.
# Idempotent: creates missing labels, updates existing labels' color and description.
#
# Doctrine §3.17 — AI Coding Agent Doctrine label set.
# Requires: gh, yq (mikefarah/yq v4+).
set -euo pipefail

REPO="${REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
ROOT="$(git rev-parse --show-toplevel)"
LABELS_FILE="${LABELS_FILE:-$ROOT/.github/labels.yml}"

if ! command -v yq >/dev/null 2>&1; then
  echo "ERROR: yq (mikefarah/yq v4+) required. Install: https://github.com/mikefarah/yq" >&2
  exit 1
fi

if [[ ! -f "$LABELS_FILE" ]]; then
  echo "ERROR: labels file not found at $LABELS_FILE" >&2
  exit 1
fi

echo "Repo:        $REPO"
echo "Labels file: $LABELS_FILE"
echo

count=$(yq '. | length' "$LABELS_FILE")
echo "Found $count labels in $LABELS_FILE"
echo

existing="$(gh label list --repo "$REPO" --limit 200 --json name -q '.[].name')"

for i in $(seq 0 $((count - 1))); do
  name=$(yq ".[$i].name" "$LABELS_FILE")
  color=$(yq ".[$i].color" "$LABELS_FILE")
  description=$(yq ".[$i].description" "$LABELS_FILE")

  if printf '%s\n' "$existing" | grep -Fxq "$name"; then
    gh label edit "$name" --repo "$REPO" --color "$color" --description "$description" >/dev/null
    echo "  ~ updated  $name"
  else
    gh label create "$name" --repo "$REPO" --color "$color" --description "$description" >/dev/null
    echo "  + created  $name"
  fi
done

echo
echo "Done. Verify with: gh label list --repo $REPO"
