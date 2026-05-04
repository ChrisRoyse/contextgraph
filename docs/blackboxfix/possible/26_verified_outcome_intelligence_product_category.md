# Verified Outcome Intelligence Product Category

## Category Statement

ContextGraph is now positioned as:

```text
Verified Outcome Intelligence for Tool-Using AI Agents
```

The investor-facing claim is:

```text
Bring any LLM. Route its tools through ContextGraph. Every action becomes verified evidence.
```

This is broader than a trace viewer. The system now materializes a product
category pack from real completed runs:

- model comparison arena;
- verified outcome leaderboard;
- failure injection studio;
- any-LLM connector contract;
- governance and compliance evidence map;
- tamper-evident signed attestation;
- reward compiler dashboard;
- controlled self-improvement loop design;
- buyer-facing executive view.

## Implemented Surface

The Gemma frontend exposes the product category layer in:

```text
gemmafrontend/
```

Current endpoints:

- `POST /api/chat` - Gemma chooses one ContextGraph tool, including
  `build_investor_category_pack`.
- `POST /api/category/build` - builds the category pack directly from real
  completed run roots.
- `GET /api/category/latest` - reads back the latest pack from disk and adds
  the pack file artifact hash dynamically.

Current UI:

- `Product` telemetry tab;
- `Investor pack` prompt chip;
- `Build Investor Pack` button;
- direct `#product` route for demo screenshots.

## Product Pack Source Of Truth

Default product pack root:

```text
/tmp/gemmafrontend-runs/product-category/<pack_id>/
```

Latest verified local pack:

```text
/tmp/gemmafrontend-runs/product-category/category_1777909648987202052_17b480eee5/product-category-pack.json
```

Latest browser screenshot:

```text
gemmafrontend/screenshots/product-category-ui-after.png
```

Latest manual FSV log:

```text
/tmp/gemmafrontend-fsv-2igz1vz9/manual-fsv.log
```

## Real Data Contract

The pack builder does not invent missing models or fake external systems.

If only Gemma has measured runs, the model arena shows only Gemma. If cost or
latency is not persisted by the current local run artifacts, those fields are
reported as `null` with a `not measured` note. External connector modes are
listed as contracts unless a real connector execution has occurred.

Missing completed runs, invalid audience, invalid `max_runs`, missing
OpenSSL, signing failure, or tamper test failure are blocking typed errors.

## Latest Measured Product Pack Values

The latest live Gemma tool-call run created:

```text
pack_id=category_1777909648987202052_17b480eee5
input_runs=2
models_measured=1
actions_attempted=2
incidents_caught=2
evidence_packets=4
training_episodes=8
average_reward=-2.765
attestation_status=verified
tamper_status=tamper_detected
```

The signed attestation verified successfully:

```text
stdout=Verified OK
exit_code=0
```

The tampered manifest failed verification:

```text
stdout=Verification failure
exit_code=1
```

## Full State Verification

Manual FSV reads and proves:

- frontend snapshot index before and after actions;
- direct SQLite ledger counts from `blackbox.sqlite#ledger_records`;
- Gemma selecting `inspect_latest_run` through native tool calling;
- product category pack creation under a physical directory;
- direct readback of `product-category-pack.json`;
- SHA-256 validation for every category artifact;
- signed attestation verification;
- tampered manifest verification failure;
- edge case no-mutation for empty prompts, missing roots, overlong prompts,
  empty category audience, `max_runs=0`, and `max_runs=13`.

Latest FSV evidence:

```text
CATEGORY_PACK_ID=category_1777909730184800387_e0bef50cf5
CATEGORY_INPUT_RUNS=1
CATEGORY_ATTESTATION_STATUS=verified
CATEGORY_TAMPER_STATUS=tamper_detected
EDGE_CATEGORY_EMPTY_AUDIENCE_ERROR=GEMMAFRONTEND_CATEGORY_AUDIENCE_INVALID
EDGE_CATEGORY_ZERO_RUNS_ERROR=GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID
EDGE_CATEGORY_TOO_MANY_RUNS_ERROR=GEMMAFRONTEND_CATEGORY_MAX_RUNS_INVALID
MANUAL_FSV_LOG=/tmp/gemmafrontend-fsv-2igz1vz9/manual-fsv.log
```

## Research Basis

- OpenTelemetry GenAI semantic conventions support model/tool telemetry and
  cross-system correlation.
- MCP tools define the model-requested, application-executed tool boundary
  that ContextGraph occupies.
- NIST AI RMF motivates govern/map/measure/manage evidence exports.
- W3C PROV and SLSA-style provenance motivate artifact lineage, hashes, and
  signed manifests.
- Ollama/Gemma tool calling provides the local native tool-call mechanism.

## Current Boundaries

- Model comparison is real but currently only has measured `gemma4:e4b` rows.
- Cost and latency are not yet persisted as first-class run metrics for local
  Ollama completed-run artifacts.
- External systems such as GitHub, Kubernetes, Terraform, Jira, Slack, and
  Postgres are represented as connector contracts until credentials and real
  executions are added.
- The product category pack is demo-scoped and signed by a local demo key
  under `/tmp/gemmafrontend-runs/attestation-keys/`.

## Next Implementation Targets

- Persist token/cost/latency in first-class `model_io` records.
- Add real external connector demos with credentials supplied through governed
  configuration.
- Run the same benchmark tasks across additional local or API-backed models.
- Promote signed evidence bundles to a dedicated attestation command and
  rotate keys through a real KMS or enterprise signing service.
