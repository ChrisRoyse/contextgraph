# Security, Governance, And Compliance

BlackBoxFix should be designed as a security and governance product from day
one. The product's value comes from proving that AI agent actions were
authorized, bounded, observable, and verified against real state.

## Governance Mapping

### NIST AI RMF

NIST AI RMF frames AI risk management around Govern, Map, Measure, and Manage.
BlackBoxFix maps naturally:

- Govern: policy registry, approvals, RBAC, audit evidence, tenant controls.
- Map: agent inventory, tool inventory, source-of-truth inventory, risk tiers.
- Measure: verifier pass rates, failed deltas, blocked actions, incident rates.
- Manage: remediation workflow, policy updates, eval generation, control export.

### OWASP GenAI Top 10

BlackBoxFix should directly address the agentic risks that appear in OWASP
guidance:

- Prompt injection: record untrusted inputs, policy context, and unexpected tool
  requests.
- Sensitive information disclosure: redact and tag sensitive fields before
  persistence.
- Supply chain: verify dependencies, tools, prompts, and model/runtime versions.
- Improper output handling: never treat model output as execution proof.
- Excessive agency: bind tools to explicit policies, approvals, and verifiers.
- Unbounded consumption: record costs, token use, tool loops, and rate limits.

### MITRE ATLAS

Use MITRE ATLAS as the adversarial AI threat taxonomy for incident labels and
security reporting. Incidents should map to tactics/techniques when applicable,
especially prompt injection, data leakage, model/tool abuse, and supply-chain
manipulation.

## Control Objectives

- Every agent has a stable identity.
- Every tool has a capability id and risk tier.
- Every mutating tool has an owner and verifier.
- Every high-risk action requires policy approval.
- Every governed action has source-of-truth readback.
- Every blocked or failed verifier emits a typed incident.
- Every evidence packet is tamper-evident.

## Policy Modes

- `observe_only`: record and verify, do not block.
- `supervised`: require human approval for configured actions.
- `bounded_autonomous`: allow actions within policy and verifier coverage.
- `blocked`: deny the action.

Policy mode is not a hidden runtime flag. It must be persisted on every
`PolicyDecision` and visible in the audit timeline.

## Secret And Sensitive Data Handling

Requirements:

- never store raw credentials;
- store credential key ids, not credential values;
- hash sensitive stable identifiers when full values are not needed;
- use field-level redaction for prompts and tool inputs;
- tag PII, secrets, customer data, and regulated data;
- preserve enough structure to debug failures without exposing sensitive data;
- make redaction deterministic for equality checks where possible.

## Authorization

Authorization should combine:

- tenant;
- human user;
- agent identity;
- tool capability;
- source-of-truth scope;
- data classification;
- runtime environment;
- risk tier;
- approval state.

Example denial:

```json
{
  "status": "denied",
  "error_code": "POLICY_CAPABILITY_SCOPE_DENIED",
  "field_path": "tool_input.repo",
  "remediation": "request repo write scope for this workflow or choose a read-only verifier"
}
```

## Audit Exports

Enterprise buyers will need:

- evidence packet PDF/JSON export;
- SIEM events;
- SOC investigation packet;
- risk dashboard;
- CSV export for control testing;
- immutable ledger range hash.

## Compliance Positioning

BlackBoxFix should not claim to make a customer compliant by itself. It should
claim to provide operational evidence that supports AI governance, security
monitoring, incident response, and audit controls.

## Enterprise Security Baseline

- SSO/SAML/OIDC.
- SCIM user provisioning.
- RBAC and service accounts.
- Tenant isolation.
- Customer-managed encryption keys for enterprise tier.
- Append-only audit ledger.
- Configurable retention.
- SIEM export.
- Data residency option.
- Private deployment option.

