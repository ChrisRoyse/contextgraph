# Risks And Open Questions

## Technical Risks

### Connector Sprawl

Risk: every customer needs different systems.

Mitigation:

- start with DevOps/SecOps connectors;
- define connector contract early;
- make readback specs modular;
- support customer-authored connector plugins later.

### Replay Nondeterminism

Risk: many agent actions cannot be deterministically replayed.

Mitigation:

- make forensic replay first-class;
- reserve deterministic replay for sandboxable actions;
- persist source-of-truth snapshots and artifact hashes.

### Sensitive Data Exposure

Risk: agent traces can contain secrets, PII, and customer data.

Mitigation:

- redaction before persistence;
- field-level classification;
- encrypted artifacts;
- tenant keys;
- export approval.

### Verifier Gaps

Risk: users expect proof where no source of truth is available.

Mitigation:

- explicitly mark unverified actions;
- block unverified mutating actions in governed mode;
- make source-of-truth coverage a dashboard metric.

### Evidence Tampering

Risk: audit evidence loses value if mutable.

Mitigation:

- append-only records;
- hash chains;
- artifact hashes;
- signed export manifests.

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

- ship one end-to-end workflow;
- use local CLI and GitHub/CI first;
- delay broad connector marketplace.

### Enterprise Sales Cycle

Risk: long procurement slows learning.

Mitigation:

- local developer recorder;
- team-tier SaaS pilot;
- security review packet;
- private deployment path only after wedge validation.

## Business Risks

### Agent Framework Fragmentation

Risk: runtimes change quickly.

Mitigation:

- instrument at tool boundary;
- support MCP gateway;
- provide generic HTTP/SDK ingestion.

### Model Provider Platform Moves

Risk: model providers add tracing and evals.

Mitigation:

- remain provider-neutral;
- focus on source-of-truth verification across systems;
- own the evidence and connector layer.

## Open Questions

- Should the first implementation live under `context-graph blackbox` or as a
  standalone `blackbox` binary?
- Which hosted CI connector should come after GitHub Actions?
- Should evidence packets be signed in MVP or Phase 4?
- What is the minimum dashboard required for a paid pilot?
- How much raw model text should be stored by default?
- What is the best customer-managed key path for enterprise?
- Which eval format should be exported first beyond JSONL?

