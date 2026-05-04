# Governance, Security, Privacy, And Risk

BlackBoxFix is now capable of turning verifier evidence into governance
decisions. These decisions are persisted as ledger rows and can be audited,
queried, scored, packaged, and used by release or risk workflows.

## Policy Gateway

The policy subsystem can:

- list capability registry entries;
- evaluate policy mode for a tool/capability;
- persist `policy_decision` rows;
- block unverified mutating actions in governed workflows;
- support modes such as observe-only, supervised, bounded autonomous, and
  blocked.

The MCP gateway uses this to govern `verified_git_apply_patch`.

## Enterprise Controls

Enterprise commands can persist and verify:

- tenant SSO/OIDC/SAML metadata;
- SCIM-style users/groups/roles;
- RBAC permission decisions;
- tenant isolation checks;
- retention class configuration;
- tenant key metadata;
- audit export authorization.

Record kinds:

- `enterprise_tenant_config`
- `enterprise_directory_snapshot`
- `enterprise_access_decision`

Export artifacts:

- `audit.jsonl`
- `siem-rfc5424.log`
- `controls.json`
- `manifest.json`

## Privacy

Privacy commands can:

- evaluate retention for one run and data subject;
- persist retention decision rows;
- evaluate erasure/deletion requests;
- approve or deny deletion requests;
- enforce retention classes and legal holds;
- require RBAC authorization;
- store data-subject digests rather than raw subject values.

Record kinds:

- `retention_decision`
- `evidence_deletion`

## Release Gates

Release gate evaluation can:

- require scorecards;
- require policy evidence;
- require privacy state;
- apply minimum trust thresholds;
- apply maximum risk thresholds;
- block when incidents exist;
- require RBAC authorization for release decisions;
- persist approved or blocked `release_gate` rows.

This creates an auditable release/promotion decision based on verified
evidence rather than a human summary alone.

## Scorecards

Scorecards can compute run-level:

- trust;
- risk;
- readiness;
- evidence quality;
- training/eval value.

Inputs include physical verifier/evidence/policy/incident/tool-call rows. The
scorecard is deterministic and persisted as a `scorecard` row.

## Risk And Open Questions

Risk assessment can:

- inspect physical ledger state;
- inspect SQLite runtime PRAGMAs;
- inspect package and dashboard rows/files;
- inspect incidents and eval-conversion state;
- require a real decision-register JSON file;
- enforce required open-question decisions;
- persist `risk_assessment` rows.

This supports pilot readiness and risk closure before moving forward.

## HTTP Request Audit

The local HTTP server can persist `http_request_audit` rows for protected
requests. It captures:

- request method/path;
- tenant;
- request id;
- auth outcome;
- response status;
- error code when applicable;
- source-of-truth references.

Invalid auth, tenant mismatch, schema errors, and oversized bodies are
observable in the ledger rather than hidden in logs only.

## Security Posture

Shipped security behaviors include:

- append-only ledger triggers;
- canonical JSON hashing;
- SHA-256 record and run chains;
- redaction before persistence;
- secret literal post-scan;
- typed errors with source-of-truth and field paths;
- fail-closed verification;
- RBAC-gated enterprise/privacy/release/package/risk operations;
- manifest hashes for generated file packages;
- no mock data requirement in FSV tests.
