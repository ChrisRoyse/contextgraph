# Security, Governance, And Compliance

BlackBoxFix is a security and governance product from day one. The product's
value comes from proving that AI agent actions were authorized, bounded,
observable, and verified against real state.

## Governance Mapping

### NIST AI RMF

NIST AI RMF frames AI risk management around Govern, Map, Measure, and Manage.

| RMF function | BlackBoxFix surface |
|---|---|
| Govern | policy registry, approvals, RBAC, audit evidence, tenant controls |
| Map | agent inventory, tool inventory, source-of-truth inventory, risk tiers |
| Measure | verifier pass rates, failed deltas, blocked actions, incident rates |
| Manage | remediation workflow, policy updates, eval generation, control export |

Phase 1 contributes append-only audit evidence and the redaction policy
metadata; the remaining functions arrive with Phases 3–8.

### OWASP GenAI Top 10

| Risk | BlackBoxFix mitigation |
|---|---|
| Prompt injection | record untrusted inputs, policy context, unexpected tool requests |
| Sensitive information disclosure | `redact_v1` pre-persistence + post-scan (Phase 1) |
| Supply chain | verify dependencies, tools, prompts, model/runtime versions |
| Improper output handling | never treat model output as execution proof |
| Excessive agency | bind tools to explicit policies, approvals, verifiers |
| Unbounded consumption | record costs, token use, tool loops, rate limits |

### MITRE ATLAS

Use MITRE ATLAS as the adversarial AI threat taxonomy for incident labels and
security reporting. Incidents must map to tactics/techniques when applicable —
especially prompt injection, data leakage, model/tool abuse, and supply-chain
manipulation.

## Control Objectives

- Every agent has a stable identity.
- Every tool has a capability id and risk tier.
- Every mutating tool has an owner and verifier.
- Every high-risk action requires policy approval.
- Every governed action has source-of-truth readback.
- Every blocked or failed verifier emits a typed incident.
- Every evidence packet is tamper-evident. (Phase 1 satisfies this for the
  ledger via dual-chain SHA-256; Phase 4 extends it to evidence packets.)

## Policy Modes (Phase 6 shipped)

| Mode | Behavior |
|---|---|
| `observe_only` | record and verify, do not block |
| `supervised` | require human approval for configured actions |
| `bounded_autonomous` | allow actions within policy and verifier coverage |
| `blocked` | deny the action |

Policy mode must be persisted on every `PolicyDecision` and visible in the
audit timeline. It is **not** a hidden runtime flag. Phase 6 persists this as
`policy_decision` rows in `ledger_records`; the MCP gateway returns a typed tool
error and does not mutate the external source when mode is `blocked` or when a
mutating capability lacks verifier coverage.

## Secret And Sensitive Data Handling

Phase 1 already enforces:

- never store raw credentials (post-scan rejects unredacted secret-like
  literals);
- redacted values are written as `[REDACTED:sha256:<hex>]` so equality
  comparisons remain possible without exposing the value;
- `redaction_json` records every redacted JSON path and its digest;
- post-scan walks the redacted tree once more to catch any literal that
  escaped field-name detection.

Additional rules for Phase 2+:

- store credential key ids, not credential values;
- hash sensitive stable identifiers when full values are not needed;
- tag PII, secrets, customer data, and regulated data;
- preserve enough structure to debug failures without exposing sensitive data;
- make redaction deterministic for equality checks (already true in Phase 1
  because the digest is over canonical JSON).

## Authorization (Phase 6 shipped locally)

Authorization combines:

- tenant;
- human user;
- agent identity;
- tool capability;
- source-of-truth scope;
- data classification;
- runtime environment;
- risk tier;
- approval state.

The shipped local registry includes `verified_git_apply_patch` with capability
`local.git.apply_patch` and verifier `git_file_diff_matches_patch_v1`.
Unregistered capabilities and mutating calls without verifier ids are denied
fail-closed and persisted for audit.

Example denial:

```json
{
  "status": "denied",
  "error_code": "POLICY_CAPABILITY_SCOPE_DENIED",
  "field_path": "tool_input.repo",
  "remediation": "request repo write scope for this workflow or choose a read-only verifier"
}
```

## Audit Exports (Phase 8 shipped)

Enterprise buyers need:

- evidence packet JSON export (Phase 4);
- SIEM events (`blackbox enterprise export-audit` writes
  `siem-rfc5424.log`);
- SOC investigation packet source material (`audit.jsonl`, `controls.json`,
  and the append-only ledger);
- risk dashboard (Phase 7);
- CSV export for control testing (reserved; JSONL is shipped first);
- immutable ledger range hash (already computable from Phase 1's
  `ledger_tip_sha256` between two `inspect ledger` snapshots).

## Compliance Positioning

BlackBoxFix does **not** make a customer compliant by itself. It provides
operational evidence that supports AI governance, security monitoring,
incident response, and audit controls.

## Enterprise Security Baseline (Phase 8 shipped)

- SSO/SAML/OIDC: tenant config accepts `protocol=oidc` with issuer/client id,
  JWKS SHA-256, and required `iss`/`sub`/`aud`/`exp` claims, or
  `protocol=saml` with entity id, SSO URL, audience, and X.509 certificate
  SHA-256.
- SCIM user provisioning: directory snapshots require SCIM User and Group
  schema URNs, non-empty users/groups, and valid user/group references.
- RBAC and service accounts: roles and permissions are explicit in the tenant
  config; principals inherit roles only through synced groups.
- Tenant isolation: governed resources must be scoped as
  `tenant:<tenant_id>:<resource>`; cross-tenant attempts persist denied
  `enterprise_access_decision` rows.
- Audit export authorization: `blackbox enterprise export-audit` persists an
  `enterprise_access_decision` before creating files. Denied export attempts
  create no files but remain inspectable in the append-only ledger.
- Customer-managed encryption keys for enterprise tier: tenant config persists
  `tenant_key_store.customer_managed=true` and a tenant `kms_key_id`; raw key
  material is refused.
- Append-only audit ledger (Phase 1).
- Configurable retention: tenant config persists named retention classes and
  a default class.
- SIEM export: `siem-rfc5424.log` is generated and read back before success.
- Data residency option: tenant config persists region and data residency.
- Private deployment option: tenant config requires `deployment.mode=private_vpc`.

## FSV Of Redaction (Phase 1)

Synthetic happy path: a JSON object containing `api_key` is redacted; the
field path appears in `redaction_json.redacted_fields`; the digest appears in
`redaction_json.secret_digest_fields`; the persisted `payload_json` shows
`[REDACTED:sha256:...]`. Verified by
`phase1_happy_path_persists_and_inspects_physical_rows`.

Synthetic edge case: a JSON object whose only sensitive content is the literal
string `Bearer should-not-be-persisted` (i.e. NOT in a sensitive-named field).
Phase 1 must still refuse to write the row. Verified by
`phase1_redaction_violation_blocks_persistence`.
