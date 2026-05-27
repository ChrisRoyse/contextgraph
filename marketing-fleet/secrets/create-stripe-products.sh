#!/usr/bin/env bash
# Create the marketing-fleet's subscription products + recurring prices in Stripe.
# Run once after Stripe account is set up. Reads STRIPE_SECRET_KEY from .env.local.
# Output: marketing-fleet/state/stripe-products.json with all product + price IDs for agents to reference.
#
# Catalog (per docs/marketingplan/11_PRODUCT_PLAYBOOKS.md):
#   Leapable:  Subscriber $9/mo | Creator Pro $29/mo | Creator Plus $99/mo
#   Synapse:   Individual Pro $29/mo | Team $499/mo  (Enterprise = custom, not auto-created)
#   Polis:     Plus $9/mo | Host $29/mo | Curator $99/mo

set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -f .env.local ]]; then
    set -a; source .env.local; set +a
fi
: "${STRIPE_SECRET_KEY:?STRIPE_SECRET_KEY not set in .env.local}"

API=https://api.stripe.com/v1
AUTH="$STRIPE_SECRET_KEY"
mkdir -p state
OUT=state/stripe-products.json
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

declare -a CATALOG=(
    # product_name|product_description|monthly_price_usd|metadata_key=metadata_value
    "Leapable Subscriber|Unlimited Asks across all subscribed Vaults|9|product=leapable;tier=subscriber"
    "Leapable Creator Pro|Unlimited Vaults, marketplace promotion, analytics|29|product=leapable;tier=creator_pro"
    "Leapable Creator Plus|Featured placement, custom pricing, AI landing pages|99|product=leapable;tier=creator_plus"
    "Synapse Individual Pro|ViGEm controller, advanced hotkeys, encrypted session recording|29|product=synapse;tier=individual_pro"
    "Synapse Team|Shared playbooks, multi-machine fleet, audit trails|499|product=synapse;tier=team"
    "Polis Plus|Early access to high-density curated events; host-prize draws|9|product=polis;tier=plus"
    "Polis Host|Tools to host events in your city|29|product=polis;tier=host"
    "Polis Curator|Editorial-grade events with revenue share|99|product=polis;tier=curator"
)

echo "[" > "$OUT"
first=1
for entry in "${CATALOG[@]}"; do
    IFS='|' read -r name desc price meta <<<"$entry"

    # Build metadata POST args from "k=v;k=v" pairs
    meta_args=()
    IFS=';' read -ra pairs <<<"$meta"
    for kv in "${pairs[@]}"; do
        meta_args+=(-d "metadata[${kv%%=*}]=${kv#*=}")
    done

    echo "==> Product: $name (\$$price/mo)"

    # Create product
    product_json=$(curl -sf "$API/products" \
        -u "$AUTH:" \
        -d "name=$name" \
        -d "description=$desc" \
        "${meta_args[@]}")
    product_id=$(echo "$product_json" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")

    # Create recurring monthly price
    price_cents=$((price * 100))
    price_json=$(curl -sf "$API/prices" \
        -u "$AUTH:" \
        -d "product=$product_id" \
        -d "currency=usd" \
        -d "unit_amount=$price_cents" \
        -d "recurring[interval]=month")
    price_id=$(echo "$price_json" | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")

    # Append to output
    if [[ $first -eq 0 ]]; then echo "," >> "$OUT"; fi
    cat >>"$OUT" <<JSON
  {
    "name": "$name",
    "metadata": {$(IFS=';'; for kv in $meta; do echo -n "\"${kv%%=*}\":\"${kv#*=}\","; done | sed 's/,$//')},
    "product_id": "$product_id",
    "price_id": "$price_id",
    "monthly_usd": $price
  }
JSON
    first=0
done
echo "]" >> "$OUT"

echo ""
echo "Wrote $OUT"
python3 -c "import json; print(f\"Created {len(json.load(open('$OUT')))} products.\")"
