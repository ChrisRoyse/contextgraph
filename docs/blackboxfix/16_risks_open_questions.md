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

Mitigation (already partly shipped in Phase 1):

- redaction before persistence (`redact_v1`, post-scan);
- field-level classification (Phase 8);
- encrypted artifacts (Phase 8);
- tenant keys (Phase 8);
- export approval workflow (Phase 8).

### Verifier Gaps

Risk: users expect proof where no source of truth is available.

Mitigation:

- explicitly mark unverified actions;
- block unverified mutating actions in governed mode (Phase 6);
- make source-of-truth coverage a dashboard metric (Phase 7).

### Evidence Tampering

Risk: audit evidence loses value if mutable.

Mitigation (Phase 1 baseline):

- append-only triggers (`ledger_records_no_update`, `ledger_records_no_delete`);
- dual hash chain (per-record + per-run);
- `BBX_HASH_CHAIN_BROKEN` aborts further writes;
- artifact hashes (Phase 4);
- signed export manifests (Phase 5).

### SQLite Concurrency

Risk: multiple processes writing the same ledger; WAL contention.

Mitigation:

- `busy_timeout=5000`, `synchronous=FULL`, WAL journaling (already enabled);
- recommend one writer per ledger file in Phase 1;
- Phase 7 introduces a writer service with a single SQLite connection
  (or moves to Postgres).

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
- delay broad connector marketplace until Phase 8.

### Enterprise Sales Cycle

Risk: long procurement slows learning.

Mitigation:

- local developer recorder (Phase 1 today);
- team-tier SaaS pilot (Phase 7);
- security review packet;
- private deployment path only after wedge validation (Phase 8).

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

## Open Questions (still open)

- Which hosted CI connector should come after GitHub Actions?
  Candidates: Buildkite, CircleCI, GitLab CI. Decide before Phase 3 closes.
- Should evidence packets be signed in Phase 4 or Phase 5?
  Signing in Phase 4 lets the exporter copy a signed bundle directly; the
  signing-key infrastructure work may slip Phase 5. Defer the call until
  Phase 4 design.
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
