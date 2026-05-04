# Current Boundaries

This file lists what is not fully shipped or should not be overstated. These
are boundaries, not failures.

## Not A Hosted SaaS Yet

BlackBoxFix is currently local/private-deployment oriented. It does not yet
ship:

- public multi-tenant SaaS hosting;
- tenant billing/accounts;
- hosted control-plane operations;
- provider-managed TLS;
- public internet deployment hardening.

The local HTTP server is authenticated and audited, but it is a loopback or
private single-ledger service, not a public hosted API.

## Packaged Language SDKs Are Not Shipped Yet

Phase 24 ships the provider-neutral CloudEvents ingestion contract, connector,
validation, atomic append, and SQLite readback. It does not yet ship packaged
Python/TypeScript/Rust SDK libraries.

External runtimes can still integrate today by emitting the documented
CloudEvents JSON object/batch file and running:

```bash
blackbox sdk ingest --ledger PATH --input events.json
```

## Some Connectors Need Live Credentials Or Captured Artifacts

Hosted-provider connectors may need:

- GitHub token;
- GitLab token or public project access;
- rate-limit availability;
- real provider state.

Artifact connectors need physical JSON/JSONL/files captured from their real
systems.

## No Migration Shims

The current ledger schema is version 13. The implementation intentionally
wipes old ledgers on init rather than providing compatibility migrations. This
matches the phase doctrine, but it means old experimental ledgers should not
be treated as production-upgradeable stores.

## OpenTelemetry Is Export-Only

OpenTelemetry export creates trace/log/metric files from verified ledger
state. OpenTelemetry is not the authoritative store. The SQLite ledger remains
the source of truth.

## Verification Scope Is Connector-Bound

BlackBoxFix can only verify sources for which a connector/verifier exists.
Unsupported systems should be added as explicit connectors rather than
handled by ad hoc string parsing or mocked success.

## No Mock-Backed Success Standard

The shipped FSV doctrine rejects tests that pass while the real source of
truth is broken. New capabilities should follow the same pattern: physical
input, command execution, separate readback, edge cases, and durable proof.

## Current Strongest Use Cases

The current system is strongest for:

- local/private agent governance;
- verified code-change workflows;
- CI/scanner/ticket evidence capture;
- infrastructure artifact verification;
- buyer proof packages;
- release/risk/privacy governance;
- internal audit and FSV certification;
- SDK/runtime event capture through files.

## Current Weakest Fit

The current system is not yet the right shape for:

- public multi-customer SaaS operation;
- high-throughput distributed ingestion without an external queue;
- packaged SDK ergonomics without a thin wrapper around CloudEvents files;
- arbitrary third-party systems with no connector;
- replacing existing observability tools as a primary telemetry backend.
