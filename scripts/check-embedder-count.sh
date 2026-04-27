#!/usr/bin/env bash
#
# TASK-S05: Embedder Count Validation Gate
#
# Verifies that the E14-era embedder/model counts are consistent.
# Constitution Reference: Multi-Array Storage pipeline (14 production embedders)
#
# Checks:
#   1. ModelId enum has exactly 15 variants (14 production + legacy Entity)
#   2. ModelId::all() returns exactly 15 items
#   3. ModelId::production() returns exactly 14 items
#   4. Maximum discriminant value is 14 (0-indexed)
#   5. Warm registry tracks all 15 loadable model IDs
#
# Exit codes:
#   0 - All embedder counts correct
#   1 - Embedder count mismatch detected
#
# Usage:
#   ./scripts/check-embedder-count.sh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

EXPECTED_MODEL_ID_COUNT=15
EXPECTED_PRODUCTION_COUNT=14
EXPECTED_WARM_COUNT=15

echo "========================================"
echo "  Embedder Count Check (E1-E14)"
echo "========================================"
echo ""
echo "Expected: $EXPECTED_PRODUCTION_COUNT production embedders, $EXPECTED_MODEL_ID_COUNT ModelId variants"
echo ""

# Change to repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Verify crates directory exists
if [ ! -d "crates" ]; then
    echo -e "${RED}ERROR: crates/ directory not found${NC}"
    echo "Expected repository structure with crates/ at root"
    exit 1
fi

ERRORS=0

# =============================================================================
# Check 1: Count ModelId enum variants
# =============================================================================
echo "Check 1: ModelId enum variant count..."

MODEL_ID_FILE="crates/context-graph-embeddings/src/types/model_id/core.rs"

if [ ! -f "$MODEL_ID_FILE" ]; then
    echo -e "${RED}ERROR: ModelId file not found: $MODEL_ID_FILE${NC}"
    exit 1
fi

# Count enum variants by looking for lines matching "VariantName = N," pattern
# This matches: "    Semantic = 0," etc.
VARIANT_COUNT=$(grep -cE '^\s+[A-Z][A-Za-z0-9]+ = [0-9]+,' "$MODEL_ID_FILE" 2>/dev/null || echo "0")

if [ "$VARIANT_COUNT" -eq "$EXPECTED_MODEL_ID_COUNT" ]; then
    echo -e "${GREEN}  PASS: Found $VARIANT_COUNT ModelId variants${NC}"
else
    echo -e "${RED}  FAIL: Found $VARIANT_COUNT ModelId variants (expected $EXPECTED_MODEL_ID_COUNT)${NC}"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Check 2: Verify maximum discriminant value
# =============================================================================
echo "Check 2: Maximum discriminant value..."

# Extract maximum discriminant (should be 14 for 0-indexed 15 variants)
MAX_DISCRIMINANT=$(grep -oE '= [0-9]+,' "$MODEL_ID_FILE" | grep -oE '[0-9]+' | sort -n | tail -1)

EXPECTED_MAX=$((EXPECTED_MODEL_ID_COUNT - 1))

if [ "$MAX_DISCRIMINANT" -eq "$EXPECTED_MAX" ]; then
    echo -e "${GREEN}  PASS: Maximum discriminant is $MAX_DISCRIMINANT (correct for $EXPECTED_MODEL_ID_COUNT variants)${NC}"
else
    echo -e "${RED}  FAIL: Maximum discriminant is $MAX_DISCRIMINANT (expected $EXPECTED_MAX)${NC}"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Check 3: Verify ModelId::all() array count
# =============================================================================
echo "Check 3: ModelId::all() array count..."

# Count items in the all() const fn array
# Find the function and count Self:: entries until the closing }
ALL_FN_START=$(grep -n 'pub const fn all()' "$MODEL_ID_FILE" | cut -d: -f1)
if [ -n "$ALL_FN_START" ]; then
    # Get the next 20 lines and count Self:: occurrences
    ALL_FN_COUNT=$(sed -n "${ALL_FN_START},$((ALL_FN_START + 24))p" "$MODEL_ID_FILE" | grep -cE 'Self::[A-Za-z0-9]+,' 2>/dev/null || echo "0")
