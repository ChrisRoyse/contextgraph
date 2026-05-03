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
- Phase 7 remains local and read-only for dashboard generation. A hosted
  writer service with a single SQLite/Postgres write path per tenant remains
  for the shared-ledger deployment.

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
- provide generic HTTP/SDK ingestion.

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

## Open Questions (still open)

- Which hosted CI connector should follow GitLab CI?
  Candidates: Buildkite and CircleCI. Choose based on first paid pilot CI
  footprint and availability of public read-only fixtures for live FSV.
- What is the minimum dashboard required for a paid pilot?
  Hypothesis: timeline + filter + evidence viewer + JSONL download. Validate
  with two pilot prospects before building.
- How much raw model text should be stored by default?
  Hypothesis: redacted prompt + redacted response, truncated at 8 KB. Track
  whether buyers consistently ask for un-truncated raw text in regulated
  tiers.
- What is the best customer-managed key path for enterprise?
  Likely AWS KMS / GCP KMS / HashiCorp Vault — choose based on first
  enterprise pilot's environment.
- Which eval format should be exported first beyond JSONL?
  Candidates: Parquet (we already ship this for `training-corpus`), HuggingFace
  Datasets format. Default to Parquet to reuse the
  `crates/context-graph-cli/src/commands/export_training.rs` wiring.
