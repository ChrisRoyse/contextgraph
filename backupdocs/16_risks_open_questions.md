# Risks And Open Questions

## Technical Risks

### Connector Sprawl

Risk: every customer needs different systems.

Mitigation:

- start with DevOps/SecOps connectors;
- define connector contract early (`05_source_of_truth_connectors.md`);
- make readback specs modular;
- support customer-authored connector plugins later.

### Replay Nondeterminism

Risk: many agent actions cannot be deterministically replayed.

Mitigation:

- make forensic replay first-class (Phase 4 default);
- reserve deterministic replay for sandboxable actions;
- persist source-of-truth snapshots and artifact hashes.

### Sensitive Data Exposure

Risk: agent traces can contain secrets, PII, customer data.

Mitigation:

- redaction before persistence (`redact_v1`, post-scan);
- raw credential material refused in Phase 8 enterprise-control input;
- tenant customer-managed key ids persisted in Phase 8;
- export approval workflow remains open for later enterprise workflow work;
- field-level classification and encrypted artifact storage remain later
  storage-layer work.

### Verifier Gaps

Risk: users expect proof where no source of truth is available.

Mitigation:

- explicitly mark unverified actions;
- block unverified mutating actions in governed mode (Phase 6);
- make source-of-truth coverage a dashboard metric (shipped in Phase 7).

### Evidence Tampering

Risk: audit evidence loses value if mutable.

Mitigation (Phase 1 baseline):

- append-only triggers (`ledger_records_no_update`, `ledger_records_no_delete`);
- dual hash chain (per-record + per-run);
- `BBX_HASH_CHAIN_BROKEN` aborts further writes;
- evidence packet row-reference hashes and export file hashes (Phase 4);
- signed export manifests (later key-management work);
- audit/SIEM export manifests with physical file hashes (Phase 8).

### SQLite Concurrency

Risk: multiple processes writing the same ledger; WAL contention.

Mitigation:

- `busy_timeout=5000`, `synchronous=FULL`, WAL journaling (already enabled);
- recommend one writer per ledger file in Phase 1;
- Phase 17 ships a loopback authenticated HTTP writer with a single in-process
  ledger lock and durable `http_request_audit` rows. Public multi-tenant SaaS
  serving and Postgres-backed tenancy remain later work.

## Product Risks

### Looks Like Generic Observability

Risk: buyers compare it to trace tools.

Mitigation:

- lead with verified outcomes and audit packets;
- show source-of-truth readback in every demo;
- export eval/training data as a second differentiator.

### Too Much Platform Too Early

Risk: building every connector and dashboard before proving wedge.

Mitigation:

- ship one end-to-end workflow first;
- use local CLI and GitHub/CI before adding more sources;
- delay broad connector marketplace until after enterprise controls are proven.

### Enterprise Sales Cycle

Risk: long procurement slows learning.

Mitigation:

- local developer recorder (Phase 1);
- team-tier SaaS pilot (Phase 7);
- security review packet;
- private deployment path metadata shipped in Phase 8; provider-specific
  deployment automation remains pilot-driven.

## Business Risks

### Agent Framework Fragmentation

Risk: runtimes change quickly.

Mitigation:

- instrument at the tool boundary;
- support MCP gateway (Phase 6);
- provide generic HTTP ingestion (Phase 17 shipped the first authenticated
  server) and CloudEvents SDK ingestion (Phase 24).

### Model Provider Platform Moves

Risk: model providers add tracing and evals.

Mitigation:

- remain provider-neutral;
- focus on source-of-truth verification across systems;
- own the evidence and connector layer.

## Resolved Open Questions

- **Should the first implementation live under `context-graph blackbox` or
  as a standalone `blackbox` binary?**
  → **Both.** Phase 1 ships both binaries from the same code path
  (`crates/context-graph-cli/src/bin/{blackbox,context-graph}.rs` calling
  `commands::blackbox`). Operators choose; no divergence allowed.
- **Should evidence packets be signed in Phase 4 or Phase 5?**
  → **Not Phase 4.** Phase 4 ships unsigned, hash-verified evidence packets
  and JSON exports. Signing needs key-management and envelope decisions and
  moves to Phase 5+.
- **Which hosted CI connector should come after GitHub Actions?**
  → **GitLab CI first.** Phase 9 ships `gitlab_ci_v1` because GitLab exposes
  public, read-only project/pipeline/job source-of-truth endpoints suitable for
  live FSV without test tokens. Buildkite and CircleCI remain candidates for a
  later hosted-CI connector phase.
- **How are remaining Phase 16 risks and open questions closed?**
  → **Ledger-backed risk assessment.** Phase 16 ships
  `blackbox risk evaluate`, connector `blackbox_risk_v1`, dashboard
  `api/risk-assessments.json`, and ledger record kind `risk_assessment`. A
  required decision-register JSON file must close every tracked open question
  before an accepted risk posture can be persisted. Blocked posture is also
  durable evidence and exits non-zero.

## Phase 16 Required Decision Register

The following questions are no longer prose-only. They are required ids in
the Phase 16 decision-register JSON file consumed by `blackbox risk evaluate`.
Each id must appear exactly once, must be `resolved`, `accepted`, or
`deferred`, must have a future `review_due`, and must include owner,
decision, rationale, review trigger, evidence, severity, and secret-free
bounded text.

| Required id | Default Phase 16 decision |
|---|---|
| `next_hosted_ci_connector` | Defer Buildkite/CircleCI selection until first paid-pilot CI footprint and available public read-only fixtures are known. |
| `paid_pilot_dashboard_minimum` | Use the shipped dashboard timeline, evidence, error, package, and risk API files as the minimum paid-pilot surface. |
| `raw_model_text_storage_default` | Store redacted/truncated model text only; raw untruncated text requires a later explicit enterprise storage/key decision. |
| `customer_managed_key_path` | Keep tenant key ids and customer-managed metadata in enterprise config; provider-specific KMS/Vault automation remains pilot-driven. |
| `eval_export_format_after_jsonl` | Keep JSONL/package outputs as shipped and prefer Parquet next because the repository already contains training-corpus export wiring for it. |

## Open Questions (tracked by risk assessment)

There are no untracked Phase 16 open questions. New questions must be added as
explicit required decision ids and enforced by `blackbox_risk_v1`; otherwise
they are documentation-only and do not satisfy FSV.