else
    ALL_FN_COUNT=0
fi

if [ "$ALL_FN_COUNT" -eq "$EXPECTED_MODEL_ID_COUNT" ]; then
    echo -e "${GREEN}  PASS: ModelId::all() returns $ALL_FN_COUNT items${NC}"
else
    echo -e "${RED}  FAIL: ModelId::all() returns $ALL_FN_COUNT items (expected $EXPECTED_MODEL_ID_COUNT)${NC}"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Check 3b: Verify ModelId::production() array count
# =============================================================================
echo "Check 3b: ModelId::production() array count..."

PROD_FN_START=$(grep -n 'pub const fn production()' "$MODEL_ID_FILE" | cut -d: -f1)
if [ -n "$PROD_FN_START" ]; then
    PROD_FN_COUNT=$(sed -n "${PROD_FN_START},$((PROD_FN_START + 24))p" "$MODEL_ID_FILE" | grep -cE 'Self::[A-Za-z0-9]+,' 2>/dev/null || echo "0")
else
    PROD_FN_COUNT=0
fi

if [ "$PROD_FN_COUNT" -eq "$EXPECTED_PRODUCTION_COUNT" ]; then
    echo -e "${GREEN}  PASS: ModelId::production() returns $PROD_FN_COUNT items${NC}"
else
    echo -e "${RED}  FAIL: ModelId::production() returns $PROD_FN_COUNT items (expected $EXPECTED_PRODUCTION_COUNT)${NC}"
    ERRORS=$((ERRORS + 1))
fi

echo "Check 3c: ModelId::production() slot order..."
if [ -n "$PROD_FN_START" ]; then
    PROD_ORDER=$(awk -v start="$PROD_FN_START" '
        NR < start { next }
        /&\[/ { in_array = 1 }
        in_array { print }
        in_array && /^[[:space:]]*\]/ { exit }
    ' "$MODEL_ID_FILE" \
        | grep -oE 'Self::[A-Za-z0-9]+' \
        | sed 's/Self:://' \
        | paste -sd ',' -)
else
    PROD_ORDER=""
fi
EXPECTED_PROD_ORDER="Semantic,TemporalRecent,TemporalPeriodic,TemporalPositional,Causal,Sparse,Code,Graph,Hdc,Contextual,Kepler,LateInteraction,Splade,BgeM3Dense"
if [ "$PROD_ORDER" = "$EXPECTED_PROD_ORDER" ]; then
    echo -e "${GREEN}  PASS: production() order matches E1-E14 slots${NC}"
else
    echo -e "${RED}  FAIL: production() order mismatch${NC}"
    echo "        expected: $EXPECTED_PROD_ORDER"
    echo "        actual:   $PROD_ORDER"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Check 4: Verify dimension() match has all variants
# =============================================================================
echo "Check 4: dimension() match completeness..."

# Count match arms in dimension() function
# Find function start and count Self:: in the match block
DIM_FN_START=$(grep -n 'pub const fn dimension' "$MODEL_ID_FILE" | cut -d: -f1)
if [ -n "$DIM_FN_START" ]; then
    # Get the next 20 lines and count Self:: occurrences
    DIMENSION_ARMS=$(sed -n "${DIM_FN_START},$((DIM_FN_START + 20))p" "$MODEL_ID_FILE" | grep -c 'Self::' 2>/dev/null || echo "0")
else
    DIMENSION_ARMS=0
fi

if [ "$DIMENSION_ARMS" -eq "$EXPECTED_MODEL_ID_COUNT" ]; then
    echo -e "${GREEN}  PASS: dimension() covers $DIMENSION_ARMS variants${NC}"
else
    echo -e "${RED}  FAIL: dimension() covers $DIMENSION_ARMS variants (expected $EXPECTED_MODEL_ID_COUNT)${NC}"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Check 5: Verify warm loading registry matches
