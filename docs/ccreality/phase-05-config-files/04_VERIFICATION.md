# Phase 5 — Verification

## Canonical end-state block

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 5.V1 Both config files exist and are valid JSON ==="
jq -e . config/ccreality_governed_paths.json >/dev/null
jq -e . config/ccreality_recommendation_schema.json >/dev/null
echo "  pass"

echo "=== 5.V2 Governed-paths policy has expected shape ==="
jq -e '.allow | length > 0' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | length > 0' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | any(. == ".env")' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny | any(. | startswith("swebench/"))' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. | startswith("crates/context-graph-cli/"))' config/ccreality_governed_paths.json >/dev/null
echo "  pass"

echo "=== 5.V3 Recommendation schema has expected shape ==="
jq -e '.["$schema"] | startswith("https://json-schema.org/draft/")' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.["$id"]' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.title == "ccreality optimizer recommendation"' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.required | type == "array"' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.required | any(. == "shift_log_excerpt")' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.properties.status.enum | type == "array"' config/ccreality_recommendation_schema.json >/dev/null
echo "  pass"

echo "=== 5.V4 Sample recommendation exists ==="
test -f docs/ccreality/phase-05-config-files/sample_recommendation.json
jq -e . docs/ccreality/phase-05-config-files/sample_recommendation.json >/dev/null
echo "  pass"

echo "=== 5.V5 Sample validates against schema (python check) ==="
if command -v python3 >/dev/null && python3 -c "import jsonschema" 2>/dev/null; then
  python3 -c "
import json, jsonschema
schema = json.load(open('config/ccreality_recommendation_schema.json'))
sample = json.load(open('docs/ccreality/phase-05-config-files/sample_recommendation.json'))
jsonschema.Draft202012Validator.check_schema(schema)
jsonschema.validate(sample, schema)
"
  echo "  pass (python jsonschema)"
elif command -v ajv >/dev/null; then
  ajv validate -s config/ccreality_recommendation_schema.json -d docs/ccreality/phase-05-config-files/sample_recommendation.json --strict=false
  echo "  pass (ajv)"
else
  echo "  (warn: no python jsonschema or ajv available; skipping schema validation. Install with: pip install jsonschema)"
fi

echo "=== 5.V6 Schema rejects an obviously bad payload ==="
if command -v python3 >/dev/null && python3 -c "import jsonschema" 2>/dev/null; then
  cat > /tmp/bad-rec.json <<'EOF'
{ "status": "changed", "reason": "missing required fields" }
EOF
  if python3 -c "
import json, jsonschema, sys
schema = json.load(open('config/ccreality_recommendation_schema.json'))
bad = json.load(open('/tmp/bad-rec.json'))
try:
    jsonschema.validate(bad, schema)
    sys.exit(1)
except jsonschema.ValidationError:
    sys.exit(0)
"; then
    echo "  pass"
  else
    echo "  FAIL: schema accepted obviously-incomplete payload"
    exit 1
  fi
  rm -f /tmp/bad-rec.json
else
  echo "  (warn: skipping)"
fi

echo
echo "Phase 5 verification: ALL PASS"
```

## What to record

```bash
echo "phase-05: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```
