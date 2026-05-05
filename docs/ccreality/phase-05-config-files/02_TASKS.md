# Phase 5 — Tasks

## Task 5.1 — Confirm config directory exists

```bash
cd /home/cabdru/contextgraph
test -d config || mkdir -p config
ls config/
```

Expected: `config/` exists. (It already exists per the repo layout — `config/default.toml` is there.)

## Task 5.2 — Write `config/ccreality_governed_paths.json`

Use the exact JSON from [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`config/ccreality_governed_paths.json`".

Verify:
```bash
jq -e '.allow' config/ccreality_governed_paths.json >/dev/null
jq -e '.deny' config/ccreality_governed_paths.json >/dev/null
echo "exit=$?"
```
Expected: `exit=0`.

## Task 5.3 — Write `config/ccreality_recommendation_schema.json`

Use the exact JSON from [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`config/ccreality_recommendation_schema.json`". This is the JSON Schema from [../13_STRUCTURED_OUTPUT_SCHEMA.md](../13_STRUCTURED_OUTPUT_SCHEMA.md).

Verify:
```bash
jq -e '.["$schema"]' config/ccreality_recommendation_schema.json >/dev/null
jq -e '.title' config/ccreality_recommendation_schema.json >/dev/null
echo "exit=$?"
```

## Task 5.4 — Write `docs/ccreality/phase-05-config-files/sample_recommendation.json`

Use the worked example from [03_FILE_SPECS.md](03_FILE_SPECS.md) §"`sample_recommendation.json`".

This file is used by the Phase 5 verification AND by Phase 4's verification (clause 4.V11 sends a malformed payload; the sample is the negative-control reference).

Verify:
```bash
jq -e . docs/ccreality/phase-05-config-files/sample_recommendation.json >/dev/null
echo "exit=$?"
```

## Task 5.5 — Validate the schema is well-formed

If `python3` and `jsonschema` (Python library) are available:
```bash
python3 -c "
import json, jsonschema
s = json.load(open('config/ccreality_recommendation_schema.json'))
jsonschema.Draft202012Validator.check_schema(s)
print('schema is valid')
"
```

If python jsonschema is not installed:
```bash
pip install jsonschema
```

Or use a Rust one-shot:
```bash
cargo run --quiet --manifest-path crates/context-graph-mcp/Cargo.toml --example validate_schema -- config/ccreality_recommendation_schema.json
```
(Only works if such an example exists; can be added in Phase 4.)

If neither `python3 + jsonschema` nor a Rust validator is available, stop and install a validator before proceeding. Do not mark Phase 5 complete without schema validation; Phase 4 depends on this file as a hard contract.

## Task 5.6 — Validate the sample passes the schema

```bash
python3 -c "
import json, jsonschema
schema = json.load(open('config/ccreality_recommendation_schema.json'))
sample = json.load(open('docs/ccreality/phase-05-config-files/sample_recommendation.json'))
jsonschema.validate(sample, schema)
print('sample validates')
"
```

If this fails, either the schema or the sample is wrong. Fix whichever is inconsistent.

## Task 5.7 — Verify

Execute [04_VERIFICATION.md](04_VERIFICATION.md). All three clauses pass.