# =============================================================================
echo "Check 5: Warm loading registry count..."

WARM_REGISTRY_FILE="crates/context-graph-embeddings/src/warm/registry/types.rs"

if [ -f "$WARM_REGISTRY_FILE" ]; then
    # Check the array type annotation declares all loadable model IDs.
    WARM_TYPE_COUNT=$(grep -oE 'EMBEDDING_MODEL_IDS: \[&str; [0-9]+\]' "$WARM_REGISTRY_FILE" | grep -oE '[0-9]+' | head -1)

    if [ -n "$WARM_TYPE_COUNT" ] && [ "$WARM_TYPE_COUNT" -eq "$EXPECTED_WARM_COUNT" ]; then
        echo -e "${GREEN}  PASS: EMBEDDING_MODEL_IDS type declares $WARM_TYPE_COUNT entries${NC}"
    else
        echo -e "${RED}  FAIL: EMBEDDING_MODEL_IDS type declares ${WARM_TYPE_COUNT:-0} entries (expected $EXPECTED_WARM_COUNT)${NC}"
        ERRORS=$((ERRORS + 1))
    fi
else
    echo -e "${YELLOW}  SKIP: Warm registry file not found (optional)${NC}"
fi

# =============================================================================
# Check 6: Verify as_str() match has all variants
# =============================================================================
echo "Check 6: as_str() match completeness..."

# Find function start and count Self:: in the match block
STR_FN_START=$(grep -n 'pub const fn as_str' "$MODEL_ID_FILE" | cut -d: -f1)
if [ -n "$STR_FN_START" ]; then
    # Get the next 20 lines and count Self:: occurrences
    AS_STR_ARMS=$(sed -n "${STR_FN_START},$((STR_FN_START + 20))p" "$MODEL_ID_FILE" | grep -c 'Self::' 2>/dev/null || echo "0")
else
    AS_STR_ARMS=0
fi

if [ "$AS_STR_ARMS" -eq "$EXPECTED_MODEL_ID_COUNT" ]; then
    echo -e "${GREEN}  PASS: as_str() covers $AS_STR_ARMS variants${NC}"
else
    echo -e "${RED}  FAIL: as_str() covers $AS_STR_ARMS variants (expected $EXPECTED_MODEL_ID_COUNT)${NC}"
    ERRORS=$((ERRORS + 1))
fi

# =============================================================================
# Results
# =============================================================================
echo ""
echo "========================================"
echo "  Results"
echo "========================================"
echo ""

if [ "$ERRORS" -eq 0 ]; then
    echo -e "${GREEN}PASSED: All embedder count checks passed${NC}"
    echo ""
    echo "Verified:"
    echo "  - $EXPECTED_MODEL_ID_COUNT ModelId enum variants (14 production + legacy Entity)"
    echo "  - Discriminants 0-$EXPECTED_MAX correctly assigned"
    echo "  - ModelId::all() returns all $EXPECTED_MODEL_ID_COUNT variants"
    echo "  - ModelId::production() returns $EXPECTED_PRODUCTION_COUNT production variants"
    echo "  - dimension() covers all variants"
    echo "  - as_str() covers all variants"
    exit 0
else
    echo -e "${RED}FAILED: $ERRORS embedder count check(s) failed${NC}"
    echo ""
    echo "Expected: $EXPECTED_PRODUCTION_COUNT production embedders (E1-E14), $EXPECTED_MODEL_ID_COUNT ModelId variants"
    echo ""
    echo "ModelId variants should be:"
    echo "  E1:  Semantic          E8:  Graph"
    echo "  E2:  TemporalRecent    E9:  Hdc"
    echo "  E3:  TemporalPeriodic  E10: Multimodal"
    echo "  E4:  TemporalPositional E11: Kepler (Entity legacy retained)"
    echo "  E5:  Causal            E12: LateInteraction"
    echo "  E6:  Sparse            E13: Splade"
    echo "  E7:  Code              E14: BgeM3Dense"
    echo ""
    echo "See: constitution.yaml for model specifications"
    exit 1
fi
