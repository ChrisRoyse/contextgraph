# Phase 8 — Enterprise Controls And Audit Export FSV Tests

This document is the canonical reference for the **shipped** Phase 8
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| Tenant config | latest `enterprise_tenant_config` row for the tenant |
| SCIM directory | latest `enterprise_directory_snapshot` row for the tenant |
| RBAC / tenant isolation | `enterprise_access_decision` rows |
| Audit export | `audit.jsonl` under `--out` |
| SIEM export | `siem-rfc5424.log` under `--out` |
| Control packet | `controls.json` under `--out` |
| Export manifest | `manifest.json.files[]` hashes/byte counts matched against physical files |

Phase 8 bumps the live ledger schema to `LEDGER_SCHEMA_VERSION = 5`. Older
ledgers are wiped on `blackbox init`, never migrated in place.

## Shipped Commands

```text
blackbox enterprise configure --ledger PATH --tenant TENANT --input CONFIG --agent AGENT [--model MODEL]
blackbox enterprise sync-directory --ledger PATH --tenant TENANT --input SCIM_JSON --agent AGENT [--model MODEL]
blackbox enterprise access-evaluate --ledger PATH --tenant TENANT --principal-id PRINCIPAL --permission PERM --resource tenant:TENANT:RESOURCE --agent AGENT [--model MODEL]
blackbox enterprise export-audit --ledger PATH --tenant TENANT --principal-id PRINCIPAL --out DIR [--force] --agent AGENT [--model MODEL]
```

## Shipped Record Kinds

```text
enterprise_tenant_config
enterprise_directory_snapshot
enterprise_access_decision
```

## Invariants

- Tenant config must be present before a directory snapshot can be synced.
- Directory snapshot must be present before access can be evaluated.
- OIDC config requires issuer, client id, JWKS SHA-256, and required
  `iss`/`sub`/`aud`/`exp` claims.
- SAML config requires entity id, SSO URL, audience, and X.509 certificate
  SHA-256.
- Raw credential material and secret-like literals are refused before any
  enterprise row is written.
- SCIM snapshots must include User and Group schema URNs.
- Groups may reference only configured RBAC roles.
- Users may reference only synced groups.
- Resource locators must be tenant scoped as `tenant:<tenant_id>:<resource>`.
- Allowed and denied access decisions both persist as
  `enterprise_access_decision` rows.
- `export-audit` persists an `enterprise_access_decision` for the export
  authorization attempt before creating any files.
- Denied `export-audit` attempts create no files but still persist the denied
  access decision for auditability.
- Audit export success requires readback of every generated file and manifest
  hash verification against physical bytes.

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase8_enterprise_fsv -- --nocapture
```

Current result: 6/6 passing.

Coverage:

- happy path persists tenant config, SCIM directory, allowed access decision,
  and exports audit/SIEM/control/manifest files;
- invalid OIDC required claims fail before config persistence;
- unknown SCIM role assignment fails before directory persistence;
- cross-tenant resource persists a denied access decision;
- unauthorized audit export creates no files and persists a denied access
  decision;
- tampered ledger blocks audit export before files are written.

## Manual FSV Checklist

For a manual run, the expected source-of-truth reads are:

```bash
sqlite3 "$ledger" "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind;"
sqlite3 "$ledger" "SELECT json_extract(payload_json,'$.allowed'), json_extract(payload_json,'$.reason_code') FROM ledger_records WHERE record_kind='enterprise_access_decision' ORDER BY sequence;"
wc -l "$out/audit.jsonl" "$out/siem-rfc5424.log"
sha256sum "$out/audit.jsonl" "$out/siem-rfc5424.log" "$out/controls.json"
jq '.files' "$out/manifest.json"
```

The happy path should show one row each for `enterprise_tenant_config` and
`enterprise_directory_snapshot`, plus one explicit `access-evaluate` row and
one `export-audit` authorization row. The export line counts must match the
tenant row count at export time; every manifest hash must match the physical
file.

## Manual FSV Evidence (2026-05-02)

Evidence log:

```text
/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/fsv.log
```

Physical sources of truth:

```text
sqlite:/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/enterprise.sqlite#ledger_records
file:/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/audit-export#audit.jsonl
file:/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/audit-export#siem-rfc5424.log
file:/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/audit-export#controls.json
file:/tmp/bbx-phase0-8-hardening-fsv.gZyO3B/audit-export#manifest.json
```

Final physical SQLite state after the happy path plus edge cases:

```text
enterprise_tenant_config      1
enterprise_directory_snapshot 1
enterprise_access_decision    4
```

The four access decisions were:

```text
sequence=3 principal=user_alice allowed=true  reason_code=null resource=tenant:tenant_harden_phase8:audit/export
sequence=4 principal=user_alice allowed=true  reason_code=null resource=tenant:tenant_harden_phase8:audit/export
sequence=5 principal=user_alice allowed=false reason_code=TENANT_SCOPE_DENIED resource=tenant:tenant_harden_phase8_other:audit/export
sequence=6 principal=user_bob   allowed=false reason_code=RBAC_PERMISSION_DENIED resource=tenant:tenant_harden_phase8:audit/export
```

The audit export was created after the export authorization decision was
persisted, when the tenant had four rows. Both export streams contained four
lines:

```text
audit.jsonl        4 lines sha256:534250ac8f52374563047baefc78ebb5d69a68854f01771bb15122cfa3c36b62
siem-rfc5424.log   4 lines sha256:cc669f58bf40ea9671faf9a699bd487f9c376d2c9aa5050931624c6165a27b5f
controls.json      sha256:2e2cc33b60fbd86e4cb4d2872e774c85d1401a88cfad422544ae6747a134ebde
manifest.json      sha256:8dab0993d0fbcd732a60a58f06fec047d8243eca4ea7152aeb3d7512c025a8a8
```

Manual edge cases executed with before/after state in the log:

- invalid OIDC config missing `exp` → `ENTERPRISE_SSO_CONFIG_INVALID`; config
  row count unchanged;
- SCIM group referencing unknown role → `ENTERPRISE_DIRECTORY_INVALID`;
  directory row count unchanged;
- cross-tenant resource → persisted denied `enterprise_access_decision` with
  `TENANT_SCOPE_DENIED`;
- unauthorized viewer audit export → `ENTERPRISE_RBAC_DENIED`; no export
  directory created, and a denied `enterprise_access_decision` persisted with
  `RBAC_PERMISSION_DENIED`.

The same log also includes `blackbox connector test --connector
enterprise_controls_v1 --source <ledger>`, which read the physical ledger and
reported one tenant config, one directory snapshot, and four access decisions.
