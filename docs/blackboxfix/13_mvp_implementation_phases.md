# MVP Implementation Phases

The MVP should prove source-of-truth verification and training/eval export
before expanding into many connectors.

## Phase 0: Product Foundation

Deliverables:

- planning docs;
- ledger schema;
- verifier spec format;
- connector contract;
- first wedge demo design;
- threat model;
- acceptance criteria.

Exit criteria:

- every core record has a schema;
- first workflow has sources of truth defined;
- first three connectors are selected.

## Phase 1: Local Ledger And CLI

Deliverables:

- local append-only ledger;
- `blackbox init`;
- `blackbox record run`;
- `blackbox record tool-call`;
- `blackbox inspect ledger`;
- hash chain;
- redaction policy v1.

Exit criteria:

- manual FSV proves records persist in the ledger;
- hash chain verification catches tampering;
- redaction failure blocks persistence.

## Phase 2: Git And File Verification

Deliverables:

- local Git connector;
- file readback connector;
- test command evidence capture;
- verifiers for commit exists, diff matches, working tree clean, command passed.

Exit criteria:

- agent/code workflow can be recorded and verified locally;
- three edge cases per mutating verifier;
- failed verifiers produce typed errors.

## Phase 3: GitHub And CI Connectors

Deliverables:

- GitHub PR/checks/issues connector;
- CI readback connector for GitHub Actions first;
- verifiers for PR state, branch head, check suite, issue state.

Exit criteria:

- vulnerability remediation demo verifies GitHub and CI state;
- permission failures are `blocked`;
- stale readback and missing PR edge cases are covered.

## Phase 4: Evidence Packets And Replay

Deliverables:

- evidence packet builder;
- forensic replay timeline;
- incident packet generation;
- JSON/PDF-ready audit export.

Exit criteria:

- failed verifier produces incident packet;
- replay reconstructs full run without mutation;
- evidence hashes verify against ledger records.

## Phase 5: Training/Eval Exporter

Deliverables:

- `blackbox export episodes`;
- manifest;
- JSONL row validation;
- eval fixture generation for failure classes.

Exit criteria:

- exported JSONL parses line by line;
- manifest hash matches file;
- every row links back to ledger evidence.

## Phase 6: MCP Gateway

Deliverables:

- recorder MCP server;
- verified tool wrappers for first workflow;
- strict schemas;
- capability registry;
- policy modes.

Exit criteria:

- an MCP-compatible agent can run through the gateway;
- every mutating MCP tool returns verifier evidence;
- governed mode blocks unverified mutating tools.

## Phase 7: Team Dashboard

Deliverables:

- run timeline;
- evidence viewer;
- verifier status dashboard;
- error-code drilldown;
- export UI.

Exit criteria:

- user can inspect a passed run and a failed run;
- user can export evidence and episodes;
- source-of-truth links are visible.

## Phase 8: Enterprise Controls

Deliverables:

- SSO;
- RBAC;
- tenant isolation;
- SIEM export;
- retention policies;
- private deployment path.

Exit criteria:

- enterprise pilot can run in a controlled environment;
- audit export satisfies security review needs.

